//! Archivo: engine/crates/pulse_scripting/src/js_vm/bindings_world.rs
//! Propósito: Registra en el objeto global de QuickJS los bindings de registro (`log`),
//! sistema multicámara, manipulación/animación de entidades del escenario y modcharts (strums).

use super::JsVmError;
use crate::bridge::{ScriptCommand, ScriptHostState};
use rquickjs::{Ctx, Function, Object};
use std::sync::Arc;

pub(super) fn register_world_bindings<'js>(
    ctx: &Ctx<'js>,
    globals: &Object<'js>,
    host: &Arc<ScriptHostState>,
) -> Result<(), JsVmError> {
    macro_rules! bind_js {
        ($name:expr, $closure:expr) => {{
            let f = Function::new(ctx.clone(), $closure)
                .map_err(|e| JsVmError::Eval(e.to_string()))?;
            globals
                .set($name, f)
                .map_err(|e| JsVmError::Eval(e.to_string()))?;
        }};
    }

    {
        let h = Arc::clone(host);
        bind_js!("log", move |msg: String| {
            h.push_command(ScriptCommand::Log(msg));
        });
    }

    // --- Cámara ---
    {
        let h = Arc::clone(host);
        bind_js!("setCameraPos", move |x: f32, y: f32| {
            if x.is_finite() && y.is_finite() {
                *h.camera_x.lock() = x;
                *h.camera_y.lock() = y;
                h.push_command(ScriptCommand::SetCameraPos { x, y });
            }
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setCameraZoom", move |zoom: f32| {
            if zoom.is_finite() {
                *h.camera_zoom.lock() = zoom;
                h.push_command(ScriptCommand::SetCameraZoom(zoom));
            }
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setCameraBaseZoom", move |zoom: f32| {
            if zoom.is_finite() {
                *h.camera_base_zoom.lock() = zoom;
                h.push_command(ScriptCommand::SetCameraBaseZoom(zoom));
            }
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getCameraX", move || -> f32 { *h.camera_x.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getCameraY", move || -> f32 { *h.camera_y.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getCameraZoom", move || -> f32 { *h.camera_zoom.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getCameraBaseZoom", move || -> f32 { *h.camera_base_zoom.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("shakeCamera", move |intensity: f32| {
            h.push_command(ScriptCommand::ShakeCamera { intensity });
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!(
            "__nativeSwitchCamera",
            move |slot: i32, zoom: f32, dur: f32, ease: String| {
                let slot_idx = slot.max(0) as usize;
                *h.active_camera_slot.lock() = slot_idx;
                h.push_command(ScriptCommand::SwitchCameraSlot {
                    slot: slot_idx,
                    zoom: if zoom.is_finite() && zoom > 0.1 {
                        Some(zoom)
                    } else {
                        None
                    },
                    duration_sec: if dur.is_finite() { dur.max(0.0) } else { 0.0 },
                    ease: if ease.trim().is_empty() {
                        "sineOut".to_string()
                    } else {
                        ease
                    },
                });
            }
        );
        let _: rquickjs::Value = ctx
            .eval(
                "globalThis.switchCamera = function(slot, zoom, dur, ease) { __nativeSwitchCamera((slot|0), (typeof zoom==='number'&&isFinite(zoom))?zoom:-1, (typeof dur==='number'&&isFinite(dur))?dur:0, ease||'sineOut'); };",
            )
            .map_err(|e| JsVmError::Eval(e.to_string()))?;
    }
    {
        let h = Arc::clone(host);
        bind_js!("getActiveCamera", move || -> i32 {
            *h.active_camera_slot.lock() as i32
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setCameraFollowPos", move |x: f32, y: f32| {
            if x.is_finite() && y.is_finite() {
                h.push_command(ScriptCommand::SetCameraFollowLock { x, y });
            }
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("clearCameraFollowPos", move || {
            h.push_command(ScriptCommand::ClearCameraFollowLock);
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setCameraClamp", move |enabled: bool| {
            h.push_command(ScriptCommand::SetCameraClamp { enabled });
        });
    }

    // --- Entidades ---
    {
        let h = Arc::clone(host);
        bind_js!("getEntityX", move |id_or_role: String| -> Option<f32> {
            h.entity_positions.lock().get(&id_or_role).map(|p| p.0)
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getEntityY", move |id_or_role: String| -> Option<f32> {
            h.entity_positions.lock().get(&id_or_role).map(|p| p.1)
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getEntityProp", move |entity_id: String, prop: String| -> Option<f32> {
            h.entity_props
                .lock()
                .get(&entity_id)
                .and_then(|snap| snap.get(&prop))
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setEntityProp", move |entity_id: String, prop: String, value: f32| {
            if let Some(snap) = h.entity_props.lock().get_mut(&entity_id) {
                snap.set(&prop, value);
            }
            h.push_command(ScriptCommand::SetEntityProp {
                entity_id,
                prop,
                value,
            });
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("playAnim", move |entity_id: String, anim_name: String| {
            h.push_command(ScriptCommand::PlayEntityAnim {
                entity_id,
                anim_name,
            });
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("playEntityAnim", move |entity_id: String, anim_name: String| {
            h.push_command(ScriptCommand::PlayEntityAnim {
                entity_id,
                anim_name,
            });
        });
    }

    // --- Strums / Modcharts ---
    {
        let h = Arc::clone(host);
        bind_js!("setStrumProp", move |strum_idx: i32, prop: String, value: f32| {
            let idx = strum_idx.clamp(0, 7) as usize;
            h.strum_props.lock()[idx].set(&prop, value);
            h.push_command(ScriptCommand::SetStrumProp {
                strum_idx: idx,
                prop,
                value,
            });
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getStrumProp", move |strum_idx: i32, prop: String| -> f32 {
            let idx = strum_idx.clamp(0, 7) as usize;
            h.strum_props.lock()[idx].get(&prop).unwrap_or(0.0)
        });
    }

    Ok(())
}
