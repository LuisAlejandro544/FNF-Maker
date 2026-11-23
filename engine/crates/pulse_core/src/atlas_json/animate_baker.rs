//! Archivo: engine/crates/pulse_core/src/atlas_json/animate_baker.rs
//! Propósito: Evaluador jerárquico y rasterizador por software (`bake_animate_atlas`) que
//! compone los frames de `Animation.json` + `spritemap1.json`, omite capas ocultas o máscaras,
//! des-rota piezas `+90° CW` con muestreo bilineal premultiplicado, deduplica poses repetidas
//! y empaqueta un spritesheet RGBA `<= 4096x4096` listo para `SpriteLab` y `Gameplay`.

use super::animate_model::{AnimateDocument, AnimateElement, AnimateTimeline, SymbolLoopMode};
use super::direct_formats::{AnimateSpritemap, SpritemapRect};
use super::transform::{Affine2D, ColorTransform2D};
use crate::sparrow::{AnimSet, SparrowAtlas, SparrowFrame};
use image::{DynamicImage, Rgba, RgbaImage};
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub(super) enum AnimSegmentSource<'a> {
    MainRange { start: usize, len: usize },
    Symbol(&'a AnimateTimeline),
}

pub(super) fn extract_animation_segments<'a>(
    main_tl: &'a AnimateTimeline,
    symbols: &'a HashMap<String, AnimateTimeline>,
) -> Vec<(String, AnimSegmentSource<'a>)> {
    // 1. Buscar etiquetas de frame (`N`) en el timeline principal (formato clásico y BTA Animate).
    let mut labeled: Vec<(usize, usize, String)> = Vec::new();
    for layer in &main_tl.layers {
        for fr in &layer.frames {
            if let Some(lbl) = &fr.label {
                labeled.push((fr.index, fr.index + fr.duration, lbl.clone()));
            }
        }
    }
    if !labeled.is_empty() {
        labeled.sort_by_key(|(start, _, _)| *start);
        let mut out = Vec::new();
        for i in 0..labeled.len() {
            let (start, own_end, ref lbl) = labeled[i];
            let next_start = labeled
                .get(i + 1)
                .map(|x| x.0)
                .unwrap_or(main_tl.total_frames);
            let end = if own_end <= start + 1 {
                next_start.min(main_tl.total_frames)
            } else {
                own_end.min(next_start).min(main_tl.total_frames)
            };
            let len = end.saturating_sub(start).clamp(1, 60);
            out.push((lbl.clone(), AnimSegmentSource::MainRange { start, len }));
        }
        return out;
    }

    // 2. Si no hay etiquetas, revisar símbolos con nombres direccionales o de animación en SD.
    let mut sym_anims: Vec<(String, AnimSegmentSource<'a>)> = symbols
        .iter()
        .filter(|(k, tl)| {
            let low = k.to_ascii_lowercase();
            tl.total_frames > 1
                || low.contains("idle")
                || low.contains("left")
                || low.contains("down")
                || low.contains("up")
                || low.contains("right")
                || low.contains("sing")
        })
        .map(|(k, tl)| (k.clone(), AnimSegmentSource::Symbol(tl)))
        .collect();
    sym_anims.sort_by(|a, b| a.0.cmp(&b.0));

    if !sym_anims.is_empty() {
        return sym_anims;
    }

    // 3. Respaldo: todo el timeline principal como una sola animación.
    vec![(
        "Main".to_string(),
        AnimSegmentSource::MainRange {
            start: 0,
            len: main_tl.total_frames.clamp(1, 72),
        },
    )]
}

#[derive(Debug, Clone)]
struct DrawCall {
    sprite_name: String,
    matrix: Affine2D,
    color: ColorTransform2D,
}

fn pose_signature(draws: &[DrawCall]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    draws.len().hash(&mut hasher);
    for dc in draws {
        dc.sprite_name.hash(&mut hasher);
        (dc.matrix.a * 1000.0).round().to_bits().hash(&mut hasher);
        (dc.matrix.b * 1000.0).round().to_bits().hash(&mut hasher);
        (dc.matrix.c * 1000.0).round().to_bits().hash(&mut hasher);
        (dc.matrix.d * 1000.0).round().to_bits().hash(&mut hasher);
        (dc.matrix.tx * 20.0).round().to_bits().hash(&mut hasher);
        (dc.matrix.ty * 20.0).round().to_bits().hash(&mut hasher);
        (dc.color.rm * 100.0).round().to_bits().hash(&mut hasher);
        (dc.color.gm * 100.0).round().to_bits().hash(&mut hasher);
        (dc.color.bm * 100.0).round().to_bits().hash(&mut hasher);
        (dc.color.am * 100.0).round().to_bits().hash(&mut hasher);
        dc.color.ro.round().to_bits().hash(&mut hasher);
        dc.color.go.round().to_bits().hash(&mut hasher);
        dc.color.bo.round().to_bits().hash(&mut hasher);
        dc.color.ao.round().to_bits().hash(&mut hasher);
    }
    hasher.finish()
}

fn collect_draws_at_frame(
    tl: &AnimateTimeline,
    symbols: &HashMap<String, AnimateTimeline>,
    frame_idx: usize,
    parent_matrix: Affine2D,
    parent_color: ColorTransform2D,
    depth: usize,
    out: &mut Vec<DrawCall>,
) {
    if depth > 12 || out.len() > 256 || parent_color.is_invisible() {
        return;
    }
    // Adobe Animate lista las capas de arriba hacia abajo; se dibujan de abajo hacia arriba.
    for layer in tl.layers.iter().rev() {
        if !layer.visible || layer.is_mask_or_guide {
            continue;
        }
        let Some(fr) = layer
            .frames
            .iter()
            .find(|f| frame_idx >= f.index && frame_idx < f.index + f.duration)
        else {
            continue;
        };

        let frame_color = parent_color.concat(fr.color);
        if frame_color.is_invisible() {
            continue;
        }

        let local_offset = frame_idx - fr.index;
        for el in &fr.elements {
            match el {
                AnimateElement::Sprite {
                    name,
                    matrix,
                    color,
                } => {
                    let elem_color = frame_color.concat(*color);
                    if elem_color.is_invisible() {
                        continue;
                    }
                    out.push(DrawCall {
                        sprite_name: name.clone(),
                        matrix: parent_matrix.mul(*matrix),
                        color: elem_color,
                    });
                }
                AnimateElement::Symbol {
                    name,
                    matrix,
                    loop_mode,
                    reverse,
                    first_frame,
                    color,
                } => {
                    let elem_color = frame_color.concat(*color);
                    if elem_color.is_invisible() {
                        continue;
                    }
                    if let Some(sub_tl) = symbols.get(name) {
                        let sub_total = sub_tl.total_frames.max(1);
                        let sub_frame = if *reverse {
                            let step = match loop_mode {
                                SymbolLoopMode::SingleFrame => 0,
                                SymbolLoopMode::PlayOnce => local_offset.min(sub_total - 1),
                                SymbolLoopMode::Loop => local_offset % sub_total,
                            };
                            ((*first_frame + sub_total * 1024).saturating_sub(step)) % sub_total
                        } else {
                            match loop_mode {
                                SymbolLoopMode::SingleFrame => (*first_frame).min(sub_total - 1),
                                SymbolLoopMode::PlayOnce => {
                                    (first_frame + local_offset).min(sub_total - 1)
                                }
                                SymbolLoopMode::Loop => (first_frame + local_offset) % sub_total,
                            }
                        };
                        collect_draws_at_frame(
                            sub_tl,
                            symbols,
                            sub_frame,
                            parent_matrix.mul(*matrix),
                            elem_color,
                            depth + 1,
                            out,
                        );
                    }
                }
            }
        }
    }
}

/// Empaquetador en estantes (altura decreciente) para estimar el tamaño final antes de rasterizar.
fn pack_shelves(
    sizes: &[(usize, u32, u32)],
    max_atlas_w: u32,
    pad: u32,
) -> (u32, u32, Vec<(u32, u32)>) {
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by(|&a, &b| {
        sizes[b]
            .2
            .cmp(&sizes[a].2)
            .then_with(|| sizes[b].1.cmp(&sizes[a].1))
    });

    let mut coords = vec![(0_u32, 0_u32); sizes.len()];
    let mut cur_x = pad;
    let mut cur_y = pad;
    let mut row_h = 0_u32;
    let mut sheet_w = 64_u32;

    for idx in order {
        let (_, fw, fh) = sizes[idx];
        if cur_x + fw + pad > max_atlas_w && cur_x > pad {
            cur_x = pad;
            cur_y += row_h + pad;
            row_h = 0;
        }
        coords[idx] = (cur_x, cur_y);
        sheet_w = sheet_w.max(cur_x + fw + pad);
        cur_x += fw + pad;
        row_h = row_h.max(fh);
    }

    let sheet_h = (cur_y + row_h + pad).max(64);
    (sheet_w, sheet_h, coords)
}

/// Rasteriza por software todas las animaciones de `Animation.json` + `spritemap1.json` + textura
/// en una sola hoja RGBA empaquetada `<= 4096x4096` con escala unificada y offsets exactos.
pub fn bake_animate_atlas(
    doc: &AnimateDocument,
    spritemap: &AnimateSpritemap,
    source_image: &DynamicImage,
) -> Result<(DynamicImage, SparrowAtlas), String> {
    let segments = extract_animation_segments(&doc.main_timeline, &doc.symbols);
    if segments.is_empty() {
        return Err("No se encontraron animaciones en Animation.json".to_string());
    }

    let src_rgba = source_image.to_rgba8();
    let (src_w, src_h) = src_rgba.dimensions();

    struct UniquePose {
        draws: Vec<DrawCall>,
        min_x: f32,
        min_y: f32,
        max_x: f32,
        max_y: f32,
    }

    struct TimelineSlot {
        anim_name: String,
        frame_idx_in_anim: usize,
        pose_idx: usize,
    }

    let mut unique_poses: Vec<UniquePose> = Vec::new();
    let mut signature_to_pose: HashMap<u64, usize> = HashMap::new();
    let mut timeline_slots: Vec<TimelineSlot> = Vec::new();

    for (anim_name, source) in segments {
        let len = match source {
            AnimSegmentSource::MainRange { len, .. } => len,
            AnimSegmentSource::Symbol(tl) => tl.total_frames.clamp(1, 72),
        };

        for f_i in 0..len {
            let mut draws = Vec::new();
            match source {
                AnimSegmentSource::MainRange { start, .. } => {
                    collect_draws_at_frame(
                        &doc.main_timeline,
                        &doc.symbols,
                        start + f_i,
                        Affine2D::identity(),
                        ColorTransform2D::identity(),
                        0,
                        &mut draws,
                    );
                }
                AnimSegmentSource::Symbol(tl) => {
                    collect_draws_at_frame(
                        tl,
                        &doc.symbols,
                        f_i,
                        Affine2D::identity(),
                        ColorTransform2D::identity(),
                        0,
                        &mut draws,
                    );
                }
            }

            if draws.is_empty() {
                continue;
            }

            let mut min_x = f32::INFINITY;
            let mut min_y = f32::INFINITY;
            let mut max_x = f32::NEG_INFINITY;
            let mut max_y = f32::NEG_INFINITY;

            for dc in &draws {
                let Some(rect) = spritemap.sprites.get(&dc.sprite_name) else {
                    continue;
                };
                let lw = if rect.rotated {
                    rect.h as f32
                } else {
                    rect.w as f32
                };
                let lh = if rect.rotated {
                    rect.w as f32
                } else {
                    rect.h as f32
                };
                for (cx, cy) in [(0.0, 0.0), (lw, 0.0), (0.0, lh), (lw, lh)] {
                    let (tx, ty) = dc.matrix.transform_point(cx, cy);
                    min_x = min_x.min(tx);
                    min_y = min_y.min(ty);
                    max_x = max_x.max(tx);
                    max_y = max_y.max(ty);
                }
            }

            if !min_x.is_finite() || max_x <= min_x || max_y <= min_y {
                continue;
            }

            let sig = pose_signature(&draws);
            let pose_idx = if let Some(&existing) = signature_to_pose.get(&sig) {
                existing
            } else {
                let idx = unique_poses.len();
                signature_to_pose.insert(sig, idx);
                unique_poses.push(UniquePose {
                    draws,
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                });
                idx
            };

            timeline_slots.push(TimelineSlot {
                anim_name: anim_name.clone(),
                frame_idx_in_anim: f_i,
                pose_idx,
            });
        }
    }

    if unique_poses.is_empty() || timeline_slots.is_empty() {
        return Err(
            "No se pudo generar ningún frame desde Animation.json + spritemap1.json".to_string(),
        );
    }

    // Caja envolvente unificada para mantener pivote y escala constantes en todas las animaciones.
    let mut global_min_x = f32::INFINITY;
    let mut global_min_y = f32::INFINITY;
    let mut global_max_x = f32::NEG_INFINITY;
    let mut global_max_y = f32::NEG_INFINITY;
    let mut max_pose_side = 1.0_f32;

    for pose in &unique_poses {
        global_min_x = global_min_x.min(pose.min_x);
        global_min_y = global_min_y.min(pose.min_y);
        global_max_x = global_max_x.max(pose.max_x);
        global_max_y = global_max_y.max(pose.max_y);
        max_pose_side = max_pose_side
            .max(pose.max_x - pose.min_x)
            .max(pose.max_y - pose.min_y);
    }

    // Escala inicial nítida (hasta 640px por pose sin reescalar hacia arriba) + autoajuste <= 4096x4096.
    let pad = 2_u32;
    let max_atlas_dim = 4096_u32;
    let mut world_scale = (640.0_f32 / max_pose_side).min(1.0);
    let mut packed_layout = Vec::new();
    let mut sheet_w = 64_u32;
    let mut sheet_h = 64_u32;

    for _attempt in 0..5 {
        let sizes: Vec<(usize, u32, u32)> = unique_poses
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let fw = ((p.max_x - p.min_x) * world_scale).ceil().max(4.0) as u32;
                let fh = ((p.max_y - p.min_y) * world_scale).ceil().max(4.0) as u32;
                (i, fw, fh)
            })
            .collect();

        let (sw, sh, coords) = pack_shelves(&sizes, max_atlas_dim, pad);
        sheet_w = sw;
        sheet_h = sh;
        packed_layout = sizes
            .into_iter()
            .zip(coords.into_iter())
            .map(|((_, fw, fh), (ox, oy))| (ox, oy, fw, fh))
            .collect();

        if sh <= 3840 {
            break;
        }
        let shrink = ((3600.0_f32 / sh as f32).sqrt() * 0.94).clamp(0.35, 0.92);
        world_scale = (world_scale * shrink).max(0.22);
    }

    let logical_canvas_w = ((global_max_x - global_min_x) * world_scale)
        .ceil()
        .max(4.0) as u32;
    let logical_canvas_h = ((global_max_y - global_min_y) * world_scale)
        .ceil()
        .max(4.0) as u32;

    let mut sheet = RgbaImage::new(sheet_w.max(64), sheet_h.max(64));
    let mut pose_frames: Vec<SparrowFrame> = Vec::with_capacity(unique_poses.len());

    for (pose_idx, pose) in unique_poses.iter().enumerate() {
        let (ox, oy, fw, fh) = packed_layout[pose_idx];
        let mut frame_img = RgbaImage::new(fw, fh);
        let view_shift = Affine2D {
            a: world_scale,
            b: 0.0,
            c: 0.0,
            d: world_scale,
            tx: -pose.min_x * world_scale,
            ty: -pose.min_y * world_scale,
        };

        for dc in &pose.draws {
            let Some(rect) = spritemap.sprites.get(&dc.sprite_name) else {
                continue;
            };
            let full_m = view_shift.mul(dc.matrix);
            let Some(inv_m) = full_m.invert() else {
                continue;
            };

            let lw = if rect.rotated {
                rect.h as f32
            } else {
                rect.w as f32
            };
            let lh = if rect.rotated {
                rect.w as f32
            } else {
                rect.h as f32
            };

            let mut d_min_x = f32::INFINITY;
            let mut d_min_y = f32::INFINITY;
            let mut d_max_x = f32::NEG_INFINITY;
            let mut d_max_y = f32::NEG_INFINITY;
            for (cx, cy) in [(0.0, 0.0), (lw, 0.0), (0.0, lh), (lw, lh)] {
                let (px, py) = full_m.transform_point(cx, cy);
                d_min_x = d_min_x.min(px);
                d_min_y = d_min_y.min(py);
                d_max_x = d_max_x.max(px);
                d_max_y = d_max_y.max(py);
            }

            let x0 = (d_min_x.floor() as i32).clamp(0, fw as i32 - 1);
            let y0 = (d_min_y.floor() as i32).clamp(0, fh as i32 - 1);
            let x1 = (d_max_x.ceil() as i32).clamp(0, fw as i32 - 1);
            let y1 = (d_max_y.ceil() as i32).clamp(0, fh as i32 - 1);

            for py in y0..=y1 {
                for px in x0..=x1 {
                    let (lx, ly) = inv_m.transform_point(px as f32 + 0.5, py as f32 + 0.5);
                    if lx < 0.0 || ly < 0.0 || lx >= lw || ly >= lh {
                        continue;
                    }
                    let (u, v) = if rect.rotated {
                        let max_u = (rect.w as f32 - 1.0).max(0.0);
                        let max_v = (rect.h as f32 - 1.0).max(0.0);
                        (
                            (max_u - (ly - 0.5)).clamp(0.0, max_u),
                            (lx - 0.5).clamp(0.0, max_v),
                        )
                    } else {
                        let max_u = (rect.w as f32 - 1.0).max(0.0);
                        let max_v = (rect.h as f32 - 1.0).max(0.0);
                        ((lx - 0.5).clamp(0.0, max_u), (ly - 0.5).clamp(0.0, max_v))
                    };

                    let sampled = sample_rect_bilinear_premul(&src_rgba, src_w, src_h, rect, u, v);
                    if sampled[3] == 0 {
                        continue;
                    }
                    let tinted = dc.color.apply(sampled);
                    if tinted[3] == 0 {
                        continue;
                    }
                    alpha_blend(frame_img.get_pixel_mut(px as u32, py as u32), tinted);
                }
            }
        }

        image::imageops::overlay(&mut sheet, &frame_img, ox as i64, oy as i64);

        let frame_x = -((pose.min_x - global_min_x) * world_scale).round() as i32;
        let frame_y = -((pose.min_y - global_min_y) * world_scale).round() as i32;

        pose_frames.push(SparrowFrame {
            raw_name: format!("pose_{:04}", pose_idx),
            x: ox,
            y: oy,
            width: fw,
            height: fh,
            frame_x,
            frame_y,
            frame_width: logical_canvas_w.max(fw),
            frame_height: logical_canvas_h.max(fh),
            rotated: false,
            page_index: 0,
        });
    }

    let mut animations: BTreeMap<String, AnimSet> = BTreeMap::new();
    for slot in timeline_slots {
        let entry = animations
            .entry(slot.anim_name.clone())
            .or_insert_with(|| {
                let low = slot.anim_name.to_ascii_lowercase();
                AnimSet {
                    prefix: slot.anim_name.clone(),
                    fps: doc.framerate,
                    looped: low.contains("idle") || low.contains("dance") || low.contains("loop"),
                    frames: Vec::new(),
                }
            });
        let mut frame = pose_frames[slot.pose_idx].clone();
        frame.raw_name = format!("{}{:04}", slot.anim_name, slot.frame_idx_in_anim);
        entry.frames.push(frame);
    }

    let bindings = SparrowAtlas::auto_detect_bindings(&animations);
    let mut atlas = SparrowAtlas {
        image_path_hint: format!("{}_baked.png", doc.name),
        animations,
        bindings,
    };
    atlas.normalize_and_center_animations();
    Ok((DynamicImage::ImageRgba8(sheet), atlas))
}

/// Muestreo bilineal en alfa premultiplicada estrictamente confinado dentro de `rect` para
/// evitar bordes negros o sangrado de piezas vecinas en `spritemap1.png` / `.astc`.
#[inline]
fn sample_rect_bilinear_premul(
    img: &RgbaImage,
    img_w: u32,
    img_h: u32,
    rect: &SpritemapRect,
    u: f32,
    v: f32,
) -> Rgba<u8> {
    if rect.w == 0 || rect.h == 0 || rect.x >= img_w || rect.y >= img_h {
        return Rgba([0, 0, 0, 0]);
    }
    let max_u = (rect.w - 1).min(img_w - 1 - rect.x);
    let max_v = (rect.h - 1).min(img_h - 1 - rect.y);

    let u0 = (u.floor() as u32).min(max_u);
    let v0 = (v.floor() as u32).min(max_v);
    let u1 = (u0 + 1).min(max_u);
    let v1 = (v0 + 1).min(max_v);

    let fu = (u - u0 as f32).clamp(0.0, 1.0);
    let fv = (v - v0 as f32).clamp(0.0, 1.0);

    let p00 = *img.get_pixel(rect.x + u0, rect.y + v0);
    let p10 = *img.get_pixel(rect.x + u1, rect.y + v0);
    let p01 = *img.get_pixel(rect.x + u0, rect.y + v1);
    let p11 = *img.get_pixel(rect.x + u1, rect.y + v1);

    let w00 = (1.0 - fu) * (1.0 - fv);
    let w10 = fu * (1.0 - fv);
    let w01 = (1.0 - fu) * fv;
    let w11 = fu * fv;

    let a00 = p00[3] as f32 / 255.0 * w00;
    let a10 = p10[3] as f32 / 255.0 * w10;
    let a01 = p01[3] as f32 / 255.0 * w01;
    let a11 = p11[3] as f32 / 255.0 * w11;

    let out_a = a00 + a10 + a01 + a11;
    if out_a <= 0.001 {
        return Rgba([0, 0, 0, 0]);
    }

    let r = (p00[0] as f32 * a00
        + p10[0] as f32 * a10
        + p01[0] as f32 * a01
        + p11[0] as f32 * a11)
        / out_a;
    let g = (p00[1] as f32 * a00
        + p10[1] as f32 * a10
        + p01[1] as f32 * a01
        + p11[1] as f32 * a11)
        / out_a;
    let b = (p00[2] as f32 * a00
        + p10[2] as f32 * a10
        + p01[2] as f32 * a01
        + p11[2] as f32 * a11)
        / out_a;

    Rgba([
        r.clamp(0.0, 255.0).round() as u8,
        g.clamp(0.0, 255.0).round() as u8,
        b.clamp(0.0, 255.0).round() as u8,
        (out_a * 255.0).clamp(0.0, 255.0).round() as u8,
    ])
}

#[inline]
fn alpha_blend(dst: &mut Rgba<u8>, src: Rgba<u8>) {
    let sa = src[3] as f32 / 255.0;
    let da = dst[3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0001 {
        return;
    }
    for c in 0..3 {
        let sc = src[c] as f32 / 255.0;
        let dc = dst[c] as f32 / 255.0;
        let out_c = (sc * sa + dc * da * (1.0 - sa)) / out_a;
        dst[c] = (out_c.clamp(0.0, 1.0) * 255.0).round() as u8;
    }
    dst[3] = (out_a.clamp(0.0, 1.0) * 255.0).round() as u8;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas_json::{DetectedSpriteJson, parse_sprite_json_str};

    #[test]
    fn parses_spritemap_and_texture_packer_json() {
        let sm_raw = r#"{
            "ATLAS": {
                "SPRITES": [
                    { "SPRITE": { "name": "0000", "x": 4, "y": 4, "w": 64, "h": 64, "rotated": false } }
                ]
            },
            "meta": { "image": "spritemap1.astc" }
        }"#;
        let DetectedSpriteJson::AnimateSpritemap(sm) = parse_sprite_json_str(sm_raw).unwrap()
        else {
            panic!("esperaba AnimateSpritemap");
        };
        assert_eq!(sm.sprites.len(), 1);
        assert_eq!(sm.image_hint, "spritemap1.astc");

        let tp_raw = r#"{
            "frames": {
                "hero_idle0000.png": { "frame": {"x":0,"y":0,"w":80,"h":100}, "rotated": false },
                "hero_idle0001.png": { "frame": {"x":80,"y":0,"w":80,"h":100}, "rotated": false },
                "hero_left0000.png": { "frame": {"x":160,"y":0,"w":80,"h":100}, "rotated": true }
            }
        }"#;
        let DetectedSpriteJson::DirectAtlas(atlas) = parse_sprite_json_str(tp_raw).unwrap() else {
            panic!("esperaba DirectAtlas");
        };
        assert_eq!(atlas.animations.len(), 2);
        assert_eq!(atlas.animations["hero_idle"].frames.len(), 2);
        assert!(atlas
            .bindings
            .contains_key(&crate::sparrow::CharacterAnimKind::Idle));
        assert!(atlas
            .bindings
            .contains_key(&crate::sparrow::CharacterAnimKind::Left));
    }

    #[test]
    fn bakes_rotated_cw_sprite_skips_clipper_and_deduplicates_frames() {
        let sm_raw = r#"{
            "ATLAS": {
                "SPRITES": [
                    { "SPRITE": { "name": "top_bar", "x": 0, "y": 0, "w": 8, "h": 4, "rotated": true } },
                    { "SPRITE": { "name": "mask_dot", "x": 8, "y": 0, "w": 4, "h": 4, "rotated": false } }
                ]
            }
        }"#;
        let anim_raw = r#"{
            "MD": { "V": "BTA 1.1.0", "FRT": 24 },
            "AN": {
                "N": "RotRig",
                "TL": {
                    "L": [
                        { "LN": "Mask", "LT": "Clipper", "FR": [
                            { "I": 0, "DU": 3, "E": [ { "ASI": { "N": "mask_dot", "MX": [1,0,0,1, 0,0] } } ] }
                        ]},
                        { "LN": "InvisibleAlpha", "FR": [
                            { "I": 0, "DU": 3, "E": [ { "ASI": { "N": "mask_dot", "C": { "M": "CA", "AM": 0.0 }, "MX": [1,0,0,1, 0,0] } } ] }
                        ]},
                        { "LN": "Labels", "FR": [
                            { "I": 0, "DU": 3, "N": "idle", "E": [] }
                        ]},
                        { "LN": "Body", "FR": [
                            { "I": 0, "DU": 3, "E": [
                                { "ASI": { "N": "top_bar", "MX": [1,0,0,1, 0,0] } }
                            ]}
                        ]}
                    ]
                }
            },
            "SD": { "S": [] }
        }"#;

        let DetectedSpriteJson::AnimateSpritemap(sm) = parse_sprite_json_str(sm_raw).unwrap()
        else {
            panic!("spritemap");
        };
        let DetectedSpriteJson::AnimateDocument(doc) = parse_sprite_json_str(anim_raw).unwrap()
        else {
            panic!("animate doc");
        };

        let mut img = RgbaImage::new(16, 8);
        for v in 0..4 {
            img.put_pixel(7, v, Rgba([20, 120, 255, 255]));
        }
        for y in 0..4 {
            for x in 8..12 {
                img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }

        let (sheet, atlas) = bake_animate_atlas(&doc, &sm, &DynamicImage::ImageRgba8(img)).unwrap();
        assert!(sheet.width() <= 4096 && sheet.height() <= 4096);
        let frames = &atlas.animations["idle"].frames;
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].x, frames[1].x);
        assert_eq!(frames[0].y, frames[2].y);

        let f0 = &frames[0];
        let rgba = sheet.to_rgba8();
        let top_px = rgba.get_pixel(f0.x + 1, f0.y);
        assert!(
            top_px[2] > 200 && top_px[0] < 60,
            "esperaba azul arriba al des-rotar +90° CW: {:?}",
            top_px
        );
        for py in f0.y..(f0.y + f0.height) {
            for px in f0.x..(f0.x + f0.width) {
                let p = rgba.get_pixel(px, py);
                assert!(
                    p[0] < 50 || p[3] == 0,
                    "se filtró rojo de Clipper/AM=0 en ({}, {}): {:?}",
                    px,
                    py,
                    p
                );
            }
        }
    }
}
