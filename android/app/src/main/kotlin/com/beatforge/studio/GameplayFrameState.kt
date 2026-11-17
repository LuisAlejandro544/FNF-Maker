// Archivo: android/app/src/main/kotlin/com/beatforge/studio/GameplayFrameState.kt
// Propósito: Estado de gameplay de cero asignaciones por frame decodificado desde un
// `DirectByteBuffer` Little-Endian llenado por Rust (`nativeStepGameplayBinary`).
// Separa el tick de dibujo (`drawTick` leído solo dentro de Compose `Canvas`) del estado
// del HUD (`hudState`), eliminando recomposiciones completas de pantalla a 60 FPS.

package com.beatforge.studio

import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.nio.ByteBuffer
import java.nio.ByteOrder

private const val BINARY_FRAME_MAGIC = 0x42464750 // "BFGP"
private const val BINARY_HEADER_SIZE = 112

@Immutable
data class GameplayHudState(
    val active: Boolean = false,
    val downscroll: Boolean = false,
    val paused: Boolean = false,
    val finished: Boolean = false,
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
    val health: Float = 0.5f,
    val lastJudgement: String = "",
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
    var visible: Boolean = true
    var flipX: Boolean = false
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
}

class MutableVisibleNote {
    var id: Long = 0L
    var timeMs: Double = 0.0
    var sustainMs: Double = 0.0
    var direction: Int = 0
    var isPlayer: Boolean = true
    var headHit: Boolean = false
}

class GameplayFrameState {
    var directBuffer: ByteBuffer = ByteBuffer
        .allocateDirect(128 * 1024)
        .order(ByteOrder.LITTLE_ENDIAN)
        private set

    private var strScratch = ByteArray(1024)

    // Tick leído exclusivamente dentro de `Canvas` y de la capa visual de hitbox para redibujar a 60 FPS sin recomponer el árbol Compose
    var drawTick by mutableLongStateOf(0L)
        private set

    // Estado ligero del HUD: solo se reasigna cuando cambian puntaje, combo, fallos, vida, pausa, fin o cuenta regresiva
    var hudState by mutableStateOf(GameplayHudState())
        private set

    // Datos de frame leídos únicamente en fase Draw / PointerInput
    var active: Boolean = false
    var downscroll: Boolean = false
    var paused: Boolean = false
    var finished: Boolean = false
    var controlMode: String = MobileControlMode.TapArrows.key
    var holdingLanesMask: Int = 0
    var opponentHoldingLanesMask: Int = 0
    var songPosMs: Double = 0.0
    var durationMs: Double = 0.0
    var scrollSpeed: Double = 1.8
    var activeBpm: Double = 120.0
    var cameraZoom: Float = 1f
    var cameraShake: Float = 0f
    var cameraX: Float = 640f
    var cameraY: Float = 360f
    var virtualWidth: Float = 1280f
    var virtualHeight: Float = 720f

    var entityCount: Int = 0
    val entitiesPool = ArrayList<MutableGameplayEntity>(16)

    var visibleNoteCount: Int = 0
    val notesPool = ArrayList<MutableVisibleNote>(256)

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
        entityCount = 0
        visibleNoteCount = 0
        holdingLanesMask = 0
        opponentHoldingLanesMask = 0
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
        controlMode = step.controlMode
        holdingLanesMask = step.holdingLanesMask
        opponentHoldingLanesMask = step.opponentHoldingLanesMask
        songPosMs = step.songPosMs
        durationMs = step.durationMs
        scrollSpeed = step.scrollSpeed
        activeBpm = step.activeBpm
        cameraZoom = step.cameraZoom
        cameraShake = step.cameraShake
        cameraX = step.cameraX
        cameraY = step.cameraY
        virtualWidth = step.virtualWidth
        virtualHeight = step.virtualHeight

        entityCount = step.entities.size
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
            dst.visible = src.visible
            dst.flipX = src.flipX
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
            } else {
                dst.hasFrame = false
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
            health = step.health,
            lastJudgement = cachedJudgement,
            scriptError = cachedScriptError,
            scriptLog = cachedScriptLog
        )
        drawTick += 1L
    }

    /**
     * Decodifica el paquete binario escrito por Rust en `directBuffer` reutilizando los pools
     * sin crear objetos por frame. Debe invocarse en el hilo principal al finalizar JNI.
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
        val entCount = buf.short.toInt() and 0xFFFF
        val noteCount = buf.short.toInt() and 0xFFFF

        active = (flags and 1) != 0
        downscroll = (flags and (1 shl 1)) != 0
        paused = (flags and (1 shl 2)) != 0
        finished = (flags and (1 shl 3)) != 0
        val classicMode = (flags and (1 shl 4)) != 0
        val hasHudStrings = (flags and (1 shl 5)) != 0
        controlMode = if (classicMode) {
            MobileControlMode.ClassicHitbox.key
        } else {
            MobileControlMode.TapArrows.key
        }

        holdingLanesMask = masks and 0xFF
        opponentHoldingLanesMask = (masks ushr 8) and 0xFF

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
            val ef = buf.int
            dst.visible = (ef and 1) != 0
            dst.flipX = (ef and (1 shl 1)) != 0
            dst.hasFrame = (ef and (1 shl 2)) != 0
            dst.frameRotated = (ef and (1 shl 3)) != 0
            dst.roleCode = (ef ushr 4) and 0x3
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
        val countdownAnimating = cdStage in 0..3 && !paused && !finished

        val hudNeedsUpdate = hasHudStrings ||
            prev.active != active ||
            prev.downscroll != downscroll ||
            prev.paused != paused ||
            prev.finished != finished ||
            prev.controlMode != controlMode ||
            prev.countdownStage != cdStage ||
            prev.score != scoreVal ||
            prev.combo != comboVal ||
            prev.misses != missesVal ||
            prev.hits != hitsVal ||
            prevHealth != quantizedHealth ||
            countdownAnimating

        if (hudNeedsUpdate) {
            hudState = GameplayHudState(
                active = active,
                downscroll = downscroll,
                paused = paused,
                finished = finished,
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
                health = healthVal,
                lastJudgement = cachedJudgement,
                scriptError = cachedScriptError,
                scriptLog = cachedScriptLog
            )
        }

        drawTick += 1L
    }

    private fun readUtf16LenStr(buf: ByteBuffer): String {
        if (buf.remaining() < 2) return ""
        val len = buf.short.toInt() and 0xFFFF
        if (len == 0 || buf.remaining() < len) return ""
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
}
