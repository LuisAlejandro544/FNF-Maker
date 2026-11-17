// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/HotCompileScreen.kt
// Propósito: Pantalla de preparación en caliente con progreso real reportado desde Rust
// (caché incremental por huella, conversión a WebP sin pérdida, precompilación de scripts y audio).

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
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.RocketLaunch
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.DeviceTier
import com.beatforge.studio.HotCompileProgress
import com.beatforge.studio.ui.theme.StudioColors

@Composable
fun HotCompileScreen(
    tier: DeviceTier,
    progress: HotCompileProgress
) {
    val targetProgress = progress.progress.coerceIn(0f, 1f)
    val animatedProgress by animateFloatAsState(
        targetValue = targetProgress,
        animationSpec = tween(durationMillis = 120),
        label = "hotCompileProgress"
    )
    val pctInt = (animatedProgress * 100f).toInt().coerceIn(0, 100)

    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center
    ) {
        Surface(
            modifier = Modifier
                .fillMaxWidth(0.58f)
                .padding(16.dp),
            shape = RoundedCornerShape(16.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.45f))
        ) {
            Column(
                modifier = Modifier.padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                Icon(
                    imageVector = Icons.Rounded.RocketLaunch,
                    contentDescription = null,
                    tint = StudioColors.AccentCyan,
                    modifier = Modifier.size(34.dp)
                )
                Text(
                    text = "Preparando Partida...",
                    color = StudioColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 18.sp
                )
                Text(
                    text = "Perfil: ${tier.label} · WebP sin pérdida + Caché Incremental",
                    color = StudioColors.TextMuted,
                    fontSize = 11.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )

                Spacer(Modifier.height(4.dp))

                Box(contentAlignment = Alignment.Center) {
                    CircularProgressIndicator(
                        progress = { animatedProgress },
                        color = StudioColors.AccentEmerald,
                        trackColor = StudioColors.BgDark,
                        strokeWidth = 4.dp,
                        modifier = Modifier.size(56.dp)
                    )
                    Text(
                        text = "$pctInt%",
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 13.sp
                    )
                }

                Column(
                    modifier = Modifier.fillMaxWidth(),
                    verticalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = progress.phase.ifBlank { "Compilando recursos..." },
                            color = StudioColors.AccentCyan,
                            fontWeight = FontWeight.SemiBold,
                            fontSize = 12.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f)
                        )
                        Text(
                            text = "$pctInt%",
                            color = StudioColors.TextMuted,
                            fontWeight = FontWeight.Bold,
                            fontSize = 12.sp
                        )
                    }

                    LinearProgressIndicator(
                        progress = { animatedProgress },
                        color = StudioColors.AccentCyan,
                        trackColor = StudioColors.BgDark,
                        modifier = Modifier
                            .fillMaxWidth()
                            .height(7.dp)
                            .clip(RoundedCornerShape(4.dp))
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
