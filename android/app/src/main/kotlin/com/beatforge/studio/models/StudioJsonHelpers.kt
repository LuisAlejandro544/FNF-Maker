// Archivo: android/app/src/main/kotlin/com/beatforge/studio/models/StudioJsonHelpers.kt
// Propósito: Funciones auxiliares `org.json` internas para decodificar mapas, listas,
// frames Sparrow, cámaras de escenario, slots de countdown, ventanas de ciclo de vida
// y reportes de adaptación acústica sin dependencias externas.

package com.beatforge.studio

import org.json.JSONArray
import org.json.JSONObject

internal object StudioJsonHelpers {

    fun optNullableString(obj: JSONObject, key: String): String? {
        if (!obj.has(key) || obj.isNull(key)) return null
        val v = obj.optString(key, "")
        return v.ifBlank { null }
    }

    fun intList(arr: JSONArray?): List<Int> {
        if (arr == null) return emptyList()
        return buildList(arr.length()) {
            for (i in 0 until arr.length()) add(arr.optInt(i, 0))
        }
    }

    fun floatList(arr: JSONArray?): List<Float> {
        if (arr == null) return emptyList()
        return buildList(arr.length()) {
            for (i in 0 until arr.length()) add(arr.optDouble(i, 0.0).toFloat())
        }
    }

    fun stringList(arr: JSONArray?): List<String> {
        if (arr == null) return emptyList()
        return buildList(arr.length()) {
            for (i in 0 until arr.length()) add(arr.optString(i, ""))
        }
    }

    fun stringMap(obj: JSONObject?): Map<String, String> {
        if (obj == null) return emptyMap()
        val out = linkedMapOf<String, String>()
        val keys = obj.keys()
        while (keys.hasNext()) {
            val k = keys.next()
            out[k] = obj.optString(k, "")
        }
        return out
    }

    fun stringListMap(obj: JSONObject?): Map<String, List<String>> {
        if (obj == null) return emptyMap()
        val out = linkedMapOf<String, List<String>>()
        val keys = obj.keys()
        while (keys.hasNext()) {
            val k = keys.next()
            out[k] = stringList(obj.optJSONArray(k))
        }
        return out
    }

    fun offsetMap(obj: JSONObject?): Map<String, Pair<Float, Float>> {
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

    fun parseSparrowFrameObject(obj: JSONObject): SparrowFrameInfo {
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

    fun parseTimelineWindows(arr: JSONArray?): List<EntityLifecycleWindowSnapshot> {
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

    fun parseStageCameraObject(camObj: JSONObject?): StageCameraSnapshot {
        if (camObj == null) return StageCameraSnapshot()
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
        return StageCameraSnapshot(
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
            vignetteEnabled = camObj.optBoolean("vignetteEnabled", false),
            vignetteColor = camObj.optString("vignetteColor", "#EF4444").ifBlank { "#EF4444" },
            vignetteAlpha = camObj.optDouble("vignetteAlpha", 0.58).toFloat().coerceIn(0f, 1f),
            vignetteWidth = camObj.optDouble("vignetteWidth", 0.24).toFloat().coerceIn(0.05f, 0.50f),
            vignettePulse = camObj.optDouble("vignettePulse", 0.35).toFloat().coerceIn(0f, 1f),
            vignetteOverHud = camObj.optBoolean("vignetteOverHud", false),
            cameras = parsedSlots
        )
    }

    fun parseCountdownSlotsArray(cdArr: JSONArray?): List<CountdownStepSnapshot> {
        if (cdArr == null) return emptyList()
        return buildList(cdArr.length()) {
            for (i in 0 until cdArr.length()) {
                val c = cdArr.optJSONObject(i) ?: continue
                val framesArr = c.optJSONArray("frames") ?: JSONArray()
                val frames = buildList(framesArr.length()) {
                    for (j in 0 until framesArr.length()) {
                        val f = framesArr.optJSONObject(j) ?: continue
                        add(parseSparrowFrameObject(f))
                    }
                }
                add(
                    CountdownStepSnapshot(
                        slotIndex = c.optInt("slotIndex", i),
                        label = c.optString("label", ""),
                        imagePath = optNullableString(c, "imagePath"),
                        imageName = optNullableString(c, "imageName"),
                        xmlName = optNullableString(c, "xmlName"),
                        frames = frames,
                        fps = c.optDouble("fps", 24.0).toFloat().coerceIn(4f, 60f),
                        audioName = optNullableString(c, "audioName"),
                        audioDurationMs = c.optDouble("audioDurationMs", 0.0)
                    )
                )
            }
        }
    }

    fun parseAudioAdaptationReportObject(obj: JSONObject): AudioAdaptationReportSnapshot {
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
            aaudioDeviceId = devObj.optInt("aaudioDeviceId", 0),
            aaudioMatched = devObj.optBoolean("aaudioMatched", false),
            estimatedOutputLatencyMs = devObj.optDouble("estimatedOutputLatencyMs", 0.0).toFloat(),
            recommendedAudioOffsetMs = devObj.optDouble("recommendedAudioOffsetMs", 0.0)
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
            outputTruePeakDbtp = obj.optDouble("outputTruePeakDbtp", -1.0).toFloat(),
            appliedAutoOffsetMs = obj.optDouble("appliedAutoOffsetMs", 0.0)
        )
    }
}
