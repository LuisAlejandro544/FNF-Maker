// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/ChartScreen.kt
// Propósito: Coordinador principal del Editor Manual de Chart (8 carriles: 4 Oponente + 4 Jugador)
// sincronizado con Rust (`pulse_core/src/chart.rs`). Orquesta el panel lateral de herramientas
// (`ChartToolsPanel`), la rejilla continua (`ChartTimelineGrid`) y el modal (`ChartJsonDetailsDialog`).

package com.beatforge.studio.ui.screens

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.beatforge.studio.ImportKind
import com.beatforge.studio.ProjectSnapshot
import com.beatforge.studio.VisibleNote
import com.beatforge.studio.ui.popups.ChartJsonDetailsDialog
import com.beatforge.studio.ui.screens.chart.ChartTimelineGrid
import com.beatforge.studio.ui.screens.chart.ChartTimelineRow
import com.beatforge.studio.ui.screens.chart.ChartToolsPanel
import kotlinx.coroutines.launch
import kotlin.math.abs

@Composable
fun ChartScreen(
    snapshot: ProjectSnapshot,
    audioPreviewPlaying: Boolean,
    onRequestImport: (ImportKind) -> Unit,
    onRequestExportJson: () -> Unit,
    onRestoreDemoChart: () -> Unit,
    onSetChartBpm: (Double) -> Unit,
    onUpdateChartDetails: (
        title: String,
        speed: Double,
        player1: String,
        player2: String,
        gfVersion: String,
        stage: String,
        needsVoices: Boolean
    ) -> Unit,
    onSetSectionBpm: (secIndex: Int, bpm: Double, changeBpm: Boolean) -> Unit,
    onAddChartEvent: (timeMs: Double, name: String, value1: String, value2: String) -> Unit,
    onRemoveChartEvent: (index: Int) -> Unit,
    onAddSection: (mustHit: Boolean) -> Unit,
    onToggleSectionMustHit: (secIndex: Int) -> Unit,
    onToggleChartNote: (secIndex: Int, stepInSec: Int, col0To7: Int, sustainSteps: Int) -> Unit,
    onToggleAudioPreviewAtMs: (Double) -> Unit
) {
    val chart = snapshot.chart
    var selectedSustainSteps by remember { mutableIntStateOf(0) }
    var selectedSectionIdx by remember { mutableIntStateOf(0) }
    var showDetailsDialog by remember { mutableStateOf(false) }
    val listState = rememberLazyListState()
    val scope = rememberCoroutineScope()

    // Construye la línea temporal continua larga con todas las secciones y pasos
    val timelineData = remember(chart.sections, chart.notes, chart.bpm) {
        val rows = mutableListOf<ChartTimelineRow>()
        val sectionHeaderRowIndex = linkedMapOf<Int, Int>()

        chart.sections.forEach { sec ->
            sectionHeaderRowIndex[sec.index] = rows.size
            rows.add(ChartTimelineRow.SectionHeader(sec))

            val stepMs = (60_000.0 / sec.bpm.coerceAtLeast(40.0)) / 4.0
            val tolerance = stepMs * 0.45
            val secEndMs = sec.startMs + stepMs * sec.lengthInSteps
            val secNotes = chart.notes.filter {
                it.timeMs >= sec.startMs - tolerance && it.timeMs < secEndMs + tolerance
            }

            for (step in 0 until sec.lengthInSteps) {
                val targetMs = sec.startMs + step * stepMs
                val map = mutableMapOf<Int, VisibleNote>()
                for (n in secNotes) {
                    if (abs(n.timeMs - targetMs) <= tolerance) {
                        val col = (if (n.isPlayer) 4 else 0) + (n.direction.coerceIn(0, 3))
                        map[col] = n
                    }
                }
                rows.add(ChartTimelineRow.StepRow(sec, step, stepMs, map))
            }
        }
        rows to sectionHeaderRowIndex
    }

    val rows = timelineData.first
    val sectionPositions = timelineData.second
    val activeSection = chart.sections.getOrNull(selectedSectionIdx) ?: chart.sections.firstOrNull()

    Row(
        modifier = Modifier
            .fillMaxSize()
            .padding(10.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        ChartToolsPanel(
            chart = chart,
            activeSection = activeSection,
            selectedSustainSteps = selectedSustainSteps,
            selectedSectionIdx = selectedSectionIdx,
            sectionPositions = sectionPositions,
            audioPreviewPlaying = audioPreviewPlaying,
            onRequestImport = onRequestImport,
            onRequestExportJson = onRequestExportJson,
            onOpenDetailsDialog = { showDetailsDialog = true },
            onRestoreDemoChart = onRestoreDemoChart,
            onSetChartBpm = onSetChartBpm,
            onSelectSustainSteps = { selectedSustainSteps = it },
            onToggleAudioPreviewAtMs = onToggleAudioPreviewAtMs,
            onAddSection = { mustHit ->
                onAddSection(mustHit)
                selectedSectionIdx = chart.sections.size
            },
            onSelectSection = { secIndex, rowIdx ->
                selectedSectionIdx = secIndex
                scope.launch { listState.animateScrollToItem(rowIdx) }
            },
            modifier = Modifier.weight(0.32f)
        )

        ChartTimelineGrid(
            rows = rows,
            listState = listState,
            selectedSustainSteps = selectedSustainSteps,
            onToggleSectionMustHit = onToggleSectionMustHit,
            onSelectSection = { selectedSectionIdx = it },
            onToggleChartNote = onToggleChartNote,
            modifier = Modifier.weight(0.68f)
        )
    }

    if (showDetailsDialog) {
        ChartJsonDetailsDialog(
            chart = chart,
            activeSection = activeSection,
            onSaveDetails = onUpdateChartDetails,
            onSetSectionBpm = onSetSectionBpm,
            onAddEvent = onAddChartEvent,
            onRemoveEvent = onRemoveChartEvent,
            onDismiss = { showDetailsDialog = false }
        )
    }
}
