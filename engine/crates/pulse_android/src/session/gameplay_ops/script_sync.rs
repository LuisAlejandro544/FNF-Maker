//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/script_sync.rs
//! Propósito: Sincronización bidireccional entre el estado del juego Rust y las máquinas
//! virtuales Lua/QuickJS: evaluación de curvas de interpolación (`tweens`), temporizadores,
//! lectura/escritura de propiedades de entidades y receptores (`strumOverrides`), y drenaje
//! de `ScriptCommand` en cada tick de simulación.

use crate::session::{
    ActiveTimerState, ActiveTweenState, DynamicDrawableState, GameplayRuntimeState,
    StrumOverrideState, TweenTarget, parse_anim_kind,
};
use pulse_audio::AudioEngineHandle;
use pulse_core::project::MAX_AUDIO_TRACKS;
use pulse_core::{EntityRole, StageEntity, StageScene};
use pulse_scripting::{EntityPropSnapshot, ScriptCommand, ScriptHostState, StrumPropSnapshot};
use std::sync::atomic::Ordering;

pub(super) fn eval_ease(name: &str, raw_t: f32) -> f32 {
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

pub(super) fn read_entity_prop(
    entities: &[StageEntity],
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

pub(super) fn apply_entity_prop(
    entities: &mut [StageEntity],
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

pub(super) fn read_strum_prop(strums: &[StrumOverrideState; 8], idx: usize, prop: &str) -> f32 {
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

pub(super) fn apply_strum_prop(
    strums: &mut [StrumOverrideState; 8],
    idx: usize,
    prop: &str,
    value: f32,
) {
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

/// Publica una instantánea del estado actual del juego dentro de `ScriptHostState` para que
/// los scripts Lua/TS/JS lean posiciones, BPM, cámara y estadísticas en tiempo real.
pub(super) fn sync_script_host_state(
    state: &ScriptHostState,
    stage: &StageScene,
    entities: &[StageEntity],
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

/// Avanza temporizadores activos y dispara `onTimerCompleted` al expirar.
pub(super) fn update_active_timers(session: &mut GameplayRuntimeState, dt: f32) {
    if session.active_timers.is_empty() {
        return;
    }
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

/// Avanza tweens activos sobre entidades, cámara y receptores, disparando `onTweenCompleted`.
pub(super) fn update_active_tweens(
    session: &mut GameplayRuntimeState,
    stage: &mut StageScene,
    dt: f32,
) {
    if session.active_tweens.is_empty() {
        return;
    }
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
                stage.camera_x = val.clamp(-1000.0, 2600.0);
            }
            TweenTarget::CameraPosY => {
                stage.camera_y = val.clamp(-1000.0, 1800.0);
            }
            TweenTarget::CameraZoom => {
                stage.camera_zoom = val.clamp(0.35, 2.5);
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

/// Drena todos los `ScriptCommand` emitidos por Lua/JS/TS en el frame actual y aplica sus efectos.
pub(super) fn drain_and_apply_script_commands(
    session: &mut GameplayRuntimeState,
    stage: &mut StageScene,
    audio_engine: &AudioEngineHandle,
) {
    let mut needs_z_sort = false;
    for cmd in session.runtime.drain_commands() {
        match cmd {
            ScriptCommand::SetCameraPos { x, y } => {
                stage.camera_x = x.clamp(-1000.0, 2600.0);
                stage.camera_y = y.clamp(-1000.0, 1800.0);
            }
            ScriptCommand::SetCameraZoom(z) => {
                stage.camera_zoom = z.clamp(0.35, 2.5);
            }
            ScriptCommand::SetCameraBaseZoom(z) => {
                stage.camera_base_zoom = z.clamp(0.35, 2.5);
            }
            ScriptCommand::ShakeCamera { intensity } => {
                stage.camera_shake = intensity.clamp(0.0, 25.0);
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
                    start_val: stage.camera_x,
                    end_val: x,
                    elapsed_sec: 0.0,
                    duration_sec: duration_sec.max(0.001),
                    ease: ease.clone(),
                });
                session.active_tweens.push(ActiveTweenState {
                    tag: tag_y,
                    target: TweenTarget::CameraPosY,
                    start_val: stage.camera_y,
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
                    start_val: stage.camera_zoom,
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
                    audio_engine.mixer.configs[slot].set_gain(gain);
                }
            }
            ScriptCommand::SetTrackMute { slot, muted } => {
                if slot < MAX_AUDIO_TRACKS {
                    audio_engine.mixer.configs[slot]
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

    for err in session.runtime.drain_runtime_errors() {
        if !session.script_errors.iter().any(|e| e == &err) {
            if session.script_errors.len() >= 6 {
                session.script_errors.remove(0);
            }
            session.script_errors.push(err);
        }
    }
}
