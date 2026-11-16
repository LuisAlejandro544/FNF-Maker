//! Archivo: engine/crates/pulse_audio/src/lib.rs
//! Propósito: Punto de entrada de `pulse_audio`.
//! Expone el decodificador multiformato (OGG, WAV, MP3), el mezclador de 1 a 3 pistas
//! con reloj atómico de frames y el backend nativo de Google Oboe para Android.

pub mod decoder;
pub mod mixer;
pub mod oboe_backend;

pub use decoder::{DecodedTrack, decode_audio_file};
pub use mixer::{AudioMixerState, TrackRuntimeConfig};
pub use oboe_backend::AudioEngineHandle;
