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
import com.beatforge.studio.NoteSpecialType
import com.beatforge.studio.ui.theme.StudioColors
import java.util.Locale
import kotlin.math.roundToInt

@Composable
fun ChartToolsPanel(
    chart: ChartSnapshot,
    activeSection: ChartSectionSnapshot?,
    selectedSustainSteps: Int,
    selectedNoteType: NoteSpecialType,
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
    onSelectNoteType: (NoteSpecialType) -> Unit,
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

            // Tipo de Nota Especial (Normal, Nota X Daño, Muerte X, Esquive, Alt, Sin Anim)
            Text("Tipo de nota al colocar:", color = StudioColors.TextMuted, fontSize = 11.sp)
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                NoteSpecialType.entries.chunked(3).forEach { rowTypes ->
                    Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.fillMaxWidth()) {
                        rowTypes.forEach { nType ->
                            val isSel = selectedNoteType == nType
                            val chipColor = when (nType) {
                                NoteSpecialType.Normal -> StudioColors.AccentCyan
                                NoteSpecialType.Hurt -> StudioColors.AccentCoral
                                NoteSpecialType.Instakill -> Color(0xFFFF1744)
                                NoteSpecialType.Dodge -> StudioColors.AccentAmber
                                NoteSpecialType.AltAnim -> StudioColors.AccentPurple
                                NoteSpecialType.NoAnim -> StudioColors.TextMuted
                            }
                            Box(
                                modifier = Modifier
                                    .weight(1f)
                                    .height(28.dp)
                                    .clip(RoundedCornerShape(7.dp))
                                    .background(if (isSel) chipColor.copy(alpha = 0.22f) else StudioColors.BgCard)
                                    .border(
                                        width = 1.dp,
                                        color = if (isSel) chipColor else StudioColors.BorderSubtle,
                                        shape = RoundedCornerShape(7.dp)
                                    )
                                    .clickable { onSelectNoteType(nType) },
                                contentAlignment = Alignment.Center
                            ) {
                                Text(
                                    text = nType.label,
                                    color = if (isSel) chipColor else StudioColors.TextMuted,
                                    fontSize = 9.sp,
                                    fontWeight = if (isSel) FontWeight.Bold else FontWeight.Medium,
                                    maxLines = 1,
                                    softWrap = false
                                )
                            }
                        }
                    }
                }
            }

            // Módulo de Reproducción + Hilo descendente + Seguimiento aislado para evitar recomposiciones a 60 Hz
            ChartPlayheadToolCard(
                activeSectionIndex = activeSection?.index ?: 0,
                activeSectionStartMs = activeSection?.startMs ?: 0.0,
                audioPreviewPlaying = audioPreviewPlaying,
                playheadState = playheadState,
                onPlayOrPausePlayhead = onPlayOrPausePlayhead,
                onPlayFromSection = onPlayFromSection,
                onRewindPlayheadToStart = onRewindPlayheadToStart,
                onToggleFollowPlayhead = onToggleFollowPlayhead,
                onOpenCueAtPlayhead = onOpenCueAtPlayhead
            )

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
                            val turnLabel = if (sec.mustHitSection) "Jugador" else "Oponente"
                            val camTag = sec.cameraSlot?.let { " · Cam $it" } ?: ""
                            Text(
                                text = "$turnLabel$camTag",
                                color = if (sec.cameraSlot != null) {
                                    StudioColors.AccentCyan
                                } else if (sec.mustHitSection) {
                                    StudioColors.AccentEmerald
                                } else {
                                    StudioColors.AccentCoral
                                },
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

/**
 * Tarjeta modular de transporte y seguimiento del hilo reproductor.
 * Aísla las lecturas mutables de `ChartPlayheadState` (`posMs`, `activeTarget`, `followThread`)
 * en sub-composables hoja para evitar recomponer botones, chips y la lista de secciones a 60 Hz.
 */
@Composable
private fun ChartPlayheadToolCard(
    activeSectionIndex: Int,
    activeSectionStartMs: Double,
    audioPreviewPlaying: Boolean,
    playheadState: ChartPlayheadState,
    onPlayOrPausePlayhead: () -> Unit,
    onPlayFromSection: (Double) -> Unit,
    onRewindPlayheadToStart: () -> Unit,
    onToggleFollowPlayhead: () -> Unit,
    onOpenCueAtPlayhead: () -> Unit
) {
    Surface(
        shape = RoundedCornerShape(8.dp),
        color = StudioColors.BgCard,
        border = BorderStroke(
            width = 1.dp,
            color = if (audioPreviewPlaying) StudioColors.AccentEmerald.copy(alpha = 0.65f) else StudioColors.BorderSubtle
        )
    ) {
        Column(modifier = Modifier.padding(8.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            ChartPlayheadStatusHeader(
                audioPreviewPlaying = audioPreviewPlaying,
                playheadState = playheadState
            )

            Row(horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                Button(
                    onClick = onPlayOrPausePlayhead,
                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = if (audioPreviewPlaying) StudioColors.AccentAmber else StudioColors.AccentEmerald,
                        contentColor = Color(0xFF0A1410)
                    ),
                    modifier = Modifier
                        .weight(1.25f)
                        .height(31.dp)
                ) {
                    Icon(
                        imageVector = if (audioPreviewPlaying) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
                        contentDescription = null,
                        modifier = Modifier.size(15.dp)
                    )
                    Spacer(Modifier.width(3.dp))
                    Text(
                        text = if (audioPreviewPlaying) "Pausar" else "Reproducir",
                        fontSize = 10.sp,
                        fontWeight = FontWeight.Bold,
                        maxLines = 1,
                        softWrap = false
                    )
                }

                OutlinedButton(
                    onClick = onRewindPlayheadToStart,
                    contentPadding = PaddingValues(0.dp),
                    modifier = Modifier.size(31.dp)
                ) {
                    Icon(Icons.Rounded.SkipPrevious, contentDescription = "Inicio", modifier = Modifier.size(15.dp))
                }

                ChartPlayheadFollowChip(
                    playheadState = playheadState,
                    onToggleFollowPlayhead = onToggleFollowPlayhead
                )
            }

            Row(horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                OutlinedButton(
                    onClick = { onPlayFromSection(activeSectionStartMs) },
                    contentPadding = PaddingValues(horizontal = 6.dp, vertical = 2.dp),
                    modifier = Modifier
                        .weight(1f)
                        .height(27.dp)
                ) {
                    Text(
                        text = "Desde Sec ${activeSectionIndex + 1}",
                        fontSize = 9.sp,
                        maxLines = 1,
                        softWrap = false
                    )
                }

                OutlinedButton(
                    onClick = onOpenCueAtPlayhead,
                    contentPadding = PaddingValues(horizontal = 6.dp, vertical = 2.dp),
                    modifier = Modifier
                        .weight(1f)
                        .height(27.dp)
                ) {
                    Text(
                        text = "+ Evento en hilo",
                        fontSize = 9.sp,
                        maxLines = 1,
                        softWrap = false
                    )
                }
            }
        }
    }
}

@Composable
private fun ChartPlayheadStatusHeader(
    audioPreviewPlaying: Boolean,
    playheadState: ChartPlayheadState
) {
    val activeStepInSec = playheadState.activeTarget?.stepRow?.stepInSec ?: 0
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text(
            text = "Hilo y Audio",
            color = if (audioPreviewPlaying) StudioColors.AccentEmerald else StudioColors.TextPrimary,
            fontSize = 11.sp,
            fontWeight = FontWeight.Bold
        )
        Text(
            text = "${formatChartTimeMs(playheadState.posMs)} · Paso #$activeStepInSec",
            color = StudioColors.AccentCyan,
            fontSize = 10.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            softWrap = false
        )
    }
}

@Composable
private fun ChartPlayheadFollowChip(
    playheadState: ChartPlayheadState,
    onToggleFollowPlayhead: () -> Unit
) {
    val following = playheadState.followThread
    Box(
        modifier = Modifier
            .height(31.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(
                if (following) StudioColors.AccentCyan.copy(alpha = 0.18f) else StudioColors.BgPanel
            )
            .border(
                width = 1.dp,
                color = if (following) StudioColors.AccentCyan else StudioColors.BorderSubtle,
                shape = RoundedCornerShape(8.dp)
            )
            .clickable(onClick = onToggleFollowPlayhead)
            .padding(horizontal = 8.dp),
        contentAlignment = Alignment.Center
    ) {
        Row(
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(3.dp)
        ) {
            Icon(
                imageVector = if (following) Icons.Rounded.GpsFixed else Icons.Rounded.GpsOff,
                contentDescription = null,
                tint = if (following) StudioColors.AccentCyan else StudioColors.TextMuted,
                modifier = Modifier.size(13.dp)
            )
            Text(
                text = if (following) "Seguir" else "Libre",
                color = if (following) StudioColors.AccentCyan else StudioColors.TextMuted,
                fontSize = 9.sp,
                fontWeight = FontWeight.Bold,
                maxLines = 1,
                softWrap = false
            )
        }
    }
}

