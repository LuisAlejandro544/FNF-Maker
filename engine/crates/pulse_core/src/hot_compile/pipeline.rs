//! Archivo: engine/crates/pulse_core/src/hot_compile/pipeline.rs
//! Propósito: Orquestador de la tubería paralela de compilación en caliente (`hot_compile_project*`),
//! delegando la caché de scripts (`script_cache`), la compilación de texturas (`entity_compile`),
//! auxiliares (`aux_textures`) y rasterizado de flechas SVG.

use super::analysis::{
    EntityLifecycleWindow, collect_script_referenced_ids, compute_project_max_camera_zoom,
    compute_timeline_asset_lifecycle, is_permanently_invisible,
};
use super::aux_textures::compile_countdown_and_icons_with_manifest;
use super::entity_compile::{
    CompileCounters, CompiledTexOutcome, EntityCompileContext, cleanup_orphan_compiled_assets,
    compile_single_entity_texture,
};
use super::fingerprints::{DeviceResourceTier, ScreenViewportMetrics};
use super::manifest::{
    CompiledBundle, HotCompileManifest, HotCompileReport, MANIFEST_SCHEMA_VERSION,
    TexturePixelFormat,
};
use super::script_cache::optimize_scripts_with_manifest;
use crate::atlas_repack::collect_referenced_anim_prefixes;
use crate::project::ModProject;
use crate::sparrow::SparrowAtlas;
use crate::svg_skin::rasterize_arrow_skin;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Instant;

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
    hot_compile_project_to_dir_with_viewport(
        project,
        tier,
        ScreenViewportMetrics::default(),
        out_dir,
        on_progress,
    )
}

/// Compila en caliente hacia `out_dir` usando la resolución real de pantalla (`ScreenViewportMetrics`).
pub fn hot_compile_project_to_dir_with_viewport<F>(
    project: &ModProject,
    tier: DeviceResourceTier,
    viewport: ScreenViewportMetrics,
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
    let bundle = hot_compile_project_to_dir_with_manifest_viewport(
        project,
        tier,
        viewport,
        out_dir,
        &mut manifest,
        on_progress,
    );
    if let Some(dir) = out_dir {
        let _ = manifest.save_to_dir(dir);
    }
    bundle
}

/// Compila en caliente permitiendo compartir un `HotCompileManifest` externo (compatibilidad 1280x720).
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
    hot_compile_project_to_dir_with_manifest_viewport(
        project,
        tier,
        ScreenViewportMetrics::default(),
        out_dir,
        manifest,
        on_progress,
    )
}

/// Compila en caliente permitiendo compartir un `HotCompileManifest` externo y la resolución física
/// real de la pantalla para pre-escalado inteligente (`Lanczos3`) ajustado al zoom máximo.
pub fn hot_compile_project_to_dir_with_manifest_viewport<F>(
    project: &ModProject,
    tier: DeviceResourceTier,
    viewport: ScreenViewportMetrics,
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
    let norm_viewport = viewport.normalized();
    let max_camera_zoom = compute_project_max_camera_zoom(project);
    let viewport_scale = norm_viewport.virtual_to_pixel_scale(
        project.stage.virtual_width,
        project.stage.virtual_height,
    );

    on_progress(0.06, "Revisando eventos y efectos del mod...");
    let (optimized_scripts, discarded_heavy_loops, neutralized_unbounded_loops) =
        optimize_scripts_with_manifest(&project.scripts, manifest);

    on_progress(0.14, "Organizando apariciones en el escenario...");

    let timeline_windows: Vec<EntityLifecycleWindow> = compute_timeline_asset_lifecycle(project);
    let scheduled_preload_entities = timeline_windows
        .iter()
        .filter(|w| w.preload_at_ms.is_some())
        .count();
    let scheduled_retire_entities = timeline_windows
        .iter()
        .filter(|w| w.retire_after_ms.is_some())
        .count();

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

    on_progress(0.22, "Preparando personajes y fondos...");

    let referenced_anim_prefixes =
        collect_referenced_anim_prefixes(&active_entities, &project.scripts, &project.chart);

    let counters = CompileCounters::default();
    let ctx = EntityCompileContext {
        tier,
        norm_viewport,
        viewport_scale,
        max_camera_zoom,
        virtual_width: project.stage.virtual_width,
        virtual_height: project.stage.virtual_height,
        out_dir,
        prev_textures: &manifest.textures,
        referenced_anim_prefixes: &referenced_anim_prefixes,
        total_entities: active_entities.len().max(1),
        counters: &counters,
        on_progress: &on_progress,
    };

    let outcomes: Vec<CompiledTexOutcome> = active_entities
        .par_iter()
        .filter_map(|entity| compile_single_entity_texture(entity, &ctx))
        .collect();

    let mut textures = HashMap::new();
    let mut compiled_texture_paths = HashMap::new();
    let mut compiled_texture_pages: HashMap<String, Vec<PathBuf>> = HashMap::new();
    let mut texture_pixel_formats: HashMap<String, TexturePixelFormat> = HashMap::new();
    let mut passthrough_paths = HashMap::new();
    let mut next_cached_textures = HashMap::new();
    let mut repacked_atlases_map: HashMap<String, SparrowAtlas> = HashMap::new();
    let mut repacked_entity_ids: Vec<String> = Vec::new();

    for item in outcomes {
        match item {
            CompiledTexOutcome::CacheHit(id, entry) => {
                let mut pages = Vec::with_capacity(1 + entry.extra_page_paths.len());
                pages.push(entry.compiled_webp_path.clone());
                pages.extend(entry.extra_page_paths.iter().cloned());
                compiled_texture_paths.insert(id.clone(), entry.compiled_webp_path.clone());
                compiled_texture_pages.insert(id.clone(), pages);
                texture_pixel_formats.insert(id.clone(), entry.pixel_format);
                if let Some(dir) = out_dir {
                    if !entry.compiled_webp_path.starts_with(dir) {
                        passthrough_paths.insert(id.clone(), entry.compiled_webp_path.clone());
                    }
                }
                if entry.atlas_repacked {
                    if let Some(atlas) = entry.repacked_atlas.clone() {
                        repacked_atlases_map.insert(id.clone(), atlas);
                        repacked_entity_ids.push(id.clone());
                    }
                }
                next_cached_textures.insert(id, entry);
            }
            CompiledTexOutcome::DiskWebp {
                entity_id,
                entry,
                path,
                extra_paths,
                pixel_format,
                palette_quantized: _,
                repacked_atlas,
                repack_stats: _,
            } => {
                let mut pages = Vec::with_capacity(1 + extra_paths.len());
                pages.push(path.clone());
                pages.extend(extra_paths);
                compiled_texture_paths.insert(entity_id.clone(), path);
                compiled_texture_pages.insert(entity_id.clone(), pages);
                texture_pixel_formats.insert(entity_id.clone(), pixel_format);
                if let Some(atlas) = repacked_atlas {
                    repacked_atlases_map.insert(entity_id.clone(), atlas);
                    repacked_entity_ids.push(entity_id.clone());
                }
                if let Some(e) = entry {
                    next_cached_textures.insert(entity_id, e);
                }
            }
            CompiledTexOutcome::InMemory {
                entity_id,
                texture,
                pixel_format,
                repacked_atlas,
                repack_stats: _,
            } => {
                texture_pixel_formats.insert(entity_id.clone(), pixel_format);
                if let Some(atlas) = repacked_atlas {
                    repacked_atlases_map.insert(entity_id.clone(), atlas);
                    repacked_entity_ids.push(entity_id.clone());
                }
                textures.insert(entity_id, texture);
            }
            CompiledTexOutcome::PassThrough {
                entity_id,
                path,
                entry,
            } => {
                compiled_texture_paths.insert(entity_id.clone(), path.clone());
                compiled_texture_pages.insert(entity_id.clone(), vec![path.clone()]);
                texture_pixel_formats.insert(entity_id.clone(), TexturePixelFormat::Argb8888);
                passthrough_paths.insert(entity_id.clone(), path);
                if let Some(e) = entry {
                    next_cached_textures.insert(entity_id, e);
                }
            }
        }
    }

    for ent in &mut active_entities {
        if let Some(repacked) = repacked_atlases_map.remove(&ent.id) {
            ent.atlas = Some(repacked);
        }
    }

    on_progress(0.65, "Optimizando cuenta regresiva e íconos...");
    let aux_result = compile_countdown_and_icons_with_manifest(
        project,
        &active_entities,
        tier,
        out_dir,
        &manifest.countdown,
        &manifest.icons,
    );
    counters
        .vram_saved_bytes
        .fetch_add(aux_result.vram_saved_bytes, Ordering::Relaxed);

    if let Some(dir) = out_dir {
        cleanup_orphan_compiled_assets(dir, manifest, &next_cached_textures, &aux_result);
    }
    manifest.textures = next_cached_textures;
    manifest.countdown = aux_result.next_cached_countdown;
    manifest.icons = aux_result.next_cached_icons;

    on_progress(0.68, "Preparando estilo de flechas...");
    let arrow_textures = rasterize_arrow_skin(&project.arrow_skin).unwrap_or_default();

    on_progress(0.72, "Imágenes y escenario preparados");

    let repacked_atlases = repacked_entity_ids.len();
    let report = HotCompileReport {
        elapsed_ms: started.elapsed().as_millis() as u64,
        kept_entities: active_entities.len(),
        discarded_invisible_entities: discarded,
        downscaled_textures: counters.downscaled.load(Ordering::Relaxed),
        skipped_optimal_textures: counters.skipped_optimal.load(Ordering::Relaxed),
        cached_textures_reused: counters.cached_reused.load(Ordering::Relaxed),
        webp_encoded_textures: counters.webp_encoded.load(Ordering::Relaxed),
        discarded_heavy_loops,
        neutralized_unbounded_loops,
        estimated_vram_saved_kb: counters.vram_saved_bytes.load(Ordering::Relaxed) / 1024,
        dead_frames_removed: counters.dead_frames.load(Ordering::Relaxed),
        duplicate_frames_merged: counters.dup_frames.load(Ordering::Relaxed),
        trimmed_frames: counters.trimmed_frames.load(Ordering::Relaxed),
        pruned_unused_anims: counters.pruned_anims.load(Ordering::Relaxed),
        repacked_atlases,
        repacked_entity_ids,
        rgb565_textures: counters.rgb565_textures.load(Ordering::Relaxed),
        quantized_textures: counters.quantized_textures.load(Ordering::Relaxed),
        split_atlas_pages_total: counters.split_atlas_pages_total.load(Ordering::Relaxed),
        scheduled_preload_entities,
        scheduled_retire_entities,
        compiled_countdown_count: aux_result.compiled_countdown_count,
        compiled_icon_count: aux_result.compiled_icon_count,
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
        compiled_texture_pages,
        texture_pixel_formats,
        passthrough_paths,
        compiled_countdown_paths: aux_result.countdown_paths,
        compiled_icon_normal_paths: aux_result.icon_normal_paths,
        compiled_icon_losing_paths: aux_result.icon_losing_paths,
        timeline_windows,
        optimized_scripts,
        arrow_textures,
        report,
    }
}
