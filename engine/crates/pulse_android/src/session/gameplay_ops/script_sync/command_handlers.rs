//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/script_sync/command_handlers.rs
//! Propósito: Drenaje y ejecución por frame de los `ScriptCommand` restantes emitidos por Lua/JS/TS:
//! entidades y dibujables dinámicos, propiedades de HUD/flechas/blackout, registro de tweens y
//! timers, pistas de audio, flash de pantalla, precarga/retiro/swap de sprites en RAM, Game Over
//! y recolección de errores de ejecución de scripts.

use super::camera_commands::try_apply_camera_command;
use super::props::{apply_entity_prop, apply_strum_prop, read_entity_prop, read_strum_prop};
use crate::session::{
    ActiveTimerState, ActiveTweenState, DynamicDrawableState, GameplayRuntimeState, TweenTarget,
    parse_anim_kind,
};
use pulse_audio::AudioEngineHandle;
use pulse_core::StageScene;
use pulse_core::project::MAX_AUDIO_TRACKS;
use pulse_scripting::ScriptCommand;
use std::sync::atomic::Ordering;

/// Drena todos los `ScriptCommand` emitidos por Lua/JS/TS en el frame actual y aplica sus efectos.
pub(in crate::session::gameplay_ops) fn drain_and_apply_script_commands(
    session: &mut GameplayRuntimeState,
    stage: &mut StageScene,
    audio_engine: &AudioEngineHandle,
) {
    let mut needs_z_sort = false;
    for raw_cmd in session.runtime.drain_commands() {
        let Some(cmd) = try_apply_camera_command(session, stage, raw_cmd) else {
            continue;
        };
        match cmd {
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
                    "visible" => {
                        if session.hud_visible {
                            1.0
                        } else {
                            0.0
                        }
                    }
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
                    "visible" => {
                        if session.arrows_visible {
                            1.0
                        } else {
                            0.0
                        }
                    }
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
                    super::super::enter_game_over(session, audio_engine);
                }
            }
            ScriptCommand::Log(msg) => {
                session.last_logs.push(msg);
                if session.last_logs.len() > 6 {
                    session.last_logs.remove(0);
                }
            }
            _ => {}
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
