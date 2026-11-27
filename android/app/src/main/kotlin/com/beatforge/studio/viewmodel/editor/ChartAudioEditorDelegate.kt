// Archivo: android/app/src/main/kotlin/com/beatforge/studio/viewmodel/editor/ChartAudioEditorDelegate.kt
// Propósito: Subdelegado para edición del Chart (BPM, secciones, notas, eventos, cámara de sección,
// intro UI) y control del mezclador/reproducción de pistas de audio y Adaptador Acústico.

package com.beatforge.studio.viewmodel.editor

import com.beatforge.studio.AudioPreviewTransportState
import com.beatforge.studio.NativeEngineBridge
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

internal class ChartAudioEditorDelegate(
    private val helper: EditorStateHelper
) {
    private val audioStateBuf = DoubleArray(3)
    private val fastAudioStateBuf = DoubleArray(3)

    fun restoreDemoChart() {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeRestoreDemoChart())
        }
    }

    fun setChartBpm(bpm: Double) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeSetChartBpm(bpm), silentOk = true)
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
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(
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
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(
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
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(
                NativeEngineBridge.nativeSetSectionBpm(secIndex, bpm, changeBpm),
                silentOk = true
            )
        }
    }

    fun addChartEvent(timeMs: Double, name: String, value1: String, value2: String) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(
                NativeEngineBridge.nativeAddChartEvent(timeMs, name, value1, value2)
            )
        }
    }

    fun removeChartEvent(index: Int) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeRemoveChartEvent(index), silentOk = true)
        }
    }

    fun addChartSection(mustHit: Boolean) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeAddChartSection(mustHit))
        }
    }

    fun toggleSectionMustHit(secIndex: Int) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeToggleSectionMustHit(secIndex), silentOk = true)
        }
    }

    fun setSectionCameraSlot(secIndex: Int, cameraSlot: Int?) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(
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
        helper.scope.launch(helper.engineIoDispatcher) {
            val raw = if (noteType == "default") {
                NativeEngineBridge.nativeToggleChartNote(secIndex, stepInSec, col0To7, sustainSteps)
            } else {
                NativeEngineBridge.nativeToggleChartNoteWithType(secIndex, stepInSec, col0To7, sustainSteps, noteType)
            }
            helper.applyActionJson(raw, silentOk = true)
        }
    }

    fun removeAudioTrack(slot: Int) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.applyActionJson(NativeEngineBridge.nativeRemoveAudioTrack(slot))
        }
    }

    fun updateAudioTrackConfig(slot: Int, gain: Float, muted: Boolean, duckOnMiss: Boolean) {
        helper.scope.launch(helper.engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeUpdateAudioTrackConfig(slot, gain, muted, duckOnMiss)
            helper.applyActionJson(raw, silentOk = true)
        }
    }

    fun setAudioAdaptEnabled(enabled: Boolean) {
        helper.scope.launch(helper.engineIoDispatcher) {
            val raw = NativeEngineBridge.nativeSetAudioAdaptEnabled(enabled)
            helper.applyActionJson(raw, silentOk = true)
        }
    }

    fun toggleAudioPreview(startMs: Double) {
        helper.scope.launch(helper.engineIoDispatcher) {
            val nextPlay = !helper.uiState.value.audioPreviewPlaying
            val playing = NativeEngineBridge.nativeToggleAudioPreview(nextPlay, startMs)
            helper.uiState.update { it.copy(audioPreviewPlaying = playing) }
        }
    }

    fun setAudioPreviewTransport(play: Boolean, seekMs: Double = -1.0) {
        helper.scope.launch(helper.engineIoDispatcher) {
            val playing = NativeEngineBridge.nativeSetAudioPreviewTransport(play, seekMs)
            helper.uiState.update { it.copy(audioPreviewPlaying = playing) }
        }
    }

    suspend fun readAudioPreviewState(): AudioPreviewTransportState {
        return withContext(helper.engineIoDispatcher) {
            val ok = NativeEngineBridge.nativeReadAudioPreviewState(audioStateBuf)
            if (!ok) {
                AudioPreviewTransportState()
            } else {
                val posMs = audioStateBuf[0].coerceAtLeast(0.0)
                val playing = audioStateBuf[1] >= 0.5
                val durationMs = audioStateBuf[2].coerceAtLeast(0.0)
                if (helper.uiState.value.audioPreviewPlaying != playing) {
                    helper.uiState.update { it.copy(audioPreviewPlaying = playing) }
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
        if (helper.uiState.value.audioPreviewPlaying != playing) {
            helper.uiState.update { it.copy(audioPreviewPlaying = playing) }
        }
        return AudioPreviewTransportState(
            posMs = posMs,
            isPlaying = playing,
            durationMs = durationMs
        )
    }
}
