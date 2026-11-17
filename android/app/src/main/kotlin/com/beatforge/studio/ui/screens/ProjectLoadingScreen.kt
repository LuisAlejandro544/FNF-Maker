// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/ProjectLoadingScreen.kt
// Propósito: Pantalla dedicada de carga que se muestra al seleccionar, crear o importar un
// proyecto desde el menú principal, informando el progreso real en `Dispatchers.IO` antes
// de abrir el estudio de edición.

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
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AudioFile
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material.icons.rounded.Memory
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.gfx.GfxCapabilities
import com.beatforge.studio.gfx.RenderBackendKind
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
        animationSpec = tween(durationMillis = 150),
        label = "projectLoadingProgress"
    )
    val pct = (animatedProgress * 100f).toInt().coerceIn(0, 100)
    val backendKind = RenderBackendKind.fromKey(preferredBackend)

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(StudioColors.BgDark),
        contentAlignment = Alignment.Center
    ) {
        Surface(
            modifier = Modifier
                .fillMaxWidth(0.62f)
                .padding(16.dp),
            shape = RoundedCornerShape(16.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.5f))
        ) {
            Column(
                modifier = Modifier.padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.Center
                ) {
                    Icon(
                        imageVector = Icons.Rounded.FolderOpen,
                        contentDescription = null,
                        tint = StudioColors.AccentCyan,
                        modifier = Modifier.size(28.dp)
                    )
                    Spacer(Modifier.width(10.dp))
                    Column {
                        Text(
                            text = "Cargando Proyecto...",
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp,
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

                Box(contentAlignment = Alignment.Center) {
                    CircularProgressIndicator(
                        progress = { animatedProgress },
                        color = StudioColors.AccentEmerald,
                        trackColor = StudioColors.BgDark,
                        strokeWidth = 4.dp,
                        modifier = Modifier.size(60.dp)
                    )
                    Text(
                        text = "$pct%",
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 13.sp
                    )
                }

                Column(
                    modifier = Modifier.fillMaxWidth(),
                    verticalArrangement = Arrangement.spacedBy(5.dp)
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = phase.ifBlank { "Preparando recursos..." },
                            color = StudioColors.AccentCyan,
                            fontWeight = FontWeight.SemiBold,
                            fontSize = 12.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f)
                        )
                        Spacer(Modifier.width(8.dp))
                        Text(
                            text = "$pct%",
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
                            .height(8.dp)
                            .clip(RoundedCornerShape(4.dp))
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    LoadingSubsystemBadge(
                        icon = Icons.Rounded.AudioFile,
                        label = "Motor Rust AAudio",
                        accent = StudioColors.AccentEmerald,
                        modifier = Modifier.weight(1f)
                    )
                    LoadingSubsystemBadge(
                        icon = Icons.Rounded.Layers,
                        label = "Sparrow & Skins",
                        accent = StudioColors.AccentAmber,
                        modifier = Modifier.weight(1f)
                    )
                    LoadingSubsystemBadge(
                        icon = Icons.Rounded.Memory,
                        label = "${backendKind.shortBadge} · ${gfxCapabilities.glesVersionLabel}",
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
        shape = RoundedCornerShape(8.dp),
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
