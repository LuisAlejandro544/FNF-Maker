//! Archivo: engine/crates/pulse_core/src/atlas_json/direct_formats.rs
//! Propósito: Parsers directos para atlas JSON planos:
//! 1. `spritemap1.json` de Adobe Animate (`ATLAS.SPRITES`) -> `AnimateSpritemap` con fallback `SparrowAtlas`.
//! 2. TexturePacker / Aseprite JSON (Hash y Array) -> convertido directo a `SparrowAtlas`.

use super::val_u32;
use crate::sparrow::{
    AnimSet, SparrowAtlas, SparrowFrame, natural_frame_cmp, strip_trailing_frame_digits,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

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
                let prefix =
                    if raw_prefix.chars().all(|c| c.is_ascii_digit()) || raw_prefix.is_empty() {
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
                    frame_width: if rect.rotated {
                        rect.h.max(1)
                    } else {
                        rect.w.max(1)
                    },
                    frame_height: if rect.rotated {
                        rect.w.max(1)
                    } else {
                        rect.h.max(1)
                    },
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
        let mut atlas = SparrowAtlas {
            image_path_hint: self.image_hint.clone(),
            animations,
            bindings,
        };
        atlas.normalize_and_center_animations();
        atlas
    }
}

pub(super) fn parse_spritemap_json(root: &Value) -> Result<AnimateSpritemap, String> {
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

pub(super) fn parse_texture_packer_json(root: &Value) -> Result<SparrowAtlas, String> {
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
    let mut atlas = SparrowAtlas {
        image_path_hint: image_hint,
        animations,
        bindings,
    };
    atlas.normalize_and_center_animations();
    Ok(atlas)
}
