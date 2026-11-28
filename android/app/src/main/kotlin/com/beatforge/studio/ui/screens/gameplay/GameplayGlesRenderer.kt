// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/gameplay/GameplayGlesRenderer.kt
// Propósito: Renderizador GPU activo para Gameplay sobre OpenGL ES 3.0 por defecto con
// fallback automático a OpenGL ES 2.0. Replica a 60 FPS todo el pipeline visual del estudio:
// mundo con cámara/shake/zoom, entidades Sparrow/Animate y Game Over, popups de calificación,
// viñeta lateral reactiva al beat, receptores con `strumOverrides`, notas/sustain/notas X,
// Note Splashes y objetos dinámicos de scripts sin asignar objetos por frame en `GLThread`.

package com.beatforge.studio.ui.screens.gameplay

import android.opengl.GLES20
import android.opengl.GLSurfaceView
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.MobileControlMode
import com.beatforge.studio.MutableDynamicDrawable
import com.beatforge.studio.MutableGameplayEntity
import com.beatforge.studio.MutableStrumOverride
import com.beatforge.studio.MutableVisibleNote
import com.beatforge.studio.gfx.Gles2dBatch
import com.beatforge.studio.gfx.GlesTextureCache
import com.beatforge.studio.gfx.StudioGlesConfigChooser
import com.beatforge.studio.gfx.StudioGlesContextFactory
import com.beatforge.studio.gfx.parseGlMajorVersion
import com.beatforge.studio.ui.theme.StudioColors
import javax.microedition.khronos.egl.EGLConfig
import javax.microedition.khronos.opengles.GL10
import kotlin.math.cos
import kotlin.math.min
import kotlin.math.roundToInt
import kotlin.math.sin

/**
 * Copia ligera y reutilizable del estado de frame tomada bajo `frameState.renderLock`
 * para dibujar en `GLThread` sin bloqueos largos ni asignaciones por frame.
 */
internal class GlRenderSnapshot {
    var drawTick: Long = -1L
    var downscroll: Boolean = false
    var gameOver: Boolean = false
    var gameOverPhase: Int = 0
    var controlMode: String = MobileControlMode.TapArrows.key
    var holdingLanesMask: Int = 0
    var opponentHoldingLanesMask: Int = 0
    var songPosMs: Double = 0.0
    var scrollSpeed: Double = 1.8
    var beatPhase: Float = 0f
    var cameraZoom: Float = 1f
    var cameraShake: Float = 0f
    var cameraX: Float = 640f
    var cameraY: Float = 360f
    var virtualWidth: Float = 1280f
    var virtualHeight: Float = 720f
    var hudVisible: Boolean = true
    var hudOpacity: Float = 1f
    var arrowsVisible: Boolean = true
    var arrowsOpacity: Float = 1f
    var flashColorArgb: Int = -1
    var flashAlpha: Float = 0f
    var flashOverHud: Boolean = false
    var vignetteColorArgb: Int = 0xFFEF4444.toInt()
    var vignetteAlpha: Float = 0f
    var vignetteWidth: Float = 0.24f
    var vignettePulse: Float = 0.35f
    var vignetteOverHud: Boolean = false
    var activePlayerIdx: Int = -1
    var activeOpponentIdx: Int = -1
    var lastRatingCode: Int = 0
    var ratingEventId: Int = 0
    var splashEventId: Int = 0
    var splashLane: Int = 0

    var entityCount: Int = 0
    val entities = ArrayList<MutableGameplayEntity>(16)

    var visibleNoteCount: Int = 0
    val notes = ArrayList<MutableVisibleNote>(256)

    val strumOverrides = Array(8) { MutableStrumOverride() }

    var dynamicCount: Int = 0
    val dynamics = ArrayList<MutableDynamicDrawable>(16)

    fun copyFrom(src: GameplayFrameState) {
        synchronized(src.renderLock) {
            drawTick = src.drawTick
            if (drawTick < 0L) return
            downscroll = src.downscroll
            gameOver = src.gameOver
            gameOverPhase = src.gameOverPhase
            controlMode = src.controlMode
            holdingLanesMask = src.holdingLanesMask
            opponentHoldingLanesMask = src.opponentHoldingLanesMask
            songPosMs = src.songPosMs
            scrollSpeed = src.scrollSpeed
            beatPhase = src.beatPhase
            cameraZoom = src.cameraZoom
            cameraShake = src.cameraShake
            cameraX = src.cameraX
            cameraY = src.cameraY
            virtualWidth = src.virtualWidth
            virtualHeight = src.virtualHeight
            hudVisible = src.hudVisible
            hudOpacity = src.hudOpacity
            arrowsVisible = src.arrowsVisible
            arrowsOpacity = src.arrowsOpacity
            flashColorArgb = src.flashColorArgb
            flashAlpha = src.flashAlpha
            flashOverHud = src.flashOverHud
            vignetteColorArgb = src.vignetteColorArgb
            vignetteAlpha = src.vignetteAlpha
            vignetteWidth = src.vignetteWidth
            vignettePulse = src.vignettePulse
            vignetteOverHud = src.vignetteOverHud
            activePlayerIdx = src.activePlayerIdx
            activeOpponentIdx = src.activeOpponentIdx
            lastRatingCode = src.lastRatingCode
            ratingEventId = src.ratingEventId
            splashEventId = src.splashEventId
            splashLane = src.splashLane

            entityCount = src.entityCount
            while (entities.size < entityCount) entities.add(MutableGameplayEntity())
            for (i in 0 until entityCount) {
                val s = src.entitiesPool[i]
                val d = entities[i]
                d.x = s.x
                d.y = s.y
                d.width = s.width
                d.height = s.height
                d.scale = s.scale
                d.opacity = s.opacity
                d.rotationDeg = s.rotationDeg
                d.visible = s.visible
                d.flipX = s.flipX
                d.preloadRequested = s.preloadRequested
                d.retired = s.retired
                d.roleCode = s.roleCode
                d.hasFrame = s.hasFrame
                d.frameRotated = s.frameRotated
                d.atlasX = s.atlasX
                d.atlasY = s.atlasY
                d.atlasW = s.atlasW
                d.atlasH = s.atlasH
                d.offX = s.offX
                d.offY = s.offY
                d.logicalW = s.logicalW
                d.logicalH = s.logicalH
                d.pageIndex = s.pageIndex
            }

            visibleNoteCount = src.visibleNoteCount
            while (notes.size < visibleNoteCount) notes.add(MutableVisibleNote())
            for (i in 0 until visibleNoteCount) {
                val s = src.notesPool[i]
                val d = notes[i]
                d.id = s.id
                d.timeMs = s.timeMs
                d.sustainMs = s.sustainMs
                d.direction = s.direction
                d.isPlayer = s.isPlayer
                d.headHit = s.headHit
                d.noteKindCode = s.noteKindCode
            }

            for (i in 0 until 8) {
                val s = src.strumOverrides[i]
                val d = strumOverrides[i]
                d.offsetX = s.offsetX
                d.offsetY = s.offsetY
                d.angle = s.angle
                d.scale = s.scale
                d.alpha = s.alpha
            }

            dynamicCount = src.dynamicCount
            while (dynamics.size < dynamicCount) dynamics.add(MutableDynamicDrawable())
            for (i in 0 until dynamicCount) {
                val s = src.dynamicPool[i]
                val d = dynamics[i]
                d.isText = s.isText
                d.isHud = s.isHud
                d.visible = s.visible
                d.x = s.x
                d.y = s.y
                d.width = s.width
                d.height = s.height
                d.opacity = s.opacity
                d.angle = s.angle
                d.colorArgb = s.colorArgb
                d.text = s.text
            }
        }
    }
}

internal class GameplayGlesRenderer(
    private val frameState: GameplayFrameState,
    private val touchMaskState: GameplayTouchMaskState,
    private val ratingPool: RatingPopupPoolHolder,
    private val splashPool: SplashPoolHolder,
    private val onGlContextReady: (resolvedLabel: String) -> Unit,
    private val onGlFatalFallback: (reason: String) -> Unit
) : GLSurfaceView.Renderer {

    @Volatile var compiledBitmapsByIndex: List<LoadedEntityBitmap?> = emptyList()
    @Volatile var arrowBitmaps: Map<Int, ImageBitmap> = emptyMap()
    @Volatile var receptorBitmaps: Map<Int, ImageBitmap> = emptyMap()
    @Volatile var hurtArrowXBitmap: ImageBitmap? = null
    @Volatile var ratingBitmaps: Map<Int, ImageBitmap> = emptyMap()
    @Volatile var noteSplashBitmap: ImageBitmap? = null
    @Volatile var lanePalettes: List<ArrowLanePalette> = emptyList()
    @Volatile var densityPx: Float = 2f
    @Volatile var eglRequestedMajor: Int = 3
    @Volatile var eglFellBackToEs2: Boolean = false

    private val snap = GlRenderSnapshot()
    private val batch = Gles2dBatch()
    private val texCache = GlesTextureCache()

    private var viewW: Float = 1280f
    private var viewH: Float = 720f
    private var glInitializedOk: Boolean = false

    override fun onSurfaceCreated(gl: GL10?, config: EGLConfig?) {
        GLES20.glClearColor(
            StudioColors.BgDark.red,
            StudioColors.BgDark.green,
            StudioColors.BgDark.blue,
            1.0f
        )
        GLES20.glDisable(GLES20.GL_DEPTH_TEST)
        GLES20.glDisable(GLES20.GL_CULL_FACE)
        GLES20.glEnable(GLES20.GL_BLEND)
        GLES20.glBlendFunc(GLES20.GL_ONE, GLES20.GL_ONE_MINUS_SRC_ALPHA)

        val versionStr = GLES20.glGetString(GLES20.GL_VERSION).orEmpty()
        val parsedMajor = parseGlMajorVersion(versionStr)
        val effectiveMajor = minOf(parsedMajor, eglRequestedMajor.coerceAtLeast(2))

        texCache.releaseAll()
        glInitializedOk = batch.initGl(effectiveMajor)
        if (!glInitializedOk) {
            onGlFatalFallback("Error compilando shaders OpenGL ES")
            return
        }

        val resolvedLabel = when {
            effectiveMajor >= 3 && batch.usingEs3Shader -> "GLES 3.0"
            eglFellBackToEs2 -> "GLES 2.0 (Fallback)"
            else -> "GLES 2.0"
        }
        onGlContextReady(resolvedLabel)
    }

    override fun onSurfaceChanged(gl: GL10?, width: Int, height: Int) {
        val safeW = width.coerceAtLeast(1)
        val safeH = height.coerceAtLeast(1)
        viewW = safeW.toFloat()
        viewH = safeH.toFloat()
        GLES20.glViewport(0, 0, safeW, safeH)
        batch.setViewport(safeW, safeH)
    }

    override fun onDrawFrame(gl: GL10?) {
        GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT)
        if (!glInitializedOk || viewW <= 1f || viewH <= 1f) return

        snap.copyFrom(frameState)
        if (snap.drawTick < 0L) return

        texCache.beginFrame()
        batch.beginFrame()

        val nowNs = System.nanoTime()
        if (snap.ratingEventId == 0 && ratingPool.lastEventId != 0) {
            ratingPool.lastEventId = 0
            ratingPool.clear()
        } else if (snap.ratingEventId != ratingPool.lastEventId) {
            ratingPool.lastEventId = snap.ratingEventId
            if (snap.lastRatingCode in 1..4) {
                val slot = ratingPool.slots[ratingPool.cursor % ratingPool.slots.size]
                ratingPool.cursor += 1
                slot.active = true
                slot.ratingCode = snap.lastRatingCode
                slot.startNano = nowNs
                slot.jitterX = (((ratingPool.cursor * 19) % 28) - 14).toFloat()
                slot.jitterY = (((ratingPool.cursor * 23) % 24) - 12).toFloat()
            }
        }

        if (snap.splashEventId != splashPool.lastEventId) {
            splashPool.lastEventId = snap.splashEventId
            val slot = splashPool.slots[splashPool.cursor and 7]
            splashPool.cursor += 1
            slot.active = true
            slot.lane = snap.splashLane.coerceIn(0, 3)
            slot.startNano = nowNs
            slot.angleOffset = ((splashPool.cursor * 37) % 60 - 30).toFloat()
        }

        val vw = snap.virtualWidth.coerceAtLeast(1f)
        val vh = snap.virtualHeight.coerceAtLeast(1f)
        val sx = viewW / vw
        val sy = viewH / vh

        drawSceneLayer(nowNs, sx, sy)
        if (!snap.vignetteOverHud) {
            drawCameraSideVignette()
        }
        val pressedMask = touchMaskState.pressedMask
        drawClassicHitboxColumns(pressedMask)
        drawStrumsNotesAndSplashes(pressedMask, nowNs, sx, sy)

        batch.endFrame()
    }

    fun releaseGl() {
        glInitializedOk = false
        texCache.releaseAll()
        batch.releaseGl()
    }

    private fun drawSceneLayer(nowNs: Long, sx: Float, sy: Float) {
        val shakePhase = (snap.songPosMs * 0.08).toFloat()
        val shakeX = sin(shakePhase) * snap.cameraShake * sx
        val shakeY = cos(shakePhase * 1.3f) * snap.cameraShake * sy
        val pivotX = viewW * 0.5f
        val pivotY = viewH * 0.5f
        val camTransX = pivotX - snap.cameraX * sx
        val camTransY = pivotY - snap.cameraY * sy

        batch.pushMatrix()
        batch.translate(shakeX, shakeY)
        batch.scale(snap.cameraZoom, snap.cameraZoom, pivotX, pivotY)
        batch.translate(camTransX, camTransY)

        val entCount = snap.entityCount
        val inGameOver = snap.gameOver
        val playerIdx = snap.activePlayerIdx
        val passLimit = if (inGameOver) 2 else 1
        val curBitmaps = compiledBitmapsByIndex

        for (pass in 0 until passLimit) {
            if (pass == 1) {
                batch.drawSolidRect(-2000f, -2000f, 6000f, 6000f, 0xFF000000.toInt(), alphaScale = 0.88f)
            }
            for (i in 0 until entCount) {
                val ent = snap.entities[i]
                if (ent.retired || !ent.visible || ent.opacity <= 0.01f) continue
                val isPlayerEnt = (i == playerIdx) || (playerIdx < 0 && ent.roleCode == 2)
                if (inGameOver) {
                    if (pass == 0 && isPlayerEnt) continue
                    if (pass == 1 && !isPlayerEnt) continue
                }

                val cx = ent.x * sx
                val cy = ent.y * sy
                val w = ent.width * ent.scale * sx
                val h = ent.height * ent.scale * sy
                val left = cx - w * 0.5f
                val top = cy - h * 0.5f

                val rawLoaded = curBitmaps.getOrNull(i)?.pageAt(ent.pageIndex)
                val cachedTex = texCache.getOrUploadBitmap(rawLoaded?.bitmap)

                val deathTiltDeg = if (inGameOver && isPlayerEnt && !ent.hasFrame && snap.gameOverPhase >= 2) {
                    -14f
                } else {
                    0f
                }
                if (inGameOver && isPlayerEnt) {
                    val ovalColor = if (snap.gameOverPhase == 3) 0xFF10B981.toInt() else 0xFF3B82F6.toInt()
                    val ovalAlpha = if (snap.gameOverPhase == 3) 0.25f else 0.22f
                    batch.drawOvalFill(cx - w * 0.42f, top + h * 0.80f, w * 0.84f, h * 0.16f, ovalColor, ovalAlpha)
                }

                val applyDeathTint = inGameOver && isPlayerEnt && !ent.hasFrame
                val tintR = if (applyDeathTint) 0x93 / 255f else 1f
                val tintG = if (applyDeathTint) 0xC5 / 255f else 1f
                val tintB = if (applyDeathTint) 0xFD / 255f else 1f

                batch.pushMatrix()
                val totalRot = ent.rotationDeg + deathTiltDeg
                if (totalRot != 0f) {
                    batch.rotateDeg(totalRot, cx, cy)
                }
                if (ent.flipX) {
                    batch.scale(-1f, 1f, cx, cy)
                }

                if (cachedTex != null && rawLoaded != null) {
                    val scaleRatioX = cachedTex.width.toFloat() / rawLoaded.origWidth.coerceAtLeast(1).toFloat()
                    val scaleRatioY = cachedTex.height.toFloat() / rawLoaded.origHeight.coerceAtLeast(1).toFloat()

                    if (ent.hasFrame && ent.atlasW > 0 && ent.atlasH > 0) {
                        val srcX = (ent.atlasX * scaleRatioX).roundToInt().coerceIn(0, cachedTex.width - 1)
                        val srcY = (ent.atlasY * scaleRatioY).roundToInt().coerceIn(0, cachedTex.height - 1)
                        val srcW = (ent.atlasW * scaleRatioX).roundToInt().coerceIn(1, cachedTex.width - srcX)
                        val srcH = (ent.atlasH * scaleRatioY).roundToInt().coerceIn(1, cachedTex.height - srcY)

                        val visW = if (ent.frameRotated) ent.atlasH else ent.atlasW
                        val visH = if (ent.frameRotated) ent.atlasW else ent.atlasH
                        val logicalW = ent.logicalW.coerceAtLeast(visW).toFloat().coerceAtLeast(1f)
                        val logicalH = ent.logicalH.coerceAtLeast(visH).toFloat().coerceAtLeast(1f)
                        val fit = min(w / logicalW, h / logicalH)

                        val boxW = logicalW * fit
                        val boxH = logicalH * fit
                        val boxLeft = cx - boxW * 0.5f
                        val boxTop = (top + h) - boxH

                        val visDrawW = visW * fit
                        val visDrawH = visH * fit
                        val visLeft = boxLeft + (-ent.offX * fit)
                        val visTop = boxTop + (-ent.offY * fit)
                        val frameCenterX = visLeft + visDrawW * 0.5f
                        val frameCenterY = visTop + visDrawH * 0.5f

                        val dstW = (ent.atlasW * fit).coerceAtLeast(2f)
                        val dstH = (ent.atlasH * fit).coerceAtLeast(2f)
                        val dstLeft = frameCenterX - dstW * 0.5f
                        val dstTop = frameCenterY - dstH * 0.5f

                        val u0 = srcX.toFloat() / cachedTex.width.toFloat()
                        val v0 = srcY.toFloat() / cachedTex.height.toFloat()
                        val u1 = (srcX + srcW).toFloat() / cachedTex.width.toFloat()
                        val v1 = (srcY + srcH).toFloat() / cachedTex.height.toFloat()

                        batch.pushMatrix()
                        if (ent.frameRotated) {
                            batch.rotateDeg(-90f, frameCenterX, frameCenterY)
                        }
                        batch.drawTexturedQuad(
                            texId = cachedTex.texId,
                            left = dstLeft,
                            top = dstTop,
                            width = dstW,
                            height = dstH,
                            u0 = u0,
                            v0 = v0,
                            u1 = u1,
                            v1 = v1,
                            tintR = tintR,
                            tintG = tintG,
                            tintB = tintB,
                            alpha = ent.opacity.coerceIn(0f, 1f)
                        )
                        batch.popMatrix()
                    } else {
                        batch.drawTexturedQuad(
                            texId = cachedTex.texId,
                            left = left,
                            top = top,
                            width = w.coerceAtLeast(2f),
                            height = h.coerceAtLeast(2f),
                            tintR = tintR,
                            tintG = tintG,
                            tintB = tintB,
                            alpha = ent.opacity.coerceIn(0f, 1f)
                        )
                    }
                } else {
                    val boxArgb = when (ent.roleCode) {
                        0 -> 0xFF172033.toInt()
                        1 -> StudioColors.AccentCoral.copy(alpha = 0.28f).toArgb()
                        2 -> StudioColors.AccentCyan.copy(alpha = 0.28f).toArgb()
                        else -> StudioColors.AccentIndigo.copy(alpha = 0.25f).toArgb()
                    }
                    batch.drawSolidRect(left, top, w, h, boxArgb)
                    batch.drawRectStroke(left, top, w, h, 2f, StudioColors.BorderSubtle.toArgb(), ent.opacity.coerceIn(0f, 1f))
                }

                batch.popMatrix()
            }
        }

        if (!inGameOver) {
            for (i in 0 until snap.dynamicCount) {
                val item = snap.dynamics[i]
                if (!item.isHud && item.visible && item.opacity > 0.01f) {
                    drawDynamicItem(item, sx, sy, opacityScale = 1f)
                }
            }
        }

        var hasActiveRating = false
        for (rIdx in ratingPool.slots.indices) {
            if (ratingPool.slots[rIdx].active) {
                hasActiveRating = true
                break
            }
        }
        if (hasActiveRating && !inGameOver) {
            val anchor = ratingPool.livePlayerAnchor
            resolveSnapshotPlayerAnchor(anchor)
            val baseOffsetX = anchor.sideSign * (anchor.width * 0.32f).coerceIn(68f, 155f)
            val baseOffsetY = -(anchor.height * 0.22f).coerceIn(48f, 118f)

            for (rIdx in ratingPool.slots.indices) {
                val slot = ratingPool.slots[rIdx]
                if (!slot.active) continue
                val ageSec = ((nowNs - slot.startNano) / 1_000_000_000.0).toFloat()
                if (ageSec >= 0.68f) {
                    slot.active = false
                    continue
                }
                val popScale = if (ageSec < 0.08f) 1.28f - (ageSec / 0.08f) * 0.28f else 1f
                val alpha = if (ageSec > 0.38f) (1f - (ageSec - 0.38f) / 0.30f).coerceIn(0f, 1f) else 1f
                val worldX = anchor.x + baseOffsetX + slot.jitterX + anchor.sideSign * 18f * ageSec
                val worldY = anchor.y + baseOffsetY + slot.jitterY - 115f * ageSec + 260f * ageSec * ageSec
                val curX = worldX * sx
                val curY = worldY * sy

                val ratingTex = texCache.getOrUploadBitmap(ratingBitmaps[slot.ratingCode])
                if (ratingTex != null) {
                    val drawW = (190f * minOf(sx, sy) * popScale).coerceAtLeast(2f)
                    val drawH = (72f * minOf(sx, sy) * popScale).coerceAtLeast(2f)
                    batch.drawTexturedQuad(
                        texId = ratingTex.texId,
                        left = curX - drawW * 0.5f,
                        top = curY - drawH * 0.5f,
                        width = drawW,
                        height = drawH,
                        alpha = alpha
                    )
                } else {
                    val label = when (slot.ratingCode) {
                        1 -> "SICK!"
                        2 -> "GOOD!"
                        3 -> "BAD"
                        else -> "MISS"
                    }
                    val colorRgb = when (slot.ratingCode) {
                        1 -> 0x22D3EE
                        2 -> 0x34D399
                        3 -> 0xF59E0B
                        else -> 0xFB7185
                    }
                    val fontPx = (36f * minOf(sx, sy) * popScale).coerceAtLeast(14f)
                    val textTex = texCache.getOrCreateTextTexture(label, fontPx, colorRgb)
                    if (textTex != null) {
                        batch.drawTexturedQuad(
                            texId = textTex.texId,
                            left = curX - textTex.width * 0.5f,
                            top = curY - textTex.baselineYOffset,
                            width = textTex.width.toFloat(),
                            height = textTex.height.toFloat(),
                            alpha = alpha
                        )
                    }
                }
            }
        }

        batch.popMatrix()

        if (!snap.flashOverHud && snap.flashAlpha > 0.002f) {
            batch.drawSolidRect(0f, 0f, viewW, viewH, snap.flashColorArgb, alphaScale = snap.flashAlpha.coerceIn(0f, 1f))
        }
    }

    private fun drawCameraSideVignette() {
        val baseAlpha = snap.vignetteAlpha.coerceIn(0f, 1f)
        if (baseAlpha <= 0.004f || viewW <= 4f || viewH <= 4f) return

        val pulseAmount = snap.vignettePulse.coerceIn(0f, 1f)
        val beatCurve = (1f - snap.beatPhase.coerceIn(0f, 1f)).let { it * it }
        val shakeBoost = (snap.cameraShake / 12f).coerceIn(0f, 0.45f)

        val effAlpha = (baseAlpha * (1f + pulseAmount * (beatCurve * 0.42f + shakeBoost * 0.35f))).coerceIn(0f, 1f)
        val baseWidthFrac = snap.vignetteWidth.coerceIn(0.04f, 0.50f)
        val effEdgeW = (viewW * (baseWidthFrac * (1f + pulseAmount * beatCurve * 0.14f)).coerceIn(0.04f, 0.52f))
            .coerceAtLeast(2f)
        val topBandH = (viewH * 0.14f).coerceAtLeast(2f)
        val edgeArgb = snap.vignetteColorArgb

        // Izquierda
        batch.drawGradientRect(0f, 0f, effEdgeW, viewH, edgeArgb, effAlpha, effAlpha, 0f, 0f)
        // Derecha
        batch.drawGradientRect(viewW - effEdgeW, 0f, effEdgeW, viewH, edgeArgb, 0f, 0f, effAlpha, effAlpha)
        // Superior
        batch.drawGradientRect(0f, 0f, viewW, topBandH, edgeArgb, effAlpha * 0.28f, 0f, 0f, effAlpha * 0.28f)
        // Inferior
        batch.drawGradientRect(0f, viewH - topBandH, viewW, topBandH, edgeArgb, 0f, effAlpha * 0.28f, effAlpha * 0.28f, 0f)
    }

    private fun drawClassicHitboxColumns(pressedMask: Int) {
        if (snap.controlMode != MobileControlMode.ClassicHitbox.key) return
        val arrowsAlpha = if (snap.arrowsVisible) snap.arrowsOpacity.coerceIn(0f, 1f) else 0f
        if (arrowsAlpha <= 0.01f) return
        val colW = viewW / 4f
        val curPalettes = lanePalettes
        for (lane in 0..3) {
            val active = (pressedMask and (1 shl lane)) != 0 ||
                (snap.holdingLanesMask and (1 shl lane)) != 0
            val col = curPalettes.getOrNull(lane)?.bodyColor ?: StudioColors.LaneColors[lane]
            batch.drawSolidRect(
                left = lane * colW,
                top = 0f,
                width = colW,
                height = viewH,
                argb = col.toArgb(),
                alphaScale = (if (active) 0.22f else 0.03f) * arrowsAlpha
            )
            if (lane < 3) {
                val divX = (lane + 1) * colW
                batch.drawLine(
                    x0 = divX,
                    y0 = 0f,
                    x1 = divX,
                    y1 = viewH,
                    strokeWidth = 1f * densityPx,
                    argb = 0xFFFFFFFF.toInt(),
                    alphaScale = 0.14f * arrowsAlpha
                )
            }
        }
    }

    private fun drawStrumsNotesAndSplashes(pressedMask: Int, nowNs: Long, sx: Float, sy: Float) {
        if (snap.gameOver) return
        val classicMode = snap.controlMode == MobileControlMode.ClassicHitbox.key
        val useDownscrollTactileLayout = !classicMode

        val baseOppY = if (useDownscrollTactileLayout) 100f * sy else (if (snap.downscroll) 575f else 118f) * sy
        val basePlrY = if (useDownscrollTactileLayout) 595f * sy else (if (snap.downscroll) 575f else 118f) * sy
        val oppDownscroll = if (useDownscrollTactileLayout) false else snap.downscroll
        val plrDownscroll = if (useDownscrollTactileLayout) true else snap.downscroll

        val oppNoteSize = if (useDownscrollTactileLayout) 58f * minOf(sx, sy) else 84f * minOf(sx, sy)
        val plrNoteSize = if (useDownscrollTactileLayout) 98f * minOf(sx, sy) else 84f * minOf(sx, sy)
        val globalArrowsAlpha = if (snap.arrowsVisible) snap.arrowsOpacity.coerceIn(0f, 1f) else 0f

        val curArrows = arrowBitmaps
        val curReceptors = receptorBitmaps
        val curPalettes = lanePalettes

        if (globalArrowsAlpha > 0.01f) {
            for (dir in 0..3) {
                val oppOverride = snap.strumOverrides[dir]
                val plrOverride = snap.strumOverrides[4 + dir]

                val oppX = resolveOpponentLaneBaseX(dir, useDownscrollTactileLayout, sx) + oppOverride.offsetX * sx
                val oppY = baseOppY + oppOverride.offsetY * sy
                val plrX = resolvePlayerLaneBaseX(dir, useDownscrollTactileLayout, sx) + plrOverride.offsetX * sx
                val plrY = basePlrY + plrOverride.offsetY * sy

                val coloredBmp = curArrows[dir]
                val neutralBmp = curReceptors[dir] ?: coloredBmp
                val laneColor = curPalettes.getOrNull(dir)?.bodyColor ?: StudioColors.LaneColors[dir]

                val oppHolding = (snap.opponentHoldingLanesMask and (1 shl dir)) != 0
                val oppAlpha = ((if (oppHolding) 0.98f else 0.72f) * oppOverride.alpha * globalArrowsAlpha).coerceIn(0f, 1f)
                if (oppAlpha > 0.01f) {
                    drawReceptorQuad(
                        cx = oppX,
                        cy = oppY,
                        sizePx = oppNoteSize * (if (oppHolding) 1.06f else 0.94f) * oppOverride.scale,
                        angleDeg = oppOverride.angle,
                        colorArgb = (if (oppHolding) laneColor else Color(0xFF94A3B8)).toArgb(),
                        bitmap = if (oppHolding) coloredBmp else neutralBmp,
                        alpha = oppAlpha
                    )
                }

                val isPressed = (pressedMask and (1 shl dir)) != 0
                val isHolding = (snap.holdingLanesMask and (1 shl dir)) != 0
                val showColoredPlayer = isPressed || isHolding
                val playerScale = when {
                    isHolding -> 1.08f
                    isPressed -> 0.92f
                    else -> 1f
                } * plrOverride.scale
                val plrAlpha = (when {
                    isHolding -> 1f
                    isPressed -> 0.96f
                    else -> 0.86f
                } * plrOverride.alpha * globalArrowsAlpha).coerceIn(0f, 1f)

                if (plrAlpha > 0.01f) {
                    drawReceptorQuad(
                        cx = plrX,
                        cy = plrY,
                        sizePx = plrNoteSize * playerScale,
                        angleDeg = plrOverride.angle,
                        colorArgb = (if (showColoredPlayer) laneColor else Color(0xFF94A3B8)).toArgb(),
                        bitmap = if (showColoredPlayer) coloredBmp else neutralBmp,
                        alpha = plrAlpha
                    )
                }
            }

            val pxPerMs = 0.44f * snap.scrollSpeed.toFloat() * sy
            val noteCount = snap.visibleNoteCount
            val curSongPosMs = snap.songPosMs
            val hurtTex = texCache.getOrUploadBitmap(hurtArrowXBitmap)

            for (i in 0 until noteCount) {
                val note = snap.notes[i]
                val dir = note.direction.coerceIn(0, 3)
                val strumOv = snap.strumOverrides[if (note.isPlayer) 4 + dir else dir]
                val noteAlpha = (strumOv.alpha * globalArrowsAlpha).coerceIn(0f, 1f)
                if (noteAlpha <= 0.01f) continue

                val baseNoteX = if (note.isPlayer) {
                    resolvePlayerLaneBaseX(dir, useDownscrollTactileLayout, sx)
                } else {
                    resolveOpponentLaneBaseX(dir, useDownscrollTactileLayout, sx)
                }
                val noteX = baseNoteX + strumOv.offsetX * sx
                val receptorY = (if (note.isPlayer) basePlrY else baseOppY) + strumOv.offsetY * sy
                val laneDownscroll = if (note.isPlayer) plrDownscroll else oppDownscroll
                val notePxSize = (if (note.isPlayer) plrNoteSize else oppNoteSize) * strumOv.scale

                val headDeltaMs = (note.timeMs - curSongPosMs).toFloat()
                val tailEndDeltaMs = (note.timeMs + note.sustainMs - curSongPosMs).toFloat()
                val headY = if (laneDownscroll) receptorY - headDeltaMs * pxPerMs else receptorY + headDeltaMs * pxPerMs
                val tailEndY = if (laneDownscroll) receptorY - tailEndDeltaMs * pxPerMs else receptorY + tailEndDeltaMs * pxPerMs

                val anchorY = if (note.headHit) receptorY else headY
                val minY = minOf(anchorY, tailEndY)
                val maxY = maxOf(anchorY, tailEndY)
                if (maxY < -notePxSize || minY > viewH + notePxSize) continue

                val palette = curPalettes.getOrNull(dir)
                val bodyColor = (palette?.bodyColor ?: StudioColors.LaneColors[dir]).toArgb()
                val coreColor = (palette?.coreColor ?: Color.White).toArgb()

                if (note.sustainMs > 45.0) {
                    val tailTop = if (laneDownscroll) tailEndY else anchorY
                    val tailBottom = if (laneDownscroll) anchorY else tailEndY
                    val tailHeight = tailBottom - tailTop
                    if (tailHeight > 2f) {
                        val bodyWidth = notePxSize * 0.30f
                        val coreWidth = notePxSize * 0.11f
                        batch.drawRoundedVerticalPill(
                            left = noteX - bodyWidth * 0.5f,
                            top = tailTop,
                            width = bodyWidth,
                            height = tailHeight,
                            argb = bodyColor,
                            alphaScale = (if (note.headHit) 0.84f else 0.64f) * noteAlpha
                        )
                        batch.drawRoundedVerticalPill(
                            left = noteX - coreWidth * 0.5f,
                            top = tailTop,
                            width = coreWidth,
                            height = tailHeight,
                            argb = coreColor,
                            alphaScale = (if (note.headHit) 0.58f else 0.36f) * noteAlpha
                        )
                    }
                }

                if (!note.headHit) {
                    val isHazardX = note.noteKindCode == 1 || note.noteKindCode == 2
                    val isInstakill = note.noteKindCode == 2
                    val isDodge = note.noteKindCode == 3
                    val noteTex = if (isHazardX) hurtTex else texCache.getOrUploadBitmap(curArrows[dir])

                    batch.pushMatrix()
                    if (strumOv.angle != 0f) {
                        batch.rotateDeg(strumOv.angle, noteX, headY)
                    }

                    if (isDodge) {
                        batch.drawCircleRing(noteX, headY, notePxSize * 0.52f, StudioColors.AccentAmber.toArgb(), 0.72f * noteAlpha)
                    } else if (isInstakill) {
                        batch.drawCircleRing(noteX, headY, notePxSize * 0.54f, 0xFFFF1744.toInt(), 0.65f * noteAlpha)
                    }

                    if (noteTex != null) {
                        val half = notePxSize * 0.5f
                        val tintR = if (isInstakill) 0x99 / 255f else 1f
                        val tintG = if (isInstakill) 0x1B / 255f else 1f
                        val tintB = if (isInstakill) 0x1B / 255f else 1f
                        batch.drawTexturedQuad(
                            texId = noteTex.texId,
                            left = noteX - half,
                            top = headY - half,
                            width = notePxSize.coerceAtLeast(2f),
                            height = notePxSize.coerceAtLeast(2f),
                            tintR = tintR,
                            tintG = tintG,
                            tintB = tintB,
                            alpha = noteAlpha
                        )
                    } else if (isHazardX) {
                        val halfCross = notePxSize * 0.34f
                        val xColor = if (isInstakill) 0xFFFF1744.toInt() else StudioColors.AccentCoral.toArgb()
                        batch.drawCircleFill(noteX, headY, notePxSize * 0.43f, 0xFF1F070A.toInt(), 0.9f * noteAlpha)
                        batch.drawLine(noteX - halfCross, headY - halfCross, noteX + halfCross, headY + halfCross, 6f * densityPx, xColor, noteAlpha)
                        batch.drawLine(noteX + halfCross, headY - halfCross, noteX - halfCross, headY + halfCross, 6f * densityPx, xColor, noteAlpha)
                    } else {
                        batch.drawCircleFill(noteX, headY, notePxSize * 0.4f, bodyColor, noteAlpha)
                    }

                    batch.popMatrix()
                }
            }

            val splashTex = texCache.getOrUploadBitmap(noteSplashBitmap)
            for (sIdx in splashPool.slots.indices) {
                val slot = splashPool.slots[sIdx]
                if (!slot.active) continue
                val ageSec = ((nowNs - slot.startNano) / 1_000_000_000.0).toFloat()
                if (ageSec >= 0.28f) {
                    slot.active = false
                    continue
                }
                val lane = slot.lane.coerceIn(0, 3)
                val plrOv = snap.strumOverrides[4 + lane]
                val splashX = resolvePlayerLaneBaseX(lane, useDownscrollTactileLayout, sx) + plrOv.offsetX * sx
                val splashY = basePlrY + plrOv.offsetY * sy
                val progress = (ageSec / 0.28f).coerceIn(0f, 1f)
                val splashAlpha = ((1f - progress * progress) * globalArrowsAlpha).coerceIn(0f, 1f)
                val splashSize = plrNoteSize * (1.25f + progress * 0.85f) * plrOv.scale
                val laneColor = curPalettes.getOrNull(lane)?.splashColor ?: StudioColors.LaneColors[lane]

                if (splashTex != null) {
                    val halfSplash = splashSize * 0.5f
                    batch.pushMatrix()
                    batch.rotateDeg(slot.angleOffset + progress * 35f, splashX, splashY)
                    batch.drawTexturedQuad(
                        texId = splashTex.texId,
                        left = splashX - halfSplash,
                        top = splashY - halfSplash,
                        width = splashSize.coerceAtLeast(4f),
                        height = splashSize.coerceAtLeast(4f),
                        tintR = laneColor.red,
                        tintG = laneColor.green,
                        tintB = laneColor.blue,
                        alpha = splashAlpha * 0.94f
                    )
                    batch.popMatrix()
                } else {
                    batch.drawCircleRing(splashX, splashY, splashSize * 0.45f, laneColor.toArgb(), splashAlpha * 0.7f)
                }
            }
        }

        val globalHudOpacity = snap.hudOpacity.coerceIn(0f, 1f)
        if (snap.hudVisible && globalHudOpacity > 0.01f) {
            for (i in 0 until snap.dynamicCount) {
                val item = snap.dynamics[i]
                if (item.isHud && item.visible && item.opacity > 0.01f) {
                    drawDynamicItem(item, sx, sy, opacityScale = globalHudOpacity)
                }
            }
        }
    }

    private fun drawReceptorQuad(
        cx: Float,
        cy: Float,
        sizePx: Float,
        angleDeg: Float,
        colorArgb: Int,
        bitmap: ImageBitmap?,
        alpha: Float
    ) {
        val half = sizePx * 0.5f
        batch.pushMatrix()
        if (angleDeg != 0f) {
            batch.rotateDeg(angleDeg, cx, cy)
        }
        val tex = texCache.getOrUploadBitmap(bitmap)
        if (tex != null) {
            batch.drawTexturedQuad(
                texId = tex.texId,
                left = cx - half,
                top = cy - half,
                width = sizePx.coerceAtLeast(2f),
                height = sizePx.coerceAtLeast(2f),
                alpha = alpha
            )
        } else {
            batch.drawCircleRing(cx, cy, half * 0.8f, colorArgb, alpha)
        }
        batch.popMatrix()
    }

    private fun drawDynamicItem(
        item: MutableDynamicDrawable,
        sx: Float,
        sy: Float,
        opacityScale: Float
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
            val origAlpha = ((baseColor ushr 24) and 0xFF) / 255f
            val combinedAlpha = (origAlpha * alpha).coerceIn(0f, 1f)
            if (combinedAlpha <= 0.004f) return

            val textTex = texCache.getOrCreateTextTexture(item.text, scaledFontPx, baseColor) ?: return
            batch.pushMatrix()
            if (item.angle != 0f) {
                batch.rotateDeg(item.angle, px, py)
            }
            batch.drawTexturedQuad(
                texId = textTex.texId,
                left = px - 4f,
                top = py + scaledFontPx * 0.85f - textTex.baselineYOffset,
                width = textTex.width.toFloat(),
                height = textTex.height.toFloat(),
                alpha = combinedAlpha
            )
            batch.popMatrix()
        } else {
            batch.pushMatrix()
            if (item.angle != 0f) {
                batch.rotateDeg(item.angle, px + pw * 0.5f, py + ph * 0.5f)
            }
            batch.drawSolidRect(px, py, pw, ph, item.colorArgb, alphaScale = alpha)
            batch.popMatrix()
        }
    }

    private fun resolveSnapshotPlayerAnchor(out: LivePlayerAnchor) {
        val entCount = snap.entityCount
        val activePlayerIdx = snap.activePlayerIdx
        var playerEntIdx = -1

        if (activePlayerIdx in 0 until entCount && !snap.entities[activePlayerIdx].retired) {
            playerEntIdx = activePlayerIdx
        } else {
            for (i in 0 until entCount) {
                val ent = snap.entities[i]
                if (ent.roleCode == 2 && ent.visible && !ent.retired) {
                    playerEntIdx = i
                    break
                }
            }
            if (playerEntIdx < 0) {
                for (i in 0 until entCount) {
                    val ent = snap.entities[i]
                    if (ent.roleCode == 2 && !ent.retired) {
                        playerEntIdx = i
                        break
                    }
                }
            }
        }

        if (playerEntIdx >= 0) {
            val ent = snap.entities[playerEntIdx]
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

        val activeOppIdx = snap.activeOpponentIdx
        var oppEntIdx = -1
        if (activeOppIdx in 0 until entCount && !snap.entities[activeOppIdx].retired) {
            oppEntIdx = activeOppIdx
        } else {
            for (i in 0 until entCount) {
                val ent = snap.entities[i]
                if (ent.roleCode == 1 && ent.visible && !ent.retired) {
                    oppEntIdx = i
                    break
                }
            }
            if (oppEntIdx < 0) {
                for (i in 0 until entCount) {
                    val ent = snap.entities[i]
                    if (ent.roleCode == 1 && !ent.retired) {
                        oppEntIdx = i
                        break
                    }
                }
            }
        }

        out.sideSign = if (oppEntIdx >= 0) {
            val oppX = snap.entities[oppEntIdx].x
            if (out.x < oppX - 40f) 1f else -1f
        } else {
            if (out.x < snap.virtualWidth * 0.42f) 1f else -1f
        }
    }
}

@Composable
internal fun GameplayGlesSurface(
    frameState: GameplayFrameState,
    touchMaskState: GameplayTouchMaskState,
    preferEs3: Boolean,
    arrowBitmaps: Map<Int, ImageBitmap>,
    receptorBitmaps: Map<Int, ImageBitmap>,
    hurtArrowXBitmap: ImageBitmap?,
    ratingBitmaps: Map<Int, ImageBitmap>,
    noteSplashBitmap: ImageBitmap?,
    compiledBitmapsByIndex: List<LoadedEntityBitmap?>,
    ratingPool: RatingPopupPoolHolder,
    splashPool: SplashPoolHolder,
    lanePalettes: List<ArrowLanePalette>,
    onGlContextReady: (resolvedLabel: String) -> Unit,
    onGlFatalFallback: (reason: String) -> Unit,
    modifier: Modifier = Modifier
) {
    val lifecycleOwner = LocalLifecycleOwner.current
    var glViewRef by remember { mutableStateOf<GLSurfaceView?>(null) }

    val renderer = remember(frameState, touchMaskState, ratingPool, splashPool) {
        GameplayGlesRenderer(
            frameState = frameState,
            touchMaskState = touchMaskState,
            ratingPool = ratingPool,
            splashPool = splashPool,
            onGlContextReady = onGlContextReady,
            onGlFatalFallback = onGlFatalFallback
        )
    }

    SideEffect {
        val bitmapsChanged = renderer.compiledBitmapsByIndex !== compiledBitmapsByIndex
        renderer.compiledBitmapsByIndex = compiledBitmapsByIndex
        renderer.arrowBitmaps = arrowBitmaps
        renderer.receptorBitmaps = receptorBitmaps
        renderer.hurtArrowXBitmap = hurtArrowXBitmap
        renderer.ratingBitmaps = ratingBitmaps
        renderer.noteSplashBitmap = noteSplashBitmap
        renderer.lanePalettes = lanePalettes
        if (bitmapsChanged) {
            glViewRef?.requestRender()
        }
    }

    LaunchedEffect(glViewRef) {
        val view = glViewRef ?: return@LaunchedEffect
        snapshotFlow {
            (frameState.drawTick shl 6) xor touchMaskState.pressedMask.toLong()
        }.collect {
            view.requestRender()
        }
    }

    DisposableEffect(lifecycleOwner, glViewRef) {
        val view = glViewRef
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_RESUME -> view?.onResume()
                Lifecycle.Event.ON_PAUSE -> view?.onPause()
                else -> Unit
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose {
            lifecycleOwner.lifecycle.removeObserver(observer)
            view?.queueEvent {
                renderer.releaseGl()
            }
        }
    }

    AndroidView(
        modifier = modifier,
        factory = { context ->
            renderer.densityPx = context.resources.displayMetrics.density.coerceAtLeast(1f)
            try {
                GLSurfaceView(context).apply {
                    setEGLConfigChooser(StudioGlesConfigChooser())
                    setEGLContextFactory(
                        StudioGlesContextFactory(preferEs3 = preferEs3) { actualMajor, fellBack ->
                            renderer.eglRequestedMajor = actualMajor
                            renderer.eglFellBackToEs2 = fellBack
                        }
                    )
                    preserveEGLContextOnPause = true
                    setRenderer(renderer)
                    renderMode = GLSurfaceView.RENDERMODE_WHEN_DIRTY
                    glViewRef = this
                }
            } catch (err: Throwable) {
                onGlFatalFallback(err.message ?: "Error creando GLSurfaceView")
                GLSurfaceView(context)
            }
        },
        update = { view ->
            if (glViewRef !== view) {
                glViewRef = view
            }
        }
    )
}
