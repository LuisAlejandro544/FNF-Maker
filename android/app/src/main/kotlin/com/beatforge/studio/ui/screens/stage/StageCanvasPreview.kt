// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/StageCanvasPreview.kt
// Propósito: Lienzo interactivo central 16:9 (espacio virtual 1280x720) del editor de escenario.
// Dibuja la rejilla guía, los bitmaps o placeholders etiquetados por rol, soporta volteo flipX,
// selección táctil por toque, arrastre para mover, pellizco de dos dedos y manija inferior derecha
// para agrandar o achicar directamente cualquier elemento (jugador, oponente, fondo o decoración).

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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
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
    entityBitmaps: Map<String, LoadedEntityBitmap>,
    onSelectEntity: (String) -> Unit,
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
    modifier: Modifier = Modifier
) {
    val latestEntities by rememberUpdatedState(entities)
    val latestSelected by rememberUpdatedState(selected)

    var dragX by remember(selected?.id) { mutableFloatStateOf(selected?.x ?: 640f) }
    var dragY by remember(selected?.id) { mutableFloatStateOf(selected?.y ?: 360f) }
    var dragScale by remember(selected?.id) { mutableFloatStateOf(selected?.scale ?: 1f) }
    var isInteracting by remember(selected?.id) { mutableStateOf(false) }

    LaunchedEffect(selected?.id, selected?.x, selected?.y, selected?.scale) {
        if (!isInteracting && selected != null) {
            dragX = selected.x
            dragY = selected.y
            dragScale = selected.scale
        }
    }

    val textMeasurer = rememberTextMeasurer()

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
                    .pointerInput(selected?.id) {
                        val handleHitRadiusPx = 30.dp.toPx()
                        awaitEachGesture {
                            val down = awaitFirstDown(requireUnconsumed = false)
                            val scaleToVirtX = 1280f / size.width.toFloat().coerceAtLeast(1f)
                            val scaleToVirtY = 720f / size.height.toFloat().coerceAtLeast(1f)
                            val selAtDown = latestSelected

                            var resizeHandleMode = false
                            var startScale = dragScale
                            var startDistPx = 1f
                            var cxPx = 0f
                            var cyPx = 0f

                            if (selAtDown != null && selAtDown.visible) {
                                cxPx = dragX / scaleToVirtX
                                cyPx = dragY / scaleToVirtY
                                val hwPx = (selAtDown.width * dragScale * 0.5f) / scaleToVirtX
                                val hhPx = (selAtDown.height * dragScale * 0.5f) / scaleToVirtY
                                val handleX = cxPx + hwPx
                                val handleY = cyPx + hhPx
                                if (hypot(down.position.x - handleX, down.position.y - handleY) <= handleHitRadiusPx) {
                                    resizeHandleMode = true
                                    isInteracting = true
                                    startScale = dragScale
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

                                val sel = latestSelected
                                if (pressedChanges.size >= 2 && sel != null) {
                                    resizeHandleMode = false
                                    val zoom = event.calculateZoom()
                                    val pan = event.calculatePan()
                                    if (!moved && (abs(zoom - 1f) > 0.01f || pan.getDistance() > touchSlop)) {
                                        moved = true
                                        isInteracting = true
                                    }
                                    if (moved) {
                                        dragScale = (dragScale * zoom).coerceIn(0.1f, 5.0f)
                                        dragX = (dragX + pan.x * scaleToVirtX).coerceIn(-600f, 1880f)
                                        dragY = (dragY + pan.y * scaleToVirtY).coerceIn(-400f, 1120f)
                                        onUpdateEntity(
                                            sel.id,
                                            sel.displayName,
                                            sel.role,
                                            0f,
                                            0f,
                                            dragX,
                                            dragY,
                                            true,
                                            dragScale,
                                            sel.opacity,
                                            sel.visible,
                                            sel.flipX,
                                            false
                                        )
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
                                            isInteracting = true
                                            if (sel != null) {
                                                dragX = (dragX + accumX * scaleToVirtX).coerceIn(-600f, 1880f)
                                                dragY = (dragY + accumY * scaleToVirtY).coerceIn(-400f, 1120f)
                                            }
                                        }
                                    }

                                    if (moved && sel != null) {
                                        if (resizeHandleMode) {
                                            val currDist = hypot(ch.position.x - cxPx, ch.position.y - cyPx)
                                            dragScale = (startScale * (currDist / startDistPx)).coerceIn(0.1f, 5.0f)
                                        } else if (delta != Offset.Zero) {
                                            dragX = (dragX + delta.x * scaleToVirtX).coerceIn(-600f, 1880f)
                                            dragY = (dragY + delta.y * scaleToVirtY).coerceIn(-400f, 1120f)
                                        }
                                        onUpdateEntity(
                                            sel.id,
                                            sel.displayName,
                                            sel.role,
                                            0f,
                                            0f,
                                            dragX,
                                            dragY,
                                            true,
                                            dragScale,
                                            sel.opacity,
                                            sel.visible,
                                            sel.flipX,
                                            false
                                        )
                                        didMutate = true
                                        if (ch.positionChanged()) ch.consume()
                                    }
                                }
                            }

                            val selEnd = latestSelected
                            if (didMutate && selEnd != null) {
                                onUpdateEntity(
                                    selEnd.id,
                                    selEnd.displayName,
                                    selEnd.role,
                                    0f,
                                    0f,
                                    dragX,
                                    dragY,
                                    true,
                                    dragScale,
                                    selEnd.opacity,
                                    selEnd.visible,
                                    selEnd.flipX,
                                    true
                                )
                            } else if (!moved) {
                                val vx = down.position.x * scaleToVirtX
                                val vy = down.position.y * scaleToVirtY
                                val hit = latestEntities.reversed().firstOrNull { ent ->
                                    if (!ent.visible) return@firstOrNull false
                                    val entScale = if (ent.id == selEnd?.id) dragScale else ent.scale
                                    val hw = (ent.width * entScale) * 0.5f
                                    val hh = (ent.height * entScale) * 0.5f
                                    abs(vx - ent.x) <= hw.coerceAtLeast(32f) &&
                                        abs(vy - ent.y) <= hh.coerceAtLeast(32f)
                                }
                                if (hit != null) {
                                    onSelectEntity(hit.id)
                                }
                            }
                            isInteracting = false
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

                entities.forEach { ent ->
                    if (!ent.visible || ent.opacity <= 0.01f) return@forEach
                    val isSel = ent.id == selected?.id
                    val effScale = if (isSel) dragScale else ent.scale
                    val cx = (if (isSel) dragX else ent.x) * sx
                    val cy = (if (isSel) dragY else ent.y) * sy
                    val w = ent.width * effScale * sx
                    val h = ent.height * effScale * sy
                    val left = cx - w / 2f
                    val top = cy - h / 2f
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
                                        alpha = ent.opacity.coerceIn(0f, 1f),
                                        filterQuality = FilterQuality.Medium
                                    )
                                }
                            } else {
                                drawImage(
                                    image = bmp,
                                    srcOffset = IntOffset.Zero,
                                    srcSize = IntSize(bmp.width, bmp.height),
                                    dstOffset = IntOffset(left.roundToInt(), top.roundToInt()),
                                    dstSize = IntSize(w.roundToInt().coerceAtLeast(2), h.roundToInt().coerceAtLeast(2)),
                                    alpha = ent.opacity.coerceIn(0f, 1f),
                                    filterQuality = FilterQuality.Medium
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
                                color = fallbackColor.copy(alpha = 0.22f * ent.opacity),
                                topLeft = Offset(left, top),
                                size = Size(w, h)
                            )
                            drawRect(
                                color = fallbackColor.copy(alpha = ent.opacity),
                                topLeft = Offset(left, top),
                                size = Size(w, h),
                                style = Stroke(width = 2f)
                            )
                        }
                    }

                    if (loaded == null && w > 36f && h > 18f) {
                        val layout = textMeasurer.measure(
                            text = ent.displayName,
                            style = TextStyle(
                                color = StudioColors.TextPrimary.copy(alpha = ent.opacity),
                                fontSize = 9.sp,
                                fontWeight = FontWeight.SemiBold
                            )
                        )
                        drawText(
                            textLayoutResult = layout,
                            topLeft = Offset(
                                (cx - layout.size.width / 2f).coerceIn(left + 4f, (left + w - layout.size.width - 4f).coerceAtLeast(left + 4f)),
                                (cy - layout.size.height / 2f).coerceIn(top + 2f, (top + h - layout.size.height - 2f).coerceAtLeast(top + 2f))
                            )
                        )
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
            }

            if (selected != null) {
                Surface(
                    modifier = Modifier
                        .align(Alignment.BottomCenter)
                        .padding(bottom = 4.dp),
                    shape = RoundedCornerShape(6.dp),
                    color = Color(0xAA101012),
                    border = BorderStroke(1.dp, StudioColors.BorderSubtle)
                ) {
                    Text(
                        text = "${selected.displayName} · Escala ${((dragScale * 100f).roundToInt() / 100f)}x (arrastra la esquina verde o pellizca)",
                        color = StudioColors.TextPrimary,
                        fontSize = 10.sp,
                        modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp)
                    )
                }
            }
        }
    }
}
