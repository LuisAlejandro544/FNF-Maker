// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/GameplayScreen.kt
// Propósito: Pantalla jugable en tiempo real en Compose Canvas a 60 FPS sincronizada
// con el reloj del mezclador Rust AAudio (`nativeStepGameplay`).
// Implementa:
// 1. Receptores principales sin color (gris neutro FNF) que se iluminan con el color SVG al pulsar/sostener.
// 2. Notas largas (sustain) que no desaparecen al golpear la cabeza: la cola sigue bajando/subiendo
//    hasta consumirse por completo en el receptor.
// 3. Dos modos de control táctil móvil:
//    - Por defecto (`tap_arrows`): 100% táctil directo sobre las 4 flechas del jugador.
//    - Clásico (`classic_hitbox`): 4 columnas verticales translúcidas estilo Psych Engine Mobile.

package com.beatforge.studio.ui.screens

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
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.MobileControlMode
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sin

private val TOUCH_LANE_NAMES = listOf("IZQ", "ABA", "ARR", "DER")

@Composable
fun GameplayScreen(
    frameState: GameplayFrameState,
    arrowBitmaps: Map<Int, ImageBitmap>,
    receptorBitmaps: Map<Int, ImageBitmap>,
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

            val vw = frameState.virtualWidth.coerceAtLeast(1f)
            val vh = frameState.virtualHeight.coerceAtLeast(1f)
            val sx = size.width / vw
            val sy = size.height / vh

            val shakePhase = (frameState.songPosMs * 0.08).toFloat()
            val shakeX = sin(shakePhase) * frameState.cameraShake * sx
            val shakeY = cos(shakePhase * 1.3f) * frameState.cameraShake * sy
            val pivot = Offset(size.width * 0.5f, size.height * 0.5f)

            withTransform({
                translate(shakeX, shakeY)
                scale(frameState.cameraZoom, frameState.cameraZoom, pivot = pivot)
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

            // 2. Carriles y receptores (sin color en reposo; se iluminan con color SVG al pulsar/sostener)
            val classicMode = frameState.controlMode == MobileControlMode.ClassicHitbox.key
            val useDownscrollTactileLayout = frameState.downscroll && !classicMode

            val oppReceptorY = if (useDownscrollTactileLayout) 100f * sy else (if (frameState.downscroll) 575f else 118f) * sy
            val plrReceptorY = if (useDownscrollTactileLayout) 595f * sy else (if (frameState.downscroll) 575f else 118f) * sy
            val oppDownscroll = if (useDownscrollTactileLayout) false else frameState.downscroll
            val plrDownscroll = frameState.downscroll

            val oppNoteSize = if (useDownscrollTactileLayout) 58f * minOf(sx, sy) else 84f * minOf(sx, sy)
            val plrNoteSize = if (useDownscrollTactileLayout) 98f * minOf(sx, sy) else 84f * minOf(sx, sy)

            for (dir in 0..3) {
                val oppX = if (useDownscrollTactileLayout) {
                    (72f + dir * 68f) * sx
                } else {
                    (105f + dir * 105f) * sx
                }
                val plrX = if (useDownscrollTactileLayout) {
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
                val coloredBmp = arrowBitmaps[dir]
                val neutralBmp = receptorBitmaps[dir] ?: coloredBmp
                val laneColor = StudioColors.LaneColors[dir]

                val oppHolding = (frameState.opponentHoldingLanesMask and (1 shl dir)) != 0
                drawReceptor(
                    center = Offset(oppX, oppReceptorY),
                    sizePx = oppNoteSize * (if (oppHolding) 1.06f else 0.94f),
                    color = if (oppHolding) laneColor else Color(0xFF94A3B8),
                    bitmap = if (oppHolding) coloredBmp else neutralBmp,
                    alpha = if (oppHolding) 0.98f else 0.72f
                )

                val isPressed = (pressedMask and (1 shl dir)) != 0
                val isHolding = (frameState.holdingLanesMask and (1 shl dir)) != 0
                val showColoredPlayer = isPressed || isHolding
                val playerScale = when {
                    isHolding -> 1.08f
                    isPressed -> 0.92f
                    else -> 1f
                }

                drawReceptor(
                    center = Offset(plrX, plrReceptorY),
                    sizePx = plrNoteSize * playerScale,
                    color = if (showColoredPlayer) laneColor else Color(0xFF94A3B8),
                    bitmap = if (showColoredPlayer) coloredBmp else neutralBmp,
                    alpha = when {
                        isHolding -> 1f
                        isPressed -> 0.96f
                        else -> 0.86f
                    }
                )
            }

            // 3. Notas visibles y colas sustain desplazándose hasta consumirse en el receptor
            val pxPerMs = 0.44f * frameState.scrollSpeed.toFloat() * sy
            val noteCount = frameState.visibleNoteCount
            val curSongPosMs = frameState.songPosMs
            for (i in 0 until noteCount) {
                val note = frameState.notesPool[i]
                val dir = note.direction.coerceIn(0, 3)
                val noteX = if (note.isPlayer) {
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
                val receptorY = if (note.isPlayer) plrReceptorY else oppReceptorY
                val laneDownscroll = if (note.isPlayer) plrDownscroll else oppDownscroll
                val notePxSize = if (note.isPlayer) plrNoteSize else oppNoteSize

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

                if (note.sustainMs > 45.0) {
                    val tailTop = if (laneDownscroll) tailEndY else anchorY
                    val tailBottom = if (laneDownscroll) anchorY else tailEndY
                    val tailHeight = tailBottom - tailTop
                    if (tailHeight > 2f) {
                        val laneColor = StudioColors.LaneColors[dir]
                        val bodyWidth = notePxSize * 0.30f
                        val coreWidth = notePxSize * 0.11f
                        drawRoundRect(
                            color = laneColor.copy(alpha = if (note.headHit) 0.84f else 0.64f),
                            topLeft = Offset(noteX - bodyWidth / 2f, tailTop),
                            size = Size(bodyWidth, tailHeight),
                            cornerRadius = CornerRadius(bodyWidth * 0.45f, bodyWidth * 0.45f)
                        )
                        drawRoundRect(
                            color = Color.White.copy(alpha = if (note.headHit) 0.55f else 0.32f),
                            topLeft = Offset(noteX - coreWidth / 2f, tailTop),
                            size = Size(coreWidth, tailHeight),
                            cornerRadius = CornerRadius(coreWidth * 0.5f, coreWidth * 0.5f)
                        )
                    }
                }

                if (!note.headHit) {
                    val bmp = arrowBitmaps[dir]
                    if (bmp != null) {
                        val half = notePxSize / 2f
                        drawImage(
                            image = bmp,
                            dstOffset = IntOffset((noteX - half).roundToInt(), (headY - half).roundToInt()),
                            dstSize = IntSize(notePxSize.roundToInt(), notePxSize.roundToInt())
                        )
                    } else {
                        drawCircle(
                            color = StudioColors.LaneColors[dir],
                            radius = notePxSize * 0.4f,
                            center = Offset(noteX, headY)
                        )
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
                                val receptorY = if (useDownscrollTactile) 595f * sy else 118f * sy
                                val notePxSize = (if (useDownscrollTactile) 98f else 84f) * minOf(sx, sy)
                                val maxHitX = if (useDownscrollTactile) 82f * sx else maxOf(105f * sx * 0.54f, notePxSize * 0.80f)
                                val maxHitY = maxOf(notePxSize * 1.30f, 96f)

                                event.changes.forEach { change ->
                                    if (change.pressed) {
                                        val px = change.position.x
                                        val py = change.position.y
                                        if (abs(py - receptorY) <= maxHitY) {
                                            var bestLane = -1
                                            var bestDistX = Float.MAX_VALUE
                                            for (dir in 0..3) {
                                                val cx = if (useDownscrollTactile) {
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
                                                val dx = abs(px - cx)
                                                if (dx <= maxHitX && dx < bestDistX) {
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

        // HUD Superior compacto (solo recomposiciones cuando cambia puntaje, combo, fallos o juicio)
        Surface(
            modifier = Modifier
                .align(Alignment.TopCenter)
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 4.dp),
            shape = RoundedCornerShape(10.dp),
            color = StudioColors.BgPanel.copy(alpha = 0.80f),
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 12.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    Text(
                        text = "Puntaje: ${hud.score}",
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 12.sp
                    )
                    Text(
                        text = "Combo: ${hud.combo}",
                        color = StudioColors.AccentCyan,
                        fontWeight = FontWeight.Bold,
                        fontSize = 12.sp
                    )
                    Text(
                        text = "Fallos: ${hud.misses}",
                        color = StudioColors.AccentCoral,
                        fontSize = 11.sp
                    )
                    if (hud.lastJudgement.isNotBlank()) {
                        Text(
                            text = hud.lastJudgement,
                            color = StudioColors.AccentAmber,
                            fontWeight = FontWeight.Bold,
                            fontSize = 12.sp
                        )
                    }
                }

                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                    Column(modifier = Modifier.width(140.dp)) {
                        LinearProgressIndicator(
                            progress = { hud.health.coerceIn(0f, 1f) },
                            color = if (hud.health >= 0.35f) StudioColors.AccentEmerald else StudioColors.AccentCoral,
                            trackColor = StudioColors.BgDark,
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(7.dp)
                        )
                    }
                    IconButton(
                        onClick = { onSetPaused(true) },
                        modifier = Modifier.size(28.dp)
                    ) {
                        Icon(Icons.Rounded.Pause, "Pausa", tint = StudioColors.TextPrimary, modifier = Modifier.size(18.dp))
                    }
                }
            }
        }

        if (hud.scriptError.isNotBlank() || hud.scriptLog.isNotBlank()) {
            val isErr = hud.scriptError.isNotBlank()
            val text = if (isErr) hud.scriptError else hud.scriptLog
            Surface(
                modifier = Modifier
                    .align(Alignment.TopStart)
                    .padding(start = 14.dp, top = 46.dp),
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
                        Text("¡Canción Completada!", color = StudioColors.AccentEmerald, fontWeight = FontWeight.Bold, fontSize = 18.sp)
                        Text("Puntaje Final: ${hud.score}", color = StudioColors.TextPrimary, fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
                        Text("Combo Máximo: ${hud.maxCombo}  |  Aciertos: ${hud.hits}  |  Fallos: ${hud.misses}", color = StudioColors.TextMuted, fontSize = 12.sp)
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
    color: Color,
    bitmap: ImageBitmap?,
    alpha: Float
) {
    val half = sizePx / 2f
    if (bitmap != null) {
        drawImage(
            image = bitmap,
            dstOffset = IntOffset((center.x - half).roundToInt(), (center.y - half).roundToInt()),
            dstSize = IntSize(sizePx.roundToInt(), sizePx.roundToInt()),
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
