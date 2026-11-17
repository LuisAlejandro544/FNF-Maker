//! Archivo: engine/crates/pulse_core/src/hot_compile.rs
//! Propósito: Compilador en caliente paralelo (`rayon`) inteligente con caché incremental en disco.
//! Funciones clave:
//! 1. Calcula huellas FNV-1a (ruta, tamaño, mtime y muestras de bytes + tier + geometría) para
//!    reutilizar texturas `.webp`, mezcla de audio `compiled_mix.wav` y scripts precompilados desde
//!    `compiled_cache/manifest.json` sin recomprimir cuando nada relevante cambió.
//! 2. Analiza visibilidad para DESCARTAR objetos que permanecerán invisibles toda la canción y elimina
//!    artefactos huérfanos o obsoletos en disco.
//! 3. Compila todos los sprites (`.png`, `.jpg`, `.astc` o reescalados inteligentes) a `.webp`
//!    VP8L a máxima compresión sin pérdida de calidad ni canal alfa dentro del pool `rayon`.
//! 4. Preoptimiza scripts Lua, TS y JS (`script_optimizer`) con caché por `source_hash` y rasteriza
//!    flechas `.svg` con `resvg`.

use crate::astc_loader::{
    is_astc_path, is_webp_path, load_sprite_image, read_sprite_dimensions, save_lossless_webp,
};
use crate::chart::NoteDirection;
use crate::project::{AudioTrackSlot, ModProject, ScriptAsset, ScriptLanguage};
use crate::script_optimizer::optimize_single_script;
use crate::stage::StageEntity;
use crate::svg_skin::{RasterizedArrow, rasterize_arrow_skin};
use image::imageops::FilterType;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Instant, UNIX_EPOCH};

/// Versión del esquema de manifiesto incremental en `compiled_cache/manifest.json`.
pub const MANIFEST_SCHEMA_VERSION: u32 = 2;
/// Versión del analizador de bucles en scripts; incrementarla invalida caché previa de scripts.
pub const SCRIPT_OPTIMIZER_CACHE_VERSION: u32 = 2;

/// Perfil de optimización según la memoria/arquitectura del teléfono del usuario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceResourceTier {
    /// Teléfonos 32-bit (`armeabi-v7a`) o 2–3 GB RAM: atlas hasta 2560px y fondos estáticos hasta 1536px.
    LowRam32Bit,
    /// Teléfonos gama media 64-bit: protege atlas hasta 4096px y fondos hasta 2560px.
    Balanced,
    /// Gama alta: hasta 4096px sin pérdida visible.
    HighQuality,
}

impl DeviceResourceTier {
    pub fn as_code(self) -> u8 {
        match self {
            DeviceResourceTier::LowRam32Bit => 0,
            DeviceResourceTier::Balanced => 1,
            DeviceResourceTier::HighQuality => 2,
        }
    }

    pub fn max_texture_dim(self) -> u32 {
        match self {
            DeviceResourceTier::LowRam32Bit => 1536,
            DeviceResourceTier::Balanced => 2560,
            DeviceResourceTier::HighQuality => 4096,
        }
    }

    /// Los atlas multi-frame (Sparrow/Animate) dividen la hoja entre decenas de cuadros,
    /// por lo que conservar mayor margen en atlas evita borrosidad del personaje.
    pub fn max_dim_for_entity(self, has_atlas: bool) -> u32 {
        if has_atlas {
            match self {
                DeviceResourceTier::LowRam32Bit => 2560,
                DeviceResourceTier::Balanced => 4096,
                DeviceResourceTier::HighQuality => 4096,
            }
        } else {
            self.max_texture_dim()
        }
    }

    /// Presupuesto de memoria RAM recomendado para caché auxiliar según la gama del dispositivo.
    pub fn lru_cache_budget_bytes(self) -> usize {
        match self {
            DeviceResourceTier::LowRam32Bit => 24 * 1024 * 1024,
            DeviceResourceTier::Balanced => 64 * 1024 * 1024,
            DeviceResourceTier::HighQuality => 128 * 1024 * 1024,
        }
    }

    pub fn label_es(self) -> &'static str {
        match self {
            DeviceResourceTier::LowRam32Bit => "Económico (32-bit / <=1536–2560px)",
            DeviceResourceTier::Balanced => "Balanceado HD (Atlas <=4096px)",
            DeviceResourceTier::HighQuality => "Máxima Calidad (<=4096px)",
        }
    }
}

/// Hash determinista FNV-1a de 64 bits (sin dependencias externas ni colisiones triviales).
pub fn fnv1a_64_bytes(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// Hash determinista FNV-1a de 64 bits sobre una cadena UTF-8.
pub fn fnv1a_64_str(s: &str) -> u64 {
    fnv1a_64_bytes(s.as_bytes())
}

/// Huella ligera de un archivo en disco combinando tamaño, fecha de modificación y hash FNV-1a
/// de bloques inicial/medio/final (o completo si pesa <= 192 KB) para máxima velocidad en ARM32.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetFileFingerprint {
    pub path: String,
    pub size_bytes: u64,
    pub modified_ms: u64,
    pub sample_hash: u64,
}

impl AssetFileFingerprint {
    pub fn from_path(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        let size_bytes = meta.len();
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let mut f = std::fs::File::open(path).ok()?;
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut feed = |chunk: &[u8]| {
            for &b in chunk {
                hash ^= b as u64;
                hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
            }
        };

        const CHUNK: usize = 64 * 1024;
        if size_bytes <= (CHUNK as u64) * 3 {
            let mut buf = Vec::with_capacity(size_bytes as usize);
            f.read_to_end(&mut buf).ok()?;
            feed(&buf);
        } else {
            let mut buf = vec![0u8; CHUNK];
            let n1 = f.read(&mut buf).ok()?;
            feed(&buf[..n1]);

            let mid_off = size_bytes / 2;
            f.seek(SeekFrom::Start(mid_off)).ok()?;
            let n2 = f.read(&mut buf).ok()?;
            feed(&buf[..n2]);

            let tail_off = size_bytes.saturating_sub(CHUNK as u64);
            f.seek(SeekFrom::Start(tail_off)).ok()?;
            let n3 = f.read(&mut buf).ok()?;
            feed(&buf[..n3]);
        }

        Some(Self {
            path: path.to_string_lossy().into_owned(),
            size_bytes,
            modified_ms,
            sample_hash: hash,
        })
    }
}

/// Huella completa de una textura en el escenario + configuración del dispositivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextureFingerprint {
    pub file: AssetFileFingerprint,
    pub tier_code: u8,
    pub has_atlas: bool,
    pub entity_w_bits: u32,
    pub entity_h_bits: u32,
    pub scale_bits: u32,
    pub atlas_frame_hash: u64,
}

impl TextureFingerprint {
    pub fn compute(entity: &StageEntity, path: &Path, tier: DeviceResourceTier) -> Option<Self> {
        let file = AssetFileFingerprint::from_path(path)?;
        let mut atlas_hash: u64 = 0xcbf2_9ce4_8422_2325;
        if let Some(atlas) = &entity.atlas {
            let mut max_frame_w = 0u32;
            let mut max_frame_h = 0u32;
            let mut count = 0u64;
            for anim in atlas.animations.values() {
                for fr in &anim.frames {
                    max_frame_w = max_frame_w.max(fr.width);
                    max_frame_h = max_frame_h.max(fr.height);
                    count += 1;
                }
            }
            for bytes in [
                max_frame_w.to_le_bytes().as_slice(),
                max_frame_h.to_le_bytes().as_slice(),
                count.to_le_bytes().as_slice(),
            ] {
                for &b in bytes {
                    atlas_hash ^= b as u64;
                    atlas_hash = atlas_hash.wrapping_mul(0x0000_0100_0000_01B3);
                }
            }
        }
        Some(Self {
            file,
            tier_code: tier.as_code(),
            has_atlas: entity.atlas.is_some(),
            entity_w_bits: entity.width.to_bits(),
            entity_h_bits: entity.height.to_bits(),
            scale_bits: entity.scale.to_bits(),
            atlas_frame_hash: atlas_hash,
        })
    }
}

/// Entrada cacheada de textura WebP compilada en disco.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedTextureEntry {
    pub entity_id: String,
    pub fingerprint: TextureFingerprint,
    pub compiled_webp_path: PathBuf,
    pub downscaled: bool,
    pub vram_saved_bytes: usize,
}

/// Huella de una ranura de audio para invalidar mezcla WAV solo cuando cambia archivo, ganancia o mute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioSlotFingerprint {
    pub slot_index: usize,
    pub file: AssetFileFingerprint,
    pub gain_bits: u32,
    pub muted: bool,
    pub duck_on_miss: bool,
}

/// Huella global de mezcla de pistas hacia `compiled_mix.wav`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioMergeFingerprint {
    pub target_sample_rate: u32,
    pub slots: Vec<AudioSlotFingerprint>,
}

impl AudioMergeFingerprint {
    pub fn compute(slots: &[AudioTrackSlot], target_sample_rate: u32) -> Option<Self> {
        let mut active = Vec::new();
        for slot in slots {
            let Some(path) = slot.file_path.as_ref() else {
                continue;
            };
            let file = AssetFileFingerprint::from_path(path)?;
            active.push(AudioSlotFingerprint {
                slot_index: slot.slot_index,
                file,
                gain_bits: slot.gain.to_bits(),
                muted: slot.muted,
                duck_on_miss: slot.duck_on_player_miss,
            });
        }
        if active.len() <= 1 {
            return None;
        }
        Some(Self {
            target_sample_rate,
            slots: active,
        })
    }
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
    pub passthrough_paths: HashMap<String, PathBuf>,
    pub optimized_scripts: Vec<ScriptAsset>,
    pub arrow_textures: HashMap<NoteDirection, RasterizedArrow>,
    pub report: HotCompileReport,
}

/// Recorre las fuentes de todos los scripts activos Y los eventos del chart (`chart.events`)
/// para saber qué IDs de entidad podrían revelarse, intercambiarse o animarse durante la canción.
pub fn collect_script_referenced_ids(project: &ModProject) -> HashSet<String> {
    let mut referenced = HashSet::new();
    for script in project.scripts.iter().filter(|s| s.enabled) {
        for entity in &project.stage.entities {
            if script.source_code.contains(&entity.id) {
                referenced.insert(entity.id.clone());
            }
        }
    }
    for ev in &project.chart.events {
        for entity in &project.stage.entities {
            if ev.value1.contains(&entity.id)
                || ev.value2.contains(&entity.id)
                || ev.name.contains(&entity.id)
            {
                referenced.insert(entity.id.clone());
            }
        }
    }
    referenced
}

/// Analiza los eventos del chart (`Stage Swap`, `Preload Sprite`, etc.) para detectar qué sprites
/// que se ocultan/desactivan en un punto de la canción vuelven a aparecer más adelante.
/// Estos sprites siguen pudiendo precargarse bajo demanda en el segundo uso aunque hayan salido de RAM.
pub fn analyze_reused_entity_ids(project: &ModProject) -> HashSet<String> {
    let mut sorted_events = project.chart.events.clone();
    sorted_events.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));

    let mut hidden_at_some_point: HashSet<String> = HashSet::new();
    let mut reused_later: HashSet<String> = HashSet::new();

    for ev in &sorted_events {
        let lower_name = ev.name.trim().to_ascii_lowercase();
        if lower_name != "stage swap"
            && lower_name != "swap stage"
            && lower_name != "change character"
            && lower_name != "preload sprite"
        {
            continue;
        }
        let mut shown: Vec<String> = Vec::new();
        let mut hidden: Vec<String> = Vec::new();

        for part in ev.value1.split(';').chain(ev.value2.split(';')) {
            let trimmed = part.trim();
            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim().to_ascii_lowercase();
                let ids: Vec<String> = v
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                match key.as_str() {
                    "show" | "mostrar" | "player" | "jugador" | "opp" | "opponent" | "enemigo"
                    | "rival" => shown.extend(ids),
                    "hide" | "ocultar" => hidden.extend(ids),
                    _ => {}
                }
            } else if lower_name == "preload sprite" && !trimmed.is_empty() {
                shown.extend(
                    trimmed
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty()),
                );
            }
        }

        for id in &shown {
            if hidden_at_some_point.contains(id) {
                reused_later.insert(id.clone());
            }
        }
        for id in hidden {
            hidden_at_some_point.insert(id);
        }
    }

    reused_later
}

/// Determina si una entidad está garantizada a ser invisible durante toda la canción.
pub fn is_permanently_invisible(
    entity: &StageEntity,
    script_refs: &HashSet<String>,
    virtual_w: f32,
    virtual_h: f32,
) -> bool {
    let touched_by_script = script_refs.contains(&entity.id);
    if touched_by_script {
        return false;
    }

    if !entity.visible || entity.opacity <= 0.005 || entity.scale <= 0.005 {
        return true;
    }

    let half_w = (entity.width * entity.scale) * 0.5;
    let half_h = (entity.height * entity.scale) * 0.5;
    let margin = 450.0_f32;
    let out_x = (entity.x + half_w) < -margin || (entity.x - half_w) > (virtual_w + margin);
    let out_y = (entity.y + half_h) < -margin || (entity.y - half_h) > (virtual_h + margin);
    out_x || out_y
}

/// Calcula inteligentemente si una textura requiere bajar resolución o si ya es óptima.
/// Devuelve `1.0` cuando NO conviene bajar resolución de píxeles.
pub fn compute_smart_texture_scale(
    entity: &StageEntity,
    orig_w: u32,
    orig_h: u32,
    tier: DeviceResourceTier,
) -> f32 {
    if orig_w == 0 || orig_h == 0 {
        return 1.0;
    }
    let largest = orig_w.max(orig_h);
    let has_atlas = entity.atlas.is_some();
    let tier_cap = tier.max_dim_for_entity(has_atlas);

    // 1. Límite duro de arquitectura/GPU del dispositivo
    let mut target_ratio = if largest > tier_cap {
        (tier_cap as f32) / (largest as f32)
    } else {
        1.0
    };

    // 2. Evaluación inteligente de nitidez respecto al tamaño visible en pantalla (1280x720 + zoom)
    let display_max = (entity.width.max(entity.height) * entity.scale).max(128.0);
    if let Some(atlas) = &entity.atlas {
        let mut max_frame_side = 0u32;
        for anim in atlas.animations.values() {
            for frame in &anim.frames {
                max_frame_side = max_frame_side.max(frame.width.max(frame.height));
            }
        }
        // Conservar al menos ~2.2x de resolución por frame frente a su tamaño en pantalla para cero borrosidad
        if max_frame_side > 0 {
            let ideal_frame_max = (display_max * 2.2).max(512.0);
            if (max_frame_side as f32) > ideal_frame_max * 1.15 && largest > 1536 {
                let frame_ratio = (ideal_frame_max / (max_frame_side as f32)).clamp(0.45, 1.0);
                target_ratio = target_ratio.min(frame_ratio);
            }
        }
    } else {
        // Imagen estática o fondo: conservar al menos 1.5x del tamaño en pantalla (mínimo 720p/1280p)
        let ideal_static_max = (display_max * 1.5).max(960.0);
        if (largest as f32) > ideal_static_max * 1.2 {
            let static_ratio = (ideal_static_max / (largest as f32)).clamp(0.40, 1.0);
            target_ratio = target_ratio.min(static_ratio);
        }
    }

    // Si la reducción sería menor al 8% (ratio >= 0.92) y cabe en tier_cap, conserva resolución 1:1.
    if target_ratio >= 0.92 && largest <= tier_cap {
        1.0
    } else {
        target_ratio.clamp(0.25, 1.0)
    }
}

enum CompiledTexOutcome {
    CacheHit(String, CachedTextureEntry),
    DiskWebp {
        entity_id: String,
        entry: Option<CachedTextureEntry>,
        path: PathBuf,
    },
    InMemory(String, OptimizedTexture),
    PassThrough(String, PathBuf),
}

/// Optimiza bucles pesados de scripts reutilizando resultados cacheados por `source_hash` cuando aplica,
/// preservando artefactos `.luac`/`.js` precompilados en `manifest.scripts` si el hash coincide.
fn optimize_scripts_with_manifest(
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

/// Ejecuta la compilación en caliente de todos los recursos y scripts del proyecto en paralelo usando `rayon`.
pub fn hot_compile_project<F>(
    project: &ModProject,
    tier: DeviceResourceTier,
    on_progress: F,
) -> CompiledBundle
where
    F: Fn(f32, &str) + Send + Sync,
{
    hot_compile_project_to_dir(project, tier, None, on_progress)
}

/// Compila en caliente hacia `out_dir` administrando su propio `manifest.json` en disco.
pub fn hot_compile_project_to_dir<F>(
    project: &ModProject,
    tier: DeviceResourceTier,
    out_dir: Option<&Path>,
    on_progress: F,
) -> CompiledBundle
where
    F: Fn(f32, &str) + Send + Sync,
{
    let mut manifest = out_dir
        .map(|d| HotCompileManifest::load_or_default(d, &project.id))
        .unwrap_or_else(|| HotCompileManifest {
            project_id: project.id.clone(),
            ..Default::default()
        });
    let bundle =
        hot_compile_project_to_dir_with_manifest(project, tier, out_dir, &mut manifest, on_progress);
    if let Some(dir) = out_dir {
        let _ = manifest.save_to_dir(dir);
    }
    bundle
}

/// Compila en caliente permitiendo compartir un `HotCompileManifest` externo (para integrar también
/// caché incremental de mezcla WAV y scripts precompilados `.luac`/`.js` en `pulse_android`).
pub fn hot_compile_project_to_dir_with_manifest<F>(
    project: &ModProject,
    tier: DeviceResourceTier,
    out_dir: Option<&Path>,
    manifest: &mut HotCompileManifest,
    on_progress: F,
) -> CompiledBundle
where
    F: Fn(f32, &str) + Send + Sync,
{
    let started = Instant::now();
    manifest.manifest_version = MANIFEST_SCHEMA_VERSION;
    manifest.project_id = project.id.clone();

    on_progress(0.06, "Optimizando bucles en scripts Lua, TS y JS...");
    let (optimized_scripts, discarded_heavy_loops, neutralized_unbounded_loops) =
        optimize_scripts_with_manifest(&project.scripts, manifest);

    on_progress(0.14, "Analizando visibilidad y objetos del escenario...");

    let script_refs = collect_script_referenced_ids(project);
    let mut active_entities = Vec::new();
    let mut discarded = Vec::new();

    for entity in &project.stage.entities {
        if is_permanently_invisible(
            entity,
            &script_refs,
            project.stage.virtual_width,
            project.stage.virtual_height,
        ) {
            discarded.push(entity.display_name.clone());
        } else {
            active_entities.push(entity.clone());
        }
    }

    active_entities.sort_by_key(|e| e.z_index);

    if let Some(dir) = out_dir {
        let _ = std::fs::create_dir_all(dir);
    }

    on_progress(0.22, "Verificando caché incremental y compilando WebP sin pérdida...");

    let total_entities = active_entities.len().max(1);
    let done_counter = Arc::new(AtomicUsize::new(0));
    let vram_saved_bytes = Arc::new(AtomicUsize::new(0));
    let downscaled_count = Arc::new(AtomicUsize::new(0));
    let skipped_optimal_count = Arc::new(AtomicUsize::new(0));
    let cached_reused_count = Arc::new(AtomicUsize::new(0));
    let webp_encoded_count = Arc::new(AtomicUsize::new(0));

    let prev_textures = &manifest.textures;

    let outcomes: Vec<CompiledTexOutcome> = active_entities
        .par_iter()
        .filter_map(|entity| {
            let path = entity.image_path.as_ref()?;
            let is_astc = is_astc_path(path);
            let fp_opt = TextureFingerprint::compute(entity, path, tier);

            // 1. Cache Hit incremental en disco: misma huella, mismo tier y WebP existente
            if out_dir.is_some() {
                if let (Some(fp), Some(cached)) = (fp_opt.as_ref(), prev_textures.get(&entity.id)) {
                    if &cached.fingerprint == fp
                        && cached.compiled_webp_path.exists()
                        && is_webp_path(&cached.compiled_webp_path)
                    {
                        cached_reused_count.fetch_add(1, Ordering::Relaxed);
                        vram_saved_bytes.fetch_add(cached.vram_saved_bytes, Ordering::Relaxed);
                        if cached.downscaled {
                            downscaled_count.fetch_add(1, Ordering::Relaxed);
                        } else {
                            skipped_optimal_count.fetch_add(1, Ordering::Relaxed);
                        }
                        let completed = done_counter.fetch_add(1, Ordering::Relaxed) + 1;
                        let pct = 0.22 + 0.43 * ((completed as f32) / (total_entities as f32));
                        on_progress(
                            pct,
                            &format!("Caché WebP reutilizada ({}/{})", completed, total_entities),
                        );
                        return Some(CompiledTexOutcome::CacheHit(
                            entity.id.clone(),
                            cached.clone(),
                        ));
                    }
                }
            }

            // 2. Si el sprite ya es .webp nativo y su escala inteligente es 1.0, pasar directo 1:1
            if let Some((orig_w, orig_h)) = read_sprite_dimensions(path) {
                let smart_scale = compute_smart_texture_scale(entity, orig_w, orig_h, tier);
                if smart_scale >= 0.999 && !is_astc && is_webp_path(path) {
                    skipped_optimal_count.fetch_add(1, Ordering::Relaxed);
                    let completed = done_counter.fetch_add(1, Ordering::Relaxed) + 1;
                    let pct = 0.22 + 0.43 * ((completed as f32) / (total_entities as f32));
                    on_progress(
                        pct,
                        &format!("WebP óptimo directo ({}/{})", completed, total_entities),
                    );
                    return Some(CompiledTexOutcome::PassThrough(entity.id.clone(), path.clone()));
                }
            }

            // 3. Cargar imagen origen (.png, .jpg, .astc, etc.) y compilar a WebP sin pérdida
            let img = load_sprite_image(path).ok()?;
            let orig_w = img.width();
            let orig_h = img.height();
            let smart_scale = compute_smart_texture_scale(entity, orig_w, orig_h, tier);

            if smart_scale >= 0.999 && !is_astc && is_webp_path(path) {
                skipped_optimal_count.fetch_add(1, Ordering::Relaxed);
                let completed = done_counter.fetch_add(1, Ordering::Relaxed) + 1;
                let pct = 0.22 + 0.43 * ((completed as f32) / (total_entities as f32));
                on_progress(
                    pct,
                    &format!("WebP óptimo directo ({}/{})", completed, total_entities),
                );
                return Some(CompiledTexOutcome::PassThrough(entity.id.clone(), path.clone()));
            }

            let (final_img, scale_applied, downscaled, saved_bytes) = if smart_scale < 0.999 {
                let new_w = ((orig_w as f32) * smart_scale).round().max(1.0) as u32;
                let new_h = ((orig_h as f32) * smart_scale).round().max(1.0) as u32;
                let orig_bytes = (orig_w as usize) * (orig_h as usize) * 4;
                let new_bytes = (new_w as usize) * (new_h as usize) * 4;
                let saved = orig_bytes.saturating_sub(new_bytes);
                vram_saved_bytes.fetch_add(saved, Ordering::Relaxed);
                downscaled_count.fetch_add(1, Ordering::Relaxed);
                (
                    img.resize_exact(new_w, new_h, FilterType::CatmullRom),
                    smart_scale,
                    true,
                    saved,
                )
            } else {
                skipped_optimal_count.fetch_add(1, Ordering::Relaxed);
                (img, 1.0, false, 0)
            };

            let completed = done_counter.fetch_add(1, Ordering::Relaxed) + 1;
            let pct = 0.22 + 0.43 * ((completed as f32) / (total_entities as f32));
            on_progress(
                pct,
                &format!(
                    "Comprimiendo sprites a WebP sin pérdida ({}/{})...",
                    completed, total_entities
                ),
            );

            if let Some(dir) = out_dir {
                let target_webp = dir.join(format!("{}.webp", entity.id));
                if save_lossless_webp(&final_img, &target_webp).is_ok() {
                    webp_encoded_count.fetch_add(1, Ordering::Relaxed);
                    // Limpiar formato heredado `.png` si existía de versiones previas del caché
                    let legacy_png = dir.join(format!("{}.png", entity.id));
                    if legacy_png.exists() {
                        let _ = std::fs::remove_file(&legacy_png);
                    }
                    let entry = fp_opt.map(|fingerprint| CachedTextureEntry {
                        entity_id: entity.id.clone(),
                        fingerprint,
                        compiled_webp_path: target_webp.clone(),
                        downscaled,
                        vram_saved_bytes: saved_bytes,
                    });
                    return Some(CompiledTexOutcome::DiskWebp {
                        entity_id: entity.id.clone(),
                        entry,
                        path: target_webp,
                    });
                }
            }

            let rgba8 = final_img.to_rgba8();
            let w = rgba8.width();
            let h = rgba8.height();
            Some(CompiledTexOutcome::InMemory(
                entity.id.clone(),
                OptimizedTexture {
                    entity_id: entity.id.clone(),
                    width: w,
                    height: h,
                    scale_factor_applied: scale_applied,
                    rgba: rgba8.into_raw(),
                },
            ))
        })
        .collect();

    let mut textures = HashMap::new();
    let mut compiled_texture_paths = HashMap::new();
    let mut passthrough_paths = HashMap::new();
    let mut next_cached_textures = HashMap::new();

    for item in outcomes {
        match item {
            CompiledTexOutcome::CacheHit(id, entry) => {
                compiled_texture_paths.insert(id.clone(), entry.compiled_webp_path.clone());
                next_cached_textures.insert(id, entry);
            }
            CompiledTexOutcome::DiskWebp {
                entity_id,
                entry,
                path,
            } => {
                compiled_texture_paths.insert(entity_id.clone(), path);
                if let Some(e) = entry {
                    next_cached_textures.insert(entity_id, e);
                }
            }
            CompiledTexOutcome::InMemory(id, tex) => {
                textures.insert(id, tex);
            }
            CompiledTexOutcome::PassThrough(id, path) => {
                compiled_texture_paths.insert(id.clone(), path.clone());
                passthrough_paths.insert(id, path);
            }
        }
    }

    // Limpiar archivos `.webp` / `.png` huérfanos en `out_dir` de entidades eliminadas o pasadas a passthrough
    if let Some(dir) = out_dir {
        for (old_id, old_entry) in &manifest.textures {
            if !next_cached_textures.contains_key(old_id)
                && old_entry.compiled_webp_path.starts_with(dir)
            {
                let _ = std::fs::remove_file(&old_entry.compiled_webp_path);
            }
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for ent in entries.flatten() {
                let p = ent.path();
                if !p.is_file() {
                    continue;
                }
                let ext = p
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_ascii_lowercase())
                    .unwrap_or_default();
                if ext == "webp" || ext == "png" {
                    let keep = next_cached_textures
                        .values()
                        .any(|c| c.compiled_webp_path == p);
                    if !keep {
                        let _ = std::fs::remove_file(&p);
                    }
                }
            }
        }
    }
    manifest.textures = next_cached_textures;

    on_progress(0.68, "Rasterizando skin de flechas SVG con resvg...");
    let arrow_textures = rasterize_arrow_skin(&project.arrow_skin).unwrap_or_default();

    on_progress(0.72, "Sprites listos en WebP sin pérdida");

    let report = HotCompileReport {
        elapsed_ms: started.elapsed().as_millis() as u64,
        kept_entities: active_entities.len(),
        discarded_invisible_entities: discarded,
        downscaled_textures: downscaled_count.load(Ordering::Relaxed),
        skipped_optimal_textures: skipped_optimal_count.load(Ordering::Relaxed),
        cached_textures_reused: cached_reused_count.load(Ordering::Relaxed),
        webp_encoded_textures: webp_encoded_count.load(Ordering::Relaxed),
        discarded_heavy_loops,
        neutralized_unbounded_loops,
        estimated_vram_saved_kb: vram_saved_bytes.load(Ordering::Relaxed) / 1024,
        merged_audio_tracks: 0,
        compiled_audio_path: None,
        estimated_audio_ram_saved_kb: 0,
        cached_audio_reused: false,
        precompiled_scripts: 0,
        cached_scripts_reused: 0,
        script_precompile_errors: Vec::new(),
    };

    CompiledBundle {
        active_entities,
        textures,
        compiled_texture_paths,
        passthrough_paths,
        optimized_scripts,
        arrow_textures,
        report,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::EntityRole;
    use image::{DynamicImage, Rgba, RgbaImage};

    #[test]
    fn smart_scale_skips_optimal_sprites_and_downscales_oversized_ones() {
        let mut prop = StageEntity::new("bg", "Fondo", EntityRole::Background);
        prop.width = 1280.0;
        prop.height = 720.0;
        prop.scale = 1.0;

        assert_eq!(
            compute_smart_texture_scale(&prop, 1920, 1080, DeviceResourceTier::Balanced),
            1.0
        );

        let low_ratio =
            compute_smart_texture_scale(&prop, 4096, 2048, DeviceResourceTier::LowRam32Bit);
        assert!(low_ratio <= 1536.0 / 4096.0 + 0.001);

        let mut icon = StageEntity::new("icon", "Decoración", EntityRole::Decoration);
        icon.width = 200.0;
        icon.height = 200.0;
        icon.scale = 1.0;
        let ratio = compute_smart_texture_scale(&icon, 4096, 4096, DeviceResourceTier::Balanced);
        assert!(ratio < 0.6);
    }

    #[test]
    fn incremental_webp_cache_reuses_valid_hits_and_invalidates_on_tier_or_source_change() {
        let base_dir = std::env::temp_dir().join("beatforge_incremental_cache_test");
        let _ = std::fs::remove_dir_all(&base_dir);
        let cache_dir = base_dir.join("compiled_cache");
        std::fs::create_dir_all(&base_dir).unwrap();

        let src_png = base_dir.join("hero.png");
        let img = RgbaImage::from_pixel(64, 64, Rgba([20, 180, 240, 255]));
        DynamicImage::ImageRgba8(img)
            .save_with_format(&src_png, image::ImageFormat::Png)
            .unwrap();

        let mut project = ModProject::new_demo();
        project.stage.entities.clear();
        let mut hero = StageEntity::new("hero_1", "Hero", EntityRole::Player);
        hero.image_path = Some(src_png.clone());
        hero.width = 220.0;
        hero.height = 220.0;
        project.stage.entities.push(hero);

        // 1ra compilación: debe codificar .png -> .webp sin pérdida
        let b1 = hot_compile_project_to_dir(
            &project,
            DeviceResourceTier::Balanced,
            Some(&cache_dir),
            |_, _| {},
        );
        assert_eq!(b1.report.webp_encoded_textures, 1);
        assert_eq!(b1.report.cached_textures_reused, 0);
        let cached_webp = b1.compiled_texture_paths.get("hero_1").unwrap();
        assert_eq!(cached_webp.extension().and_then(|e| e.to_str()), Some("webp"));
        assert!(cached_webp.exists());

        // 2da compilación idéntica: Cache Hit sin recodificar
        let b2 = hot_compile_project_to_dir(
            &project,
            DeviceResourceTier::Balanced,
            Some(&cache_dir),
            |_, _| {},
        );
        assert_eq!(b2.report.webp_encoded_textures, 0);
        assert_eq!(b2.report.cached_textures_reused, 1);

        // Cambio de Tier: debe invalidar caché y recompilar
        let b3 = hot_compile_project_to_dir(
            &project,
            DeviceResourceTier::LowRam32Bit,
            Some(&cache_dir),
            |_, _| {},
        );
        assert_eq!(b3.report.webp_encoded_textures, 1);
        assert_eq!(b3.report.cached_textures_reused, 0);

        // Limpiar
        let _ = std::fs::remove_dir_all(&base_dir);
    }

    #[test]
    fn chart_events_preserve_hidden_entities_and_detect_reused_later_sprites() {
        let mut project = ModProject::new_demo();
        let mut alt_bg = StageEntity::new("bg_night", "Fondo Noche", EntityRole::Background);
        alt_bg.visible = false;
        project.stage.entities.push(alt_bg);
        project.scripts.clear();
        project.chart.events.push(crate::chart::ChartEvent {
            time_ms: 10000.0,
            name: "Stage Swap".to_string(),
            value1: "show=bg_night;hide=bg_day;dispose=1".to_string(),
            value2: String::new(),
        });
        project.chart.events.push(crate::chart::ChartEvent {
            time_ms: 25000.0,
            name: "Stage Swap".to_string(),
            value1: "show=bg_day;hide=bg_night;dispose=0".to_string(),
            value2: String::new(),
        });

        let refs = collect_script_referenced_ids(&project);
        assert!(refs.contains("bg_night"));
        assert!(!is_permanently_invisible(
            project
                .stage
                .entities
                .iter()
                .find(|e| e.id == "bg_night")
                .unwrap(),
            &refs,
            1280.0,
            720.0,
        ));

        let reused = analyze_reused_entity_ids(&project);
        assert!(reused.contains("bg_day"));
        assert!(!reused.contains("bg_night"));
    }
}
