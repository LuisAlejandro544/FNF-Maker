// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/gameplay/GameplayWorldRenderer.kt
// Propósito: Funciones de extensión sobre `DrawScope` para dibujar a 60 FPS sin recomposiciones
// las capas del gameplay: mundo con cámara Lua/shake y sprites Sparrow/Animate, popups SVG
// (`SICK!`/`GOOD!`/`BAD`/`MISS`), receptores con `strumOverrides`, notas con colas sustain,
// destellos `Note Splash` y elementos HUD dinámicos creados por scripts.

package com.beatforge.studio.ui.screens.gameplay

import android.graphics.Paint
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.MobileControlMode
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sin

internal fun DrawScope.drawGameplaySceneLayer(
    frameState: GameplayFrameState,
    nowNs: Long,
    sx: Float,
    sy: Float,
    compiledBitmapsByIndex: List<LoadedEntityBitmap?>,
    ratingBitmaps: Map<Int, ImageBitmap>,
    ratingPopup: RatingPopupHolder,
    dynamicTextPaint: Paint
) {
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
}

internal fun DrawScope.drawClassicHitboxColumns(
    frameState: GameplayFrameState,
    pressedMask: Int
) {
    if (frameState.controlMode != MobileControlMode.ClassicHitbox.key) return
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

internal fun DrawScope.drawStrumsNotesAndSplashes(
    frameState: GameplayFrameState,
    pressedMask: Int,
    nowNs: Long,
    sx: Float,
    sy: Float,
    arrowBitmaps: Map<Int, ImageBitmap>,
    receptorBitmaps: Map<Int, ImageBitmap>,
    noteSplashBitmap: ImageBitmap?,
    splashPool: SplashPoolHolder,
    dynamicTextPaint: Paint
) {
    val classicMode = frameState.controlMode == MobileControlMode.ClassicHitbox.key
    val useDownscrollTactileLayout = frameState.downscroll && !classicMode

    val baseOppY = if (useDownscrollTactileLayout) 100f * sy else (if (frameState.downscroll) 575f else 118f) * sy
    val basePlrY = if (useDownscrollTactileLayout) 595f * sy else (if (frameState.downscroll) 575f else 118f) * sy
    val oppDownscroll = if (useDownscrollTactileLayout) false else frameState.downscroll
    val plrDownscroll = frameState.downscroll

    val oppNoteSize = if (useDownscrollTactileLayout) 58f * minOf(sx, sy) else 84f * minOf(sx, sy)
    val plrNoteSize = if (useDownscrollTactileLayout) 98f * minOf(sx, sy) else 84f * minOf(sx, sy)

    // 2. Carriles y receptores (aplica strumOverrides modchart 0..7)
    for (dir in 0..3) {
        val oppOverride = frameState.strumOverrides[dir]
        val plrOverride = frameState.strumOverrides[4 + dir]

        val oppX = resolveOpponentLaneBaseX(dir, useDownscrollTactileLayout, sx) + oppOverride.offsetX * sx
        val oppY = baseOppY + oppOverride.offsetY * sy
        val plrX = resolvePlayerLaneBaseX(dir, useDownscrollTactileLayout, sx) + plrOverride.offsetX * sx
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
            resolvePlayerLaneBaseX(dir, useDownscrollTactileLayout, sx)
        } else {
            resolveOpponentLaneBaseX(dir, useDownscrollTactileLayout, sx)
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
        val splashX = resolvePlayerLaneBaseX(lane, useDownscrollTactileLayout, sx) + plrOv.offsetX * sx
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
