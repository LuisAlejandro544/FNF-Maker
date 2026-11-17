//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/mod.rs
//! Propósito: Coordinador modular del ciclo de compilación en caliente y gameplay en tiempo real.
//! Divide la lógica en submódulos especializados:
//! - `hot_compile_ops`: Compilación incremental en caché (`manifest.json`), WebP lossless y WAV mix.
//! - `script_sync`: Sincronización Lua/QuickJS, tweens, timers y comandos `ScriptCommand`.
//! - `simulation`: Simulación por frame (`step_gameplay_internal`) y evaluación de notas/sustain.
//! - `packet_encoder`: Serialización JSON y binaria `BFGP` v2 sin asignaciones por frame.

mod hot_compile_ops;
mod packet_encoder;
mod script_sync;
mod simulation;

use super::persistence_ops::sanitize_project_id;
use super::{EngineSession, GameplayRuntimeState, StrumOverrideState};
use pulse_audio::StreamingAudioSource;
use pulse_core::disk_cache::{evict_lru_subdirs, touch_dir_last_used};
use pulse_core::frame_memory::{FrameBumpArena, NoteSlidingWindow};
use pulse_core::hot_compile::{
    DeviceResourceTier, analyze_reused_entity_ids, hot_compile_project_to_dir,
};
use pulse_core::project::MAX_AUDIO_TRACKS;
use pulse_core::EntityRole;
use pulse_scripting::ModScriptRuntime;
use script_sync::sync_script_host_state;
use std::collections::HashSet;
use std::sync::atomic::Ordering;

pub const BINARY_FRAME_MAGIC: u32 = 0x4246_4750; // "BFGP"
pub const BINARY_HEADER_SIZE: usize = 152;
pub const BINARY_ENTITY_SIZE: usize = 64;
pub const BINARY_NOTE_SIZE: usize = 28;
pub const BINARY_STRUM_SIZE: usize = 20;

impl EngineSession {
    /// Inicializa la sesión de gameplay: instancia VMs Lua/QuickJS, abre el streaming WAV
    /// (liberando pistas PCM de la RAM del editor) y arranca la cuenta regresiva al BPM.
    pub fn start_gameplay(&mut self) -> String {
        self.gameplay = None;
        self.project.ensure_camera_script();
        self.project.ensure_stage_events_script();
        self.project.ensure_ui_blackout_script();
        self.project
            .stage
            .apply_camera_config(self.project.stage.camera.clone());
        self.project.stage.camera_shake = 0.0;

        let out_dir = self.active_compiled_cache_dir();
        let bundle = if let Some(b) = self.compiled_bundle.clone() {
            if out_dir.is_dir() {
                touch_dir_last_used(&out_dir);
            }
            b
        } else {
            let compiled = hot_compile_project_to_dir(
                &self.project,
                DeviceResourceTier::Balanced,
                Some(&out_dir),
                |_, _| {},
            );
            touch_dir_last_used(&out_dir);
            let safe_active = sanitize_project_id(&self.project.id);
            let _ = evict_lru_subdirs(
                &self.compiled_cache_root_dir(),
                self.cache_budget().compiled_cache_bytes,
                &[&safe_active],
            );
            compiled
        };

        self.project
            .chart
            .notes
            .sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        self.project
            .chart
            .events
            .sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));

        let active_player_id = bundle
            .active_entities
            .iter()
            .find(|e| e.role == EntityRole::Player && e.visible)
            .or_else(|| {
                bundle
                    .active_entities
                    .iter()
                    .find(|e| e.role == EntityRole::Player)
            })
            .map(|e| e.id.clone())
            .unwrap_or_default();

        let active_opponent_id = bundle
            .active_entities
            .iter()
            .find(|e| e.role == EntityRole::Opponent && e.visible)
            .or_else(|| {
                bundle
                    .active_entities
                    .iter()
                    .find(|e| e.role == EntityRole::Opponent)
            })
            .map(|e| e.id.clone())
            .unwrap_or_default();

        let reused_later_entities = analyze_reused_entity_ids(&self.project);

        let (mut runtime, script_errors) =
            ModScriptRuntime::load_from_scripts(&bundle.optimized_scripts);
        let initial_must_hit = self.project.chart.must_hit_at_ms(0.0);
        let default_strums = [StrumOverrideState::default(); 8];
        sync_script_host_state(
            &runtime.state,
            &self.project.stage,
            &bundle.active_entities,
            &active_player_id,
            &active_opponent_id,
            &default_strums,
            0.0,
            self.project.chart.bpm.max(40.0),
            initial_must_hit,
            0.5,
            0,
            0,
            0,
            0,
            100.0,
            self.project.chart.speed,
            self.project.downscroll,
        );
        runtime.call_on_create();

        self.audio_engine.mixer.pause();
        self.audio_engine.mixer.clear_stream_source();
        self.audio_engine.ensure_started();

        let compiled_audio = self
            .compiled_audio_path
            .clone()
            .or_else(|| bundle.report.compiled_audio_path.clone());
        let mut using_stream = false;
        if let Some(wav_path) = compiled_audio {
            if wav_path.exists() {
                if let Ok(stream) = StreamingAudioSource::open(&wav_path) {
                    self.audio_engine.mixer.set_stream_source(Some(stream));
                    self.audio_engine.mixer.clear_tracks();
                    for slot in 0..MAX_AUDIO_TRACKS {
                        self.decoded_tracks[slot] = None;
                    }
                    using_stream = true;
                }
            }
        }

        let any_audio = using_stream || self.project.active_audio_count() > 0;
        self.audio_engine
            .mixer
            .use_demo_synth
            .store(!any_audio, Ordering::Relaxed);
        self.audio_engine.mixer.demo_bpm_bits.store(
            (self.project.chart.bpm as f32).to_bits(),
            Ordering::Relaxed,
        );
        for (slot, cfg) in self.project.audio_tracks.iter().enumerate() {
            self.audio_engine.mixer.configs[slot].set_gain(cfg.gain);
            self.audio_engine.mixer.configs[slot]
                .muted
                .store(cfg.muted, Ordering::Relaxed);
            self.audio_engine.mixer.configs[slot]
                .duck_on_miss
                .store(cfg.duck_on_player_miss, Ordering::Relaxed);
        }
        self.audio_engine
            .mixer
            .start_countdown(self.project.chart.bpm);
        self.audio_engine.mixer.play();

        let note_len = self.project.chart.notes.len();
        self.gameplay = Some(GameplayRuntimeState {
            bundle,
            compiled_texture_paths: self.compiled_texture_paths.clone(),
            runtime,
            script_errors,
            note_window: NoteSlidingWindow::new(note_len),
            frame_arena: FrameBumpArena::new(16 * 1024),
            visible_note_scratch: Vec::with_capacity(128),
            step_entities_scratch: Vec::with_capacity(16),
            step_notes_scratch: Vec::with_capacity(128),
            strum_overrides: default_strums,
            dynamic_items: Vec::new(),
            active_tweens: Vec::new(),
            active_timers: Vec::new(),
            score: 0,
            combo: 0,
            max_combo: 0,
            hits: 0,
            misses: 0,
            accuracy_sum: 0.0,
            judged_count: 0,
            accuracy_pct: 100.0,
            health: 0.5,
            last_judgement: "¡Prepárate!".to_string(),
            last_rating_code: 0,
            rating_event_id: 0,
            splash_event_id: 0,
            splash_lane: 0,
            hud_visible: true,
            hud_opacity: 1.0,
            hud_zoom: 1.0,
            arrows_visible: true,
            arrows_opacity: 1.0,
            screen_blackout_alpha: 0.0,
            active_player_id,
            active_opponent_id,
            flash_color_argb: 0xFFFFFFFF,
            flash_alpha: 0.0,
            flash_decay_per_sec: 1.0,
            flash_over_hud: false,
            preload_requested_entities: HashSet::new(),
            retired_entities: HashSet::new(),
            reused_later_entities,
            active_scroll_speed: self.project.chart.speed.max(0.5),
            active_downscroll: self.project.downscroll,
            song_started: false,
            last_beat: -10,
            last_step: -40,
            next_event_idx: 0,
            next_preload_event_idx: 0,
            paused: false,
            finished: false,
            last_logs: Vec::new(),
            last_countdown_stage: -2,
            hud_revision: 1,
            last_binary_hud_revision: 0,
        });

        self.step_gameplay(0.0, 0, 0)
    }

    /// Alterna el estado de pausa del gameplay y del reloj maestro AAudio.
    pub fn set_gameplay_paused(&mut self, paused: bool) -> String {
        if let Some(session) = self.gameplay.as_mut() {
            session.paused = paused;
            session.hud_revision = session.hud_revision.wrapping_add(1);
            if paused {
                self.audio_engine.mixer.pause();
            } else {
                self.audio_engine.ensure_started();
                self.audio_engine.mixer.play();
            }
        }
        self.step_gameplay(0.0, 0, 0)
    }

    /// Finaliza el gameplay, cierra el streaming de disco y recarga las pistas PCM del editor.
    pub fn stop_gameplay(&mut self) -> String {
        self.audio_engine.mixer.pause();
        self.audio_engine.mixer.clear_stream_source();
        self.gameplay = None;
        if self.project.active_audio_count() > 0
            && self.decoded_tracks.iter().all(|t| t.is_none())
        {
            self.reload_project_audio_tracks();
        }
        self.ok_response("Partida finalizada")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_gameplay_packet_encodes_header_entities_notes_and_hud_strings() {
        let tmp = std::env::temp_dir().join("beatforge_bin_packet_test");
        let _ = std::fs::remove_dir_all(&tmp);
        let mut session = EngineSession::new_with_dirs(tmp.clone(), tmp.clone());
        let _ = session.start_gameplay();

        let mut tiny = [0u8; 32];
        let needed = session.step_gameplay_binary(0.016, 0, 0, &mut tiny);
        assert!(needed < -(BINARY_HEADER_SIZE as i32));

        let mut buf = vec![0u8; 32 * 1024];
        let written = session.step_gameplay_binary(0.016, 0, 0, &mut buf);
        assert!(written >= BINARY_HEADER_SIZE as i32);

        let magic = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        assert_eq!(magic, BINARY_FRAME_MAGIC);
        let flags = u32::from_le_bytes(buf[4..8].try_into().unwrap());
        assert_ne!(flags & 1, 0);
        assert_ne!(flags & (1 << 5), 0);

        let written_second = session.step_gameplay_binary(0.016, 0, 0, &mut buf);
        assert!(written_second > 0 && written_second <= written);
        let flags_second = u32::from_le_bytes(buf[4..8].try_into().unwrap());
        assert_eq!(flags_second & (1 << 5), 0);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
