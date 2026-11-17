//! Archivo: engine/crates/pulse_core/src/hot_compile.rs
//! Propósito: Compilador en caliente paralelo (`rayon`) inteligente que se ejecuta antes de jugar.
//! Funciones clave:
//! 1. Analiza scripts Lua, JS y TypeScript (`script_optimizer`) para descartar bucles pesados sin
//!    lógica necesaria o neutralizar bucles infinitos que consumirían demasiados recursos.
//! 2. Analiza visibilidad para DESCARTAR objetos que permanecerán invisibles toda la canción.
//! 3. Evalúa de forma inteligente la resolución de cada sprite respecto a su tamaño real en pantalla
//!    y la capacidad del teléfono (optimizado para 32-bit `armeabi-v7a` y 2-3 GB RAM):
//!    si ya es óptima, pasa directo sin recompresión; si requiere ajuste, escribe el PNG optimizado
//!    directo a caché en disco dentro del pool `rayon` sin retener buffers RGBA duplicados en RAM.
//! 4. Rasteriza la skin de flechas `.svg` en paralelo y entrega un `CompiledBundle` listo para juego.

use crate::astc_loader::{is_astc_path, load_sprite_image, read_sprite_dimensions};
use crate::chart::NoteDirection;
use crate::project::{ModProject, ScriptAsset};
use crate::script_optimizer::optimize_scripts_for_hot_compile;
use crate::stage::StageEntity;
use crate::svg_skin::{RasterizedArrow, rasterize_arrow_skin};
use image::ImageFormat;
use image::imageops::FilterType;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

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
    pub discarded_heavy_loops: usize,
    pub neutralized_unbounded_loops: usize,
    pub estimated_vram_saved_kb: usize,
    pub merged_audio_tracks: usize,
    pub compiled_audio_path: Option<PathBuf>,
    pub estimated_audio_ram_saved_kb: usize,
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

/// Recorre las fuentes de todos los scripts activos para saber qué IDs de entidad
/// podrían volverse visibles o moverse dinámicamente durante la canción.
pub fn collect_script_referenced_ids(project: &ModProject) -> HashSet<String> {
    let mut referenced = HashSet::new();
    for script in project.scripts.iter().filter(|s| s.enabled) {
        for entity in &project.stage.entities {
            if script.source_code.contains(&entity.id) {
                referenced.insert(entity.id.clone());
            }
        }
    }
    referenced
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
/// Devuelve `1.0` cuando NO conviene tocarla (mantiene calidad original y evita recompresión).
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

    // Si la reducción sería menor al 8% (ratio >= 0.92), no vale la pena degradar ni recomprimir.
    if target_ratio >= 0.92 && largest <= tier_cap {
        1.0
    } else {
        target_ratio.clamp(0.25, 1.0)
    }
}

enum CompiledTexOutcome {
    DiskCached(String, PathBuf),
    InMemory(String, OptimizedTexture),
    PassThrough(String, PathBuf),
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

/// Compila en caliente escribiendo las texturas reescaladas/decodificadas directamente a `out_dir`
/// en disco para que NO queden copias RGBA duplicadas en la RAM nativa de Rust.
pub fn hot_compile_project_to_dir<F>(
    project: &ModProject,
    tier: DeviceResourceTier,
    out_dir: Option<&Path>,
    on_progress: F,
) -> CompiledBundle
where
    F: Fn(f32, &str) + Send + Sync,
{
    let started = Instant::now();
    on_progress(0.08, "Analizando bucles en scripts Lua, TS y JS...");

    let (optimized_scripts, script_report) = optimize_scripts_for_hot_compile(&project.scripts);

    on_progress(0.18, "Analizando visibilidad y objetos del escenario...");

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

    on_progress(0.35, "Evaluando resolución inteligente de sprites...");

    let total_entities = active_entities.len().max(1);
    let done_counter = Arc::new(AtomicUsize::new(0));
    let vram_saved_bytes = Arc::new(AtomicUsize::new(0));
    let downscaled_count = Arc::new(AtomicUsize::new(0));
    let skipped_optimal_count = Arc::new(AtomicUsize::new(0));

    let outcomes: Vec<CompiledTexOutcome> = active_entities
        .par_iter()
        .filter_map(|entity| {
            let path = entity.image_path.as_ref()?;
            let is_astc = is_astc_path(path);

            // Lectura ultrarrápida de cabecera para decidir si realmente hace falta decodificar y reescalar
            if let Some((orig_w, orig_h)) = read_sprite_dimensions(path) {
                let smart_scale = compute_smart_texture_scale(entity, orig_w, orig_h, tier);
                if smart_scale >= 0.999 && !is_astc {
                    skipped_optimal_count.fetch_add(1, Ordering::Relaxed);
                    let completed = done_counter.fetch_add(1, Ordering::Relaxed) + 1;
                    let pct = 0.35 + 0.45 * ((completed as f32) / (total_entities as f32));
                    on_progress(pct, "Textura óptima detectada (sin pérdida)...");
                    return Some(CompiledTexOutcome::PassThrough(entity.id.clone(), path.clone()));
                }
            }

            let img = load_sprite_image(path).ok()?;
            let orig_w = img.width();
            let orig_h = img.height();
            let smart_scale = compute_smart_texture_scale(entity, orig_w, orig_h, tier);

            if smart_scale >= 0.999 && !is_astc {
                skipped_optimal_count.fetch_add(1, Ordering::Relaxed);
                let completed = done_counter.fetch_add(1, Ordering::Relaxed) + 1;
                let pct = 0.35 + 0.45 * ((completed as f32) / (total_entities as f32));
                on_progress(pct, "Textura óptima detectada (sin pérdida)...");
                return Some(CompiledTexOutcome::PassThrough(entity.id.clone(), path.clone()));
            }

            let (final_img, scale_applied) = if smart_scale < 0.999 {
                let new_w = ((orig_w as f32) * smart_scale).round().max(1.0) as u32;
                let new_h = ((orig_h as f32) * smart_scale).round().max(1.0) as u32;
                let orig_bytes = (orig_w as usize) * (orig_h as usize) * 4;
                let new_bytes = (new_w as usize) * (new_h as usize) * 4;
                vram_saved_bytes.fetch_add(orig_bytes.saturating_sub(new_bytes), Ordering::Relaxed);
                downscaled_count.fetch_add(1, Ordering::Relaxed);
                (img.resize_exact(new_w, new_h, FilterType::CatmullRom), smart_scale)
            } else {
                (img, 1.0)
            };

            let completed = done_counter.fetch_add(1, Ordering::Relaxed) + 1;
            let pct = 0.35 + 0.45 * ((completed as f32) / (total_entities as f32));
            on_progress(pct, "Optimizando spritesheets necesarios...");

            // Si hay carpeta de caché en disco, escribir el PNG ahí y liberar RGBA inmediatamente
            // para evitar retener doble/triple copia de texturas en RAM.
            if let Some(dir) = out_dir {
                let target_file = dir.join(format!("{}.png", entity.id));
                if final_img
                    .save_with_format(&target_file, ImageFormat::Png)
                    .is_ok()
                {
                    return Some(CompiledTexOutcome::DiskCached(entity.id.clone(), target_file));
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
    for item in outcomes {
        match item {
            CompiledTexOutcome::DiskCached(id, path) => {
                compiled_texture_paths.insert(id, path);
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

    on_progress(0.85, "Rasterizando skin de flechas SVG con resvg...");
    let arrow_textures = rasterize_arrow_skin(&project.arrow_skin).unwrap_or_default();

    on_progress(1.0, "Compilación en caliente completada");

    let report = HotCompileReport {
        elapsed_ms: started.elapsed().as_millis() as u64,
        kept_entities: active_entities.len(),
        discarded_invisible_entities: discarded,
        downscaled_textures: downscaled_count.load(Ordering::Relaxed),
        skipped_optimal_textures: skipped_optimal_count.load(Ordering::Relaxed),
        discarded_heavy_loops: script_report.discarded_heavy_loops,
        neutralized_unbounded_loops: script_report.neutralized_unbounded_loops,
        estimated_vram_saved_kb: vram_saved_bytes.load(Ordering::Relaxed) / 1024,
        merged_audio_tracks: 0,
        compiled_audio_path: None,
        estimated_audio_ram_saved_kb: 0,
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

    #[test]
    fn smart_scale_skips_optimal_sprites_and_downscales_oversized_ones() {
        let mut prop = StageEntity::new("bg", "Fondo", EntityRole::Background);
        prop.width = 1280.0;
        prop.height = 720.0;
        prop.scale = 1.0;

        // 1920x1080 en pantalla 1280x720 es óptimo -> no tocar (1.0)
        assert_eq!(
            compute_smart_texture_scale(&prop, 1920, 1080, DeviceResourceTier::Balanced),
            1.0
        );

        // En modo 32-bit/RAM baja un fondo 4096 se ajusta a <=1536px
        let low_ratio =
            compute_smart_texture_scale(&prop, 4096, 2048, DeviceResourceTier::LowRam32Bit);
        assert!(low_ratio <= 1536.0 / 4096.0 + 0.001);

        // 4096x4096 para un ícono de 200x200 -> sí reduce para ahorrar VRAM manteniendo nitidez
        let mut icon = StageEntity::new("icon", "Decoración", EntityRole::Decoration);
        icon.width = 200.0;
        icon.height = 200.0;
        icon.scale = 1.0;
        let ratio = compute_smart_texture_scale(&icon, 4096, 4096, DeviceResourceTier::Balanced);
        assert!(ratio < 0.6);
    }
}
