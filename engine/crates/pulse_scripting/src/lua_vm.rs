//! Archivo: engine/crates/pulse_scripting/src/lua_vm.rs
//! Propósito: Máquina virtual Lua 5.4 (`mlua`) embebida para mods avanzados.
//! Expone control total de cámara, entidades, strums (modcharts), HUD,
//! gráficos/textos dinámicos, tweens, timers, pistas de audio y reporte de errores.

use crate::bridge::{ScriptCommand, ScriptHostState, parse_color_argb};
use mlua::{Function, HookTriggers, Lua, VmState};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;
use thiserror::Error;

const LUA_INIT_BUDGET_MS: u64 = 250;
const LUA_FRAME_BUDGET_MS: u64 = 25;
const LUA_MAX_WATCHDOG_TRIPS: u32 = 3;

#[derive(Debug, Error)]
pub enum LuaVmError {
    #[error("Error Lua: {0}")]
    Lua(#[from] mlua::Error),
}

pub struct LuaScriptVm {
    lua: Lua,
    host: Arc<ScriptHostState>,
    script_name: String,
    watchdog_origin: Instant,
    watchdog_deadline_us: Arc<AtomicU64>,
    watchdog_tripped: Arc<AtomicBool>,
    watchdog_trips: u32,
    disabled_by_watchdog: bool,
}

impl LuaScriptVm {
    /// Precompila código fuente Lua 5.4 a bytecode binario portable (manteniendo líneas para errores).
    pub fn precompile_bytecode(source: &str) -> Result<Vec<u8>, LuaVmError> {
        let lua = Lua::new();
        let func = lua.load(source).into_function()?;
        Ok(func.dump(false))
    }

    pub fn new(source: &str, host: Arc<ScriptHostState>) -> Result<Self, LuaVmError> {
        Self::new_with_bytecode("script.lua", source, None, host)
    }

    pub fn new_with_bytecode(
        script_name: &str,
        source: &str,
        precompiled: Option<&[u8]>,
        host: Arc<ScriptHostState>,
    ) -> Result<Self, LuaVmError> {
        let lua = Lua::new();
        let watchdog_origin = Instant::now();
        let watchdog_deadline_us = Arc::new(AtomicU64::new(0));
        let watchdog_tripped = Arc::new(AtomicBool::new(false));

        {
            let origin = watchdog_origin;
            let deadline_ref = Arc::clone(&watchdog_deadline_us);
            let tripped_ref = Arc::clone(&watchdog_tripped);
            let triggers = HookTriggers::new().every_nth_instruction(4096);
            lua.set_hook(triggers, move |_lua, _debug| -> mlua::Result<VmState> {
                let limit_us = deadline_ref.load(Ordering::Relaxed);
                if limit_us > 0 {
                    let now_us = origin.elapsed().as_micros() as u64;
                    if now_us > limit_us {
                        tripped_ref.store(true, Ordering::Relaxed);
                        return Err(mlua::Error::RuntimeError(
                            "Watchdog Lua: tiempo de ejecución excedido (bucle pesado/infinito)"
                                .to_string(),
                        ));
                    }
                }
                Ok(VmState::Continue)
            })?;
        }

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
            let h = Arc::clone(&host);
            globals.set(
                "getHudVisible",
                lua.create_function(move |_, ()| Ok(*h.hud_visible.lock()))?,
            )?;
        }
        {
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
            globals.set(
                "getHudOpacity",
                lua.create_function(move |_, ()| Ok(*h.hud_opacity.lock()))?,
            )?;
        }
        {
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
            globals.set(
                "getHudZoom",
                lua.create_function(move |_, ()| Ok(*h.hud_zoom.lock()))?,
            )?;
        }
        {
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
            globals.set(
                "getArrowsVisible",
                lua.create_function(move |_, ()| Ok(*h.arrows_visible.lock()))?,
            )?;
        }
        {
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
            globals.set(
                "getArrowsOpacity",
                lua.create_function(move |_, ()| Ok(*h.arrows_opacity.lock()))?,
            )?;
        }
        {
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
            globals.set(
                "getScreenBlackout",
                lua.create_function(move |_, ()| Ok(*h.screen_blackout_alpha.lock()))?,
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
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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

        // --- Flashes, Precarga IO, Swaps de Escenario y Personajes Activos ---
        {
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
            globals.set(
                "preloadEntity",
                lua.create_function(move |_, entity_id: String| {
                    h.push_command(ScriptCommand::PreloadEntity(entity_id));
                    Ok(())
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
            globals.set(
                "retireEntity",
                lua.create_function(move |_, entity_id: String| {
                    h.push_command(ScriptCommand::RetireEntity(entity_id));
                    Ok(())
                })?,
            )?;
        }
        {
            let h = Arc::clone(&host);
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
            let h = Arc::clone(&host);
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

        Self::arm_deadline(
            &watchdog_origin,
            &watchdog_deadline_us,
            &watchdog_tripped,
            LUA_INIT_BUDGET_MS,
        );
        let load_res = if let Some(bytecode) = precompiled {
            match lua.load(bytecode).exec() {
                Ok(()) => Ok(()),
                Err(_) if !watchdog_tripped.load(Ordering::Relaxed) => lua.load(source).exec(),
                Err(e) => Err(e),
            }
        } else {
            lua.load(source).exec()
        };
        watchdog_deadline_us.store(0, Ordering::Relaxed);
        load_res?;

        Ok(Self {
            lua,
            host,
            script_name: script_name.to_string(),
            watchdog_origin,
            watchdog_deadline_us,
            watchdog_tripped,
            watchdog_trips: 0,
            disabled_by_watchdog: false,
        })
    }

    fn arm_deadline(
        origin: &Instant,
        deadline: &AtomicU64,
        tripped: &AtomicBool,
        budget_ms: u64,
    ) {
        tripped.store(false, Ordering::Relaxed);
        let now_us = origin.elapsed().as_micros() as u64;
        deadline.store(now_us.saturating_add(budget_ms * 1000), Ordering::Relaxed);
    }

    #[inline]
    fn begin_frame_watchdog(&self) {
        Self::arm_deadline(
            &self.watchdog_origin,
            &self.watchdog_deadline_us,
            &self.watchdog_tripped,
            LUA_FRAME_BUDGET_MS,
        );
    }

    #[inline]
    fn end_frame_watchdog(&self) {
        self.watchdog_deadline_us.store(0, Ordering::Relaxed);
    }

    fn handle_call_error(&mut self, fn_name: &str, err: mlua::Error) {
        if self.watchdog_tripped.swap(false, Ordering::Relaxed) {
            self.watchdog_trips += 1;
            if self.watchdog_trips >= LUA_MAX_WATCHDOG_TRIPS {
                self.disabled_by_watchdog = true;
                self.host.push_runtime_error(format!(
                    "Watchdog Lua [{}]: script puesto en cuarentena tras {} excesos en {}",
                    self.script_name, self.watchdog_trips, fn_name
                ));
            } else {
                self.host.push_runtime_error(format!(
                    "Watchdog Lua [{}] ({}): interrumpido por superar {}ms",
                    self.script_name, fn_name, LUA_FRAME_BUDGET_MS
                ));
            }
        } else {
            self.host
                .push_runtime_error(format!("Lua [{}] {}: {}", self.script_name, fn_name, err));
        }
    }

    pub fn is_quarantined(&self) -> bool {
        self.disabled_by_watchdog
    }

    pub fn call_void(&mut self, fn_name: &str) {
        if self.disabled_by_watchdog {
            return;
        }
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            self.begin_frame_watchdog();
            let res = func.call::<()>(());
            self.end_frame_watchdog();
            if let Err(e) = res {
                self.handle_call_error(fn_name, e);
            }
        }
    }

    pub fn call_f32(&mut self, fn_name: &str, arg: f32) {
        if self.disabled_by_watchdog {
            return;
        }
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            self.begin_frame_watchdog();
            let res = func.call::<()>(arg);
            self.end_frame_watchdog();
            if let Err(e) = res {
                self.handle_call_error(fn_name, e);
            }
        }
    }

    pub fn call_i32(&mut self, fn_name: &str, arg: i32) {
        if self.disabled_by_watchdog {
            return;
        }
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            self.begin_frame_watchdog();
            let res = func.call::<()>(arg);
            self.end_frame_watchdog();
            if let Err(e) = res {
                self.handle_call_error(fn_name, e);
            }
        }
    }

    pub fn call_str(&mut self, fn_name: &str, arg: &str) {
        if self.disabled_by_watchdog {
            return;
        }
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            self.begin_frame_watchdog();
            let res = func.call::<()>(arg);
            self.end_frame_watchdog();
            if let Err(e) = res {
                self.handle_call_error(fn_name, e);
            }
        }
    }

    pub fn call_note_event(&mut self, fn_name: &str, lane: i32, is_player: bool) {
        if self.disabled_by_watchdog {
            return;
        }
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            self.begin_frame_watchdog();
            let res = func.call::<()>((lane, is_player));
            self.end_frame_watchdog();
            if let Err(e) = res {
                self.handle_call_error(fn_name, e);
            }
        }
    }

    pub fn call_chart_event(&mut self, name: &str, val1: &str, val2: &str) {
        if self.disabled_by_watchdog {
            return;
        }
        if let Ok(func) = self.lua.globals().get::<Function>("onEvent") {
            self.begin_frame_watchdog();
            let res = func.call::<()>((name, val1, val2));
            self.end_frame_watchdog();
            if let Err(e) = res {
                self.handle_call_error("onEvent", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua_bytecode_precompile_loads_and_runs_cleanly() {
        let src = r#"
            function onBeatHit(beat)
                setCameraZoom(1.0 + beat * 0.01)
            end
        "#;
        let bytecode = LuaScriptVm::precompile_bytecode(src).expect("precompilación Lua");
        assert!(!bytecode.is_empty());

        let host = Arc::new(ScriptHostState::default());
        let mut vm = LuaScriptVm::new_with_bytecode("test.lua", src, Some(&bytecode), Arc::clone(&host))
            .expect("carga de bytecode");
        vm.call_i32("onBeatHit", 4);
        assert!((*host.camera_zoom.lock() - 1.04).abs() < 0.001);
    }

    #[test]
    fn lua_watchdog_interrupts_infinite_loop_and_quarantines_after_three_trips() {
        let src = r#"
            function onUpdate(dt)
                while true do
                end
            end
        "#;
        let host = Arc::new(ScriptHostState::default());
        let mut vm = LuaScriptVm::new_with_bytecode("hang.lua", src, None, Arc::clone(&host))
            .expect("carga inicial válida");

        for _ in 0..3 {
            vm.call_f32("onUpdate", 0.016);
        }
        assert!(vm.is_quarantined());
        let errs = host.drain_runtime_errors();
        assert!(errs.iter().any(|e| e.contains("cuarentena")));
    }
}
