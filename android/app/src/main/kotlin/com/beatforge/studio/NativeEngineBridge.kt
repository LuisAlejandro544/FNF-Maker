// Archivo: android/app/src/main/kotlin/com/beatforge/studio/NativeEngineBridge.kt
// Propósito: Declaraciones JNI directas hacia `libpulse_android.so` (motor Rust puro).
// No contiene lógica de juego, audio ni fallbacks en Kotlin: todas las operaciones reales
// (audio AAudio en Rust, parseo de charts, XML Sparrow, scripting Lua/TS/JS, SVG resvg,
// compilación en caliente con Rayon y simulación de gameplay) se ejecutan en Rust.

package com.beatforge.studio

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
    external fun nativeInit(cacheDir: String): String

    @JvmStatic
    external fun nativeGetSnapshot(): String

    @JvmStatic
    external fun nativeUpdateProjectMeta(
        name: String,
        author: String,
        downscroll: Boolean,
        audioOffsetMs: Double,
        scrollSpeed: Double
    ): String

    @JvmStatic
    external fun nativeImportChart(path: String): String

    @JvmStatic
    external fun nativeRestoreDemoChart(): String

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
    external fun nativeToggleAudioPreview(play: Boolean): Boolean

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
        flipX: Boolean
    ): String

    @JvmStatic
    external fun nativeReorderStageEntity(entityId: String, action: String): String

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
    external fun nativeHotCompile(tierCode: Int): String

    @JvmStatic
    external fun nativeStartGameplay(): String

    @JvmStatic
    external fun nativeStepGameplay(
        dtSec: Float,
        lanePressedMask: Int,
        laneJustPressedMask: Int
    ): String

    @JvmStatic
    external fun nativeSetGameplayPaused(paused: Boolean): String

    @JvmStatic
    external fun nativeStopGameplay(): String
}
