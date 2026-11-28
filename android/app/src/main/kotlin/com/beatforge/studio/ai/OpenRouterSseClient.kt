// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ai/OpenRouterSseClient.kt
// Propósito: Cliente HTTP SSE streaming (OkHttp + okhttp-sse, licencia Apache-2.0) compatible con
// OpenRouter (`/api/v1/chat/completions`), incluyendo streaming incremental de tokens y acumulación
// fragmentada de `tool_calls` para invocación de herramientas en tiempo real.

package com.beatforge.studio.ai

import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import okhttp3.sse.EventSource
import okhttp3.sse.EventSourceListener
import okhttp3.sse.EventSources
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

data class OpenRouterToolCall(
    val id: String,
    val name: String,
    val argumentsJson: String
)

data class OpenRouterStreamTurnResult(
    val assistantText: String,
    val toolCalls: List<OpenRouterToolCall>,
    val finishReason: String?,
    val errorMessage: String? = null
)

class OpenRouterSseClient {

    private val httpClient = OkHttpClient.Builder()
        .connectTimeout(25, TimeUnit.SECONDS)
        .readTimeout(120, TimeUnit.SECONDS)
        .writeTimeout(25, TimeUnit.SECONDS)
        .build()

    @Volatile
    private var activeEventSource: EventSource? = null

    fun cancelActiveStream() {
        activeEventSource?.cancel()
        activeEventSource = null
    }

    fun streamCompletionTurn(
        apiKey: String,
        model: String,
        messages: JSONArray,
        tools: JSONArray,
        onTokenDelta: (String) -> Unit
    ): OpenRouterStreamTurnResult {
        val bodyJson = JSONObject().apply {
            put("model", model.trim().ifBlank { DEFAULT_OPENROUTER_MODEL })
            put("messages", messages)
            put("tools", tools)
            put("tool_choice", "auto")
            put("stream", true)
            put("temperature", 0.25)
        }.toString()

        val request = Request.Builder()
            .url("https://openrouter.ai/api/v1/chat/completions")
            .addHeader("Authorization", "Bearer ${apiKey.trim()}")
            .addHeader("Content-Type", "application/json")
            .addHeader("HTTP-Referer", "https://beatforge.studio")
            .addHeader("X-Title", "BeatForge Studio")
            .post(bodyJson.toRequestBody("application/json; charset=utf-8".toMediaType()))
            .build()

        val textBuffer = StringBuilder()
        val toolAccumulators = linkedMapOf<Int, MutableToolAccumulator>()
        val finishReasonRef = AtomicReference<String?>(null)
        val errorRef = AtomicReference<String?>(null)
        val latch = CountDownLatch(1)

        val listener = object : EventSourceListener() {
            override fun onEvent(
                eventSource: EventSource,
                id: String?,
                type: String?,
                data: String
            ) {
                val trimmed = data.trim()
                if (trimmed.isEmpty()) return
                if (trimmed == "[DONE]") {
                    latch.countDown()
                    return
                }

                try {
                    val obj = JSONObject(trimmed)
                    val errObj = obj.optJSONObject("error")
                    if (errObj != null) {
                        errorRef.set(errObj.optString("message", "Error devuelto por OpenRouter"))
                        latch.countDown()
                        return
                    }

                    val choices = obj.optJSONArray("choices") ?: return
                    if (choices.length() == 0) return
                    val firstChoice = choices.optJSONObject(0) ?: return

                    val fReason = firstChoice.optString("finish_reason", "")
                    if (fReason.isNotBlank() && fReason != "null") {
                        finishReasonRef.set(fReason)
                    }

                    val delta = firstChoice.optJSONObject("delta") ?: return
                    if (delta.has("content") && !delta.isNull("content")) {
                        val contentDelta = delta.optString("content", "")
                        if (contentDelta.isNotEmpty()) {
                            textBuffer.append(contentDelta)
                            onTokenDelta(contentDelta)
                        }
                    }

                    val tcArr = delta.optJSONArray("tool_calls")
                    if (tcArr != null) {
                        for (i in 0 until tcArr.length()) {
                            val tc = tcArr.optJSONObject(i) ?: continue
                            val idx = tc.optInt("index", i)
                            val acc = toolAccumulators.getOrPut(idx) {
                                MutableToolAccumulator(id = "call_$idx")
                            }
                            val tcId = tc.optString("id", "")
                            if (tcId.isNotBlank()) acc.id = tcId
                            val fn = tc.optJSONObject("function")
                            if (fn != null) {
                                val fnName = fn.optString("name", "")
                                if (fnName.isNotBlank()) acc.name.append(fnName)
                                val fnArgs = fn.optString("arguments", "")
                                if (fnArgs.isNotEmpty()) acc.arguments.append(fnArgs)
                            }
                        }
                    }
                } catch (_: Exception) {
                }
            }

            override fun onClosed(eventSource: EventSource) {
                latch.countDown()
            }

            override fun onFailure(
                eventSource: EventSource,
                t: Throwable?,
                response: Response?
            ) {
                val httpDetail = try {
                    response?.body?.string()?.let { body ->
                        val parsed = JSONObject(body)
                        parsed.optJSONObject("error")?.optString("message")?.takeIf { it.isNotBlank() }
                            ?: body.take(240)
                    }
                } catch (_: Exception) {
                    null
                }
                val codeInfo = response?.let { "HTTP ${it.code}" }
                val finalErr = when {
                    !httpDetail.isNullOrBlank() && codeInfo != null -> "$codeInfo: $httpDetail"
                    !httpDetail.isNullOrBlank() -> httpDetail
                    t != null -> t.message ?: "Error de red en streaming OpenRouter"
                    else -> codeInfo ?: "Conexión interrumpida con OpenRouter"
                }
                errorRef.compareAndSet(null, finalErr)
                latch.countDown()
            }
        }

        val factory = EventSources.createFactory(httpClient)
        val es = factory.newEventSource(request, listener)
        activeEventSource = es
        try {
            latch.await()
        } catch (_: InterruptedException) {
            es.cancel()
            errorRef.compareAndSet(null, "Solicitud cancelada")
        } finally {
            activeEventSource = null
            es.cancel()
        }

        val resolvedToolCalls = toolAccumulators.entries
            .sortedBy { it.key }
            .mapNotNull { (_, acc) ->
                val fnName = acc.name.toString().trim()
                if (fnName.isEmpty()) null
                else OpenRouterToolCall(
                    id = acc.id,
                    name = fnName,
                    argumentsJson = acc.arguments.toString().ifBlank { "{}" }
                )
            }

        return OpenRouterStreamTurnResult(
            assistantText = textBuffer.toString(),
            toolCalls = resolvedToolCalls,
            finishReason = finishReasonRef.get(),
            errorMessage = errorRef.get()
        )
    }

    private class MutableToolAccumulator(
        var id: String,
        val name: StringBuilder = StringBuilder(),
        val arguments: StringBuilder = StringBuilder()
    )
}
