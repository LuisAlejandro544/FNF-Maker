// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/ChartAudioScreen.kt
// Propósito: Pantalla de Chart & Mezclador de Audio de hasta 3 pistas independientes.
// Permite importar charts JSON estándar, visualizar un histograma real de densidad de notas
// en Compose Canvas y gestionar 3 pistas de audio decodificadas en Rust (AAudio puro).

package com.beatforge.studio.ui.screens

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AudioFile
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.RestartAlt
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material.icons.rounded.UploadFile
import androidx.compose.material.icons.rounded.VolumeOff
import androidx.compose.material.icons.rounded.VolumeUp
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CheckboxDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.ImportKind
import com.beatforge.studio.ProjectSnapshot
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

@Composable
fun ChartAudioScreen(
    snapshot: ProjectSnapshot,
    audioPreviewPlaying: Boolean,
    onRequestImport: (ImportKind) -> Unit,
    onRestoreDemoChart: () -> Unit,
    onToggleAudioPreview: () -> Unit,
    onUpdateTrackConfig: (slot: Int, gain: Float, muted: Boolean, duckOnMiss: Boolean) -> Unit,
    onRemoveTrack: (slot: Int) -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxSize()
            .padding(12.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // Columna izquierda: Importador de Chart + Histograma de densidad
        Surface(
            modifier = Modifier
                .weight(0.45f)
                .fillMaxHeight(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Column(
                modifier = Modifier
                    .padding(12.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = "Chart JSON",
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 15.sp
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                        OutlinedButton(
                            onClick = onRestoreDemoChart,
                            contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                            modifier = Modifier.height(34.dp)
                        ) {
                            Icon(Icons.Rounded.RestartAlt, null, modifier = Modifier.size(15.dp))
                            Spacer(Modifier.width(4.dp))
                            Text("Demo", fontSize = 11.sp)
                        }
                        Button(
                            onClick = { onRequestImport(ImportKind.Chart) },
                            contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                            colors = ButtonDefaults.buttonColors(
                                containerColor = StudioColors.AccentCyan,
                                contentColor = Color(0xFF041C24)
                            ),
                            modifier = Modifier.height(34.dp)
                        ) {
                            Icon(Icons.Rounded.UploadFile, null, modifier = Modifier.size(15.dp))
                            Spacer(Modifier.width(4.dp))
                            Text("Importar .json", fontSize = 11.sp, fontWeight = FontWeight.Bold)
                        }
                    }
                }

                val c = snapshot.chart
                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = StudioColors.BgCard,
                    border = BorderStroke(1.dp, StudioColors.BorderSubtle)
                ) {
                    Column(modifier = Modifier.padding(10.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text(
                            text = "Canción: ${c.title}",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.SemiBold,
                            fontSize = 13.sp
                        )
                        Text(
                            text = "BPM: ${c.bpm}  |  Secciones: ${c.sectionCount}  |  Duración: ${(c.durationMs / 1000.0).roundToInt()} s",
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp
                        )
                        Text(
                            text = "Notas Jugador: ${c.playerNotes}  |  Oponente: ${c.opponentNotes}  |  Sustains: ${c.sustainNotes}",
                            color = StudioColors.AccentCyan,
                            fontSize = 11.sp
                        )
                    }
                }

                Text(
                    text = "Densidad de Notas por Segmento",
                    color = StudioColors.TextMuted,
                    fontSize = 12.sp,
                    fontWeight = FontWeight.Medium
                )

                val buckets = c.densityBuckets
                Canvas(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(140.dp)
                        .background(StudioColors.BgDark, RoundedCornerShape(8.dp))
                        .padding(8.dp)
                ) {
                    if (buckets.isNotEmpty()) {
                        val maxVal = (buckets.maxOrNull() ?: 1).coerceAtLeast(1).toFloat()
                        val barW = size.width / buckets.size.toFloat()
                        buckets.forEachIndexed { idx, value ->
                            val ratio = value.toFloat() / maxVal
                            val h = (size.height * ratio).coerceAtLeast(2f)
                            val x = idx * barW
                            val y = size.height - h
                            drawRoundRect(
                                color = if (ratio > 0.72f) StudioColors.AccentCoral else StudioColors.AccentCyan,
                                topLeft = Offset(x + 1.5f, y),
                                size = Size((barW - 3f).coerceAtLeast(2f), h),
                                cornerRadius = CornerRadius(3f, 3f)
                            )
                        }
                    }
                }
            }
        }

        // Columna derecha: Mezclador de 3 pistas de audio
        Surface(
            modifier = Modifier
                .weight(0.55f)
                .fillMaxHeight(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Column(
                modifier = Modifier
                    .padding(12.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Rounded.GraphicEq, null, tint = StudioColors.AccentEmerald, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(6.dp))
                        Text(
                            text = "Mezclador Multicanal (3 Pistas Rust AAudio)",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 14.sp
                        )
                    }

                    Button(
                        onClick = onToggleAudioPreview,
                        shape = RoundedCornerShape(8.dp),
                        contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = if (audioPreviewPlaying) StudioColors.AccentCoral else StudioColors.AccentEmerald,
                            contentColor = Color(0xFF071713)
                        ),
                        modifier = Modifier.height(34.dp)
                    ) {
                        Icon(
                            imageVector = if (audioPreviewPlaying) Icons.Rounded.Stop else Icons.Rounded.PlayArrow,
                            contentDescription = null,
                            modifier = Modifier.size(16.dp)
                        )
                        Spacer(Modifier.width(4.dp))
                        Text(
                            text = if (audioPreviewPlaying) "Detener Preview" else "Probar Mezcla",
                            fontSize = 11.sp,
                            fontWeight = FontWeight.Bold
                        )
                    }
                }

                snapshot.audioTracks.forEach { track ->
                    Surface(
                        shape = RoundedCornerShape(10.dp),
                        color = StudioColors.BgCard,
                        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
                    ) {
                        Column(
                            modifier = Modifier.padding(10.dp),
                            verticalArrangement = Arrangement.spacedBy(4.dp)
                        ) {
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.SpaceBetween
                            ) {
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(
                                        text = track.roleLabel,
                                        color = StudioColors.TextPrimary,
                                        fontWeight = FontWeight.Bold,
                                        fontSize = 13.sp
                                    )
                                    Text(
                                        text = track.fileName ?: "Sin archivo (se usará sinte demo si todas están vacías)",
                                        color = if (track.fileName != null) StudioColors.AccentEmerald else StudioColors.TextMuted,
                                        fontSize = 11.sp
                                    )
                                }
                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    IconButton(
                                        onClick = {
                                            onUpdateTrackConfig(
                                                track.slotIndex,
                                                track.gain,
                                                !track.muted,
                                                track.duckOnPlayerMiss
                                            )
                                        },
                                        modifier = Modifier.size(32.dp)
                                    ) {
                                        Icon(
                                            imageVector = if (track.muted) Icons.Rounded.VolumeOff else Icons.Rounded.VolumeUp,
                                            contentDescription = "Silenciar",
                                            tint = if (track.muted) StudioColors.AccentCoral else StudioColors.AccentCyan,
                                            modifier = Modifier.size(18.dp)
                                        )
                                    }
                                    if (track.fileName != null) {
                                        IconButton(
                                            onClick = { onRemoveTrack(track.slotIndex) },
                                            modifier = Modifier.size(32.dp)
                                        ) {
                                            Icon(
                                                imageVector = Icons.Rounded.DeleteOutline,
                                                contentDescription = "Quitar pista",
                                                tint = StudioColors.TextMuted,
                                                modifier = Modifier.size(18.dp)
                                            )
                                        }
                                    }
                                    Spacer(Modifier.width(4.dp))
                                    Button(
                                        onClick = { onRequestImport(ImportKind.AudioTrack(track.slotIndex)) },
                                        contentPadding = PaddingValues(horizontal = 9.dp, vertical = 3.dp),
                                        colors = ButtonDefaults.buttonColors(
                                            containerColor = StudioColors.BgElevated,
                                            contentColor = StudioColors.TextPrimary
                                        ),
                                        modifier = Modifier.height(30.dp)
                                    ) {
                                        Icon(Icons.Rounded.AudioFile, null, modifier = Modifier.size(14.dp))
                                        Spacer(Modifier.width(4.dp))
                                        Text("Cargar", fontSize = 11.sp)
                                    }
                                }
                            }

                            if (track.waveformPeaks.isNotEmpty()) {
                                Canvas(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .height(28.dp)
                                        .background(StudioColors.BgDark, RoundedCornerShape(6.dp))
                                        .padding(horizontal = 4.dp, vertical = 3.dp)
                                ) {
                                    val peaks = track.waveformPeaks
                                    val step = size.width / peaks.size.toFloat()
                                    val midY = size.height / 2f
                                    peaks.forEachIndexed { idx, amp ->
                                        val h = (amp * size.height).coerceAtLeast(2f)
                                        drawRect(
                                            color = if (track.muted) StudioColors.TextMuted else StudioColors.AccentEmerald,
                                            topLeft = Offset(idx * step, midY - h / 2f),
                                            size = Size((step - 1f).coerceAtLeast(1.5f), h)
                                        )
                                    }
                                }
                            }

                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    text = "Vol: ${(track.gain * 100).roundToInt()}%",
                                    color = StudioColors.TextMuted,
                                    fontSize = 11.sp,
                                    modifier = Modifier.width(62.dp)
                                )
                                Slider(
                                    value = track.gain,
                                    onValueChange = { newGain ->
                                        onUpdateTrackConfig(
                                            track.slotIndex,
                                            newGain,
                                            track.muted,
                                            track.duckOnPlayerMiss
                                        )
                                    },
                                    valueRange = 0f..2f,
                                    modifier = Modifier.weight(1f),
                                    colors = SliderDefaults.colors(
                                        thumbColor = StudioColors.AccentCyan,
                                        activeTrackColor = StudioColors.AccentCyan
                                    )
                                )
                                Checkbox(
                                    checked = track.duckOnPlayerMiss,
                                    onCheckedChange = { checked ->
                                        onUpdateTrackConfig(
                                            track.slotIndex,
                                            track.gain,
                                            track.muted,
                                            checked
                                        )
                                    },
                                    colors = CheckboxDefaults.colors(checkedColor = StudioColors.AccentAmber)
                                )
                                Text("Bajar al fallar", color = StudioColors.TextMuted, fontSize = 11.sp)
                            }
                        }
                    }
                }
            }
        }
    }
}
