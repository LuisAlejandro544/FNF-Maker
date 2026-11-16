//! Archivo: engine/crates/pulse_audio/src/oboe_backend.rs
//! Propósito: Implementa la salida de audio de ultra-baja latencia usando Google Oboe (`oboe` crate)
//! en dispositivos Android (`AAudio` en Android 8.1+ o `OpenSL ES` en dispositivos más antiguos).
//! En compilaciones de escritorio/host no-Android provee un manejador seguro sin bloquear la UI.

use crate::mixer::AudioMixerState;
use std::sync::Arc;

/// Manejador vivo del stream de Oboe. Mientras permanezca en memoria, el audio fluye.
/// Se inicializa de forma diferida (`new_deferred` + `ensure_started`) para no bloquear ni
/// provocar fallos de AAudio/OEM durante el arranque inicial de la actividad Android.
pub struct AudioEngineHandle {
    pub mixer: Arc<AudioMixerState>,
    #[cfg(target_os = "android")]
    _stream: Option<oboe::AudioStreamAsync<oboe::Output, OboeCallback>>,
    #[cfg(target_os = "android")]
    start_attempted: bool,
}

impl AudioEngineHandle {
    /// Crea el estado del mezclador sin abrir todavía hardware de audio Android.
    pub fn new_deferred() -> Self {
        let default_sr = 48_000_u32;
        let mixer = AudioMixerState::new(default_sr);
        Self {
            mixer,
            #[cfg(target_os = "android")]
            _stream: None,
            #[cfg(target_os = "android")]
            start_attempted: false,
        }
    }

    /// Inicializa inmediatamente el motor de audio (útil en pruebas o invocación directa).
    pub fn start_default() -> Self {
        let mut handle = Self::new_deferred();
        handle.ensure_started();
        handle
    }

    /// Abre el stream de baja latencia Google Oboe bajo demanda (idempotente) con fallback
    /// seguro si el modo LowLatency o 48 kHz falla en algún dispositivo Android específico.
    pub fn ensure_started(&mut self) {
        #[cfg(target_os = "android")]
        {
            if self._stream.is_some() || self.start_attempted {
                return;
            }
            self.start_attempted = true;

            use oboe::{
                AudioStream, AudioStreamBase, AudioStreamBuilder, PerformanceMode, SharingMode,
                Stereo,
            };

            let try_open = |low_latency: bool, explicit_sr: Option<i32>| {
                let callback = OboeCallback {
                    mixer: Arc::clone(&self.mixer),
                };
                let mut builder = AudioStreamBuilder::default()
                    .set_performance_mode(if low_latency {
                        PerformanceMode::LowLatency
                    } else {
                        PerformanceMode::None
                    })
                    .set_sharing_mode(SharingMode::Shared)
                    .set_format::<f32>()
                    .set_channel_count::<Stereo>();
                if let Some(sr) = explicit_sr {
                    builder = builder.set_sample_rate(sr);
                }
                builder.set_callback(callback).open_stream()
            };

            let opened = try_open(true, Some(48_000))
                .or_else(|err| {
                    log::warn!("Oboe LowLatency 48kHz falló ({:?}); reintentando con perfil seguro...", err);
                    try_open(false, None)
                });

            match opened {
                Ok(mut stream) => {
                    let actual_sr = stream.get_sample_rate() as u32;
                    if actual_sr > 0 {
                        self.mixer
                            .sample_rate
                            .store(actual_sr, std::sync::atomic::Ordering::Relaxed);
                    }
                    if let Err(err) = stream.start() {
                        log::error!("Error iniciando stream Oboe: {:?}", err);
                    } else {
                        log::info!("Stream Oboe iniciado a {} Hz", actual_sr);
                    }
                    self._stream = Some(stream);
                }
                Err(err) => {
                    log::error!("No se pudo abrir Oboe AudioStream (continuando en modo silencioso): {:?}", err);
                }
            }
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
