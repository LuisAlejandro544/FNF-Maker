// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/chart/grid/ChartSectionHeader.kt
// Propósito: Cabecera de sección dentro de la rejilla DAW del chart: muestra tiempo/BPM/eventos,
// estela continua de notas sostenidas que cruzan desde la sección anterior, selector `Auto / Cam N`
// y toggle de turno `Jugador / Oponente`.

package com.beatforge.studio.ui.screens.chart.grid

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.SwapHoriz
import androidx.compose.material.icons.rounded.Videocam
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.ui.screens.chart.ChartTimelineRow
import com.beatforge.studio.ui.theme.StudioColors
import java.util.Locale
import kotlin.math.roundToInt

@Composable
internal fun ChartSectionHeaderRow(
    row: ChartTimelineRow.SectionHeader,
    stageCamera: StageCameraSnapshot,
    onToggleSectionMustHit: () -> Unit,
    onSetSectionCameraSlot: (Int?) -> Unit
) {
    val sec = row.section
    val slots = stageCamera.resolvedCameras()
    var camMenuExpanded by remember { mutableStateOf(false) }

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(30.dp)
            .background(StudioColors.BgElevated.copy(alpha = 0.92f))
    ) {
        // Si alguna nota sostenida cruza desde la sección anterior, dibuja la estela continua detrás de la cabecera
        if (row.crossingCols.isNotEmpty()) {
            Row(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(horizontal = 8.dp)
            ) {
                Spacer(Modifier.width(52.dp))
                Row(modifier = Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                    for (c in 0..3) {
                        Box(modifier = Modifier.weight(1f).fillMaxHeight()) {
                            if (c in row.crossingCols) {
                                SustainConnectorCanvas(laneIndex = c)
                            }
                        }
                    }
                }
                Spacer(Modifier.width(8.dp))
                Row(modifier = Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                    for (c in 4..7) {
                        Box(modifier = Modifier.weight(1f).fillMaxHeight()) {
                            if (c in row.crossingCols) {
                                SustainConnectorCanvas(laneIndex = c - 4)
                            }
                        }
                    }
                }
            }
        }

        Row(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(
                    text = "Sec ${sec.index + 1} · ${String.format(Locale.US, "%.2fs", sec.startMs / 1000.0)} · ${sec.bpm.roundToInt()} BPM",
                    color = StudioColors.TextPrimary,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Bold
                )
                if (row.sectionCueCount > 0) {
                    Box(
                        modifier = Modifier
                            .clip(RoundedCornerShape(4.dp))
                            .background(StudioColors.AccentCyan.copy(alpha = 0.2f))
                            .padding(horizontal = 5.dp, vertical = 1.dp)
                    ) {
                        Text(
                            text = "${row.sectionCueCount} ev.",
                            color = StudioColors.AccentCyan,
                            fontSize = 9.sp,
                            fontWeight = FontWeight.Bold
                        )
                    }
                }
            }

            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(5.dp)
            ) {
                Box {
                    val slotIdx = sec.cameraSlot
                    val slotLabel = if (slotIdx == null) {
                        "Auto"
                    } else {
                        "Cam $slotIdx"
                    }
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier
                            .clip(RoundedCornerShape(6.dp))
                            .background(
                                if (slotIdx != null) StudioColors.AccentCyan.copy(alpha = 0.18f)
                                else StudioColors.BgCard.copy(alpha = 0.92f)
                            )
                            .border(
                                1.dp,
                                if (slotIdx != null) StudioColors.AccentCyan else StudioColors.BorderSubtle,
                                RoundedCornerShape(6.dp)
                            )
                            .clickable { camMenuExpanded = true }
                            .padding(horizontal = 7.dp, vertical = 3.dp)
                    ) {
                        Icon(
                            imageVector = Icons.Rounded.Videocam,
                            contentDescription = null,
                            tint = if (slotIdx != null) StudioColors.AccentCyan else StudioColors.TextMuted,
                            modifier = Modifier.size(12.dp)
                        )
                        Spacer(Modifier.width(3.dp))
                        Text(
                            text = slotLabel,
                            color = if (slotIdx != null) StudioColors.AccentCyan else StudioColors.TextPrimary,
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Bold
                        )
                    }

                    DropdownMenu(
                        expanded = camMenuExpanded,
                        onDismissRequest = { camMenuExpanded = false }
                    ) {
                        DropdownMenuItem(
                            text = { Text("Auto (mustHit / GF)", fontSize = 12.sp) },
                            onClick = {
                                camMenuExpanded = false
                                onSetSectionCameraSlot(null)
                            }
                        )
                        slots.forEach { slot ->
                            DropdownMenuItem(
                                text = { Text(slot.name, fontSize = 12.sp) },
                                onClick = {
                                    camMenuExpanded = false
                                    onSetSectionCameraSlot(slot.index)
                                }
                            )
                        }
                    }
                }

                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier
                        .clip(RoundedCornerShape(6.dp))
                        .background(StudioColors.BgCard.copy(alpha = 0.92f))
                        .clickable(onClick = onToggleSectionMustHit)
                        .padding(horizontal = 7.dp, vertical = 3.dp)
                ) {
                    Icon(Icons.Rounded.SwapHoriz, null, tint = StudioColors.AccentAmber, modifier = Modifier.size(13.dp))
                    Spacer(Modifier.width(3.dp))
                    Text(
                        text = if (sec.mustHitSection) "Jugador" else "Oponente",
                        color = if (sec.mustHitSection) StudioColors.AccentEmerald else StudioColors.AccentCoral,
                        fontSize = 10.sp,
                        fontWeight = FontWeight.Bold
                    )
                }
            }
        }
    }
}

@Composable
private fun SustainConnectorCanvas(laneIndex: Int) {
    val laneColor = StudioColors.LaneColors[laneIndex]
    Canvas(modifier = Modifier.fillMaxSize()) {
        val beamW = (size.width * 0.34f).coerceAtLeast(8f)
        val coreW = (size.width * 0.13f).coerceAtLeast(3f)
        drawRect(
            color = laneColor.copy(alpha = 0.38f),
            topLeft = Offset((size.width - beamW) / 2f, 0f),
            size = Size(beamW, size.height)
        )
        drawRect(
            color = Color.White.copy(alpha = 0.24f),
            topLeft = Offset((size.width - coreW) / 2f, 0f),
            size = Size(coreW, size.height)
        )
    }
}
