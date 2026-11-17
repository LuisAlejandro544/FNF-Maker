//! Archivo: engine/crates/pulse_scripting/src/lib.rs
//! Propósito: Punto de entrada de `pulse_scripting`.
//! Coordina la validación, precompilación en `hot_compile` (bytecode Lua 5.4 `.luac` y
//! transpilación TypeScript/JavaScript con `oxc`) y ejecución protegida por Watchdogs en tiempo real.

pub mod bridge;
pub mod js_vm;
pub mod lua_vm;
pub mod ts_transpiler;

pub use bridge::{EntityPropSnapshot, ScriptCommand, ScriptHostState, StrumPropSnapshot};
pub use js_vm::{JsScriptVm, JsWorkerHandle};
pub use lua_vm::LuaScriptVm;
pub use ts_transpiler::{transpile_typescript_to_js, validate_js_syntax};

use pulse_core::hot_compile::{HotCompileManifest, fnv1a_64_str};
use pulse_core::project::{ScriptAsset, ScriptLanguage};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Estadísticas del paso de precompilación e invalidación incremental de scripts.
#[derive(Debug, Clone, Default)]
pub struct ScriptPrecompileStats {
    pub precompiled_count: usize,
    pub cached_reused_count: usize,
    pub errors: Vec<String>,
}

/// Precompila los scripts optimizados durante `hot_compile` reutilizando caché en disco cuando el hash
/// del código optimizado coincide y eliminando artefactos obsoletos de `scripts_cache_dir`.
pub fn precompile_scripts_with_cache(
    scripts: &mut [ScriptAsset],
    manifest: &mut HotCompileManifest,
    scripts_cache_dir: Option<&Path>,
) -> ScriptPrecompileStats {
    let mut stats = ScriptPrecompileStats::default();
    if let Some(dir) = scripts_cache_dir {
        let _ = std::fs::create_dir_all(dir);
    }

    let mut kept_artifacts: HashSet<PathBuf> = HashSet::new();

    for script in scripts.iter_mut() {
        if !script.enabled {
            continue;
        }
        let opt_hash = fnv1a_64_str(&script.source_code);
        let cache_hit = manifest
            .scripts
            .get(&script.id)
            .filter(|entry| {
                entry.enabled
                    && entry.language == script.language
                    && entry.optimized_source_hash == opt_hash
                    && entry.compile_error.is_none()
                    && entry
                        .artifact_path
                        .as_ref()
                        .map(|p| p.exists())
                        .unwrap_or(false)
            })
            .and_then(|entry| entry.artifact_path.clone());

        if let Some(artifact) = cache_hit {
            let loaded_ok = match script.language {
                ScriptLanguage::Lua => match std::fs::read(&artifact) {
                    Ok(bytes) if !bytes.is_empty() => {
                        script.precompiled_bytecode = Some(bytes);
                        true
                    }
                    _ => false,
                },
                ScriptLanguage::JavaScript | ScriptLanguage::TypeScript => {
                    match std::fs::read_to_string(&artifact) {
                        Ok(js) if !js.is_empty() => {
                            script.precompiled_js = Some(js);
                            true
                        }
                        _ => false,
                    }
                }
            };
            if loaded_ok {
                kept_artifacts.insert(artifact);
                stats.cached_reused_count += 1;
                continue;
            }
        }

        // Cache Miss: precompilar desde cero y persistir artefacto en `compiled_cache/scripts/`
        match script.language {
            ScriptLanguage::Lua => match LuaScriptVm::precompile_bytecode(&script.source_code) {
                Ok(bytecode) => {
                    let artifact_path = scripts_cache_dir.map(|d| d.join(format!("{}.luac", script.id)));
                    if let Some(p) = &artifact_path {
                        let _ = std::fs::write(p, &bytecode);
                        kept_artifacts.insert(p.clone());
                    }
                    script.precompiled_bytecode = Some(bytecode);
                    stats.precompiled_count += 1;
                    if let Some(entry) = manifest.scripts.get_mut(&script.id) {
                        entry.optimized_source_hash = opt_hash;
                        entry.artifact_path = artifact_path;
                        entry.compile_error = None;
                    }
                }
                Err(e) => {
                    let msg = format!("[{}] {}", script.name, e);
                    stats.errors.push(msg.clone());
                    if let Some(entry) = manifest.scripts.get_mut(&script.id) {
                        entry.artifact_path = None;
                        entry.compile_error = Some(msg);
                    }
                }
            },
            ScriptLanguage::TypeScript => {
                match transpile_typescript_to_js(&script.name, &script.source_code)
                    .and_then(|js| validate_js_syntax(&script.name, &js).map(|_| js))
                {
                    Ok(js) => {
                        let artifact_path =
                            scripts_cache_dir.map(|d| d.join(format!("{}.js", script.id)));
                        if let Some(p) = &artifact_path {
                            let _ = std::fs::write(p, &js);
                            kept_artifacts.insert(p.clone());
                        }
                        script.precompiled_js = Some(js);
                        stats.precompiled_count += 1;
                        if let Some(entry) = manifest.scripts.get_mut(&script.id) {
                            entry.optimized_source_hash = opt_hash;
                            entry.artifact_path = artifact_path;
                            entry.compile_error = None;
                        }
                    }
                    Err(e) => {
                        let msg = format!("[{}] {}", script.name, e);
                        stats.errors.push(msg.clone());
                        if let Some(entry) = manifest.scripts.get_mut(&script.id) {
                            entry.artifact_path = None;
                            entry.compile_error = Some(msg);
                        }
                    }
                }
            }
            ScriptLanguage::JavaScript => {
                match validate_js_syntax(&script.name, &script.source_code) {
                    Ok(()) => {
                        let artifact_path =
                            scripts_cache_dir.map(|d| d.join(format!("{}.js", script.id)));
                        if let Some(p) = &artifact_path {
                            let _ = std::fs::write(p, &script.source_code);
                            kept_artifacts.insert(p.clone());
                        }
                        script.precompiled_js = Some(script.source_code.clone());
                        stats.precompiled_count += 1;
                        if let Some(entry) = manifest.scripts.get_mut(&script.id) {
                            entry.optimized_source_hash = opt_hash;
                            entry.artifact_path = artifact_path;
                            entry.compile_error = None;
                        }
                    }
                    Err(e) => {
                        let msg = format!("[{}] {}", script.name, e);
                        stats.errors.push(msg.clone());
                        if let Some(entry) = manifest.scripts.get_mut(&script.id) {
                            entry.artifact_path = None;
                            entry.compile_error = Some(msg);
                        }
                    }
                }
            }
        }
    }

    // Limpiar artefactos huérfanos o de scripts deshabilitados/eliminados en `scripts_cache_dir`
    if let Some(dir) = scripts_cache_dir {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for ent in entries.flatten() {
                let p = ent.path();
                if p.is_file() && !kept_artifacts.contains(&p) {
                    let _ = std::fs::remove_file(&p);
                }
            }
        }
    }

    stats
}

/// Orquestador unificado que mantiene vivas las máquinas virtuales Lua y QuickJS durante el gameplay.
pub struct ModScriptRuntime {
    pub state: Arc<ScriptHostState>,
    lua_vms: Vec<LuaScriptVm>,
    js_worker: Option<JsWorkerHandle>,
}

impl ModScriptRuntime {
    /// Carga e instancia todas las VMs consumiendo bytecode/JS precompilado cuando esté disponible.
    pub fn load_from_scripts(scripts: &[ScriptAsset]) -> (Self, Vec<String>) {
        let state = Arc::new(ScriptHostState::default());
        let mut lua_vms = Vec::new();
        let mut js_sources: Vec<(String, String)> = Vec::new();
        let mut errors = Vec::new();

        for script in scripts.iter().filter(|s| s.enabled) {
            match script.language {
                ScriptLanguage::Lua => match LuaScriptVm::new_with_bytecode(
                    &script.name,
                    &script.source_code,
                    script.precompiled_bytecode.as_deref(),
                    Arc::clone(&state),
                ) {
                    Ok(vm) => lua_vms.push(vm),
                    Err(e) => errors.push(format!("[{}] {}", script.name, e)),
                },
                ScriptLanguage::JavaScript => {
                    let js = script
                        .precompiled_js
                        .clone()
                        .unwrap_or_else(|| script.source_code.clone());
                    js_sources.push((script.name.clone(), js));
                }
                ScriptLanguage::TypeScript => {
                    if let Some(precompiled) = &script.precompiled_js {
                        js_sources.push((script.name.clone(), precompiled.clone()));
                    } else {
                        match transpile_typescript_to_js(&script.name, &script.source_code) {
                            Ok(js_code) => js_sources.push((script.name.clone(), js_code)),
                            Err(e) => errors.push(format!("[{}] {}", script.name, e)),
                        }
                    }
                }
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
            LuaScriptVm::new_with_bytecode(&script.name, &script.source_code, None, dummy_state)
                .map_err(|e| e.to_string())?;
            Ok("Lua 5.4 válido".to_string())
        }
        ScriptLanguage::JavaScript => {
            validate_js_syntax(&script.name, &script.source_code)?;
            JsScriptVm::new_named(&script.name, &script.source_code, dummy_state)
                .map_err(|e| e.to_string())?;
            Ok("JavaScript ES2022 válido".to_string())
        }
        ScriptLanguage::TypeScript => {
            let js = transpile_typescript_to_js(&script.name, &script.source_code)?;
            validate_js_syntax(&script.name, &js)?;
            JsScriptVm::new_named(&script.name, &js, dummy_state).map_err(|e| e.to_string())?;
            Ok("TypeScript compilado con oxc y verificado en QuickJS".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulse_core::hot_compile::hot_compile_project_to_dir_with_manifest;
    use pulse_core::hot_compile::DeviceResourceTier;
    use pulse_core::project::ModProject;

    #[test]
    fn precompile_scripts_caches_lua_bytecode_and_transpiled_ts() {
        let base_dir = std::env::temp_dir().join("beatforge_script_precompile_test");
        let _ = std::fs::remove_dir_all(&base_dir);
        let cache_dir = base_dir.join("compiled_cache");
        let scripts_dir = cache_dir.join("scripts");

        let mut project = ModProject::new_demo();
        project.stage.entities.clear();

        let mut manifest = HotCompileManifest::load_or_default(&cache_dir, &project.id);
        let mut bundle1 = hot_compile_project_to_dir_with_manifest(
            &project,
            DeviceResourceTier::Balanced,
            Some(&cache_dir),
            &mut manifest,
            |_, _| {},
        );
        let st1 = precompile_scripts_with_cache(
            &mut bundle1.optimized_scripts,
            &mut manifest,
            Some(&scripts_dir),
        );
        assert!(st1.errors.is_empty());
        assert_eq!(st1.precompiled_count, 3);
        assert_eq!(st1.cached_reused_count, 0);
        assert!(bundle1
            .optimized_scripts
            .iter()
            .filter(|s| s.language == ScriptLanguage::Lua)
            .all(|s| s.precompiled_bytecode.is_some()));
        assert!(bundle1
            .optimized_scripts
            .iter()
            .filter(|s| s.language == ScriptLanguage::TypeScript)
            .all(|s| s.precompiled_js.is_some()));
        manifest.save_to_dir(&cache_dir).unwrap();

        // Segunda corrida con el mismo proyecto -> 100% Cache Hit
        let mut manifest2 = HotCompileManifest::load_or_default(&cache_dir, &project.id);
        let mut bundle2 = hot_compile_project_to_dir_with_manifest(
            &project,
            DeviceResourceTier::Balanced,
            Some(&cache_dir),
            &mut manifest2,
            |_, _| {},
        );
        let st2 = precompile_scripts_with_cache(
            &mut bundle2.optimized_scripts,
            &mut manifest2,
            Some(&scripts_dir),
        );
        assert!(st2.errors.is_empty());
        assert_eq!(st2.precompiled_count, 0);
        assert_eq!(st2.cached_reused_count, 3);

        let _ = std::fs::remove_dir_all(&base_dir);
    }
}
