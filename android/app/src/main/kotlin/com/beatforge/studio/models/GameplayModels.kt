// Archivo: android/app/src/main/kotlin/com/beatforge/studio/models/GameplayModels.kt
// Propósito: Modelos de scripts, progreso/reporte de Hot Compile (incluyendo rutas compiladas
// de cuenta regresiva e íconos), snapshot completo del proyecto y frame de estado GameplayStep.

package com.beatforge.studio

data class ScriptSnapshot(
    val id: String = "",
    val name: String = "",
    val language: String = "lua",
    val languageLabel: String = "Lua 5.4",
    val sourceCode: String = "",
    val enabled: Boolean = true
)

data class HotCompileProgress(
    val active: Boolean = false,
    val progress: Float = 0f,
    val phase: String = "Preparando el escenario...",
    val detail: String = ""
)

data class EntityLifecycleWindowSnapshot(
    val entityId: String = "",
    val initiallyVisible: Boolean = true,
    val preloadAtMs: Double? = null,
    val firstVisibleMs: Double? = null,
    val lastUsedMs: Double? = null,
    val retireAfterMs: Double? = null,
    val reusedLater: Boolean = false,
    val triggerReason: String = ""
)

data class CompileReportSnapshot(
    val elapsedMs: Long = 0L,
    val keptEntities: Int = 0,
    val discardedInvisibleEntities: List<String> = emptyList(),
    val downscaledTextures: Int = 0,
    val skippedOptimalTextures: Int = 0,
    val cachedTexturesReused: Int = 0,
    val webpEncodedTextures: Int = 0,
    val discardedHeavyLoops: Int = 0,
    val neutralizedUnboundedLoops: Int = 0,
    val estimatedVramSavedKb: Int = 0,
    val deadFramesRemoved: Int = 0,
    val duplicateFramesMerged: Int = 0,
    val trimmedFrames: Int = 0,
    val prunedUnusedAnims: Int = 0,
    val repackedAtlases: Int = 0,
    val repackedEntityIds: List<String> = emptyList(),
    val rgb565Textures: Int = 0,
    val quantizedTextures: Int = 0,
    val splitAtlasPagesTotal: Int = 0,
    val scheduledPreloadEntities: Int = 0,
    val scheduledRetireEntities: Int = 0,
    val compiledCountdownCount: Int = 0,
    val compiledIconCount: Int = 0,
    val mergedAudioTracks: Int = 0,
    val compiledAudioPath: String? = null,
    val estimatedAudioRamSavedKb: Int = 0,
    val cachedAudioReused: Boolean = false,
    val precompiledScripts: Int = 0,
    val cachedScriptsReused: Int = 0,
    val scriptPrecompileErrors: List<String> = emptyList(),
    val compiledTexturePaths: Map<String, String> = emptyMap(),
    val compiledTexturePages: Map<String, List<String>> = emptyMap(),
    val texturePixelFormats: Map<String, String> = emptyMap(),
    val compiledCountdownPaths: Map<String, String> = emptyMap(),
    val compiledIconNormalPaths: Map<String, String> = emptyMap(),
    val compiledIconLosingPaths: Map<String, String> = emptyMap(),
    val timelineWindows: List<EntityLifecycleWindowSnapshot> = emptyList()
)

data class ProjectSnapshot(
    val id: String = "",
    val name: String = "Nuevo Mod BeatForge",
    val author: String = "Creador",
    val downscroll: Boolean = false,
    val controlMode: String = "tap_arrows",
    val audioOffsetMs: Double = 0.0,
    val chart: ChartSnapshot = ChartSnapshot(),
    val audioTracks: List<AudioTrackSnapshot> = emptyList(),
    val stageCamera: StageCameraSnapshot = StageCameraSnapshot(),
    val stageEntities: List<StageEntitySnapshot> = emptyList(),
    val scripts: List<ScriptSnapshot> = emptyList(),
    val arrowSkinName: String = "Vector Original",
    val customArrowPaths: Map<String, String> = emptyMap(),
    val hasCustomPngArrowSkin: Boolean = false,
    val healthBarSvgPath: String? = null,
    val countdownSlots: List<CountdownStepSnapshot> = emptyList(),
    val preferredRenderBackend: String = "canvas2d",
    val audioAdaptEnabled: Boolean = true,
    val lastCompileReport: CompileReportSnapshot? = null,
    val lastAudioAdaptReport: AudioAdaptationReportSnapshot? = null
)

data class GameplayEntityFrame(
    val id: String,
    val displayName: String,
    val role: String,
    val x: Float,
    val y: Float,
    val width: Float,
    val height: Float,
    val scale: Float,
    val opacity: Float,
    val rotationDeg: Float = 0f,
    val flipX: Boolean,
    val visible: Boolean,
    val preloadRequested: Boolean = false,
    val retired: Boolean = false,
    val texturePath: String?,
    val frame: SparrowFrameInfo?
)

data class StrumOverrideSnapshot(
    val offsetX: Float = 0f,
    val offsetY: Float = 0f,
    val angle: Float = 0f,
    val scale: Float = 1f,
    val alpha: Float = 1f
)

data class DynamicScriptItemSnapshot(
    val id: String = "",
    val kind: String = "graphic",
    val isHud: Boolean = true,
    val visible: Boolean = true,
    val x: Float = 0f,
    val y: Float = 0f,
    val width: Float = 100f,
    val height: Float = 100f,
    val opacity: Float = 1f,
    val angle: Float = 0f,
    val colorArgb: Long = 0xFFFFFFFFL,
    val text: String = ""
)

data class GameplayStep(
    val active: Boolean = false,
    val songPosMs: Double = 0.0,
    val durationMs: Double = 0.0,
    val scrollSpeed: Double = 1.8,
    val activeBpm: Double = 120.0,
    val beatPhase: Float = 0f,
    val downscroll: Boolean = false,
    val controlMode: String = "tap_arrows",
    val holdingLanesMask: Int = 0,
    val opponentHoldingLanesMask: Int = 0,
    val countdownStage: Int = -1,
    val countdownLabel: String = "",
    val countdownProgress: Float = 0f,
    val cameraZoom: Float = 1f,
    val cameraShake: Float = 0f,
    val cameraX: Float = 640f,
    val cameraY: Float = 360f,
    val virtualWidth: Float = 1280f,
    val virtualHeight: Float = 720f,
    val score: Long = 0L,
    val combo: Int = 0,
    val maxCombo: Int = 0,
    val hits: Int = 0,
    val misses: Int = 0,
    val accuracyPct: Float = 100f,
    val health: Float = 0.5f,
    val lastJudgement: String = "",
    val ratingCode: Int = 0,
    val ratingEventId: Int = 0,
    val splashEventId: Int = 0,
    val splashLane: Int = 0,
    val hudVisible: Boolean = true,
    val hudOpacity: Float = 1f,
    val hudZoom: Float = 1f,
    val arrowsVisible: Boolean = true,
    val arrowsOpacity: Float = 1f,
    val screenBlackoutAlpha: Float = 0f,
    val flashColorArgb: Long = 0xFFFFFFFFL,
    val flashAlpha: Float = 0f,
    val flashOverHud: Boolean = false,
    val vignetteColorArgb: Long = 0xFFEF4444L,
    val vignetteAlpha: Float = 0f,
    val vignetteWidth: Float = 0.24f,
    val vignettePulse: Float = 0.35f,
    val vignetteOverHud: Boolean = false,
    val activePlayerIdx: Int = -1,
    val activeOpponentIdx: Int = -1,
    val paused: Boolean = false,
    val finished: Boolean = false,
    val gameOver: Boolean = false,
    val gameOverPhase: Int = 0,
    val botplay: Boolean = false,
    val practiceMode: Boolean = false,
    val scriptErrors: List<String> = emptyList(),
    val scriptLogs: List<String> = emptyList(),
    val entities: List<GameplayEntityFrame> = emptyList(),
    val visibleNotes: List<VisibleNote> = emptyList(),
    val strumOverrides: List<StrumOverrideSnapshot> = emptyList(),
    val dynamicItems: List<DynamicScriptItemSnapshot> = emptyList()
)
