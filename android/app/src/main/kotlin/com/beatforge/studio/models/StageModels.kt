// Archivo: android/app/src/main/kotlin/com/beatforge/studio/models/StageModels.kt
// Propósito: Snapshots de entidades del escenario, cámara multi-slot con viñeta,
// metadata de recortes Sparrow XML/Animate y pasos de cuenta regresiva (3-2-1-¡YA!).

package com.beatforge.studio

data class SparrowFrameInfo(
    val found: Boolean = false,
    val rawName: String = "",
    val x: Int = 0,
    val y: Int = 0,
    val width: Int = 0,
    val height: Int = 0,
    val frameX: Int = 0,
    val frameY: Int = 0,
    val frameWidth: Int = 0,
    val frameHeight: Int = 0,
    val rotated: Boolean = false,
    val pageIndex: Int = 0
)

data class StageEntitySnapshot(
    val id: String = "",
    val displayName: String = "",
    val role: String = "decoration",
    val roleLabel: String = "Decoración",
    val imagePath: String? = null,
    val rawImagePath: String? = null,
    val xmlPath: String? = null,
    val animateJsonPath: String? = null,
    val characterConfigPath: String? = null,
    val iconNormalPath: String? = null,
    val iconLosingPath: String? = null,
    val x: Float = 0f,
    val y: Float = 0f,
    val width: Float = 200f,
    val height: Float = 200f,
    val scale: Float = 1f,
    val opacity: Float = 1f,
    val flipX: Boolean = false,
    val zIndex: Int = 0,
    val visible: Boolean = true,
    val animationPrefixes: List<String> = emptyList(),
    val animBindings: Map<String, String> = emptyMap(),
    val animOffsets: Map<String, Pair<Float, Float>> = emptyMap(),
    val previewFrame: SparrowFrameInfo? = null
)

data class StageCameraSlotSnapshot(
    val index: Int = 0,
    val name: String = "Cam 0 · Inicial Fija",
    val targetKind: String = "fixed",
    val targetEntityId: String? = null,
    val x: Float = 640f,
    val y: Float = 360f,
    val zoom: Float = 1f,
    val smoothSpeed: Float = 6f,
    val panStrength: Float = 0f
)

fun defaultStageCameraSlots(
    baseX: Float = 640f,
    baseY: Float = 360f,
    baseZoom: Float = 1f,
    smoothSpeed: Float = 6f,
    panStrength: Float = 0.72f
): List<StageCameraSlotSnapshot> = listOf(
    StageCameraSlotSnapshot(0, "Cam 0 · Inicial Fija", "fixed", null, baseX, baseY, baseZoom, smoothSpeed, 0f),
    StageCameraSlotSnapshot(1, "Cam 1 · Player", "player", "player_main", 0f, -24f, baseZoom, smoothSpeed, panStrength),
    StageCameraSlotSnapshot(2, "Cam 2 · Enemigo", "opponent", "opponent_main", 0f, -24f, baseZoom, smoothSpeed, panStrength)
)

data class StageCameraSnapshot(
    val enabled: Boolean = true,
    val baseX: Float = 640f,
    val baseY: Float = 360f,
    val baseZoom: Float = 1f,
    val followMode: String = "sections",
    val followIntervalBeats: Int = 4,
    val smoothSpeed: Float = 6f,
    val panStrength: Float = 0.72f,
    val activeCameraIndex: Int = 0,
    val clampToBackground: Boolean = true,
    val vignetteEnabled: Boolean = false,
    val vignetteColor: String = "#EF4444",
    val vignetteAlpha: Float = 0.58f,
    val vignetteWidth: Float = 0.24f,
    val vignettePulse: Float = 0.35f,
    val vignetteOverHud: Boolean = false,
    val cameras: List<StageCameraSlotSnapshot> = defaultStageCameraSlots()
) {
    fun resolvedCameras(): List<StageCameraSlotSnapshot> =
        cameras.ifEmpty { defaultStageCameraSlots(baseX, baseY, baseZoom, smoothSpeed, panStrength) }

    fun activeSlot(): StageCameraSlotSnapshot {
        val list = resolvedCameras()
        return list.firstOrNull { it.index == activeCameraIndex }
            ?: list.getOrNull(activeCameraIndex)
            ?: list.first()
    }
}

data class CountdownStepSnapshot(
    val slotIndex: Int = 0,
    val label: String = "",
    val imagePath: String? = null,
    val imageName: String? = null,
    val xmlName: String? = null,
    val frames: List<SparrowFrameInfo> = emptyList(),
    val fps: Float = 24f,
    val audioName: String? = null,
    val audioDurationMs: Double = 0.0
)
