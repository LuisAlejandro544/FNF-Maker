// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/GameplayScreen.kt
// Propósito: Pantalla jugable en tiempo real en Compose Canvas a 60 FPS sincronizada
// con el reloj del mezclador Rust AAudio (`nativeStepGameplayBinary`).
// Implementa:
// 1. Barra de vida estilo FNF con marco vectorial SVG (`resvg`), relleno dinámico rojo/verde,
//    íconos deslizantes con rebote al ritmo (normal y perdiendo <20%) y estadísticas debajo.
// 2. Popups vectoriales SVG (`SICK!`, `GOOD!`, `BAD`, `MISS`) junto al jugador y destello
//    de chispas (`Note Splash`) sobre las flechas al acertar notas perfectas.
// 3. Transformaciones modchart completas (`strumOverrides`), rotación de entidades y
//    objetos/textos dinámicos creados desde Lua, TypeScript o JavaScript.

package com.beatforge.studio.ui.screens

import android.graphics.Paint
import android.graphics.Typeface
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Shadow
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.MobileControlMode
import com.beatforge.studio.MutableDynamicDrawable
import com.beatforge.studio.ui.theme.StudioColors
import java.util.Locale
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sin

private val TOUCH_LANE_NAMES = listOf("IZQ", "ABA", "ARR", "DER")

private class RatingPopupHolder {
    var lastEventId: Int = 0
    var active: Boolean = false
    var ratingCode: Int = 0
    var startNano: Long = 0L
    var worldX: Float = 860f
    var worldY: Float = 360f
}

private class SplashSlot {
    var active: Boolean = false
    var lane: Int = 0
    var startNano: Long = 0L
    var angleOffset: Float = 0f
}

private class SplashPoolHolder {
    var lastEventId: Int = 0
    var cursor: Int = 0
    val slots = Array(8) { SplashSlot() }
}

@Composable
fun GameplayScreen(
    frameState: GameplayFrameState,
    arrowBitmaps: Map<Int, ImageBitmap>,
    receptorBitmaps: Map<Int, ImageBitmap>,
    healthBarBitmap: ImageBitmap?,
    ratingBitmaps: Map<Int, ImageBitmap>,
    noteSplashBitmap: ImageBitmap?,
    playerNormalIcon: ImageBitmap?,
    playerLosingIcon: ImageBitmap?,
    opponentNormalIcon: ImageBitmap?,
    opponentLosingIcon: ImageBitmap?,
    compiledBitmapsByIndex: List<LoadedEntityBitmap?>,
    onStepFrame: suspend (dtSec: Float, pressedMask: Int, justPressedMask: Int) -> Unit,
    onChangeControlMode: (MobileControlMode) -> Unit,
    onSetPaused: (Boolean) -> Unit,
    onRestart: () -> Unit,
    onExitToStudio: () -> Unit
) {
    var pressedMask by remember { mutableIntStateOf(0) }
    var pendingJustPressedMask by remember { mutableIntStateOf(0) }
    val hud = frameState.hudState

    val ratingPopup = remember { RatingPopupHolder() }
    val splashPool = remember { SplashPoolHolder() }
    val dynamicTextPaint = remember {
        Paint(Paint.ANTI_ALIAS_FLAG).apply {
            typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
            setShadowLayer(4f, 2f, 2f, android.graphics.Color.BLACK)
        }
    }

    LaunchedEffect(Unit) {
        var lastNs = 0L
        while (true) {
            var dt = 0.016f
            withFrameNanos { ns ->
                if (lastNs != 0L) {
                    dt = ((ns - lastNs) / 1_000_000_000f).coerceIn(0.001f, 0.05f)
                }
                lastNs = ns
            }
            if (!frameState.paused && !frameState.finished) {
                val justMask = pendingJustPressedMask
                pendingJustPressedMask = 0
                onStepFrame(dt, pressedMask, justMask)
            }
        }
    }

    val isClassicHitbox = hud.controlMode == MobileControlMode.ClassicHitbox.key

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(StudioColors.BgDark)
    ) {
        // Lienzo principal de gameplay: lee `frameState.drawTick` solo dentro de DrawScope (0 recomposiciones a 60 FPS)
        Canvas(modifier = Modifier.fillMaxSize()) {
            if (frameState.drawTick < 0L) return@Canvas
            val nowNs = System.nanoTime()

            // Capturar eventos nuevos de calificación (`SICK!`/`GOOD!`/`BAD`/`MISS`) y Note Splash
            if (frameState.ratingEventId != ratingPopup.lastEventId) {
                ratingPopup.lastEventId = frameState.ratingEventId
                if (frameState.lastRatingCode in 1..4) {
                    var plrX = 880f
                    var plrY = 430f
                    var plrW = 240f
                    var plrH = 300f
                    for (i in 0 until frameState.entityCount) {
                        val ent = frameState.entitiesPool[i]
                        if (ent.roleCode == 2) {
                            plrX = ent.x
                            plrY = ent.y
                            plrW = ent.width * ent.scale
                            plrH = ent.height * ent.scale
                            break
                        }
                    }
                    ratingPopup.active = true
                    ratingPopup.ratingCode = frameState.lastRatingCode
                    ratingPopup.startNano = nowNs
                    ratingPopup.worldX = plrX - plrW * 0.56f
                    ratingPopup.worldY = plrY - plrH * 0.28f
                }
            }

            if (frameState.splashEventId != splashPool.lastEventId) {
                splashPool.lastEventId = frameState.splashEventId
                val slot = splashPool.slots[splashPool.cursor and 7]
                splashPool.cursor += 1
                slot.active = true
                slot.lane = frameState.splashLane.coerceIn(0, 3)
                slot.startNano = nowNs
                slot.angleOffset = ((splashPool.cursor * 37) % 60 - 30).toFloat()
            }

            val vw = frameState.virtualWidth.coerceAtLeast(1f)
            val vh = frameState.virtualHeight.coerceAtLeast(1f)
            val sx = size.width / vw
            val sy = size.height / vh

            val shakePhase = (frameState.songPosMs * 0.08).toFloat()
            val shakeX = sin(shakePhase) * frameState.cameraShake * sx
            val shakeY = cos(shakePhase * 1.3f) * frameState.cameraShake * sy
            val pivot = Offset(size.width * 0.5f, size.height * 0.5f)
            val camTransX = pivot.x - frameState.cameraX * sx
            val camTransY = pivot.y - frameState.cameraY * sy

            withTransform({
                translate(shakeX, shakeY)
                scale(frameState.cameraZoom, frameState.cameraZoom, pivot = pivot)
                translate(camTransX, camTransY)
            }) {
                // 1. Dibujar entidades compiladas en Z-order usando índice pre-resuelto O(1)
                val entCount = frameState.entityCount
                for (i in 0 until entCount) {
                    val ent = frameState.entitiesPool[i]
                    if (!ent.visible || ent.opacity <= 0.01f) continue
                    val cx = ent.x * sx
                    val cy = ent.y * sy
                    val w = ent.width * ent.scale * sx
                    val h = ent.height * ent.scale * sy
                    val left = cx - w / 2f
                    val top = cy - h / 2f
                    val loaded = compiledBitmapsByIndex.getOrNull(i)

                    withTransform({
                        if (ent.rotationDeg != 0f) {
                            rotate(ent.rotationDeg, pivot = Offset(cx, cy))
                        }
                        if (ent.flipX) {
                            scale(scaleX = -1f, scaleY = 1f, pivot = Offset(cx, cy))
                        }
                    }) {
                        if (loaded != null) {
                            val bmp = loaded.bitmap
                            val scaleRatioX = bmp.width.toFloat() / loaded.origWidth.toFloat()
                            val scaleRatioY = bmp.height.toFloat() / loaded.origHeight.toFloat()

                            if (ent.hasFrame && ent.atlasW > 0 && ent.atlasH > 0) {
                                val srcX = (ent.atlasX * scaleRatioX).roundToInt().coerceIn(0, bmp.width - 1)
                                val srcY = (ent.atlasY * scaleRatioY).roundToInt().coerceIn(0, bmp.height - 1)
                                val srcW = (ent.atlasW * scaleRatioX).roundToInt().coerceIn(1, bmp.width - srcX)
                                val srcH = (ent.atlasH * scaleRatioY).roundToInt().coerceIn(1, bmp.height - srcY)

                                val visW = if (ent.frameRotated) ent.atlasH else ent.atlasW
                                val visH = if (ent.frameRotated) ent.atlasW else ent.atlasH
                                val logicalW = ent.logicalW.coerceAtLeast(visW).toFloat().coerceAtLeast(1f)
                                val logicalH = ent.logicalH.coerceAtLeast(visH).toFloat().coerceAtLeast(1f)
                                val fit = min(w / logicalW, h / logicalH)

                                val boxW = logicalW * fit
                                val boxH = logicalH * fit
                                val boxLeft = cx - boxW / 2f
                                val boxTop = (top + h) - boxH

                                val visDrawW = visW * fit
                                val visDrawH = visH * fit
                                val visLeft = boxLeft + (-ent.offX * fit)
                                val visTop = boxTop + (-ent.offY * fit)
                                val frameCenterX = visLeft + visDrawW / 2f
                                val frameCenterY = visTop + visDrawH / 2f

                                val dstW = (ent.atlasW * fit).roundToInt().coerceAtLeast(2)
                                val dstH = (ent.atlasH * fit).roundToInt().coerceAtLeast(2)
                                val dstLeft = (frameCenterX - dstW / 2f).roundToInt()
                                val dstTop = (frameCenterY - dstH / 2f).roundToInt()

                                withTransform({
                                    if (ent.frameRotated) {
                                        rotate(-90f, pivot = Offset(frameCenterX, frameCenterY))
                                    }
                                }) {
                                    drawImage(
                                        image = bmp,
                                        srcOffset = IntOffset(srcX, srcY),
                                        srcSize = IntSize(srcW, srcH),
                                        dstOffset = IntOffset(dstLeft, dstTop),
                                        dstSize = IntSize(dstW, dstH),
                                        alpha = ent.opacity.coerceIn(0f, 1f),
                                        filterQuality = FilterQuality.Medium
                                    )
                                }
                            } else {
                                drawImage(
                                    image = bmp,
                                    srcOffset = IntOffset(0, 0),
                                    srcSize = IntSize(bmp.width, bmp.height),
                                    dstOffset = IntOffset(left.roundToInt(), top.roundToInt()),
                                    dstSize = IntSize(w.roundToInt().coerceAtLeast(2), h.roundToInt().coerceAtLeast(2)),
                                    alpha = ent.opacity.coerceIn(0f, 1f),
                                    filterQuality = FilterQuality.Medium
                                )
                            }
                        } else {
                            val boxColor = when (ent.roleCode) {
                                0 -> Color(0xFF172033)
                                1 -> StudioColors.AccentCoral.copy(alpha = 0.28f)
                                2 -> StudioColors.AccentCyan.copy(alpha = 0.28f)
                                else -> StudioColors.AccentIndigo.copy(alpha = 0.25f)
                            }
                            drawRect(
                                color = boxColor,
                                topLeft = Offset(left, top),
                                size = Size(w, h)
                            )
                            drawRect(
                                color = StudioColors.BorderSubtle.copy(alpha = ent.opacity),
                                topLeft = Offset(left, top),
                                size = Size(w, h),
                                style = Stroke(width = 2f)
                            )
                        }
                    }
                }

                // 1.5 Gráficos y textos dinámicos de mundo creados por scripts (`makeGraphic`)
                for (i in 0 until frameState.dynamicCount) {
                    val item = frameState.dynamicPool[i]
                    if (!item.isHud && item.visible && item.opacity > 0.01f) {
                        drawDynamicItem(item, sx, sy, dynamicTextPaint)
                    }
                }

                // 1.6 Popup SVG de calificación (`SICK!`, `GOOD!`, `BAD`, `MISS`) junto al jugador
                if (ratingPopup.active) {
                    val ageSec = ((nowNs - ratingPopup.startNano) / 1_000_000_000.0).toFloat()
                    if (ageSec >= 0.68f) {
                        ratingPopup.active = false
                    } else {
                        val ratingBmp = ratingBitmaps[ratingPopup.ratingCode]
                        if (ratingBmp != null) {
                            val popScale = if (ageSec < 0.08f) {
                                1.28f - (ageSec / 0.08f) * 0.28f
                            } else {
                                1f
                            }
                            val alpha = if (ageSec > 0.38f) {
                                (1f - (ageSec - 0.38f) / 0.30f).coerceIn(0f, 1f)
                            } else {
                                1f
                            }
                            val curX = (ratingPopup.worldX - 18f * ageSec) * sx
                            val curY = (ratingPopup.worldY - 125f * ageSec + 280f * ageSec * ageSec) * sy
                            val drawW = 190f * minOf(sx, sy) * popScale
                            val drawH = 72f * minOf(sx, sy) * popScale
                            drawImage(
                                image = ratingBmp,
                                dstOffset = IntOffset(
                                    (curX - drawW / 2f).roundToInt(),
                                    (curY - drawH / 2f).roundToInt()
                                ),
                                dstSize = IntSize(drawW.roundToInt().coerceAtLeast(2), drawH.roundToInt().coerceAtLeast(2)),
                                alpha = alpha,
                                filterQuality = FilterQuality.High
                            )
                        }
                    }
                }
            }

            // Columnas translúcidas Hitbox Clásico dibujadas en Canvas para evitar recomposiciones al tocar
            if (frameState.controlMode == MobileControlMode.ClassicHitbox.key) {
                val colW = size.width / 4f
                for (lane in 0..3) {
                    val active = (pressedMask and (1 shl lane)) != 0 ||
                        (frameState.holdingLanesMask and (1 shl lane)) != 0
                    val col = StudioColors.LaneColors[lane]
                    drawRect(
                        color = col.copy(alpha = if (active) 0.22f else 0.03f),
                        topLeft = Offset(lane * colW, 0f),
                        size = Size(colW, size.height)
                    )
                    if (lane < 3) {
                        val divX = (lane + 1) * colW
                        drawLine(
                            color = Color.White.copy(alpha = 0.14f),
                            start = Offset(divX, 0f),
                            end = Offset(divX, size.height),
                            strokeWidth = 1.dp.toPx()
                        )
                    }
                }
            }

            // 2. Carriles y receptores (aplica strumOverrides modchart 0..7)
            val classicMode = frameState.controlMode == MobileControlMode.ClassicHitbox.key
            val useDownscrollTactileLayout = frameState.downscroll && !classicMode

            val baseOppY = if (useDownscrollTactileLayout) 100f * sy else (if (frameState.downscroll) 575f else 118f) * sy
            val basePlrY = if (useDownscrollTactileLayout) 595f * sy else (if (frameState.downscroll) 575f else 118f) * sy
            val oppDownscroll = if (useDownscrollTactileLayout) false else frameState.downscroll
            val plrDownscroll = frameState.downscroll

            val oppNoteSize = if (useDownscrollTactileLayout) 58f * minOf(sx, sy) else 84f * minOf(sx, sy)
            val plrNoteSize = if (useDownscrollTactileLayout) 98f * minOf(sx, sy) else 84f * minOf(sx, sy)

            for (dir in 0..3) {
                val oppOverride = frameState.strumOverrides[dir]
                val plrOverride = frameState.strumOverrides[4 + dir]

                val baseOppX = if (useDownscrollTactileLayout) {
                    (72f + dir * 68f) * sx
                } else {
                    (105f + dir * 105f) * sx
                }
                val basePlrX = if (useDownscrollTactileLayout) {
                    val vx = when (dir) {
                        0 -> 340f
                        1 -> 495f
                        2 -> 785f
                        else -> 940f
                    }
                    vx * sx
                } else {
                    (765f + dir * 105f) * sx
                }

                val oppX = baseOppX + oppOverride.offsetX * sx
                val oppY = baseOppY + oppOverride.offsetY * sy
                val plrX = basePlrX + plrOverride.offsetX * sx
                val plrY = basePlrY + plrOverride.offsetY * sy

                val coloredBmp = arrowBitmaps[dir]
                val neutralBmp = receptorBitmaps[dir] ?: coloredBmp
                val laneColor = StudioColors.LaneColors[dir]

                val oppHolding = (frameState.opponentHoldingLanesMask and (1 shl dir)) != 0
                val oppAlpha = ((if (oppHolding) 0.98f else 0.72f) * oppOverride.alpha).coerceIn(0f, 1f)
                if (oppAlpha > 0.01f) {
                    drawReceptor(
                        center = Offset(oppX, oppY),
                        sizePx = oppNoteSize * (if (oppHolding) 1.06f else 0.94f) * oppOverride.scale,
                        angleDeg = oppOverride.angle,
                        color = if (oppHolding) laneColor else Color(0xFF94A3B8),
                        bitmap = if (oppHolding) coloredBmp else neutralBmp,
                        alpha = oppAlpha
                    )
                }

                val isPressed = (pressedMask and (1 shl dir)) != 0
                val isHolding = (frameState.holdingLanesMask and (1 shl dir)) != 0
                val showColoredPlayer = isPressed || isHolding
                val playerScale = when {
                    isHolding -> 1.08f
                    isPressed -> 0.92f
                    else -> 1f
                } * plrOverride.scale
                val plrAlpha = (when {
                    isHolding -> 1f
                    isPressed -> 0.96f
                    else -> 0.86f
                } * plrOverride.alpha).coerceIn(0f, 1f)

                if (plrAlpha > 0.01f) {
                    drawReceptor(
                        center = Offset(plrX, plrY),
                        sizePx = plrNoteSize * playerScale,
                        angleDeg = plrOverride.angle,
                        color = if (showColoredPlayer) laneColor else Color(0xFF94A3B8),
                        bitmap = if (showColoredPlayer) coloredBmp else neutralBmp,
                        alpha = plrAlpha
                    )
                }
            }

            // 3. Notas visibles y colas sustain desplazándose hasta consumirse en el receptor
            val pxPerMs = 0.44f * frameState.scrollSpeed.toFloat() * sy
            val noteCount = frameState.visibleNoteCount
            val curSongPosMs = frameState.songPosMs
            for (i in 0 until noteCount) {
                val note = frameState.notesPool[i]
                val dir = note.direction.coerceIn(0, 3)
                val strumOv = frameState.strumOverrides[if (note.isPlayer) 4 + dir else dir]
                if (strumOv.alpha <= 0.01f) continue

                val baseNoteX = if (note.isPlayer) {
                    if (useDownscrollTactileLayout) {
                        val vx = when (dir) {
                            0 -> 340f
                            1 -> 495f
                            2 -> 785f
                            else -> 940f
                        }
                        vx * sx
                    } else {
                        (765f + dir * 105f) * sx
                    }
                } else {
                    if (useDownscrollTactileLayout) {
                        (72f + dir * 68f) * sx
                    } else {
                        (105f + dir * 105f) * sx
                    }
                }
                val noteX = baseNoteX + strumOv.offsetX * sx
                val receptorY = (if (note.isPlayer) basePlrY else baseOppY) + strumOv.offsetY * sy
                val laneDownscroll = if (note.isPlayer) plrDownscroll else oppDownscroll
                val notePxSize = (if (note.isPlayer) plrNoteSize else oppNoteSize) * strumOv.scale

                val headDeltaMs = (note.timeMs - curSongPosMs).toFloat()
                val tailEndDeltaMs = (note.timeMs + note.sustainMs - curSongPosMs).toFloat()
                val headY = if (laneDownscroll) {
                    receptorY - headDeltaMs * pxPerMs
                } else {
                    receptorY + headDeltaMs * pxPerMs
                }
                val tailEndY = if (laneDownscroll) {
                    receptorY - tailEndDeltaMs * pxPerMs
                } else {
                    receptorY + tailEndDeltaMs * pxPerMs
                }

                val anchorY = if (note.headHit) receptorY else headY
                val minY = minOf(anchorY, tailEndY)
                val maxY = maxOf(anchorY, tailEndY)
                if (maxY < -notePxSize || minY > size.height + notePxSize) continue

                val noteAlpha = strumOv.alpha.coerceIn(0f, 1f)
                if (note.sustainMs > 45.0) {
                    val tailTop = if (laneDownscroll) tailEndY else anchorY
                    val tailBottom = if (laneDownscroll) anchorY else tailEndY
                    val tailHeight = tailBottom - tailTop
                    if (tailHeight > 2f) {
                        val laneColor = StudioColors.LaneColors[dir]
                        val bodyWidth = notePxSize * 0.30f
                        val coreWidth = notePxSize * 0.11f
                        drawRoundRect(
                            color = laneColor.copy(alpha = (if (note.headHit) 0.84f else 0.64f) * noteAlpha),
                            topLeft = Offset(noteX - bodyWidth / 2f, tailTop),
                            size = Size(bodyWidth, tailHeight),
                            cornerRadius = CornerRadius(bodyWidth * 0.45f, bodyWidth * 0.45f)
                        )
                        drawRoundRect(
                            color = Color.White.copy(alpha = (if (note.headHit) 0.55f else 0.32f) * noteAlpha),
                            topLeft = Offset(noteX - coreWidth / 2f, tailTop),
                            size = Size(coreWidth, tailHeight),
                            cornerRadius = CornerRadius(coreWidth * 0.5f, coreWidth * 0.5f)
                        )
                    }
                }

                if (!note.headHit) {
                    val bmp = arrowBitmaps[dir]
                    withTransform({
                        if (strumOv.angle != 0f) {
                            rotate(strumOv.angle, pivot = Offset(noteX, headY))
                        }
                    }) {
                        if (bmp != null) {
                            val half = notePxSize / 2f
                            drawImage(
                                image = bmp,
                                dstOffset = IntOffset((noteX - half).roundToInt(), (headY - half).roundToInt()),
                                dstSize = IntSize(notePxSize.roundToInt().coerceAtLeast(2), notePxSize.roundToInt().coerceAtLeast(2)),
                                alpha = noteAlpha
                            )
                        } else {
                            drawCircle(
                                color = StudioColors.LaneColors[dir].copy(alpha = noteAlpha),
                                radius = notePxSize * 0.4f,
                                center = Offset(noteX, headY)
                            )
                        }
                    }
                }
            }

            // 4. Destellos de chispas (`Note Splash`) sobre las flechas al acertar notas perfectas (`Sick`)
            for (sIdx in splashPool.slots.indices) {
                val slot = splashPool.slots[sIdx]
                if (!slot.active) continue
                val ageSec = ((nowNs - slot.startNano) / 1_000_000_000.0).toFloat()
                if (ageSec >= 0.28f) {
                    slot.active = false
                    continue
                }
                val lane = slot.lane.coerceIn(0, 3)
                val plrOv = frameState.strumOverrides[4 + lane]
                val baseSplashX = if (useDownscrollTactileLayout) {
                    val vx = when (lane) {
                        0 -> 340f
                        1 -> 495f
                        2 -> 785f
                        else -> 940f
                    }
                    vx * sx
                } else {
                    (765f + lane * 105f) * sx
                }
                val splashX = baseSplashX + plrOv.offsetX * sx
                val splashY = basePlrY + plrOv.offsetY * sy
                val progress = (ageSec / 0.28f).coerceIn(0f, 1f)
                val splashAlpha = (1f - progress * progress).coerceIn(0f, 1f)
                val splashSize = plrNoteSize * (1.25f + progress * 0.85f) * plrOv.scale
                val laneColor = StudioColors.LaneColors[lane]

                if (noteSplashBitmap != null) {
                    val halfSplash = splashSize / 2f
                    withTransform({
                        rotate(slot.angleOffset + progress * 35f, pivot = Offset(splashX, splashY))
                    }) {
                        drawImage(
                            image = noteSplashBitmap,
                            dstOffset = IntOffset((splashX - halfSplash).roundToInt(), (splashY - halfSplash).roundToInt()),
                            dstSize = IntSize(splashSize.roundToInt().coerceAtLeast(4), splashSize.roundToInt().coerceAtLeast(4)),
                            alpha = splashAlpha * 0.94f,
                            colorFilter = ColorFilter.tint(laneColor, BlendMode.Modulate),
                            filterQuality = FilterQuality.High
                        )
                    }
                } else {
                    drawCircle(
                        color = laneColor.copy(alpha = splashAlpha * 0.7f),
                        radius = splashSize * 0.45f,
                        center = Offset(splashX, splashY),
                        style = Stroke(width = 6f * (1f - progress))
                    )
                }
            }

            // 5. Gráficos y textos HUD creados dinámicamente desde scripts (`makeHudGraphic`, `makeText`)
            if (frameState.hudVisible) {
                for (i in 0 until frameState.dynamicCount) {
                    val item = frameState.dynamicPool[i]
                    if (item.isHud && item.visible && item.opacity > 0.01f) {
                        drawDynamicItem(item, sx, sy, dynamicTextPaint)
                    }
                }
            }
        }

        // Capa multi-touch inteligente según el modo seleccionado
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
                                val useDownscrollTactile = frameState.downscroll
                                val baseReceptorY = if (useDownscrollTactile) 595f * sy else 118f * sy
                                val notePxSize = (if (useDownscrollTactile) 98f else 84f) * minOf(sx, sy)
                                val maxHitX = if (useDownscrollTactile) 82f * sx else maxOf(105f * sx * 0.54f, notePxSize * 0.80f)
                                val maxHitY = maxOf(notePxSize * 1.30f, 96f)

                                event.changes.forEach { change ->
                                    if (change.pressed) {
                                        val px = change.position.x
                                        val py = change.position.y
                                        var bestLane = -1
                                        var bestDistX = Float.MAX_VALUE
                                        for (dir in 0..3) {
                                            val ov = frameState.strumOverrides[4 + dir]
                                            val baseCx = if (useDownscrollTactile) {
                                                val vx = when (dir) {
                                                    0 -> 340f
                                                    1 -> 495f
                                                    2 -> 785f
                                                    else -> 940f
                                                }
                                                vx * sx
                                            } else {
                                                (765f + dir * 105f) * sx
                                            }
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

                            val newlyPressed = nextMask and pressedMask.inv()
                            if (newlyPressed != 0) {
                                pendingJustPressedMask = pendingJustPressedMask or newlyPressed
                            }
                            pressedMask = nextMask
                        }
                    }
                }
        ) {
            if (isClassicHitbox) {
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
                                color = col.copy(alpha = 0.82f),
                                fontWeight = FontWeight.Bold,
                                fontSize = 11.sp
                            )
                        }
                    }
                }
            }
        }

        // Nuevo HUD FNF limpio: Barra de vida SVG + íconos animados con rebote + Puntaje/Fallos/Precisión debajo
        if (hud.hudVisible) {
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
                // Canvas dedicado 60 FPS para la barra rojo/verde + marco SVG + íconos con latido BPM
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

                    // Lado izquierdo: Enemigo (Rojo); Lado derecho: Jugador (Verde)
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

                    // Marco SVG vectorial de barra de vida rasterizado por `resvg`
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

                    // Íconos de Enemigo y Jugador siguiendo el divisor y rebotando con el BPM
                    val beatImpact = (1f - frameState.beatPhase).coerceIn(0f, 1f)
                    val iconSize = 42.dp.toPx() * (1f + 0.14f * beatImpact * beatImpact)
                    val iconCy = barTop + barH * 0.5f

                    val oppBmp = if (clampedHealth > 0.80f) {
                        opponentLosingIcon ?: opponentNormalIcon
                    } else {
                        opponentNormalIcon
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
                        playerLosingIcon ?: playerNormalIcon
                    } else {
                        playerNormalIcon
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

                // Contadores limpios debajo de la barra: Puntaje | Fallos | Precisión
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

        // Botón compacto de Pausa en esquina superior derecha (sin barra invasiva)
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
                onClick = { onSetPaused(true) },
                modifier = Modifier.fillMaxSize()
            ) {
                Icon(Icons.Rounded.Pause, "Pausa", tint = StudioColors.TextPrimary, modifier = Modifier.size(19.dp))
            }
        }

        if (hud.scriptError.isNotBlank() || hud.scriptLog.isNotBlank()) {
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

        // Animación central de cuenta regresiva sincronizada con el BPM ("3, 2, 1, ¡YA!")
        if (hud.countdownStage in 0..3 && !hud.paused && !hud.finished) {
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

        // Diálogo de Pausa (incluye cambio rápido entre Tocar Flechas y Hitbox Clásico)
        if (hud.paused && !hud.finished) {
            Dialog(onDismissRequest = { onSetPaused(false) }) {
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
                            onClick = { onSetPaused(false) },
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

        // Diálogo de Resultados Finales
        if (hud.finished) {
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
    }
}

private fun DrawScope.drawReceptor(
    center: Offset,
    sizePx: Float,
    angleDeg: Float,
    color: Color,
    bitmap: ImageBitmap?,
    alpha: Float
) {
    val half = sizePx / 2f
    withTransform({
        if (angleDeg != 0f) {
            rotate(angleDeg, pivot = center)
        }
    }) {
        if (bitmap != null) {
            drawImage(
                image = bitmap,
                dstOffset = IntOffset((center.x - half).roundToInt(), (center.y - half).roundToInt()),
                dstSize = IntSize(sizePx.roundToInt().coerceAtLeast(2), sizePx.roundToInt().coerceAtLeast(2)),
                alpha = alpha
            )
        } else {
            drawCircle(
                color = color.copy(alpha = alpha),
                radius = half * 0.8f,
                center = center,
                style = Stroke(width = 4f)
            )
        }
    }
}

private fun DrawScope.drawDynamicItem(
    item: MutableDynamicDrawable,
    sx: Float,
    sy: Float,
    textPaint: Paint
) {
    val px = item.x * sx
    val py = item.y * sy
    val pw = (item.width * sx).coerceAtLeast(2f)
    val ph = (item.height * sy).coerceAtLeast(2f)
    val alpha = item.opacity.coerceIn(0f, 1f)

    if (item.isText) {
        if (item.text.isBlank()) return
        val scaledFontPx = (item.height.coerceIn(10f, 96f) * minOf(sx, sy)).coerceAtLeast(12f)
        val baseColor = item.colorArgb
        val origAlpha = (baseColor ushr 24) and 0xFF
        val combinedAlpha = (origAlpha * alpha).roundToInt().coerceIn(0, 255)
        textPaint.color = (combinedAlpha shl 24) or (baseColor and 0x00FFFFFF)
        textPaint.textSize = scaledFontPx
        withTransform({
            if (item.angle != 0f) {
                rotate(item.angle, pivot = Offset(px, py))
            }
        }) {
            drawContext.canvas.nativeCanvas.drawText(
                item.text,
                px,
                py + scaledFontPx * 0.85f,
                textPaint
            )
        }
    } else {
        val baseColor = Color(item.colorArgb)
        val finalColor = baseColor.copy(alpha = (baseColor.alpha * alpha).coerceIn(0f, 1f))
        withTransform({
            if (item.angle != 0f) {
                rotate(item.angle, pivot = Offset(px + pw * 0.5f, py + ph * 0.5f))
            }
        }) {
            drawRect(
                color = finalColor,
                topLeft = Offset(px, py),
                size = Size(pw, ph)
            )
        }
    }
}
