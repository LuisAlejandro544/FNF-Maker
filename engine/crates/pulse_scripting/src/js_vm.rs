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

enum JsWorkerCmd {
    CallVoid {
        fn_name: &'static str,
        ack: std::sync::mpsc::SyncSender<()>,
    },
    CallF32 {
        fn_name: &'static str,
        arg: f32,
        ack: std::sync::mpsc::SyncSender<()>,
    },
    CallI32 {
        fn_name: &'static str,
        arg: i32,
        ack: std::sync::mpsc::SyncSender<()>,
    },
    CallNoteEvent {
        fn_name: &'static str,
        lane: i32,
        is_player: bool,
        ack: std::sync::mpsc::SyncSender<()>,
    },
}

/// Manejador `Send` que encapsula todas las instancias `JsScriptVm` (!Send) dentro de un único
/// hilo dedicado de QuickJS, evitando cruzar `JSRuntime`/`JSContext` entre hilos de Android JNI.
pub struct JsWorkerHandle {
    tx: Option<std::sync::mpsc::Sender<JsWorkerCmd>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl JsWorkerHandle {
    pub fn spawn(
        scripts: Vec<(String, String)>,
        host: Arc<ScriptHostState>,
    ) -> (Self, Vec<String>) {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<JsWorkerCmd>();
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<Vec<String>>(1);

        let handle = std::thread::Builder::new()
            .name("pulse_quickjs_worker".to_string())
            .spawn(move || {
                let mut vms = Vec::new();
                let mut errors = Vec::new();

                for (name, js_source) in scripts {
                    match JsScriptVm::new(&js_source, Arc::clone(&host)) {
                        Ok(vm) => vms.push(vm),
                        Err(e) => errors.push(format!("[{}] {}", name, e)),
                    }
                }

                let _ = init_tx.send(errors);

                while let Ok(cmd) = cmd_rx.recv() {
                    match cmd {
                        JsWorkerCmd::CallVoid { fn_name, ack } => {
                            for vm in &mut vms {
                                vm.call_void(fn_name);
                            }
                            let _ = ack.send(());
                        }
                        JsWorkerCmd::CallF32 { fn_name, arg, ack } => {
                            for vm in &mut vms {
                                vm.call_f32(fn_name, arg);
                            }
                            let _ = ack.send(());
                        }
                        JsWorkerCmd::CallI32 { fn_name, arg, ack } => {
                            for vm in &mut vms {
                                vm.call_i32(fn_name, arg);
                            }
                            let _ = ack.send(());
                        }
                        JsWorkerCmd::CallNoteEvent {
                            fn_name,
                            lane,
                            is_player,
                            ack,
                        } => {
                            for vm in &mut vms {
                                vm.call_note_event(fn_name, lane, is_player);
                            }
                            let _ = ack.send(());
                        }
                    }
                }
            });

        match handle {
            Ok(join_handle) => {
                let init_errors = init_rx.recv().unwrap_or_default();
                (
                    Self {
                        tx: Some(cmd_tx),
                        thread: Some(join_handle),
                    },
                    init_errors,
                )
            }
            Err(e) => (
                Self {
                    tx: None,
                    thread: None,
                },
                vec![format!("No se pudo iniciar hilo QuickJS: {}", e)],
            ),
        }
    }

    pub fn call_void(&self, fn_name: &'static str) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
            if tx.send(JsWorkerCmd::CallVoid { fn_name, ack: ack_tx }).is_ok() {
                let _ = ack_rx.recv();
            }
        }
    }

    pub fn call_f32(&self, fn_name: &'static str, arg: f32) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
            if tx
                .send(JsWorkerCmd::CallF32 {
                    fn_name,
                    arg,
                    ack: ack_tx,
                })
                .is_ok()
            {
                let _ = ack_rx.recv();
            }
        }
    }

    pub fn call_i32(&self, fn_name: &'static str, arg: i32) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
            if tx
                .send(JsWorkerCmd::CallI32 {
                    fn_name,
                    arg,
                    ack: ack_tx,
                })
                .is_ok()
            {
                let _ = ack_rx.recv();
            }
        }
    }

    pub fn call_note_event(&self, fn_name: &'static str, lane: i32, is_player: bool) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
            if tx
                .send(JsWorkerCmd::CallNoteEvent {
                    fn_name,
                    lane,
                    is_player,
                    ack: ack_tx,
                })
                .is_ok()
            {
                let _ = ack_rx.recv();
            }
        }
    }
}

impl Drop for JsWorkerHandle {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(join) = self.thread.take() {
            let _ = join.join();
        }
    }
}

