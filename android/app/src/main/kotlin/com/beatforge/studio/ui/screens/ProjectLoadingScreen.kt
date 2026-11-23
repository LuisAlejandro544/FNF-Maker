// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/ProjectLoadingScreen.kt
// Propósito: Pantalla dedicada de carga que se muestra al seleccionar, crear o importar un
// proyecto desde el menú principal, informando el progreso real en `Dispatchers.IO` antes
// de abrir el estudio de edición con animaciones fluidas y mensajes claros.

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
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AudioFile
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material.icons.rounded.Memory
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.gfx.GfxCapabilities
import com.beatforge.studio.gfx.RenderBackendKind
import com.beatforge.studio.ui.components.StudioEqualizerBars
import com.beatforge.studio.ui.components.StudioGlowingProgressBar
import com.beatforge.studio.ui.components.StudioMilestoneStepper
import com.beatforge.studio.ui.components.StudioOrbitProgressRing
import com.beatforge.studio.ui.theme.StudioColors

@Composable
fun ProjectLoadingScreen(
    projectName: String,
    phase: String,
    progress: Float,
    gfxCapabilities: GfxCapabilities,
    preferredBackend: String
) {
    val clamped = progress.coerceIn(0f, 1f)
    val animatedProgress by animateFloatAsState(
        targetValue = clamped,
        animationSpec = tween(durationMillis = 220),
        label = "projectLoadingProgress"
    )
    val pct = (animatedProgress * 100f).toInt().coerceIn(0, 100)
    val backendKind = RenderBackendKind.fromKey(preferredBackend)
    val activeStep = when {
        animatedProgress < 0.45f -> 0
        animatedProgress < 0.80f -> 1
        else -> 2
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(StudioColors.BgDark),
        contentAlignment = Alignment.Center
    ) {
        Surface(
            modifier = Modifier
                .fillMaxWidth(0.68f)
                .padding(16.dp),
            shape = RoundedCornerShape(18.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.5f))
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
                                imageVector = Icons.Rounded.FolderOpen,
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
                                text = "Abriendo Proyecto",
                                color = StudioColors.TextMuted,
                                fontSize = 11.5.sp,
                                fontWeight = FontWeight.Medium
                            )
                            Text(
                                text = projectName.ifBlank { "BeatForge Mod" },
                                color = StudioColors.TextPrimary,
                                fontSize = 18.sp,
                                fontWeight = FontWeight.Bold,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis
                            )
                        }
                    }

                    StudioOrbitProgressRing(
                        progress = animatedProgress,
                        primaryColor = StudioColors.AccentCyan,
                        secondaryColor = StudioColors.AccentEmerald,
                        size = 64.dp,
                        strokeWidth = 4.5.dp
                    ) {
                        Text(
                            text = "$pct%",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 13.sp
                        )
                    }
                }

                StudioMilestoneStepper(
                    steps = listOf("Proyecto", "Personajes", "Gráficos"),
                    activeIndex = activeStep,
                    accentColor = StudioColors.AccentEmerald
                )

                Column(
                    modifier = Modifier.fillMaxWidth(),
                    verticalArrangement = Arrangement.spacedBy(7.dp)
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = phase.ifBlank { "Preparando los archivos del proyecto..." },
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
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    LoadingSubsystemBadge(
                        icon = Icons.Rounded.AudioFile,
                        label = "Audio del Estudio",
                        accent = StudioColors.AccentEmerald,
                        modifier = Modifier.weight(1f)
                    )
                    LoadingSubsystemBadge(
                        icon = Icons.Rounded.Layers,
                        label = "Personajes y Skins",
                        accent = StudioColors.AccentAmber,
                        modifier = Modifier.weight(1f)
                    )
                    LoadingSubsystemBadge(
                        icon = Icons.Rounded.Memory,
                        label = "Gráficos (${backendKind.shortBadge})",
                        accent = StudioColors.AccentPurple,
                        modifier = Modifier.weight(1f)
                    )
                }
            }
        }
    }
}

@Composable
private fun LoadingSubsystemBadge(
    icon: ImageVector,
    label: String,
    accent: Color,
    modifier: Modifier = Modifier
) {
    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(9.dp),
        color = StudioColors.BgCard,
        border = BorderStroke(1.dp, accent.copy(alpha = 0.35f))
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.Center
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = accent,
                modifier = Modifier.size(14.dp)
            )
            Spacer(Modifier.width(5.dp))
            Text(
                text = label,
                color = StudioColors.TextMuted,
                fontSize = 10.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
        }
    }
}
