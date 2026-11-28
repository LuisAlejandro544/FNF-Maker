// Archivo: android/app/src/main/kotlin/com/beatforge/studio/models/StudioJsonParser.kt
// Propósito: Deserializador principal `StudioJsonParser` basado en `org.json` para
// convertir los snapshots JSON del motor Rust (`ProjectSnapshot`, `CompileReportSnapshot`,
// `GameplayStep`, listas de proyectos y estados de progreso).

package com.beatforge.studio

import com.beatforge.studio.StudioJsonHelpers.floatList
import com.beatforge.studio.StudioJsonHelpers.intList
import com.beatforge.studio.StudioJsonHelpers.offsetMap
import com.beatforge.studio.StudioJsonHelpers.optNullableString
import com.beatforge.studio.StudioJsonHelpers.parseAudioAdaptationReportObject
import com.beatforge.studio.StudioJsonHelpers.parseCountdownSlotsArray
import com.beatforge.studio.StudioJsonHelpers.parseSparrowFrameObject
import com.beatforge.studio.StudioJsonHelpers.parseStageCameraObject
import com.beatforge.studio.StudioJsonHelpers.parseTimelineWindows
import com.beatforge.studio.StudioJsonHelpers.stringList
import com.beatforge.studio.StudioJsonHelpers.stringListMap
import com.beatforge.studio.StudioJsonHelpers.stringMap
import org.json.JSONArray
import org.json.JSONObject

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
                            devApiVersion = obj.optInt("devApiVersion", 1),
                            currentDevApiVersion = obj.optInt("currentDevApiVersion", 2),
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
        val chart = parseChartObject(chartObj)

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
                compiledCountdownCount = r.optInt("compiledCountdownCount", 0),
                compiledIconCount = r.optInt("compiledIconCount", 0),
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
                compiledCountdownPaths = stringMap(r.optJSONObject("compiledCountdownPaths")),
                compiledIconNormalPaths = stringMap(r.optJSONObject("compiledIconNormalPaths")),
                compiledIconLosingPaths = stringMap(r.optJSONObject("compiledIconLosingPaths")),
                timelineWindows = parseTimelineWindows(r.optJSONArray("timelineWindows"))
            )
        }

        val stageCamera = parseStageCameraObject(obj.optJSONObject("stageCamera"))
        val countdownSlots = parseCountdownSlotsArray(obj.optJSONArray("countdownSlots"))
        val audioAdaptReport = obj.optJSONObject("lastAudioAdaptReport")?.let {
            parseAudioAdaptationReportObject(it)
        }

        val parsedDevApiVersion = obj.optInt("devApiVersion", 1).coerceAtLeast(1)
        val parsedCurrentDevApi = obj.optInt("currentDevApiVersion", 2).coerceAtLeast(1)
        val parsedDevApiLabel = obj.optString("devApiLabel", "").ifBlank {
            if (parsedDevApiVersion >= 2) "v2 · Scripting Pro & Multicámara" else "v1 · Base Inicial"
        }

        return ProjectSnapshot(
            id = obj.optString("id", ""),
            name = obj.optString("name", "Nuevo Mod"),
            author = obj.optString("author", "Creador"),
            devApiVersion = parsedDevApiVersion,
            currentDevApiVersion = parsedCurrentDevApi,
            devApiLabel = parsedDevApiLabel,
            downscroll = obj.optBoolean("downscroll", false),
            controlMode = obj.optString("controlMode", "tap_arrows"),
            audioOffsetMs = obj.optDouble("audioOffsetMs", 0.0),
            autoAudioOffsetMs = obj.optDouble("autoAudioOffsetMs", 0.0),
            chart = chart,
            audioTracks = tracks,
            stageCamera = stageCamera,
            stageEntities = entities,
            scripts = scripts,
            arrowSkinName = obj.optString("arrowSkinName", "Original"),
            customArrowPaths = stringMap(obj.optJSONObject("customArrowPaths")),
            hasCustomPngArrowSkin = obj.optBoolean("hasCustomPngArrowSkin", false),
            healthBarSvgPath = optNullableString(obj, "healthBarSvgPath"),
            countdownSlots = countdownSlots,
            preferredRenderBackend = obj.optString("preferredRenderBackend", "gles3"),
            audioAdaptEnabled = obj.optBoolean("audioAdaptEnabled", true),
            lastCompileReport = compileReport,
            lastAudioAdaptReport = audioAdaptReport
        )
    }

    private fun parseChartObject(chartObj: JSONObject): ChartSnapshot {
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
        return ChartSnapshot(
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
    }

    fun parseSparrowFrameInfo(rawJson: String): SparrowFrameInfo {
        return try {
            parseSparrowFrameObject(JSONObject(rawJson))
        } catch (_: Exception) {
            SparrowFrameInfo()
        }
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
                vignetteColorArgb = obj.optLong("vignetteColorArgb", 0xFFEF4444L),
                vignetteAlpha = obj.optDouble("vignetteAlpha", 0.0).toFloat(),
                vignetteWidth = obj.optDouble("vignetteWidth", 0.24).toFloat(),
                vignettePulse = obj.optDouble("vignettePulse", 0.35).toFloat(),
                vignetteOverHud = obj.optBoolean("vignetteOverHud", false),
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
}
