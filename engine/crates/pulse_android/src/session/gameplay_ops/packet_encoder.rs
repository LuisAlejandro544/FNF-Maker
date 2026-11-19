//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/packet_encoder.rs
//! Propósito: Serialización de cada frame de gameplay hacia Kotlin Compose, tanto en JSON
//! (`step_gameplay`, usado en inicialización/pausa) como en paquete binario Little-Endian
//! `BFGP` v2 (`step_gameplay_binary`, sin asignaciones GC por frame a 60 FPS), respetando
//! el espejado horizontal de offsets cuando `flip_x` está activo.

use super::{
    BINARY_ENTITY_SIZE, BINARY_FRAME_MAGIC, BINARY_HEADER_SIZE, BINARY_NOTE_SIZE,
    BINARY_STRUM_SIZE,
};
use crate::session::{
    DynamicScriptItemJson, EngineSession, GameplayEntityFrameJson, GameplayStepJson,
    SparrowFrameJson, StrumOverrideJson, VisibleNoteJson, control_mode_to_key, role_to_key,
};
use pulse_core::project::MobileControlMode;
use pulse_core::sparrow::{CharacterAnimKind, SparrowFrame};
use pulse_core::{EntityRole, NoteSpecialKind, StageEntity};

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

fn compute_entity_frame_render<'a>(
    ent: &'a StageEntity,
    beat_pulse: f32,
) -> (f32, f32, f32, Option<&'a SparrowFrame>) {
    let is_char = matches!(ent.role, EntityRole::Player | EntityRole::Opponent);
    let clamp_idle = ent.active_anim == CharacterAnimKind::Idle && is_char;
    let frame = ent
        .atlas
        .as_ref()
        .and_then(|a| a.frame_at_synced(ent.active_anim, ent.anim_timer_sec, clamp_idle));
    let idle_frames = ent
        .atlas
        .as_ref()
        .map(|a| a.frame_count(CharacterAnimKind::Idle))
        .unwrap_or(0);
    let use_static_bpm_bounce =
        is_char && ent.active_anim == CharacterAnimKind::Idle && idle_frames <= 1;
    let base_y = if use_static_bpm_bounce {
        ent.y - beat_pulse * 6.0
    } else {
        ent.y
    };
    let render_scale = if use_static_bpm_bounce {
        ent.scale * (1.0 + beat_pulse * 0.025)
    } else {
        ent.scale
    };
    let [raw_off_x, off_y] = ent
        .anim_offsets
        .get(&ent.active_anim)
        .copied()
        .unwrap_or([0.0, 0.0]);
    let off_x = if ent.flip_x && !crate::session::has_pre_swapped_flip_descriptor(ent) {
        -raw_off_x
    } else {
        raw_off_x
    };
    let offset_fit = if let Some(f) = frame {
        let vis_w = if f.rotated { f.height } else { f.width }.max(1);
        let vis_h = if f.rotated { f.width } else { f.height }.max(1);
        let log_w = f.frame_width.max(vis_w).max(1) as f32;
        let log_h = f.frame_height.max(vis_h).max(1) as f32;
        ((ent.width * render_scale) / log_w).min((ent.height * render_scale) / log_h)
    } else {
        0.5 * render_scale
    };
    let render_x = ent.x + off_x * offset_fit;
    let render_y = base_y + off_y * offset_fit;
    (render_x, render_y, render_scale, frame)
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
    fn write_u8(&mut self, v: u8) {
        self.buf[self.pos] = v;
        self.pos += 1;
    }

    #[inline]
    fn write_u16(&mut self, v: u16) {
        self.write_bytes(&v.to_le_bytes());
    }

    #[inline]
    fn write_i16(&mut self, v: i16) {
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
    /// Avanza un paso de simulación y devuelve su representación JSON completa.
    pub fn step_gameplay(
        &mut self,
        dt_sec: f32,
        lane_pressed_mask: i32,
        lane_just_pressed_mask: i32,
    ) -> String {
        let control_mode_str = control_mode_to_key(self.project.control_mode).to_string();
        let Some(sim) =
            self.step_gameplay_internal(dt_sec, lane_pressed_mask, lane_just_pressed_mask)
        else {
            return serde_json::to_string(&GameplayStepJson {
                active: false,
                song_pos_ms: 0.0,
                duration_ms: 0.0,
                scroll_speed: 1.8,
                active_bpm: 120.0,
                beat_phase: 0.0,
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
                accuracy_pct: 100.0,
                health: 0.5,
                last_judgement: String::new(),
                rating_code: 0,
                rating_event_id: 0,
                splash_event_id: 0,
                splash_lane: 0,
                hud_visible: true,
                hud_opacity: 1.0,
                hud_zoom: 1.0,
                arrows_visible: true,
                arrows_opacity: 1.0,
                screen_blackout_alpha: 0.0,
                flash_color_argb: 0xFFFFFFFF,
                flash_alpha: 0.0,
                flash_over_hud: false,
                active_player_idx: -1,
                active_opponent_idx: -1,
                paused: false,
                finished: false,
                game_over: false,
                game_over_phase: 0,
                botplay: false,
                practice_mode: false,
                script_errors: Vec::new(),
                script_logs: Vec::new(),
                entities: Vec::new(),
                visible_notes: Vec::new(),
                strum_overrides: Vec::new(),
                dynamic_items: Vec::new(),
            })
            .unwrap_or_default();
        };

        let session = self.gameplay.as_mut().expect("gameplay active");
        let beat_pulse = (1.0 - sim.beat_phase).clamp(0.0, 1.0).powi(2);

        let active_player_idx = session
            .bundle
            .active_entities
            .iter()
            .position(|e| e.id == session.active_player_id)
            .or_else(|| {
                session
                    .bundle
                    .active_entities
                    .iter()
                    .position(|e| e.role == EntityRole::Player && e.visible)
            })
            .map(|i| i as i32)
            .unwrap_or(-1);
        let active_opponent_idx = session
            .bundle
            .active_entities
            .iter()
            .position(|e| e.id == session.active_opponent_id)
            .or_else(|| {
                session
                    .bundle
                    .active_entities
                    .iter()
                    .position(|e| e.role == EntityRole::Opponent && e.visible)
            })
            .map(|i| i as i32)
            .unwrap_or(-1);

        let mut entities = std::mem::take(&mut session.step_entities_scratch);
        entities.clear();
        for ent in &session.bundle.active_entities {
            let tex_path = session
                .compiled_texture_paths
                .get(&ent.id)
                .cloned()
                .or_else(|| ent.image_path.as_ref().map(|p| p.to_string_lossy().into_owned()));
            let (render_x, render_y, render_scale, frame_obj) =
                compute_entity_frame_render(ent, beat_pulse);

            let frame = frame_obj.map(|f| SparrowFrameJson {
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
                x: render_x,
                y: render_y,
                width: ent.width,
                height: ent.height,
                scale: render_scale,
                opacity: ent.opacity,
                rotation_deg: ent.rotation_deg,
                flip_x: ent.flip_x,
                visible: ent.visible,
                preload_requested: session.preload_requested_entities.contains(&ent.id),
                retired: session.retired_entities.contains(&ent.id),
                texture_path: tex_path,
                frame,
            });
        }

        let mut visible_notes = std::mem::take(&mut session.step_notes_scratch);
        visible_notes.clear();
        for n in session.frame_arena.get_slice(sim.visible_span) {
            let note_type = NoteSpecialKind::from_code(n.note_kind_code);
            visible_notes.push(VisibleNoteJson {
                id: n.id,
                time_ms: n.time_ms,
                direction: n.direction,
                is_player: n.is_player,
                sustain_ms: n.sustain_ms,
                head_hit: n.is_holding,
                note_type: note_type.canonical_key().to_string(),
                note_kind_code: n.note_kind_code,
            });
        }

        let strum_overrides = session
            .strum_overrides
            .iter()
            .map(|s| StrumOverrideJson {
                offset_x: s.offset_x,
                offset_y: s.offset_y,
                angle: s.angle,
                scale: s.scale,
                alpha: s.alpha,
            })
            .collect();

        let dynamic_items = session
            .dynamic_items
            .iter()
            .map(|d| DynamicScriptItemJson {
                id: d.id.clone(),
                kind: if d.is_text { "text" } else { "graphic" }.to_string(),
                is_hud: d.is_hud,
                visible: d.visible,
                x: d.x,
                y: d.y,
                width: d.width,
                height: d.height,
                opacity: d.opacity,
                angle: d.angle,
                color_argb: d.color_argb,
                text: d.text.clone(),
            })
            .collect();

        let step_dto = GameplayStepJson {
            active: true,
            song_pos_ms: sim.song_pos_ms,
            duration_ms: self.project.chart.stats.duration_ms,
            scroll_speed: session.active_scroll_speed,
            active_bpm: sim.active_bpm,
            beat_phase: sim.beat_phase,
            downscroll: session.active_downscroll,
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
            accuracy_pct: session.accuracy_pct,
            health: session.health,
            last_judgement: session.last_judgement.clone(),
            rating_code: session.last_rating_code,
            rating_event_id: session.rating_event_id,
            splash_event_id: session.splash_event_id,
            splash_lane: session.splash_lane,
            hud_visible: session.hud_visible,
            hud_opacity: session.hud_opacity,
            hud_zoom: session.hud_zoom,
            arrows_visible: session.arrows_visible,
            arrows_opacity: session.arrows_opacity,
            screen_blackout_alpha: session.screen_blackout_alpha,
            flash_color_argb: session.flash_color_argb,
            flash_alpha: session.flash_alpha,
            flash_over_hud: session.flash_over_hud,
            active_player_idx,
            active_opponent_idx,
            paused: session.paused,
            finished: session.finished,
            game_over: session.game_over,
            game_over_phase: session.game_over_phase,
            botplay: session.botplay,
            practice_mode: session.practice_mode,
            script_errors: session.script_errors.clone(),
            script_logs: session.last_logs.clone(),
            entities,
            visible_notes,
            strum_overrides,
            dynamic_items,
        };

        let out_json = serde_json::to_string(&step_dto).unwrap_or_else(|_| "{}".to_string());

        session.step_entities_scratch = step_dto.entities;
        session.step_notes_scratch = step_dto.visible_notes;

        out_json
    }

    /// Serializa el frame de gameplay en formato binario Little-Endian (`BFGP` v2) sin asignaciones JSON
    /// directamente dentro de un `DirectByteBuffer` de Kotlin.
    pub fn step_gameplay_binary(
        &mut self,
        dt_sec: f32,
        lane_pressed_mask: i32,
        lane_just_pressed_mask: i32,
        out_buf: &mut [u8],
    ) -> i32 {
        let Some(sim) =
            self.step_gameplay_internal(dt_sec, lane_pressed_mask, lane_just_pressed_mask)
        else {
            if out_buf.len() < BINARY_HEADER_SIZE {
                return -(BINARY_HEADER_SIZE as i32);
            }
            let mut w = BinWriter::new(out_buf);
            w.write_u32(BINARY_FRAME_MAGIC);
            w.write_u32(1 << 6); // inactive, hud_visible
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
            w.write_f32(100.0); // accuracy_pct
            w.write_f32(1.0); // hud_opacity
            w.write_f32(1.0); // hud_zoom
            w.write_u32(0); // rating_event_id
            w.write_u32(0); // splash_event_id
            w.write_f32(0.0); // beat_phase
            w.write_u32(0xFFFFFFFF); // flash_color_argb
            w.write_f32(0.0); // flash_alpha
            w.write_i16(-1); // active_player_idx
            w.write_i16(-1); // active_opponent_idx
            w.write_u16(0); // entity_count
            w.write_u16(0); // note_count
            w.write_u16(0); // strum_count
            w.write_u16(0); // dynamic_count
            debug_assert_eq!(w.pos, BINARY_HEADER_SIZE);
            return BINARY_HEADER_SIZE as i32;
        };

        let session = self.gameplay.as_mut().expect("gameplay active");
        let beat_pulse = (1.0 - sim.beat_phase).clamp(0.0, 1.0).powi(2);
        let entity_count = session.bundle.active_entities.len().min(u16::MAX as usize);
        let active_player_idx = session
            .bundle
            .active_entities
            .iter()
            .take(entity_count)
            .position(|e| e.id == session.active_player_id)
            .or_else(|| {
                session
                    .bundle
                    .active_entities
                    .iter()
                    .take(entity_count)
                    .position(|e| e.role == EntityRole::Player && e.visible)
            })
            .map(|i| i as i16)
            .unwrap_or(-1);
        let active_opponent_idx = session
            .bundle
            .active_entities
            .iter()
            .take(entity_count)
            .position(|e| e.id == session.active_opponent_id)
            .or_else(|| {
                session
                    .bundle
                    .active_entities
                    .iter()
                    .take(entity_count)
                    .position(|e| e.role == EntityRole::Opponent && e.visible)
            })
            .map(|i| i as i16)
            .unwrap_or(-1);
        let visible_notes = session.frame_arena.get_slice(sim.visible_span);
        let note_count = visible_notes.len().min(u16::MAX as usize);
        let strum_count: usize = 8;
        let dynamic_count = session.dynamic_items.len().min(u16::MAX as usize);

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

        let mut dynamic_bytes: usize = 0;
        for d in session.dynamic_items.iter().take(dynamic_count) {
            let txt = truncate_utf8_for_u16(&d.text);
            dynamic_bytes += 32 + 2 + txt.len();
        }

        let hud_bytes = if has_hud_strings {
            8 + s_judgement.len() + s_countdown.len() + s_err.len() + s_log.len()
        } else {
            0
        };

        let total_bytes = BINARY_HEADER_SIZE
            + entity_count * BINARY_ENTITY_SIZE
            + note_count * BINARY_NOTE_SIZE
            + strum_count * BINARY_STRUM_SIZE
            + dynamic_bytes
            + hud_bytes;

        if total_bytes > out_buf.len() {
            return -(total_bytes as i32);
        }

        let mut flags: u32 = 1; // active
        if session.active_downscroll {
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
        if session.hud_visible {
            flags |= 1 << 6;
        }
        if session.flash_over_hud {
            flags |= 1 << 7;
        }
        if !session.arrows_visible {
            flags |= 1 << 8;
        }
        if session.game_over {
            flags |= 1 << 9;
        }
        flags |= ((session.game_over_phase as u32) & 0x3) << 10;
        if session.botplay {
            flags |= 1 << 12;
        }
        if session.practice_mode {
            flags |= 1 << 13;
        }
        let blackout_u8 = (session.screen_blackout_alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
        let arrows_inv_u8 =
            ((1.0 - session.arrows_opacity.clamp(0.0, 1.0)) * 255.0).round() as u32;
        flags |= (blackout_u8 & 0xFF) << 16;
        flags |= (arrows_inv_u8 & 0xFF) << 24;

        let masks = (sim.player_holding_mask as u32)
            | ((sim.opp_holding_mask as u32) << 8)
            | (((session.last_rating_code as u32) & 0xF) << 16)
            | (((session.splash_lane as u32) & 0xF) << 20);

        let mut w = BinWriter::new(out_buf);
        w.write_u32(BINARY_FRAME_MAGIC);
        w.write_u32(flags);
        w.write_u32(session.hud_revision);
        w.write_u32(masks);
        w.write_i32(sim.countdown_stage);
        w.write_f32(sim.countdown_progress);
        w.write_f64(sim.song_pos_ms);
        w.write_f64(self.project.chart.stats.duration_ms);
        w.write_f64(session.active_scroll_speed);
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
        w.write_f32(session.accuracy_pct);
        w.write_f32(session.hud_opacity);
        w.write_f32(session.hud_zoom);
        w.write_u32(session.rating_event_id);
        w.write_u32(session.splash_event_id);
        w.write_f32(sim.beat_phase);
        w.write_u32(session.flash_color_argb);
        w.write_f32(session.flash_alpha);
        w.write_i16(active_player_idx);
        w.write_i16(active_opponent_idx);
        w.write_u16(entity_count as u16);
        w.write_u16(note_count as u16);
        w.write_u16(strum_count as u16);
        w.write_u16(dynamic_count as u16);

        for ent in session.bundle.active_entities.iter().take(entity_count) {
            let (render_x, render_y, render_scale, frame) =
                compute_entity_frame_render(ent, beat_pulse);

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
            if session.preload_requested_entities.contains(&ent.id) {
                e_flags |= 1 << 6;
            }
            if session.retired_entities.contains(&ent.id) {
                e_flags |= 1 << 7;
            }

            w.write_f32(render_x);
            w.write_f32(render_y);
            w.write_f32(ent.width);
            w.write_f32(ent.height);
            w.write_f32(render_scale);
            w.write_f32(ent.opacity);
            w.write_f32(ent.rotation_deg);
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
            n_flags |= ((n.note_kind_code as u32) & 0xF) << 4;
            w.write_u64(n.id);
            w.write_f64(n.time_ms);
            w.write_f64(n.sustain_ms);
            w.write_u32(n_flags);
        }

        for s in session.strum_overrides.iter() {
            w.write_f32(s.offset_x);
            w.write_f32(s.offset_y);
            w.write_f32(s.angle);
            w.write_f32(s.scale);
            w.write_f32(s.alpha);
        }

        for d in session.dynamic_items.iter().take(dynamic_count) {
            w.write_u8(if d.is_text { 1 } else { 0 });
            w.write_u8(if d.is_hud { 1 } else { 0 });
            w.write_u8(if d.visible { 1 } else { 0 });
            w.write_u8(0);
            w.write_f32(d.x);
            w.write_f32(d.y);
            w.write_f32(d.width);
            w.write_f32(d.height);
            w.write_f32(d.opacity);
            w.write_f32(d.angle);
            w.write_u32(d.color_argb);
            let txt = truncate_utf8_for_u16(&d.text);
            w.write_str_u16(txt);
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
}
