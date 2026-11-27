//! Archivo: engine/crates/pulse_core/src/sparrow/normalize.rs
//! Propósito: Normalización geométrica (`normalize_and_center_animations`) y alineación
//! por píxeles opacos de pies/suelo (`align_animations_with_image`, `align_animations_with_rgba`)
//! sobre el lienzo lógico unificado para evitar saltos entre poses asimétricas o frames rotados.

use super::models::{SparrowAtlas, SparrowFrame};

impl SparrowAtlas {
    /// Normaliza y centra automáticamente todas las animaciones del atlas dentro de un lienzo
    /// lógico unificado `(unified_w, unified_h)` compartido por todo el spritesheet.
    /// Elimina márgenes vacíos asimétricos entre poses (`Idle` derecho vs `Left`/`Down`/`Up`/`Right`
    /// movidos), mantiene intacta la fluidez relativa interna entre frames de una misma animación,
    /// alinea horizontalmente al centro y verticalmente al suelo, y garantiza que el escalado (`fit`)
    /// no cambie de tamaño al alternar poses. Es idempotente y funciona en todos los formatos.
    pub fn normalize_and_center_animations(&mut self) {
        if self.animations.is_empty() {
            return;
        }

        struct AnimFrameLayout {
            prefix: String,
            span_w: u32,
            span_h: u32,
            rel_offsets: Vec<(i32, i32)>,
        }

        let mut layouts: Vec<AnimFrameLayout> = Vec::with_capacity(self.animations.len());
        let mut unified_w = 1_u32;
        let mut unified_h = 1_u32;

        for (prefix, anim) in &self.animations {
            if anim.frames.is_empty() {
                continue;
            }

            let vis_dims: Vec<(u32, u32)> = anim
                .frames
                .iter()
                .map(|f| {
                    let vw = if f.rotated { f.height } else { f.width }.max(1);
                    let vh = if f.rotated { f.width } else { f.height }.max(1);
                    (vw, vh)
                })
                .collect();

            let has_offsets = anim.frames.iter().zip(vis_dims.iter()).any(|(f, &(vw, vh))| {
                f.frame_x != 0 || f.frame_y != 0 || f.frame_width > vw || f.frame_height > vh
            });

            let (span_w, span_h, rel_offsets) = if !has_offsets {
                // Atlas sin frameX/frameY: centrar cada frame horizontalmente y alinearlo a la base.
                let mut max_w = 1_u32;
                let mut max_h = 1_u32;
                for &(vw, vh) in &vis_dims {
                    max_w = max_w.max(vw);
                    max_h = max_h.max(vh);
                }
                let rels = vis_dims
                    .iter()
                    .map(|&(vw, vh)| {
                        let rx = ((max_w.saturating_sub(vw)) as i32) / 2;
                        let ry = (max_h.saturating_sub(vh)) as i32;
                        (rx, ry)
                    })
                    .collect();
                (max_w, max_h, rels)
            } else {
                // Atlas con offsets: calcular la caja visual ajustada de la animación conservando
                // los desplazamientos relativos entre sus propios frames.
                let mut min_x = i32::MAX;
                let mut min_y = i32::MAX;
                let mut max_x = i32::MIN;
                let mut max_y = i32::MIN;
                let mut raw_coords = Vec::with_capacity(anim.frames.len());

                for (f, &(vw, vh)) in anim.frames.iter().zip(vis_dims.iter()) {
                    let vx0 = -f.frame_x;
                    let vy0 = -f.frame_y;
                    let vx1 = vx0.saturating_add(vw as i32);
                    let vy1 = vy0.saturating_add(vh as i32);
                    min_x = min_x.min(vx0);
                    min_y = min_y.min(vy0);
                    max_x = max_x.max(vx1);
                    max_y = max_y.max(vy1);
                    raw_coords.push((vx0, vy0));
                }

                let sw = (max_x.saturating_sub(min_x)).max(1) as u32;
                let sh = (max_y.saturating_sub(min_y)).max(1) as u32;
                let rels = raw_coords
                    .into_iter()
                    .map(|(vx0, vy0)| (vx0.saturating_sub(min_x), vy0.saturating_sub(min_y)))
                    .collect();
                (sw, sh, rels)
            };

            unified_w = unified_w.max(span_w);
            unified_h = unified_h.max(span_h);
            layouts.push(AnimFrameLayout {
                prefix: prefix.clone(),
                span_w,
                span_h,
                rel_offsets,
            });
        }

        for layout in layouts {
            let Some(anim) = self.animations.get_mut(&layout.prefix) else {
                continue;
            };
            let pad_x = ((unified_w.saturating_sub(layout.span_w)) as i32) / 2;
            let pad_y = (unified_h.saturating_sub(layout.span_h)) as i32;
            for (frame, (rx, ry)) in anim.frames.iter_mut().zip(layout.rel_offsets.into_iter()) {
                frame.frame_x = -(rx.saturating_add(pad_x));
                frame.frame_y = -(ry.saturating_add(pad_y));
                frame.frame_width = unified_w;
                frame.frame_height = unified_h;
            }
        }
    }

    /// Alinea todas las animaciones usando los píxeles reales de la textura decodificada.
    /// Ancla el centro horizontal de los pies/base y el punto opaco más bajo al mismo punto
    /// compartido `(unified_w / 2, target_floor_y)`, eliminando saltos hacia los lados o hacia arriba
    /// cuando una pose asimétrica estira brazos/cabello o tiene márgenes transparentes inferiores.
    pub fn align_animations_with_image(&mut self, img: &image::DynamicImage) {
        let rgba_img = img.to_rgba8();
        self.align_animations_with_rgba(rgba_img.width(), rgba_img.height(), rgba_img.as_raw());
    }

    /// Variante pura sobre buffer RGBA8 (`align_animations_with_image`) testeable sin E/S de disco.
    pub fn align_animations_with_rgba(&mut self, img_w: u32, img_h: u32, rgba: &[u8]) {
        if self.animations.is_empty()
            || img_w == 0
            || img_h == 0
            || rgba.len() < (img_w as usize) * (img_h as usize) * 4
        {
            self.normalize_and_center_animations();
            return;
        }

        struct FrameAnchorSpec {
            vx0: i32,
            vy0: i32,
            vw: i32,
            vh: i32,
            foot_x: f32,
            floor_y: f32,
            has_opaque: bool,
        }

        enum AlignGroupKind {
            PerAnim {
                prefix: String,
                foot_x: i32,
                floor_y: i32,
                frames: Vec<(i32, i32)>,
                rect_min_x: i32,
                rect_max_x: i32,
                rect_min_y: i32,
                rect_max_y: i32,
            },
            PerFrame {
                prefix: String,
                frames: Vec<(i32, i32, i32, i32)>, // (foot_x, floor_y, vw, vh)
            },
        }

        let mut groups: Vec<AlignGroupKind> = Vec::with_capacity(self.animations.len());
        let mut max_half_w = 1_i32;
        let mut max_top_extent = 1_i32;
        let mut max_bottom_extent = 0_i32;

        for (prefix, anim) in &self.animations {
            if anim.frames.is_empty() {
                continue;
            }

            let mut specs: Vec<FrameAnchorSpec> = Vec::with_capacity(anim.frames.len());
            let mut has_offsets = false;

            for f in &anim.frames {
                let vw = if f.rotated { f.height } else { f.width }.max(1);
                let vh = if f.rotated { f.width } else { f.height }.max(1);
                if f.frame_x != 0 || f.frame_y != 0 || f.frame_width > vw || f.frame_height > vh {
                    has_offsets = true;
                }
                let vx0 = -f.frame_x;
                let vy0 = -f.frame_y;
                let (foot_x, floor_y, has_opaque) =
                    detect_frame_foot_anchor(f, img_w, img_h, rgba);
                specs.push(FrameAnchorSpec {
                    vx0,
                    vy0,
                    vw: vw as i32,
                    vh: vh as i32,
                    foot_x,
                    floor_y,
                    has_opaque,
                });
            }

            if !has_offsets {
                let mut per_frames = Vec::with_capacity(specs.len());
                for s in specs {
                    let fx = s.foot_x.round() as i32;
                    let fy = s.floor_y.round() as i32;
                    let left_ext = fx.max(1);
                    let right_ext = (s.vw - fx).max(1);
                    let top_ext = fy.max(1);
                    let bottom_ext = (s.vh - fy).max(0);
                    max_half_w = max_half_w.max(left_ext).max(right_ext);
                    max_top_extent = max_top_extent.max(top_ext);
                    max_bottom_extent = max_bottom_extent.max(bottom_ext);
                    per_frames.push((fx, fy, s.vw, s.vh));
                }
                groups.push(AlignGroupKind::PerFrame {
                    prefix: prefix.clone(),
                    frames: per_frames,
                });
            } else {
                let mut rect_min_x = i32::MAX;
                let mut rect_max_x = i32::MIN;
                let mut rect_min_y = i32::MAX;
                let mut rect_max_y = i32::MIN;
                let mut foot_samples: Vec<f32> = Vec::with_capacity(specs.len());
                let mut floor_samples: Vec<f32> = Vec::with_capacity(specs.len());
                let mut raw_coords: Vec<(i32, i32)> = Vec::with_capacity(specs.len());

                for s in &specs {
                    rect_min_x = rect_min_x.min(s.vx0);
                    rect_min_y = rect_min_y.min(s.vy0);
                    rect_max_x = rect_max_x.max(s.vx0.saturating_add(s.vw));
                    rect_max_y = rect_max_y.max(s.vy0.saturating_add(s.vh));
                    raw_coords.push((s.vx0, s.vy0));
                    if s.has_opaque {
                        foot_samples.push(s.vx0 as f32 + s.foot_x);
                        floor_samples.push(s.vy0 as f32 + s.floor_y);
                    }
                }

                let anim_foot_x = if foot_samples.is_empty() {
                    ((rect_min_x + rect_max_x) as f32 * 0.5).round() as i32
                } else {
                    median_f32(&mut foot_samples).round() as i32
                };
                let anim_floor_y = if floor_samples.is_empty() {
                    rect_max_y
                } else {
                    median_f32(&mut floor_samples).round() as i32
                };

                let left_ext = (anim_foot_x - rect_min_x).max(1);
                let right_ext = (rect_max_x - anim_foot_x).max(1);
                let top_ext = (anim_floor_y - rect_min_y).max(1);
                let bottom_ext = (rect_max_y - anim_floor_y).max(0);
                max_half_w = max_half_w.max(left_ext).max(right_ext);
                max_top_extent = max_top_extent.max(top_ext);
                max_bottom_extent = max_bottom_extent.max(bottom_ext);

                groups.push(AlignGroupKind::PerAnim {
                    prefix: prefix.clone(),
                    foot_x: anim_foot_x,
                    floor_y: anim_floor_y,
                    frames: raw_coords,
                    rect_min_x,
                    rect_max_x,
                    rect_min_y,
                    rect_max_y,
                });
            }
        }

        let unified_w = (max_half_w.saturating_mul(2)).max(1) as u32;
        let unified_h = (max_top_extent.saturating_add(max_bottom_extent)).max(1) as u32;

        for group in groups {
            match group {
                AlignGroupKind::PerAnim {
                    prefix,
                    foot_x,
                    floor_y,
                    frames,
                    rect_min_x: _,
                    rect_max_x: _,
                    rect_min_y: _,
                    rect_max_y: _,
                } => {
                    let Some(anim) = self.animations.get_mut(&prefix) else {
                        continue;
                    };
                    let shift_x = max_half_w.saturating_sub(foot_x);
                    let shift_y = max_top_extent.saturating_sub(floor_y);
                    for (frame, (vx0, vy0)) in anim.frames.iter_mut().zip(frames.into_iter()) {
                        let new_vx0 = vx0.saturating_add(shift_x);
                        let new_vy0 = vy0.saturating_add(shift_y);
                        frame.frame_x = -new_vx0;
                        frame.frame_y = -new_vy0;
                        frame.frame_width = unified_w;
                        frame.frame_height = unified_h;
                    }
                }
                AlignGroupKind::PerFrame { prefix, frames } => {
                    let Some(anim) = self.animations.get_mut(&prefix) else {
                        continue;
                    };
                    for (frame, (fx, fy, _vw, _vh)) in anim.frames.iter_mut().zip(frames.into_iter())
                    {
                        let new_vx0 = max_half_w.saturating_sub(fx);
                        let new_vy0 = max_top_extent.saturating_sub(fy);
                        frame.frame_x = -new_vx0;
                        frame.frame_y = -new_vy0;
                        frame.frame_width = unified_w;
                        frame.frame_height = unified_h;
                    }
                }
            }
        }
    }
}

fn median_f32(values: &mut [f32]) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let mid = values.len() / 2;
    if values.len() % 2 == 1 {
        values[mid]
    } else {
        (values[mid - 1] + values[mid]) * 0.5
    }
}

#[inline]
fn sample_frame_alpha(
    f: &SparrowFrame,
    lx: u32,
    ly: u32,
    img_w: u32,
    img_h: u32,
    rgba: &[u8],
) -> u8 {
    let (tx, ty) = if f.rotated {
        let rev_y = f.width.saturating_sub(1).saturating_sub(ly);
        (f.x.saturating_add(rev_y), f.y.saturating_add(lx))
    } else {
        (f.x.saturating_add(lx), f.y.saturating_add(ly))
    };
    if tx >= img_w || ty >= img_h {
        return 0;
    }
    let idx = ((ty as usize) * (img_w as usize) + (tx as usize)) * 4 + 3;
    rgba.get(idx).copied().unwrap_or(0)
}

/// Detecta `(foot_center_x, floor_y, found_opaque)` en coordenadas visuales `[0..vw] x [0..vh]`
/// para un `SparrowFrame` respetando rotación `+90°` y descartando halo/ruido tenue.
fn detect_frame_foot_anchor(
    f: &SparrowFrame,
    img_w: u32,
    img_h: u32,
    rgba: &[u8],
) -> (f32, f32, bool) {
    let vw = if f.rotated { f.height } else { f.width }.max(1);
    let vh = if f.rotated { f.width } else { f.height }.max(1);
    let step_x = if vw > 640 { 2 } else { 1 };
    let min_row_hits = if vw >= 8 && step_x == 1 { 2_u32 } else { 1_u32 };

    for threshold in [28_u8, 1_u8] {
        let mut op_min_y = u32::MAX;
        let mut op_max_y_excl = 0_u32;

        for ly in 0..vh {
            let mut row_hits = 0_u32;
            let mut lx = 0_u32;
            while lx < vw {
                if sample_frame_alpha(f, lx, ly, img_w, img_h, rgba) >= threshold {
                    row_hits += 1;
                    if row_hits >= min_row_hits {
                        break;
                    }
                }
                lx += step_x;
            }
            if row_hits >= min_row_hits {
                op_min_y = op_min_y.min(ly);
                op_max_y_excl = ly + 1;
            }
        }

        if op_max_y_excl <= op_min_y {
            continue;
        }

        let op_h = op_max_y_excl - op_min_y;
        let band_h = ((op_h as f32) * 0.16).round().clamp(6.0, 120.0) as u32;
        let band_y0 = op_max_y_excl.saturating_sub(band_h).max(op_min_y);

        let mut sum_x = 0_f64;
        let mut count = 0_u64;
        for ly in band_y0..op_max_y_excl {
            for lx in 0..vw {
                if sample_frame_alpha(f, lx, ly, img_w, img_h, rgba) >= threshold {
                    sum_x += (lx as f64) + 0.5;
                    count += 1;
                }
            }
        }

        let foot_x = if count > 0 {
            (sum_x / count as f64) as f32
        } else {
            (vw as f32) * 0.5
        };
        return (foot_x, op_max_y_excl as f32, true);
    }

    ((vw as f32) * 0.5, vh as f32, false)
}
