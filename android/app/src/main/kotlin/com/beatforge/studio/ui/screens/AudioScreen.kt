// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/AudioScreen.kt
// Propósito: Mezclador multicanal dedicado para hasta 3 pistas de audio (.ogg, .wav, .mp3)
// decodificadas en Rust y reproducidas con AAudio puro. Dispone de ancho completo horizontal
// para evitar recortes en botones, sliders, checkboxes y formas de onda.

package com.beatforge.studio.ui.screens

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
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
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material.icons.rounded.Timer
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
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
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
fun AudioScreen(
    snapshot: ProjectSnapshot,
    audioPreviewPlaying: Boolean,
    onRequestImport: (ImportKind) -> Unit,
    onToggleAudioPreview: () -> Unit,
    onUpdateTrackConfig: (slot: Int, gain: Float, muted: Boolean, duckOnMiss: Boolean) -> Unit,
    onRemoveTrack: (slot: Int) -> Unit,
    onSetAudioAdaptEnabled: (Boolean) -> Unit,
    onOpenCountdownDialog: () -> Unit
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(10.dp)
            .verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        Surface(
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 14.dp, vertical = 10.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.weight(1f)
                ) {
                    Icon(
                        imageVector = Icons.Rounded.GraphicEq,
                        contentDescription = null,
                        tint = StudioColors.AccentEmerald,
                        modifier = Modifier.size(20.dp)
                    )
                    Spacer(Modifier.width(8.dp))
                    Column {
                        Text(
                            text = "Mezclador de Audio (1 a 3 Pistas)",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 15.sp
                        )
                        Text(
                            text = "Soporta .ogg, .wav y .mp3 sincronizados en tiempo real",
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp
                        )
                    }
                }

                Spacer(Modifier.width(10.dp))

                OutlinedButton(
                    onClick = onOpenCountdownDialog,
                    shape = RoundedCornerShape(9.dp),
                    border = BorderStroke(1.dp, StudioColors.AccentAmber.copy(alpha = 0.55f)),
                    contentPadding = PaddingValues(horizontal = 12.dp, vertical = 6.dp),
                    modifier = Modifier.height(38.dp)
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Timer,
                        contentDescription = null,
                        tint = StudioColors.AccentAmber,
                        modifier = Modifier.size(16.dp)
                    )
                    Spacer(Modifier.width(5.dp))
                    Text(
                        text = "Voces 3-2-1",
                        color = StudioColors.AccentAmber,
                        fontSize = 11.5.sp,
                        fontWeight = FontWeight.Bold,
                        maxLines = 1,
                        softWrap = false
                    )
                }

                Spacer(Modifier.width(8.dp))

                Button(
                    onClick = onToggleAudioPreview,
                    shape = RoundedCornerShape(9.dp),
                    contentPadding = PaddingValues(horizontal = 14.dp, vertical = 6.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = if (audioPreviewPlaying) StudioColors.AccentCoral else StudioColors.AccentEmerald,
                        contentColor = if (audioPreviewPlaying) Color.White else Color(0xFF042117)
                    ),
                    modifier = Modifier.height(38.dp)
                ) {
                    Icon(
                        imageVector = if (audioPreviewPlaying) Icons.Rounded.Stop else Icons.Rounded.PlayArrow,
                        contentDescription = null,
                        modifier = Modifier.size(18.dp)
                    )
                    Spacer(Modifier.width(6.dp))
                    Text(
                        text = if (audioPreviewPlaying) "Detener Mezcla" else "Probar Mezcla",
                        fontSize = 12.sp,
                        fontWeight = FontWeight.Bold,
                        maxLines = 1,
                        softWrap = false
                    )
                }
            }
        }

        Surface(
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(
                1.dp,
                if (snapshot.audioAdaptEnabled) StudioColors.AccentEmerald.copy(alpha = 0.45f) else StudioColors.BorderSubtle
            )
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 14.dp, vertical = 10.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = "Adaptador Acústico Adaptativo (Rust DSP)",
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 13.sp
                    )
                    val lastReport = snapshot.lastAudioAdaptReport
                    val subtitle = when {
                        !snapshot.audioAdaptEnabled ->
                            "Desactivado: reproduce el audio compilado sin calibración por teléfono"
                        lastReport != null && !lastReport.bypassed ->
                            "Última calibración: ${lastReport.device.routeLabel} (${lastReport.device.effectiveSampleRate} Hz) · ${lastReport.dspPlan?.profileName ?: "Listo"}"
                        else ->
                            "Sondea AudioManager + AAudio, mide LUFS/FFT por canción y ajusta Biquad/Rubato antes de jugar"
                    }
                    Text(
                        text = subtitle,
                        color = if (snapshot.audioAdaptEnabled) StudioColors.AccentEmerald else StudioColors.TextMuted,
                        fontSize = 11.sp
                    )
                }

                Spacer(Modifier.width(12.dp))

                Switch(
                    checked = snapshot.audioAdaptEnabled,
                    onCheckedChange = onSetAudioAdaptEnabled,
                    colors = SwitchDefaults.colors(
                        checkedThumbColor = Color(0xFF042117),
                        checkedTrackColor = StudioColors.AccentEmerald
                    )
                )
            }
        }

        snapshot.audioTracks.forEach { track ->
            Surface(
                shape = RoundedCornerShape(12.dp),
                color = StudioColors.BgPanel,
                border = BorderStroke(1.dp, StudioColors.BorderSubtle)
            ) {
                Column(
                    modifier = Modifier.padding(12.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp)
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
                                fontSize = 14.sp
                            )
                            Text(
                                text = track.fileName ?: "Sin archivo (se usará ritmo demo si todas están vacías)",
                                color = if (track.fileName != null) StudioColors.AccentEmerald else StudioColors.TextMuted,
                                fontSize = 11.sp
                            )
                        }

                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            IconButton(
                                onClick = {
                                    onUpdateTrackConfig(
                                        track.slotIndex,
                                        track.gain,
                                        !track.muted,
                                        track.duckOnPlayerMiss
                                    )
                                },
                                modifier = Modifier.size(34.dp)
                            ) {
                                Icon(
                                    imageVector = if (track.muted) Icons.Rounded.VolumeOff else Icons.Rounded.VolumeUp,
                                    contentDescription = "Silenciar",
                                    tint = if (track.muted) StudioColors.AccentCoral else StudioColors.AccentCyan
                                )
                            }

                            if (track.fileName != null) {
                                IconButton(
                                    onClick = { onRemoveTrack(track.slotIndex) },
                                    modifier = Modifier.size(34.dp)
                                ) {
                                    Icon(
                                        imageVector = Icons.Rounded.DeleteOutline,
                                        contentDescription = "Quitar pista",
                                        tint = StudioColors.TextMuted
                                    )
                                }
                            }

                            Button(
                                onClick = { onRequestImport(ImportKind.AudioTrack(track.slotIndex)) },
                                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp),
                                colors = ButtonDefaults.buttonColors(
                                    containerColor = StudioColors.BgElevated,
                                    contentColor = StudioColors.TextPrimary
                                ),
                                modifier = Modifier.height(34.dp)
                            ) {
                                Icon(Icons.Rounded.AudioFile, null, modifier = Modifier.size(15.dp))
                                Spacer(Modifier.width(5.dp))
                                Text("Cargar Audio", fontSize = 11.sp, maxLines = 1, softWrap = false)
                            }
                        }
                    }

                    if (track.waveformPeaks.isNotEmpty()) {
                        Canvas(
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(34.dp)
                                .background(StudioColors.BgDark, RoundedCornerShape(6.dp))
                                .padding(horizontal = 6.dp, vertical = 4.dp)
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
                            fontSize = 12.sp,
                            modifier = Modifier.width(68.dp)
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
                        Spacer(Modifier.width(8.dp))
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
                        Text("Bajar al fallar", color = StudioColors.TextMuted, fontSize = 12.sp, maxLines = 1, softWrap = false)
                    }
                }
            }
        }
    }
}
