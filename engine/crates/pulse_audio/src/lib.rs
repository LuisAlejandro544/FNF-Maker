//! Archivo: engine/crates/pulse_audio/src/lib.rs
//! Propósito: Punto de entrada de `pulse_audio` (100% Rust puro, sin C++).
//! Expone el decodificador multiformato (OGG, WAV, MP3), el mezclador de 1 a 3 pistas
//! con reloj atómico de frames y el backend nativo AAudio en Rust puro para Android.

pub mod decoder;
pub mod mixer;
pub mod rust_audio_backend;

pub use decoder::{DecodedTrack, decode_audio_file};
pub use mixer::{AudioMixerState, TrackRuntimeConfig};
pub use rust_audio_backend::AudioEngineHandle;
