//! Archivo: engine/crates/pulse_core/src/atlas_json.rs
//! Propósito: Parser y ensamblador de formatos JSON exóticos usados en mods optimizados de ritmo:
//! 1. TexturePacker / Aseprite JSON (Hash y Array) -> convertido directo a `SparrowAtlas`.
//! 2. Adobe Animate `spritemap1.json` (`ATLAS.SPRITES`) -> rectángulos de piezas o frames.
//! 3. Adobe Animate `Animation.json` (`AN` + `SD` + `TL` + `M3D`) -> evalúa jerarquías de
//!    símbolos y matrices afines 2D, rasteriza cada frame compuesto sobre un spritesheet RGBA
//!    limpio en caché y genera un `SparrowAtlas` listo para `SpriteLab` y `Gameplay`.

use crate::sparrow::{AnimSet, SparrowAtlas, SparrowFrame, strip_trailing_frame_digits};
use image::{DynamicImage, GenericImageView, RgbaImage};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

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
            frames.sort_by(|a, b| a.raw_name.cmp(&b.raw_name));
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
    },
    Symbol {
        name: String,
        matrix: Affine2D,
        loop_mode: SymbolLoopMode,
        first_frame: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimateFrame {
    pub index: usize,
    pub duration: usize,
    pub label: Option<String>,
    pub elements: Vec<AnimateElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimateLayer {
    pub name: String,
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
    let root: Value = serde_json::from_str(raw)
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
        let rotated = sp.get("rotated").and_then(|v| v.as_bool()).unwrap_or(false);
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
        let fw = val_u32(src_size.and_then(|o| o.get("w"))).max(w);
        let fh = val_u32(src_size.and_then(|o| o.get("h"))).max(h);

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
        frames.sort_by(|a, b| a.raw_name.cmp(&b.raw_name));
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

                    let mut elements = Vec::new();
                    if let Some(el_arr) = fr.get("E").or_else(|| fr.get("elements")).and_then(|v| v.as_array()) {
                        for el in el_arr {
                            if let Some(si) = el.get("SI").or_else(|| el.get("SPRITE_Instance")) {
                                if let Some(sp_name) = si.get("N").or_else(|| si.get("name")).and_then(|v| v.as_str()) {
                                    let matrix = parse_matrix(si);
                                    elements.push(AnimateElement::Sprite {
                                        name: sp_name.to_string(),
                                        matrix,
                                    });
                                }
                            } else if let Some(asi) = el.get("ASI").or_else(|| el.get("ATLAS_SPRITE_instance")) {
                                let sym_name = asi
                                    .get("N")
                                    .or_else(|| asi.get("name"))
                                    .or_else(|| asi.get("SN"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("");
                                if !sym_name.is_empty() {
                                    let matrix = parse_matrix(asi);
                                    let lp = asi
                                        .get("LP")
                                        .or_else(|| asi.get("loop"))
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("LP");
                                    let loop_mode = match lp.to_ascii_uppercase().as_str() {
                                        "PO" | "PLAYONCE" | "PLAY_ONCE" => SymbolLoopMode::PlayOnce,
                                        "SF" | "SINGLEFRAME" | "SINGLE_FRAME" => SymbolLoopMode::SingleFrame,
                                        _ => SymbolLoopMode::Loop,
                                    };
                                    let first_frame = asi
                                        .get("FF")
                                        .or_else(|| asi.get("firstFrame"))
                                        .and_then(|v| v.as_u64())
                                        .unwrap_or(0) as usize;
                                    elements.push(AnimateElement::Symbol {
                                        name: sym_name.to_string(),
                                        matrix,
                                        loop_mode,
                                        first_frame,
                                    });
                                }
                            }
                        }
                    }
                    frames.push(AnimateFrame {
                        index,
                        duration,
                        label,
                        elements,
                    });
                }
            }
            layers.push(AnimateLayer {
                name: layer_name,
                frames,
            });
        }
    }

    AnimateTimeline {
        layers,
        total_frames: max_end.max(1),
    }
}

fn parse_matrix(obj: &Value) -> Affine2D {
    if let Some(m3d) = obj.get("M3D").or_else(|| obj.get("Matrix3D")).and_then(|v| v.as_array()) {
        if m3d.len() >= 14 {
            let f = |i: usize| m3d.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
            return Affine2D {
                a: f(0),
                b: f(1),
                c: f(4),
                d: f(5),
                tx: f(12),
                ty: f(13),
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
                tx: f(4),
                ty: f(5),
            };
        }
    }
    Affine2D::identity()
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
    // 1. Buscar etiquetas de frame (`N`) en el timeline principal (formato clásico Animate FNF).
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
            let end = own_end.max(next_start).min(main_tl.total_frames);
            let len = end.saturating_sub(start).clamp(1, 72);
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
}

fn collect_draws_at_frame(
    tl: &AnimateTimeline,
    symbols: &HashMap<String, AnimateTimeline>,
    frame_idx: usize,
    parent_matrix: Affine2D,
    depth: usize,
    out: &mut Vec<DrawCall>,
) {
    if depth > 12 || out.len() > 256 {
        return;
    }
    // Adobe Animate lista las capas de arriba hacia abajo; se dibujan de abajo hacia arriba.
    for layer in tl.layers.iter().rev() {
        let Some(fr) = layer
            .frames
            .iter()
            .find(|f| frame_idx >= f.index && frame_idx < f.index + f.duration)
        else {
            continue;
        };

        let local_offset = frame_idx - fr.index;
        for el in &fr.elements {
            match el {
                AnimateElement::Sprite { name, matrix } => {
                    out.push(DrawCall {
                        sprite_name: name.clone(),
                        matrix: parent_matrix.mul(*matrix),
                    });
                }
                AnimateElement::Symbol {
                    name,
                    matrix,
                    loop_mode,
                    first_frame,
                } => {
                    if let Some(sub_tl) = symbols.get(name) {
                        let sub_total = sub_tl.total_frames.max(1);
                        let sub_frame = match loop_mode {
                            SymbolLoopMode::SingleFrame => (*first_frame).min(sub_total - 1),
                            SymbolLoopMode::PlayOnce => (first_frame + local_offset).min(sub_total - 1),
                            SymbolLoopMode::Loop => (first_frame + local_offset) % sub_total,
                        };
                        collect_draws_at_frame(
                            sub_tl,
                            symbols,
                            sub_frame,
                            parent_matrix.mul(*matrix),
                            depth + 1,
                            out,
                        );
                    }
                }
            }
        }
    }
}

/// Rasteriza por software todas las animaciones de `Animation.json` + `spritemap1.json` + textura
/// en una sola hoja RGBA empaquetada y devuelve `(hoja_png_rgba, SparrowAtlas)`.
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

    struct RenderedFrame {
        anim_name: String,
        frame_idx_in_anim: usize,
        img: RgbaImage,
    }

    let mut rendered_frames: Vec<RenderedFrame> = Vec::new();
    let max_frame_side = 512.0_f32;

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
                        0,
                        &mut draws,
                    );
                }
            }

            if draws.is_empty() {
                continue;
            }

            // Calcular caja delimitadora (bounding box) transformada en espacio virtual
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

            let raw_w = (max_x - min_x).ceil().max(4.0);
            let raw_h = (max_y - min_y).ceil().max(4.0);
            let downscale = (max_frame_side / raw_w.max(raw_h)).min(1.0);
            let out_w = (raw_w * downscale).ceil().max(4.0) as u32;
            let out_h = (raw_h * downscale).ceil().max(4.0) as u32;

            let mut frame_img = RgbaImage::new(out_w, out_h);
            let view_shift = Affine2D {
                a: downscale,
                b: 0.0,
                c: 0.0,
                d: downscale,
                tx: -min_x * downscale,
                ty: -min_y * downscale,
            };

            for dc in &draws {
                let Some(rect) = spritemap.sprites.get(&dc.sprite_name) else {
                    continue;
                };
                let full_m = view_shift.mul(dc.matrix);
                let Some(inv_m) = full_m.invert() else {
                    continue;
                };

                let lw = if rect.rotated { rect.h as f32 } else { rect.w as f32 };
                let lh = if rect.rotated { rect.w as f32 } else { rect.h as f32 };

                // Caja en destino para limitar el bucle por pieza
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

                let x0 = (d_min_x.floor() as i32).clamp(0, out_w as i32 - 1);
                let y0 = (d_min_y.floor() as i32).clamp(0, out_h as i32 - 1);
                let x1 = (d_max_x.ceil() as i32).clamp(0, out_w as i32 - 1);
                let y1 = (d_max_y.ceil() as i32).clamp(0, out_h as i32 - 1);

                for py in y0..=y1 {
                    for px in x0..=x1 {
                        let (lx, ly) = inv_m.transform_point(px as f32 + 0.5, py as f32 + 0.5);
                        if lx < 0.0 || ly < 0.0 || lx >= lw || ly >= lh {
                            continue;
                        }
                        let (sx, sy) = if rect.rotated {
                            // Sprite rotado 90 grados en el atlas
                            let rx = rect.x as f32 + ly;
                            let ry = rect.y as f32 + (rect.h as f32 - 1.0 - lx);
                            (rx.floor() as u32, ry.floor() as u32)
                        } else {
                            ((rect.x as f32 + lx).floor() as u32, (rect.y as f32 + ly).floor() as u32)
                        };

                        if sx >= src_w || sy >= src_h {
                            continue;
                        }
                        let src_px = src_rgba.get_pixel(sx, sy);
                        if src_px[3] == 0 {
                            continue;
                        }
                        alpha_blend(frame_img.get_pixel_mut(px as u32, py as u32), *src_px);
                    }
                }
            }

            rendered_frames.push(RenderedFrame {
                anim_name: anim_name.clone(),
                frame_idx_in_anim: f_i,
                img: frame_img,
            });
        }
    }

    if rendered_frames.is_empty() {
        return Err("No se pudo generar ningún frame desde Animation.json + spritemap1.json".to_string());
    }

    // Empaquetado en estantes (shelf packing) dentro de una textura máxima de 2048px de ancho
    let pad = 2_u32;
    let max_atlas_w = 2048_u32;
    let mut cur_x = pad;
    let mut cur_y = pad;
    let mut row_h = 0_u32;
    let mut sheet_w = 64_u32;

    let mut placements: Vec<(u32, u32, u32, u32)> = Vec::with_capacity(rendered_frames.len());
    for rf in &rendered_frames {
        let (fw, fh) = rf.img.dimensions();
        if cur_x + fw + pad > max_atlas_w && cur_x > pad {
            cur_x = pad;
            cur_y += row_h + pad;
            row_h = 0;
        }
        placements.push((cur_x, cur_y, fw, fh));
        sheet_w = sheet_w.max(cur_x + fw + pad);
        cur_x += fw + pad;
        row_h = row_h.max(fh);
    }
    let sheet_h = (cur_y + row_h + pad).max(64);
    let mut sheet = RgbaImage::new(sheet_w, sheet_h);

    let mut animations: BTreeMap<String, AnimSet> = BTreeMap::new();
    for (rf, (ox, oy, fw, fh)) in rendered_frames.into_iter().zip(placements.into_iter()) {
        image::imageops::overlay(&mut sheet, &rf.img, ox as i64, oy as i64);
        let entry = animations.entry(rf.anim_name.clone()).or_insert_with(|| {
            let low = rf.anim_name.to_ascii_lowercase();
            AnimSet {
                prefix: rf.anim_name.clone(),
                fps: doc.framerate,
                looped: low.contains("idle") || low.contains("dance") || low.contains("loop"),
                frames: Vec::new(),
            }
        });
        entry.frames.push(SparrowFrame {
            raw_name: format!("{}{:04}", rf.anim_name, rf.frame_idx_in_anim),
            x: ox,
            y: oy,
            width: fw,
            height: fh,
            frame_x: 0,
            frame_y: 0,
            frame_width: fw,
            frame_height: fh,
            rotated: false,
        });
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

#[inline]
fn alpha_blend(dst: &mut image::Rgba<u8>, src: image::Rgba<u8>) {
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
                "hero_idle0000": { "frame": {"x":0,"y":0,"w":80,"h":100}, "rotated": false },
                "hero_left0000": { "frame": {"x":80,"y":0,"w":80,"h":100}, "rotated": true }
            }
        }"#;
        let DetectedSpriteJson::DirectAtlas(atlas) = parse_sprite_json_str(tp_raw).unwrap() else {
            panic!("esperaba DirectAtlas");
        };
        assert_eq!(atlas.animations.len(), 2);
        assert!(atlas.bindings.contains_key(&crate::sparrow::CharacterAnimKind::Idle));
        assert!(atlas.bindings.contains_key(&crate::sparrow::CharacterAnimKind::Left));
    }

    #[test]
    fn parses_and_bakes_animate_json_with_spritemap() {
        let sm_raw = r#"{
            "ATLAS": {
                "SPRITES": [
                    { "SPRITE": { "name": "0000", "x": 0, "y": 0, "w": 8, "h": 8, "rotated": false } }
                ]
            }
        }"#;
        let anim_raw = r#"{
            "MD": { "FRT": 24 },
            "AN": {
                "N": "TestRig",
                "TL": {
                    "L": [
                        {
                            "LN": "Labels",
                            "FR": [
                                { "I": 0, "DU": 2, "N": "idle", "E": [
                                    { "SI": { "N": "0000", "M3D": [1,0,0,0, 0,1,0,0, 0,0,1,0, 10,12,0,1] } }
                                ]}
                            ]
                        }
                    ]
                }
            }
        }"#;

        let DetectedSpriteJson::AnimateSpritemap(sm) = parse_sprite_json_str(sm_raw).unwrap() else {
            panic!("spritemap");
        };
        let DetectedSpriteJson::AnimateDocument(doc) = parse_sprite_json_str(anim_raw).unwrap() else {
            panic!("animate doc");
        };

        let mut img = RgbaImage::new(16, 16);
        for y in 0..8 {
            for x in 0..8 {
                img.put_pixel(x, y, image::Rgba([255, 80, 20, 255]));
            }
        }
        let (sheet, atlas) = bake_animate_atlas(&doc, &sm, &DynamicImage::ImageRgba8(img)).unwrap();
        assert!(sheet.width() >= 16);
        assert!(atlas.animations.contains_key("idle"));
        assert_eq!(atlas.animations["idle"].frames.len(), 2);
    }
}
