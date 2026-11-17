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
    Gameplay("Juego")
}

enum class DeviceTier(val code: Int, val label: String, val subtitle: String) {
    LowRam32Bit(0, "32-bit / RAM Baja", "Fondos <=1536px, atlas <=2560px y audio streaming"),
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

sealed class ImportKind(val tag: String, val mimeTypes: Array<String>) {
    data object Chart : ImportKind("chart", arrayOf("application/json", "text/plain", "*/*"))
    data class AudioTrack(val slot: Int) : ImportKind("audio_$slot", arrayOf("audio/*", "application/ogg", "*/*"))
    data class SpritePng(val entityId: String) : ImportKind("sprite_png", arrayOf("image/png", "image/*", "application/octet-stream", "*/*"))
    data class SparrowXml(val entityId: String) : ImportKind("sparrow_xml", arrayOf("text/xml", "application/xml", "application/json", "text/plain", "*/*"))
    data object Script : ImportKind("script", arrayOf("text/plain", "application/javascript", "*/*"))
    data class ArrowSvg(val direction: Int) : ImportKind("arrow_$direction", arrayOf("image/svg+xml", "*/*"))
    data class CharacterIcon(val entityId: String, val isLosing: Boolean) : ImportKind(
        "icon_${entityId}_${if (isLosing) "losing" else "normal"}",
        arrayOf("image/png", "image/*", "*/*")
    )
    data object HealthBarSvg : ImportKind("health_bar_svg", arrayOf("image/svg+xml", "*/*"))
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
    val gfSection: Boolean = false
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
    val phase: String = "Preparando compilador...",
    val detail: String = ""
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
    val mergedAudioTracks: Int = 0,
    val compiledAudioPath: String? = null,
    val estimatedAudioRamSavedKb: Int = 0,
    val cachedAudioReused: Boolean = false,
    val precompiledScripts: Int = 0,
    val cachedScriptsReused: Int = 0,
    val scriptPrecompileErrors: List<String> = emptyList(),
    val compiledTexturePaths: Map<String, String> = emptyMap()
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

data class StageCameraSnapshot(
    val enabled: Boolean = true,
    val baseX: Float = 640f,
    val baseY: Float = 360f,
    val baseZoom: Float = 1f,
    val followMode: String = "beats",
    val followIntervalBeats: Int = 4,
    val smoothSpeed: Float = 6f,
    val panStrength: Float = 0.72f
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
    val healthBarSvgPath: String? = null,
    val lastCompileReport: CompileReportSnapshot? = null
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
    val rotated: Boolean = false
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
    val headHit: Boolean = false
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
    val flashColorArgb: Long = 0xFFFFFFFFL,
    val flashAlpha: Float = 0f,
    val flashOverHud: Boolean = false,
    val activePlayerIdx: Int = -1,
    val activeOpponentIdx: Int = -1,
    val paused: Boolean = false,
    val finished: Boolean = false,
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
                phase = obj.optString("phase", "Preparando compilador..."),
                detail = obj.optString("detail", "")
            )
        } catch (_: Exception) {
            HotCompileProgress()
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
                        gfSection = s.optBoolean("gfSection", false)
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
                        headHit = n.optBoolean("headHit", false)
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
                mergedAudioTracks = r.optInt("mergedAudioTracks", 0),
                compiledAudioPath = optNullableString(r, "compiledAudioPath"),
                estimatedAudioRamSavedKb = r.optInt("estimatedAudioRamSavedKb", 0),
                cachedAudioReused = r.optBoolean("cachedAudioReused", false),
                precompiledScripts = r.optInt("precompiledScripts", 0),
                cachedScriptsReused = r.optInt("cachedScriptsReused", 0),
                scriptPrecompileErrors = stringList(r.optJSONArray("scriptPrecompileErrors")),
                compiledTexturePaths = stringMap(r.optJSONObject("compiledTexturePaths"))
            )
        }

        val camObj = obj.optJSONObject("stageCamera")
        val stageCamera = if (camObj != null) {
            StageCameraSnapshot(
                enabled = camObj.optBoolean("enabled", true),
                baseX = camObj.optDouble("baseX", 640.0).toFloat(),
                baseY = camObj.optDouble("baseY", 360.0).toFloat(),
                baseZoom = camObj.optDouble("baseZoom", 1.0).toFloat(),
                followMode = camObj.optString("followMode", "beats"),
                followIntervalBeats = camObj.optInt("followIntervalBeats", 4),
                smoothSpeed = camObj.optDouble("smoothSpeed", 6.0).toFloat(),
                panStrength = camObj.optDouble("panStrength", 0.72).toFloat()
            )
        } else {
            StageCameraSnapshot()
        }

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
            healthBarSvgPath = optNullableString(obj, "healthBarSvgPath"),
            lastCompileReport = compileReport
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
            rotated = obj.optBoolean("rotated", false)
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
                            headHit = n.optBoolean("headHit", false)
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
                flashColorArgb = obj.optLong("flashColorArgb", 0xFFFFFFFFL),
                flashAlpha = obj.optDouble("flashAlpha", 0.0).toFloat(),
                flashOverHud = obj.optBoolean("flashOverHud", false),
                activePlayerIdx = obj.optInt("activePlayerIdx", -1),
                activeOpponentIdx = obj.optInt("activeOpponentIdx", -1),
                paused = obj.optBoolean("paused", false),
                finished = obj.optBoolean("finished", false),
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
}
