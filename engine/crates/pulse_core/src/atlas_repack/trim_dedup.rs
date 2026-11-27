//! Archivo: engine/crates/pulse_core/src/atlas_repack/trim_dedup.rs
//! Propósito: Recorte estricto de transparencia (*alpha trim*), detección de frames muertos,
//! canonización RGB invisible, deduplicación exacta por FNV-1a y poda segura de animaciones
//! huérfanas que no son usadas ni en `bindings`, ni en scripts habilitados ni en `chart.events`.

use crate::chart::ChartData;
use crate::project::ScriptAsset;
use crate::sparrow::{SparrowAtlas, SparrowFrame};
use crate::stage::StageEntity;
use image::RgbaImage;
use std::collections::{HashMap, HashSet};

/// Umbral mínimo de opacidad (`alpha >= 2`) para considerar un píxel visible durante el recorte.
/// Descarta ruido invisible `alpha = 0..1` que algunos exportadores dejan en bordes transparentes.
const ALPHA_TRIM_THRESHOLD: u8 = 2;

#[derive(Debug, Clone)]
pub(super) struct UniqueSprite {
    pub(super) w: u32,
    pub(super) h: u32,
    pub(super) rgba: Vec<u8>,
}

#[derive(Debug, Clone)]
pub(super) enum FrameSourceRef {
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

pub(super) type AnimPlanEntry = (String, f32, bool, Vec<FrameSourceRef>);

pub(super) struct DedupAnalysisOutput {
    pub(super) unique_sprites: Vec<UniqueSprite>,
    pub(super) anim_plan: Vec<AnimPlanEntry>,
    pub(super) dead_frames_removed: usize,
    pub(super) duplicate_frames_merged: usize,
    pub(super) trimmed_frames: usize,
    pub(super) max_trimmed_frame_side: u32,
    pub(super) had_rotated_frames: bool,
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
        let in_script = scripts.iter().filter(|s| s.enabled).any(|s| {
            s.source_code.contains(&prefix)
                || s.source_code.to_ascii_lowercase().contains(&lower_prefix)
        });
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

pub(super) fn select_kept_animation_keys(
    atlas: &SparrowAtlas,
    referenced_anim_prefixes: Option<&HashSet<String>>,
) -> (Vec<String>, usize) {
    let bound_prefixes: HashSet<&str> = atlas.bindings.values().map(|s| s.as_str()).collect();
    let mut kept_anim_keys: Vec<String> = Vec::with_capacity(atlas.animations.len());
    let mut pruned_unused_anims = 0_usize;

    for prefix in atlas.animations.keys() {
        let is_bound = bound_prefixes.contains(prefix.as_str());
        let is_script_ref = referenced_anim_prefixes.is_some_and(|set| set.contains(prefix));
        if bound_prefixes.is_empty()
            || is_bound
            || is_script_ref
            || referenced_anim_prefixes.is_none()
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

    (kept_anim_keys, pruned_unused_anims)
}

/// Extrae el buffer RGBA8 en orientación normal (`rotated = false` tras aplicar `rotate270`)
/// de un `SparrowFrame`. Devuelve `None` si la región es inválida o vacía.
fn extract_upright_frame_rgba(src_rgba: &RgbaImage, frame: &SparrowFrame) -> Option<RgbaImage> {
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

pub(super) fn analyze_and_dedup_atlas_frames(
    atlas: &SparrowAtlas,
    src_rgba: &RgbaImage,
    kept_anim_keys: &[String],
) -> DedupAnalysisOutput {
    let mut unique_sprites: Vec<UniqueSprite> = Vec::new();
    let mut dedup_buckets: HashMap<(u64, u32, u32), Vec<usize>> = HashMap::new();
    let mut anim_plan: Vec<AnimPlanEntry> = Vec::with_capacity(kept_anim_keys.len());

    let mut dead_frames_removed = 0_usize;
    let mut duplicate_frames_merged = 0_usize;
    let mut trimmed_frames = 0_usize;
    let mut max_trimmed_frame_side = 1_u32;
    let mut had_rotated_frames = false;

    for prefix in kept_anim_keys {
        let Some(anim) = atlas.animations.get(prefix) else {
            continue;
        };
        let mut frame_refs = Vec::with_capacity(anim.frames.len());
        for frame in &anim.frames {
            if frame.rotated {
                had_rotated_frames = true;
            }
            let Some(upright) = extract_upright_frame_rgba(src_rgba, frame) else {
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

    DedupAnalysisOutput {
        unique_sprites,
        anim_plan,
        dead_frames_removed,
        duplicate_frames_merged,
        trimmed_frames,
        max_trimmed_frame_side,
        had_rotated_frames,
    }
}
