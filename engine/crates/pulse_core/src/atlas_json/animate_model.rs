//! Archivo: engine/crates/pulse_core/src/atlas_json/animate_model.rs
//! Propósito: Modelo de datos y parser completo de `Animation.json` de Adobe Animate
//! (soportando tanto formato clásico `M3D`/`SYMBOL_Instance` como Better Texture Atlas `BTA 1.1.0` `MX`/`SI`/`ASI`).

use super::animate_baker::extract_animation_segments;
use super::transform::{
    Affine2D, ColorTransform2D, parse_color_effect, parse_matrix, parse_pos_only,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

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

pub(super) fn parse_animation_json(root: &Value) -> Result<AnimateDocument, String> {
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
        if let Some(sym_arr) = sd
            .get("S")
            .or_else(|| sd.get("Symbols"))
            .and_then(|v| v.as_array())
        {
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

    if let Some(layer_arr) = tl
        .get("L")
        .or_else(|| tl.get("LAYERS"))
        .and_then(|v| v.as_array())
    {
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
            if let Some(fr_arr) = layer_val
                .get("FR")
                .or_else(|| layer_val.get("Frames"))
                .and_then(|v| v.as_array())
            {
                for fr in fr_arr {
                    let index = fr
                        .get("I")
                        .or_else(|| fr.get("index"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as usize;
                    let duration = fr
                        .get("DU")
                        .or_else(|| fr.get("duration"))
                        .and_then(|v| v.as_u64())
                        .unwrap_or(1)
                        .max(1) as usize;
                    max_end = max_end.max(index + duration);
                    let label = fr
                        .get("N")
                        .or_else(|| fr.get("name"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty());
                    let fr_color = parse_color_effect(fr);

                    let mut elements = Vec::new();
                    if let Some(el_arr) = fr
                        .get("E")
                        .or_else(|| fr.get("elements"))
                        .and_then(|v| v.as_array())
                    {
                        for el in el_arr {
                            // `ASI` / `ATLAS_SPRITE_instance` referencia una pieza atómica en `spritemap1.json`
                            if let Some(asi) = el
                                .get("ASI")
                                .or_else(|| el.get("ATLAS_SPRITE_instance"))
                                .or_else(|| el.get("SPRITE_Instance"))
                            {
                                if let Some(sp_name) =
                                    asi.get("N").or_else(|| asi.get("name")).and_then(|v| v.as_str())
                                {
                                    let matrix = parse_matrix(asi);
                                    let color = parse_color_effect(asi);
                                    elements.push(AnimateElement::Sprite {
                                        name: sp_name.to_string(),
                                        matrix,
                                        color,
                                    });
                                }
                            // `SI` / `SYMBOL_Instance` referencia un símbolo anidado en `SD.S` o `SI.bitmap`
                            } else if let Some(si) =
                                el.get("SI").or_else(|| el.get("SYMBOL_Instance"))
                            {
                                let base_matrix = parse_matrix(si);
                                let color = parse_color_effect(si);

                                // Algunos exportadores antiguos colocan `SI.bitmap: { N, POS }`
                                if let Some(bmp_obj) = si.get("bitmap") {
                                    if let Some(bmp_name) = bmp_obj
                                        .get("N")
                                        .or_else(|| bmp_obj.get("name"))
                                        .and_then(|v| v.as_str())
                                    {
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
                                    if si.get("SN").is_none()
                                        && si.get("SYMBOL_name").is_none()
                                        && si.get("LP").is_none()
                                        && si.get("ST").is_none()
                                    {
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
                                            "PO" | "PLAYONCE" | "PLAY_ONCE" => {
                                                SymbolLoopMode::PlayOnce
                                            }
                                            "SF" | "SINGLEFRAME" | "SINGLE_FRAME" => {
                                                SymbolLoopMode::SingleFrame
                                            }
                                            _ => SymbolLoopMode::Loop,
                                        };
                                        let first_frame = si
                                            .get("FF")
                                            .or_else(|| si.get("firstFrame"))
                                            .and_then(|v| v.as_u64())
                                            .unwrap_or(0)
                                            as usize;
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
