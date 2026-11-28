// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/GameplayScreen.kt
// Propósito: Coordinador modular de la pantalla jugable a 60 FPS sobre OpenGL ES 3.0 por defecto
// (con fallback automático a OpenGL ES 2.0 y respaldo Canvas 2D en modo seguro), sincronizado
// con el reloj del mezclador Rust AAudio (`nativeStepGameplayBinary`).
// Orquesta el bucle `withFrameNanos` y delega dibujo GPU/Canvas, entrada multi-touch y HUD/diálogos
// a los submódulos de `ui/screens/gameplay/`.

package com.beatforge.studio.ui.screens

import android.graphics.Paint
import android.graphics.Typeface
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import com.beatforge.studio.CountdownStepSnapshot
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.MobileControlMode
import com.beatforge.studio.gfx.RenderBackendKind
import com.beatforge.studio.ui.screens.gameplay.GameplayCountdownOverlay
import com.beatforge.studio.ui.screens.gameplay.GameplayDebugOverlay
import com.beatforge.studio.ui.screens.gameplay.GameplayGameOverOverlay
import com.beatforge.studio.ui.screens.gameplay.GameplayGlesSurface
import com.beatforge.studio.ui.screens.gameplay.GameplayHealthBarAndStats
import com.beatforge.studio.ui.screens.gameplay.GameplayPauseButton
import com.beatforge.studio.ui.screens.gameplay.GameplayPauseDialog
import com.beatforge.studio.ui.screens.gameplay.GameplayResultsDialog
import com.beatforge.studio.ui.screens.gameplay.GameplayScriptBanner
import com.beatforge.studio.ui.screens.gameplay.GameplayTouchMaskState
import com.beatforge.studio.ui.screens.gameplay.GameplayTouchOverlay
import com.beatforge.studio.ui.screens.gameplay.RatingPopupPoolHolder
import com.beatforge.studio.ui.screens.gameplay.SplashPoolHolder
import com.beatforge.studio.ui.screens.gameplay.drawCameraSideVignette
import com.beatforge.studio.ui.screens.gameplay.drawClassicHitboxColumns
import com.beatforge.studio.ui.screens.gameplay.drawGameplaySceneLayer
import com.beatforge.studio.ui.screens.gameplay.drawStrumsNotesAndSplashes
import com.beatforge.studio.ui.screens.gameplay.extractAllArrowLanePalettes
import com.beatforge.studio.ui.theme.StudioColors

@Composable
fun GameplayScreen(
    frameState: GameplayFrameState,
    arrowBitmaps: Map<Int, ImageBitmap>,
    receptorBitmaps: Map<Int, ImageBitmap>,
    hurtArrowXBitmap: ImageBitmap?,
    healthBarBitmap: ImageBitmap?,
    ratingBitmaps: Map<Int, ImageBitmap>,
    noteSplashBitmap: ImageBitmap?,
    countdownSlots: List<CountdownStepSnapshot>,
    countdownBitmaps: Map<Int, LoadedEntityBitmap>,
    fallbackPlayerId: String?,
    fallbackOpponentId: String?,
    entityNormalIcons: Map<String, ImageBitmap>,
    entityLosingIcons: Map<String, ImageBitmap>,
    defaultPlayerNormalIcon: ImageBitmap?,
    defaultPlayerLosingIcon: ImageBitmap?,
    defaultOpponentNormalIcon: ImageBitmap?,
    defaultOpponentLosingIcon: ImageBitmap?,
    compiledBitmapsByIndex: List<LoadedEntityBitmap?>,
    debugShowRam: Boolean,
    debugShowTelemetry: Boolean,
    debugBotplay: Boolean,
    debugPracticeMode: Boolean,
    debugHideHud: Boolean,
    renderBackendKey: String = RenderBackendKind.Gles3.key,
    renderBackendLabel: String,
    onStepFrame: suspend (dtSec: Float, pressedMask: Int, justPressedMask: Int) -> Unit,
    onChangeControlMode: (MobileControlMode) -> Unit,
    onSetPaused: (Boolean) -> Unit,
    onRestart: () -> Unit,
    onTriggerGameOver: () -> Unit,
    onConfirmGameOverRetry: () -> Unit,
    onUpdateDebug: (showRam: Boolean, showTelemetry: Boolean, botplay: Boolean, practiceMode: Boolean, hideHud: Boolean) -> Unit,
    onExitToStudio: () -> Unit
) {
    val touchMaskState = remember { GameplayTouchMaskState() }
    val hud = frameState.hudState
    var liveFps by remember { mutableIntStateOf(60) }
    var liveFrameMs by remember { mutableFloatStateOf(16.6f) }

    val backendKind = remember(renderBackendKey) { RenderBackendKind.fromKey(renderBackendKey) }
    var forceCanvasFallback by remember(renderBackendKey) { mutableStateOf(false) }
    var activeGlLabel by remember(renderBackendKey, renderBackendLabel) { mutableStateOf(renderBackendLabel) }
    val useGlesSurface = backendKind != RenderBackendKind.Canvas2d && !forceCanvasFallback

    val ratingPool = remember { RatingPopupPoolHolder() }
    val splashPool = remember { SplashPoolHolder() }
    val lanePalettes = remember(arrowBitmaps) { extractAllArrowLanePalettes(arrowBitmaps) }
    val dynamicTextPaint = remember {
        Paint(Paint.ANTI_ALIAS_FLAG).apply {
            typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
            setShadowLayer(4f, 2f, 2f, android.graphics.Color.BLACK)
        }
    }

    LaunchedEffect(Unit) {
        var lastNs = 0L
        var accumSec = 0f
        var frameCounter = 0
        while (true) {
            var dt = 0.016f
            withFrameNanos { ns ->
                if (lastNs != 0L) {
                    val rawDt = (ns - lastNs) / 1_000_000_000f
                    dt = rawDt.coerceIn(0.001f, 0.05f)
                    accumSec += rawDt
                    frameCounter += 1
                    if (accumSec >= 0.35f) {
                        val avgDt = accumSec / frameCounter.coerceAtLeast(1)
                        liveFps = (1f / avgDt.coerceAtLeast(0.001f)).toInt().coerceIn(1, 240)
                        liveFrameMs = avgDt * 1000f
                        accumSec = 0f
                        frameCounter = 0
                    }
                }
                lastNs = ns
            }
            if (!frameState.paused && !frameState.finished) {
                val justMask = touchMaskState.pendingJustPressedMask
                touchMaskState.pendingJustPressedMask = 0
                onStepFrame(dt, touchMaskState.pressedMask, justMask)
            }
        }
    }

    val isClassicHitbox = hud.controlMode == MobileControlMode.ClassicHitbox.key

    Box(
        modifier = if (useGlesSurface) {
            Modifier.fillMaxSize()
        } else {
            Modifier
                .fillMaxSize()
                .background(StudioColors.BgDark)
        }
    ) {
        if (useGlesSurface) {
            // Renderizador GPU por defecto: OpenGL ES 3.0 con fallback automático a OpenGL ES 2.0
            GameplayGlesSurface(
                frameState = frameState,
                touchMaskState = touchMaskState,
                preferEs3 = backendKind != RenderBackendKind.Gles2,
                arrowBitmaps = arrowBitmaps,
                receptorBitmaps = receptorBitmaps,
                hurtArrowXBitmap = hurtArrowXBitmap,
                ratingBitmaps = ratingBitmaps,
                noteSplashBitmap = noteSplashBitmap,
                compiledBitmapsByIndex = compiledBitmapsByIndex,
                ratingPool = ratingPool,
                splashPool = splashPool,
                lanePalettes = lanePalettes,
                onGlContextReady = { resolvedLabel ->
                    activeGlLabel = resolvedLabel
                },
                onGlFatalFallback = { _ ->
                    forceCanvasFallback = true
                    activeGlLabel = "Canvas 2D (Fallback)"
                },
                modifier = Modifier.fillMaxSize()
            )
        } else {
            // Modo seguro / respaldo Canvas 2D: lee `frameState.drawTick` solo dentro de DrawScope (0 recomposiciones a 60 FPS)
            Canvas(modifier = Modifier.fillMaxSize()) {
                if (frameState.drawTick < 0L) return@Canvas
                val nowNs = System.nanoTime()

                if (frameState.ratingEventId == 0 && ratingPool.lastEventId != 0) {
                    ratingPool.lastEventId = 0
                    ratingPool.clear()
                } else if (frameState.ratingEventId != ratingPool.lastEventId) {
                    ratingPool.lastEventId = frameState.ratingEventId
                    if (frameState.lastRatingCode in 1..4) {
                        val slot = ratingPool.slots[ratingPool.cursor % ratingPool.slots.size]
                        ratingPool.cursor += 1
                        slot.active = true
                        slot.ratingCode = frameState.lastRatingCode
                        slot.startNano = nowNs
                        slot.jitterX = (((ratingPool.cursor * 19) % 28) - 14).toFloat()
                        slot.jitterY = (((ratingPool.cursor * 23) % 24) - 12).toFloat()
                    }
                }

                if (frameState.splashEventId != splashPool.lastEventId) {
                    splashPool.lastEventId = frameState.splashEventId
                    val slot = splashPool.slots[splashPool.cursor and 7]
                    splashPool.cursor += 1
                    slot.active = true
                    slot.lane = frameState.splashLane.coerceIn(0, 3)
                    slot.startNano = nowNs
                    slot.angleOffset = ((splashPool.cursor * 37) % 60 - 30).toFloat()
                }

                val vw = frameState.virtualWidth.coerceAtLeast(1f)
                val vh = frameState.virtualHeight.coerceAtLeast(1f)
                val sx = size.width / vw
                val sy = size.height / vh

                drawGameplaySceneLayer(
                    frameState = frameState,
                    nowNs = nowNs,
                    sx = sx,
                    sy = sy,
                    compiledBitmapsByIndex = compiledBitmapsByIndex,
                    ratingBitmaps = ratingBitmaps,
                    ratingPool = ratingPool,
                    dynamicTextPaint = dynamicTextPaint
                )

                if (!frameState.vignetteOverHud) {
                    drawCameraSideVignette(frameState)
                }

                drawClassicHitboxColumns(
                    frameState = frameState,
                    pressedMask = touchMaskState.pressedMask,
                    lanePalettes = lanePalettes
                )

                drawStrumsNotesAndSplashes(
                    frameState = frameState,
                    pressedMask = touchMaskState.pressedMask,
                    nowNs = nowNs,
                    sx = sx,
                    sy = sy,
                    arrowBitmaps = arrowBitmaps,
                    receptorBitmaps = receptorBitmaps,
                    hurtArrowXBitmap = hurtArrowXBitmap,
                    noteSplashBitmap = noteSplashBitmap,
                    splashPool = splashPool,
                    lanePalettes = lanePalettes,
                    dynamicTextPaint = dynamicTextPaint
                )
            }
        }

        GameplayTouchOverlay(
            frameState = frameState,
            isClassicHitbox = isClassicHitbox,
            touchMaskState = touchMaskState
        )

        if (!debugHideHud && !hud.gameOver) {
            GameplayHealthBarAndStats(
                frameState = frameState,
                hud = hud,
                healthBarBitmap = healthBarBitmap,
                fallbackPlayerId = fallbackPlayerId,
                fallbackOpponentId = fallbackOpponentId,
                entityNormalIcons = entityNormalIcons,
                entityLosingIcons = entityLosingIcons,
                defaultPlayerNormalIcon = defaultPlayerNormalIcon,
                defaultPlayerLosingIcon = defaultPlayerLosingIcon,
                defaultOpponentNormalIcon = defaultOpponentNormalIcon,
                defaultOpponentLosingIcon = defaultOpponentLosingIcon
            )
        }

        if (!hud.gameOver) {
            GameplayPauseButton(
                discreet = debugHideHud,
                onPause = { onSetPaused(true) }
            )
        }

        // Cortina de pantalla en negro (`screenBlackoutAlpha`) y destello sobre HUD:
        // cubre escenario, flechas y barra de vida, pero conserva visible el conteo "2, 1, ¡YA!" y Pausa.
        Canvas(modifier = Modifier.fillMaxSize()) {
            if (frameState.drawTick < 0L) return@Canvas
            if (frameState.vignetteOverHud) {
                drawCameraSideVignette(frameState)
            }
            val blackoutAlpha = frameState.screenBlackoutAlpha.coerceIn(0f, 1f)
            if (blackoutAlpha > 0.002f) {
                drawRect(
                    color = Color.Black.copy(alpha = blackoutAlpha),
                    topLeft = Offset.Zero,
                    size = size
                )
            }
            if (frameState.flashOverHud && frameState.flashAlpha > 0.002f) {
                drawRect(
                    color = Color(frameState.flashColorArgb).copy(alpha = frameState.flashAlpha.coerceIn(0f, 1f)),
                    topLeft = Offset.Zero,
                    size = size
                )
            }
        }

        GameplayDebugOverlay(
            showRam = debugShowRam,
            showTelemetry = debugShowTelemetry,
            fps = liveFps,
            frameMs = liveFrameMs,
            compiledBitmapsCount = compiledBitmapsByIndex.count { it != null },
            renderBackendLabel = if (useGlesSurface) activeGlLabel else if (forceCanvasFallback) "Canvas 2D (Fallback)" else renderBackendLabel,
            frameState = frameState,
            hud = hud
        )

        if (!debugHideHud) {
            GameplayScriptBanner(hud = hud)
        }

        GameplayCountdownOverlay(
            hud = hud,
            countdownSlots = countdownSlots,
            countdownBitmaps = countdownBitmaps
        )

        GameplayGameOverOverlay(
            hud = hud,
            onConfirmRetry = onConfirmGameOverRetry,
            onExitToStudio = onExitToStudio
        )

        GameplayPauseDialog(
            hud = hud,
            debugShowRam = debugShowRam,
            debugShowTelemetry = debugShowTelemetry,
            debugBotplay = debugBotplay,
            debugPracticeMode = debugPracticeMode,
            debugHideHud = debugHideHud,
            onChangeControlMode = onChangeControlMode,
            onToggleDebugRam = { onUpdateDebug(it, debugShowTelemetry, debugBotplay, debugPracticeMode, debugHideHud) },
            onToggleDebugTelemetry = { onUpdateDebug(debugShowRam, it, debugBotplay, debugPracticeMode, debugHideHud) },
            onToggleDebugBotplay = { onUpdateDebug(debugShowRam, debugShowTelemetry, it, debugPracticeMode, debugHideHud) },
            onToggleDebugPractice = { onUpdateDebug(debugShowRam, debugShowTelemetry, debugBotplay, it, debugHideHud) },
            onToggleDebugHideHud = { onUpdateDebug(debugShowRam, debugShowTelemetry, debugBotplay, debugPracticeMode, it) },
            onTriggerGameOver = onTriggerGameOver,
            onResume = { onSetPaused(false) },
            onRestart = onRestart,
            onExitToStudio = onExitToStudio
        )

        GameplayResultsDialog(
            hud = hud,
            onRestart = onRestart,
            onExitToStudio = onExitToStudio
        )
    }
}
