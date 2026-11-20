//! Archivo: engine/crates/pulse_scripting/src/lua_vm/bindings_hud_fx.rs
//! Propósito: Registra en los globales de Lua 5.4 los bindings de estado del chart/gameplay,
//! HUD y flechas, gráficos/textos dinámicos, tweens, timers, audio, flashes, precarga IO,
//! swaps de escenario y game over.

use crate::bridge::{ScriptCommand, ScriptHostState, parse_color_argb};
use mlua::{Lua, Result, Table};
use std::sync::Arc;

pub(super) fn register_hud_fx_bindings(
    lua: &Lua,
    globals: &Table,
    host: &Arc<ScriptHostState>,
) -> Result<()> {
    // --- Estado del Chart / Gameplay / HUD ---
    {
        let h = Arc::clone(host);
        globals.set(
            "getMustHitSection",
            lua.create_function(move |_, ()| Ok(*h.must_hit_section.lock()))?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getGfSection",
            lua.create_function(move |_, ()| Ok(*h.gf_section.lock()))?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getActiveBpm",
            lua.create_function(move |_, ()| Ok(*h.current_bpm.lock()))?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getSongPosMs",
            lua.create_function(move |_, ()| Ok(*h.song_pos_ms.lock()))?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set("getHealth", lua.create_function(move |_, ()| Ok(*h.health.lock()))?)?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setHealth",
            lua.create_function(move |_, value: f32| {
                if value.is_finite() {
                    *h.health.lock() = value.clamp(0.0, 1.0);
                    h.push_command(ScriptCommand::SetHealth(value));
                }
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "addHealth",
            lua.create_function(move |_, delta: f32| {
                if delta.is_finite() {
                    let mut hp = h.health.lock();
                    *hp = (*hp + delta).clamp(0.0, 1.0);
                    h.push_command(ScriptCommand::AddHealth(delta));
                }
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getScore",
            lua.create_function(move |_, ()| Ok(*h.score.lock() as f64))?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setScore",
            lua.create_function(move |_, val: f64| {
                if val.is_finite() {
                    let v = val.round() as i64;
                    *h.score.lock() = v;
                    h.push_command(ScriptCommand::SetScore(v));
                }
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "addScore",
            lua.create_function(move |_, delta: f64| {
                if delta.is_finite() {
                    let d = delta.round() as i64;
                    *h.score.lock() += d;
                    h.push_command(ScriptCommand::AddScore(d));
                }
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set("getCombo", lua.create_function(move |_, ()| Ok(*h.combo.lock() as i32))?)?;
    }
    {
        let h = Arc::clone(host);
        globals.set("getMisses", lua.create_function(move |_, ()| Ok(*h.misses.lock() as i32))?)?;
    }
    {
        let h = Arc::clone(host);
        globals.set("getHits", lua.create_function(move |_, ()| Ok(*h.hits.lock() as i32))?)?;
    }
    {
        let h = Arc::clone(host);
        globals.set("getAccuracy", lua.create_function(move |_, ()| Ok(*h.accuracy.lock()))?)?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getScrollSpeed",
            lua.create_function(move |_, ()| Ok(*h.scroll_speed.lock()))?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setScrollSpeed",
            lua.create_function(move |_, speed: f64| {
                if speed.is_finite() {
                    let clamped = speed.clamp(0.3, 6.0);
                    *h.scroll_speed.lock() = clamped;
                    h.push_command(ScriptCommand::SetScrollSpeed(clamped));
                }
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getDownscroll",
            lua.create_function(move |_, ()| Ok(*h.downscroll.lock()))?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setDownscroll",
            lua.create_function(move |_, enabled: bool| {
                *h.downscroll.lock() = enabled;
                h.push_command(ScriptCommand::SetDownscroll(enabled));
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setHudVisible",
            lua.create_function(move |_, visible: bool| {
                *h.hud_visible.lock() = visible;
                h.push_command(ScriptCommand::SetHudProp {
                    prop: "visible".to_string(),
                    value: if visible { 1.0 } else { 0.0 },
                });
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getHudVisible",
            lua.create_function(move |_, ()| Ok(*h.hud_visible.lock()))?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setHudOpacity",
            lua.create_function(move |_, alpha: f32| {
                if alpha.is_finite() {
                    let clamped = alpha.clamp(0.0, 1.0);
                    *h.hud_opacity.lock() = clamped;
                    h.push_command(ScriptCommand::SetHudProp {
                        prop: "opacity".to_string(),
                        value: clamped,
                    });
                }
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getHudOpacity",
            lua.create_function(move |_, ()| Ok(*h.hud_opacity.lock()))?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setHudZoom",
            lua.create_function(move |_, zoom: f32| {
                if zoom.is_finite() {
                    let clamped = zoom.clamp(0.4, 2.5);
                    *h.hud_zoom.lock() = clamped;
                    h.push_command(ScriptCommand::SetHudProp {
                        prop: "zoom".to_string(),
                        value: clamped,
                    });
                }
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getHudZoom",
            lua.create_function(move |_, ()| Ok(*h.hud_zoom.lock()))?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setArrowsVisible",
            lua.create_function(move |_, visible: bool| {
                *h.arrows_visible.lock() = visible;
                h.push_command(ScriptCommand::SetArrowsProp {
                    prop: "visible".to_string(),
                    value: if visible { 1.0 } else { 0.0 },
                });
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getArrowsVisible",
            lua.create_function(move |_, ()| Ok(*h.arrows_visible.lock()))?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setArrowsOpacity",
            lua.create_function(move |_, alpha: f32| {
                if alpha.is_finite() {
                    let clamped = alpha.clamp(0.0, 1.0);
                    *h.arrows_opacity.lock() = clamped;
                    h.push_command(ScriptCommand::SetArrowsProp {
                        prop: "opacity".to_string(),
                        value: clamped,
                    });
                }
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getArrowsOpacity",
            lua.create_function(move |_, ()| Ok(*h.arrows_opacity.lock()))?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setScreenBlackout",
            lua.create_function(move |_, alpha: f32| {
                if alpha.is_finite() {
                    let clamped = alpha.clamp(0.0, 1.0);
                    *h.screen_blackout_alpha.lock() = clamped;
                    h.push_command(ScriptCommand::SetScreenBlackout(clamped));
                }
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getScreenBlackout",
            lua.create_function(move |_, ()| Ok(*h.screen_blackout_alpha.lock()))?,
        )?;
    }

    // --- Gráficos, Textos y Clonación dinámica ---
    {
        let h = Arc::clone(host);
        globals.set(
            "makeGraphic",
            lua.create_function(
                move |_, (id, x, y, width, height, color): (String, f32, f32, f32, f32, String)| {
                    h.push_command(ScriptCommand::MakeGraphic {
                        id,
                        x,
                        y,
                        width,
                        height,
                        color_argb: parse_color_argb(&color),
                        is_hud: false,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "makeHudGraphic",
            lua.create_function(
                move |_, (id, x, y, width, height, color): (String, f32, f32, f32, f32, String)| {
                    h.push_command(ScriptCommand::MakeGraphic {
                        id,
                        x,
                        y,
                        width,
                        height,
                        color_argb: parse_color_argb(&color),
                        is_hud: true,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "makeText",
            lua.create_function(
                move |_, (id, text, x, y, font_size, color): (String, String, f32, f32, f32, String)| {
                    h.push_command(ScriptCommand::MakeText {
                        id,
                        text,
                        x,
                        y,
                        font_size,
                        color_argb: parse_color_argb(&color),
                        is_hud: true,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setTextString",
            lua.create_function(move |_, (id, text): (String, String)| {
                h.push_command(ScriptCommand::SetTextString { id, text });
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "cloneSprite",
            lua.create_function(move |_, (id, source_id, x, y): (String, String, f32, f32)| {
                h.push_command(ScriptCommand::CloneSprite {
                    id,
                    source_id,
                    x,
                    y,
                });
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "removeSprite",
            lua.create_function(move |_, id: String| {
                h.push_command(ScriptCommand::RemoveSprite(id));
                Ok(())
            })?,
        )?;
    }

    // --- Tweens Nativos ---
    {
        let h = Arc::clone(host);
        globals.set(
            "tweenEntityProp",
            lua.create_function(
                move |_, (tag, entity_id, prop, target, dur, ease): (String, String, String, f32, f32, String)| {
                    h.push_command(ScriptCommand::TweenEntityProp {
                        tag,
                        entity_id,
                        prop,
                        target,
                        duration_sec: dur,
                        ease,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "tweenCameraPos",
            lua.create_function(
                move |_, (tag, x, y, dur, ease): (String, f32, f32, f32, String)| {
                    h.push_command(ScriptCommand::TweenCameraPos {
                        tag,
                        x,
                        y,
                        duration_sec: dur,
                        ease,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "tweenCameraZoom",
            lua.create_function(move |_, (tag, zoom, dur, ease): (String, f32, f32, String)| {
                h.push_command(ScriptCommand::TweenCameraZoom {
                    tag,
                    zoom,
                    duration_sec: dur,
                    ease,
                });
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "tweenStrumProp",
            lua.create_function(
                move |_, (tag, strum_idx, prop, target, dur, ease): (String, i32, String, f32, f32, String)| {
                    h.push_command(ScriptCommand::TweenStrumProp {
                        tag,
                        strum_idx: strum_idx.clamp(0, 7) as usize,
                        prop,
                        target,
                        duration_sec: dur,
                        ease,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "tweenHudProp",
            lua.create_function(
                move |_, (tag, prop, target, dur, ease): (String, String, f32, f32, String)| {
                    h.push_command(ScriptCommand::TweenHudProp {
                        tag,
                        prop,
                        target,
                        duration_sec: dur,
                        ease,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "tweenHudOpacity",
            lua.create_function(
                move |_, (tag, target, dur, ease): (String, f32, f32, String)| {
                    h.push_command(ScriptCommand::TweenHudProp {
                        tag,
                        prop: "opacity".to_string(),
                        target,
                        duration_sec: dur,
                        ease,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "tweenArrowsOpacity",
            lua.create_function(
                move |_, (tag, target, dur, ease): (String, f32, f32, String)| {
                    h.push_command(ScriptCommand::TweenArrowsProp {
                        tag,
                        prop: "opacity".to_string(),
                        target,
                        duration_sec: dur,
                        ease,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "tweenScreenBlackout",
            lua.create_function(
                move |_, (tag, target, dur, ease): (String, f32, f32, String)| {
                    h.push_command(ScriptCommand::TweenScreenBlackout {
                        tag,
                        target,
                        duration_sec: dur,
                        ease,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "cancelTween",
            lua.create_function(move |_, tag: String| {
                h.push_command(ScriptCommand::CancelTween(tag));
                Ok(())
            })?,
        )?;
    }

    // --- Temporizadores y Audio ---
    {
        let h = Arc::clone(host);
        globals.set(
            "runTimer",
            lua.create_function(move |_, (tag, duration_sec): (String, f32)| {
                h.push_command(ScriptCommand::RunTimer { tag, duration_sec });
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "cancelTimer",
            lua.create_function(move |_, tag: String| {
                h.push_command(ScriptCommand::CancelTimer(tag));
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setTrackVolume",
            lua.create_function(move |_, (slot, gain): (i32, f32)| {
                h.push_command(ScriptCommand::SetTrackVolume {
                    slot: slot.max(0) as usize,
                    gain,
                });
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setTrackMute",
            lua.create_function(move |_, (slot, muted): (i32, bool)| {
                h.push_command(ScriptCommand::SetTrackMute {
                    slot: slot.max(0) as usize,
                    muted,
                });
                Ok(())
            })?,
        )?;
    }

    // --- Flashes, Precarga IO, Swaps de Escenario y Personajes Activos ---
    {
        let h = Arc::clone(host);
        globals.set(
            "flashCamera",
            lua.create_function(move |_, (color, duration_sec): (String, f32)| {
                h.push_command(ScriptCommand::FlashEffect {
                    color_argb: parse_color_argb(&color),
                    duration_sec: duration_sec.max(0.02),
                    start_alpha: 1.0,
                    over_hud: false,
                });
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "flashHud",
            lua.create_function(move |_, (color, duration_sec): (String, f32)| {
                h.push_command(ScriptCommand::FlashEffect {
                    color_argb: parse_color_argb(&color),
                    duration_sec: duration_sec.max(0.02),
                    start_alpha: 1.0,
                    over_hud: true,
                });
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "flashEffect",
            lua.create_function(
                move |_, (color, duration_sec, start_alpha, over_hud): (String, f32, f32, bool)| {
                    h.push_command(ScriptCommand::FlashEffect {
                        color_argb: parse_color_argb(&color),
                        duration_sec: duration_sec.max(0.02),
                        start_alpha: start_alpha.clamp(0.0, 1.0),
                        over_hud,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "preloadEntity",
            lua.create_function(move |_, entity_id: String| {
                h.push_command(ScriptCommand::PreloadEntity(entity_id));
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "retireEntity",
            lua.create_function(move |_, entity_id: String| {
                h.push_command(ScriptCommand::RetireEntity(entity_id));
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "swapEntity",
            lua.create_function(
                move |_, (old_id, new_id, dispose_old): (String, String, bool)| {
                    h.push_command(ScriptCommand::SwapEntity {
                        old_id,
                        new_id,
                        dispose_old,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setActiveCharacter",
            lua.create_function(
                move |_, (role, entity_id, dispose_old): (String, String, bool)| {
                    h.push_command(ScriptCommand::SetActiveCharacter {
                        role,
                        entity_id,
                        dispose_old,
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "triggerGameOver",
            lua.create_function(move |_, ()| {
                *h.game_over.lock() = true;
                h.push_command(ScriptCommand::TriggerGameOver);
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "isGameOver",
            lua.create_function(move |_, ()| Ok(*h.game_over.lock()))?,
        )?;
    }

    Ok(())
}
