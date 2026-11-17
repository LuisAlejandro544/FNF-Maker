// Archivo: android/app/src/main/kotlin/com/beatforge/studio/viewmodel/StudioGameplayCoordinator.kt
// Propósito: Coordinador modular de `StudioViewModel` para el ciclo Compilar y Jugar:
// lanza `nativeHotCompile` en `engineIoDispatcher`, sondea el progreso en tiempo real sobre
// `Dispatchers.IO`, aplica exclusión mutua de bitmaps en RAM (editor vs gameplay), ejecuta
// pasos binarios `BFGP` v2 y restaura previews al volver al estudio.

package com.beatforge.studio.viewmodel

import androidx.compose.ui.graphics.asAndroidBitmap
import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.HotCompileProgress
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.NativeEngineBridge
import com.beatforge.studio.StudioJsonParser
import com.beatforge.studio.StudioScreen
import com.beatforge.studio.StudioUiState
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.Collections

internal class StudioGameplayCoordinator(
    private val scope: CoroutineScope,
    private val engineIoDispatcher: CoroutineDispatcher,
    private val uiState: MutableStateFlow<StudioUiState>,
    private val gameplayFrameState: GameplayFrameState,
    private val bitmapLoader: StudioBitmapLoader
) {
    private val loadingEntityIds: MutableSet<String> = Collections.synchronizedSet(HashSet())
    private val recycleLock = Any()
    private val pendingRecycleQueue = ArrayDeque<Pair<Long, LoadedEntityBitmap>>()

    fun startCompileAndPlay() {
        scope.launch(engineIoDispatcher) {
            clearRecycleState(recycleImmediately = true)
            NativeEngineBridge.nativeFlushSave()
            if (uiState.value.audioPreviewPlaying) {
                NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
            }
            uiState.update {
                it.copy(
                    currentScreen = StudioScreen.HotCompile,
                    audioPreviewPlaying = false,
                    isBusy = true,
                    hotCompileProgress = HotCompileProgress(
                        active = true,
                        progress = 0.02f,
                        phase = "Iniciando compilador en caliente...",
                        detail = "Verificando caché incremental y sprites WebP"
                    ),
                    statusBanner = null,
                    // Liberar bitmaps del editor antes de cargar los bitmaps compilados de partida
                    entityBitmaps = emptyMap(),
                    compiledBitmaps = emptyMap(),
                    compiledBitmapsByIndex = emptyList()
                )
            }

            bitmapLoader.evictRegionCache()
            val tier = uiState.value.selectedTier

            // Sondeo real no bloqueante sobre `Dispatchers.IO` (nunca usa `engineIoDispatcher` ni `SESSION`)
            val progressPoller = scope.launch(Dispatchers.IO) {
                while (isActive) {
                    val live = StudioJsonParser.parseHotCompileProgress(
                        NativeEngineBridge.nativeGetHotCompileProgress()
                    )
                    if (live.active || live.progress > 0f) {
                        uiState.update { st ->
                            val clampedPct = maxOf(st.hotCompileProgress.progress, live.progress)
                            st.copy(
                                hotCompileProgress = live.copy(progress = clampedPct)
                            )
                        }
                    }
                    delay(65L)
                }
            }

            val compileRaw = try {
                NativeEngineBridge.nativeHotCompile(tier.code)
            } finally {
                progressPoller.cancel()
            }

            val compileRes = StudioJsonParser.parseActionResult(compileRaw)
            val snap = compileRes.snapshot ?: uiState.value.snapshot

            uiState.update {
                it.copy(
                    hotCompileProgress = HotCompileProgress(
                        active = true,
                        progress = 0.92f,
                        phase = "Cargando sprites visibles en RAM",
                        detail = "Omitiendo sprites ocultos (se precargan al vuelo desde .webp)"
                    )
                )
            }

            val compiledMap = bitmapLoader.loadCompiledEntityBitmaps(
                snap.lastCompileReport?.compiledTexturePaths ?: emptyMap(),
                snap.stageEntities,
                tier,
                onlyInitiallyVisible = true
            )
            val arrows = bitmapLoader.loadArrowBitmaps()
            val receptors = bitmapLoader.loadReceptorBitmaps()
            val hudAssets = bitmapLoader.loadHudSvgAssets()
            val (normalIcons, losingIcons) = bitmapLoader.loadEntityIcons(snap.stageEntities)

            uiState.update {
                it.copy(
                    hotCompileProgress = HotCompileProgress(
                        active = true,
                        progress = 0.97f,
                        phase = "Arrancando máquinas virtuales y audio",
                        detail = "Cargando Lua bytecode, QuickJS y streaming WAV"
                    )
                )
            }

            val gpRaw = NativeEngineBridge.nativeStartGameplay()
            val gpStep = StudioJsonParser.parseGameplayStep(gpRaw)
            val compiledByIndex = gpStep.entities.map { compiledMap[it.id] }

            withContext(Dispatchers.Main) {
                gameplayFrameState.seedFromJsonStep(gpStep)
            }

            uiState.update {
                it.copy(
                    isBusy = false,
                    hotCompileProgress = HotCompileProgress(
                        active = false,
                        progress = 1.0f,
                        phase = "Compilación completada",
                        detail = "Iniciando gameplay..."
                    ),
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
            reconcileLazyBitmaps()
        }
    }

    private fun reconcileLazyBitmaps() {
        val ids = gameplayFrameState.entityIdsByIndex
        if (ids.isEmpty()) return

        val st = uiState.value
        val currentMap = st.compiledBitmaps
        val count = minOf(gameplayFrameState.entityCount, ids.size)
        var retiredMap: LinkedHashMap<String, LoadedEntityBitmap>? = null

        for (i in 0 until count) {
            val entId = ids[i]
            val ent = gameplayFrameState.entitiesPool[i]
            val existing = currentMap[entId]

            if (ent.retired && existing != null) {
                if (retiredMap == null) {
                    retiredMap = LinkedHashMap(currentMap)
                }
                retiredMap.remove(entId)
                synchronized(recycleLock) {
                    pendingRecycleQueue.addLast(gameplayFrameState.drawTick to existing)
                }
            } else if (ent.preloadRequested && !ent.retired && existing == null && loadingEntityIds.add(entId)) {
                val compiledPaths = st.snapshot.lastCompileReport?.compiledTexturePaths ?: emptyMap()
                val stageEntities = st.snapshot.stageEntities
                val tier = st.selectedTier
                scope.launch(Dispatchers.IO) {
                    try {
                        val decoded = bitmapLoader.decodeSingleCompiledEntityBitmap(
                            entityId = entId,
                            compiledPaths = compiledPaths,
                            entities = stageEntities,
                            tier = tier
                        )
                        if (decoded != null && gameplayFrameState.active) {
                            var droppedEarly = false
                            uiState.update { prev ->
                                val idxNow = gameplayFrameState.entityIdsByIndex.indexOf(entId)
                                val stillRetired = idxNow >= 0 &&
                                    idxNow < gameplayFrameState.entityCount &&
                                    gameplayFrameState.entitiesPool[idxNow].retired
                                if (stillRetired || !gameplayFrameState.active) {
                                    droppedEarly = true
                                    prev
                                } else {
                                    val nextMap = LinkedHashMap(prev.compiledBitmaps).apply {
                                        put(entId, decoded)
                                    }
                                    val nextByIdx = gameplayFrameState.entityIdsByIndex.map { nextMap[it] }
                                    prev.copy(
                                        compiledBitmaps = nextMap,
                                        compiledBitmapsByIndex = nextByIdx
                                    )
                                }
                            }
                            if (droppedEarly) {
                                val ab = decoded.bitmap.asAndroidBitmap()
                                if (!ab.isRecycled) ab.recycle()
                            }
                        } else if (decoded != null) {
                            val ab = decoded.bitmap.asAndroidBitmap()
                            if (!ab.isRecycled) ab.recycle()
                        }
                    } finally {
                        loadingEntityIds.remove(entId)
                    }
                }
            }
        }

        if (retiredMap != null) {
            val finalizedMap = retiredMap
            val finalizedByIdx = ids.map { finalizedMap[it] }
            uiState.update { prev ->
                prev.copy(
                    compiledBitmaps = finalizedMap,
                    compiledBitmapsByIndex = finalizedByIdx
                )
            }
        }

        drainSafeRecycles(gameplayFrameState.drawTick)
    }

    private fun drainSafeRecycles(currentTick: Long) {
        synchronized(recycleLock) {
            while (pendingRecycleQueue.isNotEmpty()) {
                val (dropTick, bmp) = pendingRecycleQueue.first()
                if (currentTick - dropTick < 3L) {
                    break
                }
                pendingRecycleQueue.removeFirst()
                val ab = bmp.bitmap.asAndroidBitmap()
                if (!ab.isRecycled) {
                    ab.recycle()
                }
            }
        }
    }

    private fun clearRecycleState(recycleImmediately: Boolean) {
        loadingEntityIds.clear()
        synchronized(recycleLock) {
            while (pendingRecycleQueue.isNotEmpty()) {
                val (_, bmp) = pendingRecycleQueue.removeFirst()
                if (recycleImmediately) {
                    val ab = bmp.bitmap.asAndroidBitmap()
                    if (!ab.isRecycled) {
                        ab.recycle()
                    }
                }
            }
        }
    }

    fun setGameplayPaused(paused: Boolean) {
        scope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeSetGameplayPaused(paused)
            val step = StudioJsonParser.parseGameplayStep(raw)
            withContext(Dispatchers.Main) {
                gameplayFrameState.seedFromJsonStep(step)
            }
        }
    }

    fun restartGameplay() {
        scope.launch(engineIoDispatcher) {
            clearRecycleState(recycleImmediately = true)
            val st = uiState.value
            val compiledPaths = st.snapshot.lastCompileReport?.compiledTexturePaths ?: emptyMap()
            val tier = st.selectedTier

            // Re-decodificar desde `compiled_cache/*.webp` cualquier sprite inicial que haya sido reciclado en el intento anterior
            val restoredMap = LinkedHashMap(st.compiledBitmaps)
            withContext(Dispatchers.IO) {
                for (ent in st.snapshot.stageEntities) {
                    if (ent.visible && ent.opacity > 0.005f && !restoredMap.containsKey(ent.id)) {
                        bitmapLoader.decodeSingleCompiledEntityBitmap(
                            entityId = ent.id,
                            compiledPaths = compiledPaths,
                            entities = st.snapshot.stageEntities,
                            tier = tier
                        )?.let { decoded ->
                            restoredMap[ent.id] = decoded
                        }
                    }
                }
            }

            val gpRaw = NativeEngineBridge.nativeStartGameplay()
            val gpStep = StudioJsonParser.parseGameplayStep(gpRaw)
            val compiledByIndex = gpStep.entities.map { restoredMap[it.id] }
            withContext(Dispatchers.Main) {
                gameplayFrameState.seedFromJsonStep(gpStep)
            }
            uiState.update {
                it.copy(
                    compiledBitmaps = restoredMap,
                    compiledBitmapsByIndex = compiledByIndex
                )
            }
        }
    }

    fun stopGameplayAndNavigate(target: StudioScreen) {
        scope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeStopGameplay()
            val res = StudioJsonParser.parseActionResult(raw)
            val nextSnap = res.snapshot ?: uiState.value.snapshot
            val bitmapsToFree = uiState.value.compiledBitmaps.values.toList()
            withContext(Dispatchers.Main) {
                gameplayFrameState.reset()
            }
            // Liberar primero referencias en Compose UI antes de reciclar los bitmaps
            uiState.update {
                it.copy(
                    compiledBitmaps = emptyMap(),
                    compiledBitmapsByIndex = emptyList()
                )
            }
            clearRecycleState(recycleImmediately = true)
            for (bmp in bitmapsToFree) {
                val ab = bmp.bitmap.asAndroidBitmap()
                if (!ab.isRecycled) {
                    ab.recycle()
                }
            }
            val restoredEditorBitmaps = bitmapLoader.loadEditorEntityBitmaps(
                nextSnap.stageEntities,
                uiState.value.selectedTier
            )
            uiState.update {
                it.copy(
                    currentScreen = target,
                    snapshot = nextSnap,
                    entityBitmaps = restoredEditorBitmaps
                )
            }
        }
    }
}
