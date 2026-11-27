//! Archivo: engine/crates/pulse_core/src/script_optimizer/mod.rs
//! Propósito: Punto de entrada modular del analizador y optimizador estático de scripts
//! Lua 5.4, JavaScript y TypeScript ejecutado durante la compilación en caliente (`hot_compile`).
//! Detecta bucles infinitos o de alto consumo de CPU (especialmente dentro de `onUpdate`):
//! - Si un bucle pesado no contiene lógica necesaria del motor (`setEntityProp`, `setCameraZoom`,
//!   `shakeCamera`, `playAnim`, `getSongPosMs`), descarta el bucle por completo.
//! - Si un bucle pesado/infinito sí contiene lógica necesaria del motor, descarta la repetición
//!   peligrosa (reemplaza bucles infinitos por ejecución única segura o limita iteraciones excesivas)
//!   para proteger la fluidez en teléfonos Android de 32 y 64 bits sin romper la sintaxis.

mod js_opt;
mod lua_opt;
mod scanner;

#[cfg(test)]
mod tests;

use crate::project::{ScriptAsset, ScriptLanguage};
use js_opt::optimize_js_ts_loops;
use lua_opt::optimize_lua_loops;

/// Resultado del análisis y optimización inteligente sobre un script individual.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptLoopOptResult {
    pub optimized_source: String,
    pub discarded_loops: usize,
    pub neutralized_loops: usize,
}

/// Reporte acumulado de optimización de scripts durante `hot_compile`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScriptBatchReport {
    pub analyzed_scripts: usize,
    pub modified_scripts: usize,
    pub discarded_heavy_loops: usize,
    pub neutralized_unbounded_loops: usize,
}

/// Optimiza todos los scripts habilitados y devuelve las copias listas para ejecutarse en gameplay.
pub fn optimize_scripts_for_hot_compile(
    scripts: &[ScriptAsset],
) -> (Vec<ScriptAsset>, ScriptBatchReport) {
    let mut out = Vec::with_capacity(scripts.len());
    let mut report = ScriptBatchReport::default();

    for script in scripts {
        if !script.enabled {
            out.push(script.clone());
            continue;
        }
        report.analyzed_scripts += 1;
        let res = optimize_single_script(script.language, &script.source_code);
        if res.discarded_loops > 0 || res.neutralized_loops > 0 {
            report.modified_scripts += 1;
            report.discarded_heavy_loops += res.discarded_loops;
            report.neutralized_unbounded_loops += res.neutralized_loops;
        }
        let mut updated = script.clone();
        updated.source_code = res.optimized_source;
        out.push(updated);
    }

    (out, report)
}

/// Analiza y optimiza bucles pesados/infinitos de un script según su lenguaje.
pub fn optimize_single_script(language: ScriptLanguage, source: &str) -> ScriptLoopOptResult {
    match language {
        ScriptLanguage::Lua => optimize_lua_loops(source),
        ScriptLanguage::JavaScript | ScriptLanguage::TypeScript => optimize_js_ts_loops(source),
    }
}
