// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/gameplay/GameplayRenderPrimitives.kt
// Propósito: Estructuras ligeras sin GC por frame (`GameplayTouchMaskState`, `RatingPopupHolder`,
// `SplashPoolHolder`), geometría compartida de carriles móviles y primitivas `DrawScope`
// reutilizadas por el renderizador de mundo, notas y capa multi-touch de `GameplayScreen`.

package com.beatforge.studio.ui.screens.gameplay

import android.graphics.Paint
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import com.beatforge.studio.MutableDynamicDrawable
import kotlin.math.roundToInt

@Stable
internal class GameplayTouchMaskState {
    var pressedMask by mutableIntStateOf(0)
    var pendingJustPressedMask: Int = 0
}

internal class RatingPopupHolder {
    var lastEventId: Int = 0
    var active: Boolean = false
    var ratingCode: Int = 0
    var startNano: Long = 0L
    var worldX: Float = 860f
    var worldY: Float = 360f
}

internal class SplashSlot {
    var active: Boolean = false
    var lane: Int = 0
    var startNano: Long = 0L
    var angleOffset: Float = 0f
}

internal class SplashPoolHolder {
    var lastEventId: Int = 0
    var cursor: Int = 0
    val slots = Array(8) { SplashSlot() }
}

internal fun resolvePlayerLaneBaseX(
    dir: Int,
    useDownscrollTactileLayout: Boolean,
    sx: Float
): Float {
    return if (useDownscrollTactileLayout) {
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
}

internal fun resolveOpponentLaneBaseX(
    dir: Int,
    useDownscrollTactileLayout: Boolean,
    sx: Float
): Float {
    return if (useDownscrollTactileLayout) {
        (72f + dir * 68f) * sx
    } else {
        (105f + dir * 105f) * sx
    }
}

internal fun DrawScope.drawReceptor(
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

internal fun DrawScope.drawDynamicItem(
    item: MutableDynamicDrawable,
    sx: Float,
    sy: Float,
    textPaint: Paint,
    opacityScale: Float = 1f
) {
    val px = item.x * sx
    val py = item.y * sy
    val pw = (item.width * sx).coerceAtLeast(2f)
    val ph = (item.height * sy).coerceAtLeast(2f)
    val alpha = (item.opacity * opacityScale).coerceIn(0f, 1f)

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
