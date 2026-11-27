// Archivo: android/app/src/main/kotlin/com/beatforge/studio/viewmodel/editor/EditorStateHelper.kt
// Propósito: Utilidades internas compartidas por los delegados de edición en `viewmodel/editor/`:
// sincronización de overlays de carga, aplicación de resultados JSON desde Rust,
// actualización de banners y recarga selectiva de bitmaps en `Dispatchers.IO`.

package com.beatforge.studio.viewmodel.editor

import android.app.Application
import com.beatforge.studio.CAMERA_SELECTION_ID
import com.beatforge.studio.CountdownStepSnapshot
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.NativeEngineBridge
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.StudioJsonParser
import com.beatforge.studio.StudioScreen
import com.beatforge.studio.StudioUiState
import com.beatforge.studio.gfx.VulkanProbeBridge
import com.beatforge.studio.viewmodel.StudioBitmapLoader
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update

internal class EditorStateHelper(
    val app: Application,
    val scope: CoroutineScope,
    val engineIoDispatcher: CoroutineDispatcher,
    val uiState: MutableStateFlow<StudioUiState>,
    val gameplayFrameState: GameplayFrameState,
    val bitmapLoader: StudioBitmapLoader
) {
    fun beginLoadingOverlay(projectName: String, progress: Float, phase: String) {
        uiState.update {
            it.copy(
                isLoadingProject = true,
                loadingProjectName = projectName,
                loadingProgress = progress.coerceIn(0f, 1f),
                loadingPhase = phase
            )
        }
    }

    fun updateLoadingOverlay(progress: Float, phase: String) {
        uiState.update {
            it.copy(
                loadingProgress = progress.coerceIn(0f, 1f),
                loadingPhase = phase
            )
        }
    }

    fun endLoadingOverlay() {
        uiState.update {
            it.copy(isLoadingProject = false, loadingProgress = 1f)
        }
    }

    fun applyProjectSwitchResult(rawJson: String, navigateToHub: Boolean) {
        val result = StudioJsonParser.parseActionResult(rawJson)
        val nextSnap = result.snapshot ?: uiState.value.snapshot
        if (uiState.value.isLoadingProject) {
            updateLoadingOverlay(0.48f, "Cargando flechas e íconos...")
        }
        bitmapLoader.evictRegionCache()
        val arrows = bitmapLoader.loadArrowBitmaps()
        val receptors = bitmapLoader.loadReceptorBitmaps()
        val hurtX = bitmapLoader.loadHurtArrowXBitmap()
        val hudAssets = bitmapLoader.loadHudSvgAssets()
        if (uiState.value.isLoadingProject) {
            updateLoadingOverlay(0.72f, "Preparando personajes y escenario...")
        }
        val (normalIcons, losingIcons) = bitmapLoader.loadEntityIcons(nextSnap.stageEntities)
        val entBmps = bitmapLoader.loadEditorEntityBitmaps(
            nextSnap.stageEntities,
            uiState.value.selectedTier
        )
        val cdBmps = bitmapLoader.loadCountdownBitmaps(
            nextSnap.countdownSlots,
            uiState.value.selectedTier
        )
        if (uiState.value.isLoadingProject) {
            updateLoadingOverlay(0.92f, "Ajustando gráficos para tu teléfono...")
        }
        val gfxCaps = VulkanProbeBridge.probeCombinedCapabilities(app)
        val projects = StudioJsonParser.parseProjectList(NativeEngineBridge.nativeListProjects())

        uiState.update { state ->
            state.copy(
                hasSelectedProject = if (navigateToHub && result.ok) true else state.hasSelectedProject,
                currentScreen = if (navigateToHub && result.ok) StudioScreen.Hub else state.currentScreen,
                snapshot = nextSnap,
                projects = projects,
                gfxCapabilities = gfxCaps,
                audioPreviewPlaying = false,
                selectedEntityId = nextSnap.stageEntities.firstOrNull()?.id ?: "",
                selectedScriptId = nextSnap.scripts.firstOrNull()?.id ?: "",
                scriptValidationMsg = null,
                arrowBitmaps = arrows,
                receptorBitmaps = receptors,
                hurtArrowXBitmap = hurtX,
                healthBarBitmap = hudAssets.healthBar,
                ratingBitmaps = hudAssets.ratings,
                noteSplashBitmap = hudAssets.noteSplash,
                countdownBitmaps = cdBmps,
                defaultPlayerNormalIcon = hudAssets.playerNormal,
                defaultPlayerLosingIcon = hudAssets.playerLosing,
                defaultOpponentNormalIcon = hudAssets.opponentNormal,
                defaultOpponentLosingIcon = hudAssets.opponentLosing,
                entityNormalIcons = normalIcons,
                entityLosingIcons = losingIcons,
                entityBitmaps = entBmps,
                compiledBitmaps = emptyMap(),
                compiledBitmapsByIndex = emptyList(),
                statusBanner = result.message.takeIf { it.isNotBlank() },
                statusIsError = !result.ok
            )
        }
    }

    fun refreshProjectsInternal() {
        val list = StudioJsonParser.parseProjectList(NativeEngineBridge.nativeListProjects())
        uiState.update { it.copy(projects = list) }
    }

    fun applyActionJson(rawJson: String, silentOk: Boolean = false) {
        val result = StudioJsonParser.parseActionResult(rawJson)
        uiState.update { state ->
            val nextSnap = result.snapshot ?: state.snapshot
            val validEntity = state.selectedEntityId == CAMERA_SELECTION_ID ||
                nextSnap.stageEntities.any { it.id == state.selectedEntityId }
            val validScript = nextSnap.scripts.any { it.id == state.selectedScriptId }
            state.copy(
                snapshot = nextSnap,
                selectedEntityId = if (validEntity) state.selectedEntityId else (nextSnap.stageEntities.firstOrNull()?.id ?: ""),
                selectedScriptId = if (validScript) state.selectedScriptId else (nextSnap.scripts.firstOrNull()?.id ?: ""),
                statusBanner = if (!result.ok || !silentOk) result.message.takeIf { it.isNotBlank() } else state.statusBanner,
                statusIsError = !result.ok
            )
        }
    }

    fun setBusy(busy: Boolean) {
        uiState.update { it.copy(isBusy = busy) }
    }

    fun postBanner(msg: String, isError: Boolean) {
        uiState.update { it.copy(statusBanner = msg, statusIsError = isError) }
    }

    fun reloadArrowBitmaps() {
        val arrows = bitmapLoader.loadArrowBitmaps()
        val receptors = bitmapLoader.loadReceptorBitmaps()
        val hurtX = bitmapLoader.loadHurtArrowXBitmap()
        uiState.update { it.copy(arrowBitmaps = arrows, receptorBitmaps = receptors, hurtArrowXBitmap = hurtX) }
    }

    fun reloadHealthBarBitmap() {
        val bmp = bitmapLoader.loadHealthBarBitmap()
        uiState.update { it.copy(healthBarBitmap = bmp) }
    }

    fun reloadCustomIcons(entities: List<StageEntitySnapshot>) {
        val (normalIcons, losingIcons) = bitmapLoader.loadEntityIcons(entities)
        uiState.update { it.copy(entityNormalIcons = normalIcons, entityLosingIcons = losingIcons) }
    }

    fun reloadEntityBitmaps(entities: List<StageEntitySnapshot>) {
        val map = bitmapLoader.loadEditorEntityBitmaps(entities, uiState.value.selectedTier)
        uiState.update { it.copy(entityBitmaps = map) }
    }

    fun reloadCountdownBitmaps(slots: List<CountdownStepSnapshot>) {
        val map = bitmapLoader.loadCountdownBitmaps(slots, uiState.value.selectedTier)
        uiState.update { it.copy(countdownBitmaps = map) }
    }
}
