// Archivo: android/app/src/main/kotlin/com/beatforge/studio/GameplayFrameState.kt
// Propósito: Estado de gameplay de cero asignaciones por frame decodificado desde un
// `DirectByteBuffer` Little-Endian llenado por Rust (`nativeStepGameplayBinary`).
// Decodifica entidades con rotación, strums modchart (0..7), gráficos/textos dinámicos,
// eventos de calificación (`SICK!`, `GOOD!`, `BAD`, `MISS`), destello perfecto y precisión.

package com.beatforge.studio

import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.nio.ByteBuffer
import java.nio.ByteOrder

private const val BINARY_FRAME_MAGIC = 0x42464750 // "BFGP"
private const val BINARY_HEADER_SIZE = 152

@Immutable
data class GameplayHudState(
    val active: Boolean = false,
    val downscroll: Boolean = false,
    val paused: Boolean = false,
    val finished: Boolean = false,
    val gameOver: Boolean = false,
    val gameOverPhase: Int = 0,
    val botplay: Boolean = false,
    val practiceMode: Boolean = false,
    val controlMode: String = MobileControlMode.TapArrows.key,
    val activeBpm: Double = 120.0,
    val countdownStage: Int = -1,
    val countdownLabel: String = "",
    val countdownProgress: Float = 0f,
    val score: Long = 0L,
    val combo: Int = 0,
    val maxCombo: Int = 0,
    val hits: Int = 0,
    val misses: Int = 0,
    val accuracyPct: Float = 100f,
    val health: Float = 0.5f,
    val lastJudgement: String = "",
    val hudVisible: Boolean = true,
    val hudOpacity: Float = 1f,
    val hudZoom: Float = 1f,
    val arrowsVisible: Boolean = true,
    val arrowsOpacity: Float = 1f,
    val activePlayerIdx: Int = -1,
    val activeOpponentIdx: Int = -1,
    val scriptError: String = "",
    val scriptLog: String = ""
)

class MutableGameplayEntity {
    var x: Float = 0f
    var y: Float = 0f
    var width: Float = 200f
    var height: Float = 200f
    var scale: Float = 1f
    var opacity: Float = 1f
    var rotationDeg: Float = 0f
    var visible: Boolean = true
    var flipX: Boolean = false
    var preloadRequested: Boolean = false
    var retired: Boolean = false
    var roleCode: Int = 3 // 0=bg, 1=opponent, 2=player, 3=decoration
    var hasFrame: Boolean = false
    var frameRotated: Boolean = false
    var atlasX: Int = 0
    var atlasY: Int = 0
    var atlasW: Int = 0
    var atlasH: Int = 0
    var offX: Int = 0
    var offY: Int = 0
    var logicalW: Int = 0
    var logicalH: Int = 0
    var pageIndex: Int = 0
}

class MutableVisibleNote {
    var id: Long = 0L
    var timeMs: Double = 0.0
    var sustainMs: Double = 0.0
    var direction: Int = 0
    var isPlayer: Boolean = true
    var headHit: Boolean = false
    var noteKindCode: Int = 0
}

class MutableStrumOverride {
    var offsetX: Float = 0f
    var offsetY: Float = 0f
    var angle: Float = 0f
    var scale: Float = 1f
    var alpha: Float = 1f

    fun reset() {
        offsetX = 0f
        offsetY = 0f
        angle = 0f
        scale = 1f
        alpha = 1f
    }
}

class MutableDynamicDrawable {
    var isText: Boolean = false
    var isHud: Boolean = true
    var visible: Boolean = true
    var x: Float = 0f
    var y: Float = 0f
    var width: Float = 100f
    var height: Float = 100f
    var opacity: Float = 1f
    var angle: Float = 0f
    var colorArgb: Int = -1
    var text: String = ""
}

class GameplayFrameState {
    var directBuffer: ByteBuffer = ByteBuffer
        .allocateDirect(128 * 1024)
        .order(ByteOrder.LITTLE_ENDIAN)
        private set

    private var strScratch = ByteArray(1024)

    // Tick leído exclusivamente dentro de `Canvas` y la capa visual para redibujar a 60 FPS sin recomponer el HUD
    var drawTick by mutableLongStateOf(0L)
        private set

    // Estado ligero del HUD: solo se reasigna cuando cambian métricas, vida, pausa, fin o textos
    var hudState by mutableStateOf(GameplayHudState())
        private set

    // Datos de frame leídos únicamente en fase Draw / PointerInput
    var active: Boolean = false
    var downscroll: Boolean = false
    var paused: Boolean = false
    var finished: Boolean = false
    var gameOver: Boolean = false
    var gameOverPhase: Int = 0
    var botplay: Boolean = false
    var practiceMode: Boolean = false
    var controlMode: String = MobileControlMode.TapArrows.key
    var holdingLanesMask: Int = 0
    var opponentHoldingLanesMask: Int = 0
    var songPosMs: Double = 0.0
    var durationMs: Double = 0.0
    var scrollSpeed: Double = 1.8
    var activeBpm: Double = 120.0
    var beatPhase: Float = 0f
    var cameraZoom: Float = 1f
    var cameraShake: Float = 0f
    var cameraX: Float = 640f
    var cameraY: Float = 360f
    var virtualWidth: Float = 1280f
    var virtualHeight: Float = 720f
    var health: Float = 0.5f
    var hudVisible: Boolean = true
    var hudOpacity: Float = 1f
    var hudZoom: Float = 1f
    var arrowsVisible: Boolean = true
    var arrowsOpacity: Float = 1f
    var screenBlackoutAlpha: Float = 0f
    var flashColorArgb: Int = -1
    var flashAlpha: Float = 0f
    var flashOverHud: Boolean = false
    var activePlayerIdx: Int = -1
    var activeOpponentIdx: Int = -1
    var lastRatingCode: Int = 0
    var ratingEventId: Int = 0
    var splashEventId: Int = 0
    var splashLane: Int = 0

    var entityCount: Int = 0
    var entityIdsByIndex: List<String> = emptyList()
    val entitiesPool = ArrayList<MutableGameplayEntity>(16)

    var visibleNoteCount: Int = 0
    val notesPool = ArrayList<MutableVisibleNote>(256)

    val strumOverrides = Array(8) { MutableStrumOverride() }

    var dynamicCount: Int = 0
    val dynamicPool = ArrayList<MutableDynamicDrawable>(16)

    private var cachedJudgement: String = ""
    private var cachedCountdownLabel: String = ""
    private var cachedScriptError: String = ""
    private var cachedScriptLog: String = ""

    fun ensureDirectCapacity(requiredBytes: Int) {
        if (requiredBytes <= directBuffer.capacity()) return
        val newCap = maxOf(requiredBytes, directBuffer.capacity() * 2)
        directBuffer = ByteBuffer.allocateDirect(newCap).order(ByteOrder.LITTLE_ENDIAN)
    }

    fun reset() {
        active = false
        paused = false
        finished = false
        gameOver = false
        gameOverPhase = 0
        botplay = false
        practiceMode = false
        entityCount = 0
        entityIdsByIndex = emptyList()
        visibleNoteCount = 0
        dynamicCount = 0
        holdingLanesMask = 0
        opponentHoldingLanesMask = 0
        health = 0.5f
        beatPhase = 0f
        hudVisible = true
        hudOpacity = 1f
        hudZoom = 1f
        arrowsVisible = true
        arrowsOpacity = 1f
        screenBlackoutAlpha = 0f
        flashColorArgb = -1
        flashAlpha = 0f
        flashOverHud = false
        activePlayerIdx = -1
        activeOpponentIdx = -1
        lastRatingCode = 0
        ratingEventId = 0
        splashEventId = 0
        splashLane = 0
        for (s in strumOverrides) s.reset()
        cachedJudgement = ""
        cachedCountdownLabel = ""
        cachedScriptError = ""
        cachedScriptLog = ""
        hudState = GameplayHudState()
        drawTick += 1L
    }

    fun seedFromJsonStep(step: GameplayStep) {
        active = step.active
        downscroll = step.downscroll
        paused = step.paused
        finished = step.finished
        gameOver = step.gameOver
        gameOverPhase = step.gameOverPhase
        botplay = step.botplay
        practiceMode = step.practiceMode
        controlMode = step.controlMode
        holdingLanesMask = step.holdingLanesMask
        opponentHoldingLanesMask = step.opponentHoldingLanesMask
        songPosMs = step.songPosMs
        durationMs = step.durationMs
        scrollSpeed = step.scrollSpeed
        activeBpm = step.activeBpm
        beatPhase = step.beatPhase
        cameraZoom = step.cameraZoom
        cameraShake = step.cameraShake
        cameraX = step.cameraX
        cameraY = step.cameraY
        virtualWidth = step.virtualWidth
        virtualHeight = step.virtualHeight
        health = step.health
        hudVisible = step.hudVisible
        hudOpacity = step.hudOpacity
        hudZoom = step.hudZoom
        arrowsVisible = step.arrowsVisible
        arrowsOpacity = step.arrowsOpacity
        screenBlackoutAlpha = step.screenBlackoutAlpha
        flashColorArgb = step.flashColorArgb.toInt()
        flashAlpha = step.flashAlpha
        flashOverHud = step.flashOverHud
        activePlayerIdx = step.activePlayerIdx
        activeOpponentIdx = step.activeOpponentIdx
        lastRatingCode = step.ratingCode
        ratingEventId = step.ratingEventId
        splashEventId = step.splashEventId
        splashLane = step.splashLane

        entityCount = step.entities.size
        entityIdsByIndex = step.entities.map { it.id }
        ensureEntityPool(entityCount)
        for (i in 0 until entityCount) {
            val src = step.entities[i]
            val dst = entitiesPool[i]
            dst.x = src.x
            dst.y = src.y
            dst.width = src.width
            dst.height = src.height
            dst.scale = src.scale
            dst.opacity = src.opacity
            dst.rotationDeg = src.rotationDeg
            dst.visible = src.visible
            dst.flipX = src.flipX
            dst.preloadRequested = src.preloadRequested
            dst.retired = src.retired
            dst.roleCode = when (src.role) {
                "background" -> 0
                "opponent" -> 1
                "player" -> 2
                else -> 3
            }
            val f = src.frame
            if (f != null && f.found && f.width > 0 && f.height > 0) {
                dst.hasFrame = true
                dst.frameRotated = f.rotated
                dst.atlasX = f.x
                dst.atlasY = f.y
                dst.atlasW = f.width
                dst.atlasH = f.height
                dst.offX = f.frameX
                dst.offY = f.frameY
                dst.logicalW = f.frameWidth
                dst.logicalH = f.frameHeight
                dst.pageIndex = f.pageIndex.coerceAtLeast(0)
            } else {
                dst.hasFrame = false
                dst.pageIndex = 0
            }
        }

        visibleNoteCount = step.visibleNotes.size
        ensureNotePool(visibleNoteCount)
        for (i in 0 until visibleNoteCount) {
            val src = step.visibleNotes[i]
            val dst = notesPool[i]
            dst.id = src.id
            dst.timeMs = src.timeMs
            dst.sustainMs = src.sustainMs
            dst.direction = src.direction
            dst.isPlayer = src.isPlayer
            dst.headHit = src.headHit
            dst.noteKindCode = src.noteKindCode
        }

        for (i in 0 until 8) {
            val src = step.strumOverrides.getOrNull(i)
            val dst = strumOverrides[i]
            if (src != null) {
                dst.offsetX = src.offsetX
                dst.offsetY = src.offsetY
                dst.angle = src.angle
                dst.scale = src.scale
                dst.alpha = src.alpha
            } else {
                dst.reset()
            }
        }

        dynamicCount = step.dynamicItems.size
        ensureDynamicPool(dynamicCount)
        for (i in 0 until dynamicCount) {
            val src = step.dynamicItems[i]
            val dst = dynamicPool[i]
            dst.isText = src.kind == "text"
            dst.isHud = src.isHud
            dst.visible = src.visible
            dst.x = src.x
            dst.y = src.y
            dst.width = src.width
            dst.height = src.height
            dst.opacity = src.opacity
            dst.angle = src.angle
            dst.colorArgb = src.colorArgb.toInt()
            dst.text = src.text
        }

        cachedJudgement = step.lastJudgement
        cachedCountdownLabel = step.countdownLabel
        cachedScriptError = step.scriptErrors.firstOrNull().orEmpty()
        cachedScriptLog = step.scriptLogs.lastOrNull().orEmpty()

        hudState = GameplayHudState(
            active = step.active,
            downscroll = step.downscroll,
            paused = step.paused,
            finished = step.finished,
            gameOver = step.gameOver,
            gameOverPhase = step.gameOverPhase,
            botplay = step.botplay,
            practiceMode = step.practiceMode,
            controlMode = step.controlMode,
            activeBpm = step.activeBpm,
            countdownStage = step.countdownStage,
            countdownLabel = cachedCountdownLabel,
            countdownProgress = step.countdownProgress,
            score = step.score,
            combo = step.combo,
            maxCombo = step.maxCombo,
            hits = step.hits,
            misses = step.misses,
            accuracyPct = step.accuracyPct,
            health = step.health,
            lastJudgement = cachedJudgement,
            hudVisible = step.hudVisible,
            hudOpacity = step.hudOpacity,
            hudZoom = step.hudZoom,
            arrowsVisible = step.arrowsVisible,
            arrowsOpacity = step.arrowsOpacity,
            activePlayerIdx = step.activePlayerIdx,
            activeOpponentIdx = step.activeOpponentIdx,
            scriptError = cachedScriptError,
            scriptLog = cachedScriptLog
        )
        drawTick += 1L
    }

    /**
     * Decodifica el paquete binario `BFGP` v2 escrito por Rust en `directBuffer`
     * reutilizando los pools sin crear objetos por frame.
     */
    fun decodeFromDirectBuffer(byteCount: Int) {
        if (byteCount < BINARY_HEADER_SIZE) return
        val buf = directBuffer
        buf.position(0)
        buf.limit(byteCount)

        val magic = buf.int
        if (magic != BINARY_FRAME_MAGIC) return

        val flags = buf.int
        /* val hudRev = */ buf.int
        val masks = buf.int
        val cdStage = buf.int
        val cdProgress = buf.float
        songPosMs = buf.double
        durationMs = buf.double
        scrollSpeed = buf.double
        activeBpm = buf.double
        cameraZoom = buf.float
        cameraShake = buf.float
        cameraX = buf.float
        cameraY = buf.float
        virtualWidth = buf.float
        virtualHeight = buf.float
        val healthVal = buf.float
        val hitsVal = buf.int
        val scoreVal = buf.long
        val comboVal = buf.int
        val maxComboVal = buf.int
        val missesVal = buf.int
        val accuracyVal = buf.float
        val hudOpVal = buf.float
        val hudZoomVal = buf.float
        ratingEventId = buf.int
        splashEventId = buf.int
        beatPhase = buf.float
        flashColorArgb = buf.int
        flashAlpha = buf.float
        activePlayerIdx = buf.short.toInt()
        activeOpponentIdx = buf.short.toInt()
        val entCount = buf.short.toInt() and 0xFFFF
        val noteCount = buf.short.toInt() and 0xFFFF
        val strumCount = buf.short.toInt() and 0xFFFF
        val dynCount = buf.short.toInt() and 0xFFFF

        active = (flags and 1) != 0
        downscroll = (flags and (1 shl 1)) != 0
        paused = (flags and (1 shl 2)) != 0
        finished = (flags and (1 shl 3)) != 0
        val classicMode = (flags and (1 shl 4)) != 0
        val hasHudStrings = (flags and (1 shl 5)) != 0
        hudVisible = (flags and (1 shl 6)) != 0
        flashOverHud = (flags and (1 shl 7)) != 0
        arrowsVisible = (flags and (1 shl 8)) == 0
        gameOver = (flags and (1 shl 9)) != 0
        gameOverPhase = (flags ushr 10) and 0x3
        botplay = (flags and (1 shl 12)) != 0
        practiceMode = (flags and (1 shl 13)) != 0
        screenBlackoutAlpha = ((flags ushr 16) and 0xFF) / 255f
        arrowsOpacity = 1f - (((flags ushr 24) and 0xFF) / 255f)
        hudOpacity = hudOpVal
        hudZoom = hudZoomVal
        health = healthVal

        controlMode = if (classicMode) {
            MobileControlMode.ClassicHitbox.key
        } else {
            MobileControlMode.TapArrows.key
        }

        holdingLanesMask = masks and 0xFF
        opponentHoldingLanesMask = (masks ushr 8) and 0xFF
        lastRatingCode = (masks ushr 16) and 0xF
        splashLane = (masks ushr 20) and 0xF

        entityCount = entCount
        ensureEntityPool(entCount)
        for (i in 0 until entCount) {
            val dst = entitiesPool[i]
            dst.x = buf.float
            dst.y = buf.float
            dst.width = buf.float
            dst.height = buf.float
            dst.scale = buf.float
            dst.opacity = buf.float
            dst.rotationDeg = buf.float
            val ef = buf.int
            dst.visible = (ef and 1) != 0
            dst.flipX = (ef and (1 shl 1)) != 0
            dst.hasFrame = (ef and (1 shl 2)) != 0
            dst.frameRotated = (ef and (1 shl 3)) != 0
            dst.roleCode = (ef ushr 4) and 0x3
            dst.preloadRequested = (ef and (1 shl 6)) != 0
            dst.retired = (ef and (1 shl 7)) != 0
            dst.pageIndex = (ef ushr 8) and 0xFF
            dst.atlasX = buf.int
            dst.atlasY = buf.int
            dst.atlasW = buf.int
            dst.atlasH = buf.int
            dst.offX = buf.int
            dst.offY = buf.int
            dst.logicalW = buf.int
            dst.logicalH = buf.int
        }

        visibleNoteCount = noteCount
        ensureNotePool(noteCount)
        for (i in 0 until noteCount) {
            val dst = notesPool[i]
            dst.id = buf.long
            dst.timeMs = buf.double
            dst.sustainMs = buf.double
            val nf = buf.int
            dst.direction = nf and 0x3
            dst.isPlayer = (nf and (1 shl 2)) != 0
            dst.headHit = (nf and (1 shl 3)) != 0
            dst.noteKindCode = (nf ushr 4) and 0xF
        }

        for (i in 0 until 8) {
            if (i < strumCount && buf.remaining() >= 20) {
                val dst = strumOverrides[i]
                dst.offsetX = buf.float
                dst.offsetY = buf.float
                dst.angle = buf.float
                dst.scale = buf.float
                dst.alpha = buf.float
            } else {
                strumOverrides[i].reset()
            }
        }

        dynamicCount = dynCount
        ensureDynamicPool(dynCount)
        for (i in 0 until dynCount) {
            if (buf.remaining() < 34) {
                dynamicCount = i
                break
            }
            val dst = dynamicPool[i]
            dst.isText = buf.get().toInt() != 0
            dst.isHud = buf.get().toInt() != 0
            dst.visible = buf.get().toInt() != 0
            buf.get() // reserved
            dst.x = buf.float
            dst.y = buf.float
            dst.width = buf.float
            dst.height = buf.float
            dst.opacity = buf.float
            dst.angle = buf.float
            dst.colorArgb = buf.int
            dst.text = readUtf16LenStr(buf)
        }

        if (hasHudStrings && buf.remaining() >= 8) {
            cachedJudgement = readUtf16LenStr(buf)
            cachedCountdownLabel = readUtf16LenStr(buf)
            cachedScriptError = readUtf16LenStr(buf)
            cachedScriptLog = readUtf16LenStr(buf)
        }

        val prev = hudState
        val quantizedHealth = (healthVal * 200f).toInt()
        val prevHealth = (prev.health * 200f).toInt()
        val quantizedAcc = (accuracyVal * 10f).toInt()
        val prevAcc = (prev.accuracyPct * 10f).toInt()
        val quantizedHudOp = (hudOpVal * 50f).toInt()
        val prevHudOp = (prev.hudOpacity * 50f).toInt()
        val quantizedHudZoom = (hudZoomVal * 100f).toInt()
        val prevHudZoom = (prev.hudZoom * 100f).toInt()
        val quantizedArrowsOp = (arrowsOpacity * 50f).toInt()
        val prevArrowsOp = (prev.arrowsOpacity * 50f).toInt()
        val countdownAnimating = cdStage in 0..3 && !paused && !finished && !gameOver

        val hudNeedsUpdate = hasHudStrings ||
            prev.active != active ||
            prev.downscroll != downscroll ||
            prev.paused != paused ||
            prev.finished != finished ||
            prev.gameOver != gameOver ||
            prev.gameOverPhase != gameOverPhase ||
            prev.botplay != botplay ||
            prev.practiceMode != practiceMode ||
            prev.controlMode != controlMode ||
            prev.countdownStage != cdStage ||
            prev.score != scoreVal ||
            prev.combo != comboVal ||
            prev.misses != missesVal ||
            prev.hits != hitsVal ||
            prev.hudVisible != hudVisible ||
            prev.arrowsVisible != arrowsVisible ||
            prev.activePlayerIdx != activePlayerIdx ||
            prev.activeOpponentIdx != activeOpponentIdx ||
            prevHealth != quantizedHealth ||
            prevAcc != quantizedAcc ||
            prevHudOp != quantizedHudOp ||
            prevHudZoom != quantizedHudZoom ||
            prevArrowsOp != quantizedArrowsOp ||
            countdownAnimating

        if (hudNeedsUpdate) {
            hudState = GameplayHudState(
                active = active,
                downscroll = downscroll,
                paused = paused,
                finished = finished,
                gameOver = gameOver,
                gameOverPhase = gameOverPhase,
                botplay = botplay,
                practiceMode = practiceMode,
                controlMode = controlMode,
                activeBpm = activeBpm,
                countdownStage = cdStage,
                countdownLabel = cachedCountdownLabel,
                countdownProgress = if (cdStage in 0..3) cdProgress else 0f,
                score = scoreVal,
                combo = comboVal,
                maxCombo = maxComboVal,
                hits = hitsVal,
                misses = missesVal,
                accuracyPct = accuracyVal,
                health = healthVal,
                lastJudgement = cachedJudgement,
                hudVisible = hudVisible,
                hudOpacity = hudOpVal,
                hudZoom = hudZoomVal,
                arrowsVisible = arrowsVisible,
                arrowsOpacity = arrowsOpacity,
                activePlayerIdx = activePlayerIdx,
                activeOpponentIdx = activeOpponentIdx,
                scriptError = cachedScriptError,
                scriptLog = cachedScriptLog
            )
        }

        drawTick += 1L
    }

    private fun readUtf16LenStr(buf: ByteBuffer): String {
        if (buf.remaining() < 2) return ""
        val len = buf.short.toInt() and 0xFFFF
        if (len == 0) return ""
        if (buf.remaining() < len) return ""
        if (strScratch.size < len) {
            strScratch = ByteArray(maxOf(len, strScratch.size * 2))
        }
        buf.get(strScratch, 0, len)
        return String(strScratch, 0, len, Charsets.UTF_8)
    }

    private fun ensureEntityPool(count: Int) {
        while (entitiesPool.size < count) {
            entitiesPool.add(MutableGameplayEntity())
        }
    }

    private fun ensureNotePool(count: Int) {
        while (notesPool.size < count) {
            notesPool.add(MutableVisibleNote())
        }
    }

    private fun ensureDynamicPool(count: Int) {
        while (dynamicPool.size < count) {
            dynamicPool.add(MutableDynamicDrawable())
        }
    }
}
