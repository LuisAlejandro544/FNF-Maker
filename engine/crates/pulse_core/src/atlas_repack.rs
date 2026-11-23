//! Archivo: engine/crates/pulse_core/src/atlas_repack.rs
//! Propósito: Optimizador y reempaquetador automático de hojas de sprites (`SparrowAtlas`) para el
//! compilador en caliente (`hot_compile`). Elimina el peso y desperdicio típico de mods FNF:
//! 1. Recorte estricto de transparencia (*alpha trim*) por fotograma, ajustando `frame_x` / `frame_y`
//!    sin alterar el lienzo lógico unificado (`frame_width`, `frame_height`) ni los pies del personaje.
//! 2. Detección de frames muertos o vacíos (`width == 0`, `height == 0` o 100 % transparentes):
//!    conserva la entrada temporal en `anim.frames` para no romper la duración/ritmo de la animación,
//!    apuntándolos a un único píxel transparente `1x1` compartido en `(0, 0)`.
//! 3. Deduplicación exacta byte-a-byte (canonizando RGB en píxeles con alfa `0` + hash FNV-1a):
//!    si múltiples frames tienen el mismo contenido visual (incluso si venían rotados `+90°` o con
//!    distinto offset `frame_x`/`frame_y`), se almacena una sola copia física en la textura final.
//! 4. Poda segura de animaciones huérfanas que no están enlazadas en `atlas.bindings` ni
//!    referenciadas en scripts habilitados o eventos del chart.
//! 5. Reempaquetado compacto usando el crate `crunch` (`MIT OR Apache-2.0`) con margen de `2 px`
//!    entre cuadros y escalado adaptativo limpio dentro del tope `tier_cap` del dispositivo.

use crate::chart::ChartData;
use crate::project::ScriptAsset;
use crate::sparrow::{AnimSet, SparrowAtlas, SparrowFrame};
use crate::stage::{EntityRole, StageEntity};
use crunch::{Item, Rect as CrunchRect, Rotation};
use image::imageops::FilterType;
use image::{DynamicImage, RgbaImage};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Umbral mínimo de opacidad (`alpha >= 2`) para considerar un píxel visible durante el recorte.
/// Descarta ruido invisible `alpha = 0..1` que algunos exportadores dejan en bordes transparentes.
const ALPHA_TRIM_THRESHOLD: u8 = 2;

/// Espacio de separación entre frames empaquetados (1 px por cada lado = 2 px entre vecinos)
/// para evitar sangrado de bordes con filtros lineales y garantizar que `(0, 0)` quede `[0,0,0,0]`.
const FRAME_EDGE_PAD: u32 = 1;

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

#[derive(Debug, Clone)]
struct UniqueSprite {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
enum FrameSourceRef {
    Dead {
        raw_name: String,
        frame_width: u32,
        frame_height: u32,
    },
    Unique {
        raw_name: String,
        unique_idx: usize,
        frame_x: i32,
        frame_y: i32,
        frame_width: u32,
        frame_height: u32,
    },
}

#[inline]
fn fnv1a_rgba_with_dims(w: u32, h: u32, rgba: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for bytes in [w.to_le_bytes().as_slice(), h.to_le_bytes().as_slice(), rgba] {
        for &b in bytes {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    hash
}

/// Recolecta qué prefijos de animación de las entidades del proyecto son mencionados
/// explícitamente en scripts habilitados o en eventos del chart (`chart.events`).
pub fn collect_referenced_anim_prefixes(
    entities: &[StageEntity],
    scripts: &[ScriptAsset],
    chart: &ChartData,
) -> HashSet<String> {
    let mut keep = HashSet::new();
    let mut candidates = HashSet::new();
    for ent in entities {
        if let Some(atlas) = &ent.atlas {
            for prefix in atlas.animations.keys() {
                candidates.insert(prefix.clone());
            }
        }
    }
    if candidates.is_empty() {
        return keep;
    }

    for prefix in candidates {
        let lower_prefix = prefix.to_ascii_lowercase();
        let in_script = scripts
            .iter()
            .filter(|s| s.enabled)
            .any(|s| s.source_code.contains(&prefix) || s.source_code.to_ascii_lowercase().contains(&lower_prefix));
        if in_script {
            keep.insert(prefix);
            continue;
        }
        let in_events = chart.events.iter().any(|ev| {
            ev.name.contains(&prefix)
                || ev.value1.contains(&prefix)
                || ev.value2.contains(&prefix)
                || ev.value1.to_ascii_lowercase().contains(&lower_prefix)
                || ev.value2.to_ascii_lowercase().contains(&lower_prefix)
        });
        if in_events {
            keep.insert(prefix);
        }
    }

    keep
}

/// Extrae el buffer RGBA8 en orientación normal (`rotated = false` tras aplicar `rotate270`)
/// de un `SparrowFrame`. Devuelve `None` si la región es inválida o vacía.
fn extract_upright_frame_rgba(
    src_rgba: &RgbaImage,
    frame: &SparrowFrame,
) -> Option<RgbaImage> {
    let (img_w, img_h) = src_rgba.dimensions();
    if frame.width == 0 || frame.height == 0 || frame.x >= img_w || frame.y >= img_h {
        return None;
    }
    let crop_w = frame.width.min(img_w - frame.x);
    let crop_h = frame.height.min(img_h - frame.y);
    if crop_w == 0 || crop_h == 0 {
        return None;
    }
    let cropped = image::imageops::crop_imm(src_rgba, frame.x, frame.y, crop_w, crop_h).to_image();
    if frame.rotated {
        Some(image::imageops::rotate270(&cropped))
    } else {
        Some(cropped)
    }
}

/// Busca el rectángulo mínimo que encierra píxeles con `alpha >= ALPHA_TRIM_THRESHOLD`.
/// Devuelve `None` si el frame completo está muerto / transparente.
fn find_opaque_bounds(img: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let (w, h) = img.dimensions();
    let raw = img.as_raw();
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x_excl = 0_u32;
    let mut max_y_excl = 0_u32;

    for y in 0..h {
        let row_off = (y as usize) * (w as usize) * 4;
        for x in 0..w {
            let a = raw[row_off + (x as usize) * 4 + 3];
            if a >= ALPHA_TRIM_THRESHOLD {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x_excl = max_x_excl.max(x + 1);
                max_y_excl = max_y_excl.max(y + 1);
            }
        }
    }

    if max_x_excl > min_x && max_y_excl > min_y {
        Some((min_x, min_y, max_x_excl, max_y_excl))
    } else {
        None
    }
}

/// Empaqueta un subconjunto `indices` de `unique_sprites` a `scale` dentro de `<= cap x cap`
/// eligiendo el lienzo cuadrado más compacto que logre acomodar todos los rectángulos.
fn pack_subset_with_crunch(
    unique_sprites: &[UniqueSprite],
    indices: &[usize],
    scale: f32,
    cap: usize,
) -> Option<(u32, u32, Vec<(u32, u32, u32, u32)>)> {
    if indices.is_empty() {
        return Some((4, 4, Vec::new()));
    }
    let pad_both = (FRAME_EDGE_PAD * 2) as usize;
    let mut scaled_dims: Vec<(usize, usize)> = Vec::with_capacity(indices.len());
    let mut total_area: usize = 0;
    let mut max_w: usize = 1;
    let mut max_h: usize = 1;

    for &idx in indices {
        let sp = &unique_sprites[idx];
        let sw = ((sp.w as f32) * scale).round().max(1.0) as usize;
        let sh = ((sp.h as f32) * scale).round().max(1.0) as usize;
        let pw = sw + pad_both;
        let ph = sh + pad_both;
        if pw > cap || ph > cap {
            return None;
        }
        total_area = total_area.saturating_add(pw.saturating_mul(ph));
        max_w = max_w.max(pw);
        max_h = max_h.max(ph);
        scaled_dims.push((sw, sh));
    }

    let sqrt_area = ((total_area as f64) * 1.12).sqrt().ceil() as usize;
    let mut candidates: Vec<(usize, usize)> = Vec::new();
    for mult in [100_usize, 112, 126, 145, 175] {
        let side = ((sqrt_area * mult) / 100)
            .max(max_w)
            .max(max_h)
            .div_ceil(64)
            * 64;
        let clamped = side.min(cap);
        if !candidates.contains(&(clamped, clamped)) {
            candidates.push((clamped, clamped));
        }
    }
    if !candidates.contains(&(cap, cap)) {
        candidates.push((cap, cap));
    }

    for (bin_w, bin_h) in candidates {
        let items: Vec<Item<usize>> = scaled_dims
            .iter()
            .enumerate()
            .map(|(local_idx, &(sw, sh))| {
                Item::new(local_idx, sw + pad_both, sh + pad_both, Rotation::None)
            })
            .collect();

        if let Ok(packed) = crunch::pack(
            CrunchRect {
                x: 0,
                y: 0,
                w: bin_w,
                h: bin_h,
            },
            items,
        ) {
            let mut placements = vec![(0_u32, 0_u32, 1_u32, 1_u32); indices.len()];
            let mut used_w = 4_u32;
            let mut used_h = 4_u32;

            for p in packed {
                let (sw, sh) = scaled_dims[p.data];
                let px = (p.rect.x as u32) + FRAME_EDGE_PAD;
                let py = (p.rect.y as u32) + FRAME_EDGE_PAD;
                placements[p.data] = (px, py, sw as u32, sh as u32);
                used_w = used_w.max(px + (sw as u32) + FRAME_EDGE_PAD);
                used_h = used_h.max(py + (sh as u32) + FRAME_EDGE_PAD);
            }

            return Some((
                used_w.min(cap as u32),
                used_h.min(cap as u32),
                placements,
            ));
        }
    }

    None
}

/// Comprueba rápido si el rango `[start..end)` cabe en una sola hoja `cap x cap`.
fn range_fits_in_cap(
    unique_sprites: &[UniqueSprite],
    start: usize,
    end: usize,
    scale: f32,
    cap: usize,
) -> bool {
    if start >= end {
        return true;
    }
    let pad_both = (FRAME_EDGE_PAD * 2) as usize;
    let mut area_sum = 0_usize;
    let mut items = Vec::with_capacity(end - start);
    for (local_idx, sp) in unique_sprites[start..end].iter().enumerate() {
        let sw = ((sp.w as f32) * scale).round().max(1.0) as usize;
        let sh = ((sp.h as f32) * scale).round().max(1.0) as usize;
        let pw = sw + pad_both;
        let ph = sh + pad_both;
        if pw > cap || ph > cap {
            return false;
        }
        area_sum = area_sum.saturating_add(pw.saturating_mul(ph));
        if area_sum > cap.saturating_mul(cap) {
            return false;
        }
        items.push(Item::new(local_idx, pw, ph, Rotation::None));
    }
    crunch::pack(
        CrunchRect {
            x: 0,
            y: 0,
            w: cap,
            h: cap,
        },
        items,
    )
    .is_ok()
}

/// Divide un mega-atlas Sparrow en `2..=max_pages` hojas ordenadas por secuencia de animación
/// manteniendo la escala óptima en pantalla (`scale`) sin sacrificar nitidez.
fn pack_multi_page_with_crunch(
    unique_sprites: &[UniqueSprite],
    scale: f32,
    tier_cap: u32,
    max_pages: usize,
) -> Option<(Vec<(u32, u32)>, Vec<(u16, u32, u32, u32, u32)>)> {
    if unique_sprites.is_empty() {
        return Some((vec![(4, 4)], Vec::new()));
    }
    let cap = tier_cap.max(64) as usize;
    let total = unique_sprites.len();
    let mut page_dims: Vec<(u32, u32)> = Vec::new();
    let mut placements = vec![(0_u16, 0_u32, 0_u32, 1_u32, 1_u32); total];
    let mut cursor = 0_usize;

    for page_idx in 0..max_pages {
        if cursor >= total {
            break;
        }
        if !range_fits_in_cap(unique_sprites, cursor, cursor + 1, scale, cap) {
            return None;
        }

        let best_end = if range_fits_in_cap(unique_sprites, cursor, total, scale, cap) {
            total
        } else {
            let mut lo = cursor + 1;
            let mut hi = total - 1;
            let mut best = cursor + 1;
            while lo <= hi {
                let mid = lo + (hi - lo) / 2;
                if range_fits_in_cap(unique_sprites, cursor, mid, scale, cap) {
                    best = mid;
                    lo = mid + 1;
                } else if mid == 0 {
                    break;
                } else {
                    hi = mid - 1;
                }
            }
            best
        };

        let indices: Vec<usize> = (cursor..best_end).collect();
        let (pw, ph, local_places) = pack_subset_with_crunch(unique_sprites, &indices, scale, cap)?;
        page_dims.push((pw, ph));
        for (offset, (px, py, sw, sh)) in local_places.into_iter().enumerate() {
            placements[cursor + offset] = (page_idx as u16, px, py, sw, sh);
        }
        cursor = best_end;
    }

    if cursor < total || page_dims.len() <= 1 {
        return None;
    }

    Some((page_dims, placements))
}

/// Empaqueta rectángulos únicos con `crunch` dentro de `<= tier_cap x tier_cap`, reduciendo
/// `frame_scale` solo cuando sea necesario para que quepan en una sola hoja.
fn pack_with_crunch(
    unique_sprites: &[UniqueSprite],
    initial_scale: f32,
    tier_cap: u32,
) -> Option<(u32, u32, f32, Vec<(u32, u32, u32, u32)>)> {
    if unique_sprites.is_empty() {
        return Some((4, 4, initial_scale, Vec::new()));
    }

    let cap = tier_cap.max(64) as usize;
    let indices: Vec<usize> = (0..unique_sprites.len()).collect();
    let mut scale = initial_scale.clamp(0.25, 1.0);

    for _attempt in 0..7 {
        if let Some((w, h, places)) = pack_subset_with_crunch(unique_sprites, &indices, scale, cap) {
            return Some((w.min(tier_cap), h.min(tier_cap), scale, places));
        }
        scale *= 0.84;
        if scale < 0.24 {
            break;
        }
    }

    None
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
    let bound_prefixes: HashSet<&str> = atlas.bindings.values().map(|s| s.as_str()).collect();
    let mut kept_anim_keys: Vec<String> = Vec::with_capacity(atlas.animations.len());
    let mut pruned_unused_anims = 0_usize;

    for prefix in atlas.animations.keys() {
        let is_bound = bound_prefixes.contains(prefix.as_str());
        let is_script_ref = referenced_anim_prefixes.is_some_and(|set| set.contains(prefix));
        if bound_prefixes.is_empty() || is_bound || is_script_ref || referenced_anim_prefixes.is_none()
        {
            kept_anim_keys.push(prefix.clone());
        } else {
            pruned_unused_anims += 1;
        }
    }
    if kept_anim_keys.is_empty() {
        kept_anim_keys = atlas.animations.keys().cloned().collect();
        pruned_unused_anims = 0;
    }

    // 2. Recortar transparencia, detectar frames muertos y deduplicar píxeles idénticos.
    let mut unique_sprites: Vec<UniqueSprite> = Vec::new();
    let mut dedup_buckets: HashMap<(u64, u32, u32), Vec<usize>> = HashMap::new();
    let mut anim_plan: Vec<(String, f32, bool, Vec<FrameSourceRef>)> =
        Vec::with_capacity(kept_anim_keys.len());

    let mut dead_frames_removed = 0_usize;
    let mut duplicate_frames_merged = 0_usize;
    let mut trimmed_frames = 0_usize;
    let mut max_trimmed_frame_side = 1_u32;
    let mut had_rotated_frames = false;

    for prefix in &kept_anim_keys {
        let Some(anim) = atlas.animations.get(prefix) else {
            continue;
        };
        let mut frame_refs = Vec::with_capacity(anim.frames.len());
        for frame in &anim.frames {
            if frame.rotated {
                had_rotated_frames = true;
            }
            let Some(upright) = extract_upright_frame_rgba(&src_rgba, frame) else {
                dead_frames_removed += 1;
                frame_refs.push(FrameSourceRef::Dead {
                    raw_name: frame.raw_name.clone(),
                    frame_width: frame.frame_width.max(1),
                    frame_height: frame.frame_height.max(1),
                });
                continue;
            };

            let (vw, vh) = upright.dimensions();
            let Some((tx0, ty0, tx1, ty1)) = find_opaque_bounds(&upright) else {
                dead_frames_removed += 1;
                frame_refs.push(FrameSourceRef::Dead {
                    raw_name: frame.raw_name.clone(),
                    frame_width: frame.frame_width.max(1),
                    frame_height: frame.frame_height.max(1),
                });
                continue;
            };

            let tw = tx1 - tx0;
            let th = ty1 - ty0;
            if tx0 > 0 || ty0 > 0 || tw < vw || th < vh {
                trimmed_frames += 1;
            }
            max_trimmed_frame_side = max_trimmed_frame_side.max(tw).max(th);

            let mut trimmed_rgba = vec![0_u8; (tw as usize) * (th as usize) * 4];
            let upright_raw = upright.as_raw();
            for row in 0..th {
                let src_start = (((ty0 + row) as usize) * (vw as usize) + (tx0 as usize)) * 4;
                let dst_start = (row as usize) * (tw as usize) * 4;
                let row_bytes = (tw as usize) * 4;
                trimmed_rgba[dst_start..dst_start + row_bytes]
                    .copy_from_slice(&upright_raw[src_start..src_start + row_bytes]);
                // Canonizar RGB a 0 en píxeles completamente transparentes para maximizar deduplicación y compresión.
                for px in trimmed_rgba[dst_start..dst_start + row_bytes].chunks_exact_mut(4) {
                    if px[3] == 0 {
                        px[0] = 0;
                        px[1] = 0;
                        px[2] = 0;
                    }
                }
            }

            let hash = fnv1a_rgba_with_dims(tw, th, &trimmed_rgba);
            let matched_idx = dedup_buckets.get(&(hash, tw, th)).and_then(|bucket| {
                bucket
                    .iter()
                    .copied()
                    .find(|&idx| unique_sprites[idx].rgba == trimmed_rgba)
            });

            let unique_idx = if let Some(existing) = matched_idx {
                duplicate_frames_merged += 1;
                existing
            } else {
                let idx = unique_sprites.len();
                dedup_buckets.entry((hash, tw, th)).or_default().push(idx);
                unique_sprites.push(UniqueSprite {
                    w: tw,
                    h: th,
                    rgba: trimmed_rgba,
                });
                idx
            };

            let new_fx = frame.frame_x.saturating_sub(tx0 as i32);
            let new_fy = frame.frame_y.saturating_sub(ty0 as i32);
            frame_refs.push(FrameSourceRef::Unique {
                raw_name: frame.raw_name.clone(),
                unique_idx,
                frame_x: new_fx,
                frame_y: new_fy,
                frame_width: frame.frame_width.max(vw),
                frame_height: frame.frame_height.max(vh),
            });
        }
        anim_plan.push((prefix.clone(), anim.fps, anim.looped, frame_refs));
    }

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
        && (max_trimmed_frame_side as f32) > ideal_frame_max * 1.15
        && max_trimmed_frame_side > 384
    {
        (ideal_frame_max / (max_trimmed_frame_side as f32)).clamp(0.35, 1.0)
    } else {
        1.0
    };

    let clamped_initial = initial_scale.clamp(0.25, 1.0);
    let cap = tier_cap.max(64) as usize;
    let all_indices: Vec<usize> = (0..unique_sprites.len()).collect();

    // Si el atlas tiene >= 6 frames únicos y tope real de GPU (>= 1024 px), priorizamos dividirlo en
    // múltiples páginas nítidas (Multi-Page Sparrow Splitting) antes que aplastar su resolución.
    let (page_dims, applied_scale, placements): (
        Vec<(u32, u32)>,
        f32,
        Vec<(u16, u32, u32, u32, u32)>,
    ) = if let Some((w, h, single)) =
        pack_subset_with_crunch(&unique_sprites, &all_indices, clamped_initial, cap)
    {
        (
            vec![(w.min(tier_cap), h.min(tier_cap))],
            clamped_initial,
            single
                .into_iter()
                .map(|(px, py, sw, sh)| (0_u16, px, py, sw, sh))
                .collect(),
        )
    } else if tier_cap >= 1024 && unique_sprites.len() >= 6 {
        if let Some((pages, multi)) =
            pack_multi_page_with_crunch(&unique_sprites, clamped_initial, tier_cap, 6)
        {
            (pages, clamped_initial, multi)
        } else if let Some((pages, multi)) = pack_multi_page_with_crunch(
            &unique_sprites,
            (clamped_initial * 0.78).max(0.35),
            tier_cap,
            6,
        ) {
            (pages, (clamped_initial * 0.78).max(0.35), multi)
        } else {
            let (sheet_w, sheet_h, s, single) =
                pack_with_crunch(&unique_sprites, clamped_initial, tier_cap)?;
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
            pack_with_crunch(&unique_sprites, clamped_initial, tier_cap)?;
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

    let modified = dead_frames_removed > 0
        || duplicate_frames_merged > 0
        || trimmed_frames > 0
        || pruned_unused_anims > 0
        || had_rotated_frames
        || applied_scale < 0.999
        || is_multi_page
        || area_shrunk_significantly;

    // 4. Construir una o más hojas RGBA8 (fondo transparente [0,0,0,0], garantizando (0,0) vacío)
    // aplicando Lanczos3 en cada frame que requiera mip-downscale para preservar líneas limpias.
    let mut page_sheets: Vec<RgbaImage> = page_dims
        .iter()
        .map(|&(pw, ph)| RgbaImage::new(pw.max(4), ph.max(4)))
        .collect();
    for (idx, sp) in unique_sprites.iter().enumerate() {
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
    for (prefix, fps, looped, frame_refs) in anim_plan {
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
        dead_frames_removed,
        duplicate_frames_merged,
        trimmed_frames,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sparrow::CharacterAnimKind;
    use image::Rgba;

    fn paint_solid(img: &mut RgbaImage, x0: u32, y0: u32, w: u32, h: u32, color: Rgba<u8>) {
        for y in y0..(y0 + h) {
            for x in x0..(x0 + w) {
                img.put_pixel(x, y, color);
            }
        }
    }

    #[test]
    fn repack_eliminates_dead_frames_merges_duplicates_trims_alpha_and_prunes_unused() {
        // Hoja grande 1024x512 con:
        // - idle0000: bloque 20x30 con bordes transparentes enormes (80x100)
        // - idle0001: copia EXACTA de idle0000 pero con basura RGB invisible (alpha=0)
        // - idle0002: frame MUERTO 100% transparente (debe conservar duración 3 frames en idle)
        // - unused0000: animación huérfana sin bind ni mención en scripts
        let mut sheet = RgbaImage::new(1024, 512);
        paint_solid(&mut sheet, 30, 20, 20, 30, Rgba([255, 80, 20, 255]));
        // Basura invisible alrededor de idle0001 con mismo dibujo 20x30
        paint_solid(&mut sheet, 200, 0, 80, 100, Rgba([12, 200, 99, 0]));
        paint_solid(&mut sheet, 230, 20, 20, 30, Rgba([255, 80, 20, 255]));
        // Animación huérfana grande
        paint_solid(&mut sheet, 500, 50, 240, 240, Rgba([40, 120, 240, 255]));

        let xml = r#"
        <TextureAtlas imagePath="hero.png">
            <SubTexture name="hero idle0000" x="0" y="0" width="80" height="100" />
            <SubTexture name="hero idle0001" x="200" y="0" width="80" height="100" />
            <SubTexture name="hero idle0002" x="400" y="0" width="80" height="100" />
            <SubTexture name="unused pose0000" x="500" y="50" width="240" height="240" />
        </TextureAtlas>
        "#;

        let atlas = SparrowAtlas::from_xml_str(xml).unwrap();
        let mut ent = StageEntity::new("hero", "Hero", EntityRole::Player);
        ent.width = 280.0;
        ent.height = 380.0;
        ent.atlas = Some(atlas.clone());

        let keep_refs = HashSet::new();
        let res = optimize_and_repack_sparrow_atlas(
            &ent,
            &atlas,
            &DynamicImage::ImageRgba8(sheet),
            2560,
            Some(&keep_refs),
        )
        .unwrap();

        assert_eq!(res.dead_frames_removed, 1);
        assert_eq!(res.duplicate_frames_merged, 1);
        assert_eq!(res.trimmed_frames, 2);
        assert_eq!(res.pruned_unused_anims, 1);
        assert!(res.modified);
        assert!(res.vram_saved_bytes > 1_000_000);
        assert!(!res.atlas.animations.contains_key("unused pose"));

        let idle_frames = &res.atlas.animations["hero idle"].frames;
        // Se conservan exactamente los 3 frames temporales
        assert_eq!(idle_frames.len(), 3);
        // idle0000 e idle0001 apuntan al mismo rectángulo único recortado 20x30
        assert_eq!(idle_frames[0].x, idle_frames[1].x);
        assert_eq!(idle_frames[0].y, idle_frames[1].y);
        assert_eq!(idle_frames[0].width, 20);
        assert_eq!(idle_frames[0].height, 30);
        assert_eq!(idle_frames[1].width, 20);
        assert_eq!(idle_frames[1].height, 30);
        // idle0002 (muerto) apunta a (0,0) 1x1 transparente
        assert_eq!((idle_frames[2].x, idle_frames[2].y), (0, 0));
        assert_eq!((idle_frames[2].width, idle_frames[2].height), (1, 1));
        assert_eq!(res.image.to_rgba8().get_pixel(0, 0), &Rgba([0, 0, 0, 0]));
        assert!(res.atlas.bindings.contains_key(&CharacterAnimKind::Idle));
    }

    #[test]
    fn repack_preserves_script_referenced_unbound_animations_and_trim_offsets() {
        let mut sheet = RgbaImage::new(256, 256);
        paint_solid(&mut sheet, 15, 25, 10, 12, Rgba([200, 200, 200, 255]));
        paint_solid(&mut sheet, 100, 10, 16, 18, Rgba([10, 220, 120, 255]));

        let xml = r#"
        <TextureAtlas imagePath="hero.png">
            <SubTexture name="hero idle0000" x="0" y="0" width="40" height="50" />
            <SubTexture name="special hey0000" x="100" y="10" width="16" height="18" />
        </TextureAtlas>
        "#;
        let atlas = SparrowAtlas::from_xml_str(xml).unwrap();
        let ent = StageEntity::new("hero", "Hero", EntityRole::Player);
        let orig_idle = &atlas.animations["hero idle"].frames[0];
        let orig_fx = orig_idle.frame_x;
        let orig_fy = orig_idle.frame_y;

        let mut keep_refs = HashSet::new();
        keep_refs.insert("special hey".to_string());

        let res = optimize_and_repack_sparrow_atlas(
            &ent,
            &atlas,
            &DynamicImage::ImageRgba8(sheet),
            2560,
            Some(&keep_refs),
        )
        .unwrap();

        assert_eq!(res.pruned_unused_anims, 0);
        assert!(res.atlas.animations.contains_key("special hey"));

        let new_idle = &res.atlas.animations["hero idle"].frames[0];
        // Como recortamos 15px a la izquierda y 25px arriba, frame_x / frame_y se compensan exactamente
        assert_eq!(new_idle.frame_x, orig_fx - 15);
        assert_eq!(new_idle.frame_y, orig_fy - 25);
        assert_eq!(new_idle.width, 10);
        assert_eq!(new_idle.height, 12);
    }

    #[test]
    fn repack_fits_oversized_unique_frames_inside_tier_cap() {
        let mut sheet = RgbaImage::new(1024, 1024);
        paint_solid(&mut sheet, 0, 0, 300, 300, Rgba([250, 40, 40, 255]));
        paint_solid(&mut sheet, 320, 0, 300, 300, Rgba([40, 250, 40, 255]));
        paint_solid(&mut sheet, 0, 320, 300, 300, Rgba([40, 40, 250, 255]));
        paint_solid(&mut sheet, 320, 320, 300, 300, Rgba([250, 250, 40, 255]));

        let xml = r#"
        <TextureAtlas imagePath="boss.png">
            <SubTexture name="boss idle0000" x="0" y="0" width="300" height="300" />
            <SubTexture name="boss idle0001" x="320" y="0" width="300" height="300" />
            <SubTexture name="boss idle0002" x="0" y="320" width="300" height="300" />
            <SubTexture name="boss idle0003" x="320" y="320" width="300" height="300" />
        </TextureAtlas>
        "#;
        let atlas = SparrowAtlas::from_xml_str(xml).unwrap();
        let ent = StageEntity::new("boss", "Boss", EntityRole::Opponent);

        // Con tier_cap muy bajo (400px), debe escalar automáticamente los frames únicos para caber en <=400x400
        let res = optimize_and_repack_sparrow_atlas(
            &ent,
            &atlas,
            &DynamicImage::ImageRgba8(sheet),
            400,
            None,
        )
        .unwrap();

        assert!(res.image.width() <= 400);
        assert!(res.image.height() <= 400);
        assert!(res.frame_scale_applied < 1.0);
        assert_eq!(res.atlas.animations["boss idle"].frames.len(), 4);
    }

    #[test]
    fn repack_downscales_oversized_frame_with_lanczos3_according_to_viewport_and_zoom() {
        let mut sheet = RgbaImage::new(1024, 1024);
        paint_solid(&mut sheet, 10, 10, 900, 900, Rgba([60, 180, 255, 255]));
        let xml = r#"
        <TextureAtlas imagePath="big_char.png">
            <SubTexture name="big idle0000" x="0" y="0" width="960" height="960" />
        </TextureAtlas>
        "#;
        let atlas = SparrowAtlas::from_xml_str(xml).unwrap();
        let mut ent = StageEntity::new("big", "Big", EntityRole::Player);
        ent.width = 240.0;
        ent.height = 300.0;
        ent.scale = 1.0;

        // Pantalla 854x480 (viewport_scale ~0.67) y zoom 1.0 debe reducir más que pantalla 1920x1080 + zoom 1.6
        let compact = optimize_and_repack_sparrow_atlas_for_viewport(
            &ent,
            &atlas,
            &DynamicImage::ImageRgba8(sheet.clone()),
            4096,
            None,
            0.667,
            1.0,
        )
        .unwrap();
        let zoomed_fhd = optimize_and_repack_sparrow_atlas_for_viewport(
            &ent,
            &atlas,
            &DynamicImage::ImageRgba8(sheet),
            4096,
            None,
            1.5,
            1.6,
        )
        .unwrap();

        assert!(compact.frame_scale_applied < zoomed_fhd.frame_scale_applied);
        assert!(compact.image.width() < zoomed_fhd.image.width());
    }

    #[test]
    fn multi_page_sparrow_splitting_preserves_crispness_across_pages_inside_tier_cap() {
        // Hoja 2048x1600 con 10 frames únicos de 400x400 px cada uno:
        // en tier_cap = 1024 una sola hoja solo admite 4 frames sin aplastarlos;
        // Multi-Page Splitting debe dividirlos automáticamente en >= 3 páginas a escala 1.0.
        let mut sheet = RgbaImage::new(2048, 1600);
        let mut sub_tags = String::new();
        for idx in 0..10_u32 {
            let col = idx % 5;
            let row = idx / 5;
            let sx = col * 405;
            let sy = row * 405;
            paint_solid(
                &mut sheet,
                sx,
                sy,
                390,
                390,
                Rgba([30 + (idx as u8) * 20, 200 - (idx as u8) * 12, 90 + (idx as u8) * 14, 255]),
            );
            sub_tags.push_str(&format!(
                r#"<SubTexture name="boss idle{:04}" x="{}" y="{}" width="390" height="390" />"#,
                idx, sx, sy
            ));
        }

        let xml = format!(r#"<TextureAtlas imagePath="mega.png">{}</TextureAtlas>"#, sub_tags);
        let atlas = SparrowAtlas::from_xml_str(&xml).unwrap();
        let mut ent = StageEntity::new("mega_boss", "Mega Boss", EntityRole::Opponent);
        ent.width = 480.0;
        ent.height = 480.0;
        ent.scale = 1.0;

        let res = optimize_and_repack_sparrow_atlas_for_viewport(
            &ent,
            &atlas,
            &DynamicImage::ImageRgba8(sheet),
            1024,
            None,
            1.5,
            1.2,
        )
        .unwrap();

        assert!(res.page_count >= 3);
        assert_eq!(res.extra_pages.len(), res.page_count - 1);
        assert!((res.frame_scale_applied - 1.0).abs() < 0.001);
        assert!(res.image.width() <= 1024 && res.image.height() <= 1024);
        for extra in &res.extra_pages {
            assert!(extra.image.width() <= 1024 && extra.image.height() <= 1024);
        }
        let max_page = res.atlas.animations["boss idle"]
            .frames
            .iter()
            .map(|f| f.page_index)
            .max()
            .unwrap_or(0);
        assert_eq!(max_page as usize, res.page_count - 1);
    }
}
