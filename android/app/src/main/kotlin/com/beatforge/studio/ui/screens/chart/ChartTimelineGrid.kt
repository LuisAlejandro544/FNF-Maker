// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/chart/ChartTimelineGrid.kt
// Propósito: Rejilla vertical continua de la línea de tiempo del chart (8 carriles:
// 4 Oponente + 4 Jugador), cabeceras de sección con toggle de cámara (`mustHitSection`)
// y celdas interactivas para colocar o quitar notas con su duración de sustain.

package com.beatforge.studio.ui.screens.chart

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.SwapHoriz
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.ChartSectionSnapshot
import com.beatforge.studio.VisibleNote
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

private val LANE_ARROWS = listOf("←", "↓", "↑", "→")

sealed class ChartTimelineRow {
    data class SectionHeader(val section: ChartSectionSnapshot) : ChartTimelineRow()
    data class StepRow(
        val section: ChartSectionSnapshot,
        val stepInSec: Int,
        val stepMs: Double,
        val rowNotesByCol: Map<Int, VisibleNote>
    ) : ChartTimelineRow()
}

@Composable
fun ChartTimelineGrid(
    rows: List<ChartTimelineRow>,
    listState: LazyListState,
    selectedSustainSteps: Int,
    onToggleSectionMustHit: (secIndex: Int) -> Unit,
    onSelectSection: (secIndex: Int) -> Unit,
    onToggleChartNote: (secIndex: Int, stepInSec: Int, col0To7: Int, sustainSteps: Int) -> Unit,
    modifier: Modifier = Modifier
) {
    Surface(
        modifier = modifier.fillMaxHeight(),
        shape = RoundedCornerShape(12.dp),
        color = StudioColors.BgPanel,
        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Cabecera fija de carriles
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(StudioColors.BgCard)
                    .padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text(
                    text = "#",
                    color = StudioColors.TextMuted,
                    fontSize = 10.sp,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.width(34.dp)
                )
                Column(modifier = Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("OPONENTE", color = StudioColors.AccentCoral, fontSize = 10.sp, fontWeight = FontWeight.Bold)
                    Row(modifier = Modifier.fillMaxWidth()) {
                        for (i in 0..3) {
                            Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                                Text(LANE_ARROWS[i], color = StudioColors.LaneColors[i], fontSize = 12.sp, fontWeight = FontWeight.Bold)
                            }
                        }
                    }
                }
                Spacer(Modifier.width(6.dp))
                Column(modifier = Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("JUGADOR", color = StudioColors.AccentEmerald, fontSize = 10.sp, fontWeight = FontWeight.Bold)
                    Row(modifier = Modifier.fillMaxWidth()) {
                        for (i in 0..3) {
                            Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                                Text(LANE_ARROWS[i], color = StudioColors.LaneColors[i], fontSize = 12.sp, fontWeight = FontWeight.Bold)
                            }
                        }
                    }
                }
            }
            HorizontalDivider(color = StudioColors.BorderSubtle)

            // Rejilla larga vertical continua de todas las secciones y pasos
            LazyColumn(
                state = listState,
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
                    .background(StudioColors.BgDark)
            ) {
                items(rows.size) { index ->
                    when (val row = rows[index]) {
                        is ChartTimelineRow.SectionHeader -> {
                            val sec = row.section
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .background(StudioColors.BgElevated)
                                    .padding(horizontal = 10.dp, vertical = 5.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.SpaceBetween
                            ) {
                                Text(
                                    text = "Sección ${sec.index + 1}  ·  ${String.format("%.2fs", sec.startMs / 1000.0)}  ·  ${sec.bpm.roundToInt()} BPM",
                                    color = StudioColors.TextPrimary,
                                    fontSize = 11.sp,
                                    fontWeight = FontWeight.Bold
                                )
                                Row(
                                    verticalAlignment = Alignment.CenterVertically,
                                    modifier = Modifier
                                        .clip(RoundedCornerShape(6.dp))
                                        .background(StudioColors.BgCard)
                                        .clickable { onToggleSectionMustHit(sec.index) }
                                        .padding(horizontal = 8.dp, vertical = 3.dp)
                                ) {
                                    Icon(Icons.Rounded.SwapHoriz, null, tint = StudioColors.AccentAmber, modifier = Modifier.size(13.dp))
                                    Spacer(Modifier.width(4.dp))
                                    Text(
                                        text = if (sec.mustHitSection) "Cámara: Jugador" else "Cámara: Oponente",
                                        color = if (sec.mustHitSection) StudioColors.AccentEmerald else StudioColors.AccentCoral,
                                        fontSize = 10.sp,
                                        fontWeight = FontWeight.Bold
                                    )
                                }
                            }
                        }

                        is ChartTimelineRow.StepRow -> {
                            val isBeat = row.stepInSec % 4 == 0
                            val rowBg = if (isBeat) StudioColors.BgPanel.copy(alpha = 0.72f) else StudioColors.BgDark
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .height(34.dp)
                                    .background(rowBg)
                                    .padding(horizontal = 8.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    text = "${row.stepInSec}",
                                    color = if (isBeat) StudioColors.AccentAmber else StudioColors.TextMuted.copy(alpha = 0.65f),
                                    fontSize = 10.sp,
                                    fontWeight = if (isBeat) FontWeight.Bold else FontWeight.Normal,
                                    modifier = Modifier.width(34.dp)
                                )

                                // 4 columnas Oponente (0..3)
                                Row(
                                    modifier = Modifier.weight(1f),
                                    horizontalArrangement = Arrangement.spacedBy(3.dp)
                                ) {
                                    for (c in 0..3) {
                                        ChartNoteCell(
                                            note = row.rowNotesByCol[c],
                                            laneIndex = c,
                                            stepMs = row.stepMs,
                                            onClick = {
                                                onSelectSection(row.section.index)
                                                onToggleChartNote(row.section.index, row.stepInSec, c, selectedSustainSteps)
                                            },
                                            modifier = Modifier.weight(1f)
                                        )
                                    }
                                }

                                Spacer(Modifier.width(6.dp))

                                // 4 columnas Jugador (4..7)
                                Row(
                                    modifier = Modifier.weight(1f),
                                    horizontalArrangement = Arrangement.spacedBy(3.dp)
                                ) {
                                    for (c in 4..7) {
                                        ChartNoteCell(
                                            note = row.rowNotesByCol[c],
                                            laneIndex = c - 4,
                                            stepMs = row.stepMs,
                                            onClick = {
                                                onSelectSection(row.section.index)
                                                onToggleChartNote(row.section.index, row.stepInSec, c, selectedSustainSteps)
                                            },
                                            modifier = Modifier.weight(1f)
                                        )
                                    }
                                }
                            }
                            HorizontalDivider(
                                color = StudioColors.BorderSubtle.copy(alpha = if (isBeat) 0.55f else 0.22f),
                                thickness = 0.5.dp
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun ChartNoteCell(
    note: VisibleNote?,
    laneIndex: Int,
    stepMs: Double,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val laneColor = StudioColors.LaneColors[laneIndex]
    val hasNote = note != null
    val sustainSteps = if (note != null && note.sustainMs > stepMs * 0.4) {
        (note.sustainMs / stepMs).roundToInt().coerceAtLeast(1)
    } else {
        0
    }

    Box(
        modifier = modifier
            .height(28.dp)
            .clip(RoundedCornerShape(5.dp))
            .background(if (hasNote) laneColor.copy(alpha = 0.88f) else StudioColors.BgCard.copy(alpha = 0.55f))
            .border(
                width = if (hasNote) 1.dp else 0.5.dp,
                color = if (hasNote) Color.White.copy(alpha = 0.65f) else StudioColors.BorderSubtle.copy(alpha = 0.45f),
                shape = RoundedCornerShape(5.dp)
            )
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        if (hasNote) {
            Text(
                text = if (sustainSteps > 0) "${LANE_ARROWS[laneIndex]}+$sustainSteps" else LANE_ARROWS[laneIndex],
                color = Color.White,
                fontSize = 11.sp,
                fontWeight = FontWeight.Bold,
                maxLines = 1,
                softWrap = false
            )
        }
    }
}
