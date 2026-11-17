// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/chart/ChartToolsPanel.kt
// Propósito: Panel lateral izquierdo mejorado del Editor de Chart con tarjetas modulares para
// importar/exportar JSON, acceso directo a `stage_events.lua` y `Config .json`, ajuste de BPM,
// selector de longitud de notas sostenidas, preescucha de audio y lista rápida de secciones.

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
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Code
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.GpsFixed
import androidx.compose.material.icons.rounded.GpsOff
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.RestartAlt
import androidx.compose.material.icons.rounded.SkipPrevious
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
import java.util.Locale
import kotlin.math.roundToInt

@Composable
fun ChartToolsPanel(
    chart: ChartSnapshot,
    activeSection: ChartSectionSnapshot?,
    selectedSustainSteps: Int,
    selectedSectionIdx: Int,
    sectionPositions: Map<Int, Int>,
    audioPreviewPlaying: Boolean,
    playheadState: ChartPlayheadState,
    onRequestImport: (ImportKind) -> Unit,
    onRequestExportJson: () -> Unit,
    onOpenDetailsDialog: () -> Unit,
    onOpenStageEventsScript: () -> Unit,
    onRestoreDemoChart: () -> Unit,
    onSetChartBpm: (Double) -> Unit,
    onSelectSustainSteps: (Int) -> Unit,
    onPlayOrPausePlayhead: () -> Unit,
    onPlayFromSection: (Double) -> Unit,
    onRewindPlayheadToStart: () -> Unit,
    onToggleFollowPlayhead: () -> Unit,
    onOpenCueAtPlayhead: () -> Unit,
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
            // Tarjeta resumen de la pista
            Surface(
                shape = RoundedCornerShape(9.dp),
                color = StudioColors.BgCard,
                border = BorderStroke(1.dp, StudioColors.BorderSubtle)
            ) {
                Column(modifier = Modifier.padding(9.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                    Text(
                        text = chart.title,
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 14.sp,
                        maxLines = 1
                    )
                    Text(
                        text = "${chart.totalNotes} notas (${chart.opponentNotes} Rival · ${chart.playerNotes} Jugador · ${chart.sustainNotes} largas)",
                        color = StudioColors.TextMuted,
                        fontSize = 10.sp
                    )
                }
            }

            // Importar / Exportar JSON
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
                        contentColor = Color(0xFF1F1200)
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
                    Text("Config .json (${chart.events.size})", fontSize = 10.sp, maxLines = 1, softWrap = false)
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

            // Acceso rápido a stage_events.lua y guía de la columna '#'
            Surface(
                shape = RoundedCornerShape(8.dp),
                color = StudioColors.AccentPurple.copy(alpha = 0.11f),
                border = BorderStroke(1.dp, StudioColors.AccentPurple.copy(alpha = 0.45f)),
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable(onClick = onOpenStageEventsScript)
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 8.dp, vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    Icon(Icons.Rounded.AutoAwesome, null, tint = StudioColors.AccentPurple, modifier = Modifier.size(15.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = "Toca '#' en la rejilla: Flashes y Swaps",
                            color = StudioColors.TextPrimary,
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Bold
                        )
                        Text(
                            text = "Sincronizado con stage_events.lua",
                            color = StudioColors.AccentPurple,
                            fontSize = 9.sp
                        )
                    }
                    Icon(Icons.Rounded.Code, null, tint = StudioColors.AccentPurple, modifier = Modifier.size(14.dp))
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
                listOf(0, 2, 4, 8, 16).forEach { steps ->
                    val isSel = selectedSustainSteps == steps
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .height(29.dp)
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
                            text = if (steps == 0) "1x" else "${steps}p",
                            color = if (isSel) StudioColors.AccentAmber else StudioColors.TextMuted,
                            fontSize = 10.sp,
                            fontWeight = if (isSel) FontWeight.Bold else FontWeight.Medium,
                            maxLines = 1,
                            softWrap = false
                        )
                    }
                }
            }

            // Módulo de Reproducción + Hilo descendente + Seguimiento para precisar Flashes y Swaps
            val currentTarget = playheadState.activeTarget
            val currentTimeText = formatChartTimeMs(playheadState.posMs)
            val currentStepLabel = currentTarget?.let {
                "Sec ${it.stepRow.section.index + 1} · #${it.stepRow.stepInSec}"
            } ?: "Sec ${(activeSection?.index ?: 0) + 1} · #0"

            Surface(
                shape = RoundedCornerShape(9.dp),
                color = StudioColors.BgCard,
                border = BorderStroke(
                    1.dp,
                    if (audioPreviewPlaying) StudioColors.AccentEmerald.copy(alpha = 0.7f)
                    else StudioColors.BorderSubtle
                )
            ) {
                Column(
                    modifier = Modifier.padding(8.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = if (audioPreviewPlaying) "Hilo bajando" else if (playheadState.posMs > 1.0) "En pausa" else "Reproductor",
                            color = if (audioPreviewPlaying) StudioColors.AccentEmerald else StudioColors.TextPrimary,
                            fontSize = 11.sp,
                            fontWeight = FontWeight.Bold
                        )
                        Text(
                            text = "$currentTimeText · $currentStepLabel",
                            color = StudioColors.AccentCyan,
                            fontSize = 10.sp,
                            fontWeight = FontWeight.SemiBold,
                            maxLines = 1,
                            softWrap = false
                        )
                    }

                    Row(horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                        Button(
                            onClick = onPlayOrPausePlayhead,
                            contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                            colors = ButtonDefaults.buttonColors(
                                containerColor = if (audioPreviewPlaying) StudioColors.AccentAmber else StudioColors.AccentEmerald,
                                contentColor = Color(0xFF0A1410)
                            ),
                            modifier = Modifier
                                .weight(1f)
                                .height(34.dp)
                        ) {
                            Icon(
                                imageVector = if (audioPreviewPlaying) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
                                contentDescription = null,
                                modifier = Modifier.size(16.dp)
                            )
                            Spacer(Modifier.width(4.dp))
                            Text(
                                text = when {
                                    audioPreviewPlaying -> "Pausar"
                                    playheadState.posMs > 1.0 -> "Reanudar"
                                    else -> "Reproducir"
                                },
                                fontSize = 11.sp,
                                fontWeight = FontWeight.Bold,
                                maxLines = 1,
                                softWrap = false
                            )
                        }

                        OutlinedButton(
                            onClick = onRewindPlayheadToStart,
                            contentPadding = PaddingValues(0.dp),
                            modifier = Modifier.size(34.dp)
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.SkipPrevious,
                                contentDescription = "Volver a 0:00",
                                tint = StudioColors.TextMuted,
                                modifier = Modifier.size(16.dp)
                            )
                        }
                    }

                    Row(horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                        OutlinedButton(
                            onClick = onToggleFollowPlayhead,
                            contentPadding = PaddingValues(horizontal = 6.dp, vertical = 4.dp),
                            border = BorderStroke(
                                1.dp,
                                if (playheadState.followThread) StudioColors.AccentCyan else StudioColors.BorderSubtle
                            ),
                            modifier = Modifier
                                .weight(1f)
                                .height(31.dp)
                        ) {
                            Icon(
                                imageVector = if (playheadState.followThread) Icons.Rounded.GpsFixed else Icons.Rounded.GpsOff,
                                contentDescription = null,
                                tint = if (playheadState.followThread) StudioColors.AccentCyan else StudioColors.TextMuted,
                                modifier = Modifier.size(13.dp)
                            )
                            Spacer(Modifier.width(4.dp))
                            Text(
                                text = if (playheadState.followThread) "Seguir: ON" else "Seguir: OFF",
                                color = if (playheadState.followThread) StudioColors.AccentCyan else StudioColors.TextMuted,
                                fontSize = 10.sp,
                                fontWeight = FontWeight.Bold,
                                maxLines = 1,
                                softWrap = false
                            )
                        }

                        OutlinedButton(
                            onClick = { onPlayFromSection(activeSection?.startMs ?: 0.0) },
                            contentPadding = PaddingValues(horizontal = 6.dp, vertical = 4.dp),
                            modifier = Modifier
                                .weight(0.9f)
                                .height(31.dp)
                        ) {
                            Text(
                                text = "Desde Sec.${(activeSection?.index ?: 0) + 1}",
                                fontSize = 10.sp,
                                maxLines = 1,
                                softWrap = false
                            )
                        }
                    }

                    if (currentTarget != null) {
                        OutlinedButton(
                            onClick = onOpenCueAtPlayhead,
                            contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp),
                            border = BorderStroke(1.dp, StudioColors.AccentPurple.copy(alpha = 0.55f)),
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(29.dp)
                        ) {
                            Icon(Icons.Rounded.AutoAwesome, null, tint = StudioColors.AccentPurple, modifier = Modifier.size(13.dp))
                            Spacer(Modifier.width(5.dp))
                            Text(
                                text = "Poner Flash / Swap en #${currentTarget.stepRow.stepInSec}",
                                color = StudioColors.TextPrimary,
                                fontSize = 10.sp,
                                fontWeight = FontWeight.SemiBold,
                                maxLines = 1,
                                softWrap = false
                            )
                        }
                    }
                }
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
                                text = "Sec ${sec.index + 1} · ${String.format(Locale.US, "%.1fs", sec.startMs / 1000.0)}",
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
