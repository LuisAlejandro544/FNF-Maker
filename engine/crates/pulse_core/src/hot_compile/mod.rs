//! Archivo: engine/crates/pulse_core/src/hot_compile/mod.rs
//! Propósito: Punto de entrada modular del compilador en caliente paralelo (`rayon`).
//! Reexporta huellas de activos, manifiesto incremental en disco, análisis de visibilidad
//! y la tubería paralela de optimización de texturas WebP, atlas Sparrow y scripts.

mod analysis;
mod fingerprints;
mod manifest;
mod pipeline;

#[cfg(test)]
mod tests;

pub use analysis::{
    analyze_reused_entity_ids, collect_script_referenced_ids, compute_smart_texture_scale,
    is_permanently_invisible,
};
pub use fingerprints::{
    AssetFileFingerprint, AudioMergeFingerprint, AudioSlotFingerprint, DeviceResourceTier,
    TextureFingerprint, fnv1a_64_bytes, fnv1a_64_str,
};
pub use manifest::{
    CachedAudioEntry, CachedScriptEntry, CachedTextureEntry, CompiledBundle, HotCompileManifest,
    HotCompileReport, MANIFEST_SCHEMA_VERSION, OptimizedTexture, SCRIPT_OPTIMIZER_CACHE_VERSION,
};
pub use pipeline::{
    hot_compile_project, hot_compile_project_to_dir, hot_compile_project_to_dir_with_manifest,
};
