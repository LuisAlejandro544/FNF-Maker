//! Archivo: engine/crates/pulse_core/src/atlas_json/transform.rs
//! Propósito: Primitivas matemáticas 2D (`Affine2D`) y efectos de color/alfa (`ColorTransform2D`)
//! junto a los parsers de matrices (`MX`, `M3D`, `M`, `POS`) y efectos (`C`) usados en Adobe Animate.

use image::Rgba;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Matriz afín 2D `[a, b, c, d, tx, ty]` donde:
/// `x' = a*x + c*y + tx`
/// `y' = b*x + d*y + ty`
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Affine2D {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Default for Affine2D {
    fn default() -> Self {
        Self::identity()
    }
}

impl Affine2D {
    pub fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    pub fn mul(self, rhs: Self) -> Self {
        Self {
            a: self.a * rhs.a + self.c * rhs.b,
            b: self.b * rhs.a + self.d * rhs.b,
            c: self.a * rhs.c + self.c * rhs.d,
            d: self.b * rhs.c + self.d * rhs.d,
            tx: self.a * rhs.tx + self.c * rhs.ty + self.tx,
            ty: self.b * rhs.tx + self.d * rhs.ty + self.ty,
        }
    }

    pub fn transform_point(self, x: f32, y: f32) -> (f32, f32) {
        (self.a * x + self.c * y + self.tx, self.b * x + self.d * y + self.ty)
    }

    pub fn invert(self) -> Option<Self> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-6 {
            return None;
        }
        let inv_det = 1.0 / det;
        Some(Self {
            a: self.d * inv_det,
            b: -self.b * inv_det,
            c: -self.c * inv_det,
            d: self.a * inv_det,
            tx: (self.c * self.ty - self.d * self.tx) * inv_det,
            ty: (self.b * self.tx - self.a * self.ty) * inv_det,
        })
    }
}

/// Transformación de color/alfa por instancia o fotograma (`C` en Adobe Animate).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ColorTransform2D {
    pub rm: f32,
    pub gm: f32,
    pub bm: f32,
    pub am: f32,
    pub ro: f32,
    pub go: f32,
    pub bo: f32,
    pub ao: f32,
}

impl Default for ColorTransform2D {
    fn default() -> Self {
        Self::identity()
    }
}

impl ColorTransform2D {
    pub fn identity() -> Self {
        Self {
            rm: 1.0,
            gm: 1.0,
            bm: 1.0,
            am: 1.0,
            ro: 0.0,
            go: 0.0,
            bo: 0.0,
            ao: 0.0,
        }
    }

    pub fn is_identity(&self) -> bool {
        (self.rm - 1.0).abs() < 1e-3
            && (self.gm - 1.0).abs() < 1e-3
            && (self.bm - 1.0).abs() < 1e-3
            && (self.am - 1.0).abs() < 1e-3
            && self.ro.abs() < 0.5
            && self.go.abs() < 0.5
            && self.bo.abs() < 0.5
            && self.ao.abs() < 0.5
    }

    pub fn is_invisible(&self) -> bool {
        self.am <= 0.01 && self.ao <= 2.0
    }

    pub fn concat(self, child: Self) -> Self {
        Self {
            rm: self.rm * child.rm,
            gm: self.gm * child.gm,
            bm: self.bm * child.bm,
            am: self.am * child.am,
            ro: self.ro + self.rm * child.ro,
            go: self.go + self.gm * child.go,
            bo: self.bo + self.bm * child.bo,
            ao: self.ao + self.am * child.ao,
        }
    }

    #[inline]
    pub fn apply(self, px: Rgba<u8>) -> Rgba<u8> {
        if self.is_identity() {
            return px;
        }
        Rgba([
            (px[0] as f32 * self.rm + self.ro).clamp(0.0, 255.0).round() as u8,
            (px[1] as f32 * self.gm + self.go).clamp(0.0, 255.0).round() as u8,
            (px[2] as f32 * self.bm + self.bo).clamp(0.0, 255.0).round() as u8,
            (px[3] as f32 * self.am + self.ao).clamp(0.0, 255.0).round() as u8,
        ])
    }
}

pub(super) fn parse_color_effect(obj: &Value) -> ColorTransform2D {
    let Some(c) = obj.get("C").or_else(|| obj.get("color")) else {
        return ColorTransform2D::identity();
    };
    let mode = c
        .get("M")
        .or_else(|| c.get("mode"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_uppercase();

    let f = |keys: &[&str], def: f32| -> f32 {
        for k in keys {
            if let Some(num) = c.get(*k).and_then(|v| v.as_f64()) {
                return num as f32;
            }
        }
        def
    };

    match mode.as_str() {
        "CA" | "ALPHA" => {
            let am = f(&["AM", "alphaMultiplier"], 1.0).clamp(0.0, 2.0);
            ColorTransform2D {
                am,
                ..ColorTransform2D::identity()
            }
        }
        "T" | "TINT" => {
            let tm = f(&["TM", "tintMultiplier"], 0.0).clamp(0.0, 1.0);
            let tc_hex = c
                .get("TC")
                .or_else(|| c.get("tintColor"))
                .and_then(|v| v.as_str())
                .unwrap_or("#FFFFFF");
            let (r, g, b) = parse_hex_rgb(tc_hex);
            let inv = 1.0 - tm;
            ColorTransform2D {
                rm: inv,
                gm: inv,
                bm: inv,
                am: 1.0,
                ro: r * tm,
                go: g * tm,
                bo: b * tm,
                ao: 0.0,
            }
        }
        "CBRT" | "BRIGHTNESS" => {
            let brt = f(&["BRT", "Brightness", "brightness"], 0.0).clamp(-1.0, 1.0);
            let mult = 1.0 - brt.abs();
            let offset = if brt > 0.0 { 255.0 * brt } else { 0.0 };
            ColorTransform2D {
                rm: mult,
                gm: mult,
                bm: mult,
                am: 1.0,
                ro: offset,
                go: offset,
                bo: offset,
                ao: 0.0,
            }
        }
        "AD" | "ADVANCED" => ColorTransform2D {
            rm: f(&["RM", "RedMultiplier", "redMultiplier"], 1.0),
            gm: f(&["GM", "greenMultiplier", "GreenMultiplier"], 1.0),
            bm: f(&["BM", "blueMultiplier", "BlueMultiplier"], 1.0),
            am: f(&["AM", "alphaMultiplier", "AlphaMultiplier"], 1.0),
            ro: f(&["RO", "redOffset", "RedOffset"], 0.0),
            go: f(&["GO", "greenOffset", "GreenOffset"], 0.0),
            bo: f(&["BO", "blueOffset", "BlueOffset"], 0.0),
            ao: f(&["AO", "AlphaOffset", "alphaOffset"], 0.0),
        },
        _ => {
            let am = f(&["AM", "alphaMultiplier"], 1.0);
            ColorTransform2D {
                am,
                ..ColorTransform2D::identity()
            }
        }
    }
}

fn parse_hex_rgb(hex: &str) -> (f32, f32, f32) {
    let s = hex.trim().trim_start_matches('#').trim_start_matches("0x");
    if s.len() >= 6 {
        let r = u8::from_str_radix(&s[0..2], 16).unwrap_or(255) as f32;
        let g = u8::from_str_radix(&s[2..4], 16).unwrap_or(255) as f32;
        let b = u8::from_str_radix(&s[4..6], 16).unwrap_or(255) as f32;
        return (r, g, b);
    }
    (255.0, 255.0, 255.0)
}

pub(super) fn parse_pos_only(obj: &Value) -> Affine2D {
    if let Some(pos) = obj
        .get("POS")
        .or_else(|| obj.get("Position"))
        .and_then(|v| v.as_object())
    {
        let tx = pos.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let ty = pos.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        return Affine2D {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            tx,
            ty,
        };
    }
    Affine2D::identity()
}

pub(super) fn parse_matrix(obj: &Value) -> Affine2D {
    let pos_extra = obj
        .get("POS")
        .or_else(|| obj.get("Position"))
        .and_then(|v| v.as_object())
        .map(|p| {
            (
                p.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
                p.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
            )
        })
        .unwrap_or((0.0, 0.0));

    // Formato Better Texture Atlas (BTA 1.1.0+): `MX` / `Matrix: [a, b, c, d, tx, ty]`
    if let Some(mx) = obj.get("MX").or_else(|| obj.get("Matrix")).and_then(|v| v.as_array()) {
        if mx.len() >= 6 {
            let f = |i: usize| mx.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            return Affine2D {
                a: f(0),
                b: f(1),
                c: f(2),
                d: f(3),
                tx: f(4) + pos_extra.0,
                ty: f(5) + pos_extra.1,
            };
        }
    }
    // Formato estándar Adobe Animate: `M3D` o `Matrix3D` como arreglo de 16 floats column-major
    if let Some(m3d_val) = obj.get("M3D").or_else(|| obj.get("Matrix3D")) {
        if let Some(m3d) = m3d_val.as_array() {
            if m3d.len() >= 14 {
                let f = |i: usize| m3d.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                return Affine2D {
                    a: f(0),
                    b: f(1),
                    c: f(4),
                    d: f(5),
                    tx: f(12) + pos_extra.0,
                    ty: f(13) + pos_extra.1,
                };
            }
        } else if let Some(mobj) = m3d_val.as_object() {
            let f = |k: &str, def: f32| {
                mobj.get(k)
                    .and_then(|v| v.as_f64())
                    .map(|n| n as f32)
                    .unwrap_or(def)
            };
            return Affine2D {
                a: f("m00", 1.0),
                b: f("m01", 0.0),
                c: f("m10", 0.0),
                d: f("m11", 1.0),
                tx: f("m30", 0.0) + pos_extra.0,
                ty: f("m31", 0.0) + pos_extra.1,
            };
        }
    }
    if let Some(m) = obj.get("M").and_then(|v| v.as_array()) {
        if m.len() >= 6 {
            let f = |i: usize| m.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            return Affine2D {
                a: f(0),
                b: f(1),
                c: f(2),
                d: f(3),
                tx: f(4) + pos_extra.0,
                ty: f(5) + pos_extra.1,
            };
        }
    }
    Affine2D {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        tx: pos_extra.0,
        ty: pos_extra.1,
    }
}
