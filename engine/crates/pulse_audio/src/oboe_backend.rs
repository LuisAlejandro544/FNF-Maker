//! Archivo: engine/crates/pulse_audio/src/oboe_backend.rs
//! Propósito: Implementa la salida de audio de ultra-baja latencia usando Google Oboe (`oboe` crate)
//! en dispositivos Android (`AAudio` en Android 8.1+ o `OpenSL ES` en dispositivos más antiguos).
//! En compilaciones de escritorio/host no-Android provee un manejador seguro sin bloquear la UI.

use crate::mixer::AudioMixerState;
use std::sync::Arc;

/// Manejador vivo del stream de Oboe. Mientras permanezca en memoria, el audio fluye.
pub struct AudioEngineHandle {
    pub mixer: Arc<AudioMixerState>,
    #[cfg(target_os = "android")]
    _stream: Option<oboe::AudioStreamAsync<oboe::Output, OboeCallback>>,
}

impl AudioEngineHandle {
    /// Inicializa el motor de audio de baja latencia Oboe (48 kHz estéreo f32 en Android).
    pub fn start_default() -> Self {
        let default_sr = 48_000_u32;
        let mixer = AudioMixerState::new(default_sr);

        #[cfg(target_os = "android")]
        {
            use oboe::{
                AudioOutputCallback, AudioStreamBuilder, DataCallbackResult, PerformanceMode,
                SharingMode, Stereo,
            };

            let callback = OboeCallback {
                mixer: Arc::clone(&mixer),
            };

            match AudioStreamBuilder::default()
                .set_performance_mode(PerformanceMode::LowLatency)
                .set_sharing_mode(SharingMode::Shared)
                .set_format::<f32>()
                .set_channel_count::<Stereo>()
                .set_sample_rate(default_sr as i32)
                .set_callback(callback)
                .open_stream()
            {
                Ok(mut stream) => {
                    let actual_sr = stream.get_sample_rate() as u32;
                    if actual_sr > 0 {
                        mixer
                            .sample_rate
                            .store(actual_sr, std::sync::atomic::Ordering::Relaxed);
                    }
                    if let Err(err) = stream.start() {
                        log::error!("Error iniciando stream Oboe: {:?}", err);
                    } else {
                        log::info!("Stream Oboe iniciado a {} Hz", actual_sr);
                    }
                    return Self {
                        mixer,
                        _stream: Some(stream),
                    };
                }
                Err(err) => {
                    log::error!("No se pudo abrir Oboe AudioStream: {:?}", err);
                }
            }
        }

        Self {
            mixer,
            #[cfg(target_os = "android")]
            _stream: None,
        }
    }
}

#[cfg(target_os = "android")]
struct OboeCallback {
    mixer: Arc<AudioMixerState>,
}

#[cfg(target_os = "android")]
impl oboe::AudioOutputCallback for OboeCallback {
    type FrameType = (f32, oboe::Stereo);

    fn on_audio_ready(
        &mut self,
        _stream: &mut dyn oboe::AudioOutputStreamSafe,
        frames: &mut [(f32, f32)],
    ) -> oboe::DataCallbackResult {
        // Reinterpretamos el slice de tuplas `(f32, f32)` como slice plano `[L, R, L, R...]` sin copiar memoria.
        let len_floats = frames.len() * 2;
        let ptr = frames.as_mut_ptr() as *mut f32;
        let interleaved = unsafe { std::slice::from_raw_parts_mut(ptr, len_floats) };
        self.mixer.render_audio_frames(interleaved);
        oboe::DataCallbackResult::Continue
    }
}
