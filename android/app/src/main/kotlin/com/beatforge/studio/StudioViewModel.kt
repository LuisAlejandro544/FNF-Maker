// Archivo: android/app/src/main/kotlin/com/beatforge/studio/StudioViewModel.kt
// Propósito: ViewModel principal de Jetpack Compose (`StudioViewModel`) que coordina
// el estado reactivo (`StateFlow`) entre las pantallas Kotlin y el motor Rust vía JNI.
// Todas las operaciones pesadas (copia SAF, decodificación de bitmaps, llamadas JNI de audio,
// compilación en caliente y validación de scripts) corren en `Dispatchers.IO` sin bloquear la UI.

package com.beatforge.studio

import android.app.Application
import android.graphics.BitmapFactory
import android.net.Uri
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

data class LoadedEntityBitmap(
    val bitmap: ImageBitmap,
    val origWidth: Int,
    val origHeight: Int
)

data class StudioUiState(
    val initialized: Boolean = false,
    val currentScreen: StudioScreen = StudioScreen.Hub,
    val snapshot: ProjectSnapshot = ProjectSnapshot(),
    val selectedEntityId: String = "",
    val selectedScriptId: String = "",
    val arrowSkinDialogOpen: Boolean = false,
    val audioPreviewPlaying: Boolean = false,
    val selectedTier: DeviceTier = DeviceTier.Balanced,
    val isBusy: Boolean = false,
    val statusBanner: String? = null,
    val statusIsError: Boolean = false,
    val scriptValidationMsg: String? = null,
    val scriptValidationOk: Boolean = true,
    val arrowBitmaps: Map<Int, ImageBitmap> = emptyMap(),
    val receptorBitmaps: Map<Int, ImageBitmap> = emptyMap(),
    val entityBitmaps: Map<String, LoadedEntityBitmap> = emptyMap(),
    val compiledBitmaps: Map<String, LoadedEntityBitmap> = emptyMap(),
    val gameplayStep: GameplayStep = GameplayStep()
)

class StudioViewModel(application: Application) : AndroidViewModel(application) {

    private val _uiState = MutableStateFlow(StudioUiState())
    val uiState: StateFlow<StudioUiState> = _uiState.asStateFlow()

    private var pendingImportKind: ImportKind? = null
    private var launchPickerCallback: ((Array<String>) -> Unit)? = null
    private var launchExportChartCallback: ((String) -> Unit)? = null

    init {
        _uiState.update { it.copy(selectedTier = detectOptimalDeviceTier()) }
        initializeEngine()
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
                is32BitOnly || totalGb <= 3.5 -> DeviceTier.LowRam32Bit
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
        viewModelScope.launch(Dispatchers.IO) {
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
        viewModelScope.launch(Dispatchers.IO) {
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
            }

            applyActionJson(rawJson)
            if (kind is ImportKind.ArrowSvg) {
                reloadArrowBitmaps()
            } else if (kind is ImportKind.SpritePng || kind is ImportKind.SparrowXml) {
                reloadEntityBitmaps(_uiState.value.snapshot.stageEntities)
            }
            setBusy(false)
        }
    }

    private fun initializeEngine() {
        viewModelScope.launch(Dispatchers.IO) {
            NativeEngineBridge.ensureLoaded()
            val cacheDir = getApplication<Application>().cacheDir.absolutePath
            val rawSnapshot = NativeEngineBridge.nativeInit(cacheDir)
            val snap = StudioJsonParser.parseProjectSnapshot(rawSnapshot)
            val arrows = loadArrowBitmapsInternal()
            val receptors = loadReceptorBitmapsInternal()
            val entBmps = loadEntityBitmapsInternal(snap.stageEntities)
            _uiState.update { state ->
                state.copy(
                    initialized = true,
                    snapshot = snap,
                    selectedEntityId = snap.stageEntities.firstOrNull()?.id ?: "",
                    selectedScriptId = snap.scripts.firstOrNull()?.id ?: "",
                    arrowBitmaps = arrows,
                    receptorBitmaps = receptors,
                    entityBitmaps = entBmps
                )
            }
        }
    }

    fun navigateTo(screen: StudioScreen) {
        if (_uiState.value.currentScreen == StudioScreen.Gameplay && screen != StudioScreen.Gameplay) {
            stopGameplayAndNavigate(screen)
            return
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
        viewModelScope.launch(Dispatchers.IO) {
            val raw = NativeEngineBridge.nativeUpdateProjectMeta(
                name,
                author,
                downscroll,
                controlMode,
                audioOffsetMs,
                scrollSpeed
            )
            applyActionJson(raw, silentOk = true)
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
        viewModelScope.launch(Dispatchers.IO) {
            applyActionJson(NativeEngineBridge.nativeRestoreDemoChart())
        }
    }

    fun setChartBpm(bpm: Double) {
        viewModelScope.launch(Dispatchers.IO) {
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
        viewModelScope.launch(Dispatchers.IO) {
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
        viewModelScope.launch(Dispatchers.IO) {
            applyActionJson(
                NativeEngineBridge.nativeSetSectionBpm(secIndex, bpm, changeBpm),
                silentOk = true
            )
        }
    }

    fun addChartEvent(timeMs: Double, name: String, value1: String, value2: String) {
        viewModelScope.launch(Dispatchers.IO) {
            applyActionJson(
                NativeEngineBridge.nativeAddChartEvent(timeMs, name, value1, value2)
            )
        }
    }

    fun removeChartEvent(index: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            applyActionJson(NativeEngineBridge.nativeRemoveChartEvent(index), silentOk = true)
        }
    }

    fun addChartSection(mustHit: Boolean) {
        viewModelScope.launch(Dispatchers.IO) {
            applyActionJson(NativeEngineBridge.nativeAddChartSection(mustHit))
        }
    }

    fun toggleSectionMustHit(secIndex: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            applyActionJson(NativeEngineBridge.nativeToggleSectionMustHit(secIndex), silentOk = true)
        }
    }

    fun toggleChartNote(secIndex: Int, stepInSec: Int, col0To7: Int, sustainSteps: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            applyActionJson(
                NativeEngineBridge.nativeToggleChartNote(secIndex, stepInSec, col0To7, sustainSteps),
                silentOk = true
            )
        }
    }

    fun removeAudioTrack(slot: Int) {
        viewModelScope.launch(Dispatchers.IO) {
            applyActionJson(NativeEngineBridge.nativeRemoveAudioTrack(slot))
        }
    }

    fun updateAudioTrackConfig(slot: Int, gain: Float, muted: Boolean, duckOnMiss: Boolean) {
        viewModelScope.launch(Dispatchers.IO) {
            val raw = NativeEngineBridge.nativeUpdateAudioTrackConfig(slot, gain, muted, duckOnMiss)
            applyActionJson(raw, silentOk = true)
        }
    }

    fun toggleAudioPreview(startMs: Double = 0.0) {
        viewModelScope.launch(Dispatchers.IO) {
            val nextPlay = !_uiState.value.audioPreviewPlaying
            val playing = NativeEngineBridge.nativeToggleAudioPreview(nextPlay, startMs)
            _uiState.update { it.copy(audioPreviewPlaying = playing) }
        }
    }

    fun setSparrowBinding(entityId: String, animKind: String, prefix: String) {
        viewModelScope.launch(Dispatchers.IO) {
            applyActionJson(NativeEngineBridge.nativeSetSparrowBinding(entityId, animKind, prefix))
        }
    }

    suspend fun querySparrowFrame(entityId: String, animKind: String, elapsedSec: Float): SparrowFrameInfo {
        return withContext(Dispatchers.IO) {
            val raw = NativeEngineBridge.nativeGetSparrowFrameInfo(entityId, animKind, elapsedSec)
            StudioJsonParser.parseSparrowFrameInfo(raw)
        }
    }

    fun addStageEntity(name: String, role: String) {
        viewModelScope.launch(Dispatchers.IO) {
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
        flipX: Boolean
    ) {
        viewModelScope.launch(Dispatchers.IO) {
            val raw = NativeEngineBridge.nativeUpdateStageEntity(
                entityId,
                name,
                role,
                dx,
                dy,
                absX,
                absY,
                useAbsPos,
                scale,
                opacity,
                visible,
                flipX
            )
            applyActionJson(raw, silentOk = true)
        }
    }

    fun reorderStageEntity(entityId: String, action: String) {
        viewModelScope.launch(Dispatchers.IO) {
            applyActionJson(NativeEngineBridge.nativeReorderStageEntity(entityId, action), silentOk = true)
        }
    }

    fun addScript(language: String) {
        viewModelScope.launch(Dispatchers.IO) {
            val raw = NativeEngineBridge.nativeAddScript(language)
            applyActionJson(raw)
            val newest = _uiState.value.snapshot.scripts.lastOrNull()?.id
            if (!newest.isNullOrBlank()) {
                _uiState.update { it.copy(selectedScriptId = newest, scriptValidationMsg = null) }
            }
        }
    }

    fun updateScript(scriptId: String, enabled: Boolean, sourceCode: String, silent: Boolean = true) {
        viewModelScope.launch(Dispatchers.IO) {
            val raw = NativeEngineBridge.nativeUpdateScript(scriptId, enabled, sourceCode)
            applyActionJson(raw, silentOk = silent)
        }
    }

    fun validateScript(scriptId: String, currentEnabled: Boolean, currentSource: String) {
        viewModelScope.launch(Dispatchers.IO) {
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
        viewModelScope.launch(Dispatchers.IO) {
            applyActionJson(NativeEngineBridge.nativeRestoreDefaultArrowSvg())
            reloadArrowBitmaps()
        }
    }

    fun startCompileAndPlay() {
        viewModelScope.launch(Dispatchers.IO) {
            if (_uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            _uiState.update {
                it.copy(
                    currentScreen = StudioScreen.HotCompile,
                    audioPreviewPlaying = false,
                    isBusy = true,
                    statusBanner = null
                )
            }

            val tier = _uiState.value.selectedTier
            val compileRaw = NativeEngineBridge.nativeHotCompile(tier.code)
            val compileRes = StudioJsonParser.parseActionResult(compileRaw)
            val snap = compileRes.snapshot ?: _uiState.value.snapshot

            val compiledMap = loadCompiledBitmapsInternal(
                snap.lastCompileReport?.compiledTexturePaths ?: emptyMap(),
                snap.stageEntities
            )
            val arrows = loadArrowBitmapsInternal()
            val receptors = loadReceptorBitmapsInternal()

            val gpRaw = NativeEngineBridge.nativeStartGameplay()
            val gpStep = StudioJsonParser.parseGameplayStep(gpRaw)

            _uiState.update {
                it.copy(
                    isBusy = false,
                    snapshot = snap,
                    compiledBitmaps = compiledMap,
                    arrowBitmaps = arrows,
                    receptorBitmaps = receptors,
                    gameplayStep = gpStep,
                    currentScreen = StudioScreen.Gameplay
                )
            }
        }
    }

    suspend fun stepGameplayFrame(dtSec: Float, pressedMask: Int, justPressedMask: Int) {
        val step = withContext(Dispatchers.IO) {
            val raw = NativeEngineBridge.nativeStepGameplay(dtSec, pressedMask, justPressedMask)
            StudioJsonParser.parseGameplayStep(raw)
        }
        _uiState.update { it.copy(gameplayStep = step) }
    }

    fun setGameplayPaused(paused: Boolean) {
        viewModelScope.launch(Dispatchers.IO) {
            val raw = NativeEngineBridge.nativeSetGameplayPaused(paused)
            val step = StudioJsonParser.parseGameplayStep(raw)
            _uiState.update { it.copy(gameplayStep = step) }
        }
    }

    fun restartGameplay() {
        viewModelScope.launch(Dispatchers.IO) {
            val gpRaw = NativeEngineBridge.nativeStartGameplay()
            val gpStep = StudioJsonParser.parseGameplayStep(gpRaw)
            _uiState.update { it.copy(gameplayStep = gpStep) }
        }
    }

    fun stopGameplayAndNavigate(target: StudioScreen = StudioScreen.Hub) {
        viewModelScope.launch(Dispatchers.IO) {
            val raw = NativeEngineBridge.nativeStopGameplay()
            val res = StudioJsonParser.parseActionResult(raw)
            _uiState.update {
                it.copy(
                    currentScreen = target,
                    snapshot = res.snapshot ?: it.snapshot,
                    gameplayStep = GameplayStep()
                )
            }
        }
    }

    private fun applyActionJson(rawJson: String, silentOk: Boolean = false) {
        val result = StudioJsonParser.parseActionResult(rawJson)
        _uiState.update { state ->
            val nextSnap = result.snapshot ?: state.snapshot
            val validEntity = nextSnap.stageEntities.any { it.id == state.selectedEntityId }
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

    private fun reloadEntityBitmaps(entities: List<StageEntitySnapshot>) {
        val map = loadEntityBitmapsInternal(entities)
        _uiState.update { it.copy(entityBitmaps = map) }
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

    private fun loadEntityBitmapsInternal(entities: List<StageEntitySnapshot>): Map<String, LoadedEntityBitmap> {
        val out = linkedMapOf<String, LoadedEntityBitmap>()
        for (ent in entities) {
            val path = ent.imagePath ?: continue
            decodeEntityBitmapWithOrigDims(path, path, maxDecodeSide = 4096)?.let { loaded ->
                out[ent.id] = loaded
            }
        }
        return out
    }

    private fun loadCompiledBitmapsInternal(
        compiledPaths: Map<String, String>,
        entities: List<StageEntitySnapshot>
    ): Map<String, LoadedEntityBitmap> {
        val out = linkedMapOf<String, LoadedEntityBitmap>()
        val origPaths = entities.associate { it.id to it.imagePath }
        for (ent in entities) {
            val cPath = compiledPaths[ent.id] ?: origPaths[ent.id] ?: continue
            val oPath = origPaths[ent.id] ?: cPath
            decodeEntityBitmapWithOrigDims(cPath, oPath, maxDecodeSide = 4096)?.let { loaded ->
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
            inPreferredConfig = android.graphics.Bitmap.Config.ARGB_8888
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
