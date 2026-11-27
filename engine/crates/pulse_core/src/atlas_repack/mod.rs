//! Archivo: engine/crates/pulse_core/src/atlas_repack/mod.rs
//! Propósito: Optimizador y reempaquetador automático de hojas de sprites (`SparrowAtlas`) para el
//! compilador en caliente (`hot_compile`). Coordina el recorte alfa, neutralización de frames
//! muertos, deduplicación exacta, poda segura, escalado `Lanczos3` y paginación multi-página.

mod packer;
mod trim_dedup;

#[cfg(test)]
mod tests;

use crate::sparrow::{AnimSet, SparrowAtlas, SparrowFrame};
use crate::stage::{EntityRole, StageEntity};
use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};
use std::collections::{BTreeMap, HashSet};

use packer::{pack_multi_page_with_crunch, pack_subset_with_crunch, pack_with_crunch};
use trim_dedup::{FrameSourceRef, analyze_and_dedup_atlas_frames, select_kept_animation_keys};

pub use trim_dedup::collect_referenced_anim_prefixes;

/// Página adicional de un atlas Sparrow dividido automáticamente cuando los frames nítidos
/// superan una sola hoja `tier_cap x tier_cap`.
#[derive(Debug, Clone)]
pub struct AtlasExtraPage {
    pub page_index: u16,
    pub image: DynamicImage,
}

/// Resultado del análisis, deduplicación y reempaquetado de un atlas Sparrow.
#[derive(Debug, Clone)]
pub struct AtlasRepackResult {
    /// Hoja principal (página 0) RGBA compacta lista para guardar en `.webp` sin pérdida.
    pub image: DynamicImage,
    /// Páginas adicionales (`_p1.webp`, `_p2.webp`, ...) cuando el atlas se divide en múltiples hojas.
    pub extra_pages: Vec<AtlasExtraPage>,
    /// Cantidad total de páginas generadas (`1 + extra_pages.len()`).
    pub page_count: usize,
    /// Atlas con coordenadas y `page_index` actualizados al nuevo lienzo (conservando tiempos y `bindings`).
    pub atlas: SparrowAtlas,
    /// Cantidad de frames muertos o 100 % transparentes neutralizados en `(0,0)` `1x1`.
    pub dead_frames_removed: usize,
    /// Cantidad de frames repetidos fusionados hacia una misma copia única.
    pub duplicate_frames_merged: usize,
    /// Cantidad de frames a los que se les recortó espacio transparente sobrante.
    pub trimmed_frames: usize,
    /// Cantidad de animaciones no usadas podadas del atlas.
    pub pruned_unused_anims: usize,
    /// Indica si hubo optimización real (frames removidos, fusionados, recortados, podados, divididos o reescalados).
    pub modified: bool,
    /// Factor de escala aplicado a los frames únicos (`1.0` = nitidez 1:1 nativa).
    pub frame_scale_applied: f32,
    /// Bytes de RGBA descomprimido en RAM ahorrados frente a la hoja original.
    pub vram_saved_bytes: usize,
}

/// Ejecuta recorte de transparencia, neutralización de frames muertos, deduplicación exacta de
/// frames repetidos, poda segura de animaciones sin uso y reempaquetado compacto con `crunch`
/// adaptado al tamaño real en pantalla + zoom máximo de cámara usando muestreo `Lanczos3`.
pub fn optimize_and_repack_sparrow_atlas_for_viewport(
    entity: &StageEntity,
    atlas: &SparrowAtlas,
    src_img: &DynamicImage,
    tier_cap: u32,
    referenced_anim_prefixes: Option<&HashSet<String>>,
    viewport_scale: f32,
    max_camera_zoom: f32,
) -> Option<AtlasRepackResult> {
    if atlas.animations.is_empty() {
        return None;
    }

    let src_rgba = src_img.to_rgba8();
    let (orig_w, orig_h) = src_rgba.dimensions();
    if orig_w == 0 || orig_h == 0 {
        return None;
    }

    // 1. Determinar animaciones activas (enlazadas en `bindings` o referenciadas por scripts/chart).
    let (kept_anim_keys, pruned_unused_anims) =
        select_kept_animation_keys(atlas, referenced_anim_prefixes);

    // 2. Recortar transparencia, detectar frames muertos y deduplicar píxeles idénticos.
    let dedup = analyze_and_dedup_atlas_frames(atlas, &src_rgba, &kept_anim_keys);

    // 3. Calcular mip-downscale inteligente de frames únicos según píxeles reales en pantalla + zoom máximo.
    let safe_vp = if viewport_scale.is_finite() {
        viewport_scale.clamp(0.45, 3.2)
    } else {
        1.0
    };
    let safe_zoom = if max_camera_zoom.is_finite() {
        max_camera_zoom.clamp(0.55, 2.5)
    } else {
        1.0
    };
    let screen_factor = (safe_vp * safe_zoom).clamp(0.45, 3.2);
    let display_max = (entity.width.max(entity.height) * entity.scale * screen_factor).max(128.0);
    let ideal_frame_max = (display_max * 1.45).max(384.0);
    let initial_scale = if !matches!(entity.role, EntityRole::Background)
        && (dedup.max_trimmed_frame_side as f32) > ideal_frame_max * 1.15
        && dedup.max_trimmed_frame_side > 384
    {
        (ideal_frame_max / (dedup.max_trimmed_frame_side as f32)).clamp(0.35, 1.0)
    } else {
        1.0
    };

    let clamped_initial = initial_scale.clamp(0.25, 1.0);
    let cap = tier_cap.max(64) as usize;
    let all_indices: Vec<usize> = (0..dedup.unique_sprites.len()).collect();

    // Si el atlas tiene >= 6 frames únicos y tope real de GPU (>= 1024 px), priorizamos dividirlo en
    // múltiples páginas nítidas (Multi-Page Sparrow Splitting) antes que aplastar su resolución.
    let (page_dims, applied_scale, placements): (
        Vec<(u32, u32)>,
        f32,
        Vec<(u16, u32, u32, u32, u32)>,
    ) = if let Some((w, h, single)) =
        pack_subset_with_crunch(&dedup.unique_sprites, &all_indices, clamped_initial, cap)
    {
        (
            vec![(w.min(tier_cap), h.min(tier_cap))],
            clamped_initial,
            single
                .into_iter()
                .map(|(px, py, sw, sh)| (0_u16, px, py, sw, sh))
                .collect(),
        )
    } else if tier_cap >= 1024 && dedup.unique_sprites.len() >= 6 {
        if let Some((pages, multi)) =
            pack_multi_page_with_crunch(&dedup.unique_sprites, clamped_initial, tier_cap, 6)
        {
            (pages, clamped_initial, multi)
        } else if let Some((pages, multi)) = pack_multi_page_with_crunch(
            &dedup.unique_sprites,
            (clamped_initial * 0.78).max(0.35),
            tier_cap,
            6,
        ) {
            (pages, (clamped_initial * 0.78).max(0.35), multi)
        } else {
            let (sheet_w, sheet_h, s, single) =
                pack_with_crunch(&dedup.unique_sprites, clamped_initial, tier_cap)?;
            (
                vec![(sheet_w, sheet_h)],
                s,
                single
                    .into_iter()
                    .map(|(px, py, sw, sh)| (0_u16, px, py, sw, sh))
                    .collect(),
            )
        }
    } else {
        let (sheet_w, sheet_h, s, single) =
            pack_with_crunch(&dedup.unique_sprites, clamped_initial, tier_cap)?;
        (
            vec![(sheet_w, sheet_h)],
            s,
            single
                .into_iter()
                .map(|(px, py, sw, sh)| (0_u16, px, py, sw, sh))
                .collect(),
        )
    };

    let orig_bytes = (orig_w as usize) * (orig_h as usize) * 4;
    let new_bytes: usize = page_dims
        .iter()
        .map(|&(pw, ph)| (pw.max(4) as usize) * (ph.max(4) as usize) * 4)
        .sum();
    let area_shrunk_significantly = (new_bytes as f64) <= (orig_bytes as f64) * 0.90;
    let is_multi_page = page_dims.len() > 1;

    let modified = dedup.dead_frames_removed > 0
        || dedup.duplicate_frames_merged > 0
        || dedup.trimmed_frames > 0
        || pruned_unused_anims > 0
        || dedup.had_rotated_frames
        || applied_scale < 0.999
        || is_multi_page
        || area_shrunk_significantly;

    // 4. Construir una o más hojas RGBA8 (fondo transparente [0,0,0,0], garantizando (0,0) vacío)
    // aplicando Lanczos3 en cada frame que requiera mip-downscale para preservar líneas limpias.
    let mut page_sheets: Vec<RgbaImage> = page_dims
        .iter()
        .map(|&(pw, ph)| RgbaImage::new(pw.max(4), ph.max(4)))
        .collect();
    for (idx, sp) in dedup.unique_sprites.iter().enumerate() {
        let (page_idx, dx, dy, dw, dh) = placements[idx];
        let src_tile = RgbaImage::from_raw(sp.w, sp.h, sp.rgba.clone())?;
        let final_tile = if dw != sp.w || dh != sp.h {
            image::imageops::resize(&src_tile, dw, dh, FilterType::Lanczos3)
        } else {
            src_tile
        };
        if let Some(sheet) = page_sheets.get_mut(page_idx as usize) {
            image::imageops::replace(sheet, &final_tile, dx as i64, dy as i64);
        }
    }

    // 5. Reconstruir `SparrowAtlas` con las nuevas coordenadas, `page_index` y escala exacta.
    let mut new_animations = BTreeMap::new();
    for (prefix, fps, looped, frame_refs) in dedup.anim_plan {
        let mut out_frames = Vec::with_capacity(frame_refs.len());
        for fref in frame_refs {
            match fref {
                FrameSourceRef::Dead {
                    raw_name,
                    frame_width,
                    frame_height,
                } => {
                    let sw = ((frame_width as f32) * applied_scale).round().max(1.0) as u32;
                    let sh = ((frame_height as f32) * applied_scale).round().max(1.0) as u32;
                    out_frames.push(SparrowFrame {
                        raw_name,
                        x: 0,
                        y: 0,
                        width: 1,
                        height: 1,
                        frame_x: 0,
                        frame_y: 0,
                        frame_width: sw,
                        frame_height: sh,
                        rotated: false,
                        page_index: 0,
                    });
                }
                FrameSourceRef::Unique {
                    raw_name,
                    unique_idx,
                    frame_x,
                    frame_y,
                    frame_width,
                    frame_height,
                } => {
                    let (page_idx, px, py, pw, ph) = placements[unique_idx];
                    let sfx = ((frame_x as f32) * applied_scale).round() as i32;
                    let sfy = ((frame_y as f32) * applied_scale).round() as i32;
                    let sfw = ((frame_width as f32) * applied_scale)
                        .round()
                        .max(pw as f32) as u32;
                    let sfh = ((frame_height as f32) * applied_scale)
                        .round()
                        .max(ph as f32) as u32;
                    out_frames.push(SparrowFrame {
                        raw_name,
                        x: px,
                        y: py,
                        width: pw,
                        height: ph,
                        frame_x: sfx,
                        frame_y: sfy,
                        frame_width: sfw,
                        frame_height: sfh,
                        rotated: false,
                        page_index: page_idx,
                    });
                }
            }
        }
        new_animations.insert(
            prefix.clone(),
            AnimSet {
                prefix,
                fps,
                looped,
                frames: out_frames,
            },
        );
    }

    let new_bindings = atlas
        .bindings
        .iter()
        .filter(|(_, pref)| new_animations.contains_key(*pref))
        .map(|(&k, pref)| (k, pref.clone()))
        .collect();

    let vram_saved_bytes = orig_bytes.saturating_sub(new_bytes);
    let page_count = page_sheets.len();
    let mut sheet_iter = page_sheets.into_iter();
    let primary_sheet = sheet_iter.next().unwrap_or_else(|| RgbaImage::new(4, 4));
    let extra_pages: Vec<AtlasExtraPage> = sheet_iter
        .enumerate()
        .map(|(offset, img)| AtlasExtraPage {
            page_index: (offset + 1) as u16,
            image: DynamicImage::ImageRgba8(img),
        })
        .collect();

    Some(AtlasRepackResult {
        image: DynamicImage::ImageRgba8(primary_sheet),
        extra_pages,
        page_count,
        atlas: SparrowAtlas {
            image_path_hint: atlas.image_path_hint.clone(),
            animations: new_animations,
            bindings: new_bindings,
        },
        dead_frames_removed: dedup.dead_frames_removed,
        duplicate_frames_merged: dedup.duplicate_frames_merged,
        trimmed_frames: dedup.trimmed_frames,
        pruned_unused_anims,
        modified,
        frame_scale_applied: applied_scale,
        vram_saved_bytes,
    })
}

/// Wrapper compatible para reempaquetar atlas Sparrow usando escala de pantalla `1.0` y zoom `1.0`.
pub fn optimize_and_repack_sparrow_atlas(
    entity: &StageEntity,
    atlas: &SparrowAtlas,
    src_img: &DynamicImage,
    tier_cap: u32,
    referenced_anim_prefixes: Option<&HashSet<String>>,
) -> Option<AtlasRepackResult> {
    optimize_and_repack_sparrow_atlas_for_viewport(
        entity,
        atlas,
        src_img,
        tier_cap,
        referenced_anim_prefixes,
        1.0,
        1.0,
    )
}
