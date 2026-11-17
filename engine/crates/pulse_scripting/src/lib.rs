//! Archivo: engine/crates/pulse_scripting/src/lib.rs
//! Propósito: Punto de entrada de `pulse_scripting`.
//! Coordina la validación, transpilación de TypeScript a JavaScript y ejecución
//! simultánea de scripts comunitarios escritos en Lua 5.4, TypeScript y JavaScript.

pub mod bridge;
pub mod js_vm;
pub mod lua_vm;
pub mod ts_transpiler;

pub use bridge::{EntityPropSnapshot, ScriptCommand, ScriptHostState, StrumPropSnapshot};
pub use js_vm::{JsScriptVm, JsWorkerHandle};
pub use lua_vm::LuaScriptVm;
pub use ts_transpiler::transpile_typescript_to_js;

use pulse_core::project::{ScriptAsset, ScriptLanguage};
use std::sync::Arc;

/// Orquestador unificado que mantiene vivas las máquinas virtuales Lua y QuickJS durante el gameplay.
pub struct ModScriptRuntime {
    pub state: Arc<ScriptHostState>,
    lua_vms: Vec<LuaScriptVm>,
    js_worker: Option<JsWorkerHandle>,
}

impl ModScriptRuntime {
    /// Carga y compila todos los scripts habilitados de un proyecto.
    pub fn load_from_scripts(scripts: &[ScriptAsset]) -> (Self, Vec<String>) {
        let state = Arc::new(ScriptHostState::default());
        let mut lua_vms = Vec::new();
        let mut js_sources: Vec<(String, String)> = Vec::new();
        let mut errors = Vec::new();

        for script in scripts.iter().filter(|s| s.enabled) {
            match script.language {
                ScriptLanguage::Lua => match LuaScriptVm::new(&script.source_code, Arc::clone(&state)) {
                    Ok(vm) => lua_vms.push(vm),
                    Err(e) => errors.push(format!("[{}] {}", script.name, e)),
                },
                ScriptLanguage::JavaScript => {
                    js_sources.push((script.name.clone(), script.source_code.clone()));
                }
                ScriptLanguage::TypeScript => match transpile_typescript_to_js(&script.name, &script.source_code) {
                    Ok(js_code) => js_sources.push((script.name.clone(), js_code)),
                    Err(e) => errors.push(format!("[{}] {}", script.name, e)),
                },
            }
        }

        let js_worker = if js_sources.is_empty() {
            None
        } else {
            let (worker, mut js_errors) = JsWorkerHandle::spawn(js_sources, Arc::clone(&state));
            errors.append(&mut js_errors);
            Some(worker)
        };

        (
            Self {
                state,
                lua_vms,
                js_worker,
            },
            errors,
        )
    }

    pub fn call_on_create(&mut self) {
        for vm in &mut self.lua_vms {
            vm.call_void("onCreate");
        }
        if let Some(worker) = &self.js_worker {
            worker.call_void("onCreate");
        }
    }

    pub fn call_on_update(&mut self, dt: f32) {
        for vm in &mut self.lua_vms {
            vm.call_f32("onUpdate", dt);
        }
        if let Some(worker) = &self.js_worker {
            worker.call_f32("onUpdate", dt);
        }
    }

    pub fn call_on_beat_hit(&mut self, beat: i32) {
        for vm in &mut self.lua_vms {
            vm.call_i32("onBeatHit", beat);
        }
        if let Some(worker) = &self.js_worker {
            worker.call_i32("onBeatHit", beat);
        }
    }

    pub fn call_on_step_hit(&mut self, step: i32) {
        for vm in &mut self.lua_vms {
            vm.call_i32("onStepHit", step);
        }
        if let Some(worker) = &self.js_worker {
            worker.call_i32("onStepHit", step);
        }
    }

    pub fn call_on_countdown_tick(&mut self, stage: i32) {
        for vm in &mut self.lua_vms {
            vm.call_i32("onCountdownTick", stage);
        }
        if let Some(worker) = &self.js_worker {
            worker.call_i32("onCountdownTick", stage);
        }
    }

    pub fn call_on_note_hit(&mut self, lane: i32, is_player: bool) {
        for vm in &mut self.lua_vms {
            vm.call_note_event("onNoteHit", lane, is_player);
        }
        if let Some(worker) = &self.js_worker {
            worker.call_note_event("onNoteHit", lane, is_player);
        }
    }

    pub fn call_on_note_miss(&mut self, lane: i32, is_player: bool) {
        for vm in &mut self.lua_vms {
            vm.call_note_event("onNoteMiss", lane, is_player);
        }
        if let Some(worker) = &self.js_worker {
            worker.call_note_event("onNoteMiss", lane, is_player);
        }
    }

    pub fn call_on_event(&mut self, name: &str, val1: &str, val2: &str) {
        for vm in &mut self.lua_vms {
            vm.call_chart_event(name, val1, val2);
        }
        if let Some(worker) = &self.js_worker {
            worker.call_chart_event(name.to_string(), val1.to_string(), val2.to_string());
        }
    }

    pub fn call_on_tween_completed(&mut self, tag: &str) {
        for vm in &mut self.lua_vms {
            vm.call_str("onTweenCompleted", tag);
        }
        if let Some(worker) = &self.js_worker {
            worker.call_str("onTweenCompleted", tag.to_string());
        }
    }

    pub fn call_on_timer_completed(&mut self, tag: &str) {
        for vm in &mut self.lua_vms {
            vm.call_str("onTimerCompleted", tag);
        }
        if let Some(worker) = &self.js_worker {
            worker.call_str("onTimerCompleted", tag.to_string());
        }
    }

    /// Extrae todos los comandos emitidos por Lua, TS y JS en este frame para aplicarlos al escenario.
    pub fn drain_commands(&self) -> Vec<ScriptCommand> {
        self.state.drain_commands()
    }

    /// Extrae errores ocurridos en tiempo de ejecución para mostrarlos en pantalla sin crashear.
    pub fn drain_runtime_errors(&self) -> Vec<String> {
        self.state.drain_runtime_errors()
    }
}

/// Valida la sintaxis de un script antes de entrar al juego (usado en el editor de scripts).
pub fn validate_script_syntax(script: &ScriptAsset) -> Result<String, String> {
    let dummy_state = Arc::new(ScriptHostState::default());
    match script.language {
        ScriptLanguage::Lua => {
            LuaScriptVm::new(&script.source_code, dummy_state).map_err(|e| e.to_string())?;
            Ok("Lua 5.4 válido".to_string())
        }
        ScriptLanguage::JavaScript => {
            JsScriptVm::new(&script.source_code, dummy_state).map_err(|e| e.to_string())?;
            Ok("JavaScript ES2022 válido".to_string())
        }
        ScriptLanguage::TypeScript => {
            let js = transpile_typescript_to_js(&script.name, &script.source_code)?;
            JsScriptVm::new(&js, dummy_state).map_err(|e| e.to_string())?;
            Ok("TypeScript compilado con oxc y verificado en QuickJS".to_string())
        }
    }
}
