// Archivo: android/app/src/main/kotlin/com/beatforge/studio/models/AudioAdaptModels.kt
// Propósito: Modelos de pistas de audio del mezclador y telemetría/reporte del adaptador
// acústico adaptativo (EBU R128, FFT 5 bandas, filtros biquad y perfil de salida AAudio).

package com.beatforge.studio

data class AudioTrackSnapshot(
    val slotIndex: Int = 0,
    val roleLabel: String = "",
    val fileName: String? = null,
    val gain: Float = 1f,
    val muted: Boolean = false,
    val duckOnPlayerMiss: Boolean = false,
    val durationMs: Double = 0.0,
    val waveformPeaks: List<Float> = emptyList()
)

data class AudioAdaptProgress(
    val active: Boolean = false,
    val progress: Float = 0f,
    val phase: String = "Calibrando el sonido...",
    val detail: String = ""
)

data class SpectralBandProfileSnapshot(
    val subBass2090Hz: Float = 0f,
    val bass90220Hz: Float = 0f,
    val lowMid220600Hz: Float = 0f,
    val presence2k4kHz: Float = 0f,
    val air6k14kHz: Float = 0f,
    val spectralTiltDb: Float = 0f
)

data class SongAcousticAnalysisSnapshot(
    val sampleRate: Int = 44_100,
    val durationSec: Float = 0f,
    val integratedLufs: Float = -14f,
    val loudnessRangeLu: Float = 0f,
    val truePeakDbtp: Float = -1f,
    val crestFactorDb: Float = 0f,
    val stereoCorrelation: Float = 1f,
    val bands: SpectralBandProfileSnapshot = SpectralBandProfileSnapshot()
)

data class EqBandAdjustmentSnapshot(
    val kind: String = "",
    val freqHz: Float = 0f,
    val gainDb: Float = 0f,
    val q: Float = 0.707f,
    val reason: String = ""
)

data class EffectiveDeviceAcousticsSnapshot(
    val route: String = "builtin_speaker",
    val routeLabel: String = "Altavoz del Teléfono",
    val deviceName: String = "Altavoz Integrado",
    val manufacturer: String = "Android",
    val model: String = "Device",
    val androidApiLevel: Int = 26,
    val effectiveSampleRate: Int = 48_000,
    val hardwareNativeSampleRate: Int = 48_000,
    val effectiveChannelCount: Int = 2,
    val isMonoSpeaker: Boolean = false,
    val framesPerBurst: Int = 192,
    val bufferCapacityFrames: Int = 0,
    val lowLatencyActive: Boolean = false,
    val aaudioActive: Boolean = false,
    val aaudioDeviceId: Int = 0,
    val aaudioMatched: Boolean = false,
    val estimatedOutputLatencyMs: Float = 0f,
    val recommendedAudioOffsetMs: Double = 0.0
)

data class AdaptiveDspPlanSnapshot(
    val profileName: String = "",
    val adaptationSummary: String = "",
    val targetLufs: Float = -14f,
    val appliedMakeupGainDb: Float = 0f,
    val ceilingDbtp: Float = -1f,
    val highpassHz: Float = 30f,
    val monoFoldEnabled: Boolean = false,
    val resampleNeeded: Boolean = false,
    val sourceSampleRate: Int = 44_100,
    val targetSampleRate: Int = 48_000,
    val stereoWidth: Float = 1f,
    val crossfeedStrength: Float = 0f,
    val limiterLookaheadMs: Float = 4f,
    val ditherApplied: Boolean = true,
    val highPrecisionF32Path: Boolean = false,
    val eqBands: List<EqBandAdjustmentSnapshot> = emptyList()
)

data class AudioAdaptationReportSnapshot(
    val enabled: Boolean = true,
    val bypassed: Boolean = false,
    val cacheReused: Boolean = false,
    val elapsedMs: Long = 0L,
    val outputWavPath: String? = null,
    val device: EffectiveDeviceAcousticsSnapshot = EffectiveDeviceAcousticsSnapshot(),
    val songAnalysis: SongAcousticAnalysisSnapshot? = null,
    val dspPlan: AdaptiveDspPlanSnapshot? = null,
    val outputTruePeakDbtp: Float = -1f,
    val appliedAutoOffsetMs: Double = 0.0
)
