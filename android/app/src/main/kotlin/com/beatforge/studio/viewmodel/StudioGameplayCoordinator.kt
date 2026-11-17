// Archivo: android/app/src/main/kotlin/com/beatforge/studio/viewmodel/StudioGameplayCoordinator.kt
// Propósito: Coordinador modular de `StudioViewModel` para el ciclo Compilar y Jugar:
// lanza `nativeHotCompile` en `engineIoDispatcher`, sondea el progreso en tiempo real sobre
// `Dispatchers.IO`, aplica exclusión mutua de bitmaps en RAM (editor vs gameplay), ejecuta
// pasos binarios `BFGP` v2 y restaura previews al volver al estudio.

package com.beatforge.studio.viewmodel

import com.beatforge.studio.GameplayFrameState
import com.beatforge.studio.HotCompileProgress
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

internal class StudioGameplayCoordinator(
    private val scope: CoroutineScope,
    private val engineIoDispatcher: CoroutineDispatcher,
    private val uiState: MutableStateFlow<StudioUiState>,
    private val gameplayFrameState: GameplayFrameState,
    private val bitmapLoader: StudioBitmapLoader
) {
    fun startCompileAndPlay() {
        scope.launch(engineIoDispatcher) {
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
                        phase = "Cargando sprites WebP en memoria",
                        detail = "Decodificando hojas compiladas e íconos HUD"
                    )
                )
            }

            val compiledMap = bitmapLoader.loadCompiledEntityBitmaps(
                snap.lastCompileReport?.compiledTexturePaths ?: emptyMap(),
                snap.stageEntities,
                tier
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
            val gpRaw = NativeEngineBridge.nativeStartGameplay()
            val gpStep = StudioJsonParser.parseGameplayStep(gpRaw)
            val compiledMap = uiState.value.compiledBitmaps
            val compiledByIndex = gpStep.entities.map { compiledMap[it.id] }
            withContext(Dispatchers.Main) {
                gameplayFrameState.seedFromJsonStep(gpStep)
            }
            uiState.update { it.copy(compiledBitmapsByIndex = compiledByIndex) }
        }
    }

    fun stopGameplayAndNavigate(target: StudioScreen) {
        scope.launch(engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeStopGameplay()
            val res = StudioJsonParser.parseActionResult(raw)
            val nextSnap = res.snapshot ?: uiState.value.snapshot
            withContext(Dispatchers.Main) {
                gameplayFrameState.reset()
            }
            // Liberar primero los bitmaps compilados de gameplay antes de recargar previews del editor
            uiState.update {
                it.copy(
                    compiledBitmaps = emptyMap(),
                    compiledBitmapsByIndex = emptyList()
                )
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
