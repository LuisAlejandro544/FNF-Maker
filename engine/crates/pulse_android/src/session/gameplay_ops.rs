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
use pulse_core::frame_memory::{ArenaSpan, FrameBumpArena, FrameVisibleNote, NotePlayState, NoteSlidingWindow};
use pulse_core::hot_compile::{DeviceResourceTier, hot_compile_project};
use pulse_core::project::MobileControlMode;
use pulse_core::sparrow::CharacterAnimKind;
use pulse_scripting::{ModScriptRuntime, ScriptCommand};
use std::collections::HashMap;
use std::sync::atomic::Ordering;

const HIT_WINDOW_GOOD_MS: f64 = 155.0;
const HIT_WINDOW_PERFECT_MS: f64 = 55.0;

pub const BINARY_FRAME_MAGIC: u32 = 0x4246_4750; // "BFGP"
pub const BINARY_HEADER_SIZE: usize = 112;
pub const BINARY_ENTITY_SIZE: usize = 60;
pub const BINARY_NOTE_SIZE: usize = 28;

struct StepSimOutput {
    song_pos_ms: f64,
    active_bpm: f64,
    countdown_stage: i32,
    countdown_label: String,
    countdown_progress: f32,
    player_holding_mask: u8,
    opp_holding_mask: u8,
    visible_span: ArenaSpan<FrameVisibleNote>,
}

fn role_to_code(role: EntityRole) -> u32 {
    match role {
        EntityRole::Background => 0,
        EntityRole::Opponent => 1,
        EntityRole::Player => 2,
        EntityRole::Decoration => 3,
    }
}

fn truncate_utf8_for_u16(s: &str) -> &[u8] {
    let bytes = s.as_bytes();
    if bytes.len() <= u16::MAX as usize {
        return bytes;
    }
    let mut end = u16::MAX as usize;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &bytes[..end]
}

struct BinWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> BinWriter<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    #[inline]
    fn write_bytes(&mut self, bytes: &[u8]) {
        let end = self.pos + bytes.len();
        self.buf[self.pos..end].copy_from_slice(bytes);
        self.pos = end;
    }

    #[inline]
    fn write_u16(&mut self, v: u16) {
        self.write_bytes(&v.to_le_bytes());
    }

    #[inline]
    fn write_u32(&mut self, v: u32) {
        self.write_bytes(&v.to_le_bytes());
    }

    #[inline]
    fn write_i32(&mut self, v: i32) {
        self.write_bytes(&v.to_le_bytes());
    }

    #[inline]
    fn write_u64(&mut self, v: u64) {
        self.write_bytes(&v.to_le_bytes());
    }

    #[inline]
    fn write_i64(&mut self, v: i64) {
        self.write_bytes(&v.to_le_bytes());
    }

    #[inline]
    fn write_f32(&mut self, v: f32) {
        self.write_bytes(&v.to_le_bytes());
    }

    #[inline]
    fn write_f64(&mut self, v: f64) {
        self.write_bytes(&v.to_le_bytes());
    }

    #[inline]
    fn write_str_u16(&mut self, bytes: &[u8]) {
        self.write_u16(bytes.len() as u16);
        self.write_bytes(bytes);
    }
}

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
            last_countdown_stage: -2,
            hud_revision: 1,
            last_binary_hud_revision: 0,
        });

        self.step_gameplay(0.0, 0, 0)
    }

    fn step_gameplay_internal(
        &mut self,
        dt_sec: f32,
        lane_pressed_mask: i32,
        lane_just_pressed_mask: i32,
    ) -> Option<StepSimOutput> {
        let session = self.gameplay.as_mut()?;

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

        let mut hud_changed = false;
        if countdown_stage != session.last_countdown_stage {
            session.last_countdown_stage = countdown_stage;
            hud_changed = true;
        }
        let prev_judgement = session.last_judgement.clone();
        let prev_paused = session.paused;
        let prev_finished = session.finished;
        let prev_log_count = session.last_logs.len();
        let prev_last_log = session.last_logs.last().cloned();

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

        if prev_judgement != session.last_judgement
            || prev_paused != session.paused
            || prev_finished != session.finished
            || prev_log_count != session.last_logs.len()
            || prev_last_log.as_deref() != session.last_logs.last().map(|s| s.as_str())
        {
            hud_changed = true;
        }
        if hud_changed {
            session.hud_revision = session.hud_revision.wrapping_add(1);
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

        Some(StepSimOutput {
            song_pos_ms,
            active_bpm,
            countdown_stage,
            countdown_label,
            countdown_progress,
            player_holding_mask,
            opp_holding_mask,
            visible_span,
        })
    }

    pub fn step_gameplay(
        &mut self,
        dt_sec: f32,
        lane_pressed_mask: i32,
        lane_just_pressed_mask: i32,
    ) -> String {
        let control_mode_str = control_mode_to_key(self.project.control_mode).to_string();
        let Some(sim) = self.step_gameplay_internal(dt_sec, lane_pressed_mask, lane_just_pressed_mask) else {
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

        let session = self.gameplay.as_mut().expect("gameplay active");

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
        for n in session.frame_arena.get_slice(sim.visible_span) {
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
            song_pos_ms: sim.song_pos_ms,
            duration_ms: self.project.chart.stats.duration_ms,
            scroll_speed: self.project.chart.speed,
            active_bpm: sim.active_bpm,
            downscroll: self.project.downscroll,
            control_mode: control_mode_str,
            holding_lanes_mask: sim.player_holding_mask,
            opponent_holding_lanes_mask: sim.opp_holding_mask,
            countdown_stage: sim.countdown_stage,
            countdown_label: sim.countdown_label,
            countdown_progress: sim.countdown_progress,
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

    /// Serializa el frame de gameplay en formato binario Little-Endian sin asignaciones JSON
    /// directamente dentro de un `DirectByteBuffer` de Kotlin.
    /// Si `out_buf` es demasiado pequeño, devuelve `-(bytes_necesarios as i32)` para redimensionar.
    pub fn step_gameplay_binary(
        &mut self,
        dt_sec: f32,
        lane_pressed_mask: i32,
        lane_just_pressed_mask: i32,
        out_buf: &mut [u8],
    ) -> i32 {
        let Some(sim) = self.step_gameplay_internal(dt_sec, lane_pressed_mask, lane_just_pressed_mask) else {
            if out_buf.len() < BINARY_HEADER_SIZE {
                return -(BINARY_HEADER_SIZE as i32);
            }
            let mut w = BinWriter::new(out_buf);
            w.write_u32(BINARY_FRAME_MAGIC);
            w.write_u32(0); // flags: inactive
            w.write_u32(0); // hud_revision
            w.write_u32(0); // masks
            w.write_i32(-1); // countdown_stage
            w.write_f32(0.0); // countdown_progress
            w.write_f64(0.0); // song_pos_ms
            w.write_f64(0.0); // duration_ms
            w.write_f64(self.project.chart.speed);
            w.write_f64(self.project.chart.bpm);
            w.write_f32(1.0); // camera_zoom
            w.write_f32(0.0); // camera_shake
            w.write_f32(self.project.stage.camera_x);
            w.write_f32(self.project.stage.camera_y);
            w.write_f32(self.project.stage.virtual_width);
            w.write_f32(self.project.stage.virtual_height);
            w.write_f32(0.5); // health
            w.write_u32(0); // hits
            w.write_i64(0); // score
            w.write_u32(0); // combo
            w.write_u32(0); // max_combo
            w.write_u32(0); // misses
            w.write_u16(0); // entity_count
            w.write_u16(0); // visible_note_count
            return BINARY_HEADER_SIZE as i32;
        };

        let session = self.gameplay.as_mut().expect("gameplay active");
        let entity_count = session.bundle.active_entities.len().min(u16::MAX as usize);
        let visible_notes = session.frame_arena.get_slice(sim.visible_span);
        let note_count = visible_notes.len().min(u16::MAX as usize);

        let has_hud_strings = session.hud_revision != session.last_binary_hud_revision;

        let s_judgement = if has_hud_strings {
            truncate_utf8_for_u16(&session.last_judgement)
        } else {
            &[]
        };
        let s_countdown = if has_hud_strings {
            truncate_utf8_for_u16(&sim.countdown_label)
        } else {
            &[]
        };
        let s_err = if has_hud_strings {
            session
                .script_errors
                .first()
                .map(|s| truncate_utf8_for_u16(s))
                .unwrap_or(&[])
        } else {
            &[]
        };
        let s_log = if has_hud_strings {
            session
                .last_logs
                .last()
                .map(|s| truncate_utf8_for_u16(s))
                .unwrap_or(&[])
        } else {
            &[]
        };

        let hud_bytes = if has_hud_strings {
            8 + s_judgement.len() + s_countdown.len() + s_err.len() + s_log.len()
        } else {
            0
        };

        let total_bytes = BINARY_HEADER_SIZE
            + entity_count * BINARY_ENTITY_SIZE
            + note_count * BINARY_NOTE_SIZE
            + hud_bytes;

        if total_bytes > out_buf.len() {
            return -(total_bytes as i32);
        }

        let mut flags: u32 = 1; // active
        if self.project.downscroll {
            flags |= 1 << 1;
        }
        if session.paused {
            flags |= 1 << 2;
        }
        if session.finished {
            flags |= 1 << 3;
        }
        if self.project.control_mode == MobileControlMode::ClassicHitbox {
            flags |= 1 << 4;
        }
        if has_hud_strings {
            flags |= 1 << 5;
        }

        let masks =
            (sim.player_holding_mask as u32) | ((sim.opp_holding_mask as u32) << 8);

        let mut w = BinWriter::new(out_buf);
        w.write_u32(BINARY_FRAME_MAGIC);
        w.write_u32(flags);
        w.write_u32(session.hud_revision);
        w.write_u32(masks);
        w.write_i32(sim.countdown_stage);
        w.write_f32(sim.countdown_progress);
        w.write_f64(sim.song_pos_ms);
        w.write_f64(self.project.chart.stats.duration_ms);
        w.write_f64(self.project.chart.speed);
        w.write_f64(sim.active_bpm);
        w.write_f32(self.project.stage.camera_zoom);
        w.write_f32(self.project.stage.camera_shake);
        w.write_f32(self.project.stage.camera_x);
        w.write_f32(self.project.stage.camera_y);
        w.write_f32(self.project.stage.virtual_width);
        w.write_f32(self.project.stage.virtual_height);
        w.write_f32(session.health);
        w.write_u32(session.hits);
        w.write_i64(session.score);
        w.write_u32(session.combo);
        w.write_u32(session.max_combo);
        w.write_u32(session.misses);
        w.write_u16(entity_count as u16);
        w.write_u16(note_count as u16);

        for ent in session.bundle.active_entities.iter().take(entity_count) {
            let _ = self.texture_lru_cache.get(&ent.id);
            let frame = ent
                .atlas
                .as_ref()
                .and_then(|a| a.frame_at(ent.active_anim, ent.anim_timer_sec));

            let mut e_flags: u32 = (role_to_code(ent.role) & 0x3) << 4;
            if ent.visible {
                e_flags |= 1 << 0;
            }
            if ent.flip_x {
                e_flags |= 1 << 1;
            }
            if let Some(f) = frame {
                e_flags |= 1 << 2;
                if f.rotated {
                    e_flags |= 1 << 3;
                }
            }

            w.write_f32(ent.x);
            w.write_f32(ent.y);
            w.write_f32(ent.width);
            w.write_f32(ent.height);
            w.write_f32(ent.scale);
            w.write_f32(ent.opacity);
            w.write_u32(e_flags);

            if let Some(f) = frame {
                w.write_u32(f.x);
                w.write_u32(f.y);
                w.write_u32(f.width);
                w.write_u32(f.height);
                w.write_i32(f.frame_x);
                w.write_i32(f.frame_y);
                w.write_u32(f.frame_width);
                w.write_u32(f.frame_height);
            } else {
                w.write_u32(0);
                w.write_u32(0);
                w.write_u32(0);
                w.write_u32(0);
                w.write_i32(0);
                w.write_i32(0);
                w.write_u32(0);
                w.write_u32(0);
            }
        }

        for n in visible_notes.iter().take(note_count) {
            let mut n_flags: u32 = (n.direction as u32) & 0x3;
            if n.is_player {
                n_flags |= 1 << 2;
            }
            if n.is_holding {
                n_flags |= 1 << 3;
            }
            w.write_u64(n.id);
            w.write_f64(n.time_ms);
            w.write_f64(n.sustain_ms);
            w.write_u32(n_flags);
        }

        if has_hud_strings {
            w.write_str_u16(s_judgement);
            w.write_str_u16(s_countdown);
            w.write_str_u16(s_err);
            w.write_str_u16(s_log);
            session.last_binary_hud_revision = session.hud_revision;
        }

        debug_assert_eq!(w.pos, total_bytes);
        total_bytes as i32
    }

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

    pub fn stop_gameplay(&mut self) -> String {
        self.audio_engine.mixer.pause();
        self.gameplay = None;
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

        // El buffer demasiado pequeño debe retornar tamaño negativo sin consumir `last_binary_hud_revision`
        let mut tiny = [0u8; 32];
        let needed = session.step_gameplay_binary(0.016, 0, 0, &mut tiny);
        assert!(needed < -(BINARY_HEADER_SIZE as i32));

        let mut buf = vec![0u8; 32 * 1024];
        let written = session.step_gameplay_binary(0.016, 0, 0, &mut buf);
        assert!(written >= BINARY_HEADER_SIZE as i32);

        let magic = u32::from_le_bytes(buf[0..4].try_into().unwrap());
        assert_eq!(magic, BINARY_FRAME_MAGIC);
        let flags = u32::from_le_bytes(buf[4..8].try_into().unwrap());
        assert_ne!(flags & 1, 0); // active
        assert_ne!(flags & (1 << 5), 0); // has_hud_strings en el primer paquete

        // Segundo frame sin cambios de HUD no debería enviar strings
        let written_second = session.step_gameplay_binary(0.016, 0, 0, &mut buf);
        assert!(written_second > 0 && written_second <= written);
        let flags_second = u32::from_le_bytes(buf[4..8].try_into().unwrap());
        assert_eq!(flags_second & (1 << 5), 0);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
