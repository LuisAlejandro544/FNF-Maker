// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/gameplay/GameplayHudOverlay.kt
// Propósito: Componentes de interfaz superpuesta (HUD) y diálogos de `GameplayScreen`:
// barra de vida SVG con íconos reactivos al BPM, estadísticas (`Puntaje | Fallos | Precisión`),
// botón compacto de Pausa, consola rápida de scripts, cuenta regresiva BPM y diálogos
// modales de Pausa y Resultados Finales.

package com.beatforge.studio.ui.screens.gameplay

import android.app.ActivityManager
import android.content.Context
import android.os.Debug
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.BugReport
import androidx.compose.material.icons.rounded.Home
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Replay
import androidx.compose.material.icons.rounded.Warning
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.scale
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.Shadow
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.beatforge.studio.CountdownStepSnapshot
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.GameplayHudState
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.MobileControlMode
import com.beatforge.studio.ui.theme.StudioColors
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import java.util.Locale
import kotlin.math.min
import kotlin.math.roundToInt

private data class LiveRamSnapshot(
    val appPssMb: Float = 0f,
    val nativeHeapMb: Float = 0f,
    val javaHeapMb: Float = 0f,
    val sysAvailMb: Long = 0L,
    val sysTotalMb: Long = 0L
)

@Composable
internal fun BoxScope.GameplayHealthBarAndStats(
    frameState: GameplayFrameState,
    hud: GameplayHudState,
    healthBarBitmap: ImageBitmap?,
    fallbackPlayerId: String?,
    fallbackOpponentId: String?,
    entityNormalIcons: Map<String, ImageBitmap>,
    entityLosingIcons: Map<String, ImageBitmap>,
    defaultPlayerNormalIcon: ImageBitmap?,
    defaultPlayerLosingIcon: ImageBitmap?,
    defaultOpponentNormalIcon: ImageBitmap?,
    defaultOpponentLosingIcon: ImageBitmap?
) {
    if (!hud.hudVisible) return
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
        // Canvas dedicado 60 FPS para la barra rojo/verde + marco SVG + íconos dinámicos con latido BPM
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

            val activePlayerId = frameState.entityIdsByIndex.getOrNull(frameState.activePlayerIdx)
                ?: fallbackPlayerId
            val activeOpponentId = frameState.entityIdsByIndex.getOrNull(frameState.activeOpponentIdx)
                ?: fallbackOpponentId

            val plrNormal = activePlayerId?.let { entityNormalIcons[it] } ?: defaultPlayerNormalIcon
            val plrLosing = activePlayerId?.let { entityLosingIcons[it] } ?: defaultPlayerLosingIcon
            val oppNormal = activeOpponentId?.let { entityNormalIcons[it] } ?: defaultOpponentNormalIcon
            val oppLosing = activeOpponentId?.let { entityLosingIcons[it] } ?: defaultOpponentLosingIcon

            val beatImpact = (1f - frameState.beatPhase).coerceIn(0f, 1f)
            val iconSize = 42.dp.toPx() * (1f + 0.14f * beatImpact * beatImpact)
            val iconCy = barTop + barH * 0.5f

            val oppBmp = if (clampedHealth > 0.80f) {
                oppLosing ?: oppNormal
            } else {
                oppNormal
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
                plrLosing ?: plrNormal
            } else {
                plrNormal
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

@Composable
internal fun BoxScope.GameplayPauseButton(
    discreet: Boolean = false,
    onPause: () -> Unit
) {
    Surface(
        modifier = Modifier
            .align(Alignment.TopEnd)
            .padding(top = 8.dp, end = 12.dp)
            .size(if (discreet) 30.dp else 36.dp)
            .alpha(if (discreet) 0.28f else 1f),
        shape = CircleShape,
        color = StudioColors.BgPanel.copy(alpha = 0.72f),
        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
    ) {
        IconButton(
            onClick = onPause,
            modifier = Modifier.fillMaxSize()
        ) {
            Icon(
                imageVector = Icons.Rounded.Pause,
                contentDescription = "Pausa",
                tint = StudioColors.TextPrimary,
                modifier = Modifier.size(if (discreet) 15.dp else 19.dp)
            )
        }
    }
}

@Composable
internal fun BoxScope.GameplayDebugOverlay(
    showRam: Boolean,
    showTelemetry: Boolean,
    fps: Int,
    frameMs: Float,
    compiledBitmapsCount: Int,
    renderBackendLabel: String,
    frameState: GameplayFrameState,
    hud: GameplayHudState
) {
    if (!showRam && !showTelemetry) return

    val context = LocalContext.current
    var ramInfo by remember { mutableStateOf(LiveRamSnapshot()) }

    LaunchedEffect(showRam) {
        if (!showRam) return@LaunchedEffect
        val am = context.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
        while (true) {
            val snap = withContext(Dispatchers.Default) {
                val memInfo = ActivityManager.MemoryInfo()
                am?.getMemoryInfo(memInfo)
                val dbgInfo = Debug.MemoryInfo()
                Debug.getMemoryInfo(dbgInfo)
                val rt = Runtime.getRuntime()
                val javaUsed = (rt.totalMemory() - rt.freeMemory()) / (1024f * 1024f)
                val nativeUsed = Debug.getNativeHeapAllocatedSize() / (1024f * 1024f)
                LiveRamSnapshot(
                    appPssMb = dbgInfo.totalPss / 1024f,
                    nativeHeapMb = nativeUsed,
                    javaHeapMb = javaUsed,
                    sysAvailMb = memInfo.availMem / (1024L * 1024L),
                    sysTotalMb = memInfo.totalMem / (1024L * 1024L)
                )
            }
            ramInfo = snap
            delay(750L)
        }
    }

    Surface(
        modifier = Modifier
            .align(Alignment.TopStart)
            .padding(start = 10.dp, top = 8.dp),
        shape = RoundedCornerShape(9.dp),
        color = Color(0xCC0D0E12),
        border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.55f))
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 7.dp),
            verticalArrangement = Arrangement.spacedBy(2.dp)
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(5.dp)
            ) {
                Icon(
                    imageVector = Icons.Rounded.BugReport,
                    contentDescription = null,
                    tint = StudioColors.AccentCyan,
                    modifier = Modifier.size(13.dp)
                )
                Text(
                    text = "DEBUG STUDIO",
                    color = StudioColors.AccentCyan,
                    fontSize = 10.sp,
                    fontWeight = FontWeight.ExtraBold
                )
                if (hud.botplay) {
                    Text(
                        text = "BOTPLAY",
                        color = StudioColors.AccentEmerald,
                        fontSize = 9.sp,
                        fontWeight = FontWeight.Bold
                    )
                }
                if (hud.practiceMode) {
                    Text(
                        text = "PRÁCTICA",
                        color = StudioColors.AccentAmber,
                        fontSize = 9.sp,
                        fontWeight = FontWeight.Bold
                    )
                }
            }

            if (showRam) {
                Text(
                    text = String.format(
                        Locale.US,
                        "RAM App (PSS): %.1f MB  |  Libre: %d / %d MB",
                        ramInfo.appPssMb,
                        ramInfo.sysAvailMb,
                        ramInfo.sysTotalMb
                    ),
                    color = StudioColors.TextPrimary,
                    fontSize = 10.sp,
                    fontWeight = FontWeight.SemiBold
                )
                Text(
                    text = String.format(
                        Locale.US,
                        "Rust Native: %.1f MB  |  JVM Heap: %.1f MB  |  Texturas: %d",
                        ramInfo.nativeHeapMb,
                        ramInfo.javaHeapMb,
                        compiledBitmapsCount
                    ),
                    color = StudioColors.TextMuted,
                    fontSize = 9.5.sp
                )
            }

            if (showTelemetry) {
                Text(
                    text = String.format(
                        Locale.US,
                        "%d FPS (%.1f ms)  |  %s  |  t=%.2fs  |  %.0f BPM",
                        fps,
                        frameMs,
                        renderBackendLabel,
                        frameState.songPosMs / 1000.0,
                        hud.activeBpm
                    ),
                    color = StudioColors.AccentEmerald,
                    fontSize = 10.sp,
                    fontWeight = FontWeight.SemiBold
                )
                Text(
                    text = "Capas: ${frameState.entityCount}  |  Notas vis.: ${frameState.visibleNoteCount}  |  Combo: ${hud.combo}",
                    color = StudioColors.TextMuted,
                    fontSize = 9.5.sp
                )
            }
        }
    }
}

@Composable
internal fun BoxScope.GameplayGameOverOverlay(
    hud: GameplayHudState,
    onConfirmRetry: () -> Unit,
    onExitToStudio: () -> Unit
) {
    if (!hud.gameOver) return
    val isConfirming = hud.gameOverPhase == 3

    Box(
        modifier = Modifier
            .fillMaxSize()
            .clickable(
                interactionSource = remember { MutableInteractionSource() },
                indication = null,
                enabled = !isConfirming,
                onClick = onConfirmRetry
            )
    ) {
        Surface(
            modifier = Modifier
                .align(Alignment.TopCenter)
                .padding(top = 18.dp),
            shape = RoundedCornerShape(14.dp),
            color = Color(0xD9160A0D),
            border = BorderStroke(
                1.5.dp,
                if (isConfirming) StudioColors.AccentEmerald else StudioColors.AccentCoral
            )
        ) {
            Column(
                modifier = Modifier.padding(horizontal = 24.dp, vertical = 12.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                Text(
                    text = if (isConfirming) "¡REINTENTANDO!" else "GAME OVER",
                    color = if (isConfirming) StudioColors.AccentEmerald else StudioColors.AccentCoral,
                    fontSize = 24.sp,
                    fontWeight = FontWeight.Black
                )
                Text(
                    text = if (isConfirming) {
                        "Reiniciando canción desde el inicio..."
                    } else {
                        "Toca el personaje o cualquier parte para reintentar"
                    },
                    color = StudioColors.TextPrimary,
                    fontSize = 12.sp,
                    fontWeight = FontWeight.SemiBold
                )
            }
        }

        if (!isConfirming) {
            OutlinedButton(
                onClick = onExitToStudio,
                shape = RoundedCornerShape(10.dp),
                border = BorderStroke(1.dp, StudioColors.BorderSubtle),
                modifier = Modifier
                    .align(Alignment.BottomEnd)
                    .padding(14.dp)
            ) {
                Icon(Icons.Rounded.Home, contentDescription = null, tint = StudioColors.TextPrimary, modifier = Modifier.size(16.dp))
                Spacer(Modifier.width(5.dp))
                Text("Salir al Estudio", color = StudioColors.TextPrimary, fontSize = 11.5.sp)
            }
        }
    }
}

@Composable
internal fun BoxScope.GameplayScriptBanner(hud: GameplayHudState) {
    if (hud.scriptError.isBlank() && hud.scriptLog.isBlank()) return
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

@Composable
internal fun BoxScope.GameplayCountdownOverlay(
    hud: GameplayHudState,
    countdownSlots: List<CountdownStepSnapshot> = emptyList(),
    countdownBitmaps: Map<Int, LoadedEntityBitmap> = emptyMap()
) {
    val stage = hud.countdownStage
    if (stage !in 0..3 || hud.paused || hud.finished) return
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
    val stageColor = when (stage) {
        0 -> StudioColors.AccentCoral
        1 -> StudioColors.AccentAmber
        2 -> StudioColors.AccentCyan
        else -> StudioColors.AccentEmerald
    }

    val loaded = countdownBitmaps[stage]?.takeIf { !it.bitmap.asAndroidBitmap().isRecycled }
    val slot = countdownSlots.firstOrNull { it.slotIndex == stage }
    val frames = slot?.frames?.filter { it.found && it.width > 0 && it.height > 0 }.orEmpty()

    Box(
        modifier = Modifier
            .align(Alignment.Center)
            .scale(popScale)
            .alpha(fadeAlpha),
        contentAlignment = Alignment.Center
    ) {
        if (loaded != null) {
            Canvas(modifier = Modifier.size(width = 360.dp, height = 180.dp)) {
                val bmp = loaded.bitmap
                if (frames.isNotEmpty()) {
                    val fIdx = (p.coerceIn(0f, 0.9999f) * frames.size)
                        .toInt()
                        .coerceIn(0, frames.lastIndex)
                    val frame = frames[fIdx]
                    val scaleRatioX = bmp.width.toFloat() / loaded.origWidth.coerceAtLeast(1).toFloat()
                    val scaleRatioY = bmp.height.toFloat() / loaded.origHeight.coerceAtLeast(1).toFloat()
                    val srcX = (frame.x * scaleRatioX).roundToInt().coerceIn(0, bmp.width - 1)
                    val srcY = (frame.y * scaleRatioY).roundToInt().coerceIn(0, bmp.height - 1)
                    val srcW = (frame.width * scaleRatioX).roundToInt().coerceIn(1, bmp.width - srcX)
                    val srcH = (frame.height * scaleRatioY).roundToInt().coerceIn(1, bmp.height - srcY)

                    val visW = if (frame.rotated) frame.height else frame.width
                    val visH = if (frame.rotated) frame.width else frame.height
                    val logicalW = frame.frameWidth.coerceAtLeast(visW).toFloat().coerceAtLeast(1f)
                    val logicalH = frame.frameHeight.coerceAtLeast(visH).toFloat().coerceAtLeast(1f)
                    val fit = min(size.width / logicalW, size.height / logicalH)

                    val boxW = logicalW * fit
                    val boxH = logicalH * fit
                    val boxLeft = (size.width - boxW) * 0.5f
                    val boxTop = (size.height - boxH) * 0.5f
                    val visLeft = boxLeft + (-frame.frameX * fit)
                    val visTop = boxTop + (-frame.frameY * fit)
                    val centerX = visLeft + (visW * fit) * 0.5f
                    val centerY = visTop + (visH * fit) * 0.5f

                    val dstW = (frame.width * fit).roundToInt().coerceAtLeast(2)
                    val dstH = (frame.height * fit).roundToInt().coerceAtLeast(2)
                    val dstLeft = (centerX - dstW * 0.5f).roundToInt()
                    val dstTop = (centerY - dstH * 0.5f).roundToInt()

                    withTransform({
                        if (frame.rotated) {
                            rotate(-90f, pivot = Offset(centerX, centerY))
                        }
                    }) {
                        drawImage(
                            image = bmp,
                            srcOffset = IntOffset(srcX, srcY),
                            srcSize = IntSize(srcW, srcH),
                            dstOffset = IntOffset(dstLeft, dstTop),
                            dstSize = IntSize(dstW, dstH),
                            filterQuality = FilterQuality.High
                        )
                    }
                } else {
                    val fit = min(size.width / bmp.width.coerceAtLeast(1), size.height / bmp.height.coerceAtLeast(1))
                    val dstW = (bmp.width * fit).roundToInt().coerceAtLeast(2)
                    val dstH = (bmp.height * fit).roundToInt().coerceAtLeast(2)
                    val dstLeft = ((size.width - dstW) * 0.5f).roundToInt()
                    val dstTop = ((size.height - dstH) * 0.5f).roundToInt()
                    drawImage(
                        image = bmp,
                        srcOffset = IntOffset(0, 0),
                        srcSize = IntSize(bmp.width, bmp.height),
                        dstOffset = IntOffset(dstLeft, dstTop),
                        dstSize = IntSize(dstW, dstH),
                        filterQuality = FilterQuality.High
                    )
                }
            }
        } else {
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
                        fontSize = if (stage == 3) 44.sp else 54.sp,
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
}

@Composable
internal fun GameplayPauseDialog(
    hud: GameplayHudState,
    debugShowRam: Boolean,
    debugShowTelemetry: Boolean,
    debugBotplay: Boolean,
    debugPracticeMode: Boolean,
    debugHideHud: Boolean,
    onChangeControlMode: (MobileControlMode) -> Unit,
    onToggleDebugRam: (Boolean) -> Unit,
    onToggleDebugTelemetry: (Boolean) -> Unit,
    onToggleDebugBotplay: (Boolean) -> Unit,
    onToggleDebugPractice: (Boolean) -> Unit,
    onToggleDebugHideHud: (Boolean) -> Unit,
    onTriggerGameOver: () -> Unit,
    onResume: () -> Unit,
    onRestart: () -> Unit,
    onExitToStudio: () -> Unit
) {
    if (!hud.paused || hud.finished) return
    Dialog(
        onDismissRequest = onResume,
        properties = DialogProperties(usePlatformDefaultWidth = false)
    ) {
        Surface(
            modifier = Modifier
                .fillMaxWidth(0.56f)
                .heightIn(max = 520.dp)
                .padding(12.dp),
            shape = RoundedCornerShape(14.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Column(
                modifier = Modifier
                    .padding(16.dp)
                    .verticalScroll(rememberScrollState()),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Text("Partida en Pausa", color = StudioColors.TextPrimary, fontWeight = FontWeight.Bold, fontSize = 17.sp)

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

                // Opciones Debug para grabación y pruebas en vivo
                Surface(
                    shape = RoundedCornerShape(10.dp),
                    color = StudioColors.BgCard,
                    border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.35f))
                ) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 10.dp, vertical = 8.dp),
                        verticalArrangement = Arrangement.spacedBy(4.dp)
                    ) {
                        Row(
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(6.dp)
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.BugReport,
                                contentDescription = null,
                                tint = StudioColors.AccentCyan,
                                modifier = Modifier.size(16.dp)
                            )
                            Text(
                                text = "Opciones Debug (Grabación y Pruebas)",
                                color = StudioColors.AccentCyan,
                                fontWeight = FontWeight.Bold,
                                fontSize = 12.sp
                            )
                        }

                        HorizontalDivider(color = StudioColors.BorderSubtle.copy(alpha = 0.6f))

                        PauseDebugSwitchRow(
                            title = "Monitor de RAM en Vivo",
                            subtitle = "PSS App, Heap Rust/JVM, Texturas y RAM libre",
                            checked = debugShowRam,
                            onCheckedChange = onToggleDebugRam
                        )
                        PauseDebugSwitchRow(
                            title = "Telemetría Motor & FPS",
                            subtitle = "FPS reales, ms/frame, Backend GPU, tiempo y entidades",
                            checked = debugShowTelemetry,
                            onCheckedChange = onToggleDebugTelemetry
                        )
                        PauseDebugSwitchRow(
                            title = "Botplay (Autoplay)",
                            subtitle = "Acierta todas las notas normales y evita las notas X",
                            checked = debugBotplay,
                            onCheckedChange = onToggleDebugBotplay
                        )
                        PauseDebugSwitchRow(
                            title = "Modo Práctica (Invencible)",
                            subtitle = "Evita morir aunque la vida llegue a cero",
                            checked = debugPracticeMode,
                            onCheckedChange = onToggleDebugPractice
                        )
                        PauseDebugSwitchRow(
                            title = "Ocultar HUD (Grabación Limpia)",
                            subtitle = "Oculta barra de vida/puntaje dejando pausa discreta",
                            checked = debugHideHud,
                            onCheckedChange = onToggleDebugHideHud
                        )

                        OutlinedButton(
                            onClick = onTriggerGameOver,
                            border = BorderStroke(1.dp, StudioColors.AccentCoral.copy(alpha = 0.7f)),
                            contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(32.dp)
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.Warning,
                                contentDescription = null,
                                tint = StudioColors.AccentCoral,
                                modifier = Modifier.size(15.dp)
                            )
                            Spacer(Modifier.width(6.dp))
                            Text(
                                text = "Probar Animación de Muerte (Game Over)",
                                color = StudioColors.AccentCoral,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.Bold
                            )
                        }
                    }
                }

                Button(
                    onClick = onResume,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Icon(Icons.Rounded.PlayArrow, null)
                    Spacer(Modifier.width(6.dp))
                    Text("Reanudar")
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    OutlinedButton(
                        onClick = onRestart,
                        modifier = Modifier.weight(1f)
                    ) {
                        Icon(Icons.Rounded.Replay, null, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(4.dp))
                        Text("Reiniciar", fontSize = 12.sp)
                    }
                    OutlinedButton(
                        onClick = onExitToStudio,
                        modifier = Modifier.weight(1f)
                    ) {
                        Icon(Icons.Rounded.Home, null, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(4.dp))
                        Text("Al Estudio", fontSize = 12.sp)
                    }
                }
            }
        }
    }
}

@Composable
private fun PauseDebugSwitchRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { onCheckedChange(!checked) }
            .padding(vertical = 2.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = title,
                color = StudioColors.TextPrimary,
                fontSize = 11.5.sp,
                fontWeight = FontWeight.SemiBold
            )
            Text(
                text = subtitle,
                color = StudioColors.TextMuted,
                fontSize = 9.5.sp,
                lineHeight = 12.sp
            )
        }
        Spacer(Modifier.width(8.dp))
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            colors = SwitchDefaults.colors(checkedThumbColor = StudioColors.AccentCyan)
        )
    }
}

@Composable
internal fun GameplayResultsDialog(
    hud: GameplayHudState,
    onRestart: () -> Unit,
    onExitToStudio: () -> Unit
) {
    if (!hud.finished) return
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
