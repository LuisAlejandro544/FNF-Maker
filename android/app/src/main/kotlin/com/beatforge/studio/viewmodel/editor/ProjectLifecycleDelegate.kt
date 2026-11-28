// Archivo: android/app/src/main/kotlin/com/beatforge/studio/viewmodel/editor/ProjectLifecycleDelegate.kt
// Propósito: Subdelegado del ciclo de vida del motor y proyectos en `engineIoDispatcher`:
// inicialización JNI, cuota `StorageManager`, apertura/creación/duplicado/borrado de mods,
// retorno al lobby, guardado diferido, metadatos y preferencia de backend gráfico.

package com.beatforge.studio.viewmodel.editor

import android.os.storage.StorageManager
import com.beatforge.studio.NativeEngineBridge
import com.beatforge.studio.SafFileBridge
import com.beatforge.studio.StudioJsonParser
import com.beatforge.studio.StudioScreen
import com.beatforge.studio.gfx.RenderBackendKind
import com.beatforge.studio.gfx.VulkanProbeBridge
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

internal class ProjectLifecycleDelegate(
    private val helper: EditorStateHelper
) {
    fun initializeEngine() {
        helper.scope.launch(helper.engineIoDispatcher) {
            NativeEngineBridge.ensureLoaded()
            SafFileBridge.sweepStaleStagingFiles(helper.app, maxAgeMs = 0L)
            NativeEngineBridge.nativeSetCacheQuota(resolveRecommendedCacheQuotaBytes())
            val cacheDir = helper.app.cacheDir.absolutePath
            val dataDir = helper.app.filesDir.absolutePath
            val rawSnapshot = NativeEngineBridge.nativeInit(cacheDir, dataDir)
            val snap = StudioJsonParser.parseProjectSnapshot(rawSnapshot)
            val projects = StudioJsonParser.parseProjectList(NativeEngineBridge.nativeListProjects())
            val arrows = helper.bitmapLoader.loadArrowBitmaps()
            val receptors = helper.bitmapLoader.loadReceptorBitmaps()
            val hurtX = helper.bitmapLoader.loadHurtArrowXBitmap()
            val hudAssets = helper.bitmapLoader.loadHudSvgAssets()
            val cdBmps = helper.bitmapLoader.loadCountdownBitmaps(snap.countdownSlots, helper.uiState.value.selectedTier)
            val gfxCaps = VulkanProbeBridge.probeCombinedCapabilities(helper.app)

            helper.uiState.update { state ->
                state.copy(
                    initialized = true,
                    hasSelectedProject = false,
                    currentScreen = StudioScreen.Projects,
                    snapshot = snap,
                    projects = projects,
                    gfxCapabilities = gfxCaps,
                    selectedEntityId = snap.stageEntities.firstOrNull()?.id ?: "",
                    selectedScriptId = snap.scripts.firstOrNull()?.id ?: "",
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
                    defaultOpponentLosingIcon = hudAssets.opponentLosing
                )
            }
        }
    }

    private fun resolveRecommendedCacheQuotaBytes(): Long {
        // minSdk = 26 (Android 8.0+): StorageManager.getCacheQuotaBytes asigna cuota automática por sistema.
        return try {
            val sm = helper.app.getSystemService(StorageManager::class.java)
            if (sm != null) {
                val uuid = sm.getUuidForPath(helper.app.cacheDir)
                val quota = sm.getCacheQuotaBytes(uuid)
                if (quota > 0L) return quota
            }
            (helper.app.cacheDir.usableSpace * 0.12).toLong().coerceAtLeast(48L * 1024L * 1024L)
        } catch (_: Exception) {
            (helper.app.cacheDir.usableSpace * 0.12).toLong().coerceAtLeast(48L * 1024L * 1024L)
        }
    }

    fun flushSave() {
        helper.scope.launch(helper.engineIoDispatcher) {
            NativeEngineBridge.nativeFlushSave()
            helper.refreshProjectsInternal()
        }
    }

    fun refreshProjects() {
        helper.scope.launch(helper.engineIoDispatcher) {
            NativeEngineBridge.nativeFlushSave()
            helper.refreshProjectsInternal()
        }
    }

    fun createProject(name: String, author: String) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.setBusy(true)
            if (helper.uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            helper.beginLoadingOverlay(name, 0.22f, "Creando carpeta del proyecto...")
            val raw = NativeEngineBridge.nativeCreateProject(name, author)
            helper.applyProjectSwitchResult(raw, navigateToHub = true)
            helper.endLoadingOverlay()
            helper.setBusy(false)
        }
    }

    fun openProject(projectId: String) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.setBusy(true)
            if (helper.uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            val projName = helper.uiState.value.projects.firstOrNull { it.id == projectId }?.name ?: "Mod BeatForge"
            helper.beginLoadingOverlay(projName, 0.18f, "Abriendo las notas y la música...")
            val raw = NativeEngineBridge.nativeOpenProject(projectId)
            helper.applyProjectSwitchResult(raw, navigateToHub = true)
            helper.endLoadingOverlay()
            helper.setBusy(false)
        }
    }

    fun returnToProjectMenu() {
        helper.scope.launch(helper.engineIoDispatcher) {
            if (helper.uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            NativeEngineBridge.nativeFlushSave()
            helper.refreshProjectsInternal()
            helper.uiState.update {
                it.copy(
                    hasSelectedProject = false,
                    isLoadingProject = false,
                    audioPreviewPlaying = false,
                    currentScreen = StudioScreen.Projects,
                    statusBanner = null
                )
            }
        }
    }

    fun setPreferredRenderBackend(kind: RenderBackendKind) {
        helper.scope.launch(helper.engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeSetPreferredRenderBackend(kind.key)
            helper.applyActionJson(raw)
            val caps = VulkanProbeBridge.probeCombinedCapabilities(helper.app)
            helper.uiState.update { it.copy(gfxCapabilities = caps) }
        }
    }

    fun upgradeProjectDevApi() {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.setBusy(true)
            val raw = NativeEngineBridge.nativeUpgradeProjectDevApi()
            helper.applyActionJson(raw)
            helper.refreshProjectsInternal()
            helper.uiState.update { it.copy(devApiDialogOpen = false, isBusy = false) }
        }
    }

    fun duplicateProject(projectId: String) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.setBusy(true)
            if (helper.uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            val raw = NativeEngineBridge.nativeDuplicateProject(projectId)
            helper.applyProjectSwitchResult(raw, navigateToHub = false)
            helper.setBusy(false)
        }
    }

    fun deleteProject(projectId: String) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.setBusy(true)
            val raw = NativeEngineBridge.nativeDeleteProject(projectId)
            helper.applyProjectSwitchResult(raw, navigateToHub = false)
            helper.setBusy(false)
        }
    }

    fun flushAndMaybeRefreshProjects(screen: StudioScreen) {
        helper.scope.launch(helper.engineIoDispatcher) {
            NativeEngineBridge.nativeFlushSave()
            if (screen == StudioScreen.Projects) {
                helper.refreshProjectsInternal()
            }
        }
    }

    fun flushSaveAsync() {
        helper.scope.launch(helper.engineIoDispatcher) {
            NativeEngineBridge.nativeFlushSave()
        }
    }

    fun updateProjectMeta(
        name: String,
        author: String,
        downscroll: Boolean,
        controlMode: String,
        audioOffsetMs: Double,
        scrollSpeed: Double
    ) {
        helper.scope.launch(helper.engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeUpdateProjectMeta(
                name,
                author,
                downscroll,
                controlMode,
                audioOffsetMs,
                scrollSpeed
            )
            helper.applyActionJson(raw, silentOk = true)
            if (helper.uiState.value.currentScreen == StudioScreen.Gameplay) {
                val gpRaw = NativeEngineBridge.nativeStepGameplay(0f, 0, 0)
                val gpStep = StudioJsonParser.parseGameplayStep(gpRaw)
                withContext(Dispatchers.Main) {
                    helper.gameplayFrameState.seedFromJsonStep(gpStep)
                }
            }
        }
    }
}
