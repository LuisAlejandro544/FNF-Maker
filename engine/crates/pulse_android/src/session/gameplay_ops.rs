//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops.rs
//! Propósito: Compilación en caliente (`rayon`) y simulación de gameplay en tiempo real
//! sincronizada con el reloj maestro de `pulse_audio` (cuenta regresiva BPM, juicio de notas,
//! ejecución de scripts Lua/TS/JS y actualización de cámara/entidades por frame).

use super::{
    EngineSession, GameplayEntityFrameJson, GameplayRuntimeState, GameplayStepJson,
    SparrowFrameJson, VisibleNoteJson, dir_to_anim, parse_anim_kind, role_to_key, trigger_role_anim,
};
use image::ImageFormat;
use pulse_core::EntityRole;
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

        let bundle = hot_compile_project(&self.project, tier, |_pct, _msg| {});
        let out_dir = self.cache_dir.join("compiled_cache");
        let _ = std::fs::create_dir_all(&out_dir);

        let mut texture_paths = HashMap::new();
        for (entity_id, tex) in &bundle.textures {
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
        self.ok_response("Compilación en caliente completada")
    }

    pub fn start_gameplay(&mut self) -> String {
        self.gameplay = None;
        let bundle = self
            .compiled_bundle
            .take()
            .unwrap_or_else(|| hot_compile_project(&self.project, DeviceResourceTier::Balanced, |_, _| {}));

        let (mut runtime, script_errors) = ModScriptRuntime::load_from_scripts(&self.project.scripts);
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
            notes_hit_state: vec![false; note_len],
            notes_miss_state: vec![false; note_len],
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
        _lane_pressed_mask: i32,
        lane_just_pressed_mask: i32,
    ) -> String {
        let Some(session) = self.gameplay.as_mut() else {
            return serde_json::to_string(&GameplayStepJson {
                active: false,
                song_pos_ms: 0.0,
                duration_ms: 0.0,
                scroll_speed: 1.8,
                active_bpm: 120.0,
                downscroll: false,
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
            for (idx, note) in self.project.chart.notes.iter().enumerate() {
                if session.notes_hit_state[idx] || session.notes_miss_state[idx] {
                    continue;
                }
                if !note.is_player {
                    if song_pos_ms >= note.time_ms {
                        session.notes_hit_state[idx] = true;
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
                        session.notes_hit_state[idx] = true;
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
                        session.notes_miss_state[idx] = true;
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

        let entities = session
            .bundle
            .active_entities
            .iter()
            .map(|ent| {
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
                GameplayEntityFrameJson {
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
                }
            })
            .collect();

        let visible_notes = self
            .project
            .chart
            .notes
            .iter()
            .enumerate()
            .filter_map(|(idx, note)| {
                if session.notes_hit_state[idx] {
                    return None;
                }
                let delta = note.time_ms - song_pos_ms;
                if !(-240.0..=1800.0).contains(&delta) {
                    return None;
                }
                Some(VisibleNoteJson {
                    id: note.id,
                    time_ms: note.time_ms,
                    direction: note.direction as u8,
                    is_player: note.is_player,
                    sustain_ms: note.sustain_ms,
                })
            })
            .collect();

        serde_json::to_string(&GameplayStepJson {
            active: true,
            song_pos_ms,
            duration_ms: self.project.chart.stats.duration_ms,
            scroll_speed: self.project.chart.speed,
            active_bpm,
            downscroll: self.project.downscroll,
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
        })
        .unwrap_or_else(|_| "{}".to_string())
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
