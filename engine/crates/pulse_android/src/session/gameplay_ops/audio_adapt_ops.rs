//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/audio_adapt_ops.rs
//! Propósito: Fase independiente del Adaptador Acústico Adaptativo ejecutada después del
//! Compilador en Caliente y antes de iniciar el Gameplay. Combina el sondeo físico del teléfono
//! (AudioManager / AudioDeviceInfo + stream real de libaaudio.so) con análisis EBU R128 + FFT
//! de cada canción y genera un WAV calibrado para la salida activa mediante Biquad + Rubato.

use crate::session::EngineSession;
use pulse_audio::{DeviceAudioProbeInput, adapt_audio_for_device};
use pulse_core::disk_cache::touch_dir_last_used;
use std::path::PathBuf;

impl EngineSession {
    /// Ejecuta la fase independiente de adaptación acústica sin callback de progreso.
    pub fn adapt_audio_for_device(&mut self, device_probe_json: &str) -> String {
        self.adapt_audio_for_device_with_progress(device_probe_json, |_pct, _phase, _detail| {})
    }

    /// Ejecuta la adaptación acústica completa para el teléfono y salida activa reportados,
    /// escribiendo o reutilizando `compiled_cache/<id>/audio_adapted/adapted_<hash>.wav`.
    pub fn adapt_audio_for_device_with_progress<F>(
        &mut self,
        device_probe_json: &str,
        on_progress: F,
    ) -> String
    where
        F: Fn(f32, &str, &str),
    {
        let os_probe: DeviceAudioProbeInput = if device_probe_json.trim().is_empty() {
            DeviceAudioProbeInput::default()
        } else {
            serde_json::from_str(device_probe_json).unwrap_or_default()
        };

        let has_raw_active = self.project.audio_tracks.iter().any(|t| {
            !t.muted
                && t.file_path
                    .as_ref()
                    .is_some_and(|p| p.is_file())
        });
        if self.project.active_audio_count() > 0
            && !has_raw_active
            && self.decoded_tracks.iter().all(|t| t.is_none())
            && self.compiled_audio_path.is_none()
        {
            self.reload_project_audio_tracks();
        }

        let aaudio_probe = self.audio_engine.probe_aaudio_hardware();
        let active_cache_dir = self.active_compiled_cache_dir();
        let adapted_dir = active_cache_dir.join("audio_adapted");
        let _ = std::fs::create_dir_all(&adapted_dir);
        touch_dir_last_used(&active_cache_dir);

        let pre_merged = self.compiled_audio_path.as_deref();
        match adapt_audio_for_device(
            self.project.audio_adapt_enabled,
            &self.decoded_tracks,
            &self.project.audio_tracks,
            pre_merged,
            &os_probe,
            &aaudio_probe,
            &adapted_dir,
            on_progress,
        ) {
            Ok(report) => {
                self.adapted_audio_path = report.output_wav_path.as_deref().map(PathBuf::from);
                self.auto_audio_offset_ms = report.applied_auto_offset_ms;
                let msg = if report.bypassed {
                    if !report.enabled {
                        "Adaptador acústico desactivado (audio original)".to_string()
                    } else {
                        "Sin pistas externas (perfil hardware sondeo listo)".to_string()
                    }
                } else if let Some(plan) = &report.dsp_plan {
                    plan.adaptation_summary.clone()
                } else {
                    "Adaptación acústica completada".to_string()
                };
                self.last_audio_adapt_report = Some(report);
                self.ok_response(msg)
            }
            Err(err) => {
                self.adapted_audio_path = None;
                self.auto_audio_offset_ms = 0.0;
                self.last_audio_adapt_report = None;
                self.err_response(format!("Error en adaptador acústico: {}", err))
            }
        }
    }
}
