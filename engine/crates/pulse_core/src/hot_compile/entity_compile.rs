//! Archivo: engine/crates/pulse_core/src/hot_compile/entity_compile.rs
//! Propósito: Compilación paralela de texturas por entidad (caché incremental por huella,
//! pass-through 1:1 WebP, reempaquetado Sparrow `crunch` multi-página, pre-escalado `Lanczos3`,
//! codificación WebP VP8L / RGB_565 / exoquant) y limpieza de archivos huérfanos en caché.

use super::analysis::compute_smart_texture_scale_for_viewport;
use super::aux_textures::AuxTexturesCompileResult;
use super::fingerprints::{DeviceResourceTier, ScreenViewportMetrics, TextureFingerprint};
use super::manifest::{
    CachedTextureEntry, HotCompileManifest, OptimizedTexture, TexturePixelFormat,
};
use crate::astc_loader::{
    WebpPixelPolicy, is_astc_path, is_webp_path, load_sprite_image, read_sprite_dimensions,
    save_compiled_webp,
};
use crate::atlas_repack::{AtlasExtraPage, optimize_and_repack_sparrow_atlas_for_viewport};
use crate::sparrow::SparrowAtlas;
use crate::stage::StageEntity;
use image::imageops::FilterType;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct AtlasRepackStats {
    pub(super) dead_frames_removed: usize,
    pub(super) duplicate_frames_merged: usize,
    pub(super) trimmed_frames: usize,
    pub(super) pruned_unused_anims: usize,
}

pub(super) enum CompiledTexOutcome {
    CacheHit(String, CachedTextureEntry),
    DiskWebp {
        entity_id: String,
        entry: Option<CachedTextureEntry>,
        path: PathBuf,
        extra_paths: Vec<PathBuf>,
        pixel_format: TexturePixelFormat,
        palette_quantized: bool,
        repacked_atlas: Option<SparrowAtlas>,
        repack_stats: Option<AtlasRepackStats>,
    },
    InMemory {
        entity_id: String,
        texture: OptimizedTexture,
        pixel_format: TexturePixelFormat,
        repacked_atlas: Option<SparrowAtlas>,
        repack_stats: Option<AtlasRepackStats>,
    },
    PassThrough {
        entity_id: String,
        path: PathBuf,
        entry: Option<CachedTextureEntry>,
    },
}

#[derive(Default)]
pub(super) struct CompileCounters {
    pub(super) done: AtomicUsize,
    pub(super) vram_saved_bytes: AtomicUsize,
    pub(super) downscaled: AtomicUsize,
    pub(super) skipped_optimal: AtomicUsize,
    pub(super) cached_reused: AtomicUsize,
    pub(super) webp_encoded: AtomicUsize,
    pub(super) dead_frames: AtomicUsize,
    pub(super) dup_frames: AtomicUsize,
    pub(super) trimmed_frames: AtomicUsize,
    pub(super) pruned_anims: AtomicUsize,
    pub(super) rgb565_textures: AtomicUsize,
    pub(super) quantized_textures: AtomicUsize,
    pub(super) split_atlas_pages_total: AtomicUsize,
}

pub(super) struct EntityCompileContext<'a, F> {
    pub(super) tier: DeviceResourceTier,
    pub(super) norm_viewport: ScreenViewportMetrics,
    pub(super) viewport_scale: f32,
    pub(super) max_camera_zoom: f32,
    pub(super) virtual_width: f32,
    pub(super) virtual_height: f32,
    pub(super) out_dir: Option<&'a Path>,
    pub(super) prev_textures: &'a HashMap<String, CachedTextureEntry>,
    pub(super) referenced_anim_prefixes: &'a HashSet<String>,
    pub(super) total_entities: usize,
    pub(super) counters: &'a CompileCounters,
    pub(super) on_progress: &'a F,
}

fn build_passthrough_cache_entry(
    entity_id: &str,
    path: &Path,
    fp_opt: Option<TextureFingerprint>,
    out_dir: Option<&Path>,
) -> Option<CachedTextureEntry> {
    if out_dir.is_none() {
        return None;
    }
    fp_opt.map(|fingerprint| CachedTextureEntry {
        entity_id: entity_id.to_string(),
        fingerprint,
        compiled_webp_path: path.to_path_buf(),
        extra_page_paths: Vec::new(),
        pixel_format: TexturePixelFormat::Argb8888,
        palette_quantized: false,
        downscaled: false,
        vram_saved_bytes: 0,
        atlas_repacked: false,
        repacked_atlas: None,
        dead_frames_removed: 0,
        duplicate_frames_merged: 0,
        trimmed_frames: 0,
        pruned_unused_anims: 0,
    })
}

fn select_webp_pixel_policy(
    has_atlas: bool,
    tier: DeviceResourceTier,
    width: u32,
    height: u32,
) -> WebpPixelPolicy {
    if !has_atlas {
        // Si es 100 % opaca, `save_compiled_webp` aplicará dithering RGB_565 y devolverá RGB8;
        // si tiene canal alfa real, `save_compiled_webp` cae limpiamente en RGBA8.
        return WebpPixelPolicy::Rgb565DitheredIfOpaque;
    }
    if tier == DeviceResourceTier::LowRam32Bit
        || (tier == DeviceResourceTier::Balanced && width.max(height) >= 1024)
    {
        WebpPixelPolicy::QuantizedRgba { max_colors: 256 }
    } else {
        WebpPixelPolicy::LosslessRgba
    }
}

/// Procesa una entidad de escenario dentro del pool paralelo `rayon`:
/// intenta reutilizar caché en disco, pass-through 1:1 o reempaquetar/codificar WebP.
pub(super) fn compile_single_entity_texture<F>(
    entity: &StageEntity,
    ctx: &EntityCompileContext<'_, F>,
) -> Option<CompiledTexOutcome>
where
    F: Fn(f32, &str) + Send + Sync,
{
    let path = entity.image_path.as_ref()?;
    let is_astc = is_astc_path(path);
    let fp_opt = TextureFingerprint::compute_with_context(
        entity,
        path,
        ctx.tier,
        Some(ctx.referenced_anim_prefixes),
        ctx.norm_viewport,
        ctx.max_camera_zoom,
    );

    // 1. Cache Hit incremental en disco: misma huella, mismo tier y todas las páginas WebP existentes
    if ctx.out_dir.is_some() {
        if let (Some(fp), Some(cached)) = (fp_opt.as_ref(), ctx.prev_textures.get(&entity.id)) {
            let all_extra_ok = cached
                .extra_page_paths
                .iter()
                .all(|p| p.exists() && is_webp_path(p));
            if &cached.fingerprint == fp
                && cached.compiled_webp_path.exists()
                && is_webp_path(&cached.compiled_webp_path)
                && all_extra_ok
            {
                ctx.counters.cached_reused.fetch_add(1, Ordering::Relaxed);
                ctx.counters
                    .vram_saved_bytes
                    .fetch_add(cached.vram_saved_bytes, Ordering::Relaxed);
                ctx.counters
                    .dead_frames
                    .fetch_add(cached.dead_frames_removed, Ordering::Relaxed);
                ctx.counters
                    .dup_frames
                    .fetch_add(cached.duplicate_frames_merged, Ordering::Relaxed);
                ctx.counters
                    .trimmed_frames
                    .fetch_add(cached.trimmed_frames, Ordering::Relaxed);
                ctx.counters
                    .pruned_anims
                    .fetch_add(cached.pruned_unused_anims, Ordering::Relaxed);
                if cached.pixel_format == TexturePixelFormat::Rgb565 {
                    ctx.counters.rgb565_textures.fetch_add(1, Ordering::Relaxed);
                }
                if cached.palette_quantized {
                    ctx.counters
                        .quantized_textures
                        .fetch_add(1, Ordering::Relaxed);
                }
                if !cached.extra_page_paths.is_empty() {
                    ctx.counters
                        .split_atlas_pages_total
                        .fetch_add(1 + cached.extra_page_paths.len(), Ordering::Relaxed);
                }
                if cached.downscaled {
                    ctx.counters.downscaled.fetch_add(1, Ordering::Relaxed);
                } else {
                    ctx.counters.skipped_optimal.fetch_add(1, Ordering::Relaxed);
                }
                let completed = ctx.counters.done.fetch_add(1, Ordering::Relaxed) + 1;
                let pct = 0.20 + 0.40 * ((completed as f32) / (ctx.total_entities as f32));
                (ctx.on_progress)(
                    pct,
                    &format!("Imagen lista al instante ({}/{})", completed, ctx.total_entities),
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
            let smart_scale = compute_smart_texture_scale_for_viewport(
                entity,
                orig_w,
                orig_h,
                ctx.tier,
                ctx.norm_viewport,
                ctx.max_camera_zoom,
                ctx.virtual_width,
                ctx.virtual_height,
            );
            if smart_scale >= 0.999 && !is_astc && is_webp_path(path) {
                ctx.counters.skipped_optimal.fetch_add(1, Ordering::Relaxed);
                let completed = ctx.counters.done.fetch_add(1, Ordering::Relaxed) + 1;
                let pct = 0.20 + 0.40 * ((completed as f32) / (ctx.total_entities as f32));
                (ctx.on_progress)(
                    pct,
                    &format!("Imagen lista ({}/{})", completed, ctx.total_entities),
                );
                let entry =
                    build_passthrough_cache_entry(&entity.id, path, fp_opt, ctx.out_dir);
                return Some(CompiledTexOutcome::PassThrough {
                    entity_id: entity.id.clone(),
                    path: path.clone(),
                    entry,
                });
            }
        }
    }

    // 3. Cargar imagen origen (.png, .jpg, .astc, .webp)
    let img = load_sprite_image(path).ok()?;
    let orig_w = img.width();
    let orig_h = img.height();

    // 3A. Si tiene SparrowAtlas, eliminar frames muertos, deduplicar, podar y paginar si excede hoja
    let (
        final_img,
        extra_pages,
        scale_applied,
        downscaled,
        saved_bytes,
        repacked_atlas,
        repack_stats,
    ): (
        _,
        Vec<AtlasExtraPage>,
        f32,
        bool,
        usize,
        Option<SparrowAtlas>,
        Option<AtlasRepackStats>,
    ) = if let Some(orig_atlas) = &entity.atlas {
        let atlas_cap = ctx.tier.max_dim_for_role(true, false);
        if let Some(repack) = optimize_and_repack_sparrow_atlas_for_viewport(
            entity,
            orig_atlas,
            &img,
            atlas_cap,
            Some(ctx.referenced_anim_prefixes),
            ctx.viewport_scale,
            ctx.max_camera_zoom,
        ) {
            if !repack.modified && !is_astc && is_webp_path(path) {
                ctx.counters.skipped_optimal.fetch_add(1, Ordering::Relaxed);
                let completed = ctx.counters.done.fetch_add(1, Ordering::Relaxed) + 1;
                let pct = 0.20 + 0.40 * ((completed as f32) / (ctx.total_entities as f32));
                (ctx.on_progress)(
                    pct,
                    &format!("Personaje listo ({}/{})", completed, ctx.total_entities),
                );
                let entry =
                    build_passthrough_cache_entry(&entity.id, path, fp_opt, ctx.out_dir);
                return Some(CompiledTexOutcome::PassThrough {
                    entity_id: entity.id.clone(),
                    path: path.clone(),
                    entry,
                });
            }

            let stats = AtlasRepackStats {
                dead_frames_removed: repack.dead_frames_removed,
                duplicate_frames_merged: repack.duplicate_frames_merged,
                trimmed_frames: repack.trimmed_frames,
                pruned_unused_anims: repack.pruned_unused_anims,
            };
            ctx.counters
                .dead_frames
                .fetch_add(stats.dead_frames_removed, Ordering::Relaxed);
            ctx.counters
                .dup_frames
                .fetch_add(stats.duplicate_frames_merged, Ordering::Relaxed);
            ctx.counters
                .trimmed_frames
                .fetch_add(stats.trimmed_frames, Ordering::Relaxed);
            ctx.counters
                .pruned_anims
                .fetch_add(stats.pruned_unused_anims, Ordering::Relaxed);
            ctx.counters
                .vram_saved_bytes
                .fetch_add(repack.vram_saved_bytes, Ordering::Relaxed);
            if repack.page_count > 1 {
                ctx.counters
                    .split_atlas_pages_total
                    .fetch_add(repack.page_count, Ordering::Relaxed);
            }

            let was_downscaled = repack.frame_scale_applied < 0.999;
            if was_downscaled {
                ctx.counters.downscaled.fetch_add(1, Ordering::Relaxed);
            } else {
                ctx.counters.skipped_optimal.fetch_add(1, Ordering::Relaxed);
            }

            (
                repack.image,
                repack.extra_pages,
                repack.frame_scale_applied,
                was_downscaled,
                repack.vram_saved_bytes,
                Some(repack.atlas),
                Some(stats),
            )
        } else {
            let smart_scale = compute_smart_texture_scale_for_viewport(
                entity,
                orig_w,
                orig_h,
                ctx.tier,
                ctx.norm_viewport,
                ctx.max_camera_zoom,
                ctx.virtual_width,
                ctx.virtual_height,
            );
            if smart_scale < 0.999 {
                let new_w = ((orig_w as f32) * smart_scale).round().max(1.0) as u32;
                let new_h = ((orig_h as f32) * smart_scale).round().max(1.0) as u32;
                let orig_bytes = (orig_w as usize) * (orig_h as usize) * 4;
                let new_bytes = (new_w as usize) * (new_h as usize) * 4;
                let saved = orig_bytes.saturating_sub(new_bytes);
                ctx.counters
                    .vram_saved_bytes
                    .fetch_add(saved, Ordering::Relaxed);
                ctx.counters.downscaled.fetch_add(1, Ordering::Relaxed);
                (
                    img.resize_exact(new_w, new_h, FilterType::Lanczos3),
                    Vec::new(),
                    smart_scale,
                    true,
                    saved,
                    None,
                    None,
                )
            } else {
                ctx.counters.skipped_optimal.fetch_add(1, Ordering::Relaxed);
                (img, Vec::new(), 1.0, false, 0, None, None)
            }
        }
    } else {
        // 3B. Fondos y props estáticos sin atlas
        let smart_scale = compute_smart_texture_scale_for_viewport(
            entity,
            orig_w,
            orig_h,
            ctx.tier,
            ctx.norm_viewport,
            ctx.max_camera_zoom,
            ctx.virtual_width,
            ctx.virtual_height,
        );
        if smart_scale >= 0.999 && !is_astc && is_webp_path(path) {
            ctx.counters.skipped_optimal.fetch_add(1, Ordering::Relaxed);
            let completed = ctx.counters.done.fetch_add(1, Ordering::Relaxed) + 1;
            let pct = 0.20 + 0.40 * ((completed as f32) / (ctx.total_entities as f32));
            (ctx.on_progress)(
                pct,
                &format!("Imagen lista ({}/{})", completed, ctx.total_entities),
            );
            let entry =
                build_passthrough_cache_entry(&entity.id, path, fp_opt, ctx.out_dir);
            return Some(CompiledTexOutcome::PassThrough {
                entity_id: entity.id.clone(),
                path: path.clone(),
                entry,
            });
        }

        if smart_scale < 0.999 {
            let new_w = ((orig_w as f32) * smart_scale).round().max(1.0) as u32;
            let new_h = ((orig_h as f32) * smart_scale).round().max(1.0) as u32;
            let orig_bytes = (orig_w as usize) * (orig_h as usize) * 4;
            let new_bytes = (new_w as usize) * (new_h as usize) * 4;
            let saved = orig_bytes.saturating_sub(new_bytes);
            ctx.counters
                .vram_saved_bytes
                .fetch_add(saved, Ordering::Relaxed);
            ctx.counters.downscaled.fetch_add(1, Ordering::Relaxed);
            (
                img.resize_exact(new_w, new_h, FilterType::Lanczos3),
                Vec::new(),
                smart_scale,
                true,
                saved,
                None,
                None,
            )
        } else {
            ctx.counters.skipped_optimal.fetch_add(1, Ordering::Relaxed);
            (img, Vec::new(), 1.0, false, 0, None, None)
        }
    };

    let completed = ctx.counters.done.fetch_add(1, Ordering::Relaxed) + 1;
    let pct = 0.20 + 0.40 * ((completed as f32) / (ctx.total_entities as f32));
    (ctx.on_progress)(
        pct,
        &format!(
            "Ajustando nitidez de imágenes ({}/{})...",
            completed, ctx.total_entities
        ),
    );

    let policy = select_webp_pixel_policy(
        entity.atlas.is_some(),
        ctx.tier,
        final_img.width(),
        final_img.height(),
    );

    if let Some(dir) = ctx.out_dir {
        let target_webp = dir.join(format!("{}.webp", entity.id));
        if let Ok(save_info) = save_compiled_webp(&final_img, &target_webp, policy) {
            ctx.counters.webp_encoded.fetch_add(1, Ordering::Relaxed);

            let pixel_format = if save_info.dithered_rgb565 {
                ctx.counters.rgb565_textures.fetch_add(1, Ordering::Relaxed);
                // RGB_565 consume 2 bytes/px frente a 4 bytes/px de ARGB_8888
                let rgb565_vram_bonus =
                    (final_img.width() as usize) * (final_img.height() as usize) * 2;
                ctx.counters
                    .vram_saved_bytes
                    .fetch_add(rgb565_vram_bonus, Ordering::Relaxed);
                TexturePixelFormat::Rgb565
            } else {
                TexturePixelFormat::Argb8888
            };
            let total_saved = if save_info.dithered_rgb565 {
                saved_bytes + (final_img.width() as usize) * (final_img.height() as usize) * 2
            } else {
                saved_bytes
            };

            let mut any_quantized = save_info.palette_quantized;
            let mut extra_paths = Vec::with_capacity(extra_pages.len());
            for extra in &extra_pages {
                let page_path = dir.join(format!("{}_p{}.webp", entity.id, extra.page_index));
                let extra_policy = select_webp_pixel_policy(
                    true,
                    ctx.tier,
                    extra.image.width(),
                    extra.image.height(),
                );
                if let Ok(ex_info) = save_compiled_webp(&extra.image, &page_path, extra_policy) {
                    any_quantized |= ex_info.palette_quantized;
                    extra_paths.push(page_path);
                }
            }
            if any_quantized {
                ctx.counters
                    .quantized_textures
                    .fetch_add(1, Ordering::Relaxed);
            }

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
                extra_page_paths: extra_paths.clone(),
                pixel_format,
                palette_quantized: any_quantized,
                downscaled,
                vram_saved_bytes: total_saved,
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
                extra_paths,
                pixel_format,
                palette_quantized: any_quantized,
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
        pixel_format: TexturePixelFormat::Argb8888,
        repacked_atlas,
        repack_stats,
    })
}

/// Elimina archivos `.webp` / `.png` y temporales huérfanos (`.webp.tmp`, `.tmp`) en `dir`
/// (incluyendo páginas `_p*.webp`, cuenta regresiva e íconos sobrantes).
pub(super) fn cleanup_orphan_compiled_assets(
    dir: &Path,
    old_manifest: &HotCompileManifest,
    next_cached_textures: &HashMap<String, CachedTextureEntry>,
    aux_result: &AuxTexturesCompileResult,
) {
    for (old_id, old_entry) in &old_manifest.textures {
        if !next_cached_textures.contains_key(old_id) {
            if old_entry.compiled_webp_path.starts_with(dir) {
                let _ = std::fs::remove_file(&old_entry.compiled_webp_path);
            }
            for ep in &old_entry.extra_page_paths {
                if ep.starts_with(dir) {
                    let _ = std::fs::remove_file(ep);
                }
            }
        }
    }
    for (old_slot, old_entry) in &old_manifest.countdown {
        if !aux_result.next_cached_countdown.contains_key(old_slot)
            && old_entry.compiled_webp_path.starts_with(dir)
        {
            let _ = std::fs::remove_file(&old_entry.compiled_webp_path);
        }
    }
    for (old_key, old_entry) in &old_manifest.icons {
        if !aux_result.next_cached_icons.contains_key(old_key)
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
            let file_name = p
                .file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.to_ascii_lowercase())
                .unwrap_or_default();
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .unwrap_or_default();
            if ext == "tmp" || file_name.ends_with(".tmp") || file_name.ends_with(".webp.tmp") {
                let _ = std::fs::remove_file(&p);
                continue;
            }
            if ext == "webp" || ext == "png" {
                let keep = next_cached_textures.values().any(|c| {
                    c.compiled_webp_path == p || c.extra_page_paths.iter().any(|ep| ep == &p)
                }) || aux_result
                    .next_cached_countdown
                    .values()
                    .any(|c| c.compiled_webp_path == p)
                    || aux_result
                        .next_cached_icons
                        .values()
                        .any(|c| c.compiled_webp_path == p);
                if !keep {
                    let _ = std::fs::remove_file(&p);
                }
            }
        }
    }
}
