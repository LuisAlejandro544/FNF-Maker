//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/script_sync/props.rs
//! Propósito: Lectura y escritura segura de propiedades sobre entidades del escenario,
//! dibujables dinámicos (`makeGraphic`/`makeText`) y receptores (`strumOverrides`), además
//! de publicar la instantánea completa del frame hacia `ScriptHostState`.

use crate::session::{DynamicDrawableState, StrumOverrideState};
use pulse_core::{EntityRole, StageEntity, StageScene};
use pulse_scripting::{EntityPropSnapshot, ScriptHostState, StrumPropSnapshot};

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
pub(in crate::session::gameplay_ops) fn sync_script_host_state(
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
