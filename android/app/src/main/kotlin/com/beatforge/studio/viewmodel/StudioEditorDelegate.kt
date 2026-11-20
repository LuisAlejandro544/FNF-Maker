// Archivo: android/app/src/main/kotlin/com/beatforge/studio/viewmodel/StudioEditorDelegate.kt
// Propósito: Delegado modular de `StudioViewModel` encargado de toda la edición del estudio
// en `engineIoDispatcher`: inicialización del motor Rust, gestión de proyectos, importación/
// exportación SAF, edición de chart, mezclador de audio, bindings Sparrow, escenario y scripts.

package com.beatforge.studio.viewmodel

import android.app.Application
import android.net.Uri
import android.os.storage.StorageManager
import com.beatforge.studio.AudioPreviewTransportState
import com.beatforge.studio.CAMERA_SELECTION_ID
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.ImportKind
import com.beatforge.studio.NativeEngineBridge
import com.beatforge.studio.SafFileBridge
import com.beatforge.studio.SparrowFrameInfo
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageCameraSlotSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.StudioJsonParser
import com.beatforge.studio.StudioScreen
import com.beatforge.studio.StudioUiState
import com.beatforge.studio.gfx.RenderBackendKind
import com.beatforge.studio.gfx.VulkanProbeBridge
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

internal class StudioEditorDelegate(
    private val app: Application,
    private val scope: CoroutineScope,
    private val engineIoDispatcher: CoroutineDispatcher,
    private val uiState: MutableStateFlow<StudioUiState>,
    private val gameplayFrameState: GameplayFrameState,
    private val bitmapLoader: StudioBitmapLoader
) {
    private val audioStateBuf = DoubleArray(3)
    private val fastAudioStateBuf = DoubleArray(3)

    fun initializeEngine() {
        scope.launch(engineIoDispatcher) {
            NativeEngineBridge.ensureLoaded()
            SafFileBridge.sweepStaleStagingFiles(app, maxAgeMs = 0L)
            NativeEngineBridge.nativeSetCacheQuota(resolveRecommendedCacheQuotaBytes())
            val cacheDir = app.cacheDir.absolutePath
            val dataDir = app.filesDir.absolutePath
            val rawSnapshot = NativeEngineBridge.nativeInit(cacheDir, dataDir)
            val snap = StudioJsonParser.parseProjectSnapshot(rawSnapshot)
            val projects = StudioJsonParser.parseProjectList(NativeEngineBridge.nativeListProjects())
            val arrows = bitmapLoader.loadArrowBitmaps()
            val receptors = bitmapLoader.loadReceptorBitmaps()
            val hurtX = bitmapLoader.loadHurtArrowXBitmap()
            val hudAssets = bitmapLoader.loadHudSvgAssets()
            val gfxCaps = VulkanProbeBridge.probeCombinedCapabilities(app)

            uiState.update { state ->
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
                    defaultPlayerNormalIcon = hudAssets.playerNormal,
                    defaultPlayerLosingIcon = hudAssets.playerLosing,
                    defaultOpponentNormalIcon = hudAssets.opponentNormal,
                    defaultOpponentLosingIcon = hudAssets.opponentLosing
                )
            }
        }
    }

    fun exportChartToUri(uri: Uri) {
        scope.launch(engineIoDispatcher) {
            setBusy(true)
            try {
                val jsonText = NativeEngineBridge.nativeExportChartJson()
                val os = app.contentResolver.openOutputStream(uri)
                if (os == null) {
                    postBanner("No se pudo abrir destino para exportar chart.json", isError = true)
                } else {
                    os.use { stream ->
                        stream.write(jsonText.toByteArray(Charsets.UTF_8))
                        stream.flush()
                    }
                    postBanner("chart.json completo exportado correctamente", isError = false)
                }
            } catch (e: Exception) {
                postBanner("Error exportando chart.json: ${e.message}", isError = true)
            }
            setBusy(false)
        }
    }

    fun exportModPackageToUri(projectId: String, uri: Uri) {
        scope.launch(engineIoDispatcher) {
            setBusy(true)
            val stagedOut = SafFileBridge.createExportStagingFile(app, "vanilla_export.mod")
            val stagedBase = stagedOut.nameWithoutExtension
            try {
                val rawJson = NativeEngineBridge.nativeExportModPackage(
                    projectId = projectId,
                    outPath = stagedOut.absolutePath,
                    optimizeImages = true
                )
                val res = StudioJsonParser.parseActionResult(rawJson)
                if (!res.ok || !stagedOut.isFile) {
                    postBanner(
                        res.message.ifBlank { "No se pudo empaquetar el mod en Vanilla .mod" },
                        isError = true
                    )
                } else {
                    val os = app.contentResolver.openOutputStream(uri)
                    if (os == null) {
                        postBanner("No se pudo abrir el destino para guardar el archivo .mod", isError = true)
                    } else {
                        os.use { output ->
                            stagedOut.inputStream().use { input ->
                                input.copyTo(output, bufferSize = 64 * 1024)
                            }
                            output.flush()
                        }
                        postBanner(res.message, isError = false)
                    }
                }
            } catch (e: Exception) {
                postBanner("Error exportando Vanilla .mod: ${e.message}", isError = true)
            } finally {
                SafFileBridge.deleteStagedFile(app, stagedOut.absolutePath)
                stagedOut.parent?.let { dir ->
                    SafFileBridge.deleteStagedFile(app, "$dir/$stagedBase.mod.blobs.tmp")
                    SafFileBridge.deleteStagedFile(app, "$dir/$stagedBase.mod.tmp")
                }
                setBusy(false)
            }
        }
    }

    private fun resolveRecommendedCacheQuotaBytes(): Long {
        // minSdk = 26 (Android 8.0+): StorageManager.getCacheQuotaBytes asigna cuota automática por sistema.
        return try {
            val sm = app.getSystemService(StorageManager::class.java)
            if (sm != null) {
                val uuid = sm.getUuidForPath(app.cacheDir)
                val quota = sm.getCacheQuotaBytes(uuid)
                if (quota > 0L) return quota
            }
            (app.cacheDir.usableSpace * 0.12).toLong().coerceAtLeast(48L * 1024L * 1024L)
        } catch (_: Exception) {
            (app.cacheDir.usableSpace * 0.12).toLong().coerceAtLeast(48L * 1024L * 1024L)
        }
    }

    fun importUri(kind: ImportKind, uri: Uri) {
        scope.launch(engineIoDispatcher) {
            setBusy(true)
            val localPath = SafFileBridge.copyUriToStagingCache(app, uri, kind.tag)
            if (localPath == null) {
                postBanner("No se pudo copiar el archivo seleccionado", isError = true)
                setBusy(false)
                return@launch
            }

            var deleteLocalInFinally = true
            try {
                when (kind) {
                    is ImportKind.ModPackage -> {
                        if (uiState.value.audioPreviewPlaying) {
                            NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
                        }
                        beginLoadingOverlay("Importando paquete .mod...", 0.25f, "Descomprimiendo mod Vanilla...")
                        val rawJson = NativeEngineBridge.nativeImportModPackage(localPath)
                        applyProjectSwitchResult(rawJson, navigateToHub = true)
                    }

                    is ImportKind.ArrowAtlasPng -> {
                        deleteLocalInFinally = false
                        uiState.value.stagedArrowAtlasPngPath?.let { old ->
                            SafFileBridge.deleteStagedFile(app, old)
                        }
                        uiState.update { it.copy(stagedArrowAtlasPngPath = localPath) }
                        maybeApplyStagedArrowAtlas()
                    }

                    is ImportKind.ArrowAtlasXml -> {
                        deleteLocalInFinally = false
                        uiState.value.stagedArrowAtlasXmlPath?.let { old ->
                            SafFileBridge.deleteStagedFile(app, old)
                        }
                        uiState.update { it.copy(stagedArrowAtlasXmlPath = localPath) }
                        maybeApplyStagedArrowAtlas()
                    }

                    else -> {
                        val rawJson = when (kind) {
                            is ImportKind.Chart -> NativeEngineBridge.nativeImportChart(localPath)
                            is ImportKind.AudioTrack -> NativeEngineBridge.nativeImportAudioTrack(kind.slot, localPath)
                            is ImportKind.SpritePng -> NativeEngineBridge.nativeImportSpriteSheetPng(kind.entityId, localPath)
                            is ImportKind.SparrowXml -> NativeEngineBridge.nativeImportSparrowXml(kind.entityId, localPath)
                            is ImportKind.CharacterConfig -> NativeEngineBridge.nativeImportCharacterConfig(kind.entityId, localPath)
                            is ImportKind.StageConfig -> NativeEngineBridge.nativeImportStageConfig(localPath)
                            is ImportKind.Script -> NativeEngineBridge.nativeImportScript(localPath)
                            is ImportKind.ArrowSvg -> NativeEngineBridge.nativeImportArrowSvg(kind.direction, localPath)
                            is ImportKind.CharacterIcon -> NativeEngineBridge.nativeImportCharacterIcon(kind.entityId, kind.isLosing, localPath)
                            is ImportKind.HealthBarSvg -> NativeEngineBridge.nativeImportHealthBarSvg(localPath)
                            is ImportKind.ModPackage,
                            is ImportKind.ArrowAtlasPng,
                            is ImportKind.ArrowAtlasXml -> error("handled above")
                        }

                        applyActionJson(rawJson)
                        when (kind) {
                            is ImportKind.ArrowSvg -> reloadArrowBitmaps()
                            is ImportKind.HealthBarSvg -> reloadHealthBarBitmap()
                            is ImportKind.CharacterIcon -> reloadCustomIcons(uiState.value.snapshot.stageEntities)
                            is ImportKind.SpritePng, is ImportKind.SparrowXml, is ImportKind.CharacterConfig, is ImportKind.StageConfig -> {
                                bitmapLoader.evictRegionCache()
                                reloadEntityBitmaps(uiState.value.snapshot.stageEntities)
                            }
                            else -> Unit
                        }
                        refreshProjectsInternal()
                    }
                }
            } finally {
                if (deleteLocalInFinally) {
                    SafFileBridge.deleteStagedFile(app, localPath)
                }
                endLoadingOverlay()
                setBusy(false)
            }
        }
    }

    private fun maybeApplyStagedArrowAtlas() {
        val st = uiState.value
        val pngPath = st.stagedArrowAtlasPngPath
        val xmlPath = st.stagedArrowAtlasXmlPath
        if (pngPath != null && xmlPath != null && File(pngPath).isFile && File(xmlPath).isFile) {
            val rawJson = NativeEngineBridge.nativeImportArrowAtlas(pngPath, xmlPath)
            applyActionJson(rawJson)
            reloadArrowBitmaps()
            SafFileBridge.deleteStagedFile(app, pngPath)
            SafFileBridge.deleteStagedFile(app, xmlPath)
            uiState.update {
                it.copy(stagedArrowAtlasPngPath = null, stagedArrowAtlasXmlPath = null)
            }
            refreshProjectsInternal()
        } else if (pngPath != null && xmlPath == null) {
            postBanner("PNG de flechas listo. Ahora selecciona el archivo .xml (NOTE_assets.xml).", isError = false)
        } else if (xmlPath != null && pngPath == null) {
            postBanner("XML de flechas listo. Ahora selecciona la hoja .png (NOTE_assets.png).", isError = false)
        }
    }

    fun clearStagedArrowAtlas() {
        val st = uiState.value
        st.stagedArrowAtlasPngPath?.let { SafFileBridge.deleteStagedFile(app, it) }
        st.stagedArrowAtlasXmlPath?.let { SafFileBridge.deleteStagedFile(app, it) }
        uiState.update { it.copy(stagedArrowAtlasPngPath = null, stagedArrowAtlasXmlPath = null) }
    }

    fun flushSave() {
        scope.launch(engineIoDispatcher) {
            NativeEngineBridge.nativeFlushSave()
            refreshProjectsInternal()
        }
    }

    fun refreshProjects() {
        scope.launch(engineIoDispatcher) {
            NativeEngineBridge.nativeFlushSave()
            refreshProjectsInternal()
        }
    }

    fun createProject(name: String, author: String) {
        scope.launch(engineIoDispatcher) {
            setBusy(true)
            if (uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            beginLoadingOverlay(name, 0.22f, "Creando estructura del mod...")
            val raw = NativeEngineBridge.nativeCreateProject(name, author)
            applyProjectSwitchResult(raw, navigateToHub = true)
            endLoadingOverlay()
            setBusy(false)
        }
    }

    fun openProject(projectId: String) {
        scope.launch(engineIoDispatcher) {
            setBusy(true)
            if (uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            val projName = uiState.value.projects.firstOrNull { it.id == projectId }?.name ?: "Mod BeatForge"
            beginLoadingOverlay(projName, 0.18f, "Cargando proyecto y chart.json...")
            val raw = NativeEngineBridge.nativeOpenProject(projectId)
            applyProjectSwitchResult(raw, navigateToHub = true)
            endLoadingOverlay()
            setBusy(false)
        }
    }

    fun returnToProjectMenu() {
        scope.launch(engineIoDispatcher) {
            if (uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            NativeEngineBridge.nativeFlushSave()
            refreshProjectsInternal()
            uiState.update {
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
        scope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeSetPreferredRenderBackend(kind.key)
            applyActionJson(raw)
            val caps = VulkanProbeBridge.probeCombinedCapabilities(app)
            uiState.update { it.copy(gfxCapabilities = caps) }
        }
    }

    fun duplicateProject(projectId: String) {
        scope.launch(engineIoDispatcher) {
            setBusy(true)
            if (uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            val raw = NativeEngineBridge.nativeDuplicateProject(projectId)
            applyProjectSwitchResult(raw, navigateToHub = false)
            setBusy(false)
        }
    }

    fun deleteProject(projectId: String) {
        scope.launch(engineIoDispatcher) {
            setBusy(true)
            val raw = NativeEngineBridge.nativeDeleteProject(projectId)
            applyProjectSwitchResult(raw, navigateToHub = false)
            setBusy(false)
        }
    }

    fun flushAndMaybeRefreshProjects(screen: StudioScreen) {
        scope.launch(engineIoDispatcher) {
            NativeEngineBridge.nativeFlushSave()
            if (screen == StudioScreen.Projects) {
                refreshProjectsInternal()
            }
        }
    }

    fun flushSaveAsync() {
        scope.launch(engineIoDispatcher) {
            NativeEngineBridge.nativeFlushSave()
        }
    }

    fun clearCharacterIcons(entityId: String) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeClearCharacterIcons(entityId))
            reloadCustomIcons(uiState.value.snapshot.stageEntities)
        }
    }

    fun restoreDefaultHealthBarSvg() {
        scope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeRestoreDefaultHealthBarSvg())
            reloadHealthBarBitmap()
        }
    }

    fun restoreDefaultArrowSvg() {
        scope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeRestoreDefaultArrowSvg())
            reloadArrowBitmaps()
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
        scope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeUpdateProjectMeta(
                name,
                author,
                downscroll,
                controlMode,
                audioOffsetMs,
                scrollSpeed
            )
            applyActionJson(raw, silentOk = true)
            if (uiState.value.currentScreen == StudioScreen.Gameplay) {
                val gpRaw = NativeEngineBridge.nativeStepGameplay(0f, 0, 0)
                val gpStep = StudioJsonParser.parseGameplayStep(gpRaw)
                withContext(Dispatchers.Main) {
                    gameplayFrameState.seedFromJsonStep(gpStep)
                }
            }
        }
    }

    fun restoreDemoChart() {
        scope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeRestoreDemoChart())
        }
    }

    fun setChartBpm(bpm: Double) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeSetChartBpm(bpm), silentOk = true)
        }
    }

    fun updateChartDetails(
        title: String,
        speed: Double,
        player1: String,
        player2: String,
        gfVersion: String,
        stage: String,
        needsVoices: Boolean
    ) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(
                NativeEngineBridge.nativeUpdateChartDetails(
                    title,
                    speed,
                    player1,
                    player2,
                    gfVersion,
                    stage,
                    needsVoices
                )
            )
        }
    }

    fun updateChartIntroUi(
        startBlackout: Boolean,
        blackoutAlpha: Float,
        hideHud: Boolean,
        hideArrows: Boolean,
        revealOnSongStart: Boolean,
        revealDelayMs: Double,
        fadeSec: Float
    ) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(
                NativeEngineBridge.nativeUpdateChartIntroUi(
                    startBlackout,
                    blackoutAlpha,
                    hideHud,
                    hideArrows,
                    revealOnSongStart,
                    revealDelayMs,
                    fadeSec
                )
            )
        }
    }

    fun setSectionBpm(secIndex: Int, bpm: Double, changeBpm: Boolean) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(
                NativeEngineBridge.nativeSetSectionBpm(secIndex, bpm, changeBpm),
                silentOk = true
            )
        }
    }

    fun addChartEvent(timeMs: Double, name: String, value1: String, value2: String) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(
                NativeEngineBridge.nativeAddChartEvent(timeMs, name, value1, value2)
            )
        }
    }

    fun removeChartEvent(index: Int) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeRemoveChartEvent(index), silentOk = true)
        }
    }

    fun addChartSection(mustHit: Boolean) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeAddChartSection(mustHit))
        }
    }

    fun toggleSectionMustHit(secIndex: Int) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeToggleSectionMustHit(secIndex), silentOk = true)
        }
    }

    fun setSectionCameraSlot(secIndex: Int, cameraSlot: Int?) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(
                NativeEngineBridge.nativeSetSectionCameraSlot(secIndex, cameraSlot ?: -1),
                silentOk = true
            )
        }
    }

    fun toggleChartNote(
        secIndex: Int,
        stepInSec: Int,
        col0To7: Int,
        sustainSteps: Int,
        noteType: String = "default"
    ) {
        scope.launch(engineIoDispatcher) {
            val raw = if (noteType == "default") {
                NativeEngineBridge.nativeToggleChartNote(secIndex, stepInSec, col0To7, sustainSteps)
            } else {
                NativeEngineBridge.nativeToggleChartNoteWithType(secIndex, stepInSec, col0To7, sustainSteps, noteType)
            }
            applyActionJson(raw, silentOk = true)
        }
    }

    fun removeAudioTrack(slot: Int) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeRemoveAudioTrack(slot))
        }
    }

    fun updateAudioTrackConfig(slot: Int, gain: Float, muted: Boolean, duckOnMiss: Boolean) {
        scope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeUpdateAudioTrackConfig(slot, gain, muted, duckOnMiss)
            applyActionJson(raw, silentOk = true)
        }
    }

    fun toggleAudioPreview(startMs: Double) {
        scope.launch(engineIoDispatcher) {
            val nextPlay = !uiState.value.audioPreviewPlaying
            val playing = NativeEngineBridge.nativeToggleAudioPreview(nextPlay, startMs)
            uiState.update { it.copy(audioPreviewPlaying = playing) }
        }
    }

    fun setAudioPreviewTransport(play: Boolean, seekMs: Double = -1.0) {
        scope.launch(engineIoDispatcher) {
            val playing = NativeEngineBridge.nativeSetAudioPreviewTransport(play, seekMs)
            uiState.update { it.copy(audioPreviewPlaying = playing) }
        }
    }

    suspend fun readAudioPreviewState(): AudioPreviewTransportState {
        return withContext(engineIoDispatcher) {
            val ok = NativeEngineBridge.nativeReadAudioPreviewState(audioStateBuf)
            if (!ok) {
                AudioPreviewTransportState()
            } else {
                val posMs = audioStateBuf[0].coerceAtLeast(0.0)
                val playing = audioStateBuf[1] >= 0.5
                val durationMs = audioStateBuf[2].coerceAtLeast(0.0)
                if (uiState.value.audioPreviewPlaying != playing) {
                    uiState.update { it.copy(audioPreviewPlaying = playing) }
                }
                AudioPreviewTransportState(
                    posMs = posMs,
                    isPlaying = playing,
                    durationMs = durationMs
                )
            }
        }
    }

    // Lectura directa lock-free desde átomos de Rust sin saltar al dispatcher de E/S ni bloquear mutex a 60 Hz.
    fun readAudioPreviewStateFast(): AudioPreviewTransportState {
        val ok = NativeEngineBridge.nativeReadAudioPreviewState(fastAudioStateBuf)
        if (!ok) return AudioPreviewTransportState()
        val posMs = fastAudioStateBuf[0].coerceAtLeast(0.0)
        val playing = fastAudioStateBuf[1] >= 0.5
        val durationMs = fastAudioStateBuf[2].coerceAtLeast(0.0)
        if (uiState.value.audioPreviewPlaying != playing) {
            uiState.update { it.copy(audioPreviewPlaying = playing) }
        }
        return AudioPreviewTransportState(
            posMs = posMs,
            isPlaying = playing,
            durationMs = durationMs
        )
    }

    fun setSparrowBinding(entityId: String, animKind: String, prefix: String) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeSetSparrowBinding(entityId, animKind, prefix))
        }
    }

    fun setEntityAnimOffset(entityId: String, animKind: String, offsetX: Float, offsetY: Float) {
        val ox = offsetX.coerceIn(-500f, 500f)
        val oy = offsetY.coerceIn(-500f, 500f)
        uiState.update { state ->
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
        scope.launch(engineIoDispatcher) {
            applyActionJson(
                NativeEngineBridge.nativeSetEntityAnimOffset(entityId, animKind, ox, oy),
                silentOk = true
            )
        }
    }

    suspend fun querySparrowFrame(entityId: String, animKind: String, elapsedSec: Float): SparrowFrameInfo {
        return withContext(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeGetSparrowFrameInfo(entityId, animKind, elapsedSec)
            StudioJsonParser.parseSparrowFrameInfo(raw)
        }
    }

    fun addStageEntity(name: String, role: String) {
        scope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeAddStageEntity(name, role)
            applyActionJson(raw)
            val newest = uiState.value.snapshot.stageEntities.lastOrNull()?.id
            if (!newest.isNullOrBlank()) {
                uiState.update { it.copy(selectedEntityId = newest) }
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
        uiState.update { state ->
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

        scope.launch(engineIoDispatcher) {
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
                applyActionJson(raw, silentOk = true)
            }
        }
    }

    fun reorderStageEntity(entityId: String, action: String) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeReorderStageEntity(entityId, action), silentOk = true)
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

        uiState.update { state ->
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
                    stageCamera = StageCameraSnapshot(
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

        scope.launch(engineIoDispatcher) {
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
                applyActionJson(raw, silentOk = true)
            }
        }
    }

    fun selectStageCamera(slotIndex: Int) {
        uiState.update { state ->
            val cam = state.snapshot.stageCamera
            val maxIdx = (cam.resolvedCameras().size - 1).coerceAtLeast(0)
            state.copy(
                snapshot = state.snapshot.copy(
                    stageCamera = cam.copy(activeCameraIndex = slotIndex.coerceIn(0, maxIdx))
                )
            )
        }
        scope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeSelectStageCamera(slotIndex), silentOk = true)
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

        uiState.update { state ->
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

        scope.launch(engineIoDispatcher) {
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
                applyActionJson(raw, silentOk = true)
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
        scope.launch(engineIoDispatcher) {
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
            applyActionJson(raw)
        }
    }

    fun removeCustomCamera(slotIndex: Int) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeRemoveCustomCamera(slotIndex))
        }
    }

    fun addScript(language: String) {
        scope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeAddScript(language)
            applyActionJson(raw)
            val newest = uiState.value.snapshot.scripts.lastOrNull()?.id
            if (!newest.isNullOrBlank()) {
                uiState.update { it.copy(selectedScriptId = newest, scriptValidationMsg = null) }
            }
        }
    }

    fun updateScript(scriptId: String, enabled: Boolean, sourceCode: String, silent: Boolean) {
        scope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeUpdateScript(scriptId, enabled, sourceCode)
            applyActionJson(raw, silentOk = silent)
        }
    }

    fun validateScript(scriptId: String, currentEnabled: Boolean, currentSource: String) {
        scope.launch(engineIoDispatcher) {
            NativeEngineBridge.nativeUpdateScript(scriptId, currentEnabled, currentSource)
            val res = StudioJsonParser.parseValidationResult(
                NativeEngineBridge.nativeValidateScript(scriptId)
            )
            val snap = StudioJsonParser.parseProjectSnapshot(NativeEngineBridge.nativeGetSnapshot())
            uiState.update {
                it.copy(
                    snapshot = snap,
                    scriptValidationOk = res.ok,
                    scriptValidationMsg = res.message
                )
            }
        }
    }

    private fun beginLoadingOverlay(projectName: String, progress: Float, phase: String) {
        uiState.update {
            it.copy(
                isLoadingProject = true,
                loadingProjectName = projectName,
                loadingProgress = progress.coerceIn(0f, 1f),
                loadingPhase = phase
            )
        }
    }

    private fun updateLoadingOverlay(progress: Float, phase: String) {
        uiState.update {
            it.copy(
                loadingProgress = progress.coerceIn(0f, 1f),
                loadingPhase = phase
            )
        }
    }

    private fun endLoadingOverlay() {
        uiState.update {
            it.copy(isLoadingProject = false, loadingProgress = 1f)
        }
    }

    private fun applyProjectSwitchResult(rawJson: String, navigateToHub: Boolean) {
        val result = StudioJsonParser.parseActionResult(rawJson)
        val nextSnap = result.snapshot ?: uiState.value.snapshot
        if (uiState.value.isLoadingProject) {
            updateLoadingOverlay(0.48f, "Cargando skins de flechas y HUD SVG...")
        }
        bitmapLoader.evictRegionCache()
        val arrows = bitmapLoader.loadArrowBitmaps()
        val receptors = bitmapLoader.loadReceptorBitmaps()
        val hurtX = bitmapLoader.loadHurtArrowXBitmap()
        val hudAssets = bitmapLoader.loadHudSvgAssets()
        if (uiState.value.isLoadingProject) {
            updateLoadingOverlay(0.72f, "Decodificando sprites e íconos del escenario...")
        }
        val (normalIcons, losingIcons) = bitmapLoader.loadEntityIcons(nextSnap.stageEntities)
        val entBmps = bitmapLoader.loadEditorEntityBitmaps(
            nextSnap.stageEntities,
            uiState.value.selectedTier
        )
        if (uiState.value.isLoadingProject) {
            updateLoadingOverlay(0.92f, "Verificando drivers OpenGL ES / Vulkan...")
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

    private fun refreshProjectsInternal() {
        val list = StudioJsonParser.parseProjectList(NativeEngineBridge.nativeListProjects())
        uiState.update { it.copy(projects = list) }
    }

    private fun applyActionJson(rawJson: String, silentOk: Boolean = false) {
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

    private fun setBusy(busy: Boolean) {
        uiState.update { it.copy(isBusy = busy) }
    }

    private fun postBanner(msg: String, isError: Boolean) {
        uiState.update { it.copy(statusBanner = msg, statusIsError = isError) }
    }

    private fun reloadArrowBitmaps() {
        val arrows = bitmapLoader.loadArrowBitmaps()
        val receptors = bitmapLoader.loadReceptorBitmaps()
        val hurtX = bitmapLoader.loadHurtArrowXBitmap()
        uiState.update { it.copy(arrowBitmaps = arrows, receptorBitmaps = receptors, hurtArrowXBitmap = hurtX) }
    }

    private fun reloadHealthBarBitmap() {
        val bmp = bitmapLoader.loadHealthBarBitmap()
        uiState.update { it.copy(healthBarBitmap = bmp) }
    }

    private fun reloadCustomIcons(entities: List<StageEntitySnapshot>) {
        val (normalIcons, losingIcons) = bitmapLoader.loadEntityIcons(entities)
        uiState.update { it.copy(entityNormalIcons = normalIcons, entityLosingIcons = losingIcons) }
    }

    private fun reloadEntityBitmaps(entities: List<StageEntitySnapshot>) {
        val map = bitmapLoader.loadEditorEntityBitmaps(entities, uiState.value.selectedTier)
        uiState.update { it.copy(entityBitmaps = map) }
    }
}
