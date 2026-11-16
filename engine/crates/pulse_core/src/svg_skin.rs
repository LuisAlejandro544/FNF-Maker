//! Archivo: engine/crates/pulse_core/src/svg_skin.rs
//! Propósito: Gestiona las skins de flechas en formato `.svg` libres de derechos de autor.
//! Incluye nuestro diseño geométrico propio integrado por defecto y permite al usuario
//! importar sus propios archivos `.svg` para cada dirección (Izquierda, Abajo, Arriba, Derecha).
//! Utiliza `resvg` (licencia MIT/Apache-2.0) para rasterizar los `.svg` a buffers RGBA nítidos.

use crate::chart::NoteDirection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SvgSkinError {
    #[error("Error leyendo SVG: {0}")]
    SvgParse(String),
    #[error("No se pudo crear el buffer de píxeles para el SVG")]
    PixmapAlloc,
}

/// Imagen RGBA ya rasterizada lista para subir como textura GPU en egui.
#[derive(Debug, Clone)]
pub struct RasterizedArrow {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Configuración de rutas SVG elegidas por el usuario (si está vacía, usa el SVG propio integrado).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArrowSkinSet {
    pub skin_name: String,
    pub custom_svg_paths: HashMap<NoteDirection, PathBuf>,
    pub custom_svg_sources: HashMap<NoteDirection, String>,
    pub note_size_px: u32,
}

impl ArrowSkinSet {
    pub fn bundled_default() -> Self {
        Self {
            skin_name: "Pulse Vector (Original)".to_string(),
            custom_svg_paths: HashMap::new(),
            custom_svg_sources: HashMap::new(),
            note_size_px: 128,
        }
    }

    pub fn svg_for(&self, dir: NoteDirection) -> String {
        if let Some(custom) = self.custom_svg_sources.get(&dir) {
            return custom.clone();
        }
        default_arrow_svg_bundle(dir).to_string()
    }
}

/// Genera nuestra skin de flechas SVG original y libre de derechos de autor
/// con colores cómodos y contrastados (Cian, Coral, Índigo, Esmeralda).
pub fn default_arrow_svg_bundle(dir: NoteDirection) -> &'static str {
    match dir {
        NoteDirection::Left => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <defs>
    <linearGradient id="gLeft" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#F472B6"/>
      <stop offset="100%" stop-color="#DB2777"/>
    </linearGradient>
  </defs>
  <path d="M16 64 L64 18 L64 42 L110 42 L110 86 L64 86 L64 110 Z" fill="url(#gLeft)" stroke="#F8FAFC" stroke-width="6" stroke-linejoin="round"/>
  <path d="M32 64 L56 42 L56 54 L94 54 L94 74 L56 74 L56 86 Z" fill="#FDF2F8" fill-opacity="0.32"/>
</svg>"##,
        NoteDirection::Down => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <defs>
    <linearGradient id="gDown" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#22D3EE"/>
      <stop offset="100%" stop-color="#0284C7"/>
    </linearGradient>
  </defs>
  <path d="M64 112 L18 64 L42 64 L42 18 L86 18 L86 64 L110 64 Z" fill="url(#gDown)" stroke="#F8FAFC" stroke-width="6" stroke-linejoin="round"/>
  <path d="M64 96 L42 72 L54 72 L54 34 L74 34 L74 72 L86 72 Z" fill="#ECFEFF" fill-opacity="0.32"/>
</svg>"##,
        NoteDirection::Up => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <defs>
    <linearGradient id="gUp" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#34D399"/>
      <stop offset="100%" stop-color="#059669"/>
    </linearGradient>
  </defs>
  <path d="M64 16 L110 64 L86 64 L86 110 L42 110 L42 64 L18 64 Z" fill="url(#gUp)" stroke="#F8FAFC" stroke-width="6" stroke-linejoin="round"/>
  <path d="M64 32 L86 56 L74 56 L74 94 L54 94 L54 56 L42 56 Z" fill="#ECFDF5" fill-opacity="0.32"/>
</svg>"##,
        NoteDirection::Right => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <defs>
    <linearGradient id="gRight" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#818CF8"/>
      <stop offset="100%" stop-color="#4F46E5"/>
    </linearGradient>
  </defs>
  <path d="M112 64 L64 110 L64 86 L18 86 L18 42 L64 42 L64 18 Z" fill="url(#gRight)" stroke="#F8FAFC" stroke-width="6" stroke-linejoin="round"/>
  <path d="M96 64 L72 86 L72 74 L34 74 L34 54 L72 54 L72 42 Z" fill="#EEF2FF" fill-opacity="0.32"/>
</svg>"##,
    }
}

/// Rasteriza un string `.svg` a RGBA8888 usando `resvg` a la resolución indicada.
pub fn rasterize_svg_str(svg_str: &str, target_px: u32) -> Result<RasterizedArrow, SvgSkinError> {
    let size = target_px.clamp(32, 512);
    let opts = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(svg_str, &opts)
        .map_err(|e| SvgSkinError::SvgParse(e.to_string()))?;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)
        .ok_or(SvgSkinError::PixmapAlloc)?;

    let tree_size = tree.size();
    let sx = (size as f32) / tree_size.width().max(1.0);
    let sy = (size as f32) / tree_size.height().max(1.0);
    let transform = resvg::tiny_skia::Transform::from_scale(sx, sy);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // tiny-skia usa premultiplied RGBA; convertimos a RGBA recto para egui ColorImage.
    let mut rgba = pixmap.take();
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3];
        if a > 0 && a < 255 {
            let af = (a as f32) / 255.0;
            px[0] = ((px[0] as f32) / af).min(255.0) as u8;
            px[1] = ((px[1] as f32) / af).min(255.0) as u8;
            px[2] = ((px[2] as f32) / af).min(255.0) as u8;
        }
    }

    Ok(RasterizedArrow {
        width: size,
        height: size,
        rgba,
    })
}

/// Rasteriza las 4 direcciones de una skin de flechas.
pub fn rasterize_arrow_skin(
    skin: &ArrowSkinSet,
) -> Result<HashMap<NoteDirection, RasterizedArrow>, SvgSkinError> {
    let mut out = HashMap::with_capacity(4);
    for dir in NoteDirection::ALL {
        let svg = skin.svg_for(dir);
        let img = rasterize_svg_str(&svg, skin.note_size_px)?;
        out.insert(dir, img);
    }
    Ok(out)
}
