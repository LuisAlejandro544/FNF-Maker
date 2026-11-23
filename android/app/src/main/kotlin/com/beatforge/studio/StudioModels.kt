// Archivo: android/app/src/main/kotlin/com/beatforge/studio/StudioModels.kt
// Propósito: Modelos de datos inmutables en Kotlin y deserializadores `org.json`
// que reflejan exactamente los snapshots JSON devueltos por el motor Rust (`pulse_android/src/session.rs`),
// evitando dependencias externas pesadas de serialización.

package com.beatforge.studio

import org.json.JSONArray
import org.json.JSONObject

const val CAMERA_SELECTION_ID = "__camera__"

enum class StudioScreen(val label: String) {
    Hub("Inicio"),
    Projects("Proyectos"),
    Chart("Chart"),
    Audio("Audio"),
    SpriteLab("Sprites"),
    StageEditor("Escena"),
    ScriptStudio("Scripts"),
    HotCompile("Compilar"),
    AudioAdapt("Acústica"),
    Gameplay("Juego")
}

enum class DeviceTier(val code: Int, val label: String, val subtitle: String) {
    LowRam32Bit(0, "32-bit / RAM Baja", "Fondos <=1920px, atlas <=2560px y audio streaming"),
    Balanced(1, "Equilibrado HD", "Atlas hasta 4096px y fondos <=2560px en 32/64 bits"),
    HighQuality(2, "Alta Calidad", "Texturas y atlas hasta 4096px, máxima nitidez HD")
}

enum class MobileControlMode(val key: String, val label: String, val description: String) {
    TapArrows(
        key = "tap_arrows",
        label = "Tocar Flechas (Por defecto)",
        description = "En Downscroll coloca las 4 flechas táctiles abajo estilo FNF Mobile"
    ),
    ClassicHitbox(
        key = "classic_hitbox",
        label = "Hitbox Clásico (Psych Mobile)",
        description = "4 columnas táctiles verticales de pantalla completa"
    );

    companion object {
        fun fromKey(raw: String): MobileControlMode {
            return entries.firstOrNull { it.key == raw } ?: TapArrows
        }
    }
}

enum class NoteSpecialType(
    val key: String,
    val code: Int,
    val label: String,
    val shortTag: String
) {
    Normal("default", 0, "Normal", ""),
    Hurt("hurt", 1, "Nota X (Daño)", "X"),
    Instakill("instakill", 2, "Muerte X", "💀"),
    Dodge("dodge", 3, "Esquive", "⚡"),
    AltAnim("alt", 4, "Alt Anim", "A"),
    NoAnim("no_anim", 5, "Sin Anim", "∅");

    companion object {
        fun fromCode(code: Int): NoteSpecialType {
            return entries.firstOrNull { it.code == code } ?: Normal
        }
    }
}

sealed class ImportKind(val tag: String, val mimeTypes: Array<String>) {
    data object Chart : ImportKind("chart", arrayOf("application/json", "text/plain", "*/*"))
    data class AudioTrack(val slot: Int) : ImportKind("audio_$slot", arrayOf("audio/*", "application/ogg", "*/*"))
    data class SpritePng(val entityId: String) : ImportKind("sprite_png", arrayOf("image/png", "image/*", "application/octet-stream", "*/*"))
    data class SparrowXml(val entityId: String) : ImportKind("sparrow_xml", arrayOf("text/xml", "application/xml", "application/json", "text/plain", "*/*"))
    data class CharacterConfig(val entityId: String) : ImportKind("char_cfg", arrayOf("application/json", "text/xml", "application/xml", "text/plain", "*/*"))
    data object StageConfig : ImportKind("stage_cfg", arrayOf("application/json", "text/xml", "application/xml", "text/plain", "*/*"))
    data object Script : ImportKind("script", arrayOf("text/plain", "application/javascript", "*/*"))
    data class ArrowSvg(val direction: Int) : ImportKind("arrow_$direction", arrayOf("image/svg+xml", "*/*"))
    data object ArrowAtlasPng : ImportKind("arrow_atlas_png", arrayOf("image/png", "image/webp", "image/*", "*/*"))
    data object ArrowAtlasXml : ImportKind("arrow_atlas_xml", arrayOf("text/xml", "application/xml", "text/plain", "*/*"))
    data class CharacterIcon(val entityId: String, val isLosing: Boolean) : ImportKind(
        "icon_${entityId}_${if (isLosing) "losing" else "normal"}",
        arrayOf("image/png", "image/*", "*/*")
    )
    data object HealthBarSvg : ImportKind("health_bar_svg", arrayOf("image/svg+xml", "*/*"))
    data object ModPackage : ImportKind("mod_pkg", arrayOf("*/*"))
}

data class ChartEventSnapshot(
    val timeMs: Double = 0.0,
    val name: String = "",
    val value1: String = "",
    val value2: String = ""
)

data class ChartSectionSnapshot(
    val index: Int = 0,
    val startMs: Double = 0.0,
    val lengthInSteps: Int = 16,
    val mustHitSection: Boolean = true,
    val bpm: Double = 128.0,
    val changeBpm: Boolean = false,
    val altAnim: Boolean = false,
    val gfSection: Boolean = false,
    val cameraSlot: Int? = null
)

data class ChartIntroUiSnapshot(
    val startBlackout: Boolean = false,
    val blackoutAlpha: Float = 1.0f,
    val hideHud: Boolean = false,
    val hideArrows: Boolean = false,
    val revealOnSongStart: Boolean = true,
    val revealDelayMs: Double = 0.0,
    val fadeSec: Float = 0.65f
)

data class ChartSnapshot(
    val title: String = "Demo Pulse",
    val bpm: Double = 120.0,
    val speed: Double = 1.8,
    val needsVoices: Boolean = true,
    val player1: String = "player_main",
    val player2: String = "opponent_main",
    val gfVersion: String = "stage_speakers",
    val stage: String = "main_stage",
    val sectionCount: Int = 0,
    val totalNotes: Int = 0,
    val playerNotes: Int = 0,
    val opponentNotes: Int = 0,
    val sustainNotes: Int = 0,
    val durationMs: Double = 0.0,
    val densityBuckets: List<Int> = emptyList(),
    val sourcePath: String? = null,
    val introUi: ChartIntroUiSnapshot = ChartIntroUiSnapshot(),
    val sections: List<ChartSectionSnapshot> = emptyList(),
    val notes: List<VisibleNote> = emptyList(),
    val events: List<ChartEventSnapshot> = emptyList()
)

data class AudioTrackSnapshot(
    val slotIndex: Int = 0,
    val roleLabel: String = "",
    val fileName: String? = null,
    val gain: Float = 1f,
    val muted: Boolean = false,
    val duckOnPlayerMiss: Boolean = false,
    val durationMs: Double = 0.0,
    val waveformPeaks: List<Float> = emptyList()
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

data class AudioAdaptProgress(
    val active: Boolean = false,
    val progress: Float = 0f,
    val phase: String = "Calibrando el sonido...",
    val detail: String = ""
)

data class SpectralBandProfileSnapshot(
    val subBass2090Hz: Float = 0f,
    val bass90220Hz: Float = 0f,
    val lowMid220600Hz: Float = 0f,
    val presence2k4kHz: Float = 0f,
    val air6k14kHz: Float = 0f,
    val spectralTiltDb: Float = 0f
)

data class SongAcousticAnalysisSnapshot(
    val sampleRate: Int = 44_100,
    val durationSec: Float = 0f,
    val integratedLufs: Float = -14f,
    val loudnessRangeLu: Float = 0f,
    val truePeakDbtp: Float = -1f,
    val crestFactorDb: Float = 0f,
    val stereoCorrelation: Float = 1f,
    val bands: SpectralBandProfileSnapshot = SpectralBandProfileSnapshot()
)

data class EqBandAdjustmentSnapshot(
    val kind: String = "",
    val freqHz: Float = 0f,
    val gainDb: Float = 0f,
    val q: Float = 0.707f,
    val reason: String = ""
)

data class EffectiveDeviceAcousticsSnapshot(
    val route: String = "builtin_speaker",
    val routeLabel: String = "Altavoz del Teléfono",
    val deviceName: String = "Altavoz Integrado",
    val manufacturer: String = "Android",
    val model: String = "Device",
    val androidApiLevel: Int = 26,
    val effectiveSampleRate: Int = 48_000,
    val hardwareNativeSampleRate: Int = 48_000,
    val effectiveChannelCount: Int = 2,
    val isMonoSpeaker: Boolean = false,
    val framesPerBurst: Int = 192,
    val bufferCapacityFrames: Int = 0,
    val lowLatencyActive: Boolean = false,
    val aaudioActive: Boolean = false,
    val aaudioDeviceId: Int = 0
)

data class AdaptiveDspPlanSnapshot(
    val profileName: String = "",
    val adaptationSummary: String = "",
    val targetLufs: Float = -14f,
    val appliedMakeupGainDb: Float = 0f,
    val ceilingDbtp: Float = -1f,
    val highpassHz: Float = 30f,
    val monoFoldEnabled: Boolean = false,
    val resampleNeeded: Boolean = false,
    val sourceSampleRate: Int = 44_100,
    val targetSampleRate: Int = 48_000,
    val stereoWidth: Float = 1f,
    val crossfeedStrength: Float = 0f,
    val limiterLookaheadMs: Float = 4f,
    val ditherApplied: Boolean = true,
    val highPrecisionF32Path: Boolean = false,
    val eqBands: List<EqBandAdjustmentSnapshot> = emptyList()
)

data class AudioAdaptationReportSnapshot(
    val enabled: Boolean = true,
    val bypassed: Boolean = false,
    val cacheReused: Boolean = false,
    val elapsedMs: Long = 0L,
    val outputWavPath: String? = null,
    val device: EffectiveDeviceAcousticsSnapshot = EffectiveDeviceAcousticsSnapshot(),
    val songAnalysis: SongAcousticAnalysisSnapshot? = null,
    val dspPlan: AdaptiveDspPlanSnapshot? = null,
    val outputTruePeakDbtp: Float = -1f
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
    val timelineWindows: List<EntityLifecycleWindowSnapshot> = emptyList()
)

data class ProjectSummary(
    val id: String = "",
    val name: String = "",
    val author: String = "",
    val chartTitle: String = "",
    val bpm: Double = 120.0,
    val noteCount: Int = 0,
    val trackCount: Int = 0,
    val entityCount: Int = 0,
    val scriptCount: Int = 0,
    val updatedAtMs: Long = 0L,
    val isActive: Boolean = false
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
    val preferredRenderBackend: String = "canvas2d",
    val audioAdaptEnabled: Boolean = true,
    val lastCompileReport: CompileReportSnapshot? = null,
    val lastAudioAdaptReport: AudioAdaptationReportSnapshot? = null
)

data class ActionResult(
    val ok: Boolean,
    val message: String,
    val snapshot: ProjectSnapshot?
)

data class ValidationResult(
    val ok: Boolean,
    val message: String
)

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

data class VisibleNote(
    val id: Long,
    val timeMs: Double,
    val direction: Int,
    val isPlayer: Boolean,
    val sustainMs: Double,
    val headHit: Boolean = false,
    val noteType: String = "default",
    val noteKindCode: Int = 0
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

object StudioJsonParser {

    fun parseActionResult(rawJson: String): ActionResult {
        return try {
            val obj = JSONObject(rawJson)
            val ok = obj.optBoolean("ok", false)
            val msg = obj.optString("message", "")
            val dataObj = obj.optJSONObject("data")
            ActionResult(ok, msg, dataObj?.let { parseProjectObject(it) })
        } catch (e: Exception) {
            ActionResult(false, "Error leyendo respuesta JSON: ${e.message}", null)
        }
    }

    fun parseValidationResult(rawJson: String): ValidationResult {
        return try {
            val obj = JSONObject(rawJson)
            ValidationResult(
                ok = obj.optBoolean("ok", false),
                message = obj.optString("message", "")
            )
        } catch (e: Exception) {
            ValidationResult(false, "Error validando script: ${e.message}")
        }
    }

    fun parseHotCompileProgress(rawJson: String): HotCompileProgress {
        return try {
            val obj = JSONObject(rawJson)
            HotCompileProgress(
                active = obj.optBoolean("active", false),
                progress = obj.optDouble("progress", 0.0).toFloat().coerceIn(0f, 1f),
                phase = obj.optString("phase", "Preparando el escenario..."),
                detail = obj.optString("detail", "")
            )
        } catch (_: Exception) {
            HotCompileProgress()
        }
    }

    fun parseAudioAdaptProgress(rawJson: String): AudioAdaptProgress {
        return try {
            val obj = JSONObject(rawJson)
            AudioAdaptProgress(
                active = obj.optBoolean("active", false),
                progress = obj.optDouble("progress", 0.0).toFloat().coerceIn(0f, 1f),
                phase = obj.optString("phase", "Calibrando el sonido..."),
                detail = obj.optString("detail", "")
            )
        } catch (_: Exception) {
            AudioAdaptProgress()
        }
    }

    fun parseProjectSnapshot(rawJson: String): ProjectSnapshot {
        return try {
            parseProjectObject(JSONObject(rawJson))
        } catch (_: Exception) {
            ProjectSnapshot()
        }
    }

    fun parseProjectList(rawJson: String): List<ProjectSummary> {
        return try {
            val arr = JSONArray(rawJson)
            buildList(arr.length()) {
                for (i in 0 until arr.length()) {
                    val obj = arr.optJSONObject(i) ?: continue
                    add(
                        ProjectSummary(
                            id = obj.optString("id", ""),
                            name = obj.optString("name", "Mod sin título"),
                            author = obj.optString("author", "Creador"),
                            chartTitle = obj.optString("chartTitle", ""),
                            bpm = obj.optDouble("bpm", 120.0),
                            noteCount = obj.optInt("noteCount", 0),
                            trackCount = obj.optInt("trackCount", 0),
                            entityCount = obj.optInt("entityCount", 0),
                            scriptCount = obj.optInt("scriptCount", 0),
                            updatedAtMs = obj.optLong("updatedAtMs", 0L),
                            isActive = obj.optBoolean("isActive", false)
                        )
                    )
                }
            }
        } catch (_: Exception) {
            emptyList()
        }
    }

    private fun parseProjectObject(obj: JSONObject): ProjectSnapshot {
        val chartObj = obj.optJSONObject("chart") ?: JSONObject()
        val secArr = chartObj.optJSONArray("sections") ?: JSONArray()
        val sections = buildList {
            for (i in 0 until secArr.length()) {
                val s = secArr.optJSONObject(i) ?: continue
                add(
                    ChartSectionSnapshot(
                        index = s.optInt("index", i),
                        startMs = s.optDouble("startMs", 0.0),
                        lengthInSteps = s.optInt("lengthInSteps", 16),
                        mustHitSection = s.optBoolean("mustHitSection", true),
                        bpm = s.optDouble("bpm", 120.0),
                        changeBpm = s.optBoolean("changeBpm", false),
                        altAnim = s.optBoolean("altAnim", false),
                        gfSection = s.optBoolean("gfSection", false),
                        cameraSlot = if (s.has("cameraSlot") && !s.isNull("cameraSlot")) s.optInt("cameraSlot", 0) else null
                    )
                )
            }
        }
        val chartNotesArr = chartObj.optJSONArray("notes") ?: JSONArray()
        val chartNotes = buildList {
            for (i in 0 until chartNotesArr.length()) {
                val n = chartNotesArr.optJSONObject(i) ?: continue
                add(
                    VisibleNote(
                        id = n.optLong("id", 0L),
                        timeMs = n.optDouble("timeMs", 0.0),
                        direction = n.optInt("direction", 0),
                        isPlayer = n.optBoolean("isPlayer", true),
                        sustainMs = n.optDouble("sustainMs", 0.0),
                        headHit = n.optBoolean("headHit", false),
                        noteType = n.optString("noteType", "default"),
                        noteKindCode = n.optInt("noteKindCode", 0)
                    )
                )
            }
        }
        val eventsArr = chartObj.optJSONArray("events") ?: JSONArray()
        val chartEvents = buildList {
            for (i in 0 until eventsArr.length()) {
                val ev = eventsArr.optJSONObject(i) ?: continue
                add(
                    ChartEventSnapshot(
                        timeMs = ev.optDouble("timeMs", 0.0),
                        name = ev.optString("name", ""),
                        value1 = ev.optString("value1", ""),
                        value2 = ev.optString("value2", "")
                    )
                )
            }
        }
        val introUiObj = chartObj.optJSONObject("introUi")
        val introUi = if (introUiObj != null) {
            ChartIntroUiSnapshot(
                startBlackout = introUiObj.optBoolean("startBlackout", false),
                blackoutAlpha = introUiObj.optDouble("blackoutAlpha", 1.0).toFloat().coerceIn(0f, 1f),
                hideHud = introUiObj.optBoolean("hideHud", false),
                hideArrows = introUiObj.optBoolean("hideArrows", false),
                revealOnSongStart = introUiObj.optBoolean("revealOnSongStart", true),
                revealDelayMs = introUiObj.optDouble("revealDelayMs", 0.0).coerceAtLeast(0.0),
                fadeSec = introUiObj.optDouble("fadeSec", 0.65).toFloat().coerceAtLeast(0f)
            )
        } else {
            ChartIntroUiSnapshot()
        }
        val chart = ChartSnapshot(
            title = chartObj.optString("title", "Demo"),
            bpm = chartObj.optDouble("bpm", 120.0),
            speed = chartObj.optDouble("speed", 1.8),
            needsVoices = chartObj.optBoolean("needsVoices", true),
            player1 = chartObj.optString("player1", "player_main"),
            player2 = chartObj.optString("player2", "opponent_main"),
            gfVersion = chartObj.optString("gfVersion", "stage_speakers"),
            stage = chartObj.optString("stage", "main_stage"),
            sectionCount = chartObj.optInt("sectionCount", 0),
            totalNotes = chartObj.optInt("totalNotes", 0),
            playerNotes = chartObj.optInt("playerNotes", 0),
            opponentNotes = chartObj.optInt("opponentNotes", 0),
            sustainNotes = chartObj.optInt("sustainNotes", 0),
            durationMs = chartObj.optDouble("durationMs", 0.0),
            densityBuckets = intList(chartObj.optJSONArray("densityBuckets")),
            sourcePath = optNullableString(chartObj, "sourcePath"),
            introUi = introUi,
            sections = sections,
            notes = chartNotes,
            events = chartEvents
        )

        val tracksArr = obj.optJSONArray("audioTracks") ?: JSONArray()
        val tracks = buildList {
            for (i in 0 until tracksArr.length()) {
                val t = tracksArr.optJSONObject(i) ?: continue
                add(
                    AudioTrackSnapshot(
                        slotIndex = t.optInt("slotIndex", i),
                        roleLabel = t.optString("roleLabel", "Pista ${i + 1}"),
                        fileName = optNullableString(t, "fileName"),
                        gain = t.optDouble("gain", 1.0).toFloat(),
                        muted = t.optBoolean("muted", false),
                        duckOnPlayerMiss = t.optBoolean("duckOnPlayerMiss", false),
                        durationMs = t.optDouble("durationMs", 0.0),
                        waveformPeaks = floatList(t.optJSONArray("waveformPeaks"))
                    )
                )
            }
        }

        val entArr = obj.optJSONArray("stageEntities") ?: JSONArray()
        val entities = buildList {
            for (i in 0 until entArr.length()) {
                val e = entArr.optJSONObject(i) ?: continue
                add(
                    StageEntitySnapshot(
                        id = e.optString("id", ""),
                        displayName = e.optString("displayName", "Objeto"),
                        role = e.optString("role", "decoration"),
                        roleLabel = e.optString("roleLabel", "Decoración"),
                        imagePath = optNullableString(e, "imagePath"),
                        rawImagePath = optNullableString(e, "rawImagePath"),
                        xmlPath = optNullableString(e, "xmlPath"),
                        animateJsonPath = optNullableString(e, "animateJsonPath"),
                        characterConfigPath = optNullableString(e, "characterConfigPath"),
                        iconNormalPath = optNullableString(e, "iconNormalPath"),
                        iconLosingPath = optNullableString(e, "iconLosingPath"),
                        x = e.optDouble("x", 0.0).toFloat(),
                        y = e.optDouble("y", 0.0).toFloat(),
                        width = e.optDouble("width", 200.0).toFloat(),
                        height = e.optDouble("height", 200.0).toFloat(),
                        scale = e.optDouble("scale", 1.0).toFloat(),
                        opacity = e.optDouble("opacity", 1.0).toFloat(),
                        flipX = e.optBoolean("flipX", false),
                        zIndex = e.optInt("zIndex", 0),
                        visible = e.optBoolean("visible", true),
                        animationPrefixes = stringList(e.optJSONArray("animationPrefixes")),
                        animBindings = stringMap(e.optJSONObject("animBindings")),
                        animOffsets = offsetMap(e.optJSONObject("animOffsets")),
                        previewFrame = e.optJSONObject("previewFrame")?.let { parseSparrowFrameObject(it) }
                    )
                )
            }
        }

        val scrArr = obj.optJSONArray("scripts") ?: JSONArray()
        val scripts = buildList {
            for (i in 0 until scrArr.length()) {
                val s = scrArr.optJSONObject(i) ?: continue
                add(
                    ScriptSnapshot(
                        id = s.optString("id", ""),
                        name = s.optString("name", "script.lua"),
                        language = s.optString("language", "lua"),
                        languageLabel = s.optString("languageLabel", "Lua"),
                        sourceCode = s.optString("sourceCode", ""),
                        enabled = s.optBoolean("enabled", true)
                    )
                )
            }
        }

        val reportObj = obj.optJSONObject("lastCompileReport")
        val compileReport = reportObj?.let { r ->
            CompileReportSnapshot(
                elapsedMs = r.optLong("elapsedMs", 0L),
                keptEntities = r.optInt("keptEntities", 0),
                discardedInvisibleEntities = stringList(r.optJSONArray("discardedInvisibleEntities")),
                downscaledTextures = r.optInt("downscaledTextures", 0),
                skippedOptimalTextures = r.optInt("skippedOptimalTextures", 0),
                cachedTexturesReused = r.optInt("cachedTexturesReused", 0),
                webpEncodedTextures = r.optInt("webpEncodedTextures", 0),
                discardedHeavyLoops = r.optInt("discardedHeavyLoops", 0),
                neutralizedUnboundedLoops = r.optInt("neutralizedUnboundedLoops", 0),
                estimatedVramSavedKb = r.optInt("estimatedVramSavedKb", 0),
                deadFramesRemoved = r.optInt("deadFramesRemoved", 0),
                duplicateFramesMerged = r.optInt("duplicateFramesMerged", 0),
                trimmedFrames = r.optInt("trimmedFrames", 0),
                prunedUnusedAnims = r.optInt("prunedUnusedAnims", 0),
                repackedAtlases = r.optInt("repackedAtlases", 0),
                repackedEntityIds = stringList(r.optJSONArray("repackedEntityIds")),
                rgb565Textures = r.optInt("rgb565Textures", 0),
                quantizedTextures = r.optInt("quantizedTextures", 0),
                splitAtlasPagesTotal = r.optInt("splitAtlasPagesTotal", 0),
                scheduledPreloadEntities = r.optInt("scheduledPreloadEntities", 0),
                scheduledRetireEntities = r.optInt("scheduledRetireEntities", 0),
                mergedAudioTracks = r.optInt("mergedAudioTracks", 0),
                compiledAudioPath = optNullableString(r, "compiledAudioPath"),
                estimatedAudioRamSavedKb = r.optInt("estimatedAudioRamSavedKb", 0),
                cachedAudioReused = r.optBoolean("cachedAudioReused", false),
                precompiledScripts = r.optInt("precompiledScripts", 0),
                cachedScriptsReused = r.optInt("cachedScriptsReused", 0),
                scriptPrecompileErrors = stringList(r.optJSONArray("scriptPrecompileErrors")),
                compiledTexturePaths = stringMap(r.optJSONObject("compiledTexturePaths")),
                compiledTexturePages = stringListMap(r.optJSONObject("compiledTexturePages")),
                texturePixelFormats = stringMap(r.optJSONObject("texturePixelFormats")),
                timelineWindows = parseTimelineWindows(r.optJSONArray("timelineWindows"))
            )
        }

        val camObj = obj.optJSONObject("stageCamera")
        val stageCamera = if (camObj != null) {
            val baseX = camObj.optDouble("baseX", 640.0).toFloat()
            val baseY = camObj.optDouble("baseY", 360.0).toFloat()
            val baseZoom = camObj.optDouble("baseZoom", 1.0).toFloat()
            val smoothSpeed = camObj.optDouble("smoothSpeed", 6.0).toFloat()
            val panStrength = camObj.optDouble("panStrength", 0.72).toFloat()
            val camArr = camObj.optJSONArray("cameras")
            val parsedSlots = if (camArr != null && camArr.length() > 0) {
                buildList(camArr.length()) {
                    for (i in 0 until camArr.length()) {
                        val slotObj = camArr.optJSONObject(i) ?: continue
                        add(
                            StageCameraSlotSnapshot(
                                index = slotObj.optInt("index", i),
                                name = slotObj.optString("name", "Cam $i"),
                                targetKind = slotObj.optString("targetKind", "fixed"),
                                targetEntityId = optNullableString(slotObj, "targetEntityId"),
                                x = slotObj.optDouble("x", 0.0).toFloat(),
                                y = slotObj.optDouble("y", 0.0).toFloat(),
                                zoom = slotObj.optDouble("zoom", 1.0).toFloat(),
                                smoothSpeed = slotObj.optDouble("smoothSpeed", smoothSpeed.toDouble()).toFloat(),
                                panStrength = slotObj.optDouble("panStrength", panStrength.toDouble()).toFloat()
                            )
                        )
                    }
                }
            } else {
                defaultStageCameraSlots(baseX, baseY, baseZoom, smoothSpeed, panStrength)
            }
            StageCameraSnapshot(
                enabled = camObj.optBoolean("enabled", true),
                baseX = baseX,
                baseY = baseY,
                baseZoom = baseZoom,
                followMode = camObj.optString("followMode", "sections"),
                followIntervalBeats = camObj.optInt("followIntervalBeats", 4),
                smoothSpeed = smoothSpeed,
                panStrength = panStrength,
                activeCameraIndex = camObj.optInt("activeCameraIndex", 0)
                    .coerceIn(0, (parsedSlots.size - 1).coerceAtLeast(0)),
                clampToBackground = camObj.optBoolean("clampToBackground", true),
                cameras = parsedSlots
            )
        } else {
            StageCameraSnapshot()
        }

        val audioAdaptReportObj = obj.optJSONObject("lastAudioAdaptReport")
        val audioAdaptReport = audioAdaptReportObj?.let { parseAudioAdaptationReportObject(it) }

        return ProjectSnapshot(
            id = obj.optString("id", ""),
            name = obj.optString("name", "Nuevo Mod"),
            author = obj.optString("author", "Creador"),
            downscroll = obj.optBoolean("downscroll", false),
            controlMode = obj.optString("controlMode", "tap_arrows"),
            audioOffsetMs = obj.optDouble("audioOffsetMs", 0.0),
            chart = chart,
            audioTracks = tracks,
            stageCamera = stageCamera,
            stageEntities = entities,
            scripts = scripts,
            arrowSkinName = obj.optString("arrowSkinName", "Original"),
            customArrowPaths = stringMap(obj.optJSONObject("customArrowPaths")),
            hasCustomPngArrowSkin = obj.optBoolean("hasCustomPngArrowSkin", false),
            healthBarSvgPath = optNullableString(obj, "healthBarSvgPath"),
            preferredRenderBackend = obj.optString("preferredRenderBackend", "canvas2d"),
            audioAdaptEnabled = obj.optBoolean("audioAdaptEnabled", true),
            lastCompileReport = compileReport,
            lastAudioAdaptReport = audioAdaptReport
        )
    }

    private fun parseAudioAdaptationReportObject(obj: JSONObject): AudioAdaptationReportSnapshot {
        val devObj = obj.optJSONObject("device") ?: JSONObject()
        val device = EffectiveDeviceAcousticsSnapshot(
            route = devObj.optString("route", "builtin_speaker"),
            routeLabel = devObj.optString("routeLabel", "Altavoz del Teléfono"),
            deviceName = devObj.optString("deviceName", "Altavoz Integrado"),
            manufacturer = devObj.optString("manufacturer", "Android"),
            model = devObj.optString("model", "Device"),
            androidApiLevel = devObj.optInt("androidApiLevel", 26),
            effectiveSampleRate = devObj.optInt("effectiveSampleRate", 48_000),
            hardwareNativeSampleRate = devObj.optInt("hardwareNativeSampleRate", 48_000),
            effectiveChannelCount = devObj.optInt("effectiveChannelCount", 2),
            isMonoSpeaker = devObj.optBoolean("isMonoSpeaker", false),
            framesPerBurst = devObj.optInt("framesPerBurst", 192),
            bufferCapacityFrames = devObj.optInt("bufferCapacityFrames", 0),
            lowLatencyActive = devObj.optBoolean("lowLatencyActive", false),
            aaudioActive = devObj.optBoolean("aaudioActive", false),
            aaudioDeviceId = devObj.optInt("aaudioDeviceId", 0)
        )

        val songObj = obj.optJSONObject("songAnalysis")
        val songAnalysis = songObj?.let { s ->
            val bandsObj = s.optJSONObject("bands") ?: JSONObject()
            SongAcousticAnalysisSnapshot(
                sampleRate = s.optInt("sampleRate", 44_100),
                durationSec = s.optDouble("durationSec", 0.0).toFloat(),
                integratedLufs = s.optDouble("integratedLufs", -14.0).toFloat(),
                loudnessRangeLu = s.optDouble("loudnessRangeLu", 0.0).toFloat(),
                truePeakDbtp = s.optDouble("truePeakDbtp", -1.0).toFloat(),
                crestFactorDb = s.optDouble("crestFactorDb", 0.0).toFloat(),
                stereoCorrelation = s.optDouble("stereoCorrelation", 1.0).toFloat(),
                bands = SpectralBandProfileSnapshot(
                    subBass2090Hz = bandsObj.optDouble("subBass2090Hz", 0.0).toFloat(),
                    bass90220Hz = bandsObj.optDouble("bass90220Hz", 0.0).toFloat(),
                    lowMid220600Hz = bandsObj.optDouble("lowMid220600Hz", 0.0).toFloat(),
                    presence2k4kHz = bandsObj.optDouble("presence2k4kHz", 0.0).toFloat(),
                    air6k14kHz = bandsObj.optDouble("air6k14kHz", 0.0).toFloat(),
                    spectralTiltDb = bandsObj.optDouble("spectralTiltDb", 0.0).toFloat()
                )
            )
        }

        val planObj = obj.optJSONObject("dspPlan")
        val dspPlan = planObj?.let { p ->
            val eqArr = p.optJSONArray("eqBands") ?: JSONArray()
            val eqBands = buildList(eqArr.length()) {
                for (i in 0 until eqArr.length()) {
                    val b = eqArr.optJSONObject(i) ?: continue
                    add(
                        EqBandAdjustmentSnapshot(
                            kind = b.optString("kind", ""),
                            freqHz = b.optDouble("freqHz", 0.0).toFloat(),
                            gainDb = b.optDouble("gainDb", 0.0).toFloat(),
                            q = b.optDouble("q", 0.707).toFloat(),
                            reason = b.optString("reason", "")
                        )
                    )
                }
            }
            AdaptiveDspPlanSnapshot(
                profileName = p.optString("profileName", ""),
                adaptationSummary = p.optString("adaptationSummary", ""),
                targetLufs = p.optDouble("targetLufs", -14.0).toFloat(),
                appliedMakeupGainDb = p.optDouble("appliedMakeupGainDb", 0.0).toFloat(),
                ceilingDbtp = p.optDouble("ceilingDbtp", -1.0).toFloat(),
                highpassHz = p.optDouble("highpassHz", 30.0).toFloat(),
                monoFoldEnabled = p.optBoolean("monoFoldEnabled", false),
                resampleNeeded = p.optBoolean("resampleNeeded", false),
                sourceSampleRate = p.optInt("sourceSampleRate", 44_100),
                targetSampleRate = p.optInt("targetSampleRate", 48_000),
                stereoWidth = p.optDouble("stereoWidth", 1.0).toFloat(),
                crossfeedStrength = p.optDouble("crossfeedStrength", 0.0).toFloat(),
                limiterLookaheadMs = p.optDouble("limiterLookaheadMs", 4.0).toFloat(),
                ditherApplied = p.optBoolean("ditherApplied", true),
                highPrecisionF32Path = p.optBoolean("highPrecisionF32Path", false),
                eqBands = eqBands
            )
        }

        return AudioAdaptationReportSnapshot(
            enabled = obj.optBoolean("enabled", true),
            bypassed = obj.optBoolean("bypassed", false),
            cacheReused = obj.optBoolean("cacheReused", false),
            elapsedMs = obj.optLong("elapsedMs", 0L),
            outputWavPath = optNullableString(obj, "outputWavPath"),
            device = device,
            songAnalysis = songAnalysis,
            dspPlan = dspPlan,
            outputTruePeakDbtp = obj.optDouble("outputTruePeakDbtp", -1.0).toFloat()
        )
    }

    fun parseSparrowFrameInfo(rawJson: String): SparrowFrameInfo {
        return try {
            parseSparrowFrameObject(JSONObject(rawJson))
        } catch (_: Exception) {
            SparrowFrameInfo()
        }
    }

    private fun parseSparrowFrameObject(obj: JSONObject): SparrowFrameInfo {
        return SparrowFrameInfo(
            found = obj.optBoolean("found", false),
            rawName = obj.optString("rawName", ""),
            x = obj.optInt("x", 0),
            y = obj.optInt("y", 0),
            width = obj.optInt("width", 0),
            height = obj.optInt("height", 0),
            frameX = obj.optInt("frameX", 0),
            frameY = obj.optInt("frameY", 0),
            frameWidth = obj.optInt("frameWidth", 0),
            frameHeight = obj.optInt("frameHeight", 0),
            rotated = obj.optBoolean("rotated", false),
            pageIndex = obj.optInt("pageIndex", 0)
        )
    }

    fun parseGameplayStep(rawJson: String): GameplayStep {
        return try {
            val obj = JSONObject(rawJson)
            val entArr = obj.optJSONArray("entities") ?: JSONArray()
            val entities = buildList {
                for (i in 0 until entArr.length()) {
                    val e = entArr.optJSONObject(i) ?: continue
                    val frameObj = e.optJSONObject("frame")
                    add(
                        GameplayEntityFrame(
                            id = e.optString("id", ""),
                            displayName = e.optString("displayName", ""),
                            role = e.optString("role", "decoration"),
                            x = e.optDouble("x", 0.0).toFloat(),
                            y = e.optDouble("y", 0.0).toFloat(),
                            width = e.optDouble("width", 200.0).toFloat(),
                            height = e.optDouble("height", 200.0).toFloat(),
                            scale = e.optDouble("scale", 1.0).toFloat(),
                            opacity = e.optDouble("opacity", 1.0).toFloat(),
                            rotationDeg = e.optDouble("rotationDeg", 0.0).toFloat(),
                            flipX = e.optBoolean("flipX", false),
                            visible = e.optBoolean("visible", true),
                            preloadRequested = e.optBoolean("preloadRequested", false),
                            retired = e.optBoolean("retired", false),
                            texturePath = optNullableString(e, "texturePath"),
                            frame = frameObj?.let { parseSparrowFrameObject(it) }
                        )
                    )
                }
            }

            val notesArr = obj.optJSONArray("visibleNotes") ?: JSONArray()
            val notes = buildList {
                for (i in 0 until notesArr.length()) {
                    val n = notesArr.optJSONObject(i) ?: continue
                    add(
                        VisibleNote(
                            id = n.optLong("id", 0L),
                            timeMs = n.optDouble("timeMs", 0.0),
                            direction = n.optInt("direction", 0),
                            isPlayer = n.optBoolean("isPlayer", true),
                            sustainMs = n.optDouble("sustainMs", 0.0),
                            headHit = n.optBoolean("headHit", false),
                            noteType = n.optString("noteType", "default"),
                            noteKindCode = n.optInt("noteKindCode", 0)
                        )
                    )
                }
            }

            val strumsArr = obj.optJSONArray("strumOverrides") ?: JSONArray()
            val strumOverrides = buildList {
                for (i in 0 until strumsArr.length()) {
                    val s = strumsArr.optJSONObject(i) ?: continue
                    add(
                        StrumOverrideSnapshot(
                            offsetX = s.optDouble("offsetX", 0.0).toFloat(),
                            offsetY = s.optDouble("offsetY", 0.0).toFloat(),
                            angle = s.optDouble("angle", 0.0).toFloat(),
                            scale = s.optDouble("scale", 1.0).toFloat(),
                            alpha = s.optDouble("alpha", 1.0).toFloat()
                        )
                    )
                }
            }

            val dynArr = obj.optJSONArray("dynamicItems") ?: JSONArray()
            val dynamicItems = buildList {
                for (i in 0 until dynArr.length()) {
                    val d = dynArr.optJSONObject(i) ?: continue
                    add(
                        DynamicScriptItemSnapshot(
                            id = d.optString("id", ""),
                            kind = d.optString("kind", "graphic"),
                            isHud = d.optBoolean("isHud", true),
                            visible = d.optBoolean("visible", true),
                            x = d.optDouble("x", 0.0).toFloat(),
                            y = d.optDouble("y", 0.0).toFloat(),
                            width = d.optDouble("width", 100.0).toFloat(),
                            height = d.optDouble("height", 100.0).toFloat(),
                            opacity = d.optDouble("opacity", 1.0).toFloat(),
                            angle = d.optDouble("angle", 0.0).toFloat(),
                            colorArgb = d.optLong("colorArgb", 0xFFFFFFFFL),
                            text = d.optString("text", "")
                        )
                    )
                }
            }

            GameplayStep(
                active = obj.optBoolean("active", false),
                songPosMs = obj.optDouble("songPosMs", 0.0),
                durationMs = obj.optDouble("durationMs", 0.0),
                scrollSpeed = obj.optDouble("scrollSpeed", 1.8),
                activeBpm = obj.optDouble("activeBpm", 120.0),
                beatPhase = obj.optDouble("beatPhase", 0.0).toFloat(),
                downscroll = obj.optBoolean("downscroll", false),
                controlMode = obj.optString("controlMode", "tap_arrows"),
                holdingLanesMask = obj.optInt("holdingLanesMask", 0),
                opponentHoldingLanesMask = obj.optInt("opponentHoldingLanesMask", 0),
                countdownStage = obj.optInt("countdownStage", -1),
                countdownLabel = obj.optString("countdownLabel", ""),
                countdownProgress = obj.optDouble("countdownProgress", 0.0).toFloat(),
                cameraZoom = obj.optDouble("cameraZoom", 1.0).toFloat(),
                cameraShake = obj.optDouble("cameraShake", 0.0).toFloat(),
                cameraX = obj.optDouble("cameraX", 640.0).toFloat(),
                cameraY = obj.optDouble("cameraY", 360.0).toFloat(),
                virtualWidth = obj.optDouble("virtualWidth", 1280.0).toFloat(),
                virtualHeight = obj.optDouble("virtualHeight", 720.0).toFloat(),
                score = obj.optLong("score", 0L),
                combo = obj.optInt("combo", 0),
                maxCombo = obj.optInt("maxCombo", 0),
                hits = obj.optInt("hits", 0),
                misses = obj.optInt("misses", 0),
                accuracyPct = obj.optDouble("accuracyPct", 100.0).toFloat(),
                health = obj.optDouble("health", 0.5).toFloat(),
                lastJudgement = obj.optString("lastJudgement", ""),
                ratingCode = obj.optInt("ratingCode", 0),
                ratingEventId = obj.optInt("ratingEventId", 0),
                splashEventId = obj.optInt("splashEventId", 0),
                splashLane = obj.optInt("splashLane", 0),
                hudVisible = obj.optBoolean("hudVisible", true),
                hudOpacity = obj.optDouble("hudOpacity", 1.0).toFloat(),
                hudZoom = obj.optDouble("hudZoom", 1.0).toFloat(),
                arrowsVisible = obj.optBoolean("arrowsVisible", true),
                arrowsOpacity = obj.optDouble("arrowsOpacity", 1.0).toFloat(),
                screenBlackoutAlpha = obj.optDouble("screenBlackoutAlpha", 0.0).toFloat(),
                flashColorArgb = obj.optLong("flashColorArgb", 0xFFFFFFFFL),
                flashAlpha = obj.optDouble("flashAlpha", 0.0).toFloat(),
                flashOverHud = obj.optBoolean("flashOverHud", false),
                activePlayerIdx = obj.optInt("activePlayerIdx", -1),
                activeOpponentIdx = obj.optInt("activeOpponentIdx", -1),
                paused = obj.optBoolean("paused", false),
                finished = obj.optBoolean("finished", false),
                gameOver = obj.optBoolean("gameOver", false),
                gameOverPhase = obj.optInt("gameOverPhase", 0),
                botplay = obj.optBoolean("botplay", false),
                practiceMode = obj.optBoolean("practiceMode", false),
                scriptErrors = stringList(obj.optJSONArray("scriptErrors")),
                scriptLogs = stringList(obj.optJSONArray("scriptLogs")),
                entities = entities,
                visibleNotes = notes,
                strumOverrides = strumOverrides,
                dynamicItems = dynamicItems
            )
        } catch (_: Exception) {
            GameplayStep()
        }
    }

    private fun optNullableString(obj: JSONObject, key: String): String? {
        if (!obj.has(key) || obj.isNull(key)) return null
        val v = obj.optString(key, "")
        return v.ifBlank { null }
    }

    private fun intList(arr: JSONArray?): List<Int> {
        if (arr == null) return emptyList()
        return buildList(arr.length()) {
            for (i in 0 until arr.length()) add(arr.optInt(i, 0))
        }
    }

    private fun floatList(arr: JSONArray?): List<Float> {
        if (arr == null) return emptyList()
        return buildList(arr.length()) {
            for (i in 0 until arr.length()) add(arr.optDouble(i, 0.0).toFloat())
        }
    }

    private fun stringList(arr: JSONArray?): List<String> {
        if (arr == null) return emptyList()
        return buildList(arr.length()) {
            for (i in 0 until arr.length()) add(arr.optString(i, ""))
        }
    }

    private fun stringMap(obj: JSONObject?): Map<String, String> {
        if (obj == null) return emptyMap()
        val out = linkedMapOf<String, String>()
        val keys = obj.keys()
        while (keys.hasNext()) {
            val k = keys.next()
            out[k] = obj.optString(k, "")
        }
        return out
    }

    private fun stringListMap(obj: JSONObject?): Map<String, List<String>> {
        if (obj == null) return emptyMap()
        val out = linkedMapOf<String, List<String>>()
        val keys = obj.keys()
        while (keys.hasNext()) {
            val k = keys.next()
            out[k] = stringList(obj.optJSONArray(k))
        }
        return out
    }

    private fun parseTimelineWindows(arr: JSONArray?): List<EntityLifecycleWindowSnapshot> {
        if (arr == null) return emptyList()
        return buildList(arr.length()) {
            for (i in 0 until arr.length()) {
                val w = arr.optJSONObject(i) ?: continue
                add(
                    EntityLifecycleWindowSnapshot(
                        entityId = w.optString("entityId", ""),
                        initiallyVisible = w.optBoolean("initiallyVisible", true),
                        preloadAtMs = if (w.has("preloadAtMs") && !w.isNull("preloadAtMs")) w.optDouble("preloadAtMs") else null,
                        firstVisibleMs = if (w.has("firstVisibleMs") && !w.isNull("firstVisibleMs")) w.optDouble("firstVisibleMs") else null,
                        lastUsedMs = if (w.has("lastUsedMs") && !w.isNull("lastUsedMs")) w.optDouble("lastUsedMs") else null,
                        retireAfterMs = if (w.has("retireAfterMs") && !w.isNull("retireAfterMs")) w.optDouble("retireAfterMs") else null,
                        reusedLater = w.optBoolean("reusedLater", false),
                        triggerReason = w.optString("triggerReason", "")
                    )
                )
            }
        }
    }

    private fun offsetMap(obj: JSONObject?): Map<String, Pair<Float, Float>> {
        if (obj == null) return emptyMap()
        val out = linkedMapOf<String, Pair<Float, Float>>()
        val keys = obj.keys()
        while (keys.hasNext()) {
            val k = keys.next()
            val arr = obj.optJSONArray(k) ?: continue
            if (arr.length() >= 2) {
                val ox = arr.optDouble(0, 0.0).toFloat()
                val oy = arr.optDouble(1, 0.0).toFloat()
                out[k] = ox to oy
            }
        }
        return out
    }
}
