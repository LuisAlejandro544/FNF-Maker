//! Archivo: engine/crates/pulse_scripting/src/js_vm/worker.rs
//! Propósito: Define `JsWorkerHandle` (`Send`), encapsulando todas las instancias de
//! `JsScriptVm` (`!Send`) dentro de un único hilo dedicado de QuickJS (`pulse_quickjs_worker`)
//! para evitar cruzar `JSRuntime`/`JSContext` entre hilos de Android JNI.

use super::JsScriptVm;
use crate::bridge::ScriptHostState;
use std::sync::Arc;

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
    CallStr {
        fn_name: &'static str,
        arg: String,
        ack: std::sync::mpsc::SyncSender<()>,
    },
    CallNoteEvent {
        fn_name: &'static str,
        lane: i32,
        is_player: bool,
        note_type: String,
        ack: std::sync::mpsc::SyncSender<()>,
    },
    CallChartEvent {
        name: String,
        val1: String,
        val2: String,
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
                    match JsScriptVm::new_named(&name, &js_source, Arc::clone(&host)) {
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
                        JsWorkerCmd::CallStr { fn_name, arg, ack } => {
                            for vm in &mut vms {
                                vm.call_str(fn_name, &arg);
                            }
                            let _ = ack.send(());
                        }
                        JsWorkerCmd::CallNoteEvent {
                            fn_name,
                            lane,
                            is_player,
                            note_type,
                            ack,
                        } => {
                            for vm in &mut vms {
                                vm.call_note_event(fn_name, lane, is_player, &note_type);
                            }
                            let _ = ack.send(());
                        }
                        JsWorkerCmd::CallChartEvent {
                            name,
                            val1,
                            val2,
                            ack,
                        } => {
                            for vm in &mut vms {
                                vm.call_chart_event(&name, &val1, &val2);
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

    pub fn call_str(&self, fn_name: &'static str, arg: String) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
            if tx
                .send(JsWorkerCmd::CallStr {
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

    pub fn call_note_event(
        &self,
        fn_name: &'static str,
        lane: i32,
        is_player: bool,
        note_type: String,
    ) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
            if tx
                .send(JsWorkerCmd::CallNoteEvent {
                    fn_name,
                    lane,
                    is_player,
                    note_type,
                    ack: ack_tx,
                })
                .is_ok()
            {
                let _ = ack_rx.recv();
            }
        }
    }

    pub fn call_chart_event(&self, name: String, val1: String, val2: String) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
            if tx
                .send(JsWorkerCmd::CallChartEvent {
                    name,
                    val1,
                    val2,
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
