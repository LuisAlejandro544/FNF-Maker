//! Archivo: engine/crates/pulse_core/src/atlas_json.rs
//! Propósito: Parser y ensamblador de formatos JSON exóticos usados en mods optimizados de ritmo:
//! 1. TexturePacker / Aseprite JSON (Hash y Array) -> convertido directo a `SparrowAtlas`.
//! 2. Adobe Animate `spritemap1.json` (`ATLAS.SPRITES`) -> rectángulos de piezas o frames.
//! 3. Adobe Animate `Animation.json` (`AN` + `SD` + `TL` + `MX`/`M3D`) -> evalúa jerarquías,
//!    omite capas ocultas/máscaras (`Clipper`/`Guide`), aplica transformaciones de color/alfa (`C`),
//!    des-rota piezas `+90° CW` (`rotated: true`) con muestreo bilineal premultiplicado sin sangrado,
//!    deduplica poses repetidas y empaqueta un spritesheet uniforme `<= 4096x4096` para `SpriteLab` y `Gameplay`.

use crate::sparrow::{AnimSet, SparrowAtlas, SparrowFrame, natural_frame_cmp, strip_trailing_frame_digits};
use image::{DynamicImage, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};

/// Clasificación detectada automáticamente al leer cualquier archivo `.json` de sprites.
#[derive(Debug, Clone)]
pub enum DetectedSpriteJson {
    /// Atlas directo (TexturePacker Hash/Array o Aseprite) listo como `SparrowAtlas`.
    DirectAtlas(SparrowAtlas),
    /// `spritemap1.json` de Adobe Animate (`ATLAS.SPRITES`).
    AnimateSpritemap(AnimateSpritemap),
    /// `Animation.json` de Adobe Animate (`AN` / `SD` / `TL`).
    AnimateDocument(AnimateDocument),
}

/// Rectángulo individual dentro de `spritemap1.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpritemapRect {
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub rotated: bool,
}

/// Mapa completo extraído de `spritemap1.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimateSpritemap {
    pub image_hint: String,
    pub sprites: HashMap<String, SpritemapRect>,
    pub ordered_names: Vec<String>,
}

impl AnimateSpritemap {
    /// Si el usuario cargó solo `spritemap1.json` sin `Animation.json`, genera un atlas
    /// previsualizable con todas las piezas/frames detectados.
    pub fn to_fallback_sparrow(&self) -> SparrowAtlas {
        let mut grouped: BTreeMap<String, Vec<SparrowFrame>> = BTreeMap::new();
        for name in &self.ordered_names {
            if let Some(rect) = self.sprites.get(name) {
                let raw_prefix = strip_trailing_frame_digits(name);
                let prefix = if raw_prefix.chars().all(|c| c.is_ascii_digit()) || raw_prefix.is_empty() {
                    "spritemap".to_string()
                } else {
                    raw_prefix
                };
                grouped.entry(prefix).or_default().push(SparrowFrame {
                    raw_name: name.clone(),
                    x: rect.x,
                    y: rect.y,
                    width: rect.w.max(1),
                    height: rect.h.max(1),
                    frame_x: 0,
                    frame_y: 0,
                    frame_width: if rect.rotated { rect.h.max(1) } else { rect.w.max(1) },
                    frame_height: if rect.rotated { rect.w.max(1) } else { rect.h.max(1) },
                    rotated: rect.rotated,
                });
            }
        }

        let mut animations = BTreeMap::new();
        for (prefix, mut frames) in grouped {
            frames.sort_by(|a, b| natural_frame_cmp(&a.raw_name, &b.raw_name));
            let low = prefix.to_ascii_lowercase();
            let looped = low.contains("idle") || low.contains("dance") || low == "spritemap";
            animations.insert(
                prefix.clone(),
                AnimSet {
                    prefix,
                    fps: 24.0,
                    looped,
                    frames,
                },
            );
        }

        let bindings = SparrowAtlas::auto_detect_bindings(&animations);
        SparrowAtlas {
            image_path_hint: self.image_hint.clone(),
            animations,
            bindings,
        }
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SymbolLoopMode {
    Loop,
    PlayOnce,
    SingleFrame,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnimateElement {
    Sprite {
        name: String,
        matrix: Affine2D,
        color: ColorTransform2D,
    },
    Symbol {
        name: String,
        matrix: Affine2D,
        loop_mode: SymbolLoopMode,
        reverse: bool,
        first_frame: usize,
        color: ColorTransform2D,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimateFrame {
    pub index: usize,
    pub duration: usize,
    pub label: Option<String>,
    pub color: ColorTransform2D,
    pub elements: Vec<AnimateElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimateLayer {
    pub name: String,
    pub visible: bool,
    pub is_mask_or_guide: bool,
    pub frames: Vec<AnimateFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimateTimeline {
    pub layers: Vec<AnimateLayer>,
    pub total_frames: usize,
}

/// Documento completo `Animation.json` de Adobe Animate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimateDocument {
    pub name: String,
    pub framerate: f32,
    pub main_timeline: AnimateTimeline,
    pub symbols: HashMap<String, AnimateTimeline>,
    pub detected_animations: Vec<String>,
}

/// Analiza cualquier archivo `.json` de sprite y determina su formato real.
pub fn parse_sprite_json_str(raw: &str) -> Result<DetectedSpriteJson, String> {
    let clean = raw.trim_start_matches('\u{FEFF}');
    let root: Value = serde_json::from_str(clean)
        .map_err(|e| format!("Error leyendo JSON de sprite: {}", e))?;

    if root.get("AN").is_some() || root.get("ANIMATION").is_some() {
        let doc = parse_animation_json(&root)?;
        return Ok(DetectedSpriteJson::AnimateDocument(doc));
    }

    if root.get("ATLAS").is_some() {
        let sm = parse_spritemap_json(&root)?;
        return Ok(DetectedSpriteJson::AnimateSpritemap(sm));
    }

    if root.get("frames").is_some() {
        let atlas = parse_texture_packer_json(&root)?;
        return Ok(DetectedSpriteJson::DirectAtlas(atlas));
    }

    Err("El archivo .json no corresponde a TexturePacker, spritemap1.json ni Animation.json".to_string())
}

fn parse_spritemap_json(root: &Value) -> Result<AnimateSpritemap, String> {
    let atlas_obj = root
        .get("ATLAS")
        .ok_or_else(|| "Falta objeto 'ATLAS' en spritemap".to_string())?;
    let sprites_arr = atlas_obj
        .get("SPRITES")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Falta arreglo 'ATLAS.SPRITES'".to_string())?;

    let image_hint = root
        .get("meta")
        .and_then(|m| m.get("image"))
        .and_then(|v| v.as_str())
        .unwrap_or("spritemap1.png")
        .to_string();

    let mut sprites = HashMap::new();
    let mut ordered_names = Vec::new();

    for entry in sprites_arr {
        let sp = entry.get("SPRITE").unwrap_or(entry);
        let Some(name) = sp.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let x = val_u32(sp.get("x"));
        let y = val_u32(sp.get("y"));
        let w = val_u32(sp.get("w")).max(1);
        let h = val_u32(sp.get("h")).max(1);
        let rotated = sp
            .get("rotated")
            .and_then(|v| {
                v.as_bool()
                    .or_else(|| v.as_u64().map(|n| n != 0))
                    .or_else(|| v.as_str().map(|s| s.eq_ignore_ascii_case("true") || s == "1"))
            })
            .unwrap_or(false);
        ordered_names.push(name.to_string());
        sprites.insert(
            name.to_string(),
            SpritemapRect {
                name: name.to_string(),
                x,
                y,
                w,
                h,
                rotated,
            },
        );
    }

    if sprites.is_empty() {
        return Err("spritemap1.json no contiene rectángulos SPRITE".to_string());
    }

    Ok(AnimateSpritemap {
        image_hint,
        sprites,
        ordered_names,
    })
}

fn parse_texture_packer_json(root: &Value) -> Result<SparrowAtlas, String> {
    let image_hint = root
        .get("meta")
        .and_then(|m| m.get("image"))
        .and_then(|v| v.as_str())
        .unwrap_or("spritesheet.png")
        .to_string();

    let mut grouped: BTreeMap<String, Vec<SparrowFrame>> = BTreeMap::new();
    let frames_val = root
        .get("frames")
        .ok_or_else(|| "Falta campo 'frames' en atlas JSON".to_string())?;

    let mut push_frame = |name: String, item: &Value| {
        let rect = item.get("frame").unwrap_or(item);
        let x = val_u32(rect.get("x"));
        let y = val_u32(rect.get("y"));
        let w = val_u32(rect.get("w")).max(1);
        let h = val_u32(rect.get("h")).max(1);
        let rotated = item.get("rotated").and_then(|v| v.as_bool()).unwrap_or(false);

        let src_offset = item.get("spriteSourceSize");
        let fx = -(val_u32(src_offset.and_then(|o| o.get("x"))) as i32);
        let fy = -(val_u32(src_offset.and_then(|o| o.get("y"))) as i32);

        let src_size = item.get("sourceSize");
        let visual_w = if rotated { h } else { w };
        let visual_h = if rotated { w } else { h };
        let fw = val_u32(src_size.and_then(|o| o.get("w"))).max(visual_w);
        let fh = val_u32(src_size.and_then(|o| o.get("h"))).max(visual_h);

        let prefix = strip_trailing_frame_digits(&name);
        grouped.entry(prefix).or_default().push(SparrowFrame {
            raw_name: name,
            x,
            y,
            width: w,
            height: h,
            frame_x: fx,
            frame_y: fy,
            frame_width: fw,
            frame_height: fh,
            rotated,
        });
    };

    if let Some(obj) = frames_val.as_object() {
        for (name, item) in obj {
            push_frame(name.clone(), item);
        }
    } else if let Some(arr) = frames_val.as_array() {
        for (idx, item) in arr.iter().enumerate() {
            let name = item
                .get("filename")
                .or_else(|| item.get("name"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("frame_{:04}", idx));
            push_frame(name, item);
        }
    }

    if grouped.is_empty() {
        return Err("El atlas JSON no contiene fotogramas en 'frames'".to_string());
    }

    let mut animations = BTreeMap::new();
    for (prefix, mut frames) in grouped {
        frames.sort_by(|a, b| natural_frame_cmp(&a.raw_name, &b.raw_name));
        let low = prefix.to_ascii_lowercase();
        let looped = low.contains("idle") || low.contains("dance") || low.contains("loop");
        animations.insert(
            prefix.clone(),
            AnimSet {
                prefix,
                fps: 24.0,
                looped,
                frames,
            },
        );
    }

    let bindings = SparrowAtlas::auto_detect_bindings(&animations);
    Ok(SparrowAtlas {
        image_path_hint: image_hint,
        animations,
        bindings,
    })
}

fn parse_animation_json(root: &Value) -> Result<AnimateDocument, String> {
    let an = root
        .get("AN")
        .or_else(|| root.get("ANIMATION"))
        .ok_or_else(|| "Falta nodo 'AN'/'ANIMATION' en Animation.json".to_string())?;

    let name = an
        .get("N")
        .or_else(|| an.get("NAME"))
        .and_then(|v| v.as_str())
        .unwrap_or("AnimateCharacter")
        .to_string();

    let framerate = root
        .get("MD")
        .or_else(|| root.get("metadata"))
        .and_then(|m| m.get("FRT").or_else(|| m.get("framerate")))
        .and_then(|v| v.as_f64())
        .unwrap_or(24.0)
        .clamp(1.0, 60.0) as f32;

    let tl_val = an
        .get("TL")
        .or_else(|| an.get("TIMELINE"))
        .ok_or_else(|| "Falta 'TL'/'TIMELINE' en Animation.json".to_string())?;
    let main_timeline = parse_timeline(tl_val);

    let mut symbols = HashMap::new();
    if let Some(sd) = root.get("SD").or_else(|| root.get("SYMBOL_DICTIONARY")) {
        if let Some(sym_arr) = sd.get("S").or_else(|| sd.get("Symbols")).and_then(|v| v.as_array()) {
            for sym in sym_arr {
                let sym_name = sym
                    .get("SN")
                    .or_else(|| sym.get("SYMBOL_name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if sym_name.is_empty() {
                    continue;
                }
                if let Some(stl) = sym.get("TL").or_else(|| sym.get("TIMELINE")) {
                    symbols.insert(sym_name, parse_timeline(stl));
                }
            }
        }
    }

    let detected_animations = extract_animation_segments(&main_timeline, &symbols)
        .into_iter()
        .map(|(anim_name, _)| anim_name)
        .collect();

    Ok(AnimateDocument {
        name,
        framerate,
        main_timeline,
        symbols,
        detected_animations,
    })
}

fn parse_timeline(tl: &Value) -> AnimateTimeline {
    let mut layers = Vec::new();
    let mut max_end = 0usize;

    if let Some(layer_arr) = tl.get("L").or_else(|| tl.get("LAYERS")).and_then(|v| v.as_array()) {
        for layer_val in layer_arr {
            let layer_name = layer_val
                .get("LN")
                .or_else(|| layer_val.get("Layer_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("Layer")
                .to_string();

            let visible = layer_val
                .get("V")
                .or_else(|| layer_val.get("visible"))
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            // En Animate/FlxAnimate, las capas con `LT`/`Layer_type` ("Clipper", "Guide", "Mask", "G")
            // definen plantillas de recorte o guías de pivote que NO deben rasterizarse directamente.
            let layer_type_raw = layer_val
                .get("LT")
                .or_else(|| layer_val.get("Layer_type"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim();
            let is_mask_or_guide = !layer_type_raw.is_empty()
                && !layer_type_raw.eq_ignore_ascii_case("normal")
                && !layer_type_raw.eq_ignore_ascii_case("clipped");

            let mut frames = Vec::new();
            if let Some(fr_arr) = layer_val.get("FR").or_else(|| layer_val.get("Frames")).and_then(|v| v.as_array()) {
                for fr in fr_arr {
                    let index = fr.get("I").or_else(|| fr.get("index")).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    let duration = fr.get("DU").or_else(|| fr.get("duration")).and_then(|v| v.as_u64()).unwrap_or(1).max(1) as usize;
                    max_end = max_end.max(index + duration);
                    let label = fr
                        .get("N")
                        .or_else(|| fr.get("name"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    let fr_color = parse_color_effect(fr);

                    let mut elements = Vec::new();
                    if let Some(el_arr) = fr.get("E").or_else(|| fr.get("elements")).and_then(|v| v.as_array()) {
                        for el in el_arr {
                            // `ASI` / `ATLAS_SPRITE_instance` referencia una pieza atómica en `spritemap1.json`
                            if let Some(asi) = el.get("ASI").or_else(|| el.get("ATLAS_SPRITE_instance")).or_else(|| el.get("SPRITE_Instance")) {
                                if let Some(sp_name) = asi.get("N").or_else(|| asi.get("name")).and_then(|v| v.as_str()) {
                                    let matrix = parse_matrix(asi);
                                    let color = parse_color_effect(asi);
                                    elements.push(AnimateElement::Sprite {
                                        name: sp_name.to_string(),
                                        matrix,
                                        color,
                                    });
                                }
                            // `SI` / `SYMBOL_Instance` referencia un símbolo anidado en `SD.S` o `SI.bitmap`
                            } else if let Some(si) = el.get("SI").or_else(|| el.get("SYMBOL_Instance")) {
                                let base_matrix = parse_matrix(si);
                                let color = parse_color_effect(si);

                                // Algunos exportadores antiguos colocan `SI.bitmap: { N, POS }`
                                if let Some(bmp_obj) = si.get("bitmap") {
                                    if let Some(bmp_name) = bmp_obj.get("N").or_else(|| bmp_obj.get("name")).and_then(|v| v.as_str()) {
                                        let pos_m = parse_pos_only(bmp_obj);
                                        elements.push(AnimateElement::Sprite {
                                            name: bmp_name.to_string(),
                                            matrix: base_matrix.mul(pos_m),
                                            color,
                                        });
                                        continue;
                                    }
                                }

                                let sym_name = si
                                    .get("SN")
                                    .or_else(|| si.get("SYMBOL_name"))
                                    .or_else(|| si.get("N"))
                                    .or_else(|| si.get("name"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                if !sym_name.is_empty() {
                                    // Si `SI` no tiene `SN` pero sí `N` sin propiedades de símbolo, permitir sprite fallback
                                    if si.get("SN").is_none() && si.get("SYMBOL_name").is_none() && si.get("LP").is_none() && si.get("ST").is_none() {
                                        elements.push(AnimateElement::Sprite {
                                            name: sym_name.to_string(),
                                            matrix: base_matrix,
                                            color,
                                        });
                                    } else {
                                        let lp = si
                                            .get("LP")
                                            .or_else(|| si.get("loop"))
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("LP");
                                        let lp_up = lp.to_ascii_uppercase();
                                        let reverse = lp_up.ends_with('R') && lp_up != "OTHER";
                                        let lp_base = lp_up.trim_end_matches('R');
                                        let loop_mode = match lp_base {
                                            "PO" | "PLAYONCE" | "PLAY_ONCE" => SymbolLoopMode::PlayOnce,
                                            "SF" | "SINGLEFRAME" | "SINGLE_FRAME" => SymbolLoopMode::SingleFrame,
                                            _ => SymbolLoopMode::Loop,
                                        };
                                        let first_frame = si
                                            .get("FF")
                                            .or_else(|| si.get("firstFrame"))
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0) as usize;
                                        elements.push(AnimateElement::Symbol {
                                            name: sym_name.to_string(),
                                            matrix: base_matrix,
                                            loop_mode,
                                            reverse,
                                            first_frame,
                                            color,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    frames.push(AnimateFrame {
                        index,
                        duration,
                        label,
                        color: fr_color,
                        elements,
                    });
                }
            }
            layers.push(AnimateLayer {
                name: layer_name,
                visible,
                is_mask_or_guide,
                frames,
            });
        }
    }

    AnimateTimeline {
        layers,
        total_frames: max_end.max(1),
    }
}

fn parse_color_effect(obj: &Value) -> ColorTransform2D {
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
            let tc_hex = c.get("TC").or_else(|| c.get("tintColor")).and_then(|v| v.as_str()).unwrap_or("#FFFFFF");
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

fn parse_pos_only(obj: &Value) -> Affine2D {
    if let Some(pos) = obj.get("POS").or_else(|| obj.get("Position")).and_then(|v| v.as_object()) {
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

fn parse_matrix(obj: &Value) -> Affine2D {
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
            let f = |k: &str, def: f32| mobj.get(k).and_then(|v| v.as_f64()).map(|n| n as f32).unwrap_or(def);
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

#[derive(Debug, Clone)]
enum AnimSegmentSource<'a> {
    MainRange { start: usize, len: usize },
    Symbol(&'a AnimateTimeline),
}

fn extract_animation_segments<'a>(
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
            let next_start = labeled.get(i + 1).map(|x| x.0).unwrap_or(main_tl.total_frames);
            // Si el label dura 1 frame actúa como marcador hasta la siguiente etiqueta; si tiene DU > 1 respeta su duración sin invadir la siguiente.
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
                AnimateElement::Sprite { name, matrix, color } => {
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
                                SymbolLoopMode::PlayOnce => (first_frame + local_offset).min(sub_total - 1),
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
fn pack_shelves(sizes: &[(usize, u32, u32)], max_atlas_w: u32, pad: u32) -> (u32, u32, Vec<(u32, u32)>) {
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by(|&a, &b| sizes[b].2.cmp(&sizes[a].2).then_with(|| sizes[b].1.cmp(&sizes[a].1)));

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
                let lw = if rect.rotated { rect.h as f32 } else { rect.w as f32 };
                let lh = if rect.rotated { rect.w as f32 } else { rect.h as f32 };
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
        return Err("No se pudo generar ningún frame desde Animation.json + spritemap1.json".to_string());
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

    // Escala inicial nítida (hasta 640px por pose sin reescalar hacia arriba) + autoajuste para caber <= 4096x4096.
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

    let logical_canvas_w = ((global_max_x - global_min_x) * world_scale).ceil().max(4.0) as u32;
    let logical_canvas_h = ((global_max_y - global_min_y) * world_scale).ceil().max(4.0) as u32;

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

            let lw = if rect.rotated { rect.h as f32 } else { rect.w as f32 };
            let lh = if rect.rotated { rect.w as f32 } else { rect.h as f32 };

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
                        // Adobe Animate / TexturePacker empaqueta sprites rotados +90° CW (`ANGLE_NEG_90` para restaurar).
                        // Inversa exacta: `u = (rect.w - 1) - ly`, `v = lx`.
                        let max_u = (rect.w as f32 - 1.0).max(0.0);
                        let max_v = (rect.h as f32 - 1.0).max(0.0);
                        ((max_u - (ly - 0.5)).clamp(0.0, max_u), (lx - 0.5).clamp(0.0, max_v))
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
        });
    }

    let mut animations: BTreeMap<String, AnimSet> = BTreeMap::new();
    for slot in timeline_slots {
        let entry = animations.entry(slot.anim_name.clone()).or_insert_with(|| {
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
    Ok((
        DynamicImage::ImageRgba8(sheet),
        SparrowAtlas {
            image_path_hint: format!("{}_baked.png", doc.name),
            animations,
            bindings,
        },
    ))
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

    let r = (p00[0] as f32 * a00 + p10[0] as f32 * a10 + p01[0] as f32 * a01 + p11[0] as f32 * a11) / out_a;
    let g = (p00[1] as f32 * a00 + p10[1] as f32 * a10 + p01[1] as f32 * a01 + p11[1] as f32 * a11) / out_a;
    let b = (p00[2] as f32 * a00 + p10[2] as f32 * a10 + p01[2] as f32 * a01 + p11[2] as f32 * a11) / out_a;

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

fn val_u32(v: Option<&Value>) -> u32 {
    v.and_then(|x| x.as_f64()).map(|f| f.max(0.0).round() as u32).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let DetectedSpriteJson::AnimateSpritemap(sm) = parse_sprite_json_str(sm_raw).unwrap() else {
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
        assert!(atlas.bindings.contains_key(&crate::sparrow::CharacterAnimKind::Idle));
        assert!(atlas.bindings.contains_key(&crate::sparrow::CharacterAnimKind::Left));
    }

    #[test]
    fn bakes_rotated_cw_sprite_skips_clipper_and_deduplicates_frames() {
        // Pieza original 4x8 girada +90° CW => guardada como 8x4 en `spritemap1`:
        // la barra superior original (y=0, x=0..4) queda en la columna derecha (u=7, v=0..4).
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

        let DetectedSpriteJson::AnimateSpritemap(sm) = parse_sprite_json_str(sm_raw).unwrap() else {
            panic!("spritemap");
        };
        let DetectedSpriteJson::AnimateDocument(doc) = parse_sprite_json_str(anim_raw).unwrap() else {
            panic!("animate doc");
        };

        let mut img = RgbaImage::new(16, 8);
        // Pintar columna u=7, v=0..4 de azul sólido (corresponde al borde superior y=0..1 del sprite des-rotado)
        for v in 0..4 {
            img.put_pixel(7, v, Rgba([20, 120, 255, 255]));
        }
        // Pintar mask_dot de rojo (NO debe aparecer en el horneado)
        for y in 0..4 {
            for x in 8..12 {
                img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }

        let (sheet, atlas) = bake_animate_atlas(&doc, &sm, &DynamicImage::ImageRgba8(img)).unwrap();
        assert!(sheet.width() <= 4096 && sheet.height() <= 4096);
        let frames = &atlas.animations["idle"].frames;
        assert_eq!(frames.len(), 3);
        // Las 3 poses idénticas deben compartir el mismo rectángulo en caché
        assert_eq!(frames[0].x, frames[1].x);
        assert_eq!(frames[0].y, frames[2].y);

        let f0 = &frames[0];
        let rgba = sheet.to_rgba8();
        // Verificar que la fila superior del frame des-rotado sea azul y no exista rojo de la máscara
        let top_px = rgba.get_pixel(f0.x + 1, f0.y);
        assert!(top_px[2] > 200 && top_px[0] < 60, "esperaba azul arriba al des-rotar +90° CW: {:?}", top_px);
        for py in f0.y..(f0.y + f0.height) {
            for px in f0.x..(f0.x + f0.width) {
                let p = rgba.get_pixel(px, py);
                assert!(p[0] < 50 || p[3] == 0, "se filtró rojo de Clipper/AM=0 en ({}, {}): {:?}", px, py, p);
            }
        }
    }
}
