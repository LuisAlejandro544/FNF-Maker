// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/ChartScreen.kt
// Propósito: Coordinador principal del Editor Manual de Chart (8 carriles: 4 Oponente + 4 Jugador)
// sincronizado con Rust (`pulse_core/src/chart.rs`). Orquesta el panel lateral de herramientas
// (`ChartToolsPanel`), la rejilla continua con hilo reproductor a 60 Hz y colas sustain
// (`ChartTimelineGrid`), el modal de metadatos (`ChartJsonDetailsDialog`) y los eventos `#`
// (`ChartStepCueDialog`).

package com.beatforge.studio.ui.screens

import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.beatforge.studio.AudioPreviewTransportState
import com.beatforge.studio.ImportKind
import com.beatforge.studio.ProjectSnapshot
import com.beatforge.studio.ui.popups.ChartJsonDetailsDialog
import com.beatforge.studio.ui.popups.ChartStepCueDialog
import com.beatforge.studio.ui.screens.chart.ChartPlayheadState
import com.beatforge.studio.ui.screens.chart.ChartTimelineGrid
import com.beatforge.studio.ui.screens.chart.ChartTimelineRow
import com.beatforge.studio.ui.screens.chart.ChartToolsPanel
import com.beatforge.studio.ui.screens.chart.buildChartStepLookup
import com.beatforge.studio.ui.screens.chart.buildChartTimeline
import com.beatforge.studio.ui.screens.chart.computeChartTotalDurationMs
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

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
    onOpenStageEventsScript: () -> Unit,
    onAddSection: (mustHit: Boolean) -> Unit,
    onToggleSectionMustHit: (secIndex: Int) -> Unit,
    onToggleChartNote: (secIndex: Int, stepInSec: Int, col0To7: Int, sustainSteps: Int) -> Unit,
    onSetAudioPreviewTransport: (play: Boolean, seekMs: Double) -> Unit,
    onReadAudioPreviewState: suspend () -> AudioPreviewTransportState
) {
    val chart = snapshot.chart
    var selectedSustainSteps by remember { mutableIntStateOf(0) }
    var selectedSectionIdx by remember { mutableIntStateOf(0) }
    var showDetailsDialog by remember { mutableStateOf(false) }
    var activeCueGlobalStep by remember { mutableStateOf<Int?>(null) }
    val listState = rememberLazyListState()
    val scope = rememberCoroutineScope()

    val timelineData = remember(chart.sections, chart.notes, chart.events, chart.bpm) {
        buildChartTimeline(chart)
    }

    val rows = timelineData.first
    val sectionPositions = timelineData.second
    val chartDurationMs = remember(chart.sections, chart.bpm) {
        computeChartTotalDurationMs(chart)
    }
    val stepLookup = remember(rows) {
        buildChartStepLookup(rows)
    }
    val playheadState = remember {
        ChartPlayheadState(initialDurationMs = chartDurationMs)
    }

    val activeSection = chart.sections.getOrNull(selectedSectionIdx) ?: chart.sections.firstOrNull()
    val activeCueStepRow = activeCueGlobalStep?.let { stepIdx ->
        rows.firstOrNull { it is ChartTimelineRow.StepRow && it.globalStepIndex == stepIdx } as? ChartTimelineRow.StepRow
    }

    // Sincronización inicial o al editar el chart mientras está en pausa
    LaunchedEffect(stepLookup, chartDurationMs) {
        val st = onReadAudioPreviewState()
        val effectiveDur = maxOf(st.durationMs, chartDurationMs)
        playheadState.updatePosition(st.posMs, effectiveDur, stepLookup)
    }

    // Bucle a 60 Hz mientras la canción suena: mueve el hilo y desplaza la rejilla si "Seguir" está activo
    LaunchedEffect(audioPreviewPlaying, stepLookup, chartDurationMs) {
        if (!audioPreviewPlaying) return@LaunchedEffect
        while (isActive) {
            val st = onReadAudioPreviewState()
            val effectiveDur = maxOf(st.durationMs, chartDurationMs)
            playheadState.updatePosition(st.posMs, effectiveDur, stepLookup)

            val target = playheadState.activeTarget
            if (playheadState.followThread && target != null) {
                selectedSectionIdx = target.stepRow.section.index
                val layout = listState.layoutInfo
                val visItem = layout.visibleItemsInfo.firstOrNull { it.index == target.rowIndex }
                if (visItem != null) {
                    val threadY = visItem.offset.toFloat() + target.stepFraction * visItem.size.toFloat()
                    val viewportH = (layout.viewportEndOffset - layout.viewportStartOffset)
                        .toFloat()
                        .coerceAtLeast(1f)
                    val anchorY = viewportH * 0.40f
                    if (threadY > anchorY) {
                        listState.scrollBy(threadY - anchorY)
                    } else if (threadY < layout.viewportStartOffset.toFloat()) {
                        listState.scrollToItem((target.rowIndex - 3).coerceAtLeast(0))
                    }
                } else {
                    listState.scrollToItem((target.rowIndex - 3).coerceAtLeast(0))
                }
            }

            if (!st.isPlaying) {
                break
            }
            delay(16L)
        }
    }

    val playOrPausePlayhead: () -> Unit = {
        if (audioPreviewPlaying) {
            onSetAudioPreviewTransport(false, -1.0)
        } else if (playheadState.durationMs > 0.0 && playheadState.posMs >= playheadState.durationMs - 25.0) {
            val dur = maxOf(playheadState.durationMs, chartDurationMs)
            playheadState.updatePosition(0.0, dur, stepLookup)
            selectedSectionIdx = 0
            if (playheadState.followThread) {
                scope.launch { listState.scrollToItem(0) }
            }
            onSetAudioPreviewTransport(true, 0.0)
        } else {
            onSetAudioPreviewTransport(true, -1.0)
        }
    }

    val playFromMs: (Double) -> Unit = { startMs ->
        val dur = maxOf(playheadState.durationMs, chartDurationMs)
        playheadState.updatePosition(startMs, dur, stepLookup)
        playheadState.activeTarget?.let { target ->
            selectedSectionIdx = target.stepRow.section.index
            if (playheadState.followThread) {
                scope.launch { listState.scrollToItem((target.rowIndex - 2).coerceAtLeast(0)) }
            }
        }
        onSetAudioPreviewTransport(true, startMs)
    }

    val rewindToStart: () -> Unit = {
        val dur = maxOf(playheadState.durationMs, chartDurationMs)
        playheadState.updatePosition(0.0, dur, stepLookup)
        selectedSectionIdx = 0
        if (playheadState.followThread) {
            scope.launch { listState.animateScrollToItem(0) }
        }
        onSetAudioPreviewTransport(audioPreviewPlaying, 0.0)
    }

    val toggleFollowPlayhead: () -> Unit = {
        val nextFollow = !playheadState.followThread
        playheadState.followThread = nextFollow
        if (nextFollow) {
            playheadState.activeTarget?.let { target ->
                selectedSectionIdx = target.stepRow.section.index
                scope.launch {
                    listState.animateScrollToItem((target.rowIndex - 3).coerceAtLeast(0))
                }
            }
        }
    }

    val scrubPlayheadMs: (Double) -> Unit = { targetMs ->
        val dur = maxOf(playheadState.durationMs, chartDurationMs)
        playheadState.updatePosition(targetMs, dur, stepLookup)
        playheadState.activeTarget?.let { target ->
            selectedSectionIdx = target.stepRow.section.index
            if (playheadState.followThread) {
                scope.launch { listState.scrollToItem((target.rowIndex - 3).coerceAtLeast(0)) }
            }
        }
        onSetAudioPreviewTransport(audioPreviewPlaying, targetMs)
    }

    val seekPlayheadToStepMs: (Double) -> Unit = { stepTimeMs ->
        val dur = maxOf(playheadState.durationMs, chartDurationMs)
        playheadState.updatePosition(stepTimeMs, dur, stepLookup)
        playheadState.activeTarget?.let { target ->
            selectedSectionIdx = target.stepRow.section.index
        }
        onSetAudioPreviewTransport(audioPreviewPlaying, stepTimeMs)
    }

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
            playheadState = playheadState,
            onRequestImport = onRequestImport,
            onRequestExportJson = onRequestExportJson,
            onOpenDetailsDialog = { showDetailsDialog = true },
            onOpenStageEventsScript = onOpenStageEventsScript,
            onRestoreDemoChart = onRestoreDemoChart,
            onSetChartBpm = onSetChartBpm,
            onSelectSustainSteps = { selectedSustainSteps = it },
            onPlayOrPausePlayhead = playOrPausePlayhead,
            onPlayFromSection = playFromMs,
            onRewindPlayheadToStart = rewindToStart,
            onToggleFollowPlayhead = toggleFollowPlayhead,
            onOpenCueAtPlayhead = {
                playheadState.activeTarget?.let { target ->
                    activeCueGlobalStep = target.stepRow.globalStepIndex
                }
            },
            onAddSection = { mustHit ->
                onAddSection(mustHit)
                selectedSectionIdx = chart.sections.size
            },
            onSelectSection = { secIndex, rowIdx ->
                selectedSectionIdx = secIndex
                scope.launch { listState.animateScrollToItem(rowIdx) }
            },
            modifier = Modifier.weight(0.31f)
        )

        ChartTimelineGrid(
            rows = rows,
            listState = listState,
            selectedSustainSteps = selectedSustainSteps,
            audioPreviewPlaying = audioPreviewPlaying,
            playheadState = playheadState,
            onPlayOrPausePlayhead = playOrPausePlayhead,
            onRewindPlayheadToStart = rewindToStart,
            onToggleFollowPlayhead = toggleFollowPlayhead,
            onScrubPlayheadMs = scrubPlayheadMs,
            onSeekPlayheadAndPauseOrKeep = seekPlayheadToStepMs,
            onToggleSectionMustHit = onToggleSectionMustHit,
            onSelectSection = { selectedSectionIdx = it },
            onToggleChartNote = onToggleChartNote,
            onOpenStepCueDialog = { row -> activeCueGlobalStep = row.globalStepIndex },
            modifier = Modifier.weight(0.69f)
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

    if (activeCueStepRow != null) {
        ChartStepCueDialog(
            stepRow = activeCueStepRow,
            entities = snapshot.stageEntities,
            onAddChartEvent = onAddChartEvent,
            onRemoveChartEvent = onRemoveChartEvent,
            onMovePlayheadHere = seekPlayheadToStepMs,
            onOpenStageEventsScript = onOpenStageEventsScript,
            onDismiss = { activeCueGlobalStep = null }
        )
    }
}

