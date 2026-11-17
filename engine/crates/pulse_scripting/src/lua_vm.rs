//! Archivo: engine/crates/pulse_scripting/src/lua_vm.rs
//! Propósito: Máquina virtual Lua 5.4 (`mlua`) embebida y aislada (sandbox).
//! Registra funciones nativas del motor (`log`, `setCameraZoom`, `shakeCamera`,
//! `setEntityProp`, `playAnim`, `getSongPosMs`) para mods hechos en Lua.

use crate::bridge::{ScriptCommand, ScriptHostState};
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
            globals.set(
                "getCameraX",
                lua.create_function(move |_, ()| Ok(*h.camera_x.lock()))?,
            )?;
        }

        {
            let h = Arc::clone(&host);
            globals.set(
                "getCameraY",
                lua.create_function(move |_, ()| Ok(*h.camera_y.lock()))?,
            )?;
        }

        {
            let h = Arc::clone(&host);
            globals.set(
                "getCameraZoom",
                lua.create_function(move |_, ()| Ok(*h.camera_zoom.lock()))?,
            )?;
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
                "shakeCamera",
                lua.create_function(move |_, intensity: f32| {
                    h.push_command(ScriptCommand::ShakeCamera { intensity });
                    Ok(())
                })?,
            )?;
        }

        {
            let h = Arc::clone(&host);
            globals.set(
                "setEntityProp",
                lua.create_function(move |_, (entity_id, prop, value): (String, String, f32)| {
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

        {
            let h = Arc::clone(&host);
            globals.set(
                "getSongPosMs",
                lua.create_function(move |_, ()| Ok(*h.song_pos_ms.lock()))?,
            )?;
        }

        lua.load(source).exec()?;

        Ok(Self { lua })
    }

    pub fn call_void(&mut self, fn_name: &str) {
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            let _ = func.call::<()>(());
        }
    }

    pub fn call_f32(&mut self, fn_name: &str, arg: f32) {
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            let _ = func.call::<()>(arg);
        }
    }

    pub fn call_i32(&mut self, fn_name: &str, arg: i32) {
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            let _ = func.call::<()>(arg);
        }
    }

    pub fn call_note_event(&mut self, fn_name: &str, lane: i32, is_player: bool) {
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            let _ = func.call::<()>((lane, is_player));
        }
    }
}
