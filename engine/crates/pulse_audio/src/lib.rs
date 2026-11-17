//! Archivo: engine/crates/pulse_audio/src/lib.rs
//! Propósito: Punto de entrada de `pulse_audio` (100% Rust puro, sin C++).
//! Expone el decodificador multiformato (OGG, WAV, MP3), el mezclador de 1 a 3 pistas,
//! la fusión a un único archivo WAV compilado con streaming lock-free (`streaming`)
//! y el backend nativo AAudio en Rust puro para Android.

pub mod audio_cache;
pub mod decoder;
pub mod mixer;
pub mod rust_audio_backend;
pub mod streaming;

pub use audio_cache::{compute_audio_cache_key, decode_audio_file_cached};
pub use decoder::{DecodedTrack, decode_audio_file};
pub use mixer::{AudioMixerState, TrackRuntimeConfig};
pub use rust_audio_backend::AudioEngineHandle;
pub use streaming::{AudioMergeReport, STREAM_RING_FRAMES, StreamingAudioSource, merge_tracks_to_wav};
