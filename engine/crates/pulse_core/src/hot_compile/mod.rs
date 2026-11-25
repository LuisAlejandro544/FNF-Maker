//! Archivo: engine/crates/pulse_core/src/hot_compile/mod.rs
//! Propósito: Punto de entrada modular del compilador en caliente paralelo (`rayon`).
//! Reexporta huellas de activos, manifiesto incremental en disco, análisis de visibilidad
//! y la tubería paralela de optimización de texturas WebP, atlas Sparrow y scripts.

mod analysis;
mod aux_textures;
mod fingerprints;
mod manifest;
mod pipeline;

#[cfg(test)]
mod tests;

pub use analysis::{
    EntityLifecycleWindow, TIMELINE_PRELOAD_LEAD_MS, TIMELINE_RETIRE_GRACE_MS,
    analyze_reused_entity_ids, collect_script_referenced_ids, compute_project_max_camera_zoom,
    compute_smart_texture_scale, compute_smart_texture_scale_for_viewport,
    compute_timeline_asset_lifecycle, is_permanently_invisible,
};
pub use aux_textures::{
    AuxTexturesCompileResult, compile_countdown_and_icons_with_manifest,
    countdown_max_side_for_tier, icon_max_side_for_tier,
};
pub use fingerprints::{
    AssetFileFingerprint, AudioMergeFingerprint, AudioSlotFingerprint, DeviceResourceTier,
    ScreenViewportMetrics, TextureFingerprint, fnv1a_64_bytes, fnv1a_64_str,
};
pub use manifest::{
    AuxTextureFingerprint, CachedAudioEntry, CachedAuxTextureEntry, CachedScriptEntry,
    CachedTextureEntry, CompiledBundle, HotCompileManifest, HotCompileReport,
    MANIFEST_SCHEMA_VERSION, OptimizedTexture, SCRIPT_OPTIMIZER_CACHE_VERSION, TexturePixelFormat,
};
pub use pipeline::{
    hot_compile_project, hot_compile_project_to_dir, hot_compile_project_to_dir_with_manifest,
    hot_compile_project_to_dir_with_manifest_viewport, hot_compile_project_to_dir_with_viewport,
};
