// Archivo: android/app/src/main/kotlin/com/beatforge/studio/viewmodel/StudioEditorDelegate.kt
// Propósito: Fachada modular de `StudioViewModel` que coordina los subdelegados de edición
// en `viewmodel/editor/` (`ProjectLifecycleDelegate`, `SafImportExportDelegate`,
// `ChartAudioEditorDelegate` y `StageAssetsEditorDelegate`) sobre `engineIoDispatcher`.

package com.beatforge.studio.viewmodel

import android.app.Application
import android.net.Uri
import com.beatforge.studio.AudioPreviewTransportState
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.ImportKind
import com.beatforge.studio.SparrowFrameInfo
import com.beatforge.studio.StudioScreen
import com.beatforge.studio.StudioUiState
import com.beatforge.studio.gfx.RenderBackendKind
import com.beatforge.studio.viewmodel.editor.ChartAudioEditorDelegate
import com.beatforge.studio.viewmodel.editor.EditorStateHelper
import com.beatforge.studio.viewmodel.editor.ProjectLifecycleDelegate
import com.beatforge.studio.viewmodel.editor.SafImportExportDelegate
import com.beatforge.studio.viewmodel.editor.StageAssetsEditorDelegate
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow

internal class StudioEditorDelegate(
    app: Application,
    scope: CoroutineScope,
    engineIoDispatcher: CoroutineDispatcher,
    uiState: MutableStateFlow<StudioUiState>,
    gameplayFrameState: GameplayFrameState,
    bitmapLoader: StudioBitmapLoader
) {
    private val helper = EditorStateHelper(
        app = app,
        scope = scope,
        engineIoDispatcher = engineIoDispatcher,
        uiState = uiState,
        gameplayFrameState = gameplayFrameState,
        bitmapLoader = bitmapLoader
    )
    private val projectLifecycle = ProjectLifecycleDelegate(helper)
    private val safImportExport = SafImportExportDelegate(helper)
    private val chartAudio = ChartAudioEditorDelegate(helper)
    private val stageAssets = StageAssetsEditorDelegate(helper)

    fun initializeEngine() = projectLifecycle.initializeEngine()

    fun exportChartToUri(uri: Uri) = safImportExport.exportChartToUri(uri)

    fun exportModPackageToUri(projectId: String, uri: Uri) =
        safImportExport.exportModPackageToUri(projectId, uri)

    fun importUri(kind: ImportKind, uri: Uri) = safImportExport.importUri(kind, uri)

    fun clearStagedArrowAtlas() = safImportExport.clearStagedArrowAtlas()

    fun flushSave() = projectLifecycle.flushSave()

    fun refreshProjects() = projectLifecycle.refreshProjects()

    fun createProject(name: String, author: String) = projectLifecycle.createProject(name, author)

    fun openProject(projectId: String) = projectLifecycle.openProject(projectId)

    fun returnToProjectMenu() = projectLifecycle.returnToProjectMenu()

    fun setPreferredRenderBackend(kind: RenderBackendKind) =
        projectLifecycle.setPreferredRenderBackend(kind)

    fun duplicateProject(projectId: String) = projectLifecycle.duplicateProject(projectId)

    fun deleteProject(projectId: String) = projectLifecycle.deleteProject(projectId)

    fun flushAndMaybeRefreshProjects(screen: StudioScreen) =
        projectLifecycle.flushAndMaybeRefreshProjects(screen)

    fun flushSaveAsync() = projectLifecycle.flushSaveAsync()

    fun updateProjectMeta(
        name: String,
        author: String,
        downscroll: Boolean,
        controlMode: String,
        audioOffsetMs: Double,
        scrollSpeed: Double
    ) = projectLifecycle.updateProjectMeta(
        name, author, downscroll, controlMode, audioOffsetMs, scrollSpeed
    )

    fun clearCharacterIcons(entityId: String) = stageAssets.clearCharacterIcons(entityId)

    fun restoreDefaultHealthBarSvg() = stageAssets.restoreDefaultHealthBarSvg()

    fun restoreDefaultArrowSvg() = stageAssets.restoreDefaultArrowSvg()

    fun restoreDemoChart() = chartAudio.restoreDemoChart()

    fun setChartBpm(bpm: Double) = chartAudio.setChartBpm(bpm)

    fun updateChartDetails(
        title: String,
        speed: Double,
        player1: String,
        player2: String,
        gfVersion: String,
        stage: String,
        needsVoices: Boolean
    ) = chartAudio.updateChartDetails(title, speed, player1, player2, gfVersion, stage, needsVoices)

    fun updateChartIntroUi(
        startBlackout: Boolean,
        blackoutAlpha: Float,
        hideHud: Boolean,
        hideArrows: Boolean,
        revealOnSongStart: Boolean,
        revealDelayMs: Double,
        fadeSec: Float
    ) = chartAudio.updateChartIntroUi(
        startBlackout, blackoutAlpha, hideHud, hideArrows, revealOnSongStart, revealDelayMs, fadeSec
    )

    fun setSectionBpm(secIndex: Int, bpm: Double, changeBpm: Boolean) =
        chartAudio.setSectionBpm(secIndex, bpm, changeBpm)

    fun addChartEvent(timeMs: Double, name: String, value1: String, value2: String) =
        chartAudio.addChartEvent(timeMs, name, value1, value2)

    fun removeChartEvent(index: Int) = chartAudio.removeChartEvent(index)

    fun addChartSection(mustHit: Boolean) = chartAudio.addChartSection(mustHit)

    fun toggleSectionMustHit(secIndex: Int) = chartAudio.toggleSectionMustHit(secIndex)

    fun setSectionCameraSlot(secIndex: Int, cameraSlot: Int?) =
        chartAudio.setSectionCameraSlot(secIndex, cameraSlot)

    fun toggleChartNote(
        secIndex: Int,
        stepInSec: Int,
        col0To7: Int,
        sustainSteps: Int,
        noteType: String = "default"
    ) = chartAudio.toggleChartNote(secIndex, stepInSec, col0To7, sustainSteps, noteType)

    fun removeAudioTrack(slot: Int) = chartAudio.removeAudioTrack(slot)

    fun updateAudioTrackConfig(slot: Int, gain: Float, muted: Boolean, duckOnMiss: Boolean) =
        chartAudio.updateAudioTrackConfig(slot, gain, muted, duckOnMiss)

    fun setAudioAdaptEnabled(enabled: Boolean) = chartAudio.setAudioAdaptEnabled(enabled)

    fun toggleAudioPreview(startMs: Double) = chartAudio.toggleAudioPreview(startMs)

    fun setAudioPreviewTransport(play: Boolean, seekMs: Double = -1.0) =
        chartAudio.setAudioPreviewTransport(play, seekMs)

    suspend fun readAudioPreviewState(): AudioPreviewTransportState =
        chartAudio.readAudioPreviewState()

    fun readAudioPreviewStateFast(): AudioPreviewTransportState =
        chartAudio.readAudioPreviewStateFast()

    fun setSparrowBinding(entityId: String, animKind: String, prefix: String) =
        stageAssets.setSparrowBinding(entityId, animKind, prefix)

    fun setEntityAnimOffset(entityId: String, animKind: String, offsetX: Float, offsetY: Float) =
        stageAssets.setEntityAnimOffset(entityId, animKind, offsetX, offsetY)

    suspend fun querySparrowFrame(entityId: String, animKind: String, elapsedSec: Float): SparrowFrameInfo =
        stageAssets.querySparrowFrame(entityId, animKind, elapsedSec)

    fun addStageEntity(name: String, role: String) = stageAssets.addStageEntity(name, role)

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
    ) = stageAssets.updateStageEntity(
        entityId, name, role, dx, dy, absX, absY, useAbsPos, scale, opacity, visible, flipX, commitSave
    )

    fun reorderStageEntity(entityId: String, action: String) =
        stageAssets.reorderStageEntity(entityId, action)

    fun removeStageEntity(entityId: String) = stageAssets.removeStageEntity(entityId)

    fun clearStageScene() = stageAssets.clearStageScene()

    fun clearCountdownSprite(slot: Int) = stageAssets.clearCountdownSprite(slot)

    fun clearCountdownAudio(slot: Int) = stageAssets.clearCountdownAudio(slot)

    fun previewCountdownAudio(slot: Int) = stageAssets.previewCountdownAudio(slot)

    fun restoreDefaultCountdownAssets() = stageAssets.restoreDefaultCountdownAssets()

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
    ) = stageAssets.updateStageCamera(
        enabled, baseX, baseY, baseZoom, followMode, followIntervalBeats, smoothSpeed, panStrength, clampToBackground, commitSave
    )

    fun updateStageCameraVignette(
        enabled: Boolean,
        colorHex: String,
        alpha: Float,
        width: Float,
        pulse: Float,
        overHud: Boolean,
        commitSave: Boolean
    ) = stageAssets.updateStageCameraVignette(
        enabled, colorHex, alpha, width, pulse, overHud, commitSave
    )

    fun selectStageCamera(slotIndex: Int) = stageAssets.selectStageCamera(slotIndex)

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
    ) = stageAssets.updateCameraSlot(
        slotIndex, name, targetKind, targetEntityId, x, y, zoom, smoothSpeed, panStrength, commitSave
    )

    fun addCustomCamera(
        name: String,
        targetKind: String,
        targetEntityId: String?,
        x: Float,
        y: Float,
        zoom: Float,
        smoothSpeed: Float,
        panStrength: Float
    ) = stageAssets.addCustomCamera(
        name, targetKind, targetEntityId, x, y, zoom, smoothSpeed, panStrength
    )

    fun removeCustomCamera(slotIndex: Int) = stageAssets.removeCustomCamera(slotIndex)

    fun addScript(language: String) = stageAssets.addScript(language)

    fun updateScript(scriptId: String, enabled: Boolean, sourceCode: String, silent: Boolean) =
        stageAssets.updateScript(scriptId, enabled, sourceCode, silent)

    fun validateScript(scriptId: String, currentEnabled: Boolean, currentSource: String) =
        stageAssets.validateScript(scriptId, currentEnabled, currentSource)
}
