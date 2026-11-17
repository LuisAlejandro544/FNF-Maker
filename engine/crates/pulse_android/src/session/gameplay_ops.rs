//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops.rs
//! Propósito: Compilación en caliente inteligente (`rayon`) y simulación de gameplay en tiempo real
//! sincronizada con el reloj maestro de `pulse_audio`, usando:
//! - Arena temporal por frame (`FrameBumpArena`) con reinicio O(1) en cada tick.
//! - Descarte progresivo por ventana deslizante (`NoteSlidingWindow`) para no recorrer todo el chart.
//! - Caché LRU (`LruCache`) con presupuesto de memoria adaptado al teléfono.
//! - Scripts optimizados por el analizador de bucles en `hot_compile`.

use super::{
    EngineSession, GameplayEntityFrameJson, GameplayRuntimeState, GameplayStepJson,
    SparrowFrameJson, VisibleNoteJson, control_mode_to_key, dir_to_anim, parse_anim_kind,
    role_to_key, trigger_role_anim,
};
use image::ImageFormat;
use pulse_core::EntityRole;
use pulse_core::frame_memory::{FrameBumpArena, NotePlayState, NoteSlidingWindow};
use pulse_core::hot_compile::{DeviceResourceTier, hot_compile_project};
use pulse_core::sparrow::CharacterAnimKind;
use pulse_scripting::{ModScriptRuntime, ScriptCommand};
use std::collections::HashMap;
use std::sync::atomic::Ordering;

const HIT_WINDOW_GOOD_MS: f64 = 155.0;
const HIT_WINDOW_PERFECT_MS: f64 = 55.0;

impl EngineSession {
    pub fn hot_compile(&mut self, tier_code: i32) -> String {
        let tier = match tier_code {
            0 => DeviceResourceTier::LowRam32Bit,
            2 => DeviceResourceTier::HighQuality,
            _ => DeviceResourceTier::Balanced,
        };

        self.texture_lru_cache
            .set_budget(tier.lru_cache_budget_bytes(), 64);

        let bundle = hot_compile_project(&self.project, tier, |_pct, _msg| {});
        let out_dir = self.cache_dir.join("compiled_cache");
        let _ = std::fs::create_dir_all(&out_dir);

        let mut texture_paths = HashMap::new();

        // Entidades cuya resolución ya era óptima pasan directo sin recompresión ni escritura extra
        for (entity_id, orig_path) in &bundle.passthrough_paths {
            texture_paths.insert(
                entity_id.clone(),
                orig_path.to_string_lossy().into_owned(),
            );
        }

        // Entidades optimizadas o decodificadas de .astc se guardan en caché disco + caché LRU en RAM
        for (entity_id, tex) in &bundle.textures {
            let byte_size = tex.rgba.len();
            self.texture_lru_cache
                .insert(entity_id.clone(), tex.clone(), byte_size);

            if let Some(rgba_img) = image::RgbaImage::from_raw(tex.width, tex.height, tex.rgba.clone()) {
                let target_file = out_dir.join(format!("{}.png", entity_id));
                if image::DynamicImage::ImageRgba8(rgba_img)
                    .save_with_format(&target_file, ImageFormat::Png)
                    .is_ok()
                {
                    texture_paths.insert(
                        entity_id.clone(),
                        target_file.to_string_lossy().into_owned(),
                    );
                }
            }
        }

        self.compiled_texture_paths = texture_paths;
        self.last_compile_report = Some(bundle.report.clone());
        self.compiled_bundle = Some(bundle);
        self.ok_response("Compilación en caliente inteligente completada")
    }

    pub fn start_gameplay(&mut self) -> String {
        self.gameplay = None;
        let bundle = self
            .compiled_bundle
            .take()
            .unwrap_or_else(|| hot_compile_project(&self.project, DeviceResourceTier::Balanced, |_, _| {}));

        // Asegurar orden cronológico para el descarte progresivo por ventana deslizante
        self.project
            .chart
            .notes
            .sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));

        let (mut runtime, script_errors) =
            ModScriptRuntime::load_from_scripts(&bundle.optimized_scripts);
        runtime.call_on_create();

        self.audio_engine.ensure_started();
        let any_audio = self.project.active_audio_count() > 0;
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
            score: 0,
            combo: 0,
            max_combo: 0,
            hits: 0,
            misses: 0,
            health: 0.5,
            last_judgement: "¡Prepárate!".to_string(),
            last_beat: -10,
            paused: false,
            finished: false,
            last_logs: Vec::new(),
        });

        self.step_gameplay(0.0, 0, 0)
    }

    pub fn step_gameplay(
        &mut self,
        dt_sec: f32,
        lane_pressed_mask: i32,
        lane_just_pressed_mask: i32,
    ) -> String {
        let control_mode_str = control_mode_to_key(self.project.control_mode).to_string();
        let Some(session) = self.gameplay.as_mut() else {
            return serde_json::to_string(&GameplayStepJson {
                active: false,
                song_pos_ms: 0.0,
                duration_ms: 0.0,
                scroll_speed: 1.8,
                active_bpm: 120.0,
                downscroll: false,
                control_mode: control_mode_str,
                holding_lanes_mask: 0,
                opponent_holding_lanes_mask: 0,
                countdown_stage: -1,
                countdown_label: String::new(),
                countdown_progress: 0.0,
                camera_zoom: 1.0,
                camera_shake: 0.0,
                camera_x: 640.0,
                camera_y: 360.0,
                virtual_width: 1280.0,
                virtual_height: 720.0,
                score: 0,
                combo: 0,
                max_combo: 0,
                hits: 0,
                misses: 0,
                health: 0.5,
                last_judgement: String::new(),
                paused: false,
                finished: false,
                script_errors: Vec::new(),
                script_logs: Vec::new(),
                entities: Vec::new(),
                visible_notes: Vec::new(),
            })
            .unwrap_or_default();
        };

        // Reinicio O(1) de la arena temporal al empezar el frame
        session.frame_arena.reset();

        let dt = dt_sec.clamp(0.0, 0.05);
        let raw_mixer_ms = self.audio_engine.mixer.position_ms();
        let in_countdown = raw_mixer_ms < 0.0;
        let song_pos_ms = if in_countdown {
            raw_mixer_ms
        } else {
            raw_mixer_ms - self.project.audio_offset_ms
        };
        let active_bpm = self.project.chart.bpm_at_ms(song_pos_ms.max(0.0)).max(40.0);
        let beat_ms = 60_000.0 / active_bpm;
        let current_beat = (song_pos_ms / beat_ms).floor() as i32;

        let (countdown_stage, countdown_label, countdown_progress) = if in_countdown {
            let cd_beat_ms = 60_000.0 / self.project.chart.bpm.max(40.0);
            let total_cd_ms = cd_beat_ms * 4.0;
            let elapsed_cd = (total_cd_ms + raw_mixer_ms).clamp(0.0, total_cd_ms - 0.001);
            let stage = ((elapsed_cd / cd_beat_ms).floor() as i32).clamp(0, 3);
            let progress = ((elapsed_cd % cd_beat_ms) / cd_beat_ms) as f32;
            let label = match stage {
                0 => "3",
                1 => "2",
                2 => "1",
                _ => "¡YA!",
            };
            (stage, label.to_string(), progress.clamp(0.0, 1.0))
        } else {
            (-1, String::new(), 0.0)
        };

        let mut just_pressed = [
            (lane_just_pressed_mask & 1) != 0,
            (lane_just_pressed_mask & 2) != 0,
            (lane_just_pressed_mask & 4) != 0,
            (lane_just_pressed_mask & 8) != 0,
        ];
        let mut player_holding_mask: u8 = 0;
        let mut opp_holding_mask: u8 = 0;

        if !session.paused && !session.finished {
            *session.runtime.state.song_pos_ms.lock() = song_pos_ms;
            if current_beat > session.last_beat {
                session.last_beat = current_beat;
                session.runtime.call_on_beat_hit(current_beat);
            }
            session.runtime.call_on_update(dt);

            for cmd in session.runtime.drain_commands() {
                match cmd {
                    ScriptCommand::SetCameraZoom(z) => {
                        self.project.stage.camera_zoom = z.clamp(0.5, 2.5);
                    }
                    ScriptCommand::ShakeCamera { intensity } => {
                        self.project.stage.camera_shake = intensity.clamp(0.0, 25.0);
                    }
                    ScriptCommand::SetEntityProp {
                        entity_id,
                        prop,
                        value,
                    } => {
                        if let Some(ent) = session
                            .bundle
                            .active_entities
                            .iter_mut()
                            .find(|e| e.id == entity_id)
                        {
                            match prop.as_str() {
                                "opacity" => ent.set_opacity(value),
                                "scale" => ent.set_scale(value),
                                "x" => ent.x = value,
                                "y" => ent.y = value,
                                _ => {}
                            }
                        }
                    }
                    ScriptCommand::PlayEntityAnim {
                        entity_id,
                        anim_name,
                    } => {
                        if let Some(ent) = session
                            .bundle
                            .active_entities
                            .iter_mut()
                            .find(|e| e.id == entity_id)
                        {
                            if let Some(kind) = parse_anim_kind(&anim_name.to_ascii_lowercase()) {
                                ent.active_anim = kind;
                                ent.anim_timer_sec = 0.0;
                            }
                        }
                    }
                    ScriptCommand::Log(msg) => {
                        session.last_logs.push(msg);
                        if session.last_logs.len() > 6 {
                            session.last_logs.remove(0);
                        }
                    }
                }
            }

            self.project.stage.camera_zoom += (1.0 - self.project.stage.camera_zoom) * (dt * 6.0);
            self.project.stage.camera_shake *= (1.0 - dt * 10.0).max(0.0);

            let mut any_recent_miss = false;
            let note_len = self.project.chart.notes.len();
            let start_idx = session.note_window.active_start_idx.min(note_len);

            // Evaluación por ventana deslizante: soporta notas cortas y notas largas (Holding) hasta que terminen
            for idx in start_idx..note_len {
                let state = session.note_window.state(idx);
                if matches!(state, NotePlayState::Hit | NotePlayState::Missed) {
                    continue;
                }
                let note = &self.project.chart.notes[idx];
                if note.time_ms > song_pos_ms + HIT_WINDOW_GOOD_MS + 10.0 {
                    break;
                }

                match state {
                    NotePlayState::Holding => {
                        let end_ms = note.time_ms + note.sustain_ms;
                        let lane_idx = note.direction as usize;
                        if !note.is_player {
                            if song_pos_ms >= end_ms {
                                session.note_window.mark_hit(idx);
                            } else {
                                opp_holding_mask |= 1u8 << (lane_idx as u8);
                                trigger_role_anim(
                                    &mut session.bundle.active_entities,
                                    EntityRole::Opponent,
                                    dir_to_anim(note.direction),
                                );
                            }
                        } else {
                            let is_held = (lane_pressed_mask & (1 << lane_idx)) != 0;
                            if song_pos_ms >= end_ms {
                                session.note_window.mark_hit(idx);
                                session.score += 100;
                                session.health = (session.health + 0.02).min(1.0);
                            } else if is_held {
                                player_holding_mask |= 1u8 << (lane_idx as u8);
                                session.score += ((dt * 220.0).round() as i64).max(1);
                                session.health = (session.health + 0.05 * dt).min(1.0);
                                trigger_role_anim(
                                    &mut session.bundle.active_entities,
                                    EntityRole::Player,
                                    dir_to_anim(note.direction),
                                );
                            } else if song_pos_ms > note.time_ms + 130.0 {
                                if song_pos_ms >= end_ms - 140.0 {
                                    session.note_window.mark_hit(idx);
                                } else {
                                    session.note_window.mark_missed(idx);
                                    session.combo = 0;
                                    session.misses += 1;
                                    session.score -= 25;
                                    session.health = (session.health - 0.03).max(0.0);
                                    session.last_judgement = "Soltada".to_string();
                                    any_recent_miss = true;
                                    session.runtime.call_on_note_miss(lane_idx as i32, true);
                                }
                            }
                        }
                    }
                    NotePlayState::Pending => {
                        if !note.is_player {
                            if song_pos_ms >= note.time_ms {
                                let lane_idx = note.direction as u8;
                                if note.sustain_ms > 45.0 {
                                    session.note_window.mark_holding(idx);
                                    opp_holding_mask |= 1u8 << lane_idx;
                                } else {
                                    session.note_window.mark_hit(idx);
                                }
                                let dir_anim = dir_to_anim(note.direction);
                                trigger_role_anim(
                                    &mut session.bundle.active_entities,
                                    EntityRole::Opponent,
                                    dir_anim,
                                );
                                session.runtime.call_on_note_hit(note.direction as i32, false);
                            }
                        } else {
                            let diff = note.time_ms - song_pos_ms;
                            let lane_idx = note.direction as usize;
                            if just_pressed[lane_idx] && diff.abs() <= HIT_WINDOW_GOOD_MS {
                                if note.sustain_ms > 45.0 {
                                    session.note_window.mark_holding(idx);
                                    player_holding_mask |= 1u8 << (lane_idx as u8);
                                } else {
                                    session.note_window.mark_hit(idx);
                                }
                                just_pressed[lane_idx] = false;

                                let pts = if diff.abs() <= HIT_WINDOW_PERFECT_MS {
                                    session.last_judgement = "¡Perfecto!".to_string();
                                    350
                                } else {
                                    session.last_judgement = "¡Bien!".to_string();
                                    200
                                };
                                session.score += pts;
                                session.combo += 1;
                                session.max_combo = session.max_combo.max(session.combo);
                                session.hits += 1;
                                session.health = (session.health + 0.035).min(1.0);

                                trigger_role_anim(
                                    &mut session.bundle.active_entities,
                                    EntityRole::Player,
                                    dir_to_anim(note.direction),
                                );
                                session.runtime.call_on_note_hit(lane_idx as i32, true);
                            } else if song_pos_ms > note.time_ms + HIT_WINDOW_GOOD_MS {
                                session.note_window.mark_missed(idx);
                                session.combo = 0;
                                session.misses += 1;
                                session.score -= 50;
                                session.health = (session.health - 0.06).max(0.0);
                                session.last_judgement = "Fallo".to_string();
                                any_recent_miss = true;
                                session.runtime.call_on_note_miss(lane_idx as i32, true);
                            }
                        }
                    }
                    NotePlayState::Hit | NotePlayState::Missed => {}
                }
            }

            // Descarta progresivamente las notas ya resueltas que salieron de la ventana
            session
                .note_window
                .advance_and_discard(&self.project.chart.notes, song_pos_ms, 240.0);

            self.audio_engine
                .mixer
                .player_missed_recently
                .store(any_recent_miss, Ordering::Relaxed);

            for ent in &mut session.bundle.active_entities {
                ent.anim_timer_sec += dt;
                if ent.active_anim != CharacterAnimKind::Idle && ent.anim_timer_sec > 0.48 {
                    ent.active_anim = CharacterAnimKind::Idle;
                    ent.anim_timer_sec = 0.0;
                }
            }

            if song_pos_ms > self.project.chart.stats.duration_ms + 1200.0 {
                session.finished = true;
                self.audio_engine.mixer.pause();
            }
        }

        // Recopilar notas visibles en la arena temporal del frame actual
        let visible_span = session.note_window.collect_visible_into_arena(
            &self.project.chart.notes,
            song_pos_ms,
            240.0,
            1800.0,
            &mut session.frame_arena,
            &mut session.visible_note_scratch,
        );

        let mut entities = std::mem::take(&mut session.step_entities_scratch);
        entities.clear();
        for ent in &session.bundle.active_entities {
            // Tocar el caché LRU si la entidad reside en él para mantener frescura de uso
            let _ = self.texture_lru_cache.get(&ent.id);

            let tex_path = session
                .compiled_texture_paths
                .get(&ent.id)
                .cloned()
                .or_else(|| ent.image_path.as_ref().map(|p| p.to_string_lossy().into_owned()));
            let frame = ent
                .atlas
                .as_ref()
                .and_then(|a| a.frame_at(ent.active_anim, ent.anim_timer_sec))
                .map(|f| SparrowFrameJson {
                    found: true,
                    raw_name: f.raw_name.clone(),
                    x: f.x,
                    y: f.y,
                    width: f.width,
                    height: f.height,
                    frame_x: f.frame_x,
                    frame_y: f.frame_y,
                    frame_width: f.frame_width,
                    frame_height: f.frame_height,
                    rotated: f.rotated,
                });
            entities.push(GameplayEntityFrameJson {
                id: ent.id.clone(),
                display_name: ent.display_name.clone(),
                role: role_to_key(ent.role).to_string(),
                x: ent.x,
                y: ent.y,
                width: ent.width,
                height: ent.height,
                scale: ent.scale,
                opacity: ent.opacity,
                flip_x: ent.flip_x,
                visible: ent.visible,
                texture_path: tex_path,
                frame,
            });
        }

        let mut visible_notes = std::mem::take(&mut session.step_notes_scratch);
        visible_notes.clear();
        for n in session.frame_arena.get_slice(visible_span) {
            visible_notes.push(VisibleNoteJson {
                id: n.id,
                time_ms: n.time_ms,
                direction: n.direction,
                is_player: n.is_player,
                sustain_ms: n.sustain_ms,
                head_hit: n.is_holding,
            });
        }

        let step_dto = GameplayStepJson {
            active: true,
            song_pos_ms,
            duration_ms: self.project.chart.stats.duration_ms,
            scroll_speed: self.project.chart.speed,
            active_bpm,
            downscroll: self.project.downscroll,
            control_mode: control_mode_str,
            holding_lanes_mask: player_holding_mask,
            opponent_holding_lanes_mask: opp_holding_mask,
            countdown_stage,
            countdown_label,
            countdown_progress,
            camera_zoom: self.project.stage.camera_zoom,
            camera_shake: self.project.stage.camera_shake,
            camera_x: self.project.stage.camera_x,
            camera_y: self.project.stage.camera_y,
            virtual_width: self.project.stage.virtual_width,
            virtual_height: self.project.stage.virtual_height,
            score: session.score,
            combo: session.combo,
            max_combo: session.max_combo,
            hits: session.hits,
            misses: session.misses,
            health: session.health,
            last_judgement: session.last_judgement.clone(),
            paused: session.paused,
            finished: session.finished,
            script_errors: session.script_errors.clone(),
            script_logs: session.last_logs.clone(),
            entities,
            visible_notes,
        };

        let out_json = serde_json::to_string(&step_dto).unwrap_or_else(|_| "{}".to_string());

        // Devolver los buffers scratch a `session` para evitar nuevas asignaciones en el próximo frame
        session.step_entities_scratch = step_dto.entities;
        session.step_notes_scratch = step_dto.visible_notes;

        out_json
    }

    pub fn set_gameplay_paused(&mut self, paused: bool) -> String {
        if let Some(session) = self.gameplay.as_mut() {
            session.paused = paused;
            if paused {
                self.audio_engine.mixer.pause();
            } else {
                self.audio_engine.ensure_started();
                self.audio_engine.mixer.play();
            }
        }
        self.step_gameplay(0.0, 0, 0)
    }

    pub fn stop_gameplay(&mut self) -> String {
        self.audio_engine.mixer.pause();
        self.gameplay = None;
        self.ok_response("Partida finalizada")
    }
}
