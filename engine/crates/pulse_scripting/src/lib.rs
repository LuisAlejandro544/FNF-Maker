//! Archivo: engine/crates/pulse_scripting/src/lib.rs
//! Propósito: Punto de entrada de `pulse_scripting`.
//! Coordina la validación, transpilación de TypeScript a JavaScript y ejecución
//! simultánea de scripts comunitarios escritos en Lua 5.4, TypeScript y JavaScript.

pub mod bridge;
pub mod js_vm;
pub mod lua_vm;
pub mod ts_transpiler;

pub use bridge::{ScriptCommand, ScriptHostState};
pub use js_vm::JsScriptVm;
pub use lua_vm::LuaScriptVm;
pub use ts_transpiler::transpile_typescript_to_js;

use pulse_core::project::{ScriptAsset, ScriptLanguage};
use std::sync::Arc;

/// Orquestador unificado que mantiene vivas las máquinas virtuales Lua y QuickJS durante el gameplay.
pub struct ModScriptRuntime {
    pub state: Arc<ScriptHostState>,
    lua_vms: Vec<LuaScriptVm>,
    js_vms: Vec<JsScriptVm>,
}

impl ModScriptRuntime {
    /// Carga y compila todos los scripts habilitados de un proyecto.
    pub fn load_from_scripts(scripts: &[ScriptAsset]) -> (Self, Vec<String>) {
        let state = Arc::new(ScriptHostState::default());
        let mut lua_vms = Vec::new();
        let mut js_vms = Vec::new();
        let mut errors = Vec::new();

        for script in scripts.iter().filter(|s| s.enabled) {
            match script.language {
                ScriptLanguage::Lua => match LuaScriptVm::new(&script.source_code, Arc::clone(&state)) {
                    Ok(vm) => lua_vms.push(vm),
                    Err(e) => errors.push(format!("[{}] {}", script.name, e)),
                },
                ScriptLanguage::JavaScript => match JsScriptVm::new(&script.source_code, Arc::clone(&state)) {
                    Ok(vm) => js_vms.push(vm),
                    Err(e) => errors.push(format!("[{}] {}", script.name, e)),
                },
                ScriptLanguage::TypeScript => match transpile_typescript_to_js(&script.name, &script.source_code) {
                    Ok(js_code) => match JsScriptVm::new(&js_code, Arc::clone(&state)) {
                        Ok(vm) => js_vms.push(vm),
                        Err(e) => errors.push(format!("[{}] {}", script.name, e)),
                    },
                    Err(e) => errors.push(format!("[{}] {}", script.name, e)),
                },
            }
        }

        (
            Self {
                state,
                lua_vms,
                js_vms,
            },
            errors,
        )
    }

    pub fn call_on_create(&mut self) {
        for vm in &mut self.lua_vms {
            vm.call_void("onCreate");
        }
        for vm in &mut self.js_vms {
            vm.call_void("onCreate");
        }
    }

    pub fn call_on_update(&mut self, dt: f32) {
        for vm in &mut self.lua_vms {
            vm.call_f32("onUpdate", dt);
        }
        for vm in &mut self.js_vms {
            vm.call_f32("onUpdate", dt);
        }
    }

    pub fn call_on_beat_hit(&mut self, beat: i32) {
        for vm in &mut self.lua_vms {
            vm.call_i32("onBeatHit", beat);
        }
        for vm in &mut self.js_vms {
            vm.call_i32("onBeatHit", beat);
        }
    }

    pub fn call_on_note_hit(&mut self, lane: i32, is_player: bool) {
        for vm in &mut self.lua_vms {
            vm.call_note_event("onNoteHit", lane, is_player);
        }
        for vm in &mut self.js_vms {
            vm.call_note_event("onNoteHit", lane, is_player);
        }
    }

    pub fn call_on_note_miss(&mut self, lane: i32, is_player: bool) {
        for vm in &mut self.lua_vms {
            vm.call_note_event("onNoteMiss", lane, is_player);
        }
        for vm in &mut self.js_vms {
            vm.call_note_event("onNoteMiss", lane, is_player);
        }
    }

    /// Extrae todos los comandos emitidos por Lua, TS y JS en este frame para aplicarlos al escenario.
    pub fn drain_commands(&self) -> Vec<ScriptCommand> {
        self.state.drain_commands()
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
