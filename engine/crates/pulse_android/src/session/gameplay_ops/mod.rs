//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/mod.rs
//! Propósito: Coordinador modular del ciclo de compilación en caliente y gameplay en tiempo real.
//! Divide la lógica en submódulos especializados:
//! - `hot_compile_ops`: Compilación incremental en caché (`manifest.json`), WebP lossless y WAV mix.
//! - `audio_adapt_ops`: Sondeo hardware + EBU R128 + FFT + Biquad/Rubato hacia WAV adaptado.
//! - `script_sync`: Sincronización Lua/QuickJS, tweens, timers y comandos `ScriptCommand`.
//! - `simulation`: Simulación por frame (`step_gameplay_internal`) y evaluación de notas/sustain.
//! - `packet_encoder`: Serialización JSON y binaria `BFGP` v2 sin asignaciones por frame.

mod audio_adapt_ops;
mod hot_compile_ops;
mod packet_encoder;
mod script_sync;
mod simulation;

use super::persistence_ops::sanitize_project_id;
use super::{EngineSession, GameplayRuntimeState, StrumOverrideState};
use crate::session::trigger_character_anim;
use pulse_audio::StreamingAudioSource;
use pulse_core::disk_cache::{evict_lru_subdirs, touch_dir_last_used};
use pulse_core::frame_memory::{FrameBumpArena, NoteSlidingWindow};
use pulse_core::hot_compile::{
    DeviceResourceTier, analyze_reused_entity_ids, hot_compile_project_to_dir,
};
use pulse_core::project::MAX_AUDIO_TRACKS;
use pulse_core::sparrow::CharacterAnimKind;
use pulse_core::EntityRole;
use pulse_scripting::ModScriptRuntime;
use script_sync::sync_script_host_state;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;

pub const BINARY_FRAME_MAGIC: u32 = 0x4246_4750; // "BFGP"
pub const BINARY_HEADER_SIZE: usize = 168;
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
        self.project.stage.camera.ensure_default_cameras();
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

        let mut reused_later_entities = analyze_reused_entity_ids(&self.project);
        for w in &bundle.timeline_windows {
            if w.reused_later {
                reused_later_entities.insert(w.entity_id.clone());
            }
        }

        let mut timeline_preload_queue: Vec<(f64, String)> = bundle
            .timeline_windows
            .iter()
            .filter_map(|w| w.preload_at_ms.map(|t| (t, w.entity_id.clone())))
            .collect();
        timeline_preload_queue.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut timeline_retire_queue: Vec<(f64, String)> = bundle
            .timeline_windows
            .iter()
            .filter_map(|w| w.retire_after_ms.map(|t| (t, w.entity_id.clone())))
            .collect();
        timeline_retire_queue.sort_by(|a, b| a.0.total_cmp(&b.0));

        if self.compiled_texture_paths.is_empty() && !bundle.compiled_texture_paths.is_empty() {
            for (entity_id, path) in &bundle.compiled_texture_paths {
                self.compiled_texture_paths
                    .insert(entity_id.clone(), path.to_string_lossy().into_owned());
            }
        }
        let runtime_texture_pages: HashMap<String, Vec<String>> =
            if !self.compiled_texture_pages.is_empty() {
                self.compiled_texture_pages.clone()
            } else {
                bundle
                    .compiled_texture_pages
                    .iter()
                    .map(|(id, pages)| {
                        (
                            id.clone(),
                            pages
                                .iter()
                                .map(|p| p.to_string_lossy().into_owned())
                                .collect(),
                        )
                    })
                    .collect()
            };
        if self.compiled_texture_pages.is_empty() && !runtime_texture_pages.is_empty() {
            self.compiled_texture_pages = runtime_texture_pages.clone();
        }
        if self.texture_pixel_formats.is_empty() && !bundle.texture_pixel_formats.is_empty() {
            for (id, fmt) in &bundle.texture_pixel_formats {
                self.texture_pixel_formats
                    .insert(id.clone(), fmt.as_key().to_string());
            }
        }
        if self.compiled_countdown_paths.is_empty() && !bundle.compiled_countdown_paths.is_empty() {
            for (slot_idx, path) in &bundle.compiled_countdown_paths {
                self.compiled_countdown_paths
                    .insert(slot_idx.to_string(), path.to_string_lossy().into_owned());
            }
        }
        if self.compiled_icon_normal_paths.is_empty()
            && !bundle.compiled_icon_normal_paths.is_empty()
        {
            for (id, path) in &bundle.compiled_icon_normal_paths {
                self.compiled_icon_normal_paths
                    .insert(id.clone(), path.to_string_lossy().into_owned());
            }
        }
        if self.compiled_icon_losing_paths.is_empty()
            && !bundle.compiled_icon_losing_paths.is_empty()
        {
            for (id, path) in &bundle.compiled_icon_losing_paths {
                self.compiled_icon_losing_paths
                    .insert(id.clone(), path.to_string_lossy().into_owned());
            }
        }
        if self.last_timeline_windows.is_empty() && !bundle.timeline_windows.is_empty() {
            self.last_timeline_windows = bundle.timeline_windows.clone();
        }

        let (cam0_x, cam0_y, cam0_zoom, _) = self.project.stage.resolve_camera_slot_target(
            0,
            &active_player_id,
            &active_opponent_id,
        );
        let (clamped_x, clamped_y) =
            self.project
                .stage
                .clamp_camera_to_background(cam0_x, cam0_y, cam0_zoom);
        self.project.stage.camera_x = clamped_x;
        self.project.stage.camera_y = clamped_y;
        self.project.stage.camera_base_zoom = cam0_zoom;
        self.project.stage.camera_zoom = cam0_zoom;

        let (mut runtime, script_errors) =
            ModScriptRuntime::load_from_scripts(&bundle.optimized_scripts);
        let (_, initial_must_hit, initial_gf_section, _) =
            self.project.chart.camera_state_at_ms(0.0);
        let default_strums = [StrumOverrideState::default(); 8];

        self.audio_engine.mixer.pause();
        self.audio_engine.mixer.clear_gameover_sfx();
        self.audio_engine.mixer.clear_stream_source();
        self.audio_engine.ensure_started();

        let compiled_audio = self
            .adapted_audio_path
            .clone()
            .or_else(|| self.compiled_audio_path.clone())
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

        sync_script_host_state(
            &runtime.state,
            &self.project.stage,
            &bundle.active_entities,
            &[],
            &active_player_id,
            &active_opponent_id,
            &default_strums,
            self.audio_engine.mixer.position_ms(),
            self.project.chart.bpm.max(40.0),
            initial_must_hit,
            initial_gf_section,
            0,
            0.5,
            0,
            0,
            0,
            0,
            100.0,
            self.project.chart.speed,
            self.project.downscroll,
            true,
            1.0,
            1.0,
            true,
            1.0,
            0.0,
            false,
        );
        runtime.call_on_create();

        let note_len = self.project.chart.notes.len();
        let mut gp = GameplayRuntimeState {
            bundle,
            compiled_texture_paths: self.compiled_texture_paths.clone(),
            compiled_texture_pages: runtime_texture_pages,
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
            active_camera_slot: 0,
            camera_slot_zoom_override: None,
            camera_follow_lock: None,
            camera_gf_override: false,
            camera_offset_override: [0.0, 0.0],
            last_section_idx: -1,
            last_cam_beat_bucket: -1,
            flash_color_argb: 0xFFFFFFFF,
            flash_alpha: 0.0,
            flash_decay_per_sec: 1.0,
            flash_over_hud: false,
            preload_requested_entities: HashSet::new(),
            retired_entities: HashSet::new(),
            reused_later_entities,
            active_scroll_speed: self.project.chart.speed.max(0.5),
            active_downscroll: self.project.downscroll
                || self.project.control_mode == MobileControlMode::TapArrows,
            song_started: false,
            last_beat: -10,
            last_step: -40,
            next_event_idx: 0,
            next_preload_event_idx: 0,
            timeline_preload_queue,
            next_timeline_preload_idx: 0,
            timeline_retire_queue,
            next_timeline_retire_idx: 0,
            paused: false,
            finished: false,
            game_over: false,
            game_over_phase: 0,
            game_over_timer_sec: 0.0,
            botplay: false,
            practice_mode: false,
            last_logs: Vec::new(),
            last_countdown_stage: -2,
            hud_revision: 1,
            last_binary_hud_revision: 0,
        };

        // Drenar comandos emitidos por `onCreate()` (pantalla en negro, ocultar HUD/flechas, tweens)
        // antes de emitir el primer paquete binario/JSON del Frame 0.
        script_sync::drain_and_apply_script_commands(
            &mut gp,
            &mut self.project.stage,
            &self.audio_engine,
        );

        self.gameplay = Some(gp);
        self.step_gameplay(0.0, 0, 0)
    }

    /// Alterna el estado de pausa del gameplay y del reloj maestro AAudio.
    pub fn set_gameplay_paused(&mut self, paused: bool) -> String {
        if let Some(session) = self.gameplay.as_mut() {
            if session.game_over {
                return self.step_gameplay(0.0, 0, 0);
            }
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

    /// Configura modos de depuración en gameplay (`botplay` y `practice_mode`).
    pub fn set_gameplay_debug(&mut self, botplay: bool, practice_mode: bool) -> String {
        if let Some(session) = self.gameplay.as_mut() {
            session.botplay = botplay;
            session.practice_mode = practice_mode;
            if practice_mode && session.health < 0.02 {
                session.health = 0.02;
            }
            session.hud_revision = session.hud_revision.wrapping_add(1);
        }
        self.step_gameplay(0.0, 0, 0)
    }

    /// Dispara manualmente el estado de Game Over (útil desde scripts o menú Debug).
    pub fn trigger_game_over(&mut self) -> String {
        if let Some(session) = self.gameplay.as_mut() {
            enter_game_over(session, &self.audio_engine);
        }
        self.step_gameplay(0.0, 0, 0)
    }

    /// Inicia la fase de confirmación (`DeathConfirm`) tras tocar la pantalla en Game Over.
    pub fn confirm_game_over_retry(&mut self) -> String {
        if let Some(session) = self.gameplay.as_mut() {
            if session.game_over && session.game_over_phase != 3 {
                session.game_over_phase = 3;
                session.game_over_timer_sec = 0.0;
                session.last_judgement = "¡Reintentando!".to_string();
                session.hud_revision = session.hud_revision.wrapping_add(1);
                trigger_character_anim(
                    &mut session.bundle.active_entities,
                    EntityRole::Player,
                    &session.active_player_id,
                    CharacterAnimKind::DeathConfirm,
                );
            }
        }
        self.step_gameplay(0.0, 0, 0)
    }

    /// Reinicia la partida desde cero conservando las banderas activas de Debug (`botplay`/`practice_mode`).
    pub fn restart_gameplay(&mut self) -> String {
        let (prev_botplay, prev_practice) = self
            .gameplay
            .as_ref()
            .map(|g| (g.botplay, g.practice_mode))
            .unwrap_or((false, false));
        let _ = self.start_gameplay();
        if let Some(session) = self.gameplay.as_mut() {
            session.botplay = prev_botplay;
            session.practice_mode = prev_practice;
            session.hud_revision = session.hud_revision.wrapping_add(1);
        }
        self.step_gameplay(0.0, 0, 0)
    }

    /// Finaliza el gameplay, cierra el streaming de disco y recarga las pistas PCM del editor.
    pub fn stop_gameplay(&mut self) -> String {
        self.audio_engine.mixer.pause();
        self.audio_engine.mixer.clear_gameover_sfx();
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

/// Activa el estado de Game Over pausando la música, disparando el sonido sintetizado y animando `DeathStart`.
pub(super) fn enter_game_over(
    session: &mut GameplayRuntimeState,
    audio_engine: &pulse_audio::AudioEngineHandle,
) {
    if session.game_over {
        return;
    }
    session.game_over = true;
    session.paused = false;
    session.game_over_phase = 1;
    session.game_over_timer_sec = 0.0;
    session.last_judgement = "Game Over".to_string();
    session.hud_revision = session.hud_revision.wrapping_add(1);
    audio_engine.mixer.pause();
    audio_engine.mixer.trigger_gameover_sfx();
    trigger_character_anim(
        &mut session.bundle.active_entities,
        EntityRole::Player,
        &session.active_player_id,
        CharacterAnimKind::DeathStart,
    );
    *session.runtime.state.game_over.lock() = true;
    session.runtime.call_on_game_over();
}

/// Determina si una entidad debe reiniciar/sincronizar su animación `Idle` al ritmo del BPM.
/// Incluye jugador, oponente y decoraciones de personaje tipo GF (`stage_speakers`,
/// descriptores con `loop: false` o nombres `gf`/`cabina`).
pub(super) fn is_beat_synced_idle_entity(ent: &StageEntity) -> bool {
    if matches!(ent.role, EntityRole::Player | EntityRole::Opponent) {
        return true;
    }
    if ent.role != EntityRole::Decoration {
        return false;
    }
    let id_low = ent.id.to_ascii_lowercase();
    let name_low = ent.display_name.to_ascii_lowercase();
    id_low == "stage_speakers"
        || id_low.contains("gf")
        || name_low.contains("gf")
        || name_low.contains("cabina")
        || name_low.contains("bocina")
        || ent.character_config_path.is_some()
        || ent.pending_character_desc.is_some()
        || ent.atlas.as_ref().is_some_and(|a| !a.is_idle_looped())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulse_core::sparrow::SparrowAtlas;

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

        let _ = session.update_stage_camera_vignette(true, "#3B82F6", 0.68, 0.28, 0.40, true, false);
        let written_vig = session.step_gameplay_binary(0.016, 0, 0, &mut buf);
        assert!(written_vig >= BINARY_HEADER_SIZE as i32);
        let flags_vig = u32::from_le_bytes(buf[4..8].try_into().unwrap());
        assert_ne!(flags_vig & (1 << 14), 0);
        let vig_color = u32::from_le_bytes(buf[152..156].try_into().unwrap());
        let vig_alpha = f32::from_le_bytes(buf[156..160].try_into().unwrap());
        assert_eq!(vig_color, 0xFF3B82F6);
        assert!((vig_alpha - 0.68).abs() < 0.01);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn gf_decoration_syncs_idle_to_bpm_and_tap_arrows_forces_active_downscroll() {
        let tmp = std::env::temp_dir().join("beatforge_gf_downscroll_test");
        let _ = std::fs::remove_dir_all(&tmp);
        let mut session = EngineSession::new_with_dirs(tmp.clone(), tmp.clone());
        session.project.downscroll = false;
        session.project.control_mode = MobileControlMode::TapArrows;

        let gf_xml = r#"
        <TextureAtlas imagePath="GF_assets.png">
            <SubTexture name="GF Dancing Beat0000" x="0" y="0" width="120" height="140" />
            <SubTexture name="GF Dancing Beat0001" x="120" y="0" width="120" height="140" />
        </TextureAtlas>
        "#;
        let gf_atlas = SparrowAtlas::from_xml_str(gf_xml).unwrap();
        if let Some(gf_ent) = session
            .project
            .stage
            .entities
            .iter_mut()
            .find(|e| e.id == "stage_speakers")
        {
            gf_ent.atlas = Some(gf_atlas);
            assert!(is_beat_synced_idle_entity(gf_ent));
        }

        let _ = session.start_gameplay();
        let gp = session.gameplay.as_ref().unwrap();
        assert!(gp.active_downscroll);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
