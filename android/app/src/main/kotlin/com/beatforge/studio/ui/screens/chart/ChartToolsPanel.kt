// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/chart/ChartToolsPanel.kt
// Propósito: Panel lateral izquierdo del editor de chart con título, estadísticas,
// importación/exportación JSON, acceso al modal Config .json, ajuste de BPM, selector
// de sustain, preescucha de audio y lista de salto rápido por secciones.

package com.beatforge.studio.ui.screens.chart

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.RestartAlt
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material.icons.rounded.UploadFile
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
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
import com.beatforge.studio.ChartSnapshot
import com.beatforge.studio.ImportKind
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

@Composable
fun ChartToolsPanel(
    chart: ChartSnapshot,
    activeSection: ChartSectionSnapshot?,
    selectedSustainSteps: Int,
    selectedSectionIdx: Int,
    sectionPositions: Map<Int, Int>,
    audioPreviewPlaying: Boolean,
    onRequestImport: (ImportKind) -> Unit,
    onRequestExportJson: () -> Unit,
    onOpenDetailsDialog: () -> Unit,
    onRestoreDemoChart: () -> Unit,
    onSetChartBpm: (Double) -> Unit,
    onSelectSustainSteps: (Int) -> Unit,
    onToggleAudioPreviewAtMs: (Double) -> Unit,
    onAddSection: (Boolean) -> Unit,
    onSelectSection: (secIndex: Int, rowIdx: Int) -> Unit,
    modifier: Modifier = Modifier
) {
    Surface(
        modifier = modifier.fillMaxHeight(),
        shape = RoundedCornerShape(12.dp),
        color = StudioColors.BgPanel,
        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
    ) {
        Column(
            modifier = Modifier
                .padding(10.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Text(
                text = chart.title,
                color = StudioColors.TextPrimary,
                fontWeight = FontWeight.Bold,
                fontSize = 14.sp,
                maxLines = 1
            )

            Text(
                text = "${chart.totalNotes} notas (${chart.opponentNotes} Rival · ${chart.playerNotes} Jugador)",
                color = StudioColors.TextMuted,
                fontSize = 11.sp
            )

            // Importar / Exportar / Demo
            Row(horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                Button(
                    onClick = { onRequestImport(ImportKind.Chart) },
                    contentPadding = PaddingValues(horizontal = 6.dp, vertical = 4.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = StudioColors.AccentCyan,
                        contentColor = Color(0xFF1F0C00)
                    ),
                    modifier = Modifier
                        .weight(1f)
                        .height(33.dp)
                ) {
                    Icon(Icons.Rounded.UploadFile, null, modifier = Modifier.size(14.dp))
                    Spacer(Modifier.width(3.dp))
                    Text("Importar", fontSize = 10.sp, fontWeight = FontWeight.Bold, maxLines = 1, softWrap = false)
                }

                Button(
                    onClick = onRequestExportJson,
                    contentPadding = PaddingValues(horizontal = 6.dp, vertical = 4.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = StudioColors.AccentIndigo,
                        contentColor = Color.White
                    ),
                    modifier = Modifier
                        .weight(1f)
                        .height(33.dp)
                ) {
                    Icon(Icons.Rounded.Download, null, modifier = Modifier.size(14.dp))
                    Spacer(Modifier.width(3.dp))
                    Text("Exportar", fontSize = 10.sp, fontWeight = FontWeight.Bold, maxLines = 1, softWrap = false)
                }
            }

            Row(horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                OutlinedButton(
                    onClick = onOpenDetailsDialog,
                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                    modifier = Modifier
                        .weight(1.1f)
                        .height(32.dp)
                ) {
                    Icon(Icons.Rounded.Tune, null, modifier = Modifier.size(14.dp))
                    Spacer(Modifier.width(4.dp))
                    Text("Config .json (${chart.events.size} ev.)", fontSize = 10.sp, maxLines = 1, softWrap = false)
                }

                OutlinedButton(
                    onClick = onRestoreDemoChart,
                    contentPadding = PaddingValues(horizontal = 6.dp, vertical = 4.dp),
                    modifier = Modifier
                        .weight(0.65f)
                        .height(32.dp)
                ) {
                    Icon(Icons.Rounded.RestartAlt, null, modifier = Modifier.size(14.dp))
                    Spacer(Modifier.width(2.dp))
                    Text("Demo", fontSize = 10.sp, maxLines = 1, softWrap = false)
                }
            }

            // Control de BPM
            Surface(
                shape = RoundedCornerShape(8.dp),
                color = StudioColors.BgCard,
                border = BorderStroke(1.dp, StudioColors.BorderSubtle)
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 8.dp, vertical = 4.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        text = "BPM: ${chart.bpm.roundToInt()}",
                        color = StudioColors.AccentAmber,
                        fontWeight = FontWeight.Bold,
                        fontSize = 13.sp
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                        OutlinedButton(
                            onClick = { onSetChartBpm((chart.bpm - 5.0).coerceAtLeast(40.0)) },
                            contentPadding = PaddingValues(0.dp),
                            modifier = Modifier.size(30.dp)
                        ) {
                            Text("-5", fontSize = 11.sp)
                        }
                        OutlinedButton(
                            onClick = { onSetChartBpm((chart.bpm + 5.0).coerceAtMost(320.0)) },
                            contentPadding = PaddingValues(0.dp),
                            modifier = Modifier.size(30.dp)
                        ) {
                            Text("+5", fontSize = 11.sp)
                        }
                    }
                }
            }

            // Duración de nota sostenida (Sustain)
            Text("Sustain al colocar (pasos):", color = StudioColors.TextMuted, fontSize = 11.sp)
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.fillMaxWidth()) {
                listOf(0, 2, 4, 8).forEach { steps ->
                    val isSel = selectedSustainSteps == steps
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .height(30.dp)
                            .clip(RoundedCornerShape(8.dp))
                            .background(
                                if (isSel) StudioColors.AccentAmber.copy(alpha = 0.22f)
                                else StudioColors.BgCard
                            )
                            .border(
                                width = 1.dp,
                                color = if (isSel) StudioColors.AccentAmber else StudioColors.BorderSubtle,
                                shape = RoundedCornerShape(8.dp)
                            )
                            .clickable { onSelectSustainSteps(steps) },
                        contentAlignment = Alignment.Center
                    ) {
                        Text(
                            text = if (steps == 0) "Normal" else "${steps}p",
                            color = if (isSel) StudioColors.AccentAmber else StudioColors.TextMuted,
                            fontSize = 11.sp,
                            fontWeight = if (isSel) FontWeight.Bold else FontWeight.Medium,
                            maxLines = 1,
                            softWrap = false
                        )
                    }
                }
            }

            // Botón escuchar audio desde la sección seleccionada
            Button(
                onClick = {
                    onToggleAudioPreviewAtMs(activeSection?.startMs ?: 0.0)
                },
                contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = if (audioPreviewPlaying) StudioColors.AccentCoral else StudioColors.AccentEmerald,
                    contentColor = if (audioPreviewPlaying) Color.White else Color(0xFF042117)
                ),
                modifier = Modifier
                    .fillMaxWidth()
                    .height(36.dp)
            ) {
                Icon(
                    imageVector = if (audioPreviewPlaying) Icons.Rounded.Stop else Icons.Rounded.PlayArrow,
                    contentDescription = null,
                    modifier = Modifier.size(16.dp)
                )
                Spacer(Modifier.width(6.dp))
                Text(
                    text = if (audioPreviewPlaying) "Detener Audio" else "Escuchar Sec. ${(activeSection?.index ?: 0) + 1}",
                    fontSize = 12.sp,
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                    softWrap = false
                )
            }

            HorizontalDivider(color = StudioColors.BorderSubtle)

            // Secciones (salto rápido + añadir nueva sección larga)
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text("Secciones (${chart.sections.size})", color = StudioColors.TextPrimary, fontWeight = FontWeight.Bold, fontSize = 12.sp)
                Button(
                    onClick = { onAddSection(true) },
                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = StudioColors.BgElevated,
                        contentColor = StudioColors.AccentCyan
                    ),
                    modifier = Modifier.height(28.dp)
                ) {
                    Icon(Icons.Rounded.Add, null, modifier = Modifier.size(14.dp))
                    Spacer(Modifier.width(2.dp))
                    Text("Sección", fontSize = 10.sp, maxLines = 1, softWrap = false)
                }
            }

            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                chart.sections.forEach { sec ->
                    val isSel = sec.index == selectedSectionIdx
                    Surface(
                        modifier = Modifier
                            .fillMaxWidth()
                            .clickable {
                                val rowIdx = sectionPositions[sec.index] ?: 0
                                onSelectSection(sec.index, rowIdx)
                            },
                        shape = RoundedCornerShape(7.dp),
                        color = if (isSel) StudioColors.AccentCyan.copy(alpha = 0.16f) else StudioColors.BgCard,
                        border = BorderStroke(1.dp, if (isSel) StudioColors.AccentCyan else StudioColors.BorderSubtle)
                    ) {
                        Row(
                            modifier = Modifier.padding(horizontal = 8.dp, vertical = 5.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween
                        ) {
                            Text(
                                text = "Sec ${sec.index + 1} · ${(sec.startMs / 1000.0).let { String.format("%.1fs", it) }}",
                                color = StudioColors.TextPrimary,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.SemiBold
                            )
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
    }
}
