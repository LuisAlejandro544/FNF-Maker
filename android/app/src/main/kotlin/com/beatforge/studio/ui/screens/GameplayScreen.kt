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
import com.beatforge.studio.GameplayStep
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
    step: GameplayStep,
    arrowBitmaps: Map<Int, ImageBitmap>,
    receptorBitmaps: Map<Int, ImageBitmap>,
    compiledBitmaps: Map<String, LoadedEntityBitmap>,
    onStepFrame: suspend (dtSec: Float, pressedMask: Int, justPressedMask: Int) -> Unit,
    onChangeControlMode: (MobileControlMode) -> Unit,
    onSetPaused: (Boolean) -> Unit,
    onRestart: () -> Unit,
    onExitToStudio: () -> Unit
) {
    var pressedMask by remember { mutableIntStateOf(0) }
    var pendingJustPressedMask by remember { mutableIntStateOf(0) }
    val latestStep by rememberUpdatedState(step)
    val latestPaused by rememberUpdatedState(step.paused)
    val latestFinished by rememberUpdatedState(step.finished)

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
            if (!latestPaused && !latestFinished) {
                val justMask = pendingJustPressedMask
                pendingJustPressedMask = 0
                onStepFrame(dt, pressedMask, justMask)
            }
        }
    }

    val isClassicHitbox = step.controlMode == MobileControlMode.ClassicHitbox.key

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(StudioColors.BgDark)
    ) {
        // Lienzo principal de gameplay (entidades + cámara + receptores grises + colas sustain + notas SVG)
        Canvas(modifier = Modifier.fillMaxSize()) {
            val vw = step.virtualWidth.coerceAtLeast(1f)
            val vh = step.virtualHeight.coerceAtLeast(1f)
            val sx = size.width / vw
            val sy = size.height / vh

            val shakePhase = (step.songPosMs * 0.08).toFloat()
            val shakeX = sin(shakePhase) * step.cameraShake * sx
            val shakeY = cos(shakePhase * 1.3f) * step.cameraShake * sy
            val pivot = Offset(size.width * 0.5f, size.height * 0.5f)

            withTransform({
                translate(shakeX, shakeY)
                scale(step.cameraZoom, step.cameraZoom, pivot = pivot)
            }) {
                // 1. Dibujar entidades compiladas en Z-order
                step.entities.forEach { ent ->
                    if (!ent.visible || ent.opacity <= 0.01f) return@forEach
                    val cx = ent.x * sx
                    val cy = ent.y * sy
                    val w = ent.width * ent.scale * sx
                    val h = ent.height * ent.scale * sy
                    val left = cx - w / 2f
                    val top = cy - h / 2f
                    val loaded = compiledBitmaps[ent.id]

                    withTransform({
                        if (ent.flipX) {
                            scale(scaleX = -1f, scaleY = 1f, pivot = Offset(cx, cy))
                        }
                    }) {
                        if (loaded != null) {
                            val bmp = loaded.bitmap
                            val f = ent.frame
                            val scaleRatioX = bmp.width.toFloat() / loaded.origWidth.toFloat()
                            val scaleRatioY = bmp.height.toFloat() / loaded.origHeight.toFloat()

                            if (f != null && f.found && f.width > 0 && f.height > 0) {
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
                                    srcOffset = IntOffset(0, 0),
                                    srcSize = IntSize(bmp.width, bmp.height),
                                    dstOffset = IntOffset(left.roundToInt(), top.roundToInt()),
                                    dstSize = IntSize(w.roundToInt().coerceAtLeast(2), h.roundToInt().coerceAtLeast(2)),
                                    alpha = ent.opacity.coerceIn(0f, 1f),
                                    filterQuality = FilterQuality.Medium
                                )
                            }
                        } else {
                            val boxColor = when (ent.role) {
                                "background" -> Color(0xFF172033)
                                "opponent" -> StudioColors.AccentCoral.copy(alpha = 0.28f)
                                "player" -> StudioColors.AccentCyan.copy(alpha = 0.28f)
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

            // 2. Carriles y receptores (gris neutro por defecto; se iluminan con color SVG al pulsar/sostener)
            val receptorY = (if (step.downscroll) 575f else 118f) * sy
            val notePxSize = 84f * minOf(sx, sy)
            val oppStartX = 105f * sx
            val plrStartX = 765f * sx
            val laneStep = 105f * sx

            for (dir in 0..3) {
                val oppX = oppStartX + dir * laneStep
                val plrX = plrStartX + dir * laneStep
                val coloredBmp = arrowBitmaps[dir]
                val neutralBmp = receptorBitmaps[dir] ?: coloredBmp
                val laneColor = StudioColors.LaneColors[dir]

                val oppHolding = (step.opponentHoldingLanesMask and (1 shl dir)) != 0
                drawReceptor(
                    center = Offset(oppX, receptorY),
                    sizePx = notePxSize * (if (oppHolding) 0.98f else 0.92f),
                    color = if (oppHolding) laneColor else Color(0xFF94A3B8),
                    bitmap = if (oppHolding) coloredBmp else neutralBmp,
                    alpha = if (oppHolding) 0.98f else 0.72f
                )

                val isPressed = (pressedMask and (1 shl dir)) != 0
                val isHolding = (step.holdingLanesMask and (1 shl dir)) != 0
                val showColoredPlayer = isPressed || isHolding
                val playerScale = when {
                    isHolding -> 1.06f
                    isPressed -> 0.92f
                    else -> 1f
                }

                // En modo Tocar Flechas dibujamos un aro táctil sutil alrededor de las flechas del jugador
                if (!isClassicHitbox) {
                    drawCircle(
                        color = if (showColoredPlayer) laneColor.copy(alpha = 0.24f) else Color.White.copy(alpha = 0.06f),
                        radius = notePxSize * 0.58f,
                        center = Offset(plrX, receptorY)
                    )
                    drawCircle(
                        color = if (showColoredPlayer) laneColor.copy(alpha = 0.85f) else Color.White.copy(alpha = 0.22f),
                        radius = notePxSize * 0.58f,
                        center = Offset(plrX, receptorY),
                        style = Stroke(width = if (showColoredPlayer) 3.5f else 2f)
                    )
                }

                drawReceptor(
                    center = Offset(plrX, receptorY),
                    sizePx = notePxSize * playerScale,
                    color = if (showColoredPlayer) laneColor else Color(0xFF94A3B8),
                    bitmap = if (showColoredPlayer) coloredBmp else neutralBmp,
                    alpha = when {
                        isHolding -> 1f
                        isPressed -> 0.95f
                        else -> 0.84f
                    }
                )
            }

            // 3. Notas visibles y colas sustain desplazándose hasta consumirse en el receptor
            val pxPerMs = 0.44f * step.scrollSpeed.toFloat() * sy
            step.visibleNotes.forEach { note ->
                val dir = note.direction.coerceIn(0, 3)
                val baseStartX = if (note.isPlayer) plrStartX else oppStartX
                val noteX = baseStartX + dir * laneStep

                val headDeltaMs = (note.timeMs - step.songPosMs).toFloat()
                val tailEndDeltaMs = (note.timeMs + note.sustainMs - step.songPosMs).toFloat()
                val headY = if (step.downscroll) {
                    receptorY - headDeltaMs * pxPerMs
                } else {
                    receptorY + headDeltaMs * pxPerMs
                }
                val tailEndY = if (step.downscroll) {
                    receptorY - tailEndDeltaMs * pxPerMs
                } else {
                    receptorY + tailEndDeltaMs * pxPerMs
                }

                val anchorY = if (note.headHit) receptorY else headY
                val minY = minOf(anchorY, tailEndY)
                val maxY = maxOf(anchorY, tailEndY)
                if (maxY < -notePxSize || minY > size.height + notePxSize) return@forEach

                // Cola de nota larga (se ancla al receptor mientras se mantiene y se va acortando hasta terminar)
                if (note.sustainMs > 45.0) {
                    val tailTop = if (step.downscroll) tailEndY else anchorY
                    val tailBottom = if (step.downscroll) anchorY else tailEndY
                    val tailHeight = tailBottom - tailTop
                    if (tailHeight > 2f) {
                        val laneColor = StudioColors.LaneColors[dir]
                        val bodyWidth = notePxSize * 0.30f
                        val coreWidth = notePxSize * 0.11f
                        drawRoundRect(
                            color = laneColor.copy(alpha = if (note.headHit) 0.82f else 0.62f),
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

                // La cabeza SVG con color solo se dibuja si aún no ha sido golpeada (o no es un hold activo)
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
                            val s = latestStep
                            var nextMask = 0
                            val w = size.width.toFloat().coerceAtLeast(1f)
                            val h = size.height.toFloat().coerceAtLeast(1f)
                            val classicMode = s.controlMode == MobileControlMode.ClassicHitbox.key

                            if (classicMode) {
                                // Hitbox Clásico (Psych Engine Mobile): 4 columnas verticales completas
                                event.changes.forEach { change ->
                                    if (change.pressed) {
                                        val lane = ((change.position.x / w) * 4f)
                                            .toInt()
                                            .coerceIn(0, 3)
                                        nextMask = nextMask or (1 shl lane)
                                    }
                                }
                            } else {
                                // Por defecto ("Tocar Flechas" FNF Mobile): tocar directamente las 4 flechas
                                val vw = s.virtualWidth.coerceAtLeast(1f)
                                val vh = s.virtualHeight.coerceAtLeast(1f)
                                val sx = w / vw
                                val sy = h / vh
                                val receptorY = (if (s.downscroll) 575f else 118f) * sy
                                val notePxSize = 84f * minOf(sx, sy)
                                val plrStartX = 765f * sx
                                val laneStep = 105f * sx
                                val maxHitX = maxOf(laneStep * 0.54f, notePxSize * 0.80f)
                                val maxHitY = maxOf(notePxSize * 1.25f, 92f)

                                event.changes.forEach { change ->
                                    if (change.pressed) {
                                        val px = change.position.x
                                        val py = change.position.y
                                        if (abs(py - receptorY) <= maxHitY) {
                                            var bestLane = -1
                                            var bestDistX = Float.MAX_VALUE
                                            for (dir in 0..3) {
                                                val cx = plrStartX + dir * laneStep
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
                // Visualización translúcida de las 4 columnas verticales estilo Psych Engine Mobile Hitbox
                Row(modifier = Modifier.fillMaxSize()) {
                    for (lane in 0..3) {
                        val active = (pressedMask and (1 shl lane)) != 0 ||
                            (step.holdingLanesMask and (1 shl lane)) != 0
                        val col = StudioColors.LaneColors[lane]
                        Box(
                            modifier = Modifier
                                .weight(1f)
                                .fillMaxHeight()
                                .background(
                                    if (active) col.copy(alpha = 0.22f) else col.copy(alpha = 0.03f)
                                )
                                .padding(bottom = 14.dp),
                            contentAlignment = Alignment.BottomCenter
                        ) {
                            // Divisor vertical fino entre columnas hitbox
                            if (lane < 3) {
                                Box(
                                    modifier = Modifier
                                        .align(Alignment.CenterEnd)
                                        .width(1.dp)
                                        .fillMaxHeight()
                                        .background(Color.White.copy(alpha = 0.14f))
                                )
                            }
                            Text(
                                text = TOUCH_LANE_NAMES[lane],
                                color = if (active) Color.White else col.copy(alpha = 0.72f),
                                fontWeight = FontWeight.Bold,
                                fontSize = 11.sp
                            )
                        }
                    }
                }
            }
        }

        // HUD Superior compacto (no bloquea toques sobre los receptores en upscroll)
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
                        text = "Puntaje: ${step.score}",
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 12.sp
                    )
                    Text(
                        text = "Combo: ${step.combo}",
                        color = StudioColors.AccentCyan,
                        fontWeight = FontWeight.Bold,
                        fontSize = 12.sp
                    )
                    Text(
                        text = "Fallos: ${step.misses}",
                        color = StudioColors.AccentCoral,
                        fontSize = 11.sp
                    )
                    if (step.lastJudgement.isNotBlank()) {
                        Text(
                            text = step.lastJudgement,
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
                            progress = { step.health.coerceIn(0f, 1f) },
                            color = if (step.health >= 0.35f) StudioColors.AccentEmerald else StudioColors.AccentCoral,
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

        if (step.scriptErrors.isNotEmpty() || step.scriptLogs.isNotEmpty()) {
            val isErr = step.scriptErrors.isNotEmpty()
            val text = if (isErr) step.scriptErrors.first() else step.scriptLogs.last()
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
        if (step.countdownStage in 0..3 && !step.paused && !step.finished) {
            val p = step.countdownProgress.coerceIn(0f, 1f)
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
            val stageColor = when (step.countdownStage) {
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
                            text = step.countdownLabel,
                            color = stageColor,
                            fontSize = if (step.countdownStage == 3) 44.sp else 54.sp,
                            fontWeight = FontWeight.ExtraBold
                        )
                        Text(
                            text = "${step.activeBpm.roundToInt()} BPM",
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp,
                            fontWeight = FontWeight.SemiBold
                        )
                    }
                }
            }
        }

        // Diálogo de Pausa (incluye cambio rápido entre Tocar Flechas y Hitbox Clásico)
        if (step.paused && !step.finished) {
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
                                val selected = step.controlMode == mode.key
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
        if (step.finished) {
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
                        Text("Puntaje Final: ${step.score}", color = StudioColors.TextPrimary, fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
                        Text("Combo Máximo: ${step.maxCombo}  |  Aciertos: ${step.hits}  |  Fallos: ${step.misses}", color = StudioColors.TextMuted, fontSize = 12.sp)
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
