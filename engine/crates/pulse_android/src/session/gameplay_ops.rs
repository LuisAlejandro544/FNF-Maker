//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops.rs
//! Propósito: Compilación en caliente inteligente (`rayon`) y simulación de gameplay en tiempo real
//! sincronizada con el reloj maestro de `pulse_audio`, usando:
//! - Arena temporal por frame (`FrameBumpArena`) con reinicio O(1) en cada tick.
//! - Descarte progresivo por ventana deslizante (`NoteSlidingWindow`) y paquete binario DirectByteBuffer.
//! - Motor de tweens/timers nativo, eventos del chart (`onEvent`, `onStepHit`, `onCountdownTick`),
//!   animaciones Miss de personaje, calificación 4 niveles (`Sick`/`Good`/`Bad`/`Miss`) y destellos.

use super::{
    ActiveTimerState, ActiveTweenState, DynamicDrawableState, DynamicScriptItemJson,
    EngineSession, GameplayEntityFrameJson, GameplayRuntimeState, GameplayStepJson,
    SparrowFrameJson, StrumOverrideJson, StrumOverrideState, TweenTarget, VisibleNoteJson,
    control_mode_to_key, dir_to_anim, dir_to_miss_anim, parse_anim_kind, role_to_key,
    trigger_role_anim,
};
use pulse_audio::{StreamingAudioSource, merge_tracks_to_wav};
use pulse_core::EntityRole;
use pulse_core::frame_memory::{
    ArenaSpan, FrameBumpArena, FrameVisibleNote, NotePlayState, NoteSlidingWindow,
};
use pulse_core::hot_compile::{DeviceResourceTier, hot_compile_project_to_dir};
use pulse_core::project::{MAX_AUDIO_TRACKS, MobileControlMode};
use pulse_core::sparrow::CharacterAnimKind;
use pulse_scripting::{
    EntityPropSnapshot, ModScriptRuntime, ScriptCommand, StrumPropSnapshot,
};
use std::collections::HashMap;
use std::sync::atomic::Ordering;

const HIT_WINDOW_SICK_MS: f64 = 55.0;
const HIT_WINDOW_GOOD_MS: f64 = 105.0;
const HIT_WINDOW_BAD_MS: f64 = 155.0;

pub const BINARY_FRAME_MAGIC: u32 = 0x4246_4750; // "BFGP"
pub const BINARY_HEADER_SIZE: usize = 140;
pub const BINARY_ENTITY_SIZE: usize = 64;
pub const BINARY_NOTE_SIZE: usize = 28;
pub const BINARY_STRUM_SIZE: usize = 20;

struct StepSimOutput {
    song_pos_ms: f64,
    active_bpm: f64,
    beat_phase: f32,
    countdown_stage: i32,
    countdown_label: String,
    countdown_progress: f32,
    player_holding_mask: u8,
    opp_holding_mask: u8,
    visible_span: ArenaSpan<FrameVisibleNote>,
}

fn eval_ease(name: &str, raw_t: f32) -> f32 {
    let t = raw_t.clamp(0.0, 1.0);
    match name.trim().to_ascii_lowercase().as_str() {
        "quadin" => t * t,
        "quadout" => 1.0 - (1.0 - t) * (1.0 - t),
        "quadinout" => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
            }
        }
        "cubicin" => t * t * t,
        "cubicout" => 1.0 - (1.0 - t).powi(3),
        "cubicinout" => {
            if t < 0.5 {
                4.0 * t * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
            }
        }
        "sinein" => 1.0 - ((t * std::f32::consts::FRAC_PI_2).cos()),
        "sineout" => (t * std::f32::consts::FRAC_PI_2).sin(),
        "sineinout" => -((std::f32::consts::PI * t).cos() - 1.0) / 2.0,
        "backout" => {
            let c1 = 1.70158;
            let c3 = c1 + 1.0;
            1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
        }
        "elasticout" => {
            if t <= 0.0 || t >= 1.0 {
                t
            } else {
                let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                2.0f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
            }
        }
        "bounceout" => {
            let n1 = 7.5625;
            let d1 = 2.75;
            if t < 1.0 / d1 {
                n1 * t * t
            } else if t < 2.0 / d1 {
                let u = t - 1.5 / d1;
                n1 * u * u + 0.75
            } else if t < 2.5 / d1 {
                let u = t - 2.25 / d1;
                n1 * u * u + 0.9375
            } else {
                let u = t - 2.625 / d1;
                n1 * u * u + 0.984375
            }
        }
        _ => t,
    }
}

fn read_entity_prop(
    entities: &[pulse_core::StageEntity],
    dynamics: &[DynamicDrawableState],
    entity_id: &str,
    prop: &str,
) -> Option<f32> {
    if let Some(ent) = entities.iter().find(|e| e.id == entity_id) {
        return match prop {
            "x" => Some(ent.x),
            "y" => Some(ent.y),
            "width" => Some(ent.width),
            "height" => Some(ent.height),
            "scale" => Some(ent.scale),
            "opacity" | "alpha" => Some(ent.opacity),
            "angle" | "rotation" | "rotationDeg" => Some(ent.rotation_deg),
            "visible" => Some(if ent.visible { 1.0 } else { 0.0 }),
            "flipX" | "flip_x" => Some(if ent.flip_x { 1.0 } else { 0.0 }),
            "zIndex" | "z_index" | "z" => Some(ent.z_index as f32),
            "scrollFactor" | "scroll_factor" => Some(ent.scroll_factor),
            _ => None,
        };
    }
    if let Some(dyn_item) = dynamics.iter().find(|d| d.id == entity_id) {
        return match prop {
            "x" => Some(dyn_item.x),
            "y" => Some(dyn_item.y),
            "width" | "scale" => Some(dyn_item.width),
            "height" => Some(dyn_item.height),
            "opacity" | "alpha" => Some(dyn_item.opacity),
            "angle" | "rotation" | "rotationDeg" => Some(dyn_item.angle),
            "visible" => Some(if dyn_item.visible { 1.0 } else { 0.0 }),
            _ => None,
        };
    }
    None
}

fn apply_entity_prop(
    entities: &mut [pulse_core::StageEntity],
    dynamics: &mut [DynamicDrawableState],
    entity_id: &str,
    prop: &str,
    value: f32,
) -> bool {
    if !value.is_finite() {
        return false;
    }
    let mut needs_z_sort = false;
    if let Some(ent) = entities.iter_mut().find(|e| e.id == entity_id) {
        match prop {
            "opacity" | "alpha" => ent.set_opacity(value),
            "scale" => ent.set_scale(value),
            "x" => ent.x = value,
            "y" => ent.y = value,
            "width" => ent.width = value.max(1.0),
            "height" => ent.height = value.max(1.0),
            "angle" | "rotation" | "rotationDeg" => ent.rotation_deg = value,
            "visible" => ent.visible = value > 0.0,
            "flipX" | "flip_x" => ent.flip_x = value > 0.0,
            "zIndex" | "z_index" | "z" => {
                ent.z_index = value.round() as i32;
                needs_z_sort = true;
            }
            "scrollFactor" | "scroll_factor" => ent.scroll_factor = value.clamp(0.0, 4.0),
            _ => {}
        }
    }
    if let Some(dyn_item) = dynamics.iter_mut().find(|d| d.id == entity_id) {
        match prop {
            "x" => dyn_item.x = value,
            "y" => dyn_item.y = value,
            "width" => dyn_item.width = value.max(1.0),
            "height" => dyn_item.height = value.max(1.0),
            "scale" => dyn_item.width = (dyn_item.width * value.clamp(0.05, 8.0)).max(1.0),
            "opacity" | "alpha" => dyn_item.opacity = value.clamp(0.0, 1.0),
            "angle" | "rotation" | "rotationDeg" => dyn_item.angle = value,
            "visible" => dyn_item.visible = value > 0.0,
            _ => {}
        }
    }
    needs_z_sort
}

fn read_strum_prop(strums: &[StrumOverrideState; 8], idx: usize, prop: &str) -> f32 {
    let s = &strums[idx.min(7)];
    match prop {
        "x" | "offsetX" | "offset_x" => s.offset_x,
        "y" | "offsetY" | "offset_y" => s.offset_y,
        "angle" | "rotation" => s.angle,
        "scale" => s.scale,
        "alpha" | "opacity" => s.alpha,
        _ => 0.0,
    }
}

fn apply_strum_prop(strums: &mut [StrumOverrideState; 8], idx: usize, prop: &str, value: f32) {
    if !value.is_finite() {
        return;
    }
    let s = &mut strums[idx.min(7)];
    match prop {
        "x" | "offsetX" | "offset_x" => s.offset_x = value.clamp(-1200.0, 1200.0),
        "y" | "offsetY" | "offset_y" => s.offset_y = value.clamp(-800.0, 800.0),
        "angle" | "rotation" => s.angle = value,
        "scale" => s.scale = value.clamp(0.05, 4.0),
        "alpha" | "opacity" => s.alpha = value.clamp(0.0, 1.0),
        _ => {}
    }
}

fn sync_script_host_state(
    state: &pulse_scripting::ScriptHostState,
    stage: &pulse_core::StageScene,
    entities: &[pulse_core::StageEntity],
    strums: &[StrumOverrideState; 8],
    song_pos_ms: f64,
    active_bpm: f64,
    must_hit: bool,
    health: f32,
    score: i64,
    combo: u32,
    misses: u32,
    hits: u32,
    accuracy: f32,
    scroll_speed: f64,
    downscroll: bool,
) {
    *state.song_pos_ms.lock() = song_pos_ms;
    *state.current_bpm.lock() = active_bpm;
    *state.camera_x.lock() = stage.camera_x;
    *state.camera_y.lock() = stage.camera_y;
    *state.camera_zoom.lock() = stage.camera_zoom;
    *state.camera_base_zoom.lock() = stage.camera_base_zoom;
    *state.must_hit_section.lock() = must_hit;
    *state.health.lock() = health;
    *state.score.lock() = score;
    *state.combo.lock() = combo;
    *state.misses.lock() = misses;
    *state.hits.lock() = hits;
    *state.accuracy.lock() = accuracy;
    *state.scroll_speed.lock() = scroll_speed;
    *state.downscroll.lock() = downscroll;

    {
        let mut s_lock = state.strum_props.lock();
        for (i, ov) in strums.iter().enumerate() {
            s_lock[i] = StrumPropSnapshot {
                offset_x: ov.offset_x,
                offset_y: ov.offset_y,
                angle: ov.angle,
                scale: ov.scale,
                alpha: ov.alpha,
            };
        }
    }

    let mut pos_map = state.entity_positions.lock();
    let mut prop_map = state.entity_props.lock();
    pos_map.clear();
    prop_map.clear();
    let mut has_opp_alias = false;
    let mut has_player_alias = false;
    let mut has_bg_alias = false;
    let mut has_deco_alias = false;
    for ent in entities {
        pos_map.insert(ent.id.clone(), (ent.x, ent.y));
        let snap = EntityPropSnapshot {
            x: ent.x,
            y: ent.y,
            width: ent.width,
            height: ent.height,
            scale: ent.scale,
            opacity: ent.opacity,
            angle: ent.rotation_deg,
            visible: if ent.visible { 1.0 } else { 0.0 },
            flip_x: if ent.flip_x { 1.0 } else { 0.0 },
            z_index: ent.z_index as f32,
            scroll_factor: ent.scroll_factor,
        };
        prop_map.insert(ent.id.clone(), snap);
        match ent.role {
            EntityRole::Opponent if !has_opp_alias => {
                pos_map.insert("opponent".to_string(), (ent.x, ent.y));
                prop_map.insert("opponent".to_string(), snap);
                has_opp_alias = true;
            }
            EntityRole::Player if !has_player_alias => {
                pos_map.insert("player".to_string(), (ent.x, ent.y));
                prop_map.insert("player".to_string(), snap);
                has_player_alias = true;
            }
            EntityRole::Background if !has_bg_alias => {
                pos_map.insert("background".to_string(), (ent.x, ent.y));
                prop_map.insert("background".to_string(), snap);
                has_bg_alias = true;
            }
            EntityRole::Decoration if !has_deco_alias => {
                pos_map.insert("decoration".to_string(), (ent.x, ent.y));
                prop_map.insert("decoration".to_string(), snap);
                has_deco_alias = true;
            }
            _ => {}
        }
    }
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
    fn write_u8(&mut self, v: u8) {
        self.buf[self.pos] = v;
        self.pos += 1;
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

        if self.project.active_audio_count() > 0
            && self.decoded_tracks.iter().all(|t| t.is_none())
        {
            self.reload_project_audio_tracks();
        }

        let out_dir = self.cache_dir.join("compiled_cache");
        let _ = std::fs::create_dir_all(&out_dir);

        let mut bundle =
            hot_compile_project_to_dir(&self.project, tier, Some(&out_dir), |_pct, _msg| {});

        let mut texture_paths = HashMap::with_capacity(bundle.compiled_texture_paths.len());
        for (entity_id, path) in &bundle.compiled_texture_paths {
            texture_paths.insert(
                entity_id.clone(),
                path.to_string_lossy().into_owned(),
            );
        }

        let target_sr = self
            .audio_engine
            .mixer
            .sample_rate
            .load(Ordering::Relaxed)
            .max(8000);
        let compiled_wav = out_dir.join("compiled_mix.wav");
        match merge_tracks_to_wav(
            &self.decoded_tracks,
            &self.project.audio_tracks,
            target_sr,
            &compiled_wav,
        ) {
            Ok(Some(merge_rep)) => {
                bundle.report.merged_audio_tracks = merge_rep.merged_tracks;
                bundle.report.compiled_audio_path = Some(merge_rep.output_path.clone());
                bundle.report.estimated_audio_ram_saved_kb = merge_rep.estimated_ram_saved_kb;
                self.compiled_audio_path = Some(merge_rep.output_path);
            }
            _ => {
                let _ = std::fs::remove_file(&compiled_wav);
                self.compiled_audio_path = None;
            }
        }

        self.compiled_texture_paths = texture_paths;
        self.last_compile_report = Some(bundle.report.clone());
        self.compiled_bundle = Some(bundle);
        self.ok_response("Compilación en caliente inteligente completada")
    }

    pub fn start_gameplay(&mut self) -> String {
        self.gameplay = None;
        self.project.ensure_camera_script();
        self.project
            .stage
            .apply_camera_config(self.project.stage.camera.clone());
        self.project.stage.camera_shake = 0.0;

        let out_dir = self.cache_dir.join("compiled_cache");
        let bundle = self.compiled_bundle.clone().unwrap_or_else(|| {
            hot_compile_project_to_dir(&self.project, DeviceResourceTier::Balanced, Some(&out_dir), |_, _| {})
        });

        self.project
            .chart
            .notes
            .sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        self.project
            .chart
            .events
            .sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));

        let (mut runtime, script_errors) =
            ModScriptRuntime::load_from_scripts(&bundle.optimized_scripts);
        let initial_must_hit = self.project.chart.must_hit_at_ms(0.0);
        let default_strums = [StrumOverrideState::default(); 8];
        sync_script_host_state(
            &runtime.state,
            &self.project.stage,
            &bundle.active_entities,
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
            active_scroll_speed: self.project.chart.speed.max(0.5),
            active_downscroll: self.project.downscroll,
            last_beat: -10,
            last_step: -40,
            next_event_idx: 0,
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
        let beat_sec = (beat_ms / 1000.0) as f32;
        let step_ms = beat_ms * 0.25;
        let beat_phase = (song_pos_ms.rem_euclid(beat_ms) / beat_ms) as f32;
        let current_beat = (song_pos_ms / beat_ms).floor() as i32;
        let current_step = (song_pos_ms / step_ms).floor() as i32;
        let must_hit_now = self.project.chart.must_hit_at_ms(song_pos_ms.max(0.0));

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
            if countdown_stage >= 0 && !session.paused && !session.finished {
                session.runtime.call_on_countdown_tick(countdown_stage);
            }
        }
        let prev_judgement = session.last_judgement.clone();
        let prev_paused = session.paused;
        let prev_finished = session.finished;
        let prev_log_count = session.last_logs.len();
        let prev_last_log = session.last_logs.last().cloned();
        let prev_err_count = session.script_errors.len();

        let mut just_pressed = [
            (lane_just_pressed_mask & 1) != 0,
            (lane_just_pressed_mask & 2) != 0,
            (lane_just_pressed_mask & 4) != 0,
            (lane_just_pressed_mask & 8) != 0,
        ];
        let mut player_holding_mask: u8 = 0;
        let mut opp_holding_mask: u8 = 0;

        if !session.paused && !session.finished {
            // 1. Actualizar temporizadores activos
            if !session.active_timers.is_empty() {
                let mut completed_timers: Vec<String> = Vec::new();
                session.active_timers.retain_mut(|timer| {
                    timer.remaining_sec -= dt;
                    if timer.remaining_sec <= 0.0 {
                        completed_timers.push(timer.tag.clone());
                        false
                    } else {
                        true
                    }
                });
                for tag in completed_timers {
                    session.runtime.call_on_timer_completed(&tag);
                }
            }

            // 2. Actualizar tweens activos
            if !session.active_tweens.is_empty() {
                let mut completed_tweens: Vec<String> = Vec::new();
                let mut sort_z = false;
                session.active_tweens.retain_mut(|tw| {
                    tw.elapsed_sec += dt;
                    let raw_t = if tw.duration_sec <= 0.0001 {
                        1.0
                    } else {
                        (tw.elapsed_sec / tw.duration_sec).clamp(0.0, 1.0)
                    };
                    let eased = eval_ease(&tw.ease, raw_t);
                    let val = tw.start_val + (tw.end_val - tw.start_val) * eased;
                    match &tw.target {
                        TweenTarget::EntityProp { entity_id, prop } => {
                            if apply_entity_prop(
                                &mut session.bundle.active_entities,
                                &mut session.dynamic_items,
                                entity_id,
                                prop,
                                val,
                            ) {
                                sort_z = true;
                            }
                        }
                        TweenTarget::CameraPosX => {
                            self.project.stage.camera_x = val.clamp(-1000.0, 2600.0);
                        }
                        TweenTarget::CameraPosY => {
                            self.project.stage.camera_y = val.clamp(-1000.0, 1800.0);
                        }
                        TweenTarget::CameraZoom => {
                            self.project.stage.camera_zoom = val.clamp(0.35, 2.5);
                        }
                        TweenTarget::StrumProp { strum_idx, prop } => {
                            apply_strum_prop(&mut session.strum_overrides, *strum_idx, prop, val);
                        }
                    }
                    if raw_t >= 1.0 {
                        completed_tweens.push(tw.tag.clone());
                        false
                    } else {
                        true
                    }
                });
                if sort_z {
                    session.bundle.active_entities.sort_by_key(|e| e.z_index);
                }
                for tag in completed_tweens {
                    session.runtime.call_on_tween_completed(&tag);
                }
            }

            // 3. Sincronizar estado maestro con Lua/QuickJS
            sync_script_host_state(
                &session.runtime.state,
                &self.project.stage,
                &session.bundle.active_entities,
                &session.strum_overrides,
                song_pos_ms,
                active_bpm,
                must_hit_now,
                session.health,
                session.score,
                session.combo,
                session.misses,
                session.hits,
                session.accuracy_pct,
                session.active_scroll_speed,
                session.active_downscroll,
            );

            // 4. Despachar eventos de chart.json cronológicamente
            while session.next_event_idx < self.project.chart.events.len() {
                let ev = &self.project.chart.events[session.next_event_idx];
                if song_pos_ms < ev.time_ms {
                    break;
                }
                session
                    .runtime
                    .call_on_event(&ev.name, &ev.value1, &ev.value2);
                session.next_event_idx += 1;
            }

            // 5. Hooks por step (16th) y por beat
            if current_step > session.last_step {
                session.last_step = current_step;
                session.runtime.call_on_step_hit(current_step);
            }

            if current_beat > session.last_beat {
                session.last_beat = current_beat;
                session.runtime.call_on_beat_hit(current_beat);

                for ent in &mut session.bundle.active_entities {
                    if ent.active_anim == CharacterAnimKind::Idle
                        && matches!(ent.role, EntityRole::Player | EntityRole::Opponent)
                    {
                        let idle_dur = ent
                            .atlas
                            .as_ref()
                            .map(|a| a.anim_duration_sec(CharacterAnimKind::Idle))
                            .unwrap_or(0.0);
                        let beat_every = if idle_dur > beat_sec * 1.35 { 2 } else { 1 };
                        if current_beat.rem_euclid(beat_every) == 0 {
                            ent.anim_timer_sec = 0.0;
                        }
                    }
                }
            }
            session.runtime.call_on_update(dt);

            // 6. Aplicar comandos emitidos por scripts
            let mut needs_z_sort = false;
            for cmd in session.runtime.drain_commands() {
                match cmd {
                    ScriptCommand::SetCameraPos { x, y } => {
                        self.project.stage.camera_x = x.clamp(-1000.0, 2600.0);
                        self.project.stage.camera_y = y.clamp(-1000.0, 1800.0);
                    }
                    ScriptCommand::SetCameraZoom(z) => {
                        self.project.stage.camera_zoom = z.clamp(0.35, 2.5);
                    }
                    ScriptCommand::SetCameraBaseZoom(z) => {
                        self.project.stage.camera_base_zoom = z.clamp(0.35, 2.5);
                    }
                    ScriptCommand::ShakeCamera { intensity } => {
                        self.project.stage.camera_shake = intensity.clamp(0.0, 25.0);
                    }
                    ScriptCommand::SetEntityProp {
                        entity_id,
                        prop,
                        value,
                    } => {
                        if apply_entity_prop(
                            &mut session.bundle.active_entities,
                            &mut session.dynamic_items,
                            &entity_id,
                            &prop,
                            value,
                        ) {
                            needs_z_sort = true;
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
                            if let Some(kind) = parse_anim_kind(&anim_name) {
                                ent.active_anim = kind;
                                ent.anim_timer_sec = 0.0;
                            }
                        }
                    }
                    ScriptCommand::SetStrumProp {
                        strum_idx,
                        prop,
                        value,
                    } => {
                        apply_strum_prop(&mut session.strum_overrides, strum_idx, &prop, value);
                    }
                    ScriptCommand::SetHealth(hp) => {
                        if hp.is_finite() {
                            session.health = hp.clamp(0.0, 1.0);
                        }
                    }
                    ScriptCommand::AddHealth(delta) => {
                        if delta.is_finite() {
                            session.health = (session.health + delta).clamp(0.0, 1.0);
                        }
                    }
                    ScriptCommand::SetScore(val) => {
                        session.score = val;
                    }
                    ScriptCommand::AddScore(delta) => {
                        session.score += delta;
                    }
                    ScriptCommand::SetScrollSpeed(spd) => {
                        if spd.is_finite() {
                            session.active_scroll_speed = spd.clamp(0.3, 6.0);
                        }
                    }
                    ScriptCommand::SetDownscroll(ds) => {
                        session.active_downscroll = ds;
                    }
                    ScriptCommand::SetHudProp { prop, value } => {
                        if value.is_finite() {
                            match prop.as_str() {
                                "visible" => session.hud_visible = value > 0.0,
                                "opacity" | "alpha" => session.hud_opacity = value.clamp(0.0, 1.0),
                                "zoom" => session.hud_zoom = value.clamp(0.3, 2.5),
                                _ => {}
                            }
                        }
                    }
                    ScriptCommand::MakeGraphic {
                        id,
                        x,
                        y,
                        width,
                        height,
                        color_argb,
                        is_hud,
                    } => {
                        session.dynamic_items.retain(|d| d.id != id);
                        session.dynamic_items.push(DynamicDrawableState {
                            id,
                            is_text: false,
                            is_hud,
                            visible: true,
                            x,
                            y,
                            width: width.max(1.0),
                            height: height.max(1.0),
                            opacity: 1.0,
                            angle: 0.0,
                            color_argb,
                            text: String::new(),
                        });
                    }
                    ScriptCommand::MakeText {
                        id,
                        text,
                        x,
                        y,
                        font_size,
                        color_argb,
                        is_hud,
                    } => {
                        session.dynamic_items.retain(|d| d.id != id);
                        session.dynamic_items.push(DynamicDrawableState {
                            id,
                            is_text: true,
                            is_hud,
                            visible: true,
                            x,
                            y,
                            width: font_size.clamp(8.0, 140.0),
                            height: font_size.clamp(8.0, 140.0),
                            opacity: 1.0,
                            angle: 0.0,
                            color_argb,
                            text,
                        });
                    }
                    ScriptCommand::SetTextString { id, text } => {
                        if let Some(item) = session.dynamic_items.iter_mut().find(|d| d.id == id) {
                            item.text = text;
                        }
                    }
                    ScriptCommand::CloneSprite {
                        id,
                        source_id,
                        x,
                        y,
                    } => {
                        if !session
                            .bundle
                            .active_entities
                            .iter()
                            .any(|e| e.id == id)
                        {
                            if let Some(src) = session
                                .bundle
                                .active_entities
                                .iter()
                                .find(|e| e.id == source_id)
                                .cloned()
                            {
                                let mut cloned = src;
                                cloned.id = id.clone();
                                cloned.x = x;
                                cloned.y = y;
                                if let Some(path) = session.compiled_texture_paths.get(&source_id).cloned() {
                                    session.compiled_texture_paths.insert(id, path);
                                }
                                session.bundle.active_entities.push(cloned);
                                needs_z_sort = true;
                            }
                        }
                    }
                    ScriptCommand::RemoveSprite(id) => {
                        session.dynamic_items.retain(|d| d.id != id);
                        session.bundle.active_entities.retain(|e| e.id != id);
                    }
                    ScriptCommand::TweenEntityProp {
                        tag,
                        entity_id,
                        prop,
                        target,
                        duration_sec,
                        ease,
                    } => {
                        session.active_tweens.retain(|t| t.tag != tag);
                        let start_val = read_entity_prop(
                            &session.bundle.active_entities,
                            &session.dynamic_items,
                            &entity_id,
                            &prop,
                        )
                        .unwrap_or(target);
                        session.active_tweens.push(ActiveTweenState {
                            tag,
                            target: TweenTarget::EntityProp { entity_id, prop },
                            start_val,
                            end_val: target,
                            elapsed_sec: 0.0,
                            duration_sec: duration_sec.max(0.001),
                            ease,
                        });
                    }
                    ScriptCommand::TweenCameraPos {
                        tag,
                        x,
                        y,
                        duration_sec,
                        ease,
                    } => {
                        let tag_y = format!("{}__y", tag);
                        session.active_tweens.retain(|t| t.tag != tag && t.tag != tag_y);
                        session.active_tweens.push(ActiveTweenState {
                            tag,
                            target: TweenTarget::CameraPosX,
                            start_val: self.project.stage.camera_x,
                            end_val: x,
                            elapsed_sec: 0.0,
                            duration_sec: duration_sec.max(0.001),
                            ease: ease.clone(),
                        });
                        session.active_tweens.push(ActiveTweenState {
                            tag: tag_y,
                            target: TweenTarget::CameraPosY,
                            start_val: self.project.stage.camera_y,
                            end_val: y,
                            elapsed_sec: 0.0,
                            duration_sec: duration_sec.max(0.001),
                            ease,
                        });
                    }
                    ScriptCommand::TweenCameraZoom {
                        tag,
                        zoom,
                        duration_sec,
                        ease,
                    } => {
                        session.active_tweens.retain(|t| t.tag != tag);
                        session.active_tweens.push(ActiveTweenState {
                            tag,
                            target: TweenTarget::CameraZoom,
                            start_val: self.project.stage.camera_zoom,
                            end_val: zoom,
                            elapsed_sec: 0.0,
                            duration_sec: duration_sec.max(0.001),
                            ease,
                        });
                    }
                    ScriptCommand::TweenStrumProp {
                        tag,
                        strum_idx,
                        prop,
                        target,
                        duration_sec,
                        ease,
                    } => {
                        session.active_tweens.retain(|t| t.tag != tag);
                        let start_val = read_strum_prop(&session.strum_overrides, strum_idx, &prop);
                        session.active_tweens.push(ActiveTweenState {
                            tag,
                            target: TweenTarget::StrumProp { strum_idx, prop },
                            start_val,
                            end_val: target,
                            elapsed_sec: 0.0,
                            duration_sec: duration_sec.max(0.001),
                            ease,
                        });
                    }
                    ScriptCommand::CancelTween(tag) => {
                        let tag_y = format!("{}__y", tag);
                        session
                            .active_tweens
                            .retain(|t| t.tag != tag && t.tag != tag_y);
                    }
                    ScriptCommand::RunTimer { tag, duration_sec } => {
                        session.active_timers.retain(|t| t.tag != tag);
                        session.active_timers.push(ActiveTimerState {
                            tag,
                            remaining_sec: duration_sec.max(0.001),
                        });
                    }
                    ScriptCommand::CancelTimer(tag) => {
                        session.active_timers.retain(|t| t.tag != tag);
                    }
                    ScriptCommand::SetTrackVolume { slot, gain } => {
                        if slot < MAX_AUDIO_TRACKS {
                            self.audio_engine.mixer.configs[slot].set_gain(gain);
                        }
                    }
                    ScriptCommand::SetTrackMute { slot, muted } => {
                        if slot < MAX_AUDIO_TRACKS {
                            self.audio_engine.mixer.configs[slot]
                                .muted
                                .store(muted, Ordering::Relaxed);
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
            if needs_z_sort {
                session.bundle.active_entities.sort_by_key(|e| e.z_index);
            }

            // Recopilar errores ocurridos en tiempo de ejecución
            for err in session.runtime.drain_runtime_errors() {
                if !session.script_errors.iter().any(|e| e == &err) {
                    if session.script_errors.len() >= 6 {
                        session.script_errors.remove(0);
                    }
                    session.script_errors.push(err);
                }
            }

            let target_base_zoom = self.project.stage.camera_base_zoom.clamp(0.35, 2.5);
            self.project.stage.camera_zoom +=
                (target_base_zoom - self.project.stage.camera_zoom) * (dt * 6.0);
            self.project.stage.camera_shake *= (1.0 - dt * 10.0).max(0.0);

            let mut any_recent_miss = false;
            let note_len = self.project.chart.notes.len();
            let start_idx = session.note_window.active_start_idx.min(note_len);

            for idx in start_idx..note_len {
                let state = session.note_window.state(idx);
                if matches!(state, NotePlayState::Hit | NotePlayState::Missed) {
                    continue;
                }
                let note = &self.project.chart.notes[idx];
                if note.time_ms > song_pos_ms + HIT_WINDOW_BAD_MS + 10.0 {
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
                                    session.judged_count += 1;
                                    session.accuracy_pct = ((session.accuracy_sum / session.judged_count as f64) as f32).clamp(0.0, 100.0);
                                    session.score -= 25;
                                    session.health = (session.health - 0.03).max(0.0);
                                    session.last_judgement = "Miss".to_string();
                                    session.last_rating_code = 4;
                                    session.rating_event_id = session.rating_event_id.wrapping_add(1);
                                    any_recent_miss = true;
                                    trigger_role_anim(
                                        &mut session.bundle.active_entities,
                                        EntityRole::Player,
                                        dir_to_miss_anim(note.direction),
                                    );
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
                            let abs_diff = diff.abs();
                            let lane_idx = note.direction as usize;
                            if just_pressed[lane_idx] && abs_diff <= HIT_WINDOW_BAD_MS {
                                if note.sustain_ms > 45.0 {
                                    session.note_window.mark_holding(idx);
                                    player_holding_mask |= 1u8 << (lane_idx as u8);
                                } else {
                                    session.note_window.mark_hit(idx);
                                }
                                just_pressed[lane_idx] = false;

                                let (pts, rating_code, label, acc_weight) = if abs_diff <= HIT_WINDOW_SICK_MS {
                                    session.splash_event_id = session.splash_event_id.wrapping_add(1);
                                    session.splash_lane = lane_idx as u8;
                                    (350, 1u8, "¡Sick!", 100.0)
                                } else if abs_diff <= HIT_WINDOW_GOOD_MS {
                                    (200, 2u8, "¡Good!", 75.0)
                                } else {
                                    (100, 3u8, "Bad", 40.0)
                                };
                                session.last_judgement = label.to_string();
                                session.last_rating_code = rating_code;
                                session.rating_event_id = session.rating_event_id.wrapping_add(1);
                                session.judged_count += 1;
                                session.accuracy_sum += acc_weight;
                                session.accuracy_pct = ((session.accuracy_sum / session.judged_count as f64) as f32).clamp(0.0, 100.0);

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
                            } else if song_pos_ms > note.time_ms + HIT_WINDOW_BAD_MS {
                                session.note_window.mark_missed(idx);
                                session.combo = 0;
                                session.misses += 1;
                                session.judged_count += 1;
                                session.accuracy_pct = ((session.accuracy_sum / session.judged_count as f64) as f32).clamp(0.0, 100.0);
                                session.score -= 50;
                                session.health = (session.health - 0.06).max(0.0);
                                session.last_judgement = "Miss".to_string();
                                session.last_rating_code = 4;
                                session.rating_event_id = session.rating_event_id.wrapping_add(1);
                                any_recent_miss = true;
                                trigger_role_anim(
                                    &mut session.bundle.active_entities,
                                    EntityRole::Player,
                                    dir_to_miss_anim(note.direction),
                                );
                                session.runtime.call_on_note_miss(lane_idx as i32, true);
                            }
                        }
                    }
                    NotePlayState::Hit | NotePlayState::Missed => {}
                }
            }

            session
                .note_window
                .advance_and_discard(&self.project.chart.notes, song_pos_ms, 240.0);

            self.audio_engine
                .mixer
                .player_missed_recently
                .store(any_recent_miss, Ordering::Relaxed);

            let sing_hold_sec = (beat_sec * 0.90).clamp(0.28, 0.58);
            for ent in &mut session.bundle.active_entities {
                ent.anim_timer_sec += dt;
                if ent.active_anim != CharacterAnimKind::Idle && ent.anim_timer_sec > sing_hold_sec {
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
            || prev_err_count != session.script_errors.len()
        {
            hud_changed = true;
        }
        if hud_changed {
            session.hud_revision = session.hud_revision.wrapping_add(1);
        }

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
            beat_phase,
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
                paused: false,
                finished: false,
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

        let mut entities = std::mem::take(&mut session.step_entities_scratch);
        entities.clear();
        for ent in &session.bundle.active_entities {
            let tex_path = session
                .compiled_texture_paths
                .get(&ent.id)
                .cloned()
                .or_else(|| ent.image_path.as_ref().map(|p| p.to_string_lossy().into_owned()));
            let is_char = matches!(ent.role, EntityRole::Player | EntityRole::Opponent);
            let clamp_idle = ent.active_anim == CharacterAnimKind::Idle && is_char;
            let frame_obj = ent
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
            let render_y = if use_static_bpm_bounce {
                ent.y - beat_pulse * 6.0
            } else {
                ent.y
            };
            let render_scale = if use_static_bpm_bounce {
                ent.scale * (1.0 + beat_pulse * 0.025)
            } else {
                ent.scale
            };

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
                x: ent.x,
                y: render_y,
                width: ent.width,
                height: ent.height,
                scale: render_scale,
                opacity: ent.opacity,
                rotation_deg: ent.rotation_deg,
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
            paused: session.paused,
            finished: session.finished,
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
        let Some(sim) = self.step_gameplay_internal(dt_sec, lane_pressed_mask, lane_just_pressed_mask) else {
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
        w.write_u16(entity_count as u16);
        w.write_u16(note_count as u16);
        w.write_u16(strum_count as u16);
        w.write_u16(dynamic_count as u16);

        for ent in session.bundle.active_entities.iter().take(entity_count) {
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
            let render_y = if use_static_bpm_bounce {
                ent.y - beat_pulse * 6.0
            } else {
                ent.y
            };
            let render_scale = if use_static_bpm_bounce {
                ent.scale * (1.0 + beat_pulse * 0.025)
            } else {
                ent.scale
            };

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
