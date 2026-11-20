// Archivo: android/app/src/main/kotlin/com/beatforge/studio/NativeEngineBridge.kt
// Propósito: Declaraciones JNI directas hacia `libpulse_android.so` (motor Rust puro).
// No contiene lógica de juego, audio ni fallbacks en Kotlin: todas las operaciones reales
// (audio AAudio en Rust, parseo de charts, XML Sparrow, scripting Lua/TS/JS, SVG resvg,
// compilación en caliente con Rayon y simulación de gameplay) se ejecutan en Rust.

package com.beatforge.studio

import java.nio.ByteBuffer

object NativeEngineBridge {

    @Volatile
    private var loaded: Boolean = false

    fun ensureLoaded() {
        if (!loaded) {
            synchronized(this) {
                if (!loaded) {
                    System.loadLibrary("pulse_android")
                    loaded = true
                }
            }
        }
    }

    @JvmStatic
    external fun nativeSetCacheQuota(quotaBytes: Long)

    @JvmStatic
    external fun nativeInit(cacheDir: String, dataDir: String): String

    @JvmStatic
    external fun nativeFlushSave()

    @JvmStatic
    external fun nativeListProjects(): String

    @JvmStatic
    external fun nativeCreateProject(name: String, author: String): String

    @JvmStatic
    external fun nativeOpenProject(projectId: String): String

    @JvmStatic
    external fun nativeDuplicateProject(projectId: String): String

    @JvmStatic
    external fun nativeDeleteProject(projectId: String): String

    @JvmStatic
    external fun nativeExportModPackage(
        projectId: String,
        outPath: String,
        optimizeImages: Boolean
    ): String

    @JvmStatic
    external fun nativeImportModPackage(path: String): String

    @JvmStatic
    external fun nativeGetSnapshot(): String

    @JvmStatic
    external fun nativeUpdateProjectMeta(
        name: String,
        author: String,
        downscroll: Boolean,
        controlMode: String,
        audioOffsetMs: Double,
        scrollSpeed: Double
    ): String

    @JvmStatic
    external fun nativeImportChart(path: String): String

    @JvmStatic
    external fun nativeRestoreDemoChart(): String

    @JvmStatic
    external fun nativeSetChartBpm(bpm: Double): String

    @JvmStatic
    external fun nativeExportChartJson(): String

    @JvmStatic
    external fun nativeUpdateChartDetails(
        title: String,
        speed: Double,
        player1: String,
        player2: String,
        gfVersion: String,
        stage: String,
        needsVoices: Boolean
    ): String

    @JvmStatic
    external fun nativeUpdateChartIntroUi(
        startBlackout: Boolean,
        blackoutAlpha: Float,
        hideHud: Boolean,
        hideArrows: Boolean,
        revealOnSongStart: Boolean,
        revealDelayMs: Double,
        fadeSec: Float
    ): String

    @JvmStatic
    external fun nativeSetSectionBpm(secIndex: Int, bpm: Double, changeBpm: Boolean): String

    @JvmStatic
    external fun nativeAddChartEvent(
        timeMs: Double,
        name: String,
        value1: String,
        value2: String
    ): String

    @JvmStatic
    external fun nativeRemoveChartEvent(index: Int): String

    @JvmStatic
    external fun nativeAddChartSection(mustHit: Boolean): String

    @JvmStatic
    external fun nativeToggleSectionMustHit(secIndex: Int): String

    @JvmStatic
    external fun nativeSetSectionCameraSlot(secIndex: Int, cameraSlot: Int): String

    @JvmStatic
    external fun nativeToggleChartNote(
        secIndex: Int,
        stepInSec: Int,
        col0To7: Int,
        sustainSteps: Int
    ): String

    @JvmStatic
    external fun nativeImportAudioTrack(slot: Int, path: String): String

    @JvmStatic
    external fun nativeRemoveAudioTrack(slot: Int): String

    @JvmStatic
    external fun nativeUpdateAudioTrackConfig(
        slot: Int,
        gain: Float,
        muted: Boolean,
        duckOnMiss: Boolean
    ): String

    @JvmStatic
    external fun nativeToggleAudioPreview(play: Boolean, startMs: Double): Boolean

    @JvmStatic
    external fun nativeSetAudioPreviewTransport(play: Boolean, seekMs: Double): Boolean

    @JvmStatic
    external fun nativeReadAudioPreviewState(outState: DoubleArray): Boolean

    @JvmStatic
    external fun nativeImportSpriteSheetPng(entityId: String, path: String): String

    @JvmStatic
    external fun nativeImportSparrowXml(entityId: String, path: String): String

    @JvmStatic
    external fun nativeSetSparrowBinding(
        entityId: String,
        animKind: String,
        prefix: String
    ): String

    @JvmStatic
    external fun nativeGetSparrowFrameInfo(
        entityId: String,
        animKind: String,
        elapsedSec: Float
    ): String

    @JvmStatic
    external fun nativeImportCharacterConfig(entityId: String, path: String): String

    @JvmStatic
    external fun nativeSetEntityAnimOffset(
        entityId: String,
        animKind: String,
        offsetX: Float,
        offsetY: Float
    ): String

    @JvmStatic
    external fun nativeImportStageConfig(path: String): String

    @JvmStatic
    external fun nativeAddStageEntity(name: String, role: String): String

    @JvmStatic
    external fun nativeUpdateStageEntity(
        entityId: String,
        name: String,
        role: String,
        dx: Float,
        dy: Float,
        absX: Float,
        absY: Float,
        useAbsPos: Boolean,
        scale: Float,
        opacity: Float,
        visible: Boolean,
        flipX: Boolean,
        commitSave: Boolean
    ): String

    @JvmStatic
    external fun nativeReorderStageEntity(entityId: String, action: String): String

    @JvmStatic
    external fun nativeUpdateStageCamera(
        enabled: Boolean,
        baseX: Float,
        baseY: Float,
        baseZoom: Float,
        followMode: String,
        followIntervalBeats: Int,
        smoothSpeed: Float,
        panStrength: Float,
        clampToBackground: Boolean,
        commitSave: Boolean
    ): String

    @JvmStatic
    external fun nativeSelectStageCamera(slotIndex: Int): String

    @JvmStatic
    external fun nativeUpdateCameraSlot(
        slotIndex: Int,
        name: String,
        targetKind: String,
        targetEntityId: String,
        x: Float,
        y: Float,
        zoom: Float,
        smoothSpeed: Float,
        panStrength: Float,
        commitSave: Boolean
    ): String

    @JvmStatic
    external fun nativeAddCustomCamera(
        name: String,
        targetKind: String,
        targetEntityId: String,
        x: Float,
        y: Float,
        zoom: Float,
        smoothSpeed: Float,
        panStrength: Float
    ): String

    @JvmStatic
    external fun nativeRemoveCustomCamera(slotIndex: Int): String

    @JvmStatic
    external fun nativeAddScript(language: String): String

    @JvmStatic
    external fun nativeImportScript(path: String): String

    @JvmStatic
    external fun nativeUpdateScript(
        scriptId: String,
        enabled: Boolean,
        sourceCode: String
    ): String

    @JvmStatic
    external fun nativeValidateScript(scriptId: String): String

    @JvmStatic
    external fun nativeImportArrowSvg(direction: Int, path: String): String

    @JvmStatic
    external fun nativeRestoreDefaultArrowSvg(): String

    @JvmStatic
    external fun nativeGetArrowPngBytes(direction: Int): ByteArray

    @JvmStatic
    external fun nativeGetReceptorPngBytes(direction: Int): ByteArray

    @JvmStatic
    external fun nativeImportCharacterIcon(
        entityId: String,
        isLosing: Boolean,
        path: String
    ): String

    @JvmStatic
    external fun nativeClearCharacterIcons(entityId: String): String

    @JvmStatic
    external fun nativeImportHealthBarSvg(path: String): String

    @JvmStatic
    external fun nativeRestoreDefaultHealthBarSvg(): String

    @JvmStatic
    external fun nativeGetHealthBarPngBytes(): ByteArray

    @JvmStatic
    external fun nativeGetRatingPngBytes(ratingCode: Int): ByteArray

    @JvmStatic
    external fun nativeGetDefaultIconPngBytes(isPlayer: Boolean, isLosing: Boolean): ByteArray

    @JvmStatic
    external fun nativeGetNoteSplashPngBytes(): ByteArray

    @JvmStatic
    external fun nativeHotCompile(tierCode: Int): String

    @JvmStatic
    external fun nativeGetHotCompileProgress(): String

    @JvmStatic
    external fun nativeStartGameplay(): String

    @JvmStatic
    external fun nativeStepGameplay(
        dtSec: Float,
        lanePressedMask: Int,
        laneJustPressedMask: Int
    ): String

    @JvmStatic
    external fun nativeStepGameplayBinary(
        dtSec: Float,
        lanePressedMask: Int,
        laneJustPressedMask: Int,
        outBuffer: ByteBuffer
    ): Int

    @JvmStatic
    external fun nativeSetGameplayPaused(paused: Boolean): String

    @JvmStatic
    external fun nativeStopGameplay(): String

    @JvmStatic
    external fun nativeToggleChartNoteWithType(
        secIndex: Int,
        stepInSec: Int,
        col0To7: Int,
        sustainSteps: Int,
        noteType: String
    ): String

    @JvmStatic
    external fun nativeImportArrowAtlas(pngPath: String, xmlPath: String): String

    @JvmStatic
    external fun nativeGetHurtArrowPngBytes(): ByteArray

    @JvmStatic
    external fun nativeTriggerGameOver(): String

    @JvmStatic
    external fun nativeConfirmGameOverRetry(): String

    @JvmStatic
    external fun nativeRestartGameplay(): String

    @JvmStatic
    external fun nativeSetGameplayDebug(botplay: Boolean, practiceMode: Boolean): String

    @JvmStatic
    external fun nativeProbeGraphicsBackends(): String

    @JvmStatic
    external fun nativeSetPreferredRenderBackend(backendKey: String): String
}
