// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/GameplayScreen.kt
// Propósito: Coordinador modular de la pantalla jugable a 60 FPS en Compose Canvas
// sincronizada con el reloj del mezclador Rust AAudio (`nativeStepGameplayBinary`).
// Orquesta el bucle `withFrameNanos` y delega dibujo, entrada multi-touch y HUD/diálogos
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
import androidx.compose.runtime.remember
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.MobileControlMode
import com.beatforge.studio.ui.screens.gameplay.GameplayCountdownOverlay
import com.beatforge.studio.ui.screens.gameplay.GameplayHealthBarAndStats
import com.beatforge.studio.ui.screens.gameplay.GameplayPauseButton
import com.beatforge.studio.ui.screens.gameplay.GameplayPauseDialog
import com.beatforge.studio.ui.screens.gameplay.GameplayResultsDialog
import com.beatforge.studio.ui.screens.gameplay.GameplayScriptBanner
import com.beatforge.studio.ui.screens.gameplay.GameplayTouchMaskState
import com.beatforge.studio.ui.screens.gameplay.GameplayTouchOverlay
import com.beatforge.studio.ui.screens.gameplay.RatingPopupHolder
import com.beatforge.studio.ui.screens.gameplay.SplashPoolHolder
import com.beatforge.studio.ui.screens.gameplay.drawClassicHitboxColumns
import com.beatforge.studio.ui.screens.gameplay.drawGameplaySceneLayer
import com.beatforge.studio.ui.screens.gameplay.drawStrumsNotesAndSplashes
import com.beatforge.studio.ui.theme.StudioColors

@Composable
fun GameplayScreen(
    frameState: GameplayFrameState,
    arrowBitmaps: Map<Int, ImageBitmap>,
    receptorBitmaps: Map<Int, ImageBitmap>,
    healthBarBitmap: ImageBitmap?,
    ratingBitmaps: Map<Int, ImageBitmap>,
    noteSplashBitmap: ImageBitmap?,
    fallbackPlayerId: String?,
    fallbackOpponentId: String?,
    entityNormalIcons: Map<String, ImageBitmap>,
    entityLosingIcons: Map<String, ImageBitmap>,
    defaultPlayerNormalIcon: ImageBitmap?,
    defaultPlayerLosingIcon: ImageBitmap?,
    defaultOpponentNormalIcon: ImageBitmap?,
    defaultOpponentLosingIcon: ImageBitmap?,
    compiledBitmapsByIndex: List<LoadedEntityBitmap?>,
    onStepFrame: suspend (dtSec: Float, pressedMask: Int, justPressedMask: Int) -> Unit,
    onChangeControlMode: (MobileControlMode) -> Unit,
    onSetPaused: (Boolean) -> Unit,
    onRestart: () -> Unit,
    onExitToStudio: () -> Unit
) {
    val touchMaskState = remember { GameplayTouchMaskState() }
    val hud = frameState.hudState

    val ratingPopup = remember { RatingPopupHolder() }
    val splashPool = remember { SplashPoolHolder() }
    val dynamicTextPaint = remember {
        Paint(Paint.ANTI_ALIAS_FLAG).apply {
            typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
            setShadowLayer(4f, 2f, 2f, android.graphics.Color.BLACK)
        }
    }

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
                val justMask = touchMaskState.pendingJustPressedMask
                touchMaskState.pendingJustPressedMask = 0
                onStepFrame(dt, touchMaskState.pressedMask, justMask)
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
            val nowNs = System.nanoTime()

            if (frameState.ratingEventId != ratingPopup.lastEventId) {
                ratingPopup.lastEventId = frameState.ratingEventId
                if (frameState.lastRatingCode in 1..4) {
                    var plrX = 880f
                    var plrY = 430f
                    var plrW = 240f
                    var plrH = 300f
                    val activeIdx = frameState.activePlayerIdx
                    if (activeIdx in 0 until frameState.entityCount) {
                        val ent = frameState.entitiesPool[activeIdx]
                        plrX = ent.x
                        plrY = ent.y
                        plrW = ent.width * ent.scale
                        plrH = ent.height * ent.scale
                    } else {
                        for (i in 0 until frameState.entityCount) {
                            val ent = frameState.entitiesPool[i]
                            if (ent.roleCode == 2) {
                                plrX = ent.x
                                plrY = ent.y
                                plrW = ent.width * ent.scale
                                plrH = ent.height * ent.scale
                                break
                            }
                        }
                    }
                    ratingPopup.active = true
                    ratingPopup.ratingCode = frameState.lastRatingCode
                    ratingPopup.startNano = nowNs
                    ratingPopup.worldX = plrX - plrW * 0.56f
                    ratingPopup.worldY = plrY - plrH * 0.28f
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
                ratingPopup = ratingPopup,
                dynamicTextPaint = dynamicTextPaint
            )

            drawClassicHitboxColumns(
                frameState = frameState,
                pressedMask = touchMaskState.pressedMask
            )

            drawStrumsNotesAndSplashes(
                frameState = frameState,
                pressedMask = touchMaskState.pressedMask,
                nowNs = nowNs,
                sx = sx,
                sy = sy,
                arrowBitmaps = arrowBitmaps,
                receptorBitmaps = receptorBitmaps,
                noteSplashBitmap = noteSplashBitmap,
                splashPool = splashPool,
                dynamicTextPaint = dynamicTextPaint
            )
        }

        GameplayTouchOverlay(
            frameState = frameState,
            isClassicHitbox = isClassicHitbox,
            touchMaskState = touchMaskState
        )

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

        GameplayPauseButton(onPause = { onSetPaused(true) })

        GameplayScriptBanner(hud = hud)

        GameplayCountdownOverlay(hud = hud)

        // Capa de flash sobre HUD (debajo de los diálogos de Pausa/Resultados)
        Canvas(modifier = Modifier.fillMaxSize()) {
            if (frameState.drawTick < 0L) return@Canvas
            if (frameState.flashOverHud && frameState.flashAlpha > 0.002f) {
                drawRect(
                    color = Color(frameState.flashColorArgb).copy(alpha = frameState.flashAlpha.coerceIn(0f, 1f)),
                    topLeft = Offset.Zero,
                    size = size
                )
            }
        }

        GameplayPauseDialog(
            hud = hud,
            onChangeControlMode = onChangeControlMode,
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
