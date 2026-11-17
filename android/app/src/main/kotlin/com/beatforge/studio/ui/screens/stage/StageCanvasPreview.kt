// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/StageCanvasPreview.kt
// Propósito: Lienzo interactivo central 16:9 (espacio virtual 1280x720) del editor de escenario.
// Optimizado para escenas con muchos objetos:
// - Lee y actualiza `StageLiveTransformState` solo en la fase de dibujo (`DrawScope`), evitando
//   recomposiciones de paneles, copias de listas o llamadas JNI durante arrastres/pellizcos 60Hz.
// - Pre-mide etiquetas de texto con `remember` fuera del bucle de dibujo.
// - Descarta objetos fuera del viewport y usa `FilterQuality.Low` mientras se interactúa.

package com.beatforge.studio.ui.screens.stage

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.abs
import kotlin.math.hypot
import kotlin.math.min
import kotlin.math.roundToInt

@Composable
fun StageCanvasPreview(
    entities: List<StageEntitySnapshot>,
    selected: StageEntitySnapshot?,
    stageCamera: StageCameraSnapshot,
    isCameraSelected: Boolean,
    entityBitmaps: Map<String, LoadedEntityBitmap>,
    liveTransform: StageLiveTransformState,
    onSelectEntity: (String) -> Unit,
    onSelectCamera: () -> Unit,
    onUpdateEntity: (
        entityId: String,
        name: String,
        role: String,
        dx: Float,
        dy: Float,
        absX: Float,
        absY: Float,
        useAbsPos: Boolean,
        scale: Float,
        opacity: Float,
        visible: Boolean,
        flipX: Boolean,
        commitSave: Boolean
    ) -> Unit,
    onUpdateCamera: (
        baseX: Float,
        baseY: Float,
        baseZoom: Float,
        commitSave: Boolean
    ) -> Unit,
    modifier: Modifier = Modifier
) {
    val latestEntities by rememberUpdatedState(entities)
    val latestSelected by rememberUpdatedState(selected)
    val latestCamera by rememberUpdatedState(stageCamera)
    val latestIsCameraSelected by rememberUpdatedState(isCameraSelected)

    val textMeasurer = rememberTextMeasurer()
    // Pre-medir etiquetas una sola vez cuando cambian las entidades, nunca dentro de Canvas DrawScope
    val preMeasuredLabels = remember(entities, textMeasurer) {
        entities.associate { ent ->
            ent.id to textMeasurer.measure(
                text = ent.displayName,
                style = TextStyle(
                    color = StudioColors.TextPrimary,
                    fontSize = 9.sp,
                    fontWeight = FontWeight.SemiBold
                )
            )
        }
    }

    Surface(
        modifier = modifier.fillMaxHeight(),
        shape = RoundedCornerShape(12.dp),
        color = StudioColors.BgPanel,
        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
    ) {
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(10.dp),
            contentAlignment = Alignment.Center
        ) {
            Canvas(
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(16f / 9f)
                    .background(StudioColors.BgDark, RoundedCornerShape(8.dp))
                    .pointerInput(selected?.id, isCameraSelected) {
                        val handleHitRadiusPx = 30.dp.toPx()
                        awaitEachGesture {
                            val down = awaitFirstDown(requireUnconsumed = false)
                            val scaleToVirtX = 1280f / size.width.toFloat().coerceAtLeast(1f)
                            val scaleToVirtY = 720f / size.height.toFloat().coerceAtLeast(1f)
                            val camMode = latestIsCameraSelected
                            val camAtDown = latestCamera
                            val selAtDown = if (camMode) null else latestSelected

                            if (camMode) {
                                liveTransform.syncFromCamera(camAtDown)
                            } else if (selAtDown != null) {
                                liveTransform.syncFromEntity(selAtDown)
                            }

                            var localX = if (selAtDown != null && liveTransform.entityId == selAtDown.id) liveTransform.x else (selAtDown?.x ?: 640f)
                            var localY = if (selAtDown != null && liveTransform.entityId == selAtDown.id) liveTransform.y else (selAtDown?.y ?: 360f)
                            var localScale = if (selAtDown != null && liveTransform.entityId == selAtDown.id) liveTransform.scale else (selAtDown?.scale ?: 1f)

                            var localCamX = if (camMode) liveTransform.camX else camAtDown.baseX
                            var localCamY = if (camMode) liveTransform.camY else camAtDown.baseY
                            var localCamZoom = if (camMode) liveTransform.camZoom else camAtDown.baseZoom

                            var resizeHandleMode = false
                            var startScale = localScale
                            var startCamZoom = localCamZoom
                            var startDistPx = 1f
                            var cxPx = 0f
                            var cyPx = 0f

                            if (camMode) {
                                cxPx = localCamX / scaleToVirtX
                                cyPx = localCamY / scaleToVirtY
                                val hwPx = ((1280f / localCamZoom) * 0.5f) / scaleToVirtX
                                val hhPx = ((720f / localCamZoom) * 0.5f) / scaleToVirtY
                                val handleX = cxPx + hwPx
                                val handleY = cyPx + hhPx
                                if (hypot(down.position.x - handleX, down.position.y - handleY) <= handleHitRadiusPx) {
                                    resizeHandleMode = true
                                    liveTransform.beginCameraInteraction(camAtDown)
                                    startCamZoom = localCamZoom
                                    startDistPx = hypot(down.position.x - cxPx, down.position.y - cyPx).coerceAtLeast(20f)
                                }
                            } else if (selAtDown != null && selAtDown.visible) {
                                cxPx = localX / scaleToVirtX
                                cyPx = localY / scaleToVirtY
                                val hwPx = (selAtDown.width * localScale * 0.5f) / scaleToVirtX
                                val hhPx = (selAtDown.height * localScale * 0.5f) / scaleToVirtY
                                val handleX = cxPx + hwPx
                                val handleY = cyPx + hhPx
                                if (hypot(down.position.x - handleX, down.position.y - handleY) <= handleHitRadiusPx) {
                                    resizeHandleMode = true
                                    liveTransform.beginInteraction(selAtDown)
                                    startScale = localScale
                                    startDistPx = hypot(down.position.x - cxPx, down.position.y - cyPx).coerceAtLeast(12f)
                                }
                            }

                            var moved = resizeHandleMode
                            var didMutate = false
                            var accumX = 0f
                            var accumY = 0f
                            val touchSlop = viewConfiguration.touchSlop

                            while (true) {
                                val event = awaitPointerEvent()
                                val pressedChanges = event.changes.filter { it.pressed }
                                if (pressedChanges.isEmpty()) break

                                if (camMode) {
                                    if (pressedChanges.size >= 2) {
                                        resizeHandleMode = false
                                        val zoom = event.calculateZoom()
                                        val pan = event.calculatePan()
                                        if (!moved && (abs(zoom - 1f) > 0.01f || pan.getDistance() > touchSlop)) {
                                            moved = true
                                            liveTransform.beginCameraInteraction(camAtDown)
                                        }
                                        if (moved) {
                                            // Separar dedos agranda el marco de captura (reduce zoom)
                                            localCamZoom = (localCamZoom / zoom).coerceIn(0.35f, 2.5f)
                                            localCamX = (localCamX + pan.x * scaleToVirtX).coerceIn(-1000f, 2600f)
                                            localCamY = (localCamY + pan.y * scaleToVirtY).coerceIn(-1000f, 1800f)
                                            liveTransform.updateCamera(localCamX, localCamY, localCamZoom)
                                            didMutate = true
                                            pressedChanges.forEach { ch ->
                                                if (ch.positionChanged()) ch.consume()
                                            }
                                        }
                                    } else {
                                        val ch = pressedChanges.first()
                                        val delta = ch.position - ch.previousPosition
                                        if (!moved) {
                                            accumX += delta.x
                                            accumY += delta.y
                                            if (hypot(accumX, accumY) > touchSlop) {
                                                moved = true
                                                liveTransform.beginCameraInteraction(camAtDown)
                                                localCamX = (localCamX + accumX * scaleToVirtX).coerceIn(-1000f, 2600f)
                                                localCamY = (localCamY + accumY * scaleToVirtY).coerceIn(-1000f, 1800f)
                                            }
                                        }
                                        if (moved) {
                                            if (resizeHandleMode) {
                                                val currDist = hypot(ch.position.x - cxPx, ch.position.y - cyPx).coerceAtLeast(20f)
                                                localCamZoom = (startCamZoom * (startDistPx / currDist)).coerceIn(0.35f, 2.5f)
                                            } else if (delta != Offset.Zero) {
                                                localCamX = (localCamX + delta.x * scaleToVirtX).coerceIn(-1000f, 2600f)
                                                localCamY = (localCamY + delta.y * scaleToVirtY).coerceIn(-1000f, 1800f)
                                            }
                                            liveTransform.updateCamera(localCamX, localCamY, localCamZoom)
                                            didMutate = true
                                            if (ch.positionChanged()) ch.consume()
                                        }
                                    }
                                } else {
                                    val sel = latestSelected
                                    if (pressedChanges.size >= 2 && sel != null) {
                                        resizeHandleMode = false
                                        val zoom = event.calculateZoom()
                                        val pan = event.calculatePan()
                                        if (!moved && (abs(zoom - 1f) > 0.01f || pan.getDistance() > touchSlop)) {
                                            moved = true
                                            liveTransform.beginInteraction(sel)
                                        }
                                        if (moved) {
                                            localScale = (localScale * zoom).coerceIn(0.1f, 5.0f)
                                            localX = (localX + pan.x * scaleToVirtX).coerceIn(-600f, 1880f)
                                            localY = (localY + pan.y * scaleToVirtY).coerceIn(-400f, 1120f)
                                            liveTransform.updatePositionAndScale(localX, localY, localScale)
                                            didMutate = true
                                            pressedChanges.forEach { ch ->
                                                if (ch.positionChanged()) ch.consume()
                                            }
                                        }
                                    } else {
                                        val ch = pressedChanges.first()
                                        val delta = ch.position - ch.previousPosition
                                        if (!moved) {
                                            accumX += delta.x
                                            accumY += delta.y
                                            if (hypot(accumX, accumY) > touchSlop) {
                                                moved = true
                                                if (sel != null) {
                                                    liveTransform.beginInteraction(sel)
                                                    localX = (localX + accumX * scaleToVirtX).coerceIn(-600f, 1880f)
                                                    localY = (localY + accumY * scaleToVirtY).coerceIn(-400f, 1120f)
                                                }
                                            }
                                        }

                                        if (moved && sel != null) {
                                            if (resizeHandleMode) {
                                                val currDist = hypot(ch.position.x - cxPx, ch.position.y - cyPx)
                                                localScale = (startScale * (currDist / startDistPx)).coerceIn(0.1f, 5.0f)
                                            } else if (delta != Offset.Zero) {
                                                localX = (localX + delta.x * scaleToVirtX).coerceIn(-600f, 1880f)
                                                localY = (localY + delta.y * scaleToVirtY).coerceIn(-400f, 1120f)
                                            }
                                            liveTransform.updatePositionAndScale(localX, localY, localScale)
                                            didMutate = true
                                            if (ch.positionChanged()) ch.consume()
                                        }
                                    }
                                }
                            }

                            val selEnd = latestSelected
                            if (camMode && didMutate) {
                                onUpdateCamera(localCamX, localCamY, localCamZoom, true)
                            } else if (!camMode && didMutate && selEnd != null) {
                                val effOpacity = if (liveTransform.entityId == selEnd.id) liveTransform.opacity else selEnd.opacity
                                onUpdateEntity(
                                    selEnd.id,
                                    selEnd.displayName,
                                    selEnd.role,
                                    0f,
                                    0f,
                                    localX,
                                    localY,
                                    true,
                                    localScale,
                                    effOpacity,
                                    selEnd.visible,
                                    selEnd.flipX,
                                    true
                                )
                            } else if (!moved) {
                                val vx = down.position.x * scaleToVirtX
                                val vy = down.position.y * scaleToVirtY
                                var hitId: String? = null
                                for (idx in latestEntities.indices.reversed()) {
                                    val ent = latestEntities[idx]
                                    if (!ent.visible) continue
                                    val isSelEnt = !camMode && ent.id == selEnd?.id && liveTransform.entityId == ent.id
                                    val entScale = if (isSelEnt) liveTransform.scale else ent.scale
                                    val entX = if (isSelEnt) liveTransform.x else ent.x
                                    val entY = if (isSelEnt) liveTransform.y else ent.y
                                    val hw = (ent.width * entScale) * 0.5f
                                    val hh = (ent.height * entScale) * 0.5f
                                    if (abs(vx - entX) <= hw.coerceAtLeast(32f) &&
                                        abs(vy - entY) <= hh.coerceAtLeast(32f)
                                    ) {
                                        hitId = ent.id
                                        break
                                    }
                                }
                                if (hitId != null) {
                                    onSelectEntity(hitId)
                                } else {
                                    val camDistCenter = hypot(vx - camAtDown.baseX, vy - camAtDown.baseY)
                                    if (camDistCenter <= 72f) {
                                        onSelectCamera()
                                    }
                                }
                            }
                            liveTransform.endInteraction()
                            liveTransform.endCameraInteraction()
                        }
                    }
            ) {
                val sx = size.width / 1280f
                val sy = size.height / 720f

                // Rejilla guía de 1280x720
                for (gx in 1..7) {
                    val px = size.width * (gx / 8f)
                    drawLine(StudioColors.BorderSubtle.copy(alpha = 0.3f), Offset(px, 0f), Offset(px, size.height))
                }
                for (gy in 1..4) {
                    val py = size.height * (gy / 5f)
                    drawLine(StudioColors.BorderSubtle.copy(alpha = 0.3f), Offset(0f, py), Offset(size.width, py))
                }

                val isFastQuality = liveTransform.isInteracting || liveTransform.isInteractingCamera || entities.size > 20
                val filterQuality = if (isFastQuality) FilterQuality.Low else FilterQuality.Medium
                val selId = if (isCameraSelected) null else selected?.id
                val useLive = selId != null && liveTransform.entityId == selId

                for (i in entities.indices) {
                    val ent = entities[i]
                    val isSel = ent.id == selId
                    val effOpacity = if (isSel && useLive) liveTransform.opacity else ent.opacity
                    if (!ent.visible || effOpacity <= 0.01f) continue

                    val effScale = if (isSel && useLive) liveTransform.scale else ent.scale
                    val cx = (if (isSel && useLive) liveTransform.x else ent.x) * sx
                    val cy = (if (isSel && useLive) liveTransform.y else ent.y) * sy
                    val w = ent.width * effScale * sx
                    val h = ent.height * effScale * sy
                    val left = cx - w / 2f
                    val top = cy - h / 2f

                    // Descartar objetos completamente fuera del lienzo visible
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
                        drawRect(
                            color = StudioColors.AccentEmerald,
                            topLeft = Offset(left - 2f, top - 2f),
                            size = Size(w + 4f, h + 4f),
                            style = Stroke(width = 2.5f)
                        )
                        // Manija táctil en la esquina inferior derecha para escalar directamente
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

                // Marco visual de captura de cámara (1280/zoom x 720/zoom centrado en camX, camY)
                val effCamX = if (liveTransform.isInteractingCamera) liveTransform.camX else stageCamera.baseX
                val effCamY = if (liveTransform.isInteractingCamera) liveTransform.camY else stageCamera.baseY
                val effCamZoom = (if (liveTransform.isInteractingCamera) liveTransform.camZoom else stageCamera.baseZoom)
                    .coerceIn(0.35f, 2.5f)
                val camBoxW = (1280f / effCamZoom) * sx
                val camBoxH = (720f / effCamZoom) * sy
                val camCxPx = effCamX * sx
                val camCyPx = effCamY * sy
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
                drawRect(
                    color = camColor,
                    topLeft = Offset(camLeft, camTop),
                    size = Size(camBoxW, camBoxH),
                    style = Stroke(width = if (isCameraSelected) 3f else 1.8f, pathEffect = camDash)
                )

                // Mira central de cámara
                val crossR = if (isCameraSelected) 12f else 8f
                drawLine(camColor, Offset(camCxPx - crossR, camCyPx), Offset(camCxPx + crossR, camCyPx), strokeWidth = 2f)
                drawLine(camColor, Offset(camCxPx, camCyPx - crossR), Offset(camCxPx, camCyPx + crossR), strokeWidth = 2f)

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

            if (isCameraSelected) {
                val pillZoom = (if (liveTransform.isInteractingCamera) liveTransform.camZoom else stageCamera.baseZoom)
                    .coerceIn(0.35f, 2.5f)
                val capW = (1280f / pillZoom).roundToInt()
                val capH = (720f / pillZoom).roundToInt()
                Surface(
                    modifier = Modifier
                        .align(Alignment.BottomCenter)
                        .padding(bottom = 4.dp),
                    shape = RoundedCornerShape(6.dp),
                    color = Color(0xAA101012),
                    border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.65f))
                ) {
                    Text(
                        text = "Cámara (.lua) · Captura ${capW}x${capH} · Zoom ${((pillZoom * 100f).roundToInt() / 100f)}x (agranda/achica esquina cian)",
                        color = StudioColors.AccentCyan,
                        fontSize = 10.sp,
                        modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp)
                    )
                }
            } else if (selected != null) {
                val pillScale = if (liveTransform.entityId == selected.id) liveTransform.scale else selected.scale
                Surface(
                    modifier = Modifier
                        .align(Alignment.BottomCenter)
                        .padding(bottom = 4.dp),
                    shape = RoundedCornerShape(6.dp),
                    color = Color(0xAA101012),
                    border = BorderStroke(1.dp, StudioColors.BorderSubtle)
                ) {
                    Text(
                        text = "${selected.displayName} · Escala ${((pillScale * 100f).roundToInt() / 100f)}x (arrastra la esquina verde o pellizca)",
                        color = StudioColors.TextPrimary,
                        fontSize = 10.sp,
                        modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp)
                    )
                }
            }
        }
    }
}
