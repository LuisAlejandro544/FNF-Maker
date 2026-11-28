// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/gameplay/GameplayTouchOverlay.kt
// Propósito: Capa de entrada multi-touch de 4 carriles para `GameplayScreen`:
// soporta Hitbox Clásico (4 columnas de pantalla completa con etiquetas) y Tocar Flechas
// (detección inteligente sobre los receptores activos, respetando `strumOverrides`).

package com.beatforge.studio.ui.screens.gameplay

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.MobileControlMode
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.abs

private val TOUCH_LANE_NAMES = listOf("IZQ", "ABA", "ARR", "DER")

@Composable
internal fun GameplayTouchOverlay(
    frameState: GameplayFrameState,
    isClassicHitbox: Boolean,
    touchMaskState: GameplayTouchMaskState
) {
    val hud = frameState.hudState
    if (hud.paused || hud.finished || hud.gameOver) {
        touchMaskState.pressedMask = 0
        touchMaskState.pendingJustPressedMask = 0
        return
    }
    Box(
        modifier = Modifier
            .fillMaxSize()
            .pointerInput(Unit) {
                awaitPointerEventScope {
                    while (true) {
                        val event = awaitPointerEvent()
                        var nextMask = 0
                        val w = size.width.toFloat().coerceAtLeast(1f)
                        val h = size.height.toFloat().coerceAtLeast(1f)
                        val classicMode = frameState.controlMode == MobileControlMode.ClassicHitbox.key

                        if (classicMode) {
                            event.changes.forEach { change ->
                                if (change.pressed) {
                                    val lane = ((change.position.x / w) * 4f)
                                        .toInt()
                                        .coerceIn(0, 3)
                                    nextMask = nextMask or (1 shl lane)
                                }
                            }
                        } else {
                            val vw = frameState.virtualWidth.coerceAtLeast(1f)
                            val vh = frameState.virtualHeight.coerceAtLeast(1f)
                            val sx = w / vw
                            val sy = h / vh
                            val useDownscrollTactile = true
                            val baseReceptorY = 595f * sy
                            val notePxSize = 98f * minOf(sx, sy)
                            val maxHitX = 82f * sx
                            val maxHitY = maxOf(notePxSize * 1.30f, 96f)

                            event.changes.forEach { change ->
                                if (change.pressed) {
                                    val px = change.position.x
                                    val py = change.position.y
                                    var bestLane = -1
                                    var bestDistX = Float.MAX_VALUE
                                    for (dir in 0..3) {
                                        val ov = frameState.strumOverrides[4 + dir]
                                        val baseCx = resolvePlayerLaneBaseX(dir, useDownscrollTactile, sx)
                                        val cx = baseCx + ov.offsetX * sx
                                        val cy = baseReceptorY + ov.offsetY * sy
                                        val dx = abs(px - cx)
                                        val dy = abs(py - cy)
                                        if (dy <= maxHitY && dx <= maxHitX && dx < bestDistX) {
                                            bestDistX = dx
                                            bestLane = dir
                                        }
                                    }
                                    if (bestLane >= 0) {
                                        nextMask = nextMask or (1 shl bestLane)
                                    }
                                }
                            }
                        }

                        val newlyPressed = nextMask and touchMaskState.pressedMask.inv()
                        if (newlyPressed != 0) {
                            touchMaskState.pendingJustPressedMask =
                                touchMaskState.pendingJustPressedMask or newlyPressed
                        }
                        touchMaskState.pressedMask = nextMask
                    }
                }
            }
    ) {
        val arrowsAlpha = if (hud.arrowsVisible) hud.arrowsOpacity.coerceIn(0f, 1f) else 0f
        if (isClassicHitbox && arrowsAlpha > 0.01f) {
            Row(modifier = Modifier.fillMaxSize()) {
                for (lane in 0..3) {
                    val col = StudioColors.LaneColors[lane]
                    Box(
                        modifier = Modifier
                            .weight(1f)
                            .fillMaxHeight()
                            .padding(bottom = 14.dp),
                        contentAlignment = Alignment.BottomCenter
                    ) {
                        Text(
                            text = TOUCH_LANE_NAMES[lane],
                            color = col.copy(alpha = 0.82f * arrowsAlpha),
                            fontWeight = FontWeight.Bold,
                            fontSize = 11.sp
                        )
                    }
                }
            }
        }
    }
}
