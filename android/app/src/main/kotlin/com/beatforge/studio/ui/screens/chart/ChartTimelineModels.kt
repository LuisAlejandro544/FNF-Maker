// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/chart/ChartTimelineModels.kt
// Propósito: Modelos puros y constructor de línea de tiempo para el Editor de Chart:
// resuelve notas sostenidas continuas (cabeza, cuerpo y punta final incluso cruzando
// secciones), mapea eventos de `chart.json` a cada paso `#` y calcula spans de flashes.

package com.beatforge.studio.ui.screens.chart

import androidx.compose.ui.graphics.Color
import com.beatforge.studio.ChartEventSnapshot
import com.beatforge.studio.ChartSectionSnapshot
import com.beatforge.studio.ChartSnapshot
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
        val activeFlashColor: Color?
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
    var activeFlashColor: Color? = null
)

private data class FlashSpan(
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
            lower == "show hud" -> StepCueKind.UiBlackout
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
            val action = params["action"] ?: v1Lower.ifBlank { lower }
            val dur = params["dur"] ?: params["duration"] ?: event.value2.trim().takeIf { it.toDoubleOrNull() != null } ?: "0.60"
            when (action) {
                "intro" -> {
                    val delayMs = params["revealms"]?.toDoubleOrNull() ?: 0.0
                    val delayTag = if (delayMs > 0.0) " +${String.format(java.util.Locale.US, "%.1fs", delayMs / 1000.0)}" else ""
                    "Intro Negra (3-2-1-¡YA! visible)$delayTag · fade ${dur}s"
                }
                "show", "reveal", "show ui", "show hud" -> "Mostrar Escena y UI/Flechas · ${dur}s"
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

    // 2. Mapear eventos a sus pasos `#` y marcar rangos de duración de Flash
    val flashSpans = mutableListOf<FlashSpan>()
    chart.events.forEachIndexed { globalIdx, ev ->
        val stepIdx = findClosestStepIndex(ev.timeMs)
        steps[stepIdx].events.add(IndexedChartEvent(globalIdx, ev))

        if (classifyChartEvent(ev) == StepCueKind.Flash) {
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

    if (flashSpans.isNotEmpty()) {
        for (entry in steps) {
            val activeSpan = flashSpans.lastOrNull {
                entry.stepTimeMs >= it.startMs - entry.stepMs * 0.45 &&
                    entry.stepTimeMs <= it.endMs + entry.stepMs * 0.25
            }
            if (activeSpan != null) {
                entry.activeFlashColor = activeSpan.color
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
                    activeFlashColor = st.activeFlashColor
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
