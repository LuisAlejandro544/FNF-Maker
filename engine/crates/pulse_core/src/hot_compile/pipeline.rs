//! Archivo: engine/crates/pulse_core/src/hot_compile/pipeline.rs
//! Propósito: Ejecuta la tubería paralela de compilación en caliente (`hot_compile_project*`),
//! optimizando scripts con caché por hash, reempaquetando atlas Sparrow, codificando WebP VP8L
//! sin pérdida mediante `rayon`, limpiando artefactos huérfanos y rasterizando flechas SVG.

use super::analysis::{
    collect_script_referenced_ids, compute_smart_texture_scale, is_permanently_invisible,
};
use super::fingerprints::{DeviceResourceTier, TextureFingerprint, fnv1a_64_str};
use super::manifest::{
    CachedScriptEntry, CachedTextureEntry, CompiledBundle, HotCompileManifest, HotCompileReport,
    MANIFEST_SCHEMA_VERSION, OptimizedTexture, SCRIPT_OPTIMIZER_CACHE_VERSION,
};
use crate::astc_loader::{
    is_astc_path, is_webp_path, load_sprite_image, read_sprite_dimensions, save_optimized_webp,
};
use crate::atlas_repack::{collect_referenced_anim_prefixes, optimize_and_repack_sparrow_atlas};
use crate::project::{ModProject, ScriptAsset};
use crate::script_optimizer::optimize_single_script;
use crate::sparrow::SparrowAtlas;
use crate::svg_skin::rasterize_arrow_skin;
use image::imageops::FilterType;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default)]
struct AtlasRepackStats {
    dead_frames_removed: usize,
    duplicate_frames_merged: usize,
    trimmed_frames: usize,
    pruned_unused_anims: usize,
}

enum CompiledTexOutcome {
    CacheHit(String, CachedTextureEntry),
    DiskWebp {
        entity_id: String,
        entry: Option<CachedTextureEntry>,
        path: PathBuf,
        repacked_atlas: Option<SparrowAtlas>,
        repack_stats: Option<AtlasRepackStats>,
    },
    InMemory {
        entity_id: String,
        texture: OptimizedTexture,
        repacked_atlas: Option<SparrowAtlas>,
        repack_stats: Option<AtlasRepackStats>,
    },
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

    on_progress(0.22, "Deduplicando frames y compilando WebP sin pérdida...");

    let referenced_anim_prefixes =
        collect_referenced_anim_prefixes(&active_entities, &project.scripts, &project.chart);

    let total_entities = active_entities.len().max(1);
    let done_counter = Arc::new(AtomicUsize::new(0));
    let vram_saved_bytes = Arc::new(AtomicUsize::new(0));
    let downscaled_count = Arc::new(AtomicUsize::new(0));
    let skipped_optimal_count = Arc::new(AtomicUsize::new(0));
    let cached_reused_count = Arc::new(AtomicUsize::new(0));
    let webp_encoded_count = Arc::new(AtomicUsize::new(0));
    let dead_frames_count = Arc::new(AtomicUsize::new(0));
    let dup_frames_count = Arc::new(AtomicUsize::new(0));
    let trimmed_frames_count = Arc::new(AtomicUsize::new(0));
    let pruned_anims_count = Arc::new(AtomicUsize::new(0));

    let prev_textures = &manifest.textures;

    let outcomes: Vec<CompiledTexOutcome> = active_entities
        .par_iter()
        .filter_map(|entity| {
            let path = entity.image_path.as_ref()?;
            let is_astc = is_astc_path(path);
            let fp_opt = TextureFingerprint::compute_with_refs(
                entity,
                path,
                tier,
                Some(&referenced_anim_prefixes),
            );

            // 1. Cache Hit incremental en disco: misma huella, mismo tier y WebP existente
            if out_dir.is_some() {
                if let (Some(fp), Some(cached)) = (fp_opt.as_ref(), prev_textures.get(&entity.id)) {
                    if &cached.fingerprint == fp
                        && cached.compiled_webp_path.exists()
                        && is_webp_path(&cached.compiled_webp_path)
                    {
                        cached_reused_count.fetch_add(1, Ordering::Relaxed);
                        vram_saved_bytes.fetch_add(cached.vram_saved_bytes, Ordering::Relaxed);
                        dead_frames_count.fetch_add(cached.dead_frames_removed, Ordering::Relaxed);
                        dup_frames_count.fetch_add(cached.duplicate_frames_merged, Ordering::Relaxed);
                        trimmed_frames_count.fetch_add(cached.trimmed_frames, Ordering::Relaxed);
                        pruned_anims_count.fetch_add(cached.pruned_unused_anims, Ordering::Relaxed);
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

            // 2. Si NO tiene atlas y ya es .webp nativo con escala inteligente 1.0, pasar directo 1:1
            if entity.atlas.is_none() {
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
                        return Some(CompiledTexOutcome::PassThrough(
                            entity.id.clone(),
                            path.clone(),
                        ));
                    }
                }
            }

            // 3. Cargar imagen origen (.png, .jpg, .astc, .webp)
            let img = load_sprite_image(path).ok()?;
            let orig_w = img.width();
            let orig_h = img.height();

            // 3A. Si tiene SparrowAtlas, eliminar frames muertos, deduplicar repetidos, recortar alfa y reempaquetar
            let (final_img, scale_applied, downscaled, saved_bytes, repacked_atlas, repack_stats) =
                if let Some(orig_atlas) = &entity.atlas {
                    let atlas_cap = tier.max_dim_for_role(true, false);
                    if let Some(repack) = optimize_and_repack_sparrow_atlas(
                        entity,
                        orig_atlas,
                        &img,
                        atlas_cap,
                        Some(&referenced_anim_prefixes),
                    ) {
                        if !repack.modified && !is_astc && is_webp_path(path) {
                            skipped_optimal_count.fetch_add(1, Ordering::Relaxed);
                            let completed = done_counter.fetch_add(1, Ordering::Relaxed) + 1;
                            let pct = 0.22 + 0.43 * ((completed as f32) / (total_entities as f32));
                            on_progress(
                                pct,
                                &format!("Atlas WebP óptimo ({}/{})", completed, total_entities),
                            );
                            return Some(CompiledTexOutcome::PassThrough(
                                entity.id.clone(),
                                path.clone(),
                            ));
                        }

                        let stats = AtlasRepackStats {
                            dead_frames_removed: repack.dead_frames_removed,
                            duplicate_frames_merged: repack.duplicate_frames_merged,
                            trimmed_frames: repack.trimmed_frames,
                            pruned_unused_anims: repack.pruned_unused_anims,
                        };
                        dead_frames_count.fetch_add(stats.dead_frames_removed, Ordering::Relaxed);
                        dup_frames_count.fetch_add(stats.duplicate_frames_merged, Ordering::Relaxed);
                        trimmed_frames_count.fetch_add(stats.trimmed_frames, Ordering::Relaxed);
                        pruned_anims_count.fetch_add(stats.pruned_unused_anims, Ordering::Relaxed);
                        vram_saved_bytes.fetch_add(repack.vram_saved_bytes, Ordering::Relaxed);

                        let was_downscaled = repack.frame_scale_applied < 0.999;
                        if was_downscaled {
                            downscaled_count.fetch_add(1, Ordering::Relaxed);
                        } else {
                            skipped_optimal_count.fetch_add(1, Ordering::Relaxed);
                        }

                        (
                            repack.image,
                            repack.frame_scale_applied,
                            was_downscaled,
                            repack.vram_saved_bytes,
                            Some(repack.atlas),
                            Some(stats),
                        )
                    } else {
                        let smart_scale = compute_smart_texture_scale(entity, orig_w, orig_h, tier);
                        if smart_scale < 0.999 {
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
                                None,
                                None,
                            )
                        } else {
                            skipped_optimal_count.fetch_add(1, Ordering::Relaxed);
                            (img, 1.0, false, 0, None, None)
                        }
                    }
                } else {
                    // 3B. Fondos y props estáticos sin atlas
                    let smart_scale = compute_smart_texture_scale(entity, orig_w, orig_h, tier);
                    if smart_scale >= 0.999 && !is_astc && is_webp_path(path) {
                        skipped_optimal_count.fetch_add(1, Ordering::Relaxed);
                        let completed = done_counter.fetch_add(1, Ordering::Relaxed) + 1;
                        let pct = 0.22 + 0.43 * ((completed as f32) / (total_entities as f32));
                        on_progress(
                            pct,
                            &format!("WebP óptimo directo ({}/{})", completed, total_entities),
                        );
                        return Some(CompiledTexOutcome::PassThrough(
                            entity.id.clone(),
                            path.clone(),
                        ));
                    }

                    if smart_scale < 0.999 {
                        let new_w = ((orig_w as f32) * smart_scale).round().max(1.0) as u32;
                        let new_h = ((orig_h as f32) * smart_scale).round().max(1.0) as u32;
                        let orig_bytes = (orig_w as usize) * (orig_h as usize) * 4;
                        let new_bytes = (new_w as usize) * (new_h as usize) * 4;
                        let saved = orig_bytes.saturating_sub(new_bytes);
                        vram_saved_bytes.fetch_add(saved, Ordering::Relaxed);
                        downscaled_count.fetch_add(1, Ordering::Relaxed);
                        (
                            img.resize_exact(new_w, new_h, FilterType::Lanczos3),
                            smart_scale,
                            true,
                            saved,
                            None,
                            None,
                        )
                    } else {
                        skipped_optimal_count.fetch_add(1, Ordering::Relaxed);
                        (img, 1.0, false, 0, None, None)
                    }
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
                let strip_alpha = entity.atlas.is_none();
                if save_optimized_webp(&final_img, &target_webp, strip_alpha).is_ok() {
                    webp_encoded_count.fetch_add(1, Ordering::Relaxed);
                    // Limpiar formato heredado `.png` si existía de versiones previas del caché
                    let legacy_png = dir.join(format!("{}.png", entity.id));
                    if legacy_png.exists() {
                        let _ = std::fs::remove_file(&legacy_png);
                    }
                    let stats = repack_stats.unwrap_or_default();
                    let entry = fp_opt.map(|fingerprint| CachedTextureEntry {
                        entity_id: entity.id.clone(),
                        fingerprint,
                        compiled_webp_path: target_webp.clone(),
                        downscaled,
                        vram_saved_bytes: saved_bytes,
                        atlas_repacked: repacked_atlas.is_some(),
                        repacked_atlas: repacked_atlas.clone(),
                        dead_frames_removed: stats.dead_frames_removed,
                        duplicate_frames_merged: stats.duplicate_frames_merged,
                        trimmed_frames: stats.trimmed_frames,
                        pruned_unused_anims: stats.pruned_unused_anims,
                    });
                    return Some(CompiledTexOutcome::DiskWebp {
                        entity_id: entity.id.clone(),
                        entry,
                        path: target_webp,
                        repacked_atlas,
                        repack_stats,
                    });
                }
            }

            let rgba8 = final_img.to_rgba8();
            let w = rgba8.width();
            let h = rgba8.height();
            Some(CompiledTexOutcome::InMemory {
                entity_id: entity.id.clone(),
                texture: OptimizedTexture {
                    entity_id: entity.id.clone(),
                    width: w,
                    height: h,
                    scale_factor_applied: scale_applied,
                    rgba: rgba8.into_raw(),
                },
                repacked_atlas,
                repack_stats,
            })
        })
        .collect();

    let mut textures = HashMap::new();
    let mut compiled_texture_paths = HashMap::new();
    let mut passthrough_paths = HashMap::new();
    let mut next_cached_textures = HashMap::new();
    let mut repacked_atlases_map: HashMap<String, SparrowAtlas> = HashMap::new();
    let mut repacked_entity_ids: Vec<String> = Vec::new();

    for item in outcomes {
        match item {
            CompiledTexOutcome::CacheHit(id, entry) => {
                compiled_texture_paths.insert(id.clone(), entry.compiled_webp_path.clone());
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
                repacked_atlas,
                repack_stats: _,
            } => {
                compiled_texture_paths.insert(entity_id.clone(), path);
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
                repacked_atlas,
                repack_stats: _,
            } => {
                if let Some(atlas) = repacked_atlas {
                    repacked_atlases_map.insert(entity_id.clone(), atlas);
                    repacked_entity_ids.push(entity_id.clone());
                }
                textures.insert(entity_id, texture);
            }
            CompiledTexOutcome::PassThrough(id, path) => {
                compiled_texture_paths.insert(id.clone(), path.clone());
                passthrough_paths.insert(id, path);
            }
        }
    }

    for ent in &mut active_entities {
        if let Some(repacked) = repacked_atlases_map.remove(&ent.id) {
            ent.atlas = Some(repacked);
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

    on_progress(0.72, "Sprites deduplicados y listos en WebP sin pérdida");

    let repacked_atlases = repacked_entity_ids.len();
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
        dead_frames_removed: dead_frames_count.load(Ordering::Relaxed),
        duplicate_frames_merged: dup_frames_count.load(Ordering::Relaxed),
        trimmed_frames: trimmed_frames_count.load(Ordering::Relaxed),
        pruned_unused_anims: pruned_anims_count.load(Ordering::Relaxed),
        repacked_atlases,
        repacked_entity_ids,
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
