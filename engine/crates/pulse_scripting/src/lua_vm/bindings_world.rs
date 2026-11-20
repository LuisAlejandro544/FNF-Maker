//! Archivo: engine/crates/pulse_scripting/src/lua_vm/bindings_world.rs
//! Propósito: Registra en los globales de Lua 5.4 los bindings de registro (`log`),
//! cámara multicámara, propiedades/animación de entidades del escenario y strums (modcharts).

use crate::bridge::{ScriptCommand, ScriptHostState};
use mlua::{Lua, Result, Table};
use std::sync::Arc;

pub(super) fn register_world_bindings(
    lua: &Lua,
    globals: &Table,
    host: &Arc<ScriptHostState>,
) -> Result<()> {
    {
        let h = Arc::clone(host);
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
        let h = Arc::clone(host);
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
        let h = Arc::clone(host);
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
        let h = Arc::clone(host);
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
        let h = Arc::clone(host);
        globals.set("getCameraX", lua.create_function(move |_, ()| Ok(*h.camera_x.lock()))?)?;
    }
    {
        let h = Arc::clone(host);
        globals.set("getCameraY", lua.create_function(move |_, ()| Ok(*h.camera_y.lock()))?)?;
    }
    {
        let h = Arc::clone(host);
        globals.set("getCameraZoom", lua.create_function(move |_, ()| Ok(*h.camera_zoom.lock()))?)?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getCameraBaseZoom",
            lua.create_function(move |_, ()| Ok(*h.camera_base_zoom.lock()))?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "shakeCamera",
            lua.create_function(move |_, intensity: f32| {
                h.push_command(ScriptCommand::ShakeCamera { intensity });
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "switchCamera",
            lua.create_function(
                move |_, (slot, zoom, dur, ease): (i32, Option<f32>, Option<f32>, Option<String>)| {
                    let slot_idx = slot.max(0) as usize;
                    *h.active_camera_slot.lock() = slot_idx;
                    h.push_command(ScriptCommand::SwitchCameraSlot {
                        slot: slot_idx,
                        zoom: zoom.filter(|z| z.is_finite() && *z > 0.1),
                        duration_sec: dur.unwrap_or(0.0).max(0.0),
                        ease: ease.unwrap_or_else(|| "sineOut".to_string()),
                    });
                    Ok(())
                },
            )?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getActiveCamera",
            lua.create_function(move |_, ()| Ok(*h.active_camera_slot.lock() as i32))?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setCameraFollowPos",
            lua.create_function(move |_, (x, y): (f32, f32)| {
                if x.is_finite() && y.is_finite() {
                    h.push_command(ScriptCommand::SetCameraFollowLock { x, y });
                }
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "clearCameraFollowPos",
            lua.create_function(move |_, ()| {
                h.push_command(ScriptCommand::ClearCameraFollowLock);
                Ok(())
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "setCameraClamp",
            lua.create_function(move |_, enabled: bool| {
                h.push_command(ScriptCommand::SetCameraClamp { enabled });
                Ok(())
            })?,
        )?;
    }

    // --- Entidades ---
    {
        let h = Arc::clone(host);
        globals.set(
            "getEntityX",
            lua.create_function(move |_, id_or_role: String| {
                Ok(h.entity_positions.lock().get(&id_or_role).map(|p| p.0))
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
        globals.set(
            "getEntityY",
            lua.create_function(move |_, id_or_role: String| {
                Ok(h.entity_positions.lock().get(&id_or_role).map(|p| p.1))
            })?,
        )?;
    }
    {
        let h = Arc::clone(host);
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
        let h = Arc::clone(host);
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
        let h = Arc::clone(host);
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
        let h = Arc::clone(host);
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
        let h = Arc::clone(host);
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
        let h = Arc::clone(host);
        globals.set(
            "getStrumProp",
            lua.create_function(move |_, (strum_idx, prop): (i32, String)| {
                let idx = strum_idx.clamp(0, 7) as usize;
                Ok(h.strum_props.lock()[idx].get(&prop).unwrap_or(0.0))
            })?,
        )?;
    }

    Ok(())
}
