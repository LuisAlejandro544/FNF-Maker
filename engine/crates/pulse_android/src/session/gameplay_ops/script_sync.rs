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
#[allow(clippy::too_many_arguments)]
pub(super) fn sync_script_host_state(
    state: &ScriptHostState,
    stage: &StageScene,
    entities: &[StageEntity],
    dynamic_items: &[DynamicDrawableState],
    active_player_id: &str,
    active_opponent_id: &str,
    strums: &[StrumOverrideState; 8],
    song_pos_ms: f64,
    active_bpm: f64,
    must_hit: bool,
    gf_section: bool,
    active_camera_slot: usize,
    health: f32,
    score: i64,
    combo: u32,
    misses: u32,
    hits: u32,
    accuracy: f32,
    scroll_speed: f64,
    downscroll: bool,
    hud_visible: bool,
    hud_opacity: f32,
    hud_zoom: f32,
    arrows_visible: bool,
    arrows_opacity: f32,
    screen_blackout_alpha: f32,
    game_over: bool,
) {
    *state.song_pos_ms.lock() = song_pos_ms;
    *state.current_bpm.lock() = active_bpm;
    *state.camera_x.lock() = stage.camera_x;
    *state.camera_y.lock() = stage.camera_y;
    *state.camera_zoom.lock() = stage.camera_zoom;
    *state.camera_base_zoom.lock() = stage.camera_base_zoom;
    *state.camera_vignette_alpha.lock() = stage.vignette_alpha;
    *state.must_hit_section.lock() = must_hit;
    *state.gf_section.lock() = gf_section;
    *state.active_camera_slot.lock() = active_camera_slot;
    *state.health.lock() = health;
    *state.score.lock() = score;
    *state.combo.lock() = combo;
    *state.misses.lock() = misses;
    *state.hits.lock() = hits;
    *state.accuracy.lock() = accuracy;
    *state.scroll_speed.lock() = scroll_speed;
    *state.downscroll.lock() = downscroll;
    *state.hud_visible.lock() = hud_visible;
    *state.hud_opacity.lock() = hud_opacity;
    *state.hud_zoom.lock() = hud_zoom;
    *state.arrows_visible.lock() = arrows_visible;
    *state.arrows_opacity.lock() = arrows_opacity;
    *state.screen_blackout_alpha.lock() = screen_blackout_alpha;
    *state.game_over.lock() = game_over;

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
        if !active_player_id.is_empty() && ent.id == active_player_id {
            pos_map.insert("player".to_string(), (ent.x, ent.y));
            prop_map.insert("player".to_string(), snap);
            has_player_alias = true;
        }
        if !active_opponent_id.is_empty() && ent.id == active_opponent_id {
            pos_map.insert("opponent".to_string(), (ent.x, ent.y));
            prop_map.insert("opponent".to_string(), snap);
            has_opp_alias = true;
        }
        match ent.role {
            EntityRole::Opponent if !has_opp_alias && ent.visible => {
                pos_map.insert("opponent".to_string(), (ent.x, ent.y));
                prop_map.insert("opponent".to_string(), snap);
                has_opp_alias = true;
            }
            EntityRole::Player if !has_player_alias && ent.visible => {
                pos_map.insert("player".to_string(), (ent.x, ent.y));
                prop_map.insert("player".to_string(), snap);
                has_player_alias = true;
            }
            EntityRole::Background if !has_bg_alias && ent.visible => {
                pos_map.insert("background".to_string(), (ent.x, ent.y));
                prop_map.insert("background".to_string(), snap);
                has_bg_alias = true;
            }
            EntityRole::Decoration if !has_deco_alias && ent.visible => {
                pos_map.insert("decoration".to_string(), (ent.x, ent.y));
                prop_map.insert("decoration".to_string(), snap);
                has_deco_alias = true;
            }
            _ => {}
        }
    }
    for dyn_item in dynamic_items {
        if !prop_map.contains_key(&dyn_item.id) {
            pos_map.insert(dyn_item.id.clone(), (dyn_item.x, dyn_item.y));
            prop_map.insert(
                dyn_item.id.clone(),
                EntityPropSnapshot {
                    x: dyn_item.x,
                    y: dyn_item.y,
                    width: dyn_item.width,
                    height: dyn_item.height,
                    scale: 1.0,
                    opacity: dyn_item.opacity,
                    angle: dyn_item.angle,
                    visible: if dyn_item.visible { 1.0 } else { 0.0 },
                    flip_x: 0.0,
                    z_index: 0.0,
                    scroll_factor: if dyn_item.is_hud { 0.0 } else { 1.0 },
                },
            );
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
            TweenTarget::HudProp { prop } => match prop.as_str() {
                "opacity" | "alpha" => {
                    let clamped = val.clamp(0.0, 1.0);
                    session.hud_opacity = clamped;
                    if raw_t >= 1.0 {
                        session.hud_visible = clamped > 0.01;
                    }
                }
                "zoom" => session.hud_zoom = val.clamp(0.3, 2.5),
                "visible" => session.hud_visible = val > 0.0,
                _ => {}
            },
            TweenTarget::ArrowsProp { prop } => match prop.as_str() {
                "opacity" | "alpha" => {
                    let clamped = val.clamp(0.0, 1.0);
                    session.arrows_opacity = clamped;
                    if raw_t >= 1.0 {
                        session.arrows_visible = clamped > 0.01;
                    }
                }
                "visible" => session.arrows_visible = val > 0.0,
                _ => {}
            },
            TweenTarget::ScreenBlackout => {
                session.screen_blackout_alpha = val.clamp(0.0, 1.0);
            }
            TweenTarget::CameraVignetteAlpha => {
                stage.vignette_alpha = val.clamp(0.0, 1.0);
            }
            TweenTarget::CameraVignetteWidth => {
                stage.vignette_width = val.clamp(0.05, 0.50);
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
                let (cx, cy) = stage.clamp_camera_to_background(x, y, stage.camera_zoom);
                stage.camera_x = cx;
                stage.camera_y = cy;
            }
            ScriptCommand::SetCameraZoom(z) => {
                stage.camera_zoom = z.clamp(0.35, 2.5);
            }
            ScriptCommand::SetCameraBaseZoom(z) => {
                let clamped = z.clamp(0.35, 2.5);
                stage.camera_base_zoom = clamped;
                session.camera_slot_zoom_override = Some(clamped);
            }
            ScriptCommand::ShakeCamera { intensity } => {
                stage.camera_shake = intensity.clamp(0.0, 25.0);
            }
            ScriptCommand::SwitchCameraSlot {
                slot,
                zoom,
                duration_sec,
                ease,
            } => {
                let max_slot = stage.camera.cameras.len().saturating_sub(1);
                let clamped_slot = slot.min(max_slot);
                session.active_camera_slot = clamped_slot;
                session.camera_follow_lock = None;
                session.camera_gf_override = false;
                session.camera_offset_override = [0.0, 0.0];
                let (tx, ty, slot_zoom, _) = stage.resolve_camera_slot_target(
                    clamped_slot,
                    &session.active_player_id,
                    &session.active_opponent_id,
                );
                let z_override = zoom.map(|z| z.clamp(0.35, 2.5));
                session.camera_slot_zoom_override = z_override;
                let final_zoom = z_override.unwrap_or(slot_zoom);
                stage.camera_base_zoom = final_zoom;
                if duration_sec > 0.01 {
                    session.active_tweens.retain(|t| {
                        !matches!(
                            t.target,
                            TweenTarget::CameraPosX | TweenTarget::CameraPosY | TweenTarget::CameraZoom
                        )
                    });
                    session.active_tweens.push(ActiveTweenState {
                        tag: "__cam_slot_x".to_string(),
                        target: TweenTarget::CameraPosX,
                        start_val: stage.camera_x,
                        end_val: tx,
                        elapsed_sec: 0.0,
                        duration_sec,
                        ease: ease.clone(),
                    });
                    session.active_tweens.push(ActiveTweenState {
                        tag: "__cam_slot_y".to_string(),
                        target: TweenTarget::CameraPosY,
                        start_val: stage.camera_y,
                        end_val: ty,
                        elapsed_sec: 0.0,
                        duration_sec,
                        ease: ease.clone(),
                    });
                    session.active_tweens.push(ActiveTweenState {
                        tag: "__cam_slot_z".to_string(),
                        target: TweenTarget::CameraZoom,
                        start_val: stage.camera_zoom,
                        end_val: final_zoom,
                        elapsed_sec: 0.0,
                        duration_sec,
                        ease,
                    });
                }
            }
            ScriptCommand::SetCameraFollowLock { x, y } => {
                let (cx, cy) = stage.clamp_camera_to_background(x, y, stage.camera_base_zoom);
                session.camera_follow_lock = Some([cx, cy]);
            }
            ScriptCommand::ClearCameraFollowLock => {
                session.camera_follow_lock = None;
            }
            ScriptCommand::SetCameraClamp { enabled } => {
                stage.camera.clamp_to_background = enabled;
                let (cx, cy) =
                    stage.clamp_camera_to_background(stage.camera_x, stage.camera_y, stage.camera_zoom);
                stage.camera_x = cx;
                stage.camera_y = cy;
            }
            ScriptCommand::SetCameraVignette {
                color_argb,
                alpha,
                width,
                pulse,
                over_hud,
            } => {
                session.active_tweens.retain(|t| {
                    !matches!(
                        t.target,
                        TweenTarget::CameraVignetteAlpha | TweenTarget::CameraVignetteWidth
                    )
                });
                stage.vignette_color_argb = color_argb;
                stage.vignette_alpha = alpha.clamp(0.0, 1.0);
                stage.vignette_width = width.clamp(0.05, 0.50);
                stage.vignette_pulse = pulse.clamp(0.0, 1.0);
                stage.vignette_over_hud = over_hud;
            }
            ScriptCommand::TweenCameraVignette {
                tag,
                target_alpha,
                duration_sec,
                ease,
                color_argb,
                target_width,
                pulse,
                over_hud,
            } => {
                if let Some(c) = color_argb {
                    stage.vignette_color_argb = c;
                }
                if let Some(p) = pulse {
                    stage.vignette_pulse = p.clamp(0.0, 1.0);
                }
                if let Some(oh) = over_hud {
                    stage.vignette_over_hud = oh;
                }
                let end_alpha = target_alpha.clamp(0.0, 1.0);
                let tag_w = format!("{}__w", tag);
                session
                    .active_tweens
                    .retain(|t| t.tag != tag && t.tag != tag_w);
                if duration_sec <= 0.01 {
                    stage.vignette_alpha = end_alpha;
                    if let Some(w) = target_width {
                        stage.vignette_width = w.clamp(0.05, 0.50);
                    }
                } else {
                    session.active_tweens.push(ActiveTweenState {
                        tag,
                        target: TweenTarget::CameraVignetteAlpha,
                        start_val: stage.vignette_alpha,
                        end_val: end_alpha,
                        elapsed_sec: 0.0,
                        duration_sec: duration_sec.max(0.001),
                        ease: ease.clone(),
                    });
                    if let Some(w) = target_width {
                        session.active_tweens.push(ActiveTweenState {
                            tag: tag_w,
                            target: TweenTarget::CameraVignetteWidth,
                            start_val: stage.vignette_width,
                            end_val: w.clamp(0.05, 0.50),
                            elapsed_sec: 0.0,
                            duration_sec: duration_sec.max(0.001),
                            ease,
                        });
                    }
                }
            }
            ScriptCommand::ClearCameraVignette { duration_sec, ease } => {
                session.active_tweens.retain(|t| {
                    !matches!(
                        t.target,
                        TweenTarget::CameraVignetteAlpha | TweenTarget::CameraVignetteWidth
                    )
                });
                if duration_sec <= 0.01 {
                    stage.vignette_alpha = 0.0;
                } else {
                    session.active_tweens.push(ActiveTweenState {
                        tag: "__cam_vignette_clear".to_string(),
                        target: TweenTarget::CameraVignetteAlpha,
                        start_val: stage.vignette_alpha,
                        end_val: 0.0,
                        elapsed_sec: 0.0,
                        duration_sec: duration_sec.max(0.001),
                        ease,
                    });
                }
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
                    let min_hp = if session.practice_mode { 0.02 } else { 0.0 };
                    session.health = hp.clamp(min_hp, 1.0);
                }
            }
            ScriptCommand::AddHealth(delta) => {
                if delta.is_finite() {
                    let min_hp = if session.practice_mode { 0.02 } else { 0.0 };
                    session.health = (session.health + delta).clamp(min_hp, 1.0);
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
            ScriptCommand::SetArrowsProp { prop, value } => {
                if value.is_finite() {
                    match prop.as_str() {
                        "visible" => session.arrows_visible = value > 0.0,
                        "opacity" | "alpha" => session.arrows_opacity = value.clamp(0.0, 1.0),
                        _ => {}
                    }
                }
            }
            ScriptCommand::SetScreenBlackout(alpha) => {
                if alpha.is_finite() {
                    session.screen_blackout_alpha = alpha.clamp(0.0, 1.0);
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
                        if let Some(pages) = session.compiled_texture_pages.get(&source_id).cloned() {
                            session.compiled_texture_pages.insert(id.clone(), pages);
                        }
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
            ScriptCommand::TweenHudProp {
                tag,
                prop,
                target,
                duration_sec,
                ease,
            } => {
                session.active_tweens.retain(|t| t.tag != tag);
                let start_val = match prop.as_str() {
                    "opacity" | "alpha" => session.hud_opacity,
                    "zoom" => session.hud_zoom,
                    "visible" => if session.hud_visible { 1.0 } else { 0.0 },
                    _ => target,
                };
                session.active_tweens.push(ActiveTweenState {
                    tag,
                    target: TweenTarget::HudProp { prop },
                    start_val,
                    end_val: target,
                    elapsed_sec: 0.0,
                    duration_sec: duration_sec.max(0.001),
                    ease,
                });
            }
            ScriptCommand::TweenArrowsProp {
                tag,
                prop,
                target,
                duration_sec,
                ease,
            } => {
                session.active_tweens.retain(|t| t.tag != tag);
                let start_val = match prop.as_str() {
                    "opacity" | "alpha" => session.arrows_opacity,
                    "visible" => if session.arrows_visible { 1.0 } else { 0.0 },
                    _ => target,
                };
                session.active_tweens.push(ActiveTweenState {
                    tag,
                    target: TweenTarget::ArrowsProp { prop },
                    start_val,
                    end_val: target,
                    elapsed_sec: 0.0,
                    duration_sec: duration_sec.max(0.001),
                    ease,
                });
            }
            ScriptCommand::TweenScreenBlackout {
                tag,
                target,
                duration_sec,
                ease,
            } => {
                session.active_tweens.retain(|t| t.tag != tag);
                let start_val = session.screen_blackout_alpha;
                session.active_tweens.push(ActiveTweenState {
                    tag,
                    target: TweenTarget::ScreenBlackout,
                    start_val,
                    end_val: target.clamp(0.0, 1.0),
                    elapsed_sec: 0.0,
                    duration_sec: duration_sec.max(0.001),
                    ease,
                });
            }
            ScriptCommand::CancelTween(tag) => {
                let tag_y = format!("{}__y", tag);
                let tag_w = format!("{}__w", tag);
                session
                    .active_tweens
                    .retain(|t| t.tag != tag && t.tag != tag_y && t.tag != tag_w);
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
            ScriptCommand::FlashEffect {
                color_argb,
                duration_sec,
                start_alpha,
                over_hud,
            } => {
                let dur = duration_sec.max(0.02);
                let alpha = start_alpha.clamp(0.0, 1.0);
                session.flash_color_argb = color_argb;
                session.flash_alpha = alpha;
                session.flash_decay_per_sec = alpha / dur;
                session.flash_over_hud = over_hud;
            }
            ScriptCommand::PreloadEntity(entity_id) => {
                if !entity_id.trim().is_empty() {
                    session.retired_entities.remove(&entity_id);
                    session.preload_requested_entities.insert(entity_id);
                }
            }
            ScriptCommand::RetireEntity(entity_id) => {
                if !entity_id.trim().is_empty() {
                    if let Some(ent) = session
                        .bundle
                        .active_entities
                        .iter_mut()
                        .find(|e| e.id == entity_id)
                    {
                        ent.visible = false;
                    }
                    session.preload_requested_entities.remove(&entity_id);
                    session.retired_entities.insert(entity_id);
                }
            }
            ScriptCommand::SwapEntity {
                old_id,
                new_id,
                dispose_old,
            } => {
                if !old_id.trim().is_empty() && old_id != new_id {
                    if let Some(old_ent) = session
                        .bundle
                        .active_entities
                        .iter_mut()
                        .find(|e| e.id == old_id)
                    {
                        old_ent.visible = false;
                    }
                    if dispose_old {
                        session.preload_requested_entities.remove(&old_id);
                        session.retired_entities.insert(old_id.clone());
                    }
                }
                if !new_id.trim().is_empty() {
                    session.retired_entities.remove(&new_id);
                    session.preload_requested_entities.insert(new_id.clone());
                    if let Some(new_ent) = session
                        .bundle
                        .active_entities
                        .iter_mut()
                        .find(|e| e.id == new_id)
                    {
                        new_ent.visible = true;
                        if new_ent.opacity <= 0.01 {
                            new_ent.set_opacity(1.0);
                        }
                    }
                }
            }
            ScriptCommand::SetActiveCharacter {
                role,
                entity_id,
                dispose_old,
            } => {
                let lower = role.trim().to_ascii_lowercase();
                let is_player = matches!(lower.as_str(), "player" | "jugador" | "bf" | "boyfriend");
                let prev_id = if is_player {
                    std::mem::replace(&mut session.active_player_id, entity_id.clone())
                } else {
                    std::mem::replace(&mut session.active_opponent_id, entity_id.clone())
                };
                if !prev_id.is_empty() && prev_id != entity_id {
                    if let Some(prev_ent) = session
                        .bundle
                        .active_entities
                        .iter_mut()
                        .find(|e| e.id == prev_id)
                    {
                        prev_ent.visible = false;
                    }
                    if dispose_old {
                        session.preload_requested_entities.remove(&prev_id);
                        session.retired_entities.insert(prev_id);
                    }
                }
                if !entity_id.trim().is_empty() {
                    session.retired_entities.remove(&entity_id);
                    session.preload_requested_entities.insert(entity_id.clone());
                    if let Some(next_ent) = session
                        .bundle
                        .active_entities
                        .iter_mut()
                        .find(|e| e.id == entity_id)
                    {
                        next_ent.visible = true;
                        if next_ent.opacity <= 0.01 {
                            next_ent.set_opacity(1.0);
                        }
                    }
                }
            }
            ScriptCommand::TriggerGameOver => {
                if !session.practice_mode {
                    super::enter_game_over(session, audio_engine);
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
