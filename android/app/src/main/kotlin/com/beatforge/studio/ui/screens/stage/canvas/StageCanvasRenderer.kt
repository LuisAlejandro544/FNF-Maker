// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/canvas/StageCanvasRenderer.kt
// Propósito: Subrutinas de dibujo `DrawScope` para el lienzo del editor de escenario:
// rejilla 1280x720, entidades/sprites Sparrow con culling, marcos secundarios de cámara,
// marco activo de cámara y previsualización recortada de la viñeta lateral.

package com.beatforge.studio.ui.screens.stage.canvas

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.drawText
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.StageCameraSlotSnapshot
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.screens.stage.StageLiveTransformState
import com.beatforge.studio.ui.screens.stage.resolveCameraSlotPreviewCenter
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.min
import kotlin.math.roundToInt

internal fun DrawScope.drawStageGrid(
    stageLeft: Float,
    stageTop: Float,
    stageW: Float,
    stageH: Float,
    showGrid: Boolean,
    wideView: Boolean
) {
    if (showGrid) {
        for (gx in 1..7) {
            val px = stageLeft + stageW * (gx / 8f)
            drawLine(
                StudioColors.BorderSubtle.copy(alpha = if (gx == 4) 0.55f else 0.28f),
                Offset(px, stageTop),
                Offset(px, stageTop + stageH),
                strokeWidth = if (gx == 4) 1.5f else 1f
            )
        }
        for (gy in 1..4) {
            val py = stageTop + stageH * (gy / 5f)
            drawLine(
                StudioColors.BorderSubtle.copy(alpha = 0.28f),
                Offset(stageLeft, py),
                Offset(stageLeft + stageW, py)
            )
        }
        drawLine(
            StudioColors.BorderSubtle.copy(alpha = 0.50f),
            Offset(stageLeft, stageTop + stageH * 0.5f),
            Offset(stageLeft + stageW, stageTop + stageH * 0.5f),
            strokeWidth = 1.3f
        )
    }

    if (wideView) {
        drawRect(
            color = StudioColors.TextMuted.copy(alpha = 0.45f),
            topLeft = Offset(stageLeft, stageTop),
            size = Size(stageW, stageH),
            style = Stroke(width = 1.6f, pathEffect = PathEffect.dashPathEffect(floatArrayOf(10f, 8f), 0f))
        )
    }
}

internal fun DrawScope.drawStageEntities(
    entities: List<StageEntitySnapshot>,
    selectedId: String?,
    isCameraSelected: Boolean,
    isSelectedLocked: Boolean,
    entityBitmaps: Map<String, LoadedEntityBitmap>,
    liveTransform: StageLiveTransformState,
    preMeasuredLabels: Map<String, TextLayoutResult>,
    stageLeft: Float,
    stageTop: Float,
    sx: Float,
    sy: Float
) {
    val isFastQuality = liveTransform.isInteracting || liveTransform.isInteractingCamera || entities.size > 20
    val filterQuality = if (isFastQuality) FilterQuality.Low else FilterQuality.Medium
    val selId = if (isCameraSelected) null else selectedId
    val useLive = selId != null && liveTransform.entityId == selId

    for (i in entities.indices) {
        val ent = entities[i]
        val isSel = ent.id == selId
        val effOpacity = if (isSel && useLive) liveTransform.opacity else ent.opacity
        if (!ent.visible || effOpacity <= 0.01f) continue

        val effScale = if (isSel && useLive) liveTransform.scale else ent.scale
        val cx = stageLeft + (if (isSel && useLive) liveTransform.x else ent.x) * sx
        val cy = stageTop + (if (isSel && useLive) liveTransform.y else ent.y) * sy
        val w = ent.width * effScale * sx
        val h = ent.height * effScale * sy
        val left = cx - w / 2f
        val top = cy - h / 2f

        if (!isSel && (left + w < -24f || left > size.width + 24f || top + h < -24f || top > size.height + 24f)) {
            continue
        }

        val loaded = entityBitmaps[ent.id]

        withTransform({
            if (ent.flipX) {
                scale(scaleX = -1f, scaleY = 1f, pivot = Offset(cx, cy))
            }
        }) {
            if (loaded != null) {
                val bmp = loaded.bitmap
                val f = ent.previewFrame
                if (f != null && f.found && f.width > 0 && f.height > 0) {
                    val scaleRatioX = bmp.width.toFloat() / loaded.origWidth.toFloat()
                    val scaleRatioY = bmp.height.toFloat() / loaded.origHeight.toFloat()
                    val srcX = (f.x * scaleRatioX).roundToInt().coerceIn(0, bmp.width - 1)
                    val srcY = (f.y * scaleRatioY).roundToInt().coerceIn(0, bmp.height - 1)
                    val srcW = (f.width * scaleRatioX).roundToInt().coerceIn(1, bmp.width - srcX)
                    val srcH = (f.height * scaleRatioY).roundToInt().coerceIn(1, bmp.height - srcY)

                    val visW = if (f.rotated) f.height else f.width
                    val visH = if (f.rotated) f.width else f.height
                    val logicalW = f.frameWidth.coerceAtLeast(visW).toFloat().coerceAtLeast(1f)
                    val logicalH = f.frameHeight.coerceAtLeast(visH).toFloat().coerceAtLeast(1f)
                    val fit = min(w / logicalW, h / logicalH)

                    val boxW = logicalW * fit
                    val boxH = logicalH * fit
                    val boxLeft = cx - boxW / 2f
                    val boxTop = (top + h) - boxH

                    val visDrawW = visW * fit
                    val visDrawH = visH * fit
                    val visLeft = boxLeft + (-f.frameX * fit)
                    val visTop = boxTop + (-f.frameY * fit)
                    val frameCenterX = visLeft + visDrawW / 2f
                    val frameCenterY = visTop + visDrawH / 2f

                    val dstW = (f.width * fit).roundToInt().coerceAtLeast(2)
                    val dstH = (f.height * fit).roundToInt().coerceAtLeast(2)
                    val dstLeft = (frameCenterX - dstW / 2f).roundToInt()
                    val dstTop = (frameCenterY - dstH / 2f).roundToInt()

                    withTransform({
                        if (f.rotated) {
                            rotate(-90f, pivot = Offset(frameCenterX, frameCenterY))
                        }
                    }) {
                        drawImage(
                            image = bmp,
                            srcOffset = IntOffset(srcX, srcY),
                            srcSize = IntSize(srcW, srcH),
                            dstOffset = IntOffset(dstLeft, dstTop),
                            dstSize = IntSize(dstW, dstH),
                            alpha = effOpacity.coerceIn(0f, 1f),
                            filterQuality = filterQuality
                        )
                    }
                } else {
                    drawImage(
                        image = bmp,
                        srcOffset = IntOffset.Zero,
                        srcSize = IntSize(bmp.width, bmp.height),
                        dstOffset = IntOffset(left.roundToInt(), top.roundToInt()),
                        dstSize = IntSize(w.roundToInt().coerceAtLeast(2), h.roundToInt().coerceAtLeast(2)),
                        alpha = effOpacity.coerceIn(0f, 1f),
                        filterQuality = filterQuality
                    )
                }
            } else {
                val fallbackColor = when (ent.role) {
                    "background" -> StudioColors.AccentIndigo
                    "opponent" -> StudioColors.AccentCoral
                    "player" -> StudioColors.AccentEmerald
                    else -> StudioColors.AccentCyan
                }
                drawRect(
                    color = fallbackColor.copy(alpha = 0.22f * effOpacity),
                    topLeft = Offset(left, top),
                    size = Size(w, h)
                )
                drawRect(
                    color = fallbackColor.copy(alpha = effOpacity),
                    topLeft = Offset(left, top),
                    size = Size(w, h),
                    style = Stroke(width = 2f)
                )
            }
        }

        if (loaded == null && w > 36f && h > 18f) {
            val layout = preMeasuredLabels[ent.id]
            if (layout != null) {
                drawText(
                    textLayoutResult = layout,
                    alpha = effOpacity.coerceIn(0f, 1f),
                    topLeft = Offset(
                        (cx - layout.size.width / 2f).coerceIn(left + 4f, (left + w - layout.size.width - 4f).coerceAtLeast(left + 4f)),
                        (cy - layout.size.height / 2f).coerceIn(top + 2f, (top + h - layout.size.height - 2f).coerceAtLeast(top + 2f))
                    )
                )
            }
        }

        if (isSel) {
            val selBorderColor = if (isSelectedLocked) StudioColors.AccentIndigo else StudioColors.AccentEmerald
            drawRect(
                color = selBorderColor,
                topLeft = Offset(left - 2f, top - 2f),
                size = Size(w + 4f, h + 4f),
                style = Stroke(
                    width = 2.5f,
                    pathEffect = if (isSelectedLocked) PathEffect.dashPathEffect(floatArrayOf(8f, 6f), 0f) else null
                )
            )
            if (!isSelectedLocked) {
                val handleCenter = Offset(left + w, top + h)
                drawCircle(
                    color = StudioColors.BgDark,
                    radius = 10.dp.toPx(),
                    center = handleCenter
                )
                drawCircle(
                    color = StudioColors.AccentEmerald,
                    radius = 8.dp.toPx(),
                    center = handleCenter
                )
            }
        }
    }
}

internal fun DrawScope.drawSecondaryCameraFrames(
    cameraSlots: List<StageCameraSlotSnapshot>,
    activeSlotIndex: Int,
    stageCamera: StageCameraSnapshot,
    entities: List<StageEntitySnapshot>,
    preMeasuredCamLabels: Map<Int, TextLayoutResult>,
    stageLeft: Float,
    stageTop: Float,
    sx: Float,
    sy: Float
) {
    cameraSlots.forEach { slot ->
        if (slot.index != activeSlotIndex) {
            val (otherX, otherY) = resolveCameraSlotPreviewCenter(stageCamera, slot, entities)
            val otherZoom = slot.zoom.coerceIn(0.35f, 2.5f)
            val oW = (1280f / otherZoom) * sx
            val oH = (720f / otherZoom) * sy
            val oCx = stageLeft + otherX * sx
            val oCy = stageTop + otherY * sy
            val ghostColor = when (slot.targetKind) {
                "player" -> StudioColors.AccentEmerald
                "opponent" -> StudioColors.AccentCoral
                "gf" -> StudioColors.AccentAmber
                "entity" -> StudioColors.AccentPurple
                else -> StudioColors.AccentCyan
            }.copy(alpha = 0.36f)
            drawRect(
                color = ghostColor,
                topLeft = Offset(oCx - oW * 0.5f, oCy - oH * 0.5f),
                size = Size(oW, oH),
                style = Stroke(width = 1.4f, pathEffect = PathEffect.dashPathEffect(floatArrayOf(10f, 8f), 0f))
            )
            drawCircle(color = ghostColor, radius = 4f, center = Offset(oCx, oCy))
            preMeasuredCamLabels[slot.index]?.let { lbl ->
                drawText(
                    textLayoutResult = lbl,
                    alpha = 0.65f,
                    topLeft = Offset(oCx + 6f, oCy - lbl.size.height - 2f)
                )
            }
        }
    }
}

internal fun DrawScope.drawActiveCameraAndVignette(
    stageCamera: StageCameraSnapshot,
    activeSlotIndex: Int,
    activeSlotZoom: Float,
    resolvedCamX: Float,
    resolvedCamY: Float,
    isCameraSelected: Boolean,
    preMeasuredCamLabels: Map<Int, TextLayoutResult>,
    stageLeft: Float,
    stageTop: Float,
    sx: Float,
    sy: Float
) {
    val camBoxW = (1280f / activeSlotZoom) * sx
    val camBoxH = (720f / activeSlotZoom) * sy
    val camCxPx = stageLeft + resolvedCamX * sx
    val camCyPx = stageTop + resolvedCamY * sy
    val camLeft = camCxPx - camBoxW * 0.5f
    val camTop = camCyPx - camBoxH * 0.5f

    val camColor = when {
        !stageCamera.enabled -> StudioColors.AccentCoral.copy(alpha = if (isCameraSelected) 0.9f else 0.45f)
        isCameraSelected -> StudioColors.AccentCyan
        else -> StudioColors.AccentCyan.copy(alpha = 0.50f)
    }
    val camDash = if (isCameraSelected && stageCamera.enabled) {
        null
    } else {
        PathEffect.dashPathEffect(floatArrayOf(14f, 10f), 0f)
    }

    if (isCameraSelected) {
        drawRect(
            color = StudioColors.AccentCyan.copy(alpha = 0.06f),
            topLeft = Offset(camLeft, camTop),
            size = Size(camBoxW, camBoxH)
        )
    }
    if (stageCamera.vignetteEnabled && stageCamera.vignetteAlpha > 0.01f && camBoxW > 4f && camBoxH > 4f) {
        val vigColor = parseStageHexColorOrDefault(stageCamera.vignetteColor)
        val vigAlpha = stageCamera.vignetteAlpha.coerceIn(0f, 1f)
        val vigSideW = (camBoxW * stageCamera.vignetteWidth.coerceIn(0.05f, 0.50f)).coerceAtLeast(2f)
        val vigVertH = (camBoxH * 0.14f).coerceAtLeast(2f)
        clipRect(
            left = camLeft,
            top = camTop,
            right = camLeft + camBoxW,
            bottom = camTop + camBoxH
        ) {
            drawRect(
                brush = Brush.horizontalGradient(
                    colors = listOf(vigColor.copy(alpha = vigAlpha), Color.Transparent),
                    startX = camLeft,
                    endX = camLeft + vigSideW
                ),
                topLeft = Offset(camLeft, camTop),
                size = Size(vigSideW, camBoxH)
            )
            drawRect(
                brush = Brush.horizontalGradient(
                    colors = listOf(Color.Transparent, vigColor.copy(alpha = vigAlpha)),
                    startX = camLeft + camBoxW - vigSideW,
                    endX = camLeft + camBoxW
                ),
                topLeft = Offset(camLeft + camBoxW - vigSideW, camTop),
                size = Size(vigSideW, camBoxH)
            )
            drawRect(
                brush = Brush.verticalGradient(
                    colors = listOf(vigColor.copy(alpha = vigAlpha * 0.28f), Color.Transparent),
                    startY = camTop,
                    endY = camTop + vigVertH
                ),
                topLeft = Offset(camLeft, camTop),
                size = Size(camBoxW, vigVertH)
            )
            drawRect(
                brush = Brush.verticalGradient(
                    colors = listOf(Color.Transparent, vigColor.copy(alpha = vigAlpha * 0.28f)),
                    startY = camTop + camBoxH - vigVertH,
                    endY = camTop + camBoxH
                ),
                topLeft = Offset(camLeft, camTop + camBoxH - vigVertH),
                size = Size(camBoxW, vigVertH)
            )
        }
    }
    drawRect(
        color = camColor,
        topLeft = Offset(camLeft, camTop),
        size = Size(camBoxW, camBoxH),
        style = Stroke(width = if (isCameraSelected) 3f else 1.8f, pathEffect = camDash)
    )

    val crossR = if (isCameraSelected) 12f else 8f
    drawLine(camColor, Offset(camCxPx - crossR, camCyPx), Offset(camCxPx + crossR, camCyPx), strokeWidth = 2f)
    drawLine(camColor, Offset(camCxPx, camCyPx - crossR), Offset(camCxPx, camCyPx + crossR), strokeWidth = 2f)

    preMeasuredCamLabels[activeSlotIndex]?.let { lbl ->
        drawText(
            textLayoutResult = lbl,
            alpha = if (isCameraSelected) 0.95f else 0.65f,
            topLeft = Offset(camCxPx + crossR + 4f, camCyPx - lbl.size.height - 2f)
        )
    }

    if (isCameraSelected) {
        val camHandleCenter = Offset(camLeft + camBoxW, camTop + camBoxH)
        drawCircle(
            color = StudioColors.BgDark,
            radius = 11.dp.toPx(),
            center = camHandleCenter
        )
        drawCircle(
            color = StudioColors.AccentCyan,
            radius = 8.5.dp.toPx(),
            center = camHandleCenter
        )
    }
}

private fun parseStageHexColorOrDefault(raw: String, fallback: Color = Color(0xFFEF4444)): Color {
    val clean = raw.trim().removePrefix("#")
    return try {
        when (clean.length) {
            6 -> Color((0xFF000000L or clean.toLong(16)).toInt())
            8 -> Color(clean.toLong(16).toInt())
            else -> fallback
        }
    } catch (_: Throwable) {
        fallback
    }
}
