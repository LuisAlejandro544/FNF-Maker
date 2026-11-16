//! Archivo: engine/crates/pulse_core/src/hot_compile.rs
//! Propósito: Compilador en caliente paralelo (`rayon`) que se ejecuta durante la pantalla de carga
//! antes de iniciar la partida.
//! Funciones clave:
//! 1. Analiza scripts y visibilidad para DESCARTAR objetos que permanecerán invisibles toda la canción
//!    (opacidad 0 o desactivados sin ser manipulados por ningún script, o fuera de cámara).
//! 2. Ajusta y reescala texturas según el nivel de recursos del dispositivo (`LowRam32Bit`,
//!    `Balanced`, `HighQuality`) para evitar cierres por falta de memoria en teléfonos modestos.
//! 3. Rasteriza la skin de flechas `.svg` en paralelo y entrega un `CompiledBundle` optimizado.

use crate::chart::NoteDirection;
use crate::project::ModProject;
use crate::stage::StageEntity;
use crate::svg_skin::{RasterizedArrow, rasterize_arrow_skin};
use image::imageops::FilterType;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

/// Perfil de optimización según la memoria/arquitectura del teléfono del usuario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceResourceTier {
    /// Teléfonos 32-bit o poca RAM: máximo 1024px por textura.
    LowRam32Bit,
    /// Teléfonos gama media 64-bit: máximo 2048px por textura.
    Balanced,
    /// Gama alta: hasta 4096px sin pérdida visible.
    HighQuality,
}

impl DeviceResourceTier {
    pub fn max_texture_dim(self) -> u32 {
        match self {
            DeviceResourceTier::LowRam32Bit => 1024,
            DeviceResourceTier::Balanced => 2048,
            DeviceResourceTier::HighQuality => 4096,
        }
    }

    pub fn label_es(self) -> &'static str {
        match self {
            DeviceResourceTier::LowRam32Bit => "Económico (32-bit / <=1024px)",
            DeviceResourceTier::Balanced => "Balanceado (<=2048px)",
            DeviceResourceTier::HighQuality => "Máxima Calidad (<=4096px)",
        }
    }
}

/// Textura RGBA optimizada en RAM lista para cargarse a GPU.
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
    pub estimated_vram_saved_kb: usize,
}

/// Paquete final ya compilado y optimizado que consume la pantalla de gameplay.
#[derive(Debug, Clone)]
pub struct CompiledBundle {
    pub active_entities: Vec<StageEntity>,
    pub textures: HashMap<String, OptimizedTexture>,
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

    // Margen amplio para cámara/zoom: si está completamente lejos del escenario y ningún script la mueve, descartar.
    let half_w = (entity.width * entity.scale) * 0.5;
    let half_h = (entity.height * entity.scale) * 0.5;
    let margin = 450.0_f32;
    let out_x = (entity.x + half_w) < -margin || (entity.x - half_w) > (virtual_w + margin);
    let out_y = (entity.y + half_h) < -margin || (entity.y - half_h) > (virtual_h + margin);
    out_x || out_y
}

/// Ejecuta la compilación en caliente de todos los recursos del proyecto en paralelo usando `rayon`.
pub fn hot_compile_project<F>(
    project: &ModProject,
    tier: DeviceResourceTier,
    on_progress: F,
) -> CompiledBundle
where
    F: Fn(f32, &str) + Send + Sync,
{
    let started = Instant::now();
    on_progress(0.10, "Analizando visibilidad y scripts del escenario...");

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

    on_progress(0.35, "Optimizando y escalando hojas de sprites en paralelo...");

    let max_dim = tier.max_texture_dim();
    let total_entities = active_entities.len().max(1);
    let done_counter = Arc::new(AtomicUsize::new(0));
    let vram_saved_bytes = Arc::new(AtomicUsize::new(0));
    let downscaled_count = Arc::new(AtomicUsize::new(0));

    let textures_vec: Vec<(String, OptimizedTexture)> = active_entities
        .par_iter()
        .filter_map(|entity| {
            let path = entity.image_path.as_ref()?;
            let img = crate::astc_loader::load_sprite_image(path).ok()?;
            let orig_w = img.width();
            let orig_h = img.height();
            let largest = orig_w.max(orig_h);

            let (final_img, scale_applied) = if largest > max_dim {
                let ratio = (max_dim as f32) / (largest as f32);
                let new_w = ((orig_w as f32) * ratio).round().max(1.0) as u32;
                let new_h = ((orig_h as f32) * ratio).round().max(1.0) as u32;
                let orig_bytes = (orig_w as usize) * (orig_h as usize) * 4;
                let new_bytes = (new_w as usize) * (new_h as usize) * 4;
                vram_saved_bytes.fetch_add(orig_bytes.saturating_sub(new_bytes), Ordering::Relaxed);
                downscaled_count.fetch_add(1, Ordering::Relaxed);
                (img.resize_exact(new_w, new_h, FilterType::Triangle), ratio)
            } else {
                (img, 1.0)
            };

            let rgba8 = final_img.to_rgba8();
            let w = rgba8.width();
            let h = rgba8.height();

            let completed = done_counter.fetch_add(1, Ordering::Relaxed) + 1;
            let pct = 0.35 + 0.45 * ((completed as f32) / (total_entities as f32));
            on_progress(pct, "Optimizando spritesheets...");

            Some((
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

    let textures: HashMap<String, OptimizedTexture> = textures_vec.into_iter().collect();

    on_progress(0.85, "Rasterizando skin de flechas SVG con resvg...");
    let arrow_textures = rasterize_arrow_skin(&project.arrow_skin).unwrap_or_default();

    on_progress(1.0, "Compilación en caliente completada");

    let report = HotCompileReport {
        elapsed_ms: started.elapsed().as_millis() as u64,
        kept_entities: active_entities.len(),
        discarded_invisible_entities: discarded,
        downscaled_textures: downscaled_count.load(Ordering::Relaxed),
        estimated_vram_saved_kb: vram_saved_bytes.load(Ordering::Relaxed) / 1024,
    };

    CompiledBundle {
        active_entities,
        textures,
        arrow_textures,
        report,
    }
}
