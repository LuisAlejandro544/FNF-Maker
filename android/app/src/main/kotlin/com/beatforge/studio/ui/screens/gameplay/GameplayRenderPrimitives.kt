// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/gameplay/GameplayRenderPrimitives.kt
// Propósito: Estructuras ligeras sin GC por frame (`GameplayTouchMaskState`, `RatingPopupPoolHolder`,
// `SplashPoolHolder`), resolvedor dinámico del jugador activo para que los popups de juicio
// sigan al jugador en cada frame, geometría de carriles y primitivas `DrawScope`.

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
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.MutableDynamicDrawable
import kotlin.math.roundToInt

@Stable
internal class GameplayTouchMaskState {
    var pressedMask by mutableIntStateOf(0)
    var pendingJustPressedMask: Int = 0
}

internal class RatingPopupSlot {
    var active: Boolean = false
    var ratingCode: Int = 0
    var startNano: Long = 0L
    var jitterX: Float = 0f
    var jitterY: Float = 0f
}

internal class LivePlayerAnchor {
    var x: Float = 880f
    var y: Float = 430f
    var width: Float = 240f
    var height: Float = 300f
    var sideSign: Float = -1f // -1f = izquierda del jugador (hacia centro), +1f = derecha
}

internal class RatingPopupPoolHolder {
    var lastEventId: Int = 0
    var cursor: Int = 0
    val slots = Array(6) { RatingPopupSlot() }
    val livePlayerAnchor = LivePlayerAnchor()

    fun clear() {
        for (i in slots.indices) {
            slots[i].active = false
        }
    }
}

/**
 * Resuelve cada frame la posición, tamaño y orientación del jugador actualmente activo
 * para que los popups `SICK!`/`GOOD!`/`BAD`/`MISS` sigan al jugador aunque camine,
 * cambie de pose (`anim_offsets`), haga tweens o cambie de personaje (`Change Character`).
 */
internal fun resolveLivePlayerAnchor(
    frameState: GameplayFrameState,
    out: LivePlayerAnchor
) {
    val entCount = frameState.entityCount
    val activePlayerIdx = frameState.activePlayerIdx
    var playerEntIdx = -1

    if (activePlayerIdx in 0 until entCount && !frameState.entitiesPool[activePlayerIdx].retired) {
        playerEntIdx = activePlayerIdx
    } else {
        for (i in 0 until entCount) {
            val ent = frameState.entitiesPool[i]
            if (ent.roleCode == 2 && ent.visible && !ent.retired) {
                playerEntIdx = i
                break
            }
        }
        if (playerEntIdx < 0) {
            for (i in 0 until entCount) {
                val ent = frameState.entitiesPool[i]
                if (ent.roleCode == 2 && !ent.retired) {
                    playerEntIdx = i
                    break
                }
            }
        }
    }

    if (playerEntIdx >= 0) {
        val ent = frameState.entitiesPool[playerEntIdx]
        out.x = ent.x
        out.y = ent.y
        out.width = (ent.width * ent.scale).coerceAtLeast(80f)
        out.height = (ent.height * ent.scale).coerceAtLeast(100f)
    } else {
        out.x = 880f
        out.y = 430f
        out.width = 240f
        out.height = 300f
    }

    val activeOppIdx = frameState.activeOpponentIdx
    var oppEntIdx = -1
    if (activeOppIdx in 0 until entCount && !frameState.entitiesPool[activeOppIdx].retired) {
        oppEntIdx = activeOppIdx
    } else {
        for (i in 0 until entCount) {
            val ent = frameState.entitiesPool[i]
            if (ent.roleCode == 1 && ent.visible && !ent.retired) {
                oppEntIdx = i
                break
            }
        }
        if (oppEntIdx < 0) {
            for (i in 0 until entCount) {
                val ent = frameState.entitiesPool[i]
                if (ent.roleCode == 1 && !ent.retired) {
                    oppEntIdx = i
                    break
                }
            }
        }
    }

    out.sideSign = if (oppEntIdx >= 0) {
        val oppX = frameState.entitiesPool[oppEntIdx].x
        if (out.x < oppX - 40f) 1f else -1f
    } else {
        if (out.x < frameState.virtualWidth * 0.42f) 1f else -1f
    }
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
