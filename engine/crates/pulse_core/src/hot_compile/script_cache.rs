//! Archivo: engine/crates/pulse_core/src/hot_compile/script_cache.rs
//! Propósito: Optimización de bucles pesados en scripts Lua/TS/JS con caché incremental
//! por `source_hash` dentro de `HotCompileManifest`.

use super::fingerprints::fnv1a_64_str;
use super::manifest::{
    CachedScriptEntry, HotCompileManifest, SCRIPT_OPTIMIZER_CACHE_VERSION,
};
use crate::project::ScriptAsset;
use crate::script_optimizer::optimize_single_script;
use std::collections::HashMap;

/// Optimiza bucles pesados de scripts reutilizando resultados cacheados por `source_hash` cuando aplica,
/// preservando artefactos `.luac`/`.js` precompilados en `manifest.scripts` si el hash coincide.
pub(super) fn optimize_scripts_with_manifest(
    scripts: &[ScriptAsset],
    manifest: &mut HotCompileManifest,
) -> (Vec<ScriptAsset>, usize, usize) {
    let mut optimized_scripts = Vec::with_capacity(scripts.len());
    let mut discarded_heavy = 0usize;
    let mut neutralized_unbounded = 0usize;
    let mut next_scripts_map = HashMap::with_capacity(scripts.len());

    for script in scripts {
        if !script.enabled {
            optimized_scripts.push(script.clone());
            continue;
        }
        let source_hash = fnv1a_64_str(&script.source_code);
        if let Some(prev) = manifest.scripts.get(&script.id) {
            if prev.source_hash == source_hash
                && prev.optimizer_version == SCRIPT_OPTIMIZER_CACHE_VERSION
                && prev.language == script.language
                && !prev.optimized_source.is_empty()
            {
                discarded_heavy += prev.discarded_heavy_loops;
                neutralized_unbounded += prev.neutralized_unbounded_loops;
                let mut cloned = script.clone();
                cloned.source_code = prev.optimized_source.clone();
                let mut updated = prev.clone();
                updated.enabled = true;
                next_scripts_map.insert(script.id.clone(), updated);
                optimized_scripts.push(cloned);
                continue;
            }
        }

        let res = optimize_single_script(script.language, &script.source_code);
        discarded_heavy += res.discarded_loops;
        neutralized_unbounded += res.neutralized_loops;

        let opt_hash = fnv1a_64_str(&res.optimized_source);
        next_scripts_map.insert(
            script.id.clone(),
            CachedScriptEntry {
                script_id: script.id.clone(),
                language: script.language,
                enabled: true,
                source_hash,
                optimized_source_hash: opt_hash,
                optimizer_version: SCRIPT_OPTIMIZER_CACHE_VERSION,
                optimized_source: res.optimized_source.clone(),
                discarded_heavy_loops: res.discarded_loops,
                neutralized_unbounded_loops: res.neutralized_loops,
                artifact_path: None,
                compile_error: None,
            },
        );

        let mut cloned = script.clone();
        cloned.source_code = res.optimized_source;
        optimized_scripts.push(cloned);
    }

    manifest.scripts = next_scripts_map;
    (optimized_scripts, discarded_heavy, neutralized_unbounded)
}
