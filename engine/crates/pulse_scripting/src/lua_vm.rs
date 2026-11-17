//! Archivo: engine/crates/pulse_scripting/src/lua_vm.rs
//! Propósito: Máquina virtual Lua 5.4 (`mlua`) embebida para mods avanzados.
//! Expone control total de cámara, entidades, strums (modcharts), HUD,
//! gráficos/textos dinámicos, tweens, timers, pistas de audio y reporte de errores.

use crate::bridge::{ScriptCommand, ScriptHostState, parse_color_argb};
use mlua::{Function, Lua};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LuaVmError {
    #[error("Error Lua: {0}")]
    Lua(#[from] mlua::Error),
}

pub struct LuaScriptVm {
    lua: Lua,
    host: Arc<ScriptHostState>,
}

impl LuaScriptVm {
    pub fn new(source: &str, host: Arc<ScriptHostState>) -> Result<Self, LuaVmError> {
        let lua = Lua::new();
        let globals = lua.globals();

        {
            let h = Arc::clone(&host);
            globals.set(
                "log",
                lua.create_function(move |_, msg: String| {
                    h.push_command(ScriptCommand::Log(msg));
                    Ok(())
                })?,
            )?;
        }

        // --- Cámara ---
        {
            let h = Arc::clone(&host);
            globals.set(
                "setCameraPos",
                lua.create_function(move |_, (x, y): (f32, f32)| {
                    if x.is_finite() && y.is_finite() {
                        *h.camera_x.lock() = x;
                        *h.camera_y.lock() = y;
                        h.push_command(ScriptCommand::SetCameraPos { x, y });
                    }
                    Ok(())
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "setCameraZoom",
                lua.create_function(move |_, zoom: f32| {
                    if zoom.is_finite() {
                        *h.camera_zoom.lock() = zoom;
                        h.push_command(ScriptCommand::SetCameraZoom(zoom));
                    }
                    Ok(())
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "setCameraBaseZoom",
                lua.create_function(move |_, zoom: f32| {
                    if zoom.is_finite() {
                        *h.camera_base_zoom.lock() = zoom;
                        h.push_command(ScriptCommand::SetCameraBaseZoom(zoom));
                    }
                    Ok(())
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set("getCameraX", lua.create_function(move |_, ()| Ok(*h.camera_x.lock()))?)?;
        }
        {
            let h = Arc::clone(&host);
            globals.set("getCameraY", lua.create_function(move |_, ()| Ok(*h.camera_y.lock()))?)?;
        }
        {
            let h = Arc::clone(&host);
            globals.set("getCameraZoom", lua.create_function(move |_, ()| Ok(*h.camera_zoom.lock()))?)?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "getCameraBaseZoom",
                lua.create_function(move |_, ()| Ok(*h.camera_base_zoom.lock()))?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "shakeCamera",
                lua.create_function(move |_, intensity: f32| {
                    h.push_command(ScriptCommand::ShakeCamera { intensity });
                    Ok(())
                })?,
            )?;
        }

        // --- Entidades ---
        {
            let h = Arc::clone(&host);
            globals.set(
                "getEntityX",
                lua.create_function(move |_, id_or_role: String| {
                    Ok(h.entity_positions.lock().get(&id_or_role).map(|p| p.0))
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "getEntityY",
                lua.create_function(move |_, id_or_role: String| {
                    Ok(h.entity_positions.lock().get(&id_or_role).map(|p| p.1))
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "getEntityProp",
                lua.create_function(move |_, (entity_id, prop): (String, String)| {
                    Ok(h.entity_props
                        .lock()
                        .get(&entity_id)
                        .and_then(|snap| snap.get(&prop)))
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "setEntityProp",
                lua.create_function(move |_, (entity_id, prop, value): (String, String, f32)| {
                    if let Some(snap) = h.entity_props.lock().get_mut(&entity_id) {
                        snap.set(&prop, value);
                    }
                    h.push_command(ScriptCommand::SetEntityProp {
                        entity_id,
                        prop,
                        value,
                    });
                    Ok(())
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "playAnim",
                lua.create_function(move |_, (entity_id, anim_name): (String, String)| {
                    h.push_command(ScriptCommand::PlayEntityAnim {
                        entity_id,
                        anim_name,
                    });
                    Ok(())
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "playEntityAnim",
                lua.create_function(move |_, (entity_id, anim_name): (String, String)| {
                    h.push_command(ScriptCommand::PlayEntityAnim {
                        entity_id,
                        anim_name,
                    });
                    Ok(())
                })?,
            )?;
        }

        // --- Strums / Modcharts (0..3 Rival, 4..7 Jugador) ---
        {
            let h = Arc::clone(&host);
            globals.set(
                "setStrumProp",
                lua.create_function(move |_, (strum_idx, prop, value): (i32, String, f32)| {
                    let idx = strum_idx.clamp(0, 7) as usize;
                    h.strum_props.lock()[idx].set(&prop, value);
                    h.push_command(ScriptCommand::SetStrumProp {
                        strum_idx: idx,
                        prop,
                        value,
                    });
                    Ok(())
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "getStrumProp",
                lua.create_function(move |_, (strum_idx, prop): (i32, String)| {
                    let idx = strum_idx.clamp(0, 7) as usize;
                    Ok(h.strum_props.lock()[idx].get(&prop).unwrap_or(0.0))
                })?,
            )?;
        }

        // --- Estado del Chart / Gameplay / HUD ---
        {
            let h = Arc::clone(&host);
            globals.set(
                "getMustHitSection",
                lua.create_function(move |_, ()| Ok(*h.must_hit_section.lock()))?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "getActiveBpm",
                lua.create_function(move |_, ()| Ok(*h.current_bpm.lock()))?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "getSongPosMs",
                lua.create_function(move |_, ()| Ok(*h.song_pos_ms.lock()))?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set("getHealth", lua.create_function(move |_, ()| Ok(*h.health.lock()))?)?;
        }
        {
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
            globals.set(
                "getScore",
                lua.create_function(move |_, ()| Ok(*h.score.lock() as f64))?,
            )?;
        }
        {
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
            globals.set("getCombo", lua.create_function(move |_, ()| Ok(*h.combo.lock() as i32))?)?;
        }
        {
            let h = Arc::clone(&host);
            globals.set("getMisses", lua.create_function(move |_, ()| Ok(*h.misses.lock() as i32))?)?;
        }
        {
            let h = Arc::clone(&host);
            globals.set("getHits", lua.create_function(move |_, ()| Ok(*h.hits.lock() as i32))?)?;
        }
        {
            let h = Arc::clone(&host);
            globals.set("getAccuracy", lua.create_function(move |_, ()| Ok(*h.accuracy.lock()))?)?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "getScrollSpeed",
                lua.create_function(move |_, ()| Ok(*h.scroll_speed.lock()))?,
            )?;
        }
        {
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
            globals.set(
                "getDownscroll",
                lua.create_function(move |_, ()| Ok(*h.downscroll.lock()))?,
            )?;
        }
        {
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
            globals.set(
                "setHudVisible",
                lua.create_function(move |_, visible: bool| {
                    h.push_command(ScriptCommand::SetHudProp {
                        prop: "visible".to_string(),
                        value: if visible { 1.0 } else { 0.0 },
                    });
                    Ok(())
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "setHudOpacity",
                lua.create_function(move |_, alpha: f32| {
                    h.push_command(ScriptCommand::SetHudProp {
                        prop: "opacity".to_string(),
                        value: alpha,
                    });
                    Ok(())
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "setHudZoom",
                lua.create_function(move |_, zoom: f32| {
                    h.push_command(ScriptCommand::SetHudProp {
                        prop: "zoom".to_string(),
                        value: zoom,
                    });
                    Ok(())
                })?,
            )?;
        }

        // --- Gráficos, Textos y Clonación dinámica ---
        {
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
            globals.set(
                "setTextString",
                lua.create_function(move |_, (id, text): (String, String)| {
                    h.push_command(ScriptCommand::SetTextString { id, text });
                    Ok(())
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
            globals.set(
                "runTimer",
                lua.create_function(move |_, (tag, duration_sec): (String, f32)| {
                    h.push_command(ScriptCommand::RunTimer { tag, duration_sec });
                    Ok(())
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "cancelTimer",
                lua.create_function(move |_, tag: String| {
                    h.push_command(ScriptCommand::CancelTimer(tag));
                    Ok(())
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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

        lua.load(source).exec()?;

        Ok(Self { lua, host })
    }

    pub fn call_void(&mut self, fn_name: &str) {
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            if let Err(e) = func.call::<()>(()) {
                self.host.push_runtime_error(format!("Lua {}: {}", fn_name, e));
            }
        }
    }

    pub fn call_f32(&mut self, fn_name: &str, arg: f32) {
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            if let Err(e) = func.call::<()>(arg) {
                self.host.push_runtime_error(format!("Lua {}: {}", fn_name, e));
            }
        }
    }

    pub fn call_i32(&mut self, fn_name: &str, arg: i32) {
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            if let Err(e) = func.call::<()>(arg) {
                self.host.push_runtime_error(format!("Lua {}: {}", fn_name, e));
            }
        }
    }

    pub fn call_str(&mut self, fn_name: &str, arg: &str) {
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            if let Err(e) = func.call::<()>(arg) {
                self.host.push_runtime_error(format!("Lua {}: {}", fn_name, e));
            }
        }
    }

    pub fn call_note_event(&mut self, fn_name: &str, lane: i32, is_player: bool) {
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            if let Err(e) = func.call::<()>((lane, is_player)) {
                self.host.push_runtime_error(format!("Lua {}: {}", fn_name, e));
            }
        }
    }

    pub fn call_chart_event(&mut self, name: &str, val1: &str, val2: &str) {
        if let Ok(func) = self.lua.globals().get::<Function>("onEvent") {
            if let Err(e) = func.call::<()>((name, val1, val2)) {
                self.host.push_runtime_error(format!("Lua onEvent: {}", e));
            }
        }
    }
}
