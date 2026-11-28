// Archivo: android/app/src/main/kotlin/com/beatforge/studio/viewmodel/editor/StageAssetsEditorDelegate.kt
// Propósito: Subdelegado de edición visual del escenario, multicámara/viñeta, sprites Sparrow,
// offsets de animación, íconos, skins SVG, assets de cuenta regresiva 2-1-¡YA! y scripts Lua/TS/JS.

package com.beatforge.studio.viewmodel.editor

import com.beatforge.studio.NativeEngineBridge
import com.beatforge.studio.SparrowFrameInfo
import com.beatforge.studio.StudioJsonParser
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

internal class StageAssetsEditorDelegate(
    private val helper: EditorStateHelper
) {
    fun clearCharacterIcons(entityId: String) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeClearCharacterIcons(entityId))
            helper.reloadCustomIcons(helper.uiState.value.snapshot.stageEntities)
        }
    }

    fun restoreDefaultHealthBarSvg() {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeRestoreDefaultHealthBarSvg())
            helper.reloadHealthBarBitmap()
        }
    }

    fun restoreDefaultArrowSvg() {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeRestoreDefaultArrowSvg())
            helper.reloadArrowBitmaps()
        }
    }

    fun setSparrowBinding(entityId: String, animKind: String, prefix: String) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeSetSparrowBinding(entityId, animKind, prefix))
        }
    }

    fun setEntityAnimOffset(entityId: String, animKind: String, offsetX: Float, offsetY: Float) {
        val ox = offsetX.coerceIn(-500f, 500f)
        val oy = offsetY.coerceIn(-500f, 500f)
        helper.uiState.update { state ->
            val updated = state.snapshot.stageEntities.map { ent ->
                if (ent.id == entityId) {
                    val nextMap = ent.animOffsets.toMutableMap()
                    nextMap[animKind] = ox to oy
                    ent.copy(animOffsets = nextMap)
                } else {
                    ent
                }
            }
            state.copy(snapshot = state.snapshot.copy(stageEntities = updated))
        }
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(
                NativeEngineBridge.nativeSetEntityAnimOffset(entityId, animKind, ox, oy),
                silentOk = true
            )
        }
    }

    suspend fun querySparrowFrame(entityId: String, animKind: String, elapsedSec: Float): SparrowFrameInfo {
        return withContext(helper.engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeGetSparrowFrameInfo(entityId, animKind, elapsedSec)
            StudioJsonParser.parseSparrowFrameInfo(raw)
        }
    }

    fun addStageEntity(name: String, role: String) {
        helper.scope.launch(helper.engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeAddStageEntity(name, role)
            helper.applyActionJson(raw)
            val newest = helper.uiState.value.snapshot.stageEntities.lastOrNull()?.id
            if (!newest.isNullOrBlank()) {
                helper.uiState.update { it.copy(selectedEntityId = newest) }
            }
        }
    }

    fun updateStageEntity(
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
    ) {
        val clampedScale = scale.coerceIn(0.05f, 6.0f)
        val clampedOpacity = opacity.coerceIn(0f, 1f)
        helper.uiState.update { state ->
            val updatedEntities = state.snapshot.stageEntities.map { ent ->
                if (ent.id != entityId) {
                    ent
                } else {
                    val nextX = when {
                        useAbsPos -> absX.coerceIn(-1000f, 2600f)
                        dx != 0f -> (ent.x + dx).coerceIn(-1000f, 2600f)
                        else -> ent.x
                    }
                    val nextY = when {
                        useAbsPos -> absY.coerceIn(-1000f, 1800f)
                        dy != 0f -> (ent.y + dy).coerceIn(-1000f, 1800f)
                        else -> ent.y
                    }
                    val cleanName = if (name.isNotBlank()) name.trim() else ent.displayName
                    val roleLabel = when (role) {
                        "background" -> "Fondo"
                        "player" -> "Player (Jugador)"
                        "opponent" -> "Enemigo (Rival)"
                        else -> "Decoración"
                    }
                    ent.copy(
                        displayName = cleanName,
                        role = role,
                        roleLabel = roleLabel,
                        x = nextX,
                        y = nextY,
                        scale = clampedScale,
                        opacity = clampedOpacity,
                        visible = visible,
                        flipX = flipX
                    )
                }
            }
            state.copy(snapshot = state.snapshot.copy(stageEntities = updatedEntities))
        }

        helper.scope.launch(helper.engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeUpdateStageEntity(
                entityId,
                name,
                role,
                dx,
                dy,
                absX,
                absY,
                useAbsPos,
                clampedScale,
                clampedOpacity,
                visible,
                flipX,
                commitSave
            )
            if (commitSave) {
                helper.applyActionJson(raw, silentOk = true)
            }
        }
    }

    fun reorderStageEntity(entityId: String, action: String) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeReorderStageEntity(entityId, action), silentOk = true)
        }
    }

    fun removeStageEntity(entityId: String) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeRemoveStageEntity(entityId))
            helper.bitmapLoader.evictRegionCache()
            val entities = helper.uiState.value.snapshot.stageEntities
            helper.reloadEntityBitmaps(entities)
            helper.reloadCustomIcons(entities)
            helper.refreshProjectsInternal()
        }
    }

    fun clearStageScene() {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeClearStageScene())
            helper.bitmapLoader.evictRegionCache()
            val entities = helper.uiState.value.snapshot.stageEntities
            helper.reloadEntityBitmaps(entities)
            helper.reloadCustomIcons(entities)
            helper.refreshProjectsInternal()
        }
    }

    fun clearCountdownSprite(slot: Int) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeClearCountdownSprite(slot))
            helper.reloadCountdownBitmaps(helper.uiState.value.snapshot.countdownSlots)
        }
    }

    fun clearCountdownAudio(slot: Int) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeClearCountdownAudio(slot))
        }
    }

    fun previewCountdownAudio(slot: Int) {
        helper.scope.launch(helper.engineIoDispatcher) {
            val raw = NativeEngineBridge.nativePreviewCountdownAudio(slot)
            helper.applyActionJson(raw, silentOk = true)
        }
    }

    fun restoreDefaultCountdownAssets() {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeRestoreDefaultCountdownAssets())
            helper.reloadCountdownBitmaps(helper.uiState.value.snapshot.countdownSlots)
        }
    }

    fun updateStageCamera(
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
    ) {
        val clampedX = baseX.coerceIn(-1000f, 2600f)
        val clampedY = baseY.coerceIn(-1000f, 1800f)
        val clampedZoom = baseZoom.coerceIn(0.35f, 2.5f)
        val clampedBeats = followIntervalBeats.coerceIn(1, 32)
        val clampedSmooth = smoothSpeed.coerceIn(1f, 20f)
        val clampedPan = panStrength.coerceIn(0f, 1f)
        val normalizedMode = when (followMode) {
            "fixed" -> "fixed"
            "beats" -> "beats"
            else -> "sections"
        }

        helper.uiState.update { state ->
            val currentCam = state.snapshot.stageCamera
            val updatedSlots = currentCam.resolvedCameras().map { slot ->
                if (slot.index == 0) {
                    slot.copy(
                        x = clampedX,
                        y = clampedY,
                        zoom = clampedZoom,
                        smoothSpeed = clampedSmooth
                    )
                } else {
                    slot
                }
            }
            state.copy(
                snapshot = state.snapshot.copy(
                    stageCamera = currentCam.copy(
                        enabled = enabled,
                        baseX = clampedX,
                        baseY = clampedY,
                        baseZoom = clampedZoom,
                        followMode = normalizedMode,
                        followIntervalBeats = clampedBeats,
                        smoothSpeed = clampedSmooth,
                        panStrength = clampedPan,
                        activeCameraIndex = currentCam.activeCameraIndex.coerceIn(0, (updatedSlots.size - 1).coerceAtLeast(0)),
                        clampToBackground = clampToBackground,
                        cameras = updatedSlots
                    )
                )
            )
        }

        helper.scope.launch(helper.engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeUpdateStageCamera(
                enabled,
                clampedX,
                clampedY,
                clampedZoom,
                normalizedMode,
                clampedBeats,
                clampedSmooth,
                clampedPan,
                clampToBackground,
                commitSave
            )
            if (commitSave) {
                helper.applyActionJson(raw, silentOk = true)
            }
        }
    }

    fun updateStageCameraVignette(
        enabled: Boolean,
        colorHex: String,
        alpha: Float,
        width: Float,
        pulse: Float,
        overHud: Boolean,
        commitSave: Boolean
    ) {
        val rawColor = colorHex.trim()
        val normalizedColor = when {
            rawColor.isEmpty() -> "#EF4444"
            rawColor.startsWith("#") -> rawColor
            else -> "#$rawColor"
        }
        val clampedAlpha = alpha.coerceIn(0f, 1f)
        val clampedWidth = width.coerceIn(0.05f, 0.50f)
        val clampedPulse = pulse.coerceIn(0f, 1f)

        helper.uiState.update { state ->
            state.copy(
                snapshot = state.snapshot.copy(
                    stageCamera = state.snapshot.stageCamera.copy(
                        vignetteEnabled = enabled,
                        vignetteColor = normalizedColor,
                        vignetteAlpha = clampedAlpha,
                        vignetteWidth = clampedWidth,
                        vignettePulse = clampedPulse,
                        vignetteOverHud = overHud
                    )
                )
            )
        }

        helper.scope.launch(helper.engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeUpdateStageCameraVignette(
                enabled,
                normalizedColor,
                clampedAlpha,
                clampedWidth,
                clampedPulse,
                overHud,
                commitSave
            )
            if (commitSave) {
                helper.applyActionJson(raw, silentOk = true)
            }
        }
    }

    fun selectStageCamera(slotIndex: Int) {
        helper.uiState.update { state ->
            val cam = state.snapshot.stageCamera
            val maxIdx = (cam.resolvedCameras().size - 1).coerceAtLeast(0)
            state.copy(
                snapshot = state.snapshot.copy(
                    stageCamera = cam.copy(activeCameraIndex = slotIndex.coerceIn(0, maxIdx))
                )
            )
        }
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeSelectStageCamera(slotIndex), silentOk = true)
        }
    }

    fun updateCameraSlot(
        slotIndex: Int,
        name: String,
        targetKind: String,
        targetEntityId: String?,
        x: Float,
        y: Float,
        zoom: Float,
        smoothSpeed: Float,
        panStrength: Float,
        commitSave: Boolean
    ) {
        val clampedX = x.coerceIn(-1200f, 2600f)
        val clampedY = y.coerceIn(-1200f, 1800f)
        val clampedZoom = zoom.coerceIn(0.35f, 2.5f)
        val clampedSmooth = smoothSpeed.coerceIn(1f, 20f)
        val clampedPan = panStrength.coerceIn(0f, 1f)
        val cleanName = name.trim()
        val cleanEntityId = targetEntityId?.trim()?.takeIf { it.isNotBlank() }

        helper.uiState.update { state ->
            val cam = state.snapshot.stageCamera
            val updatedSlots = cam.resolvedCameras().map { slot ->
                if (slot.index != slotIndex) {
                    slot
                } else {
                    slot.copy(
                        name = cleanName.ifBlank { slot.name },
                        targetKind = targetKind,
                        targetEntityId = if (targetKind == "fixed") null else cleanEntityId,
                        x = clampedX,
                        y = clampedY,
                        zoom = clampedZoom,
                        smoothSpeed = clampedSmooth,
                        panStrength = if (targetKind == "fixed") 0f else clampedPan
                    )
                }
            }
            val nextCam = if (slotIndex == 0) {
                cam.copy(
                    baseX = clampedX,
                    baseY = clampedY,
                    baseZoom = clampedZoom,
                    smoothSpeed = clampedSmooth,
                    activeCameraIndex = 0,
                    cameras = updatedSlots
                )
            } else {
                cam.copy(
                    activeCameraIndex = slotIndex.coerceIn(0, (updatedSlots.size - 1).coerceAtLeast(0)),
                    cameras = updatedSlots
                )
            }
            state.copy(snapshot = state.snapshot.copy(stageCamera = nextCam))
        }

        helper.scope.launch(helper.engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeUpdateCameraSlot(
                slotIndex,
                cleanName,
                targetKind,
                cleanEntityId ?: "",
                clampedX,
                clampedY,
                clampedZoom,
                clampedSmooth,
                clampedPan,
                commitSave
            )
            if (commitSave) {
                helper.applyActionJson(raw, silentOk = true)
            }
        }
    }

    fun addCustomCamera(
        name: String,
        targetKind: String,
        targetEntityId: String?,
        x: Float,
        y: Float,
        zoom: Float,
        smoothSpeed: Float,
        panStrength: Float
    ) {
        helper.scope.launch(helper.engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeAddCustomCamera(
                name,
                targetKind,
                targetEntityId ?: "",
                x.coerceIn(-1200f, 2600f),
                y.coerceIn(-1200f, 1800f),
                zoom.coerceIn(0.35f, 2.5f),
                smoothSpeed.coerceIn(1f, 20f),
                panStrength.coerceIn(0f, 1f)
            )
            helper.applyActionJson(raw)
        }
    }

    fun removeCustomCamera(slotIndex: Int) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeRemoveCustomCamera(slotIndex))
        }
    }

    fun addScript(language: String) {
        helper.scope.launch(helper.engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeAddScript(language)
            helper.applyActionJson(raw)
            val newest = helper.uiState.value.snapshot.scripts.lastOrNull()?.id
            if (!newest.isNullOrBlank()) {
                helper.uiState.update { it.copy(selectedScriptId = newest, scriptValidationMsg = null) }
            }
        }
    }

    fun updateScript(scriptId: String, enabled: Boolean, sourceCode: String, silent: Boolean) {
        helper.scope.launch(helper.engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeUpdateScript(scriptId, enabled, sourceCode)
            helper.applyActionJson(raw, silentOk = silent)
        }
    }

    fun validateScript(scriptId: String, currentEnabled: Boolean, currentSource: String) {
        helper.scope.launch(helper.engineIoDispatcher) {
            NativeEngineBridge.nativeUpdateScript(scriptId, currentEnabled, currentSource)
            val res = StudioJsonParser.parseValidationResult(
                NativeEngineBridge.nativeValidateScript(scriptId)
            )
            val snap = StudioJsonParser.parseProjectSnapshot(NativeEngineBridge.nativeGetSnapshot())
            helper.uiState.update {
                it.copy(
                    snapshot = snap,
                    scriptValidationOk = res.ok,
                    scriptValidationMsg = res.message
                )
            }
        }
    }
}
