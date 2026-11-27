//! Archivo: engine/crates/pulse_android/src/session/editor_ops/audio_ops.rs
//! Propósito: Gestión de hasta 3 pistas de audio en el editor (`EngineSession`): adopción
//! permanente en `assets/audio/`, decodificación con caché `.bfaud`, configuración de
//! ganancia/mute/ducking en el mezclador AAudio y activación del adaptador acústico DSP.

use crate::session::{EngineSession, friendly_import_name};
use pulse_audio::decode_audio_file_cached;
use pulse_core::project::MAX_AUDIO_TRACKS;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

impl EngineSession {
    pub fn import_audio_track(&mut self, slot: usize, path: PathBuf) -> String {
        if slot >= MAX_AUDIO_TRACKS {
            return self.err_response(format!(
                "Máximo {} pistas permitidas; índice {} fuera de rango",
                MAX_AUDIO_TRACKS, slot
            ));
        }
        if !path.is_file() {
            return self.err_response("Archivo de audio no encontrado");
        }

        let friendly_name = friendly_import_name(&path);
        let stem_hint = PathBuf::from(&friendly_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("track")
            .to_string();

        let adopted_path = match self.adopt_permanent_asset(
            &path,
            "audio",
            &format!("slot{}_{}", slot, stem_hint),
        ) {
            Ok(p) => p,
            Err(e) => return self.err_response(e),
        };

        self.audio_engine.ensure_started();
        let target_sr = self
            .audio_engine
            .mixer
            .sample_rate
            .load(Ordering::Relaxed)
            .max(8000);
        let audio_cache_dir = self.audio_cache_dir();
        let audio_quota = self.cache_budget().audio_cache_bytes;

        match decode_audio_file_cached(&adopted_path, target_sr, Some(&audio_cache_dir), audio_quota)
        {
            Ok(decoded) => {
                let old_path = self.project.audio_tracks[slot].file_path.take();
                self.project.audio_tracks[slot].file_path = Some(adopted_path);
                self.project.audio_tracks[slot].file_name = Some(friendly_name);
                self.remove_managed_asset_if_unreferenced(old_path.as_deref());

                let arc_track = Arc::new(decoded);
                self.audio_engine
                    .mixer
                    .set_track(slot, Some(Arc::clone(&arc_track)));
                self.decoded_tracks[slot] = Some(arc_track);
                self.invalidate_compiled_bundle();
                self.invalidate_adapted_audio();
                self.save_active_project_now();
                self.ok_response(format!("Pista {} decodificada y lista en Rust AAudio", slot + 1))
            }
            Err(e) => {
                let _ = std::fs::remove_file(&adopted_path);
                self.err_response(format!("Error decodificando pista {}: {}", slot + 1, e))
            }
        }
    }

    pub fn remove_audio_track(&mut self, slot: usize) -> String {
        if slot < MAX_AUDIO_TRACKS {
            let old_path = self.project.audio_tracks[slot].file_path.take();
            self.project.audio_tracks[slot].file_name = None;
            self.decoded_tracks[slot] = None;
            self.audio_engine.mixer.set_track(slot, None);
            self.remove_managed_asset_if_unreferenced(old_path.as_deref());
            self.invalidate_compiled_bundle();
            self.invalidate_adapted_audio();
            self.save_active_project_now();
        }
        self.ok_response(format!("Pista {} liberada", slot + 1))
    }

    pub fn update_audio_track_config(
        &mut self,
        slot: usize,
        gain: f32,
        muted: bool,
        duck_on_miss: bool,
    ) -> String {
        if slot < MAX_AUDIO_TRACKS {
            self.project.audio_tracks[slot].gain = gain.clamp(0.0, 2.0);
            self.project.audio_tracks[slot].muted = muted;
            self.project.audio_tracks[slot].duck_on_player_miss = duck_on_miss;

            self.audio_engine.mixer.configs[slot].set_gain(gain);
            self.audio_engine.mixer.configs[slot]
                .muted
                .store(muted, Ordering::Relaxed);
            self.audio_engine.mixer.configs[slot]
                .duck_on_miss
                .store(duck_on_miss, Ordering::Relaxed);
            self.invalidate_compiled_bundle();
            self.invalidate_adapted_audio();
            self.mark_dirty_and_maybe_save(false);
        }
        self.ok_response("Configuración de pista actualizada")
    }

    pub fn set_audio_adapt_enabled(&mut self, enabled: bool) -> String {
        self.project.audio_adapt_enabled = enabled;
        self.invalidate_adapted_audio();
        self.save_active_project_now();
        self.ok_response(if enabled {
            "Adaptador acústico activado para tu teléfono"
        } else {
            "Adaptador acústico desactivado (audio original)"
        })
    }
}
