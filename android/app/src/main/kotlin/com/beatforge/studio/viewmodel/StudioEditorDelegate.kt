// Archivo: android/app/src/main/kotlin/com/beatforge/studio/viewmodel/StudioEditorDelegate.kt
// Propósito: Delegado modular de `StudioViewModel` encargado de toda la edición del estudio
// en `engineIoDispatcher`: inicialización del motor Rust, gestión de proyectos, importación/
// exportación SAF, edición de chart, mezclador de audio, bindings Sparrow, escenario y scripts.

package com.beatforge.studio.viewmodel

import android.app.Application
import android.net.Uri
import com.beatforge.studio.CAMERA_SELECTION_ID
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.ImportKind
import com.beatforge.studio.NativeEngineBridge
import com.beatforge.studio.SafFileBridge
import com.beatforge.studio.SparrowFrameInfo
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.StudioJsonParser
import com.beatforge.studio.StudioScreen
import com.beatforge.studio.StudioUiState
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

internal class StudioEditorDelegate(
    private val app: Application,
    private val scope: CoroutineScope,
    private val engineIoDispatcher: CoroutineDispatcher,
    private val uiState: MutableStateFlow<StudioUiState>,
    private val gameplayFrameState: GameplayFrameState,
    private val bitmapLoader: StudioBitmapLoader
) {
    fun initializeEngine() {
        scope.launch(engineIoDispatcher) {
            NativeEngineBridge.ensureLoaded()
            val cacheDir = app.cacheDir.absolutePath
            val dataDir = app.filesDir.absolutePath
            val rawSnapshot = NativeEngineBridge.nativeInit(cacheDir, dataDir)
            val snap = StudioJsonParser.parseProjectSnapshot(rawSnapshot)
            val projects = StudioJsonParser.parseProjectList(NativeEngineBridge.nativeListProjects())
            val arrows = bitmapLoader.loadArrowBitmaps()
            val receptors = bitmapLoader.loadReceptorBitmaps()
            val hudAssets = bitmapLoader.loadHudSvgAssets()
            val (normalIcons, losingIcons) = bitmapLoader.loadEntityIcons(snap.stageEntities)
            val entBmps = bitmapLoader.loadEditorEntityBitmaps(
                snap.stageEntities,
                uiState.value.selectedTier
            )
            uiState.update { state ->
                state.copy(
                    initialized = true,
                    snapshot = snap,
                    projects = projects,
                    selectedEntityId = snap.stageEntities.firstOrNull()?.id ?: "",
                    selectedScriptId = snap.scripts.firstOrNull()?.id ?: "",
                    arrowBitmaps = arrows,
                    receptorBitmaps = receptors,
                    healthBarBitmap = hudAssets.healthBar,
                    ratingBitmaps = hudAssets.ratings,
                    noteSplashBitmap = hudAssets.noteSplash,
                    defaultPlayerNormalIcon = hudAssets.playerNormal,
                    defaultPlayerLosingIcon = hudAssets.playerLosing,
                    defaultOpponentNormalIcon = hudAssets.opponentNormal,
                    defaultOpponentLosingIcon = hudAssets.opponentLosing,
                    entityNormalIcons = normalIcons,
                    entityLosingIcons = losingIcons,
                    entityBitmaps = entBmps
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

    fun importUri(kind: ImportKind, uri: Uri) {
        scope.launch(engineIoDispatcher) {
            setBusy(true)
            val localPath = SafFileBridge.copyUriToInternalCache(app, uri, kind.tag)
            if (localPath == null) {
                postBanner("No se pudo copiar el archivo seleccionado", isError = true)
                setBusy(false)
                return@launch
            }

            val rawJson = when (kind) {
                is ImportKind.Chart -> NativeEngineBridge.nativeImportChart(localPath)
                is ImportKind.AudioTrack -> NativeEngineBridge.nativeImportAudioTrack(kind.slot, localPath)
                is ImportKind.SpritePng -> NativeEngineBridge.nativeImportSpriteSheetPng(kind.entityId, localPath)
                is ImportKind.SparrowXml -> NativeEngineBridge.nativeImportSparrowXml(kind.entityId, localPath)
                is ImportKind.Script -> NativeEngineBridge.nativeImportScript(localPath)
                is ImportKind.ArrowSvg -> NativeEngineBridge.nativeImportArrowSvg(kind.direction, localPath)
                is ImportKind.CharacterIcon -> NativeEngineBridge.nativeImportCharacterIcon(kind.entityId, kind.isLosing, localPath)
                is ImportKind.HealthBarSvg -> NativeEngineBridge.nativeImportHealthBarSvg(localPath)
            }

            applyActionJson(rawJson)
            when (kind) {
                is ImportKind.ArrowSvg -> reloadArrowBitmaps()
                is ImportKind.HealthBarSvg -> reloadHealthBarBitmap()
                is ImportKind.CharacterIcon -> reloadCustomIcons(uiState.value.snapshot.stageEntities)
                is ImportKind.SpritePng, is ImportKind.SparrowXml -> {
                    bitmapLoader.evictRegionCache()
                    reloadEntityBitmaps(uiState.value.snapshot.stageEntities)
                }
                else -> Unit
            }
            refreshProjectsInternal()
            setBusy(false)
        }
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
            val raw = NativeEngineBridge.nativeCreateProject(name, author)
            applyProjectSwitchResult(raw, navigateToHub = true)
            setBusy(false)
        }
    }

    fun openProject(projectId: String) {
        scope.launch(engineIoDispatcher) {
            setBusy(true)
            if (uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            val raw = NativeEngineBridge.nativeOpenProject(projectId)
            applyProjectSwitchResult(raw, navigateToHub = true)
            setBusy(false)
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

    fun toggleChartNote(secIndex: Int, stepInSec: Int, col0To7: Int, sustainSteps: Int) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(
                NativeEngineBridge.nativeToggleChartNote(secIndex, stepInSec, col0To7, sustainSteps),
                silentOk = true
            )
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

    fun setSparrowBinding(entityId: String, animKind: String, prefix: String) {
        scope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeSetSparrowBinding(entityId, animKind, prefix))
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
        commitSave: Boolean
    ) {
        val clampedX = baseX.coerceIn(-1000f, 2600f)
        val clampedY = baseY.coerceIn(-1000f, 1800f)
        val clampedZoom = baseZoom.coerceIn(0.35f, 2.5f)
        val clampedBeats = followIntervalBeats.coerceIn(1, 32)
        val clampedSmooth = smoothSpeed.coerceIn(1f, 20f)
        val clampedPan = panStrength.coerceIn(0f, 1f)
        val normalizedMode = if (followMode == "sections") "sections" else "beats"

        uiState.update { state ->
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
                        panStrength = clampedPan
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
                commitSave
            )
            if (commitSave) {
                applyActionJson(raw, silentOk = true)
            }
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

    private fun applyProjectSwitchResult(rawJson: String, navigateToHub: Boolean) {
        val result = StudioJsonParser.parseActionResult(rawJson)
        val nextSnap = result.snapshot ?: uiState.value.snapshot
        bitmapLoader.evictRegionCache()
        val arrows = bitmapLoader.loadArrowBitmaps()
        val receptors = bitmapLoader.loadReceptorBitmaps()
        val hudAssets = bitmapLoader.loadHudSvgAssets()
        val (normalIcons, losingIcons) = bitmapLoader.loadEntityIcons(nextSnap.stageEntities)
        val entBmps = bitmapLoader.loadEditorEntityBitmaps(
            nextSnap.stageEntities,
            uiState.value.selectedTier
        )
        val projects = StudioJsonParser.parseProjectList(NativeEngineBridge.nativeListProjects())

        uiState.update { state ->
            state.copy(
                currentScreen = if (navigateToHub && result.ok) StudioScreen.Hub else state.currentScreen,
                snapshot = nextSnap,
                projects = projects,
                audioPreviewPlaying = false,
                selectedEntityId = nextSnap.stageEntities.firstOrNull()?.id ?: "",
                selectedScriptId = nextSnap.scripts.firstOrNull()?.id ?: "",
                scriptValidationMsg = null,
                arrowBitmaps = arrows,
                receptorBitmaps = receptors,
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
        uiState.update { it.copy(arrowBitmaps = arrows, receptorBitmaps = receptors) }
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
