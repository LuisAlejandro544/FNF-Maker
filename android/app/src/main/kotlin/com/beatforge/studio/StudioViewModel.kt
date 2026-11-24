// Archivo: android/app/src/main/kotlin/com/beatforge/studio/StudioViewModel.kt
// Propósito: Coordinador reactivo principal de Jetpack Compose (`StudioViewModel`) que expone
// el estado (`StateFlow<StudioUiState>`) y delega de forma modular la carga de bitmaps
// (`StudioBitmapLoader`), la edición del estudio (`StudioEditorDelegate`) y el ciclo
// Hot Compile + Gameplay (`StudioGameplayCoordinator`) manteniendo todas las tareas pesadas
// en `Dispatchers.IO` / `engineIoDispatcher`.

package com.beatforge.studio

import android.app.Application
import android.net.Uri
import androidx.compose.ui.graphics.ImageBitmap
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.beatforge.studio.gfx.RenderBackendKind
import com.beatforge.studio.viewmodel.StudioBitmapLoader
import com.beatforge.studio.viewmodel.StudioEditorDelegate
import com.beatforge.studio.viewmodel.StudioGameplayCoordinator
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import java.util.concurrent.Executors

class StudioViewModel(application: Application) : AndroidViewModel(application) {

    private val _uiState = MutableStateFlow(StudioUiState())
    val uiState: StateFlow<StudioUiState> = _uiState.asStateFlow()

    val gameplayFrameState = GameplayFrameState()

    private val engineExecutor = Executors.newSingleThreadExecutor { r ->
        Thread(r, "BeatForgeEngineIO").apply { isDaemon = true }
    }
    private val engineIoDispatcher = engineExecutor.asCoroutineDispatcher()

    private val bitmapLoader = StudioBitmapLoader()
    private val editorDelegate = StudioEditorDelegate(
        app = application,
        scope = viewModelScope,
        engineIoDispatcher = engineIoDispatcher,
        uiState = _uiState,
        gameplayFrameState = gameplayFrameState,
        bitmapLoader = bitmapLoader
    )
    private val gameplayCoordinator = StudioGameplayCoordinator(
        app = application,
        scope = viewModelScope,
        engineIoDispatcher = engineIoDispatcher,
        uiState = _uiState,
        gameplayFrameState = gameplayFrameState,
        bitmapLoader = bitmapLoader
    )

    private var pendingImportKind: ImportKind? = null
    private var pendingExportModProjectId: String? = null
    private var launchPickerCallback: ((Array<String>) -> Unit)? = null
    private var launchExportChartCallback: ((String) -> Unit)? = null
    private var launchExportModCallback: ((String) -> Unit)? = null

    init {
        _uiState.update { it.copy(selectedTier = bitmapLoader.detectOptimalDeviceTier(application)) }
        editorDelegate.initializeEngine()
    }

    override fun onCleared() {
        try {
            NativeEngineBridge.nativeFlushSave()
        } catch (_: Throwable) {
        }
        engineExecutor.shutdown()
        super.onCleared()
    }

    fun registerPickerLauncher(launcher: (Array<String>) -> Unit) {
        launchPickerCallback = launcher
    }

    fun registerExportChartLauncher(launcher: (String) -> Unit) {
        launchExportChartCallback = launcher
    }

    fun registerExportModLauncher(launcher: (String) -> Unit) {
        launchExportModCallback = launcher
    }

    fun requestImport(kind: ImportKind) {
        pendingImportKind = kind
        launchPickerCallback?.invoke(kind.mimeTypes)
    }

    fun requestExportChartJson() {
        val safeTitle = _uiState.value.snapshot.chart.title
            .lowercase()
            .replace(Regex("[^a-z0-9_-]+"), "_")
            .trim('_')
            .ifBlank { "chart" }
        launchExportChartCallback?.invoke("$safeTitle.json")
    }

    fun requestExportModPackage(projectId: String, projectName: String) {
        pendingExportModProjectId = projectId
        val safeName = projectName
            .lowercase()
            .replace(Regex("[^a-z0-9_-]+"), "_")
            .trim('_')
            .ifBlank { "beatforge_mod" }
        launchExportModCallback?.invoke("$safeName.mod")
    }

    fun exportChartToUri(uri: Uri) = editorDelegate.exportChartToUri(uri)

    fun onExportModUriPicked(uri: Uri) {
        val projectId = pendingExportModProjectId ?: return
        pendingExportModProjectId = null
        editorDelegate.exportModPackageToUri(projectId, uri)
    }

    fun onUriPicked(uri: Uri) {
        val kind = pendingImportKind ?: return
        pendingImportKind = null
        editorDelegate.importUri(kind, uri)
    }

    fun flushSave() = editorDelegate.flushSave()

    fun refreshProjects() = editorDelegate.refreshProjects()

    fun createProject(name: String, author: String) = editorDelegate.createProject(name, author)

    fun openProject(projectId: String) = editorDelegate.openProject(projectId)

    fun returnToProjectMenu() = editorDelegate.returnToProjectMenu()

    fun duplicateProject(projectId: String) = editorDelegate.duplicateProject(projectId)

    fun deleteProject(projectId: String) = editorDelegate.deleteProject(projectId)

    fun setPreferredRenderBackend(kind: RenderBackendKind) = editorDelegate.setPreferredRenderBackend(kind)

    fun navigateTo(screen: StudioScreen) {
        if (_uiState.value.currentScreen == StudioScreen.Gameplay && screen != StudioScreen.Gameplay) {
            stopGameplayAndNavigate(screen)
            return
        }
        editorDelegate.flushAndMaybeRefreshProjects(screen)
        _uiState.update { it.copy(currentScreen = screen, statusBanner = null) }
    }

    fun selectEntity(entityId: String) {
        _uiState.update { it.copy(selectedEntityId = entityId) }
    }

    fun selectScript(scriptId: String) {
        _uiState.update {
            it.copy(
                selectedScriptId = scriptId,
                scriptValidationMsg = null
            )
        }
    }

    fun setArrowSkinDialogOpen(open: Boolean) {
        _uiState.update { it.copy(arrowSkinDialogOpen = open) }
    }

    fun clearStagedArrowAtlas() = editorDelegate.clearStagedArrowAtlas()

    fun setCharacterIconsDialogOpen(open: Boolean) {
        _uiState.update { it.copy(characterIconsDialogOpen = open) }
    }

    fun setModFilesGuideDialogOpen(open: Boolean) {
        _uiState.update { it.copy(modFilesGuideDialogOpen = open) }
    }

    fun setCountdownAssetsDialogOpen(open: Boolean) {
        _uiState.update { it.copy(countdownAssetsDialogOpen = open) }
    }

    fun clearCountdownSprite(slot: Int) = editorDelegate.clearCountdownSprite(slot)

    fun clearCountdownAudio(slot: Int) = editorDelegate.clearCountdownAudio(slot)

    fun previewCountdownAudio(slot: Int) = editorDelegate.previewCountdownAudio(slot)

    fun restoreDefaultCountdownAssets() = editorDelegate.restoreDefaultCountdownAssets()

    fun clearCharacterIcons(entityId: String) = editorDelegate.clearCharacterIcons(entityId)

    fun restoreDefaultHealthBarSvg() = editorDelegate.restoreDefaultHealthBarSvg()

    fun dismissBanner() {
        _uiState.update { it.copy(statusBanner = null) }
    }

    fun updateProjectMeta(
        name: String,
        author: String,
        downscroll: Boolean,
        controlMode: String = _uiState.value.snapshot.controlMode,
        audioOffsetMs: Double,
        scrollSpeed: Double
    ) = editorDelegate.updateProjectMeta(name, author, downscroll, controlMode, audioOffsetMs, scrollSpeed)

    fun updateControlMode(mode: MobileControlMode) {
        val snap = _uiState.value.snapshot
        updateProjectMeta(
            name = snap.name,
            author = snap.author,
            downscroll = snap.downscroll,
            controlMode = mode.key,
            audioOffsetMs = snap.audioOffsetMs,
            scrollSpeed = snap.chart.speed
        )
    }

    fun restoreDemoChart() = editorDelegate.restoreDemoChart()

    fun setChartBpm(bpm: Double) = editorDelegate.setChartBpm(bpm)

    fun updateChartDetails(
        title: String,
        speed: Double,
        player1: String,
        player2: String,
        gfVersion: String,
        stage: String,
        needsVoices: Boolean
    ) = editorDelegate.updateChartDetails(title, speed, player1, player2, gfVersion, stage, needsVoices)

    fun updateChartIntroUi(
        startBlackout: Boolean,
        blackoutAlpha: Float,
        hideHud: Boolean,
        hideArrows: Boolean,
        revealOnSongStart: Boolean,
        revealDelayMs: Double,
        fadeSec: Float
    ) = editorDelegate.updateChartIntroUi(
        startBlackout,
        blackoutAlpha,
        hideHud,
        hideArrows,
        revealOnSongStart,
        revealDelayMs,
        fadeSec
    )

    fun setSectionBpm(secIndex: Int, bpm: Double, changeBpm: Boolean) =
        editorDelegate.setSectionBpm(secIndex, bpm, changeBpm)

    fun addChartEvent(timeMs: Double, name: String, value1: String, value2: String) =
        editorDelegate.addChartEvent(timeMs, name, value1, value2)

    fun removeChartEvent(index: Int) = editorDelegate.removeChartEvent(index)

    fun addChartSection(mustHit: Boolean) = editorDelegate.addChartSection(mustHit)

    fun toggleSectionMustHit(secIndex: Int) = editorDelegate.toggleSectionMustHit(secIndex)

    fun setSectionCameraSlot(secIndex: Int, cameraSlot: Int?) =
        editorDelegate.setSectionCameraSlot(secIndex, cameraSlot)

    fun toggleChartNote(
        secIndex: Int,
        stepInSec: Int,
        col0To7: Int,
        sustainSteps: Int,
        noteType: String = "default"
    ) = editorDelegate.toggleChartNote(secIndex, stepInSec, col0To7, sustainSteps, noteType)

    fun removeAudioTrack(slot: Int) = editorDelegate.removeAudioTrack(slot)

    fun updateAudioTrackConfig(slot: Int, gain: Float, muted: Boolean, duckOnMiss: Boolean) =
        editorDelegate.updateAudioTrackConfig(slot, gain, muted, duckOnMiss)

    fun setAudioAdaptEnabled(enabled: Boolean) = editorDelegate.setAudioAdaptEnabled(enabled)

    fun toggleAudioPreview(startMs: Double = 0.0) = editorDelegate.toggleAudioPreview(startMs)

    fun setAudioPreviewTransport(play: Boolean, seekMs: Double = -1.0) =
        editorDelegate.setAudioPreviewTransport(play, seekMs)

    suspend fun readAudioPreviewState(): AudioPreviewTransportState =
        editorDelegate.readAudioPreviewState()

    fun readAudioPreviewStateFast(): AudioPreviewTransportState =
        editorDelegate.readAudioPreviewStateFast()

    fun setSparrowBinding(entityId: String, animKind: String, prefix: String) =
        editorDelegate.setSparrowBinding(entityId, animKind, prefix)

    fun setEntityAnimOffset(entityId: String, animKind: String, offsetX: Float, offsetY: Float) =
        editorDelegate.setEntityAnimOffset(entityId, animKind, offsetX, offsetY)

    suspend fun querySparrowFrame(entityId: String, animKind: String, elapsedSec: Float): SparrowFrameInfo =
        editorDelegate.querySparrowFrame(entityId, animKind, elapsedSec)

    suspend fun decodeSparrowRegionBitmap(imagePath: String?, frame: SparrowFrameInfo): ImageBitmap? =
        bitmapLoader.decodeSparrowRegionBitmap(imagePath, frame, _uiState.value.selectedTier)

    fun addStageEntity(name: String, role: String) = editorDelegate.addStageEntity(name, role)

    fun updateStageEntity(
        entityId: String,
        name: String,
        role: String,
        dx: Float = 0f,
        dy: Float = 0f,
        absX: Float = 0f,
        absY: Float = 0f,
        useAbsPos: Boolean = false,
        scale: Float,
        opacity: Float,
        visible: Boolean,
        flipX: Boolean,
        commitSave: Boolean = true
    ) = editorDelegate.updateStageEntity(
        entityId, name, role, dx, dy, absX, absY, useAbsPos, scale, opacity, visible, flipX, commitSave
    )

    fun reorderStageEntity(entityId: String, action: String) =
        editorDelegate.reorderStageEntity(entityId, action)

    fun removeStageEntity(entityId: String) =
        editorDelegate.removeStageEntity(entityId)

    fun clearStageScene() =
        editorDelegate.clearStageScene()

    fun updateStageCamera(
        enabled: Boolean,
        baseX: Float,
        baseY: Float,
        baseZoom: Float,
        followMode: String,
        followIntervalBeats: Int,
        smoothSpeed: Float,
        panStrength: Float,
        clampToBackground: Boolean = _uiState.value.snapshot.stageCamera.clampToBackground,
        commitSave: Boolean = true
    ) = editorDelegate.updateStageCamera(
        enabled, baseX, baseY, baseZoom, followMode, followIntervalBeats, smoothSpeed, panStrength, clampToBackground, commitSave
    )

    fun updateStageCameraVignette(
        enabled: Boolean,
        colorHex: String,
        alpha: Float,
        width: Float,
        pulse: Float,
        overHud: Boolean,
        commitSave: Boolean = true
    ) = editorDelegate.updateStageCameraVignette(
        enabled, colorHex, alpha, width, pulse, overHud, commitSave
    )

    fun selectStageCamera(slotIndex: Int) = editorDelegate.selectStageCamera(slotIndex)

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
        commitSave: Boolean = true
    ) = editorDelegate.updateCameraSlot(
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
    ) = editorDelegate.addCustomCamera(
        name, targetKind, targetEntityId, x, y, zoom, smoothSpeed, panStrength
    )

    fun removeCustomCamera(slotIndex: Int) = editorDelegate.removeCustomCamera(slotIndex)

    fun openCameraScriptInStudio() {
        val snap = _uiState.value.snapshot
        val camScriptId = snap.scripts.firstOrNull {
            it.id == "script_camera_system" || it.name == "camera_system.lua"
        }?.id ?: snap.scripts.firstOrNull()?.id ?: ""
        editorDelegate.flushSaveAsync()
        _uiState.update {
            it.copy(
                selectedScriptId = camScriptId,
                currentScreen = StudioScreen.ScriptStudio,
                scriptValidationMsg = null,
                statusBanner = null
            )
        }
    }

    fun openStageEventsScriptInStudio() {
        val snap = _uiState.value.snapshot
        val eventsScriptId = snap.scripts.firstOrNull {
            it.id == "script_stage_events" || it.name == "stage_events.lua"
        }?.id ?: snap.scripts.firstOrNull()?.id ?: ""
        editorDelegate.flushSaveAsync()
        _uiState.update {
            it.copy(
                selectedScriptId = eventsScriptId,
                currentScreen = StudioScreen.ScriptStudio,
                scriptValidationMsg = null,
                statusBanner = null
            )
        }
    }

    fun openUiBlackoutScriptInStudio() {
        val snap = _uiState.value.snapshot
        val uiScriptId = snap.scripts.firstOrNull {
            it.id == "script_ui_blackout" || it.name == "ui_blackout.lua"
        }?.id ?: snap.scripts.firstOrNull()?.id ?: ""
        editorDelegate.flushSaveAsync()
        _uiState.update {
            it.copy(
                selectedScriptId = uiScriptId,
                currentScreen = StudioScreen.ScriptStudio,
                scriptValidationMsg = null,
                statusBanner = null
            )
        }
    }

    fun addScript(language: String) = editorDelegate.addScript(language)

    fun updateScript(scriptId: String, enabled: Boolean, sourceCode: String, silent: Boolean = true) =
        editorDelegate.updateScript(scriptId, enabled, sourceCode, silent)

    fun validateScript(scriptId: String, currentEnabled: Boolean, currentSource: String) =
        editorDelegate.validateScript(scriptId, currentEnabled, currentSource)

    fun restoreDefaultArrowSvg() = editorDelegate.restoreDefaultArrowSvg()

    fun startCompileAndPlay() = gameplayCoordinator.startCompileAndPlay()

    suspend fun stepGameplayFrame(dtSec: Float, pressedMask: Int, justPressedMask: Int) =
        gameplayCoordinator.stepGameplayFrame(dtSec, pressedMask, justPressedMask)

    fun setGameplayPaused(paused: Boolean) = gameplayCoordinator.setGameplayPaused(paused)

    fun restartGameplay() = gameplayCoordinator.restartGameplay()

    fun triggerGameOver() = gameplayCoordinator.triggerGameOver()

    fun confirmGameOverRetry() = gameplayCoordinator.confirmGameOverRetry()

    fun setGameplayDebugOptions(
        showRam: Boolean = _uiState.value.debugShowRam,
        showTelemetry: Boolean = _uiState.value.debugShowTelemetry,
        botplay: Boolean = _uiState.value.debugBotplay,
        practiceMode: Boolean = _uiState.value.debugPracticeMode,
        hideHud: Boolean = _uiState.value.debugHideHud
    ) = gameplayCoordinator.setGameplayDebugOptions(
        showRam,
        showTelemetry,
        botplay,
        practiceMode,
        hideHud
    )

    fun stopGameplayAndNavigate(target: StudioScreen = StudioScreen.Hub) =
        gameplayCoordinator.stopGameplayAndNavigate(target)
}
