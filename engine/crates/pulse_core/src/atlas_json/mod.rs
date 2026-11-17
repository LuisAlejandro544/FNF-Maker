//! Archivo: engine/crates/pulse_core/src/atlas_json/mod.rs
//! Propósito: Punto de entrada modular del subsistema `atlas_json`.
//! Orquesta la detección automática y carga de atlas JSON exóticos:
//! - `transform`: Matrices afines 2D (`Affine2D`) y transformaciones de color/alfa (`ColorTransform2D`).
//! - `direct_formats`: Parsers para TexturePacker / Aseprite JSON y `spritemap1.json` de Adobe Animate.
//! - `animate_model`: Estructuras y parser de timelines, símbolos y capas para `Animation.json` (clásico y BTA).
//! - `animate_baker`: Evaluador jerárquico y rasterizador por software de poses Animate a spritesheet RGBA.

pub mod animate_baker;
pub mod animate_model;
pub mod direct_formats;
pub mod transform;

pub use animate_baker::bake_animate_atlas;
pub use animate_model::{
    AnimateDocument, AnimateElement, AnimateFrame, AnimateLayer, AnimateTimeline, SymbolLoopMode,
};
pub use direct_formats::{AnimateSpritemap, SpritemapRect};
pub use transform::{Affine2D, ColorTransform2D};

use crate::sparrow::SparrowAtlas;
use serde_json::Value;

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

/// Analiza cualquier archivo `.json` de sprite y determina su formato real.
pub fn parse_sprite_json_str(raw: &str) -> Result<DetectedSpriteJson, String> {
    let clean = raw.trim_start_matches('\u{FEFF}');
    let root: Value =
        serde_json::from_str(clean).map_err(|e| format!("Error leyendo JSON de sprite: {}", e))?;

    if root.get("AN").is_some() || root.get("ANIMATION").is_some() {
        let doc = animate_model::parse_animation_json(&root)?;
        return Ok(DetectedSpriteJson::AnimateDocument(doc));
    }

    if root.get("ATLAS").is_some() {
        let sm = direct_formats::parse_spritemap_json(&root)?;
        return Ok(DetectedSpriteJson::AnimateSpritemap(sm));
    }

    if root.get("frames").is_some() {
        let atlas = direct_formats::parse_texture_packer_json(&root)?;
        return Ok(DetectedSpriteJson::DirectAtlas(atlas));
    }

    Err(
        "El archivo .json no corresponde a TexturePacker, spritemap1.json ni Animation.json"
            .to_string(),
    )
}

pub(super) fn val_u32(v: Option<&Value>) -> u32 {
    v.and_then(|x| x.as_f64())
        .map(|f| f.max(0.0).round() as u32)
        .unwrap_or(0)
}
