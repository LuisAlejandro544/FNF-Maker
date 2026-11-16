//! Archivo: engine/crates/pulse_scripting/src/js_vm.rs
//! Propósito: Máquina virtual JavaScript (QuickJS-NG vía `rquickjs`, licencia MIT)
//! que ejecuta tanto scripts `.js` puros como scripts `.ts` previamente transpilados por `oxc`.
//! Expone exactamente la misma API global que Lua para consistencia total.

use crate::bridge::{ScriptCommand, ScriptHostState};
use rquickjs::{Context, Function, Runtime};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JsVmError {
    #[error("Error creando runtime QuickJS: {0}")]
    Runtime(String),
    #[error("Error ejecutando script JS: {0}")]
    Eval(String),
}

pub struct JsScriptVm {
    _runtime: Runtime,
    context: Context,
}

impl JsScriptVm {
    pub fn new(js_source: &str, host: Arc<ScriptHostState>) -> Result<Self, JsVmError> {
        let runtime = Runtime::new().map_err(|e| JsVmError::Runtime(e.to_string()))?;
        let context = Context::full(&runtime).map_err(|e| JsVmError::Runtime(e.to_string()))?;

        context.with(|ctx| -> Result<(), JsVmError> {
            let globals = ctx.globals();

            {
                let h = Arc::clone(&host);
                let f = Function::new(ctx.clone(), move |msg: String| {
                    h.push_command(ScriptCommand::Log(msg));
                })
                .map_err(|e| JsVmError::Eval(e.to_string()))?;
                globals
                    .set("log", f)
                    .map_err(|e| JsVmError::Eval(e.to_string()))?;
            }

            {
                let h = Arc::clone(&host);
                let f = Function::new(ctx.clone(), move |zoom: f32| {
                    h.push_command(ScriptCommand::SetCameraZoom(zoom));
                })
                .map_err(|e| JsVmError::Eval(e.to_string()))?;
                globals
                    .set("setCameraZoom", f)
                    .map_err(|e| JsVmError::Eval(e.to_string()))?;
            }

            {
                let h = Arc::clone(&host);
                let f = Function::new(ctx.clone(), move |intensity: f32| {
                    h.push_command(ScriptCommand::ShakeCamera { intensity });
                })
                .map_err(|e| JsVmError::Eval(e.to_string()))?;
                globals
                    .set("shakeCamera", f)
                    .map_err(|e| JsVmError::Eval(e.to_string()))?;
            }

            {
                let h = Arc::clone(&host);
                let f = Function::new(
                    ctx.clone(),
                    move |entity_id: String, prop: String, value: f32| {
                        h.push_command(ScriptCommand::SetEntityProp {
                            entity_id,
                            prop,
                            value,
                        });
                    },
                )
                .map_err(|e| JsVmError::Eval(e.to_string()))?;
                globals
                    .set("setEntityProp", f)
                    .map_err(|e| JsVmError::Eval(e.to_string()))?;
            }

            {
                let h = Arc::clone(&host);
                let f = Function::new(ctx.clone(), move |entity_id: String, anim_name: String| {
                    h.push_command(ScriptCommand::PlayEntityAnim {
                        entity_id,
                        anim_name,
                    });
                })
                .map_err(|e| JsVmError::Eval(e.to_string()))?;
                globals
                    .set("playAnim", f)
                    .map_err(|e| JsVmError::Eval(e.to_string()))?;
            }

            {
                let h = Arc::clone(&host);
                let f = Function::new(ctx.clone(), move || -> f64 { *h.song_pos_ms.lock() })
                    .map_err(|e| JsVmError::Eval(e.to_string()))?;
                globals
                    .set("getSongPosMs", f)
                    .map_err(|e| JsVmError::Eval(e.to_string()))?;
            }

            ctx.eval::<(), _>(js_source)
                .map_err(|e| JsVmError::Eval(e.to_string()))?;

            Ok(())
        })?;

        Ok(Self {
            _runtime: runtime,
            context,
        })
    }

    pub fn call_void(&mut self, fn_name: &str) {
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>(fn_name) {
                let _ = func.call::<_, ()>(());
            }
        });
    }

    pub fn call_f32(&mut self, fn_name: &str, arg: f32) {
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>(fn_name) {
                let _ = func.call::<_, ()>((arg,));
            }
        });
    }

    pub fn call_i32(&mut self, fn_name: &str, arg: i32) {
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>(fn_name) {
                let _ = func.call::<_, ()>((arg,));
            }
        });
    }

    pub fn call_note_event(&mut self, fn_name: &str, lane: i32, is_player: bool) {
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>(fn_name) {
                let _ = func.call::<_, ()>((lane, is_player));
            }
        });
    }
}
