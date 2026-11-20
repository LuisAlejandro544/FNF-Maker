//! Archivo: engine/crates/pulse_scripting/src/js_vm/mod.rs
//! Propósito: Punto de entrada de la máquina virtual JavaScript (QuickJS-NG vía `rquickjs`).
//! Coordina el ciclo de vida, presupuesto watchdog contra bucles infinitos, invocación de
//! callbacks del mod y montaje de los módulos de bindings y worker dedicado.

mod bindings_hud_fx;
mod bindings_world;
mod worker;

#[cfg(test)]
mod tests;

pub use worker::JsWorkerHandle;

use crate::bridge::ScriptHostState;
use rquickjs::{Context, Function, Runtime};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;
use thiserror::Error;

const JS_INIT_BUDGET_MS: u64 = 250;
const JS_FRAME_BUDGET_MS: u64 = 25;
const JS_MAX_WATCHDOG_TRIPS: u32 = 3;

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
    host: Arc<ScriptHostState>,
    script_name: String,
    watchdog_origin: Instant,
    watchdog_deadline_us: Arc<AtomicU64>,
    watchdog_tripped: Arc<AtomicBool>,
    watchdog_trips: u32,
    disabled_by_watchdog: bool,
}

impl JsScriptVm {
    pub fn new(js_source: &str, host: Arc<ScriptHostState>) -> Result<Self, JsVmError> {
        Self::new_named("script.js", js_source, host)
    }

    pub fn new_named(
        script_name: &str,
        js_source: &str,
        host: Arc<ScriptHostState>,
    ) -> Result<Self, JsVmError> {
        let runtime = Runtime::new().map_err(|e| JsVmError::Runtime(e.to_string()))?;
        let watchdog_origin = Instant::now();
        let watchdog_deadline_us = Arc::new(AtomicU64::new(0));
        let watchdog_tripped = Arc::new(AtomicBool::new(false));

        {
            let origin = watchdog_origin;
            let deadline_ref = Arc::clone(&watchdog_deadline_us);
            let tripped_ref = Arc::clone(&watchdog_tripped);
            runtime.set_interrupt_handler(Some(Box::new(move || -> bool {
                let limit_us = deadline_ref.load(Ordering::Relaxed);
                if limit_us == 0 {
                    return false;
                }
                let elapsed = origin.elapsed().as_micros() as u64;
                if elapsed > limit_us {
                    tripped_ref.store(true, Ordering::Relaxed);
                    return true;
                }
                false
            })));
        }

        let context = Context::full(&runtime).map_err(|e| JsVmError::Runtime(e.to_string()))?;

        context.with(|ctx| -> Result<(), JsVmError> {
            let globals = ctx.globals();
            bindings_world::register_world_bindings(&ctx, &globals, &host)?;
            bindings_hud_fx::register_hud_fx_bindings(&ctx, &globals, &host)?;

            Self::arm_deadline(
                &watchdog_origin,
                &watchdog_deadline_us,
                &watchdog_tripped,
                JS_INIT_BUDGET_MS,
            );
            let eval_res = ctx.eval::<(), _>(js_source);
            watchdog_deadline_us.store(0, Ordering::Relaxed);
            if let Err(e) = eval_res {
                if watchdog_tripped.load(Ordering::Relaxed) {
                    return Err(JsVmError::Eval(format!(
                        "Watchdog QuickJS: inicialización excedió {}ms y fue abortada",
                        JS_INIT_BUDGET_MS
                    )));
                }
                return Err(JsVmError::Eval(e.to_string()));
            }

            Ok(())
        })?;

        Ok(Self {
            _runtime: runtime,
            context,
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

    fn handle_js_error(&mut self, fn_name: &str, err_msg: String) {
        if self.watchdog_tripped.swap(false, Ordering::Relaxed) {
            self.watchdog_trips += 1;
            if self.watchdog_trips >= JS_MAX_WATCHDOG_TRIPS {
                self.disabled_by_watchdog = true;
                self.host.push_runtime_error(format!(
                    "Watchdog JS/TS [{}]: script en cuarentena tras {} excesos en {}",
                    self.script_name, self.watchdog_trips, fn_name
                ));
            } else {
                self.host.push_runtime_error(format!(
                    "Watchdog JS/TS [{}] ({}): interrumpido por superar {}ms",
                    self.script_name, fn_name, JS_FRAME_BUDGET_MS
                ));
            }
        } else {
            self.host
                .push_runtime_error(format!("JS [{}] {}: {}", self.script_name, fn_name, err_msg));
        }
    }

    pub fn is_quarantined(&self) -> bool {
        self.disabled_by_watchdog
    }

    pub fn call_void(&mut self, fn_name: &str) {
        if self.disabled_by_watchdog {
            return;
        }
        let mut call_err: Option<String> = None;
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>(fn_name) {
                Self::arm_deadline(
                    &self.watchdog_origin,
                    &self.watchdog_deadline_us,
                    &self.watchdog_tripped,
                    JS_FRAME_BUDGET_MS,
                );
                let res = func.call::<_, ()>(());
                self.watchdog_deadline_us.store(0, Ordering::Relaxed);
                if let Err(e) = res {
                    call_err = Some(e.to_string());
                }
            }
        });
        if let Some(err) = call_err {
            self.handle_js_error(fn_name, err);
        }
    }

    pub fn call_f32(&mut self, fn_name: &str, arg: f32) {
        if self.disabled_by_watchdog {
            return;
        }
        let mut call_err: Option<String> = None;
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>(fn_name) {
                Self::arm_deadline(
                    &self.watchdog_origin,
                    &self.watchdog_deadline_us,
                    &self.watchdog_tripped,
                    JS_FRAME_BUDGET_MS,
                );
                let res = func.call::<_, ()>((arg,));
                self.watchdog_deadline_us.store(0, Ordering::Relaxed);
                if let Err(e) = res {
                    call_err = Some(e.to_string());
                }
            }
        });
        if let Some(err) = call_err {
            self.handle_js_error(fn_name, err);
        }
    }

    pub fn call_i32(&mut self, fn_name: &str, arg: i32) {
        if self.disabled_by_watchdog {
            return;
        }
        let mut call_err: Option<String> = None;
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>(fn_name) {
                Self::arm_deadline(
                    &self.watchdog_origin,
                    &self.watchdog_deadline_us,
                    &self.watchdog_tripped,
                    JS_FRAME_BUDGET_MS,
                );
                let res = func.call::<_, ()>((arg,));
                self.watchdog_deadline_us.store(0, Ordering::Relaxed);
                if let Err(e) = res {
                    call_err = Some(e.to_string());
                }
            }
        });
        if let Some(err) = call_err {
            self.handle_js_error(fn_name, err);
        }
    }

    pub fn call_str(&mut self, fn_name: &str, arg: &str) {
        if self.disabled_by_watchdog {
            return;
        }
        let mut call_err: Option<String> = None;
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>(fn_name) {
                Self::arm_deadline(
                    &self.watchdog_origin,
                    &self.watchdog_deadline_us,
                    &self.watchdog_tripped,
                    JS_FRAME_BUDGET_MS,
                );
                let res = func.call::<_, ()>((arg,));
                self.watchdog_deadline_us.store(0, Ordering::Relaxed);
                if let Err(e) = res {
                    call_err = Some(e.to_string());
                }
            }
        });
        if let Some(err) = call_err {
            self.handle_js_error(fn_name, err);
        }
    }

    pub fn call_note_event(&mut self, fn_name: &str, lane: i32, is_player: bool, note_type: &str) {
        if self.disabled_by_watchdog {
            return;
        }
        let mut call_err: Option<String> = None;
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>(fn_name) {
                Self::arm_deadline(
                    &self.watchdog_origin,
                    &self.watchdog_deadline_us,
                    &self.watchdog_tripped,
                    JS_FRAME_BUDGET_MS,
                );
                let res = func.call::<_, ()>((lane, is_player, note_type));
                self.watchdog_deadline_us.store(0, Ordering::Relaxed);
                if let Err(e) = res {
                    call_err = Some(e.to_string());
                }
            }
        });
        if let Some(err) = call_err {
            self.handle_js_error(fn_name, err);
        }
    }

    pub fn call_chart_event(&mut self, name: &str, val1: &str, val2: &str) {
        if self.disabled_by_watchdog {
            return;
        }
        let mut call_err: Option<String> = None;
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>("onEvent") {
                Self::arm_deadline(
                    &self.watchdog_origin,
                    &self.watchdog_deadline_us,
                    &self.watchdog_tripped,
                    JS_FRAME_BUDGET_MS,
                );
                let res = func.call::<_, ()>((name, val1, val2));
                self.watchdog_deadline_us.store(0, Ordering::Relaxed);
                if let Err(e) = res {
                    call_err = Some(e.to_string());
                }
            }
        });
        if let Some(err) = call_err {
            self.handle_js_error("onEvent", err);
        }
    }
}
