// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/chart/ChartTimelineGrid.kt
// Propósito: Coordinador de la rejilla vertical continua de la línea de tiempo del chart
// (8 carriles: 4 Oponente + 4 Jugador), delegando barra de transporte, cabeceras de sección,
// filas Canvas de 8 carriles e hilo reproductor a 60 FPS en `chart/grid/*`.

package com.beatforge.studio.ui.screens.chart

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.ui.screens.chart.grid.ChartPlayheadThreadOverlay
import com.beatforge.studio.ui.screens.chart.grid.ChartPlayheadTransportBar
import com.beatforge.studio.ui.screens.chart.grid.ChartSectionHeaderRow
import com.beatforge.studio.ui.screens.chart.grid.ChartStepTimelineRow
import com.beatforge.studio.ui.screens.chart.grid.LANE_ARROWS
import com.beatforge.studio.ui.theme.StudioColors

@Composable
fun ChartTimelineGrid(
    rows: List<ChartTimelineRow>,
    listState: LazyListState,
    stageCamera: StageCameraSnapshot,
    selectedSustainSteps: Int,
    audioPreviewPlaying: Boolean,
    playheadState: ChartPlayheadState,
    onPlayOrPausePlayhead: () -> Unit,
    onRewindPlayheadToStart: () -> Unit,
    onToggleFollowPlayhead: () -> Unit,
    onScrubPlayheadMs: (Double) -> Unit,
    onSeekPlayheadAndPauseOrKeep: (Double) -> Unit,
    onToggleSectionMustHit: (secIndex: Int) -> Unit,
    onSetSectionCameraSlot: (secIndex: Int, cameraSlot: Int?) -> Unit,
    onSelectSection: (secIndex: Int) -> Unit,
    onToggleChartNote: (secIndex: Int, stepInSec: Int, col0To7: Int, sustainSteps: Int) -> Unit,
    onOpenStepCueDialog: (ChartTimelineRow.StepRow) -> Unit,
    modifier: Modifier = Modifier
) {
    val textMeasurer = rememberTextMeasurer()
    val laneTextCache = remember(textMeasurer) { HashMap<String, TextLayoutResult>() }

    Surface(
        modifier = modifier.fillMaxHeight(),
        shape = RoundedCornerShape(12.dp),
        color = StudioColors.BgPanel,
        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Barra superior compacta de transporte del hilo reproductor y seguimiento
            ChartPlayheadTransportBar(
                audioPreviewPlaying = audioPreviewPlaying,
                playheadState = playheadState,
                onPlayOrPause = onPlayOrPausePlayhead,
                onRewindToStart = onRewindPlayheadToStart,
                onToggleFollow = onToggleFollowPlayhead,
                onScrubMs = onScrubPlayheadMs
            )
            HorizontalDivider(color = StudioColors.BorderSubtle)

            // Cabecera fija de carriles y columna interactiva # / Eventos
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(StudioColors.BgCard)
                    .padding(horizontal = 8.dp, vertical = 5.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(
                    modifier = Modifier.width(52.dp),
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    Text(
                        text = "# Ev.",
                        color = StudioColors.AccentCyan,
                        fontSize = 10.sp,
                        fontWeight = FontWeight.Bold
                    )
                    Text(
                        text = "mantener=hilo",
                        color = StudioColors.TextMuted,
                        fontSize = 7.sp,
                        maxLines = 1,
                        softWrap = false
                    )
                }

                Column(modifier = Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("OPONENTE", color = StudioColors.AccentCoral, fontSize = 10.sp, fontWeight = FontWeight.Bold)
                    Row(modifier = Modifier.fillMaxWidth()) {
                        for (i in 0..3) {
                            Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                                Text(
                                    text = LANE_ARROWS[i],
                                    color = StudioColors.LaneColors[i],
                                    fontSize = 12.sp,
                                    fontWeight = FontWeight.Bold
                                )
                            }
                        }
                    }
                }

                Spacer(Modifier.width(8.dp))

                Column(modifier = Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("JUGADOR", color = StudioColors.AccentEmerald, fontSize = 10.sp, fontWeight = FontWeight.Bold)
                    Row(modifier = Modifier.fillMaxWidth()) {
                        for (i in 0..3) {
                            Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                                Text(
                                    text = LANE_ARROWS[i],
                                    color = StudioColors.LaneColors[i],
                                    fontSize = 12.sp,
                                    fontWeight = FontWeight.Bold
                                )
                            }
                        }
                    }
                }
            }
            HorizontalDivider(color = StudioColors.BorderSubtle)

            // Contenedor con LazyColumn optimizada (keys + contentType + 1 Canvas por fila) y hilo a 60 Hz
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
                    .background(StudioColors.BgDark)
            ) {
                LazyColumn(
                    state = listState,
                    modifier = Modifier.fillMaxSize()
                ) {
                    items(
                        count = rows.size,
                        key = { index ->
                            when (val r = rows[index]) {
                                is ChartTimelineRow.SectionHeader -> "sec_${r.section.index}"
                                is ChartTimelineRow.StepRow -> "step_${r.globalStepIndex}"
                            }
                        },
                        contentType = { index ->
                            when (rows[index]) {
                                is ChartTimelineRow.SectionHeader -> 0
                                is ChartTimelineRow.StepRow -> 1
                            }
                        }
                    ) { index ->
                        when (val row = rows[index]) {
                            is ChartTimelineRow.SectionHeader -> {
                                ChartSectionHeaderRow(
                                    row = row,
                                    stageCamera = stageCamera,
                                    onToggleSectionMustHit = { onToggleSectionMustHit(row.section.index) },
                                    onSetSectionCameraSlot = { camSlot ->
                                        onSetSectionCameraSlot(row.section.index, camSlot)
                                    }
                                )
                            }

                            is ChartTimelineRow.StepRow -> {
                                ChartStepTimelineRow(
                                    row = row,
                                    selectedSustainSteps = selectedSustainSteps,
                                    textMeasurer = textMeasurer,
                                    textCache = laneTextCache,
                                    onSelectSection = onSelectSection,
                                    onToggleChartNote = onToggleChartNote,
                                    onOpenStepCueDialog = onOpenStepCueDialog,
                                    onSeekToStep = { onSeekPlayheadAndPauseOrKeep(row.stepTimeMs) }
                                )
                            }
                        }
                    }
                }

                ChartPlayheadThreadOverlay(
                    listState = listState,
                    playheadState = playheadState,
                    audioPreviewPlaying = audioPreviewPlaying,
                    modifier = Modifier.fillMaxSize()
                )
            }
        }
    }
}
