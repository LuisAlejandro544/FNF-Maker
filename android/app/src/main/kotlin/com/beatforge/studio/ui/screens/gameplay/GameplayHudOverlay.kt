// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/gameplay/GameplayHudOverlay.kt
// Propósito: Componentes de interfaz superpuesta (HUD) y diálogos de `GameplayScreen`:
// barra de vida SVG con íconos reactivos al BPM, estadísticas (`Puntaje | Fallos | Precisión`),
// botón compacto de Pausa, consola rápida de scripts, cuenta regresiva BPM y diálogos
// modales de Pausa y Resultados Finales.

package com.beatforge.studio.ui.screens.gameplay

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
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
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Home
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Replay
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.GameplayHudState
import com.beatforge.studio.MobileControlMode
import com.beatforge.studio.ui.theme.StudioColors
import java.util.Locale
import kotlin.math.roundToInt

@Composable
internal fun BoxScope.GameplayHealthBarAndStats(
    frameState: GameplayFrameState,
    hud: GameplayHudState,
    healthBarBitmap: ImageBitmap?,
    fallbackPlayerId: String?,
    fallbackOpponentId: String?,
    entityNormalIcons: Map<String, ImageBitmap>,
    entityLosingIcons: Map<String, ImageBitmap>,
    defaultPlayerNormalIcon: ImageBitmap?,
    defaultPlayerLosingIcon: ImageBitmap?,
    defaultOpponentNormalIcon: ImageBitmap?,
    defaultOpponentLosingIcon: ImageBitmap?
) {
    if (!hud.hudVisible) return
    val healthBarAlign = if (hud.downscroll) Alignment.TopCenter else Alignment.BottomCenter
    Column(
        modifier = Modifier
            .align(healthBarAlign)
            .padding(
                top = if (hud.downscroll) 6.dp else 0.dp,
                bottom = if (hud.downscroll) 0.dp else 8.dp
            )
            .scale(hud.hudZoom.coerceIn(0.4f, 2f))
            .alpha(hud.hudOpacity.coerceIn(0f, 1f)),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        // Canvas dedicado 60 FPS para la barra rojo/verde + marco SVG + íconos dinámicos con latido BPM
        Canvas(
            modifier = Modifier
                .width(340.dp)
                .height(54.dp)
        ) {
            if (frameState.drawTick < 0L) return@Canvas
            val barW = size.width * 0.84f
            val barH = 14.dp.toPx()
            val barLeft = (size.width - barW) / 2f
            val barTop = (size.height - barH) / 2f + 2.dp.toPx()

            val clampedHealth = frameState.health.coerceIn(0f, 1f)
            val dividerRatio = 1f - clampedHealth
            val splitX = barLeft + barW * dividerRatio

            if (dividerRatio > 0.002f) {
                drawRect(
                    color = Color(0xFFE11D48),
                    topLeft = Offset(barLeft, barTop),
                    size = Size(barW * dividerRatio, barH)
                )
            }
            if (dividerRatio < 0.998f) {
                drawRect(
                    color = Color(0xFF22C55E),
                    topLeft = Offset(splitX, barTop),
                    size = Size(barW * (1f - dividerRatio), barH)
                )
            }

            if (healthBarBitmap != null) {
                val padX = 7.dp.toPx()
                val padY = 5.dp.toPx()
                drawImage(
                    image = healthBarBitmap,
                    dstOffset = IntOffset((barLeft - padX).roundToInt(), (barTop - padY).roundToInt()),
                    dstSize = IntSize(
                        (barW + padX * 2f).roundToInt().coerceAtLeast(4),
                        (barH + padY * 2f).roundToInt().coerceAtLeast(4)
                    ),
                    filterQuality = FilterQuality.High
                )
            } else {
                drawRect(
                    color = Color.Black,
                    topLeft = Offset(barLeft, barTop),
                    size = Size(barW, barH),
                    style = Stroke(width = 3.dp.toPx())
                )
            }

            val activePlayerId = frameState.entityIdsByIndex.getOrNull(frameState.activePlayerIdx)
                ?: fallbackPlayerId
            val activeOpponentId = frameState.entityIdsByIndex.getOrNull(frameState.activeOpponentIdx)
                ?: fallbackOpponentId

            val plrNormal = activePlayerId?.let { entityNormalIcons[it] } ?: defaultPlayerNormalIcon
            val plrLosing = activePlayerId?.let { entityLosingIcons[it] } ?: defaultPlayerLosingIcon
            val oppNormal = activeOpponentId?.let { entityNormalIcons[it] } ?: defaultOpponentNormalIcon
            val oppLosing = activeOpponentId?.let { entityLosingIcons[it] } ?: defaultOpponentLosingIcon

            val beatImpact = (1f - frameState.beatPhase).coerceIn(0f, 1f)
            val iconSize = 42.dp.toPx() * (1f + 0.14f * beatImpact * beatImpact)
            val iconCy = barTop + barH * 0.5f

            val oppBmp = if (clampedHealth > 0.80f) {
                oppLosing ?: oppNormal
            } else {
                oppNormal
            }
            if (oppBmp != null) {
                val oppCx = (splitX - iconSize * 0.48f).coerceIn(iconSize * 0.45f, size.width - iconSize * 0.9f)
                drawImage(
                    image = oppBmp,
                    dstOffset = IntOffset((oppCx - iconSize / 2f).roundToInt(), (iconCy - iconSize / 2f).roundToInt()),
                    dstSize = IntSize(iconSize.roundToInt().coerceAtLeast(2), iconSize.roundToInt().coerceAtLeast(2)),
                    filterQuality = FilterQuality.High
                )
            }

            val plrBmp = if (clampedHealth < 0.20f) {
                plrLosing ?: plrNormal
            } else {
                plrNormal
            }
            if (plrBmp != null) {
                val plrCx = (splitX + iconSize * 0.48f).coerceIn(iconSize * 0.9f, size.width - iconSize * 0.45f)
                withTransform({
                    scale(scaleX = -1f, scaleY = 1f, pivot = Offset(plrCx, iconCy))
                }) {
                    drawImage(
                        image = plrBmp,
                        dstOffset = IntOffset((plrCx - iconSize / 2f).roundToInt(), (iconCy - iconSize / 2f).roundToInt()),
                        dstSize = IntSize(iconSize.roundToInt().coerceAtLeast(2), iconSize.roundToInt().coerceAtLeast(2)),
                        filterQuality = FilterQuality.High
                    )
                }
            }
        }

        val accText = String.format(Locale.US, "%.1f", hud.accuracyPct.coerceIn(0f, 100f))
        Text(
            text = "Puntaje: ${hud.score}   |   Fallos: ${hud.misses}   |   Precisión: $accText%",
            color = Color.White,
            fontSize = 12.sp,
            fontWeight = FontWeight.Bold,
            style = TextStyle(
                shadow = Shadow(
                    color = Color.Black,
                    offset = Offset(1.5f, 1.5f),
                    blurRadius = 3f
                )
            )
        )
    }
}

@Composable
internal fun BoxScope.GameplayPauseButton(onPause: () -> Unit) {
    Surface(
        modifier = Modifier
            .align(Alignment.TopEnd)
            .padding(top = 8.dp, end = 12.dp)
            .size(36.dp),
        shape = CircleShape,
        color = StudioColors.BgPanel.copy(alpha = 0.72f),
        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
    ) {
        IconButton(
            onClick = onPause,
            modifier = Modifier.fillMaxSize()
        ) {
            Icon(Icons.Rounded.Pause, "Pausa", tint = StudioColors.TextPrimary, modifier = Modifier.size(19.dp))
        }
    }
}

@Composable
internal fun BoxScope.GameplayScriptBanner(hud: GameplayHudState) {
    if (hud.scriptError.isBlank() && hud.scriptLog.isBlank()) return
    val isErr = hud.scriptError.isNotBlank()
    val text = if (isErr) hud.scriptError else hud.scriptLog
    Surface(
        modifier = Modifier
            .align(Alignment.BottomStart)
            .padding(start = 12.dp, bottom = 10.dp),
        shape = RoundedCornerShape(6.dp),
        color = if (isErr) StudioColors.AccentCoral.copy(alpha = 0.88f) else StudioColors.BgCard.copy(alpha = 0.85f)
    ) {
        Text(
            text = text,
            color = Color.White,
            fontSize = 10.sp,
            modifier = Modifier.padding(horizontal = 8.dp, vertical = 4.dp)
        )
    }
}

@Composable
internal fun BoxScope.GameplayCountdownOverlay(hud: GameplayHudState) {
    if (hud.countdownStage !in 0..3 || hud.paused || hud.finished) return
    val p = hud.countdownProgress.coerceIn(0f, 1f)
    val popScale = if (p < 0.25f) {
        1.42f - (p / 0.25f) * 0.42f
    } else {
        1.0f - (p - 0.25f) * 0.12f
    }
    val fadeAlpha = if (p > 0.72f) {
        (1f - (p - 0.72f) / 0.28f).coerceIn(0f, 1f)
    } else {
        1f
    }
    val stageColor = when (hud.countdownStage) {
        0 -> StudioColors.AccentCoral
        1 -> StudioColors.AccentAmber
        2 -> StudioColors.AccentCyan
        else -> StudioColors.AccentEmerald
    }

    Box(
        modifier = Modifier
            .align(Alignment.Center)
            .scale(popScale)
            .alpha(fadeAlpha),
        contentAlignment = Alignment.Center
    ) {
        Canvas(modifier = Modifier.size(160.dp)) {
            val pulseRadius = (size.minDimension * 0.32f) + (size.minDimension * 0.18f * p)
            drawCircle(
                color = stageColor.copy(alpha = (1f - p) * 0.55f),
                radius = pulseRadius,
                style = Stroke(width = 6f)
            )
        }
        Surface(
            shape = RoundedCornerShape(22.dp),
            color = StudioColors.BgPanel.copy(alpha = 0.92f),
            border = BorderStroke(2.5.dp, stageColor)
        ) {
            Column(
                modifier = Modifier.padding(horizontal = 30.dp, vertical = 14.dp),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Text(
                    text = hud.countdownLabel,
                    color = stageColor,
                    fontSize = if (hud.countdownStage == 3) 44.sp else 54.sp,
                    fontWeight = FontWeight.ExtraBold
                )
                Text(
                    text = "${hud.activeBpm.roundToInt()} BPM",
                    color = StudioColors.TextMuted,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.SemiBold
                )
            }
        }
    }
}

@Composable
internal fun GameplayPauseDialog(
    hud: GameplayHudState,
    onChangeControlMode: (MobileControlMode) -> Unit,
    onResume: () -> Unit,
    onRestart: () -> Unit,
    onExitToStudio: () -> Unit
) {
    if (!hud.paused || hud.finished) return
    Dialog(onDismissRequest = onResume) {
        Surface(
            shape = RoundedCornerShape(14.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Column(
                modifier = Modifier.padding(20.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                Text("Partida en Pausa", color = StudioColors.TextPrimary, fontWeight = FontWeight.Bold, fontSize = 18.sp)

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    MobileControlMode.entries.forEach { mode ->
                        val selected = hud.controlMode == mode.key
                        Surface(
                            modifier = Modifier
                                .weight(1f)
                                .clickable { onChangeControlMode(mode) },
                            shape = RoundedCornerShape(8.dp),
                            color = if (selected) StudioColors.AccentCyan.copy(alpha = 0.16f) else StudioColors.BgCard,
                            border = BorderStroke(
                                1.dp,
                                if (selected) StudioColors.AccentCyan else StudioColors.BorderSubtle
                            )
                        ) {
                            Text(
                                text = if (mode == MobileControlMode.TapArrows) "Tocar Flechas" else "Hitbox Clásico",
                                color = if (selected) StudioColors.AccentCyan else StudioColors.TextMuted,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.Bold,
                                modifier = Modifier.padding(horizontal = 8.dp, vertical = 7.dp)
                            )
                        }
                    }
                }

                Button(
                    onClick = onResume,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Icon(Icons.Rounded.PlayArrow, null)
                    Spacer(Modifier.width(6.dp))
                    Text("Reanudar")
                }
                OutlinedButton(
                    onClick = onRestart,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Icon(Icons.Rounded.Replay, null)
                    Spacer(Modifier.width(6.dp))
                    Text("Reiniciar Canción")
                }
                OutlinedButton(
                    onClick = onExitToStudio,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Icon(Icons.Rounded.Home, null)
                    Spacer(Modifier.width(6.dp))
                    Text("Volver al Estudio")
                }
            }
        }
    }
}

@Composable
internal fun GameplayResultsDialog(
    hud: GameplayHudState,
    onRestart: () -> Unit,
    onExitToStudio: () -> Unit
) {
    if (!hud.finished) return
    Dialog(onDismissRequest = onExitToStudio) {
        Surface(
            shape = RoundedCornerShape(14.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.AccentEmerald)
        ) {
            Column(
                modifier = Modifier.padding(20.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                val accText = String.format(Locale.US, "%.1f", hud.accuracyPct.coerceIn(0f, 100f))
                Text("¡Canción Completada!", color = StudioColors.AccentEmerald, fontWeight = FontWeight.Bold, fontSize = 18.sp)
                Text("Puntaje Final: ${hud.score}", color = StudioColors.TextPrimary, fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
                Text(
                    "Combo Máximo: ${hud.maxCombo}  |  Fallos: ${hud.misses}  |  Precisión: $accText%",
                    color = StudioColors.TextMuted,
                    fontSize = 12.sp
                )
                Spacer(Modifier.height(6.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                    OutlinedButton(onClick = onRestart, contentPadding = PaddingValues(horizontal = 14.dp)) {
                        Icon(Icons.Rounded.Replay, null, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(5.dp))
                        Text("Reintentar")
                    }
                    Button(
                        onClick = onExitToStudio,
                        colors = ButtonDefaults.buttonColors(
                            containerColor = StudioColors.AccentCyan,
                            contentColor = Color(0xFF041C24)
                        )
                    ) {
                        Icon(Icons.Rounded.Home, null, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(5.dp))
                        Text("Volver al Estudio", fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}
