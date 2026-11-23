// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/AudioAdaptScreen.kt
// Propósito: Pantalla independiente del Calibrador de Sonido (después de preparar el escenario
// y antes de jugar) que muestra el reconocimiento de la salida de audio, el balance de
// frecuencias de la canción y los ajustes aplicados para el dispositivo.

package com.beatforge.studio.ui.screens

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
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
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.BluetoothAudio
import androidx.compose.material.icons.rounded.Equalizer
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Headphones
import androidx.compose.material.icons.rounded.Speaker
import androidx.compose.material.icons.rounded.Usb
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.AudioAdaptProgress
import com.beatforge.studio.AudioAdaptationReportSnapshot
import com.beatforge.studio.ui.components.StudioEqualizerBars
import com.beatforge.studio.ui.components.StudioGlowingProgressBar
import com.beatforge.studio.ui.components.StudioOrbitProgressRing
import com.beatforge.studio.ui.theme.StudioColors
import java.util.Locale
import kotlin.math.roundToInt

@Composable
fun AudioAdaptScreen(
    progress: AudioAdaptProgress,
    report: AudioAdaptationReportSnapshot?
) {
    val targetProgress = progress.progress.coerceIn(0f, 1f)
    val animatedProgress by animateFloatAsState(
        targetValue = targetProgress,
        animationSpec = tween(durationMillis = 180),
        label = "audioAdaptProgress"
    )
    val pctInt = (animatedProgress * 100f).toInt().coerceIn(0, 100)

    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center
    ) {
        Surface(
            modifier = Modifier
                .fillMaxWidth(0.78f)
                .padding(14.dp),
            shape = RoundedCornerShape(18.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.AccentEmerald.copy(alpha = 0.45f))
        ) {
            Column(
                modifier = Modifier.padding(20.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.weight(1f)
                    ) {
                        val routeIcon = when (report?.device?.route) {
                            "usb_audio" -> Icons.Rounded.Usb
                            "wired_headphones" -> Icons.Rounded.Headphones
                            "bluetooth_a2dp", "bluetooth_le" -> Icons.Rounded.BluetoothAudio
                            "builtin_speaker" -> Icons.Rounded.Speaker
                            else -> Icons.Rounded.GraphicEq
                        }
                        StudioOrbitProgressRing(
                            progress = animatedProgress,
                            primaryColor = StudioColors.AccentEmerald,
                            secondaryColor = StudioColors.AccentCyan,
                            size = 56.dp,
                            strokeWidth = 4.dp
                        ) {
                            Icon(
                                imageVector = routeIcon,
                                contentDescription = null,
                                tint = StudioColors.AccentEmerald,
                                modifier = Modifier.size(22.dp)
                            )
                        }
                        Spacer(Modifier.width(12.dp))
                        Column {
                            Text(
                                text = "Calibrando Sonido...",
                                color = StudioColors.TextPrimary,
                                fontWeight = FontWeight.Bold,
                                fontSize = 17.sp,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis
                            )
                            Text(
                                text = if (report != null) {
                                    "${report.device.manufacturer} ${report.device.model} · ${report.device.routeLabel}"
                                } else {
                                    "Ajustando volumen y claridad para tu dispositivo"
                                },
                                color = StudioColors.TextMuted,
                                fontSize = 11.sp,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis
                            )
                        }
                    }

                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(10.dp)
                    ) {
                        StudioEqualizerBars(
                            barCount = 4,
                            primaryColor = StudioColors.AccentEmerald,
                            secondaryColor = StudioColors.AccentCyan,
                            maxHeight = 20.dp,
                            barWidth = 4.dp
                        )
                        Surface(
                            shape = RoundedCornerShape(8.dp),
                            color = StudioColors.BgCard,
                            border = BorderStroke(1.dp, StudioColors.AccentEmerald.copy(alpha = 0.35f))
                        ) {
                            Text(
                                text = "$pctInt%",
                                color = StudioColors.TextPrimary,
                                fontWeight = FontWeight.Bold,
                                fontSize = 12.sp,
                                modifier = Modifier.padding(horizontal = 9.dp, vertical = 4.dp)
                            )
                        }
                    }
                }

                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = progress.phase.ifBlank { "Optimizando el sonido para tu salida de audio..." },
                            color = StudioColors.AccentEmerald,
                            fontWeight = FontWeight.SemiBold,
                            fontSize = 12.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f)
                        )
                        if (report != null && report.cacheReused) {
                            Spacer(Modifier.width(8.dp))
                            Text(
                                text = "Listo al instante",
                                color = StudioColors.AccentCyan,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.Bold
                            )
                        }
                    }

                    StudioGlowingProgressBar(
                        progress = animatedProgress,
                        primaryColor = StudioColors.AccentEmerald,
                        secondaryColor = StudioColors.AccentCyan,
                        height = 8.dp
                    )

                    if (progress.detail.isNotBlank()) {
                        Text(
                            text = progress.detail,
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis
                        )
                    }
                }

                if (report != null && !report.bypassed) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(10.dp)
                    ) {
                        // Tarjeta 1: Salida de Audio Detectada
                        Surface(
                            modifier = Modifier.weight(1f),
                            shape = RoundedCornerShape(10.dp),
                            color = StudioColors.BgCard,
                            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
                        ) {
                            Column(
                                modifier = Modifier.padding(10.dp),
                                verticalArrangement = Arrangement.spacedBy(4.dp)
                            ) {
                                Text(
                                    text = "Salida de Audio",
                                    color = StudioColors.AccentCyan,
                                    fontWeight = FontWeight.Bold,
                                    fontSize = 11.sp
                                )
                                Text(
                                    text = "${report.device.routeLabel} · ${report.device.effectiveSampleRate} Hz",
                                    color = StudioColors.TextPrimary,
                                    fontWeight = FontWeight.SemiBold,
                                    fontSize = 12.sp,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis
                                )
                                val channelText = if (report.device.isMonoSpeaker) {
                                    "1 canal (Mono)"
                                } else {
                                    "${report.device.effectiveChannelCount} canales Estéreo"
                                }
                                val latencyText = if (report.device.lowLatencyActive) {
                                    "Respuesta rápida"
                                } else {
                                    "Estándar"
                                }
                                Text(
                                    text = "$channelText · $latencyText",
                                    color = StudioColors.TextMuted,
                                    fontSize = 10.sp
                                )
                            }
                        }

                        // Tarjeta 2: Balance de Frecuencias de la Canción
                        val analysis = report.songAnalysis
                        Surface(
                            modifier = Modifier.weight(1f),
                            shape = RoundedCornerShape(10.dp),
                            color = StudioColors.BgCard,
                            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
                        ) {
                            Column(
                                modifier = Modifier.padding(10.dp),
                                verticalArrangement = Arrangement.spacedBy(5.dp)
                            ) {
                                Text(
                                    text = "Balance de Frecuencias",
                                    color = StudioColors.AccentAmber,
                                    fontWeight = FontWeight.Bold,
                                    fontSize = 11.sp
                                )
                                if (analysis != null) {
                                    Text(
                                        text = String.format(
                                            Locale.US,
                                            "Nivel %.1f · Dinámica %.1f",
                                            analysis.integratedLufs,
                                            analysis.loudnessRangeLu
                                        ),
                                        color = StudioColors.TextPrimary,
                                        fontWeight = FontWeight.SemiBold,
                                        fontSize = 11.sp
                                    )
                                    val bandValues = listOf(
                                        "Graves" to analysis.bands.subBass2090Hz,
                                        "Cuerpo" to analysis.bands.bass90220Hz,
                                        "Medios" to analysis.bands.lowMid220600Hz,
                                        "Voz" to analysis.bands.presence2k4kHz,
                                        "Brillo" to analysis.bands.air6k14kHz
                                    )
                                    Row(
                                        modifier = Modifier
                                            .fillMaxWidth()
                                            .height(26.dp),
                                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                                        verticalAlignment = Alignment.Bottom
                                    ) {
                                        bandValues.forEach { (label, ratio) ->
                                            val animatedBand by animateFloatAsState(
                                                targetValue = ratio.coerceIn(0.08f, 1f),
                                                animationSpec = tween(durationMillis = 360),
                                                label = "band_$label"
                                            )
                                            Column(
                                                modifier = Modifier.weight(1f),
                                                horizontalAlignment = Alignment.CenterHorizontally
                                            ) {
                                                Box(
                                                    modifier = Modifier
                                                        .fillMaxWidth()
                                                        .height(14.dp)
                                                        .background(StudioColors.BgDark, RoundedCornerShape(3.dp)),
                                                    contentAlignment = Alignment.BottomCenter
                                                ) {
                                                    Box(
                                                        modifier = Modifier
                                                            .fillMaxWidth()
                                                            .fillMaxHeight(animatedBand)
                                                            .background(
                                                                StudioColors.AccentEmerald,
                                                                RoundedCornerShape(3.dp)
                                                            )
                                                    )
                                                }
                                                Text(
                                                    text = label,
                                                    color = StudioColors.TextMuted,
                                                    fontSize = 8.sp,
                                                    maxLines = 1
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Tarjeta 3: Ajuste de Sonido Aplicado
                        val plan = report.dspPlan
                        Surface(
                            modifier = Modifier.weight(1f),
                            shape = RoundedCornerShape(10.dp),
                            color = StudioColors.BgCard,
                            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
                        ) {
                            Column(
                                modifier = Modifier.padding(10.dp),
                                verticalArrangement = Arrangement.spacedBy(4.dp)
                            ) {
                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    Icon(
                                        imageVector = Icons.Rounded.Equalizer,
                                        contentDescription = null,
                                        tint = StudioColors.AccentEmerald,
                                        modifier = Modifier.size(14.dp)
                                    )
                                    Spacer(Modifier.width(4.dp))
                                    Text(
                                        text = plan?.profileName?.ifBlank { "Ajuste Aplicado" } ?: "Ajuste Aplicado",
                                        color = StudioColors.AccentEmerald,
                                        fontWeight = FontWeight.Bold,
                                        fontSize = 11.sp,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis
                                    )
                                }
                                if (plan != null) {
                                    val bandsSummary = plan.eqBands.take(3).joinToString(" · ") { band ->
                                        val sign = if (band.gainDb >= 0f) "+" else ""
                                        "${band.freqHz.roundToInt()}Hz $sign${String.format(Locale.US, "%.1f", band.gainDb)}dB"
                                    }.ifBlank { "Sin cambios necesarios" }
                                    Text(
                                        text = bandsSummary,
                                        color = StudioColors.TextPrimary,
                                        fontWeight = FontWeight.SemiBold,
                                        fontSize = 11.sp,
                                        maxLines = 2,
                                        overflow = TextOverflow.Ellipsis
                                    )
                                    val extras = buildList {
                                        if (plan.highPrecisionF32Path) {
                                            add("Ruta 32f")
                                        }
                                        add("Filtro ${plan.highpassHz.roundToInt()}Hz")
                                        if (plan.resampleNeeded) {
                                            add("Adaptado a ${plan.targetSampleRate}Hz")
                                        }
                                        if (plan.monoFoldEnabled) {
                                            add("Mezcla Mono")
                                        } else {
                                            if (plan.stereoWidth > 1.02f) {
                                                add("Amplitud ${(plan.stereoWidth * 100f).roundToInt()}%")
                                            }
                                            if (plan.crossfeedStrength > 0.01f) {
                                                add("Crossfeed ${(plan.crossfeedStrength * 100f).roundToInt()}%")
                                            }
                                        }
                                        add("Lookahead ${String.format(Locale.US, "%.0f", plan.limiterLookaheadMs)}ms")
                                        add("Límite ${String.format(Locale.US, "%.1f", plan.ceilingDbtp)} dB")
                                    }.joinToString(" · ")
                                    Text(
                                        text = extras,
                                        color = Color(0xFFA7F3D0),
                                        fontSize = 10.sp,
                                        maxLines = 3,
                                        overflow = TextOverflow.Ellipsis
                                    )
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
