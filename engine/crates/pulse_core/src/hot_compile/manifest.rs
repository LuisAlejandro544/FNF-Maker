//! Archivo: engine/crates/pulse_core/src/hot_compile/manifest.rs
//! Propósito: Modela el manifiesto persistente `compiled_cache/manifest.json` y las estructuras
//! de resultado (`HotCompileManifest`, `HotCompileReport`, `CompiledBundle`, entradas cacheadas)
//! para reutilizar artefactos WebP, WAV y scripts precompilados entre sesiones.

use super::analysis::EntityLifecycleWindow;
use super::fingerprints::{AudioMergeFingerprint, TextureFingerprint};
use crate::chart::NoteDirection;
use crate::project::{ScriptAsset, ScriptLanguage};
use crate::sparrow::SparrowAtlas;
use crate::stage::StageEntity;
use crate::svg_skin::RasterizedArrow;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Versión del esquema de manifiesto incremental en `compiled_cache/manifest.json`.
pub const MANIFEST_SCHEMA_VERSION: u32 = 6;
/// Versión del analizador de bucles en scripts; incrementarla invalida caché previa de scripts.
pub const SCRIPT_OPTIMIZER_CACHE_VERSION: u32 = 3;

/// Formato de píxeles recomendado para decodificar la textura compilada en Android.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TexturePixelFormat {
    /// RGBA de 32 bits estándar (`Bitmap.Config.ARGB_8888`) para sprites con transparencia.
    #[default]
    Argb8888,
    /// RGB de 16 bits con dithering Floyd-Steinberg (`Bitmap.Config.RGB_565`) para fondos opacos (50 % menos RAM).
    Rgb565,
}

impl TexturePixelFormat {
    pub fn as_key(self) -> &'static str {
        match self {
            TexturePixelFormat::Argb8888 => "argb8888",
            TexturePixelFormat::Rgb565 => "rgb565",
        }
    }
}

/// Entrada cacheada de textura WebP compilada en disco.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTextureEntry {
    pub entity_id: String,
    pub fingerprint: TextureFingerprint,
    pub compiled_webp_path: PathBuf,
    #[serde(default)]
    pub extra_page_paths: Vec<PathBuf>,
    #[serde(default)]
    pub pixel_format: TexturePixelFormat,
    #[serde(default)]
    pub palette_quantized: bool,
    pub downscaled: bool,
    pub vram_saved_bytes: usize,
    #[serde(default)]
    pub atlas_repacked: bool,
    #[serde(default)]
    pub repacked_atlas: Option<SparrowAtlas>,
    #[serde(default)]
    pub dead_frames_removed: usize,
    #[serde(default)]
    pub duplicate_frames_merged: usize,
    #[serde(default)]
    pub trimmed_frames: usize,
    #[serde(default)]
    pub pruned_unused_anims: usize,
}

/// Entrada cacheada de la mezcla de audio en streaming WAV.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedAudioEntry {
    pub fingerprint: AudioMergeFingerprint,
    pub compiled_wav_path: PathBuf,
    pub merged_tracks: usize,
    pub estimated_ram_saved_kb: usize,
}

/// Entrada cacheada de un script optimizado y precompilado (`.luac` / `.js` transpilado).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedScriptEntry {
    pub script_id: String,
    pub language: ScriptLanguage,
    pub enabled: bool,
    pub source_hash: u64,
    #[serde(default)]
    pub optimized_source_hash: u64,
    pub optimizer_version: u32,
    pub optimized_source: String,
    pub discarded_heavy_loops: usize,
    pub neutralized_unbounded_loops: usize,
    pub artifact_path: Option<PathBuf>,
    pub compile_error: Option<String>,
}

/// Manifiesto persistente guardado en `compiled_cache/manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotCompileManifest {
    pub manifest_version: u32,
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub textures: HashMap<String, CachedTextureEntry>,
    #[serde(default)]
    pub audio: Option<CachedAudioEntry>,
    #[serde(default)]
    pub scripts: HashMap<String, CachedScriptEntry>,
}

impl Default for HotCompileManifest {
    fn default() -> Self {
        Self {
            manifest_version: MANIFEST_SCHEMA_VERSION,
            project_id: String::new(),
            textures: HashMap::new(),
            audio: None,
            scripts: HashMap::new(),
        }
    }
}

impl HotCompileManifest {
    pub fn load_or_default(cache_dir: &Path, project_id: &str) -> Self {
        let manifest_path = cache_dir.join("manifest.json");
        let Ok(raw) = std::fs::read_to_string(&manifest_path) else {
            return Self {
                project_id: project_id.to_string(),
                ..Default::default()
            };
        };
        let Ok(parsed) = serde_json::from_str::<Self>(&raw) else {
            return Self {
                project_id: project_id.to_string(),
                ..Default::default()
            };
        };
        if parsed.manifest_version != MANIFEST_SCHEMA_VERSION || parsed.project_id != project_id {
            return Self {
                project_id: project_id.to_string(),
                ..Default::default()
            };
        }
        parsed
    }

    pub fn save_to_dir(&self, cache_dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(cache_dir)
            .map_err(|e| format!("Error creando directorio de caché: {}", e))?;
        let target = cache_dir.join("manifest.json");
        let tmp = cache_dir.join("manifest.json.tmp");
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Error serializando manifest.json: {}", e))?;
        std::fs::write(&tmp, json)
            .map_err(|e| format!("Error escribiendo manifest.json.tmp: {}", e))?;
        std::fs::rename(&tmp, &target)
            .map_err(|e| format!("Error moviendo manifest.json: {}", e))?;
        Ok(())
    }
}

/// Textura RGBA optimizada temporal (solo se retiene si no se indicó carpeta de caché en disco).
#[derive(Debug, Clone)]
pub struct OptimizedTexture {
    pub entity_id: String,
    pub width: u32,
    pub height: u32,
    pub scale_factor_applied: f32,
    pub rgba: Vec<u8>,
}

/// Reporte de resultados de la compilación en caliente para mostrar al usuario o depurar.
#[derive(Debug, Clone, Default)]
pub struct HotCompileReport {
    pub elapsed_ms: u64,
    pub kept_entities: usize,
    pub discarded_invisible_entities: Vec<String>,
    pub downscaled_textures: usize,
    pub skipped_optimal_textures: usize,
    pub cached_textures_reused: usize,
    pub webp_encoded_textures: usize,
    pub discarded_heavy_loops: usize,
    pub neutralized_unbounded_loops: usize,
    pub estimated_vram_saved_kb: usize,
    pub dead_frames_removed: usize,
    pub duplicate_frames_merged: usize,
    pub trimmed_frames: usize,
    pub pruned_unused_anims: usize,
    pub repacked_atlases: usize,
    pub repacked_entity_ids: Vec<String>,
    pub rgb565_textures: usize,
    pub quantized_textures: usize,
    pub split_atlas_pages_total: usize,
    pub scheduled_preload_entities: usize,
    pub scheduled_retire_entities: usize,
    pub merged_audio_tracks: usize,
    pub compiled_audio_path: Option<PathBuf>,
    pub estimated_audio_ram_saved_kb: usize,
    pub cached_audio_reused: bool,
    pub precompiled_scripts: usize,
    pub cached_scripts_reused: usize,
    pub script_precompile_errors: Vec<String>,
}

/// Paquete final ya compilado y optimizado que consume la pantalla de gameplay.
/// Cuando se compila hacia caché en disco (`out_dir`), `textures` queda vacío para evitar
/// copias dobles/triples de píxeles RGBA en RAM.
#[derive(Debug, Clone)]
pub struct CompiledBundle {
    pub active_entities: Vec<StageEntity>,
    pub textures: HashMap<String, OptimizedTexture>,
    pub compiled_texture_paths: HashMap<String, PathBuf>,
    pub compiled_texture_pages: HashMap<String, Vec<PathBuf>>,
    pub texture_pixel_formats: HashMap<String, TexturePixelFormat>,
    pub passthrough_paths: HashMap<String, PathBuf>,
    pub timeline_windows: Vec<EntityLifecycleWindow>,
    pub optimized_scripts: Vec<ScriptAsset>,
    pub arrow_textures: HashMap<NoteDirection, RasterizedArrow>,
    pub report: HotCompileReport,
}
