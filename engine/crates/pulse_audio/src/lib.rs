//! Archivo: engine/crates/pulse_audio/src/lib.rs
//! Propósito: Punto de entrada de `pulse_audio` (100% Rust puro, sin C++).
//! Expone el decodificador multiformato (OGG, WAV, MP3), el mezclador de 1 a 3 pistas,
//! la fusión a un único archivo WAV compilado con streaming lock-free (`streaming`),
//! el adaptador acústico por hardware/canción (`adaptive_master`: EBU R128 + FFT + Biquad + Rubato)
//! y el backend nativo AAudio en Rust puro para Android.

pub mod adaptive_master;
pub mod audio_cache;
pub mod decoder;
pub mod mixer;
pub mod rust_audio_backend;
pub mod streaming;

pub use adaptive_master::{
    AdaptiveDspPlan, AndroidOutputRoute, AudioAdaptationReport, DeviceAudioProbeInput,
    EffectiveDeviceAcoustics, EqBandAdjustment, SongAcousticAnalysis, SpectralBandProfile,
    adapt_audio_for_device, analyze_song_acoustics, build_adaptive_dsp_plan,
    fuse_device_and_aaudio_probe,
};
pub use audio_cache::{compute_audio_cache_key, decode_audio_file_cached};
pub use decoder::{
    DecodedTrack, decode_audio_file, decode_audio_file_raw_f32, resample_stereo_f32_rubato,
};
pub use mixer::{AudioMixerState, TrackRuntimeConfig};
pub use rust_audio_backend::{AudioEngineHandle, AudioHardwareProbe};
pub use streaming::{AudioMergeReport, STREAM_RING_FRAMES, StreamingAudioSource, merge_tracks_to_wav};
