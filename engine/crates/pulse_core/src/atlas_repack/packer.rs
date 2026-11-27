//! Archivo: engine/crates/pulse_core/src/atlas_repack/packer.rs
//! Propósito: Empaquetado 2D compacto de frames únicos con `crunch` (`MIT OR Apache-2.0`),
//! tanto en hoja única adaptativa como en paginación multi-página (`Multi-Page Sparrow Splitting`)
//! para mega-atlas sin aplastar nitidez en dispositivos Android de 32 y 64 bits.

use super::trim_dedup::UniqueSprite;
use crunch::{Item, Rect as CrunchRect, Rotation};

/// Espacio de separación entre frames empaquetados (1 px por cada lado = 2 px entre vecinos)
/// para evitar sangrado de bordes con filtros lineales y garantizar que `(0, 0)` quede `[0,0,0,0]`.
const FRAME_EDGE_PAD: u32 = 1;

/// Empaqueta un subconjunto `indices` de `unique_sprites` a `scale` dentro de `<= cap x cap`
/// eligiendo el lienzo cuadrado más compacto que logre acomodar todos los rectángulos.
pub(super) fn pack_subset_with_crunch(
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
pub(super) fn pack_multi_page_with_crunch(
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
pub(super) fn pack_with_crunch(
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
