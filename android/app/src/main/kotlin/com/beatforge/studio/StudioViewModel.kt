// Archivo: android/app/src/main/kotlin/com/beatforge/studio/StudioViewModel.kt
// Propósito: ViewModel principal de Jetpack Compose (`StudioViewModel`) que coordina
// el estado reactivo (`StateFlow`) entre las pantallas Kotlin y el motor Rust vía JNI.
// Todas las operaciones pesadas (copia SAF, decodificación de bitmaps, llamadas JNI de audio,
// compilación en caliente y validación de scripts) corren en `Dispatchers.IO` sin bloquear la UI.

package com.beatforge.studio

import android.app.Application
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.BitmapRegionDecoder
import android.graphics.Rect
import android.net.Uri
import android.os.Build
import android.util.LruCache
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.asCoroutineDispatcher
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.util.concurrent.Executors

data class LoadedEntityBitmap(
    val bitmap: ImageBitmap,
    val origWidth: Int,
    val origHeight: Int
)

data class StudioUiState(
    val initialized: Boolean = false,
    val currentScreen: StudioScreen = StudioScreen.Hub,
    val snapshot: ProjectSnapshot = ProjectSnapshot(),
    val projects: List<ProjectSummary> = emptyList(),
    val selectedEntityId: String = "",
    val selectedScriptId: String = "",
    val arrowSkinDialogOpen: Boolean = false,
    val characterIconsDialogOpen: Boolean = false,
    val audioPreviewPlaying: Boolean = false,
    val selectedTier: DeviceTier = DeviceTier.Balanced,
    val isBusy: Boolean = false,
    val statusBanner: String? = null,
    val statusIsError: Boolean = false,
    val scriptValidationMsg: String? = null,
    val scriptValidationOk: Boolean = true,
    val arrowBitmaps: Map<Int, ImageBitmap> = emptyMap(),
    val receptorBitmaps: Map<Int, ImageBitmap> = emptyMap(),
    val healthBarBitmap: ImageBitmap? = null,
    val ratingBitmaps: Map<Int, ImageBitmap> = emptyMap(),
    val noteSplashBitmap: ImageBitmap? = null,
    val defaultPlayerNormalIcon: ImageBitmap? = null,
    val defaultPlayerLosingIcon: ImageBitmap? = null,
    val defaultOpponentNormalIcon: ImageBitmap? = null,
    val defaultOpponentLosingIcon: ImageBitmap? = null,
    val entityNormalIcons: Map<String, ImageBitmap> = emptyMap(),
    val entityLosingIcons: Map<String, ImageBitmap> = emptyMap(),
    val entityBitmaps: Map<String, LoadedEntityBitmap> = emptyMap(),
    val compiledBitmaps: Map<String, LoadedEntityBitmap> = emptyMap(),
    val compiledBitmapsByIndex: List<LoadedEntityBitmap?> = emptyList()
)

class StudioViewModel(application: Application) : AndroidViewModel(application) {

    private val _uiState = MutableStateFlow(StudioUiState())
    val uiState: StateFlow<StudioUiState> = _uiState.asStateFlow()

    val gameplayFrameState = GameplayFrameState()

    private val engineExecutor = Executors.newSingleThreadExecutor { r ->
        Thread(r, "BeatForgeEngineIO").apply { isDaemon = true }
    }
    private val engineIoDispatcher = engineExecutor.asCoroutineDispatcher()

    // Caché LRU en KB para cuadros individuales decodificados 1:1 en SpriteLab (evita vista borrosa sin OOM en 32-bit)
    private val regionFrameCache = object : LruCache<String, ImageBitmap>(20 * 1024) {
        override fun sizeOf(key: String, value: ImageBitmap): Int {
            return ((value.width.toLong() * value.height.toLong() * 4L) / 1024L).toInt().coerceAtLeast(1)
        }
    }
    private val regionDecoderLock = Any()

    private var pendingImportKind: ImportKind? = null
    private var launchPickerCallback: ((Array<String>) -> Unit)? = null
    private var launchExportChartCallback: ((String) -> Unit)? = null

    init {
        _uiState.update { it.copy(selectedTier = detectOptimalDeviceTier()) }
        initializeEngine()
    }

    override fun onCleared() {
        try {
            NativeEngineBridge.nativeFlushSave()
        } catch (_: Throwable) {
        }
        engineExecutor.shutdown()
        super.onCleared()
    }

    /**
     * Detecta automáticamente la capacidad del teléfono (arquitectura 32/64 bits y RAM total)
     * para elegir el límite óptimo de texturas durante la compilación en caliente sin molestar al usuario.
     */
    private fun detectOptimalDeviceTier(): DeviceTier {
        return try {
            val app = getApplication<Application>()
            val am = app.getSystemService(android.content.Context.ACTIVITY_SERVICE) as? android.app.ActivityManager
            val memInfo = android.app.ActivityManager.MemoryInfo()
            am?.getMemoryInfo(memInfo)
            val totalGb = memInfo.totalMem / (1024.0 * 1024.0 * 1024.0)
            val is32BitOnly = android.os.Build.SUPPORTED_64_BIT_ABIS.isEmpty()

            when {
                is32BitOnly || memInfo.lowMemory || totalGb <= 3.5 -> DeviceTier.LowRam32Bit
                totalGb >= 7.5 -> DeviceTier.HighQuality
                else -> DeviceTier.Balanced
            }
        } catch (_: Throwable) {
            DeviceTier.Balanced
        }
    }

    fun registerPickerLauncher(launcher: (Array<String>) -> Unit) {
        launchPickerCallback = launcher
    }

    fun registerExportChartLauncher(launcher: (String) -> Unit) {
        launchExportChartCallback = launcher
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

    fun exportChartToUri(uri: Uri) {
        viewModelScope.launch(engineIoDispatcher) {
            setBusy(true)
            try {
                val jsonText = NativeEngineBridge.nativeExportChartJson()
                val os = getApplication<Application>().contentResolver.openOutputStream(uri)
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

    fun onUriPicked(uri: Uri) {
        val kind = pendingImportKind ?: return
        pendingImportKind = null
        viewModelScope.launch(engineIoDispatcher) {
            setBusy(true)
            val localPath = SafFileBridge.copyUriToInternalCache(
                getApplication(),
                uri,
                kind.tag
            )
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
                is ImportKind.CharacterIcon -> reloadCustomIcons(_uiState.value.snapshot.stageEntities)
                is ImportKind.SpritePng, is ImportKind.SparrowXml -> {
                    synchronized(regionDecoderLock) { regionFrameCache.evictAll() }
                    reloadEntityBitmaps(_uiState.value.snapshot.stageEntities)
                }
                else -> Unit
            }
            refreshProjectsInternal()
            setBusy(false)
        }
    }

    private fun initializeEngine() {
        viewModelScope.launch(engineIoDispatcher) {
            NativeEngineBridge.ensureLoaded()
            val app = getApplication<Application>()
            val cacheDir = app.cacheDir.absolutePath
            val dataDir = app.filesDir.absolutePath
            val rawSnapshot = NativeEngineBridge.nativeInit(cacheDir, dataDir)
            val snap = StudioJsonParser.parseProjectSnapshot(rawSnapshot)
            val projects = StudioJsonParser.parseProjectList(NativeEngineBridge.nativeListProjects())
            val arrows = loadArrowBitmapsInternal()
            val receptors = loadReceptorBitmapsInternal()
            val hudAssets = loadHudSvgAssetsInternal()
            val (normalIcons, losingIcons) = loadEntityIconsInternal(snap.stageEntities)
            val entBmps = loadEntityBitmapsInternal(snap.stageEntities, _uiState.value.selectedTier)
            _uiState.update { state ->
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

    fun flushSave() {
        viewModelScope.launch(engineIoDispatcher) {
            NativeEngineBridge.nativeFlushSave()
            refreshProjectsInternal()
        }
    }

    fun refreshProjects() {
        viewModelScope.launch(engineIoDispatcher) {
            NativeEngineBridge.nativeFlushSave()
            refreshProjectsInternal()
        }
    }

    fun createProject(name: String, author: String) {
        viewModelScope.launch(engineIoDispatcher) {
            setBusy(true)
            if (_uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            val raw = NativeEngineBridge.nativeCreateProject(name, author)
            applyProjectSwitchResult(raw, navigateToHub = true)
            setBusy(false)
        }
    }

    fun openProject(projectId: String) {
        viewModelScope.launch(engineIoDispatcher) {
            setBusy(true)
            if (_uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            val raw = NativeEngineBridge.nativeOpenProject(projectId)
            applyProjectSwitchResult(raw, navigateToHub = true)
            setBusy(false)
        }
    }

    fun duplicateProject(projectId: String) {
        viewModelScope.launch(engineIoDispatcher) {
            setBusy(true)
            if (_uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            val raw = NativeEngineBridge.nativeDuplicateProject(projectId)
            applyProjectSwitchResult(raw, navigateToHub = false)
            setBusy(false)
        }
    }

    fun deleteProject(projectId: String) {
        viewModelScope.launch(engineIoDispatcher) {
            setBusy(true)
            val raw = NativeEngineBridge.nativeDeleteProject(projectId)
            applyProjectSwitchResult(raw, navigateToHub = false)
            setBusy(false)
        }
    }

    private fun applyProjectSwitchResult(rawJson: String, navigateToHub: Boolean) {
        val result = StudioJsonParser.parseActionResult(rawJson)
        val nextSnap = result.snapshot ?: _uiState.value.snapshot
        synchronized(regionDecoderLock) { regionFrameCache.evictAll() }
        val arrows = loadArrowBitmapsInternal()
        val receptors = loadReceptorBitmapsInternal()
        val hudAssets = loadHudSvgAssetsInternal()
        val (normalIcons, losingIcons) = loadEntityIconsInternal(nextSnap.stageEntities)
        val entBmps = loadEntityBitmapsInternal(nextSnap.stageEntities, _uiState.value.selectedTier)
        val projects = StudioJsonParser.parseProjectList(NativeEngineBridge.nativeListProjects())

        _uiState.update { state ->
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

    fun navigateTo(screen: StudioScreen) {
        if (_uiState.value.currentScreen == StudioScreen.Gameplay && screen != StudioScreen.Gameplay) {
            stopGameplayAndNavigate(screen)
            return
        }
        viewModelScope.launch(engineIoDispatcher) {
            NativeEngineBridge.nativeFlushSave()
            if (screen == StudioScreen.Projects) {
                refreshProjectsInternal()
            }
        }
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

    fun setCharacterIconsDialogOpen(open: Boolean) {
        _uiState.update { it.copy(characterIconsDialogOpen = open) }
    }

    fun clearCharacterIcons(entityId: String) {
        viewModelScope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeClearCharacterIcons(entityId))
            reloadCustomIcons(_uiState.value.snapshot.stageEntities)
        }
    }

    fun restoreDefaultHealthBarSvg() {
        viewModelScope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeRestoreDefaultHealthBarSvg())
            reloadHealthBarBitmap()
        }
    }

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
    ) {
        viewModelScope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeUpdateProjectMeta(
                name,
                author,
                downscroll,
                controlMode,
                audioOffsetMs,
                scrollSpeed
            )
            applyActionJson(raw, silentOk = true)
            if (_uiState.value.currentScreen == StudioScreen.Gameplay) {
                val gpRaw = NativeEngineBridge.nativeStepGameplay(0f, 0, 0)
                val gpStep = StudioJsonParser.parseGameplayStep(gpRaw)
                withContext(Dispatchers.Main) {
                    gameplayFrameState.seedFromJsonStep(gpStep)
                }
            }
        }
    }

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

    fun restoreDemoChart() {
        viewModelScope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeRestoreDemoChart())
        }
    }

    fun setChartBpm(bpm: Double) {
        viewModelScope.launch(engineIoDispatcher) {
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
        viewModelScope.launch(engineIoDispatcher) {
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
        viewModelScope.launch(engineIoDispatcher) {
            applyActionJson(
                NativeEngineBridge.nativeSetSectionBpm(secIndex, bpm, changeBpm),
                silentOk = true
            )
        }
    }

    fun addChartEvent(timeMs: Double, name: String, value1: String, value2: String) {
        viewModelScope.launch(engineIoDispatcher) {
            applyActionJson(
                NativeEngineBridge.nativeAddChartEvent(timeMs, name, value1, value2)
            )
        }
    }

    fun removeChartEvent(index: Int) {
        viewModelScope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeRemoveChartEvent(index), silentOk = true)
        }
    }

    fun addChartSection(mustHit: Boolean) {
        viewModelScope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeAddChartSection(mustHit))
        }
    }

    fun toggleSectionMustHit(secIndex: Int) {
        viewModelScope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeToggleSectionMustHit(secIndex), silentOk = true)
        }
    }

    fun toggleChartNote(secIndex: Int, stepInSec: Int, col0To7: Int, sustainSteps: Int) {
        viewModelScope.launch(engineIoDispatcher) {
            applyActionJson(
                NativeEngineBridge.nativeToggleChartNote(secIndex, stepInSec, col0To7, sustainSteps),
                silentOk = true
            )
        }
    }

    fun removeAudioTrack(slot: Int) {
        viewModelScope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeRemoveAudioTrack(slot))
        }
    }

    fun updateAudioTrackConfig(slot: Int, gain: Float, muted: Boolean, duckOnMiss: Boolean) {
        viewModelScope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeUpdateAudioTrackConfig(slot, gain, muted, duckOnMiss)
            applyActionJson(raw, silentOk = true)
        }
    }

    fun toggleAudioPreview(startMs: Double = 0.0) {
        viewModelScope.launch(engineIoDispatcher) {
            val nextPlay = !_uiState.value.audioPreviewPlaying
            val playing = NativeEngineBridge.nativeToggleAudioPreview(nextPlay, startMs)
            _uiState.update { it.copy(audioPreviewPlaying = playing) }
        }
    }

    fun setSparrowBinding(entityId: String, animKind: String, prefix: String) {
        viewModelScope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeSetSparrowBinding(entityId, animKind, prefix))
        }
    }

    suspend fun querySparrowFrame(entityId: String, animKind: String, elapsedSec: Float): SparrowFrameInfo {
        return withContext(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeGetSparrowFrameInfo(entityId, animKind, elapsedSec)
            StudioJsonParser.parseSparrowFrameInfo(raw)
        }
    }

    /**
     * Decodifica únicamente la región exacta 1:1 del frame activo en el sprite sheet usando
     * `BitmapRegionDecoder` (LRU caché en `Dispatchers.IO`), eliminando la vista borrosa en SpriteLab
     * incluso con hojas de 4096px en teléfonos de 32 bits.
     */
    suspend fun decodeSparrowRegionBitmap(imagePath: String?, frame: SparrowFrameInfo): ImageBitmap? {
        if (imagePath.isNullOrBlank() || !frame.found || frame.width <= 0 || frame.height <= 0) {
            return null
        }
        val cacheKey = "$imagePath|${frame.x},${frame.y},${frame.width},${frame.height}"
        regionFrameCache.get(cacheKey)?.let { return it }

        return withContext(Dispatchers.IO) {
            regionFrameCache.get(cacheKey)?.let { return@withContext it }
            val file = File(imagePath)
            if (!file.exists()) return@withContext null

            val maxFrameSide = if (_uiState.value.selectedTier == DeviceTier.LowRam32Bit) 900 else 1280
            var sampleSize = 1
            val longest = maxOf(frame.width, frame.height)
            while (longest / sampleSize > maxFrameSide) {
                sampleSize *= 2
            }

            val rect = Rect(
                frame.x.coerceAtLeast(0),
                frame.y.coerceAtLeast(0),
                (frame.x + frame.width).coerceAtLeast(frame.x + 1),
                (frame.y + frame.height).coerceAtLeast(frame.y + 1)
            )
            val opts = BitmapFactory.Options().apply {
                inSampleSize = sampleSize
                inPreferredConfig = Bitmap.Config.ARGB_8888
                inScaled = false
            }

            val decoded: Bitmap? = synchronized(regionDecoderLock) {
                try {
                    val decoder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                        BitmapRegionDecoder.newInstance(imagePath)
                    } else {
                        @Suppress("DEPRECATION")
                        BitmapRegionDecoder.newInstance(imagePath, false)
                    }
                    try {
                        decoder?.decodeRegion(rect, opts)
                    } finally {
                        decoder?.recycle()
                    }
                } catch (_: Throwable) {
                    null
                }
            }

            val img = decoded?.asImageBitmap()
            if (img != null) {
                regionFrameCache.put(cacheKey, img)
            }
            img
        }
    }

    fun addStageEntity(name: String, role: String) {
        viewModelScope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeAddStageEntity(name, role)
            applyActionJson(raw)
            val newest = _uiState.value.snapshot.stageEntities.lastOrNull()?.id
            if (!newest.isNullOrBlank()) {
                _uiState.update { it.copy(selectedEntityId = newest) }
            }
        }
    }

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
    ) {
        // Actualización optimista síncrona: evita que sliders o arrastres en 60Hz reboten con snapshots viejos
        val clampedScale = scale.coerceIn(0.05f, 6.0f)
        val clampedOpacity = opacity.coerceIn(0f, 1f)
        _uiState.update { state ->
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

        viewModelScope.launch(engineIoDispatcher) {
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
        viewModelScope.launch(engineIoDispatcher) {
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
        commitSave: Boolean = true
    ) {
        val clampedX = baseX.coerceIn(-1000f, 2600f)
        val clampedY = baseY.coerceIn(-1000f, 1800f)
        val clampedZoom = baseZoom.coerceIn(0.35f, 2.5f)
        val clampedBeats = followIntervalBeats.coerceIn(1, 32)
        val clampedSmooth = smoothSpeed.coerceIn(1f, 20f)
        val clampedPan = panStrength.coerceIn(0f, 1f)
        val normalizedMode = if (followMode == "sections") "sections" else "beats"

        _uiState.update { state ->
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

        viewModelScope.launch(engineIoDispatcher) {
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

    fun openCameraScriptInStudio() {
        val snap = _uiState.value.snapshot
        val camScriptId = snap.scripts.firstOrNull {
            it.id == "script_camera_system" || it.name == "camera_system.lua"
        }?.id ?: snap.scripts.firstOrNull()?.id ?: ""
        viewModelScope.launch(engineIoDispatcher) {
            NativeEngineBridge.nativeFlushSave()
        }
        _uiState.update {
            it.copy(
                selectedScriptId = camScriptId,
                currentScreen = StudioScreen.ScriptStudio,
                scriptValidationMsg = null,
                statusBanner = null
            )
        }
    }

    fun addScript(language: String) {
        viewModelScope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeAddScript(language)
            applyActionJson(raw)
            val newest = _uiState.value.snapshot.scripts.lastOrNull()?.id
            if (!newest.isNullOrBlank()) {
                _uiState.update { it.copy(selectedScriptId = newest, scriptValidationMsg = null) }
            }
        }
    }

    fun updateScript(scriptId: String, enabled: Boolean, sourceCode: String, silent: Boolean = true) {
        viewModelScope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeUpdateScript(scriptId, enabled, sourceCode)
            applyActionJson(raw, silentOk = silent)
        }
    }

    fun validateScript(scriptId: String, currentEnabled: Boolean, currentSource: String) {
        viewModelScope.launch(engineIoDispatcher) {
            NativeEngineBridge.nativeUpdateScript(scriptId, currentEnabled, currentSource)
            val res = StudioJsonParser.parseValidationResult(
                NativeEngineBridge.nativeValidateScript(scriptId)
            )
            val snap = StudioJsonParser.parseProjectSnapshot(NativeEngineBridge.nativeGetSnapshot())
            _uiState.update {
                it.copy(
                    snapshot = snap,
                    scriptValidationOk = res.ok,
                    scriptValidationMsg = res.message
                )
            }
        }
    }

    fun restoreDefaultArrowSvg() {
        viewModelScope.launch(engineIoDispatcher) {
            applyActionJson(NativeEngineBridge.nativeRestoreDefaultArrowSvg())
            reloadArrowBitmaps()
        }
    }

    fun startCompileAndPlay() {
        viewModelScope.launch(engineIoDispatcher) {
            NativeEngineBridge.nativeFlushSave()
            if (_uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            _uiState.update {
                it.copy(
                    currentScreen = StudioScreen.HotCompile,
                    audioPreviewPlaying = false,
                    isBusy = true,
                    statusBanner = null,
                    // Liberar bitmaps del editor antes de cargar los bitmaps compilados de partida
                    entityBitmaps = emptyMap(),
                    compiledBitmaps = emptyMap(),
                    compiledBitmapsByIndex = emptyList()
                )
            }

            synchronized(regionDecoderLock) { regionFrameCache.evictAll() }
            val tier = _uiState.value.selectedTier
            val compileRaw = NativeEngineBridge.nativeHotCompile(tier.code)
            val compileRes = StudioJsonParser.parseActionResult(compileRaw)
            val snap = compileRes.snapshot ?: _uiState.value.snapshot

            val compiledMap = loadCompiledBitmapsInternal(
                snap.lastCompileReport?.compiledTexturePaths ?: emptyMap(),
                snap.stageEntities,
                tier
            )
            val arrows = loadArrowBitmapsInternal()
            val receptors = loadReceptorBitmapsInternal()
            val hudAssets = loadHudSvgAssetsInternal()
            val (normalIcons, losingIcons) = loadEntityIconsInternal(snap.stageEntities)

            val gpRaw = NativeEngineBridge.nativeStartGameplay()
            val gpStep = StudioJsonParser.parseGameplayStep(gpRaw)
            val compiledByIndex = gpStep.entities.map { compiledMap[it.id] }

            withContext(Dispatchers.Main) {
                gameplayFrameState.seedFromJsonStep(gpStep)
            }

            _uiState.update {
                it.copy(
                    isBusy = false,
                    snapshot = snap,
                    compiledBitmaps = compiledMap,
                    compiledBitmapsByIndex = compiledByIndex,
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
                    currentScreen = StudioScreen.Gameplay
                )
            }
        }
    }

    suspend fun stepGameplayFrame(dtSec: Float, pressedMask: Int, justPressedMask: Int) {
        val written = withContext(engineIoDispatcher) {
            var res = NativeEngineBridge.nativeStepGameplayBinary(
                dtSec,
                pressedMask,
                justPressedMask,
                gameplayFrameState.directBuffer
            )
            if (res < 0) {
                val needed = -res
                if (needed in 1..(16 * 1024 * 1024)) {
                    gameplayFrameState.ensureDirectCapacity(needed)
                    res = NativeEngineBridge.nativeStepGameplayBinary(
                        dtSec,
                        pressedMask,
                        justPressedMask,
                        gameplayFrameState.directBuffer
                    )
                }
            }
            res
        }
        if (written > 0) {
            gameplayFrameState.decodeFromDirectBuffer(written)
        }
    }

    fun setGameplayPaused(paused: Boolean) {
        viewModelScope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeSetGameplayPaused(paused)
            val step = StudioJsonParser.parseGameplayStep(raw)
            withContext(Dispatchers.Main) {
                gameplayFrameState.seedFromJsonStep(step)
            }
        }
    }

    fun restartGameplay() {
        viewModelScope.launch(engineIoDispatcher) {
            val gpRaw = NativeEngineBridge.nativeStartGameplay()
            val gpStep = StudioJsonParser.parseGameplayStep(gpRaw)
            val compiledMap = _uiState.value.compiledBitmaps
            val compiledByIndex = gpStep.entities.map { compiledMap[it.id] }
            withContext(Dispatchers.Main) {
                gameplayFrameState.seedFromJsonStep(gpStep)
            }
            _uiState.update { it.copy(compiledBitmapsByIndex = compiledByIndex) }
        }
    }

    fun stopGameplayAndNavigate(target: StudioScreen = StudioScreen.Hub) {
        viewModelScope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeStopGameplay()
            val res = StudioJsonParser.parseActionResult(raw)
            val nextSnap = res.snapshot ?: _uiState.value.snapshot
            withContext(Dispatchers.Main) {
                gameplayFrameState.reset()
            }
            // Liberar primero los bitmaps compilados de gameplay antes de recargar previews del editor
            _uiState.update {
                it.copy(
                    compiledBitmaps = emptyMap(),
                    compiledBitmapsByIndex = emptyList()
                )
            }
            val restoredEditorBitmaps = loadEntityBitmapsInternal(
                nextSnap.stageEntities,
                _uiState.value.selectedTier
            )
            _uiState.update {
                it.copy(
                    currentScreen = target,
                    snapshot = nextSnap,
                    entityBitmaps = restoredEditorBitmaps
                )
            }
        }
    }

    private fun refreshProjectsInternal() {
        val list = StudioJsonParser.parseProjectList(NativeEngineBridge.nativeListProjects())
        _uiState.update { it.copy(projects = list) }
    }

    private fun applyActionJson(rawJson: String, silentOk: Boolean = false) {
        val result = StudioJsonParser.parseActionResult(rawJson)
        _uiState.update { state ->
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
        _uiState.update { it.copy(isBusy = busy) }
    }

    private fun postBanner(msg: String, isError: Boolean) {
        _uiState.update { it.copy(statusBanner = msg, statusIsError = isError) }
    }

    private fun reloadArrowBitmaps() {
        val arrows = loadArrowBitmapsInternal()
        val receptors = loadReceptorBitmapsInternal()
        _uiState.update { it.copy(arrowBitmaps = arrows, receptorBitmaps = receptors) }
    }

    private fun reloadHealthBarBitmap() {
        val bytes = NativeEngineBridge.nativeGetHealthBarPngBytes()
        val bmp = if (bytes.isNotEmpty()) {
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
        } else {
            null
        }
        _uiState.update { it.copy(healthBarBitmap = bmp) }
    }

    private fun reloadCustomIcons(entities: List<StageEntitySnapshot>) {
        val (normalIcons, losingIcons) = loadEntityIconsInternal(entities)
        _uiState.update { it.copy(entityNormalIcons = normalIcons, entityLosingIcons = losingIcons) }
    }

    private fun reloadEntityBitmaps(entities: List<StageEntitySnapshot>) {
        val map = loadEntityBitmapsInternal(entities, _uiState.value.selectedTier)
        _uiState.update { it.copy(entityBitmaps = map) }
    }

    private fun editorPreviewMaxSide(tier: DeviceTier, hasAtlas: Boolean): Int {
        return when (tier) {
            DeviceTier.LowRam32Bit -> if (hasAtlas) 1792 else 1024
            DeviceTier.Balanced -> if (hasAtlas) 2560 else 1536
            DeviceTier.HighQuality -> if (hasAtlas) 3072 else 2048
        }
    }

    private fun compiledGameplayMaxSide(tier: DeviceTier, ent: StageEntitySnapshot): Int {
        val hasAtlas = ent.xmlPath != null ||
            ent.animateJsonPath != null ||
            ent.animationPrefixes.isNotEmpty() ||
            (ent.previewFrame?.found == true)
        return when (tier) {
            DeviceTier.LowRam32Bit -> if (hasAtlas) 2560 else 1536
            DeviceTier.Balanced -> if (hasAtlas) 4096 else 2560
            DeviceTier.HighQuality -> 4096
        }
    }

    private fun loadArrowBitmapsInternal(): Map<Int, ImageBitmap> {
        val out = linkedMapOf<Int, ImageBitmap>()
        for (dir in 0..3) {
            val bytes = NativeEngineBridge.nativeGetArrowPngBytes(dir)
            if (bytes.isNotEmpty()) {
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.let { bmp ->
                    out[dir] = bmp.asImageBitmap()
                }
            }
        }
        return out
    }

    private fun loadReceptorBitmapsInternal(): Map<Int, ImageBitmap> {
        val out = linkedMapOf<Int, ImageBitmap>()
        for (dir in 0..3) {
            val bytes = NativeEngineBridge.nativeGetReceptorPngBytes(dir)
            if (bytes.isNotEmpty()) {
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.let { bmp ->
                    out[dir] = bmp.asImageBitmap()
                }
            }
        }
        return out
    }

    private data class HudSvgAssets(
        val healthBar: ImageBitmap?,
        val ratings: Map<Int, ImageBitmap>,
        val noteSplash: ImageBitmap?,
        val playerNormal: ImageBitmap?,
        val playerLosing: ImageBitmap?,
        val opponentNormal: ImageBitmap?,
        val opponentLosing: ImageBitmap?
    )

    private fun decodePngBytes(bytes: ByteArray): ImageBitmap? {
        if (bytes.isEmpty()) return null
        return BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
    }

    private fun loadHudSvgAssetsInternal(): HudSvgAssets {
        val healthBar = decodePngBytes(NativeEngineBridge.nativeGetHealthBarPngBytes())
        val ratings = linkedMapOf<Int, ImageBitmap>()
        for (code in 1..4) {
            decodePngBytes(NativeEngineBridge.nativeGetRatingPngBytes(code))?.let {
                ratings[code] = it
            }
        }
        val noteSplash = decodePngBytes(NativeEngineBridge.nativeGetNoteSplashPngBytes())
        val playerNormal = decodePngBytes(NativeEngineBridge.nativeGetDefaultIconPngBytes(isPlayer = true, isLosing = false))
        val playerLosing = decodePngBytes(NativeEngineBridge.nativeGetDefaultIconPngBytes(isPlayer = true, isLosing = true))
        val opponentNormal = decodePngBytes(NativeEngineBridge.nativeGetDefaultIconPngBytes(isPlayer = false, isLosing = false))
        val opponentLosing = decodePngBytes(NativeEngineBridge.nativeGetDefaultIconPngBytes(isPlayer = false, isLosing = true))
        return HudSvgAssets(
            healthBar = healthBar,
            ratings = ratings,
            noteSplash = noteSplash,
            playerNormal = playerNormal,
            playerLosing = playerLosing,
            opponentNormal = opponentNormal,
            opponentLosing = opponentLosing
        )
    }

    private fun decodeIconFileCapped(path: String): Bitmap? {
        val file = File(path)
        if (!file.exists()) return null
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(path, bounds)
        var sample = 1
        val maxSide = maxOf(bounds.outWidth, bounds.outHeight)
        while (maxSide / sample > 512) {
            sample *= 2
        }
        val opts = BitmapFactory.Options().apply {
            inSampleSize = sample
            inPreferredConfig = Bitmap.Config.ARGB_8888
            inScaled = false
        }
        return BitmapFactory.decodeFile(path, opts)
    }

    /**
     * Carga íconos personalizados de personaje. Si el usuario sube una tira horizontal clásica
     * de FNF (`width >= height * 1.85`), extrae automáticamente la mitad izquierda como Normal
     * y la mitad derecha como Perdiendo (<20% de vida) si no subió un archivo separado.
     */
    private fun loadEntityIconsInternal(
        entities: List<StageEntitySnapshot>
    ): Pair<Map<String, ImageBitmap>, Map<String, ImageBitmap>> {
        val normalMap = linkedMapOf<String, ImageBitmap>()
        val losingMap = linkedMapOf<String, ImageBitmap>()

        for (ent in entities) {
            val normalPath = ent.iconNormalPath
            val losingPath = ent.iconLosingPath

            if (!normalPath.isNullOrBlank()) {
                decodeIconFileCapped(normalPath)?.let { bmp ->
                    if (bmp.width >= (bmp.height * 1.85f).toInt() && bmp.width >= 4) {
                        val halfW = bmp.width / 2
                        val leftBmp = Bitmap.createBitmap(bmp, 0, 0, halfW, bmp.height)
                        normalMap[ent.id] = leftBmp.asImageBitmap()
                        if (losingPath.isNullOrBlank()) {
                            val rightBmp = Bitmap.createBitmap(bmp, halfW, 0, bmp.width - halfW, bmp.height)
                            losingMap[ent.id] = rightBmp.asImageBitmap()
                        }
                    } else {
                        normalMap[ent.id] = bmp.asImageBitmap()
                    }
                }
            }

            if (!losingPath.isNullOrBlank()) {
                decodeIconFileCapped(losingPath)?.let { bmp ->
                    if (bmp.width >= (bmp.height * 1.85f).toInt() && bmp.width >= 4) {
                        val halfW = bmp.width / 2
                        val rightBmp = Bitmap.createBitmap(bmp, halfW, 0, bmp.width - halfW, bmp.height)
                        losingMap[ent.id] = rightBmp.asImageBitmap()
                    } else {
                        losingMap[ent.id] = bmp.asImageBitmap()
                    }
                }
            }
        }
        return normalMap to losingMap
    }

    private fun loadEntityBitmapsInternal(
        entities: List<StageEntitySnapshot>,
        tier: DeviceTier
    ): Map<String, LoadedEntityBitmap> {
        val out = linkedMapOf<String, LoadedEntityBitmap>()
        for (ent in entities) {
            val path = ent.imagePath ?: continue
            val hasAtlas = ent.xmlPath != null ||
                ent.animateJsonPath != null ||
                ent.animationPrefixes.isNotEmpty() ||
                (ent.previewFrame?.found == true)
            val cap = editorPreviewMaxSide(tier, hasAtlas)
            decodeEntityBitmapWithOrigDims(path, path, maxDecodeSide = cap)?.let { loaded ->
                out[ent.id] = loaded
            }
        }
        return out
    }

    private fun loadCompiledBitmapsInternal(
        compiledPaths: Map<String, String>,
        entities: List<StageEntitySnapshot>,
        tier: DeviceTier
    ): Map<String, LoadedEntityBitmap> {
        val out = linkedMapOf<String, LoadedEntityBitmap>()
        val origPaths = entities.associate { it.id to it.imagePath }
        for (ent in entities) {
            val cPath = compiledPaths[ent.id] ?: origPaths[ent.id] ?: continue
            val oPath = origPaths[ent.id] ?: cPath
            val cap = compiledGameplayMaxSide(tier, ent)
            decodeEntityBitmapWithOrigDims(cPath, oPath, maxDecodeSide = cap)?.let { loaded ->
                out[ent.id] = loaded
            }
        }
        return out
    }

    private fun decodeEntityBitmapWithOrigDims(
        loadPath: String,
        origPath: String,
        maxDecodeSide: Int
    ): LoadedEntityBitmap? {
        val file = File(loadPath)
        if (!file.exists()) return null

        val origBounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(origPath, origBounds)
        val origW = origBounds.outWidth.coerceAtLeast(1)
        val origH = origBounds.outHeight.coerceAtLeast(1)

        val loadBounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(loadPath, loadBounds)
        var sampleSize = 1
        val maxSide = maxOf(loadBounds.outWidth, loadBounds.outHeight)
        while (maxSide / sampleSize > maxDecodeSide) {
            sampleSize *= 2
        }

        val decodeOpts = BitmapFactory.Options().apply {
            inSampleSize = sampleSize
            inPreferredConfig = Bitmap.Config.ARGB_8888
            inScaled = false
        }
        val bmp = BitmapFactory.decodeFile(loadPath, decodeOpts) ?: return null
        return LoadedEntityBitmap(
            bitmap = bmp.asImageBitmap(),
            origWidth = origW,
            origHeight = origH
        )
    }
}
