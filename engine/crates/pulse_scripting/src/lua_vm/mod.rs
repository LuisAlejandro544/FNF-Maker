//! Archivo: engine/crates/pulse_scripting/src/lua_vm/mod.rs
//! Propósito: Punto de entrada de la máquina virtual Lua 5.4 (`mlua`).
//! Gestiona precompilación a bytecode, hook watchdog contra bucles infinitos, cuarentena
//! automática, invocación de callbacks del mod y montaje de módulos de bindings.

mod bindings_hud_fx;
mod bindings_world;

#[cfg(test)]
mod tests;

use crate::bridge::ScriptHostState;
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
        bindings_world::register_world_bindings(&lua, &globals, &host)?;
        bindings_hud_fx::register_hud_fx_bindings(&lua, &globals, &host)?;

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

    pub fn call_note_event(&mut self, fn_name: &str, lane: i32, is_player: bool, note_type: &str) {
        if self.disabled_by_watchdog {
            return;
        }
        if let Ok(func) = self.lua.globals().get::<Function>(fn_name) {
            self.begin_frame_watchdog();
            let res = func.call::<()>((lane, is_player, note_type));
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
