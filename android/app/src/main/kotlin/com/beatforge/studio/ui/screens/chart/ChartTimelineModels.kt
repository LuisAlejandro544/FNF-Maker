// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/chart/ChartTimelineModels.kt
// Propósito: Modelos puros y constructor de línea de tiempo para el Editor de Chart:
// resuelve notas sostenidas continuas (cabeza, cuerpo y punta final incluso cruzando
// secciones), mapea eventos de `chart.json` a cada paso `#` y calcula spans de flashes.

package com.beatforge.studio.ui.screens.chart

import androidx.compose.ui.graphics.Color
import com.beatforge.studio.ChartEventSnapshot
import com.beatforge.studio.ChartSectionSnapshot
import com.beatforge.studio.ChartSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.VisibleNote
import kotlin.math.abs
import kotlin.math.roundToInt

enum class SustainSegmentKind {
    None,
    Head,
    Body,
    Tail
}

data class ChartSustainPass(
    val direction: Int,
    val kind: SustainSegmentKind,
    val isPlayer: Boolean
)

data class IndexedChartEvent(
    val globalIndex: Int,
    val event: ChartEventSnapshot
)

enum class StepCueKind {
    Camera,
    Vignette,
    Swap,
    Flash,
    Character,
    Preload,
    UiBlackout,
    Other
}

sealed class ChartTimelineRow {
    data class SectionHeader(
        val section: ChartSectionSnapshot,
        val crossingCols: Set<Int>,
        val sectionCueCount: Int
    ) : ChartTimelineRow()

    data class StepRow(
        val section: ChartSectionSnapshot,
        val globalStepIndex: Int,
        val stepInSec: Int,
        val stepTimeMs: Double,
        val stepMs: Double,
        val rowNotesByCol: Map<Int, VisibleNote>,
        val sustainPassesByCol: Map<Int, ChartSustainPass>,
        val rowEvents: List<IndexedChartEvent>,
        val eventKinds: List<StepCueKind>,
        val activeFlashColor: Color?,
        val activeBlackoutAlpha: Float = 0f,
        val activeVignetteColor: Color? = null
    ) : ChartTimelineRow()
}

data class ChartPlayheadTarget(
    val rowIndex: Int,
    val stepRow: ChartTimelineRow.StepRow,
    val stepFraction: Float
)

data class ChartStepLookupEntry(
    val rowIndex: Int,
    val stepRow: ChartTimelineRow.StepRow
)

private data class TempStepEntry(
    val section: ChartSectionSnapshot,
    val globalStepIndex: Int,
    val stepInSec: Int,
    val stepTimeMs: Double,
    val stepMs: Double,
    val notesByCol: MutableMap<Int, VisibleNote> = LinkedHashMap(),
    val sustainByCol: MutableMap<Int, ChartSustainPass> = LinkedHashMap(),
    val events: MutableList<IndexedChartEvent> = ArrayList(),
    var activeFlashColor: Color? = null,
    var activeBlackoutAlpha: Float = 0f,
    var activeVignetteColor: Color? = null
)

private data class FlashSpan(
    val startMs: Double,
    val endMs: Double,
    val color: Color
)

private data class BlackoutSpan(
    val startMs: Double,
    val endMs: Double,
    val alpha: Float
)

private data class VignetteSpan(
    val startMs: Double,
    val endMs: Double,
    val color: Color
)

fun classifyChartEvent(event: ChartEventSnapshot): StepCueKind {
    val lower = event.name.trim().lowercase()
    return when {
        lower == "switch camera" ||
            lower == "camera slot" ||
            lower == "set camera" ||
            lower == "cambiar camara" ||
            lower == "cambiar cámara" ||
            lower == "focuscamera" ||
            lower == "focus camera" ||
            lower == "zoomcamera" ||
            lower == "zoom camera" ||
            lower == "camera follow pos" ||
            lower == "camerafollowpos" ||
            lower == "add camera zoom" ||
            lower == "addcamera zoom" ||
            lower == "add cam zoom" ||
            lower == "camera movement" ||
            lower == "cameramovement" -> StepCueKind.Camera
        lower == "camera vignette" ||
            lower == "side glow" ||
            lower == "vignette" ||
            lower == "camera side glow" ||
            lower == "brillo lateral" ||
            lower == "viñeta camara" ||
            lower == "viñeta cámara" ||
            lower == "viñeta" -> StepCueKind.Vignette
        lower == "flash effect" || lower == "flash hud" || lower == "flash camera" || lower == "camera flash" ->
            StepCueKind.Flash
        lower == "preload sprite" -> StepCueKind.Preload
        lower == "change character" -> StepCueKind.Character
        lower == "ui blackout" ||
            lower == "blackout" ||
            lower == "screen fade" ||
            lower == "camera fade" ||
            lower == "hide ui" ||
            lower == "show ui" ||
            lower == "toggle ui" ||
            lower == "fade ui" ||
            lower == "hud fade" ||
            lower == "hide hud" ||
            lower == "show hud" ||
            lower == "hide arrows" ||
            lower == "show arrows" ||
            lower == "toggle arrows" ||
            lower == "fade arrows" ||
            lower == "arrows fade" ||
            lower == "hide strums" ||
            lower == "show strums" ||
            lower == "ocultar flechas" ||
            lower == "mostrar flechas" -> StepCueKind.UiBlackout
        lower == "stage swap" || lower == "swap stage" -> {
            val v1Lower = event.value1.lowercase()
            if (v1Lower.contains("player=") || v1Lower.contains("opp=") || v1Lower.contains("opponent=")) {
                StepCueKind.Character
            } else {
                StepCueKind.Swap
            }
        }
        else -> StepCueKind.Other
    }
}

fun summarizeChartEvent(event: ChartEventSnapshot): String {
    val lower = event.name.trim().lowercase()
    return when (classifyChartEvent(event)) {
        StepCueKind.Camera -> {
            when {
                lower == "camera follow pos" || lower == "camerafollowpos" -> {
                    if (event.value1.isBlank() && event.value2.isBlank()) {
                        "Cámara · Liberar posición fija"
                    } else {
                        "Cámara Pos Fija (${event.value1}, ${event.value2})"
                    }
                }
                lower == "add camera zoom" || lower == "addcamera zoom" || lower == "add cam zoom" -> {
                    val camAdd = event.value1.ifBlank { "0.015" }
                    val hudAdd = event.value2.ifBlank { "0.03" }
                    "Pulso Zoom · Cam +$camAdd / HUD +$hudAdd"
                }
                lower == "zoomcamera" || lower == "zoom camera" -> {
                    val zMatch = Regex("\"zoom\"\\s*:\\s*([0-9.]+)").find(event.value1)?.groupValues?.get(1)
                    val zoomTxt = zMatch ?: event.value1.ifBlank { "1.0" }
                    "Zoom Cámara → ${zoomTxt}x"
                }
                lower == "focuscamera" || lower == "focus camera" -> {
                    "FocusCamera: ${event.value1.take(36)}"
                }
                else -> {
                    val params = parseSemiKv(event.value1)
                    val camIdx = params["cam"] ?: params["slot"] ?: event.value1.trim()
                    val zoomTxt = params["zoom"] ?: params["fov"]
                    val durTxt = params["dur"] ?: params["duration"]
                    val extras = buildList {
                        if (!zoomTxt.isNullOrBlank()) add("${zoomTxt}x")
                        if (!durTxt.isNullOrBlank() && durTxt != "0.00" && durTxt != "0") add("${durTxt}s")
                    }
                    if (extras.isEmpty()) "Cámara → Cam $camIdx" else "Cámara → Cam $camIdx (${extras.joinToString(", ")})"
                }
            }
        }
        StepCueKind.Vignette -> {
            val v1 = event.value1.trim()
            val v1Lower = v1.lowercase()
            val params = parseSemiKv(event.value2)
            val action = params["action"]?.lowercase() ?: v1Lower
            val off = action == "off" || action == "hide" || action == "clear" || action == "none" ||
                params["enabled"] == "0" || params["enabled"] == "false" ||
                (params["alpha"]?.toFloatOrNull()?.let { it <= 0.01f } == true)
            val dur = params["dur"] ?: params["duration"] ?: "0.45"
            if (off) {
                "Viñeta Cámara OFF · ${dur}s"
            } else {
                val colorTag = params["color"] ?: v1.takeIf { it.startsWith("#") || it.isNotBlank() } ?: "#EF4444"
                val alphaPct = ((params["alpha"]?.toFloatOrNull() ?: 0.58f) * 100f).roundToInt()
                val layerTag = if (params["hud"] == "1" || params["hud"] == "true") "HUD" else "Escena"
                "Viñeta $colorTag ($alphaPct%) · ${dur}s ($layerTag)"
            }
        }
        StepCueKind.Flash -> {
            val col = event.value1.ifBlank { "#FFFFFF" }
            val params = parseSemiKv(event.value2)
            val overHud = params["hud"] == "1" || params["hud"] == "true" || lower == "flash hud"
            val dur = params["dur"] ?: params["duration"] ?: "0.65"
            "Flash $col · ${dur}s" + if (overHud) " (HUD)" else " (Escena)"
        }
        StepCueKind.UiBlackout -> {
            val params = parseSemiKv(event.value2)
            val v1Lower = event.value1.trim().lowercase()
            val action = params["action"] ?: when {
                lower == "hide arrows" || lower == "hide strums" || lower == "ocultar flechas" -> "hide_arrows"
                lower == "show arrows" || lower == "show strums" || lower == "mostrar flechas" -> "show_arrows"
                lower == "toggle arrows" -> "toggle_arrows"
                else -> v1Lower.ifBlank { lower }
            }
            val dur = params["dur"] ?: params["duration"] ?: event.value2.trim().takeIf { it.toDoubleOrNull() != null } ?: "0.60"
            when (action) {
                "intro" -> {
                    val delayMs = params["revealms"]?.toDoubleOrNull() ?: 0.0
                    val delayTag = if (delayMs > 0.0) " +${String.format(java.util.Locale.US, "%.1fs", delayMs / 1000.0)}" else ""
                    "Intro Negra (3-2-1-¡YA! visible)$delayTag · fade ${dur}s"
                }
                "hide_arrows" -> "Ocultar Flechas · ${dur}s"
                "show_arrows" -> "Mostrar Flechas · ${dur}s"
                "toggle_arrows" -> "Alternar Flechas · ${dur}s"
                "show", "reveal", "show ui", "show hud" -> "Volver a Color y Mostrar UI · ${dur}s"
                "toggle", "toggle ui" -> "Alternar UI y Flechas · ${dur}s"
                else -> {
                    val blackVal = params["black"]?.toDoubleOrNull()
                    val hudVal = params["hud"]?.toDoubleOrNull()
                    val arrowsVal = params["arrows"]?.toDoubleOrNull()
                    val tags = mutableListOf<String>()
                    if (blackVal != null && blackVal >= 0.05) tags.add("Negro ${(blackVal * 100).roundToInt()}%")
                    if (hudVal != null && hudVal <= 0.05) tags.add("Sin HUD")
                    if (arrowsVal != null && arrowsVal <= 0.05) tags.add("Sin Flechas")
                    if (tags.isEmpty()) "UI / Pantalla · ${event.name} (${dur}s)" else "${tags.joinToString(" + ")} · ${dur}s"
                }
            }
        }
        StepCueKind.Swap, StepCueKind.Character -> {
            val params = parseSemiKv(event.value1)
            val parts = mutableListOf<String>()
            params["show"]?.takeIf { it.isNotBlank() }?.let { parts.add("+$it") }
            params["hide"]?.takeIf { it.isNotBlank() }?.let { parts.add("-$it") }
            params["player"]?.takeIf { it.isNotBlank() }?.let { parts.add("Jug→$it") }
            params["opp"]?.takeIf { it.isNotBlank() }?.let { parts.add("Riv→$it") }
            if (params["dispose"] == "1" || params["dispose"] == "true") parts.add("RAM")
            if (parts.isEmpty()) "${event.name}: ${event.value1}" else parts.joinToString(" · ")
        }
        StepCueKind.Preload -> "Precargar IO: ${event.value1}"
        StepCueKind.Other -> "${event.name} (${event.value1}, ${event.value2})"
    }
}

fun parseHexColorOrWhite(raw: String): Color {
    val clean = raw.trim().removePrefix("#")
    return try {
        when (clean.length) {
            6 -> Color((0xFF000000L or clean.toLong(16)).toInt())
            8 -> Color(clean.toLong(16).toInt())
            else -> Color.White
        }
    } catch (_: Throwable) {
        Color.White
    }
}

private fun parseSemiKv(raw: String): Map<String, String> {
    if (raw.isBlank()) return emptyMap()
    val out = LinkedHashMap<String, String>()
    raw.split(';').forEach { part ->
        val trimmed = part.trim()
        val idx = trimmed.indexOf('=')
        if (idx > 0) {
            val k = trimmed.substring(0, idx).trim().lowercase()
            val v = trimmed.substring(idx + 1).trim()
            out[k] = v
        }
    }
    return out
}

/**
 * Construye todas las filas de la rejilla continua del Chart Editor, propagando
 * colas de sustain desde la nota origen hasta su paso final (incluso a través
 * de cambios de sección) y asociando eventos de `chart.json` a cada paso `#`.
 */
fun buildChartTimeline(chart: ChartSnapshot): Pair<List<ChartTimelineRow>, Map<Int, Int>> {
    val steps = mutableListOf<TempStepEntry>()
    val sectionFirstStepIdx = LinkedHashMap<Int, Int>()

    var globalStep = 0
    for (sec in chart.sections) {
        sectionFirstStepIdx[sec.index] = globalStep
        val stepMs = (60_000.0 / sec.bpm.coerceAtLeast(40.0)) / 4.0
        for (s in 0 until sec.lengthInSteps) {
            val timeMs = sec.startMs + s * stepMs
            steps.add(
                TempStepEntry(
                    section = sec,
                    globalStepIndex = globalStep,
                    stepInSec = s,
                    stepTimeMs = timeMs,
                    stepMs = stepMs
                )
            )
            globalStep += 1
        }
    }

    if (steps.isEmpty()) {
        return emptyList<ChartTimelineRow>() to emptyMap()
    }

    fun findClosestStepIndex(timeMs: Double): Int {
        var lo = 0
        var hi = steps.lastIndex
        while (lo < hi) {
            val mid = (lo + hi) ushr 1
            if (steps[mid].stepTimeMs < timeMs) {
                lo = mid + 1
            } else {
                hi = mid
            }
        }
        if (lo > 0 && abs(steps[lo - 1].stepTimeMs - timeMs) <= abs(steps[lo].stepTimeMs - timeMs)) {
            return lo - 1
        }
        return lo
    }

    // 1. Asignar notas y extender colas continuas hasta el paso donde terminan
    for (note in chart.notes) {
        val originIdx = findClosestStepIndex(note.timeMs)
        val origin = steps[originIdx]
        val dir = note.direction.coerceIn(0, 3)
        val col = (if (note.isPlayer) 4 else 0) + dir

        origin.notesByCol[col] = note

        if (note.sustainMs > origin.stepMs * 0.40) {
            val targetEndMs = note.timeMs + note.sustainMs
            var endIdx = findClosestStepIndex(targetEndMs)
            if (endIdx <= originIdx) {
                endIdx = (originIdx + (note.sustainMs / origin.stepMs).roundToInt().coerceAtLeast(1))
                    .coerceAtMost(steps.lastIndex)
            }
            if (endIdx > originIdx) {
                origin.sustainByCol[col] = ChartSustainPass(dir, SustainSegmentKind.Head, note.isPlayer)
                for (k in (originIdx + 1)..endIdx) {
                    val kind = if (k == endIdx) SustainSegmentKind.Tail else SustainSegmentKind.Body
                    steps[k].sustainByCol[col] = ChartSustainPass(dir, kind, note.isPlayer)
                }
            }
        }
    }

    // 2. Mapear eventos a sus pasos `#` y marcar rangos de duración de Flash, Negro (UI Blackout) y Viñeta
    val flashSpans = mutableListOf<FlashSpan>()
    val blackoutSpans = mutableListOf<BlackoutSpan>()
    val vignetteSpans = mutableListOf<VignetteSpan>()
    val timelineEndMs = steps.last().let { it.stepTimeMs + it.stepMs }

    var activeBlackStartMs: Double? = if (chart.introUi.startBlackout && chart.introUi.blackoutAlpha > 0.04f) 0.0 else null
    var activeBlackAlpha: Float = if (activeBlackStartMs != null) chart.introUi.blackoutAlpha.coerceIn(0f, 1f) else 0f
    if (activeBlackStartMs != null && chart.introUi.revealOnSongStart) {
        val endMs = chart.introUi.revealDelayMs.coerceAtLeast(10.0)
        blackoutSpans.add(BlackoutSpan(startMs = 0.0, endMs = endMs, alpha = activeBlackAlpha))
        activeBlackStartMs = null
        activeBlackAlpha = 0f
    }

    var activeVigStartMs: Double? = null
    var activeVigColor: Color = Color(0xFFEF4444)

    chart.events.forEachIndexed { globalIdx, ev ->
        val stepIdx = findClosestStepIndex(ev.timeMs)
        steps[stepIdx].events.add(IndexedChartEvent(globalIdx, ev))
        val kind = classifyChartEvent(ev)

        if (kind == StepCueKind.Flash) {
            val params = parseSemiKv(ev.value2)
            val durSec = params["dur"]?.toDoubleOrNull()
                ?: params["duration"]?.toDoubleOrNull()
                ?: ev.value2.trim().toDoubleOrNull()
                ?: 0.65
            val endMs = params["endms"]?.toDoubleOrNull()
                ?: params["end_ms"]?.toDoubleOrNull()
                ?: (ev.timeMs + durSec.coerceAtLeast(0.05) * 1000.0)
            flashSpans.add(
                FlashSpan(
                    startMs = ev.timeMs,
                    endMs = endMs.coerceAtLeast(ev.timeMs + 40.0),
                    color = parseHexColorOrWhite(ev.value1)
                )
            )
        }
    }

    // Orden cronológico para rangos continuos de Pantalla Negra y Viñeta Lateral
    for (ev in chart.events.sortedBy { it.timeMs }) {
        val kind = classifyChartEvent(ev)
        if (kind == StepCueKind.UiBlackout) {
            val params = parseSemiKv(ev.value2)
            val lower = ev.name.trim().lowercase()
            val v1Lower = ev.value1.trim().lowercase()
            val action = params["action"]?.lowercase() ?: when {
                lower == "hide arrows" || lower == "hide strums" || lower == "ocultar flechas" -> "hide_arrows"
                lower == "show arrows" || lower == "show strums" || lower == "mostrar flechas" -> "show_arrows"
                else -> v1Lower.ifBlank { lower }
            }
            if (action == "show" || action == "reveal" || action == "show ui" || action == "show hud" ||
                (params["black"]?.toFloatOrNull()?.let { it <= 0.04f } == true && action != "intro")
            ) {
                val startMs = activeBlackStartMs
                if (startMs != null && ev.timeMs >= startMs) {
                    blackoutSpans.add(BlackoutSpan(startMs, ev.timeMs, activeBlackAlpha))
                    activeBlackStartMs = null
                    activeBlackAlpha = 0f
                }
            } else if (action != "hide_arrows" && action != "show_arrows" && action != "toggle_arrows") {
                val blackVal = params["black"]?.toFloatOrNull() ?: if (action == "hide" || action == "intro") 1f else 0f
                if (blackVal > 0.04f) {
                    val onStart = params["onstart"] == "1" || params["onstart"] == "true" || action == "intro"
                    val explicitEnd = params["endms"]?.toDoubleOrNull() ?: params["end_ms"]?.toDoubleOrNull()
                    val startMs = if (action == "intro") 0.0 else ev.timeMs
                    if (explicitEnd != null && explicitEnd > startMs) {
                        blackoutSpans.add(BlackoutSpan(startMs, explicitEnd, blackVal.coerceIn(0f, 1f)))
                        activeBlackStartMs = null
                    } else if (onStart && (params["revealms"]?.toDoubleOrNull() ?: 0.0) > 0.0) {
                        val revealAt = params["revealms"]?.toDoubleOrNull() ?: 0.0
                        blackoutSpans.add(BlackoutSpan(0.0, revealAt, blackVal.coerceIn(0f, 1f)))
                        activeBlackStartMs = null
                    } else {
                        if (activeBlackStartMs != null && ev.timeMs > activeBlackStartMs!!) {
                            blackoutSpans.add(BlackoutSpan(activeBlackStartMs!!, ev.timeMs, activeBlackAlpha))
                        }
                        activeBlackStartMs = startMs
                        activeBlackAlpha = blackVal.coerceIn(0f, 1f)
                    }
                }
            }
        } else if (kind == StepCueKind.Vignette) {
            val params = parseSemiKv(ev.value2)
            val v1Lower = ev.value1.trim().lowercase()
            val action = params["action"]?.lowercase() ?: v1Lower
            val off = action == "off" || action == "hide" || action == "clear" || action == "none" ||
                params["enabled"] == "0" || params["enabled"] == "false" ||
                (params["alpha"]?.toFloatOrNull()?.let { it <= 0.01f } == true)
            if (off) {
                val startMs = activeVigStartMs
                if (startMs != null && ev.timeMs >= startMs) {
                    vignetteSpans.add(VignetteSpan(startMs, ev.timeMs, activeVigColor))
                    activeVigStartMs = null
                }
            } else {
                val rawCol = params["color"] ?: ev.value1.trim().takeIf { it.startsWith("#") } ?: "#EF4444"
                val col = parseHexColorOrWhite(rawCol)
                val explicitEnd = params["endms"]?.toDoubleOrNull() ?: params["end_ms"]?.toDoubleOrNull()
                if (explicitEnd != null && explicitEnd > ev.timeMs) {
                    vignetteSpans.add(VignetteSpan(ev.timeMs, explicitEnd, col))
                    activeVigStartMs = null
                } else {
                    if (activeVigStartMs != null && ev.timeMs > activeVigStartMs!!) {
                        vignetteSpans.add(VignetteSpan(activeVigStartMs!!, ev.timeMs, activeVigColor))
                    }
                    activeVigStartMs = ev.timeMs
                    activeVigColor = col
                }
            }
        }
    }

    if (activeBlackStartMs != null) {
        blackoutSpans.add(BlackoutSpan(activeBlackStartMs!!, timelineEndMs, activeBlackAlpha))
    }
    if (activeVigStartMs != null) {
        vignetteSpans.add(VignetteSpan(activeVigStartMs!!, timelineEndMs, activeVigColor))
    }

    if (flashSpans.isNotEmpty() || blackoutSpans.isNotEmpty() || vignetteSpans.isNotEmpty()) {
        for (entry in steps) {
            val activeSpan = flashSpans.lastOrNull {
                entry.stepTimeMs >= it.startMs - entry.stepMs * 0.45 &&
                    entry.stepTimeMs <= it.endMs + entry.stepMs * 0.25
            }
            if (activeSpan != null) {
                entry.activeFlashColor = activeSpan.color
            }
            val activeBlack = blackoutSpans.lastOrNull {
                entry.stepTimeMs >= it.startMs - entry.stepMs * 0.45 &&
                    entry.stepTimeMs < it.endMs - entry.stepMs * 0.20
            }
            if (activeBlack != null) {
                entry.activeBlackoutAlpha = activeBlack.alpha
            }
            val activeVig = vignetteSpans.lastOrNull {
                entry.stepTimeMs >= it.startMs - entry.stepMs * 0.45 &&
                    entry.stepTimeMs < it.endMs - entry.stepMs * 0.20
            }
            if (activeVig != null) {
                entry.activeVignetteColor = activeVig.color
            }
        }
    }

    // 3. Emitir filas finales con SectionHeader y conectores de sustain entre secciones
    val rows = ArrayList<ChartTimelineRow>(steps.size + chart.sections.size)
    val sectionHeaderRowIndex = LinkedHashMap<Int, Int>()

    chart.sections.forEachIndexed { idx, sec ->
        val firstStepIdx = sectionFirstStepIdx[sec.index] ?: 0
        val nextFirstStepIdx = if (idx + 1 < chart.sections.size) {
            sectionFirstStepIdx[chart.sections[idx + 1].index] ?: steps.size
        } else {
            steps.size
        }

        val firstStep = steps.getOrNull(firstStepIdx)
        val crossingCols = if (firstStep != null && firstStepIdx > 0) {
            firstStep.sustainByCol
                .filterValues { it.kind == SustainSegmentKind.Body || it.kind == SustainSegmentKind.Tail }
                .keys
        } else {
            emptySet()
        }

        var cueCount = 0
        for (sIdx in firstStepIdx until nextFirstStepIdx) {
            cueCount += steps[sIdx].events.size
        }

        sectionHeaderRowIndex[sec.index] = rows.size
        rows.add(ChartTimelineRow.SectionHeader(sec, crossingCols, cueCount))

        for (sIdx in firstStepIdx until nextFirstStepIdx) {
            val st = steps[sIdx]
            val kinds = if (st.events.isEmpty()) {
                emptyList()
            } else {
                st.events.map { classifyChartEvent(it.event) }.distinct()
            }
            rows.add(
                ChartTimelineRow.StepRow(
                    section = st.section,
                    globalStepIndex = st.globalStepIndex,
                    stepInSec = st.stepInSec,
                    stepTimeMs = st.stepTimeMs,
                    stepMs = st.stepMs,
                    rowNotesByCol = st.notesByCol,
                    sustainPassesByCol = st.sustainByCol,
                    rowEvents = st.events,
                    eventKinds = kinds,
                    activeFlashColor = st.activeFlashColor,
                    activeBlackoutAlpha = st.activeBlackoutAlpha,
                    activeVignetteColor = st.activeVignetteColor
                )
            )
        }
    }

    return rows to sectionHeaderRowIndex
}

/**
 * Extrae el índice lineal de filas `StepRow` para resolver posiciones del hilo en O(log N) a 60 FPS.
 */
fun buildChartStepLookup(rows: List<ChartTimelineRow>): List<ChartStepLookupEntry> {
    val out = ArrayList<ChartStepLookupEntry>(rows.size)
    rows.forEachIndexed { idx, row ->
        if (row is ChartTimelineRow.StepRow) {
            out.add(ChartStepLookupEntry(rowIndex = idx, stepRow = row))
        }
    }
    return out
}

/**
 * Resuelve en qué fila exacta y fracción sub-paso (`0f..1f`) se encuentra el hilo reproductor
 * para un tiempo `songPosMs` dado, respetando cambios de BPM entre secciones.
 */
fun resolveChartPlayheadTarget(
    stepLookup: List<ChartStepLookupEntry>,
    songPosMs: Double
): ChartPlayheadTarget? {
    if (stepLookup.isEmpty()) return null
    val clampedMs = songPosMs.coerceAtLeast(0.0)
    var lo = 0
    var hi = stepLookup.lastIndex
    while (lo < hi) {
        val mid = (lo + hi + 1) ushr 1
        if (stepLookup[mid].stepRow.stepTimeMs <= clampedMs) {
            lo = mid
        } else {
            hi = mid - 1
        }
    }
    val entry = stepLookup[lo]
    val row = entry.stepRow
    val stepMs = row.stepMs.coerceAtLeast(1.0)
    val frac = ((clampedMs - row.stepTimeMs) / stepMs).toFloat().coerceIn(0f, 1f)
    return ChartPlayheadTarget(
        rowIndex = entry.rowIndex,
        stepRow = row,
        stepFraction = frac
    )
}

fun computeChartTotalDurationMs(chart: ChartSnapshot): Double {
    val last = chart.sections.lastOrNull() ?: return 0.0
    val stepMs = (60_000.0 / last.bpm.coerceAtLeast(40.0)) / 4.0
    return last.startMs + last.lengthInSteps * stepMs
}

/**
 * Estado efectivo del escenario en un paso `#` del chart: qué personaje actúa como
 * Jugador, cuál como Enemigo y qué objetos están visibles tras aplicar en orden cronológico
 * los eventos `Stage Swap` / `Swap Stage` / `Change Character` hasta ese paso.
 */
data class ChartStepStageState(
    val activePlayerId: String,
    val activeOpponentId: String,
    val visibleIds: Set<String>,
    val changedEarlierIds: Set<String>,
    val changedAtThisStepIds: Set<String>
)

private fun splitCsvIds(raw: String): List<String> {
    if (raw.isBlank()) return emptyList()
    return raw.split(',')
        .map { it.trim() }
        .filter { it.isNotEmpty() }
}

fun resolveChartStageStateAtStep(
    entities: List<StageEntitySnapshot>,
    events: List<ChartEventSnapshot>,
    stepTimeMs: Double,
    stepMs: Double
): ChartStepStageState {
    val visible = LinkedHashSet<String>()
    for (ent in entities) {
        if (ent.visible && ent.opacity > 0.005f) {
            visible.add(ent.id)
        }
    }

    var activePlayerId = entities.firstOrNull { it.role == "player" && visible.contains(it.id) }?.id
        ?: entities.firstOrNull { it.role == "player" }?.id.orEmpty()
    var activeOpponentId = entities.firstOrNull { it.role == "opponent" && visible.contains(it.id) }?.id
        ?: entities.firstOrNull { it.role == "opponent" }?.id.orEmpty()

    val changedEarlier = LinkedHashSet<String>()
    val changedHere = LinkedHashSet<String>()
    val stepToleranceMs = (stepMs * 0.5).coerceAtLeast(1.0)
    val sortedEvents = events.sortedBy { it.timeMs }

    for (ev in sortedEvents) {
        if (ev.timeMs > stepTimeMs + stepToleranceMs) break
        val lowerName = ev.name.trim().lowercase()
        if (lowerName != "stage swap" && lowerName != "swap stage" && lowerName != "change character") {
            continue
        }

        val isAtThisStep = abs(ev.timeMs - stepTimeMs) <= stepToleranceMs
        val targetModifiedSet = if (isAtThisStep) changedHere else changedEarlier

        val showIds = mutableListOf<String>()
        val hideIds = mutableListOf<String>()
        var nextPlayerId = ""
        var nextOppId = ""

        val p1 = parseSemiKv(ev.value1)
        if (p1.isEmpty() && ev.value1.isNotBlank()) {
            showIds.addAll(splitCsvIds(ev.value1))
        } else {
            for ((k, v) in p1) {
                when (k) {
                    "show", "mostrar" -> showIds.addAll(splitCsvIds(v))
                    "hide", "ocultar" -> hideIds.addAll(splitCsvIds(v))
                    "player", "jugador" -> nextPlayerId = v
                    "opp", "opponent", "enemigo", "rival" -> nextOppId = v
                }
            }
        }

        val p2 = parseSemiKv(ev.value2)
        if (p2.isNotEmpty()) {
            for ((k, v) in p2) {
                when (k) {
                    "show", "mostrar" -> showIds.addAll(splitCsvIds(v))
                    "hide", "ocultar" -> hideIds.addAll(splitCsvIds(v))
                    "player", "jugador" -> nextPlayerId = v
                    "opp", "opponent", "enemigo", "rival" -> nextOppId = v
                }
            }
        }

        for (id in hideIds) {
            visible.remove(id)
            targetModifiedSet.add(id)
        }
        for (id in showIds) {
            visible.add(id)
            targetModifiedSet.add(id)
        }
        if (nextPlayerId.isNotBlank()) {
            if (activePlayerId.isNotBlank() && activePlayerId != nextPlayerId) {
                visible.remove(activePlayerId)
                targetModifiedSet.add(activePlayerId)
            }
            activePlayerId = nextPlayerId
            visible.add(nextPlayerId)
            targetModifiedSet.add(nextPlayerId)
        }
        if (nextOppId.isNotBlank()) {
            if (activeOpponentId.isNotBlank() && activeOpponentId != nextOppId) {
                visible.remove(activeOpponentId)
                targetModifiedSet.add(activeOpponentId)
            }
            activeOpponentId = nextOppId
            visible.add(nextOppId)
            targetModifiedSet.add(nextOppId)
        }
    }

    return ChartStepStageState(
        activePlayerId = activePlayerId,
        activeOpponentId = activeOpponentId,
        visibleIds = visible,
        changedEarlierIds = changedEarlier,
        changedAtThisStepIds = changedHere
    )
}

