// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/StageCanvasPreview.kt
// Propósito: Lienzo interactivo central 16:9 (espacio virtual 1280x720) del editor de escenario.
// Dibuja la rejilla guía, los bitmaps o placeholders etiquetados por rol, soporta volteo flipX,
// selección táctil por toque y reposicionamiento fluido mediante arrastre.

package com.beatforge.studio.ui.screens.stage

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.input.pointer.pointerInput
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
        flipX: Boolean
    ) -> Unit,
    modifier: Modifier = Modifier
) {
    val latestEntities by rememberUpdatedState(entities)
    val latestSelected by rememberUpdatedState(selected)

    var dragX by remember(selected?.id, selected?.x) { mutableFloatStateOf(selected?.x ?: 640f) }
    var dragY by remember(selected?.id, selected?.y) { mutableFloatStateOf(selected?.y ?: 360f) }

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
                    .pointerInput(Unit) {
                        detectTapGestures { tap ->
                            val scaleToVirtX = 1280f / size.width.toFloat()
                            val scaleToVirtY = 720f / size.height.toFloat()
                            val vx = tap.x * scaleToVirtX
                            val vy = tap.y * scaleToVirtY
                            val hit = latestEntities.reversed().firstOrNull { ent ->
                                if (!ent.visible) return@firstOrNull false
                                val hw = (ent.width * ent.scale) * 0.5f
                                val hh = (ent.height * ent.scale) * 0.5f
                                abs(vx - ent.x) <= hw.coerceAtLeast(32f) &&
                                    abs(vy - ent.y) <= hh.coerceAtLeast(32f)
                            }
                            if (hit != null) {
                                onSelectEntity(hit.id)
                            }
                        }
                    }
                    .pointerInput(selected?.id) {
                        detectDragGestures(
                            onDrag = { change, dragAmount ->
                                change.consume()
                                val sel = latestSelected ?: return@detectDragGestures
                                val scaleToVirtX = 1280f / size.width.toFloat()
                                val scaleToVirtY = 720f / size.height.toFloat()
                                dragX = (dragX + dragAmount.x * scaleToVirtX).coerceIn(-600f, 1880f)
                                dragY = (dragY + dragAmount.y * scaleToVirtY).coerceIn(-400f, 1120f)
                                onUpdateEntity(
                                    sel.id,
                                    sel.displayName,
                                    sel.role,
                                    0f,
                                    0f,
                                    dragX,
                                    dragY,
                                    true,
                                    sel.scale,
                                    sel.opacity,
                                    sel.visible,
                                    sel.flipX
                                )
                            }
                        )
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
                    val cx = (if (ent.id == selected?.id) dragX else ent.x) * sx
                    val cy = (if (ent.id == selected?.id) dragY else ent.y) * sy
                    val w = ent.width * ent.scale * sx
                    val h = ent.height * ent.scale * sy
                    val left = cx - w / 2f
                    val top = cy - h / 2f
                    val loaded = entityBitmaps[ent.id]

                    withTransform({
                        if (ent.flipX) {
                            scale(scaleX = -1f, scaleY = 1f, pivot = Offset(cx, cy))
                        }
                    }) {
                        if (loaded != null) {
                            drawImage(
                                image = loaded.bitmap,
                                srcOffset = IntOffset.Zero,
                                srcSize = IntSize(loaded.bitmap.width, loaded.bitmap.height),
                                dstOffset = IntOffset(left.roundToInt(), top.roundToInt()),
                                dstSize = IntSize(w.roundToInt().coerceAtLeast(2), h.roundToInt().coerceAtLeast(2)),
                                alpha = ent.opacity.coerceIn(0f, 1f)
                            )
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

                    if (ent.id == selected?.id) {
                        drawRect(
                            color = StudioColors.AccentEmerald,
                            topLeft = Offset(left - 2f, top - 2f),
                            size = Size(w + 4f, h + 4f),
                            style = Stroke(width = 2.5f)
                        )
                    }
                }
            }
        }
    }
}
