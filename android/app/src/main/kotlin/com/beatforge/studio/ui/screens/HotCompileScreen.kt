// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/HotCompileScreen.kt
// Propósito: Pantalla de preparación del escenario con progreso real reportado desde Rust
// (caché incremental por huella, optimización de imágenes, eventos y mezcla de audio).

package com.beatforge.studio.ui.screens

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.RocketLaunch
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.DeviceTier
import com.beatforge.studio.HotCompileProgress
import com.beatforge.studio.ui.components.StudioEqualizerBars
import com.beatforge.studio.ui.components.StudioGlowingProgressBar
import com.beatforge.studio.ui.components.StudioMilestoneStepper
import com.beatforge.studio.ui.components.StudioOrbitProgressRing
import com.beatforge.studio.ui.theme.StudioColors

@Composable
fun HotCompileScreen(
    tier: DeviceTier,
    progress: HotCompileProgress
) {
    val targetProgress = progress.progress.coerceIn(0f, 1f)
    val animatedProgress by animateFloatAsState(
        targetValue = targetProgress,
        animationSpec = tween(durationMillis = 180),
        label = "hotCompileProgress"
    )
    val pctInt = (animatedProgress * 100f).toInt().coerceIn(0, 100)
    val activeStep = when {
        animatedProgress < 0.65f -> 0
        animatedProgress < 0.80f -> 1
        animatedProgress < 0.90f -> 2
        else -> 3
    }

    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center
    ) {
        Surface(
            modifier = Modifier
                .fillMaxWidth(0.66f)
                .padding(16.dp),
            shape = RoundedCornerShape(18.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.45f))
        ) {
            Column(
                modifier = Modifier.padding(22.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(14.dp)
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
                        Surface(
                            shape = RoundedCornerShape(12.dp),
                            color = StudioColors.AccentCyan.copy(alpha = 0.14f),
                            border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.35f))
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.RocketLaunch,
                                contentDescription = null,
                                tint = StudioColors.AccentCyan,
                                modifier = Modifier
                                    .padding(9.dp)
                                    .size(24.dp)
                            )
                        }
                        Spacer(Modifier.width(12.dp))
                        Column {
                            Text(
                                text = "Preparando el Escenario...",
                                color = StudioColors.TextPrimary,
                                fontWeight = FontWeight.Bold,
                                fontSize = 18.sp,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis
                            )
                            Text(
                                text = "Ajustando calidad para tu dispositivo (${tier.label})",
                                color = StudioColors.TextMuted,
                                fontSize = 11.sp,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis
                            )
                        }
                    }

                    StudioOrbitProgressRing(
                        progress = animatedProgress,
                        primaryColor = StudioColors.AccentCyan,
                        secondaryColor = StudioColors.AccentEmerald,
                        size = 66.dp,
                        strokeWidth = 4.5.dp
                    ) {
                        Text(
                            text = "$pctInt%",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 13.sp
                        )
                    }
                }

                StudioMilestoneStepper(
                    steps = listOf("Imágenes", "Eventos", "Música", "Listo"),
                    activeIndex = activeStep,
                    accentColor = StudioColors.AccentEmerald
                )

                Column(
                    modifier = Modifier.fillMaxWidth(),
                    verticalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = progress.phase.ifBlank { "Optimizando recursos..." },
                            color = StudioColors.AccentCyan,
                            fontWeight = FontWeight.SemiBold,
                            fontSize = 12.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f)
                        )
                        Spacer(Modifier.width(10.dp))
                        StudioEqualizerBars(
                            barCount = 4,
                            primaryColor = StudioColors.AccentCyan,
                            secondaryColor = StudioColors.AccentEmerald,
                            maxHeight = 16.dp,
                            barWidth = 3.5.dp
                        )
                    }

                    StudioGlowingProgressBar(
                        progress = animatedProgress,
                        primaryColor = StudioColors.AccentCyan,
                        secondaryColor = StudioColors.AccentEmerald,
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
            }
        }
    }
}
