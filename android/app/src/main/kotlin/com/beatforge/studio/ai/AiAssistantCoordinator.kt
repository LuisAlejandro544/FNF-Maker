// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ai/AiAssistantCoordinator.kt
// Propósito: Orquestador agéntico del Asistente IA en `Dispatchers.IO`. Combina el System Prompt
// personalizable, los `SKILL.md` activos, el historial conversacional, el streaming SSE de OpenRouter
// y la ejecución automática de herramientas (lectura de scripts, parches parciales, sprites sin
// imágenes y tiempos del chart).

package com.beatforge.studio.ai

import android.app.Application
import com.beatforge.studio.ProjectSnapshot
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import java.util.UUID

class AiAssistantCoordinator(
    private val app: Application,
    private val scope: CoroutineScope,
    private val engineIoDispatcher: CoroutineDispatcher,
    private val getSnapshot: () -> ProjectSnapshot,
    private val getSelectedScriptId: () -> String,
    private val onSnapshotMutated: (ProjectSnapshot, selectedScriptId: String?) -> Unit
) {
    private val prefsStore = AiPreferencesStore(app)
    private val skillsRepo = AiSkillsRepository(app)
    private val sseClient = OpenRouterSseClient()

    private val _aiState = MutableStateFlow(AiAssistantUiState())
    val aiState: StateFlow<AiAssistantUiState> = _aiState.asStateFlow()

    private val patchEngine = AiPatchEngine(
        getSnapshot = getSnapshot,
        getSkills = { _aiState.value.skills },
        onSnapshotMutated = onSnapshotMutated
    )

    private var activeJob: Job? = null

    fun initialize() {
        if (_aiState.value.loaded) return
        scope.launch(Dispatchers.IO) {
            val settings = prefsStore.load()
            val skills = skillsRepo.loadBundledSkills(settings.disabledSkillIds)
            _aiState.update {
                it.copy(
                    loaded = true,
                    settings = settings,
                    skills = skills
                )
            }
        }
    }

    fun updateApiKey(apiKey: String) = updateSettings { it.copy(apiKey = apiKey) }

    fun selectModel(modelId: String) = updateSettings {
        it.copy(selectedModel = modelId.trim().ifBlank { DEFAULT_OPENROUTER_MODEL })
    }

    fun addCustomModel(modelId: String) {
        val clean = modelId.trim()
        if (clean.isEmpty()) return
        updateSettings { cur ->
            val nextList = if (cur.customModels.contains(clean) ||
                OPENROUTER_PRESET_MODELS.any { it.id == clean }
            ) {
                cur.customModels
            } else {
                cur.customModels + clean
            }
            cur.copy(selectedModel = clean, customModels = nextList)
        }
    }

    fun removeCustomModel(modelId: String) {
        updateSettings { cur ->
            val nextCustom = cur.customModels.filterNot { it == modelId }
            val nextSelected = if (cur.selectedModel == modelId) DEFAULT_OPENROUTER_MODEL else cur.selectedModel
            cur.copy(customModels = nextCustom, selectedModel = nextSelected)
        }
    }

    fun updateSystemPrompt(prompt: String) = updateSettings { it.copy(systemPrompt = prompt) }

    fun restoreDefaultSystemPrompt() = updateSettings { it.copy(systemPrompt = DEFAULT_AI_SYSTEM_PROMPT) }

    fun toggleSkillEnabled(skillId: String, enabled: Boolean) {
        scope.launch(Dispatchers.IO) {
            val cur = _aiState.value.settings
            val nextDisabled = if (enabled) {
                cur.disabledSkillIds - skillId
            } else {
                cur.disabledSkillIds + skillId
            }
            val nextSettings = cur.copy(disabledSkillIds = nextDisabled)
            prefsStore.save(nextSettings)
            val nextSkills = _aiState.value.skills.map { sk ->
                if (sk.id == skillId) sk.copy(enabled = enabled) else sk
            }
            _aiState.update { it.copy(settings = nextSettings, skills = nextSkills) }
        }
    }

    fun clearConversation() {
        stopStreaming()
        _aiState.update { it.copy(messages = emptyList(), activeStatus = "") }
    }

    fun stopStreaming() {
        sseClient.cancelActiveStream()
        activeJob?.cancel()
        activeJob = null
        _aiState.update { st ->
            st.copy(
                isStreaming = false,
                activeStatus = "",
                messages = st.messages.map { m -> if (m.isStreaming) m.copy(isStreaming = false) else m }
            )
        }
    }

    fun sendUserMessage(rawPrompt: String) {
        val promptText = rawPrompt.trim()
        if (promptText.isEmpty() || _aiState.value.isStreaming) return

        val settings = _aiState.value.settings
        if (settings.apiKey.isBlank()) {
            val userMsg = AiChatMessage(id = UUID.randomUUID().toString(), role = "user", content = promptText)
            val errMsg = AiChatMessage(
                id = UUID.randomUUID().toString(),
                role = "assistant",
                content = "",
                errorMessage = "Configura primero tu API Key de OpenRouter en la pestaña 'Configuración y Skills'."
            )
            _aiState.update { it.copy(messages = it.messages + userMsg + errMsg) }
            return
        }

        val userMsg = AiChatMessage(
            id = UUID.randomUUID().toString(),
            role = "user",
            content = promptText
        )
        val assistantMsgId = UUID.randomUUID().toString()
        val placeholderAssistant = AiChatMessage(
            id = assistantMsgId,
            role = "assistant",
            content = "",
            isStreaming = true
        )

        val priorHistory = _aiState.value.messages
        _aiState.update {
            it.copy(
                messages = it.messages + userMsg + placeholderAssistant,
                isStreaming = true,
                activeStatus = "Conectando con ${settings.selectedModel}..."
            )
        }

        activeJob = scope.launch(engineIoDispatcher) {
            try {
                runAgentLoop(
                    settings = settings,
                    priorHistory = priorHistory + userMsg,
                    assistantMsgId = assistantMsgId
                )
            } catch (e: Exception) {
                _aiState.update { st ->
                    st.copy(
                        isStreaming = false,
                        activeStatus = "",
                        messages = st.messages.map { m ->
                            if (m.id == assistantMsgId) {
                                m.copy(
                                    isStreaming = false,
                                    errorMessage = e.message ?: "Error inesperado en el Asistente IA"
                                )
                            } else m
                        }
                    )
                }
            }
        }
    }

    private fun runAgentLoop(
        settings: AiSettings,
        priorHistory: List<AiChatMessage>,
        assistantMsgId: String
    ) {
        val toolsSchema = patchEngine.buildToolsSchema()
        val conversation = JSONArray().apply {
            put(
                JSONObject().apply {
                    put("role", "system")
                    put("content", buildFullSystemPrompt(settings))
                }
            )
            priorHistory.takeLast(14).forEach { msg ->
                if (msg.content.isNotBlank()) {
                    put(
                        JSONObject().apply {
                            put("role", msg.role)
                            put("content", msg.content)
                        }
                    )
                }
            }
        }

        val collectedTools = mutableListOf<AiToolCallRecord>()
        val accumulatedDisplayText = StringBuilder()

        val maxToolRounds = 8
        for (round in 0 until maxToolRounds) {
            val turnResult = sseClient.streamCompletionTurn(
                apiKey = settings.apiKey,
                model = settings.selectedModel,
                messages = conversation,
                tools = toolsSchema,
                onTokenDelta = { delta ->
                    accumulatedDisplayText.append(delta)
                    val currentText = accumulatedDisplayText.toString()
                    _aiState.update { st ->
                        st.copy(
                            activeStatus = "Escribiendo respuesta...",
                            messages = st.messages.map { m ->
                                if (m.id == assistantMsgId) m.copy(content = currentText) else m
                            }
                        )
                    }
                }
            )

            if (turnResult.errorMessage != null) {
                _aiState.update { st ->
                    st.copy(
                        isStreaming = false,
                        activeStatus = "",
                        messages = st.messages.map { m ->
                            if (m.id == assistantMsgId) {
                                m.copy(
                                    isStreaming = false,
                                    errorMessage = turnResult.errorMessage
                                )
                            } else m
                        }
                    )
                }
                return
            }

            if (turnResult.toolCalls.isEmpty()) {
                _aiState.update { st ->
                    st.copy(
                        isStreaming = false,
                        activeStatus = "",
                        messages = st.messages.map { m ->
                            if (m.id == assistantMsgId) {
                                m.copy(
                                    content = accumulatedDisplayText.toString(),
                                    toolCalls = collectedTools.toList(),
                                    isStreaming = false
                                )
                            } else m
                        }
                    )
                }
                return
            }

            // Registrar turno del asistente con sus tool_calls
            val tcArrayJson = JSONArray()
            turnResult.toolCalls.forEach { tc ->
                tcArrayJson.put(
                    JSONObject().apply {
                        put("id", tc.id)
                        put("type", "function")
                        put(
                            "function",
                            JSONObject().apply {
                                put("name", tc.name)
                                put("arguments", tc.argumentsJson)
                            }
                        )
                    }
                )
            }
            conversation.put(
                JSONObject().apply {
                    put("role", "assistant")
                    put("content", if (turnResult.assistantText.isBlank()) JSONObject.NULL else turnResult.assistantText)
                    put("tool_calls", tcArrayJson)
                }
            )

            // Ejecutar herramientas localmente y alimentar resultados al modelo
            turnResult.toolCalls.forEach { tc ->
                _aiState.update { it.copy(activeStatus = "Ejecutando herramienta: ${tc.name}...") }
                val execOut = patchEngine.executeTool(tc.name, tc.argumentsJson)
                val record = AiToolCallRecord(
                    id = tc.id,
                    toolName = tc.name,
                    argumentsSummary = tc.argumentsJson.take(120),
                    resultSummary = execOut.summary,
                    ok = execOut.ok
                )
                collectedTools.add(record)

                _aiState.update { st ->
                    st.copy(
                        messages = st.messages.map { m ->
                            if (m.id == assistantMsgId) {
                                m.copy(toolCalls = collectedTools.toList())
                            } else m
                        }
                    )
                }

                conversation.put(
                    JSONObject().apply {
                        put("role", "tool")
                        put("tool_call_id", tc.id)
                        put("content", execOut.toolResultJson)
                    }
                )
            }

            if (turnResult.assistantText.isNotBlank() && round < maxToolRounds - 1) {
                accumulatedDisplayText.append("\n\n")
            }
        }

        _aiState.update { st ->
            st.copy(
                isStreaming = false,
                activeStatus = "",
                messages = st.messages.map { m ->
                    if (m.id == assistantMsgId) {
                        m.copy(
                            content = accumulatedDisplayText.toString().ifBlank {
                                "Se completaron las operaciones solicitadas sobre el proyecto."
                            },
                            toolCalls = collectedTools.toList(),
                            isStreaming = false
                        )
                    } else m
                }
            )
        }
    }

    private fun buildFullSystemPrompt(settings: AiSettings): String {
        val snap = getSnapshot()
        val activeScript = snap.scripts.find { it.id == getSelectedScriptId() } ?: snap.scripts.firstOrNull()
        val enabledSkills = _aiState.value.skills.filter { it.enabled }

        return buildString {
            append(settings.systemPrompt.trim())
            append("\n\n## Contexto actual del Mod\n")
            append("- Proyecto: ").append(snap.name)
                .append(" (API de desarrollo v").append(snap.devApiVersion)
                .append(" / vigente v").append(snap.currentDevApiVersion).append(")\n")
            append("- Chart: ").append(snap.chart.title)
                .append(" · ").append(snap.chart.bpm).append(" BPM · ")
                .append(snap.chart.sectionCount).append(" secciones\n")
            if (activeScript != null) {
                append("- Script abierto actualmente en editor: `")
                    .append(activeScript.name).append("` (id=`").append(activeScript.id)
                    .append("`, lenguaje=").append(activeScript.language).append(")\n")
            }
            if (enabledSkills.isNotEmpty()) {
                append("\n## Skills modulares de BeatForge Studio activos\n")
                enabledSkills.forEach { sk ->
                    append("\n### Skill [").append(sk.id).append("]: ").append(sk.title).append("\n")
                    append(sk.content).append("\n")
                }
            }
        }
    }

    private fun updateSettings(transform: (AiSettings) -> AiSettings) {
        scope.launch(Dispatchers.IO) {
            val next = transform(_aiState.value.settings)
            prefsStore.save(next)
            _aiState.update { it.copy(settings = next) }
        }
    }
}
