//! Archivo: engine/crates/pulse_core/src/svg_skin.rs
//! Propósito: Gestiona las skins de flechas, barra de vida, íconos por defecto,
//! calificaciones (`SICK!`, `GOOD!`, `BAD`, `MISS`) y destello de nota perfecta en formato `.svg`
//! libres de derechos de autor.
//! Utiliza `resvg` (licencia MIT/Apache-2.0) y geometría vectorial pura `<path>` (sin depender
//! de fuentes del sistema Android) para garantizar rasterización nítida en 32 y 64 bits.

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

/// Clasificación de precisión al golpear o fallar una nota.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RatingKind {
    Sick = 1,
    Good = 2,
    Bad = 3,
    Miss = 4,
}

impl RatingKind {
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(RatingKind::Sick),
            2 => Some(RatingKind::Good),
            3 => Some(RatingKind::Bad),
            4 => Some(RatingKind::Miss),
            _ => None,
        }
    }
}

/// Imagen RGBA ya rasterizada lista para subir como textura GPU / ImageBitmap.
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
/// para las notas con color vibrante (Rosa, Cian, Esmeralda, Índigo).
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

/// Genera los receptores principales sin color (gris neutro estilo FNF) para la barra fija.
pub fn default_receptor_svg_bundle(dir: NoteDirection) -> &'static str {
    match dir {
        NoteDirection::Left => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <path d="M16 64 L64 18 L64 42 L110 42 L110 86 L64 86 L64 110 Z" fill="#64748B" fill-opacity="0.72" stroke="#CBD5E1" stroke-width="6" stroke-linejoin="round"/>
  <path d="M32 64 L56 42 L56 54 L94 54 L94 74 L56 74 L56 86 Z" fill="#94A3B8" fill-opacity="0.35"/>
</svg>"##,
        NoteDirection::Down => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <path d="M64 112 L18 64 L42 64 L42 18 L86 18 L86 64 L110 64 Z" fill="#64748B" fill-opacity="0.72" stroke="#CBD5E1" stroke-width="6" stroke-linejoin="round"/>
  <path d="M64 96 L42 72 L54 72 L54 34 L74 34 L74 72 L86 72 Z" fill="#94A3B8" fill-opacity="0.35"/>
</svg>"##,
        NoteDirection::Up => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <path d="M64 16 L110 64 L86 64 L86 110 L42 110 L42 64 L18 64 Z" fill="#64748B" fill-opacity="0.72" stroke="#CBD5E1" stroke-width="6" stroke-linejoin="round"/>
  <path d="M64 32 L86 56 L74 56 L74 94 L54 94 L54 56 L42 56 Z" fill="#94A3B8" fill-opacity="0.35"/>
</svg>"##,
        NoteDirection::Right => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <path d="M112 64 L64 110 L64 86 L18 86 L18 42 L64 42 L64 18 Z" fill="#64748B" fill-opacity="0.72" stroke="#CBD5E1" stroke-width="6" stroke-linejoin="round"/>
  <path d="M96 64 L72 86 L72 74 L34 74 L34 54 L72 54 L72 42 Z" fill="#94A3B8" fill-opacity="0.35"/>
</svg>"##,
    }
}

/// Marco SVG de barra de vida estilo FNF (interior transparente para que el motor pinte
/// dinámicamente el relleno Rojo del Rival a la izquierda y Verde del Jugador a la derecha).
pub fn default_health_bar_frame_svg() -> &'static str {
    r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 600 36">
  <rect x="3" y="3" width="594" height="30" rx="8" ry="8" fill="none" stroke="#090D16" stroke-width="6"/>
  <rect x="7" y="7" width="586" height="22" rx="5" ry="5" fill="none" stroke="#1E293B" stroke-width="2.5"/>
  <path d="M14 9 L586 9" stroke="#FFFFFF" stroke-opacity="0.22" stroke-width="2" stroke-linecap="round"/>
</svg>"##
}

/// Destello SVG blanco/plateado para notas perfectas (`Sick`). Se tiñe en Kotlin según el color del carril.
pub fn default_note_splash_svg() -> &'static str {
    r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 160">
  <circle cx="80" cy="80" r="56" fill="none" stroke="#FFFFFF" stroke-opacity="0.88" stroke-width="7"/>
  <circle cx="80" cy="80" r="34" fill="none" stroke="#FFFFFF" stroke-opacity="0.72" stroke-width="4"/>
  <path d="M80 6 L92 48 L134 26 L108 64 L154 80 L108 96 L134 134 L92 112 L80 154 L68 112 L26 134 L52 96 L6 80 L52 64 L26 26 L68 48 Z" fill="#FFFFFF" fill-opacity="0.82"/>
  <circle cx="24" cy="44" r="5" fill="#FFFFFF"/>
  <circle cx="136" cy="44" r="5" fill="#FFFFFF"/>
  <circle cx="24" cy="116" r="5" fill="#FFFFFF"/>
  <circle cx="136" cy="116" r="5" fill="#FFFFFF"/>
</svg>"##
}

/// Ícono SVG original para el Jugador (normal o perdiendo <20% de vida).
pub fn default_player_icon_svg(losing: bool) -> &'static str {
    if losing {
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <circle cx="64" cy="68" r="44" fill="#22D3EE" stroke="#090D16" stroke-width="7"/>
  <path d="M22 58 C26 26, 102 26, 106 58 L98 48 L30 48 Z" fill="#0F172A" stroke="#090D16" stroke-width="5" stroke-linejoin="round"/>
  <rect x="14" y="54" width="14" height="30" rx="6" fill="#38BDF8" stroke="#090D16" stroke-width="5"/>
  <rect x="100" y="54" width="14" height="30" rx="6" fill="#38BDF8" stroke="#090D16" stroke-width="5"/>
  <path d="M40 58 L54 72 M54 58 L40 72" stroke="#090D16" stroke-width="6" stroke-linecap="round"/>
  <path d="M74 58 L88 72 M88 58 L74 72" stroke="#090D16" stroke-width="6" stroke-linecap="round"/>
  <path d="M48 92 Q64 80 80 92" fill="none" stroke="#090D16" stroke-width="6" stroke-linecap="round"/>
  <path d="M98 30 C106 38, 106 46, 98 46 C90 46, 90 38, 98 30 Z" fill="#BAE6FD" stroke="#090D16" stroke-width="3"/>
</svg>"##
    } else {
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <circle cx="64" cy="68" r="44" fill="#22D3EE" stroke="#090D16" stroke-width="7"/>
  <path d="M22 58 C26 26, 102 26, 106 58 L98 48 L30 48 Z" fill="#0F172A" stroke="#090D16" stroke-width="5" stroke-linejoin="round"/>
  <rect x="14" y="54" width="14" height="30" rx="6" fill="#38BDF8" stroke="#090D16" stroke-width="5"/>
  <rect x="100" y="54" width="14" height="30" rx="6" fill="#38BDF8" stroke="#090D16" stroke-width="5"/>
  <circle cx="48" cy="66" r="7" fill="#090D16"/>
  <circle cx="80" cy="66" r="7" fill="#090D16"/>
  <circle cx="46" cy="64" r="2.5" fill="#FFFFFF"/>
  <circle cx="78" cy="64" r="2.5" fill="#FFFFFF"/>
  <path d="M46 84 Q64 98 82 84" fill="none" stroke="#090D16" stroke-width="6" stroke-linecap="round"/>
</svg>"##
    }
}

/// Ícono SVG original para el Rival / Enemigo (normal o perdiendo >80% de vida a favor del jugador).
pub fn default_opponent_icon_svg(losing: bool) -> &'static str {
    if losing {
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <path d="M26 44 L16 18 L44 32 L84 32 L112 18 L102 44 C112 58, 110 92, 90 106 C74 114, 54 114, 38 106 C18 92, 16 58, 26 44 Z" fill="#E11D48" stroke="#090D16" stroke-width="7" stroke-linejoin="round"/>
  <path d="M38 58 L54 70 M54 58 L38 70" stroke="#FFE4E6" stroke-width="6" stroke-linecap="round"/>
  <path d="M74 58 L90 70 M90 58 L74 70" stroke="#FFE4E6" stroke-width="6" stroke-linecap="round"/>
  <path d="M44 92 L54 84 L64 92 L74 84 L84 92" fill="none" stroke="#090D16" stroke-width="6" stroke-linecap="round" stroke-linejoin="round"/>
  <path d="M84 36 L76 48 L84 54" fill="none" stroke="#090D16" stroke-width="4" stroke-linecap="round"/>
</svg>"##
    } else {
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 128 128">
  <path d="M26 44 L16 18 L44 32 L84 32 L112 18 L102 44 C112 58, 110 92, 90 106 C74 114, 54 114, 38 106 C18 92, 16 58, 26 44 Z" fill="#F43F5E" stroke="#090D16" stroke-width="7" stroke-linejoin="round"/>
  <path d="M34 52 L56 62 L38 70 Z" fill="#FEF08A" stroke="#090D16" stroke-width="4" stroke-linejoin="round"/>
  <path d="M94 52 L72 62 L90 70 Z" fill="#FEF08A" stroke="#090D16" stroke-width="4" stroke-linejoin="round"/>
  <path d="M42 86 Q64 96 86 86" fill="none" stroke="#090D16" stroke-width="6" stroke-linecap="round"/>
</svg>"##
    }
}

/// Calificaciones vectoriales SVG (`SICK!`, `GOOD!`, `BAD`, `MISS`) con trazado puro `<path>`
/// para garantizar renderizado perfecto vía `resvg` sin depender de fuentes del sistema.
pub fn default_rating_svg(kind: RatingKind) -> &'static str {
    match kind {
        RatingKind::Sick => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 280 96">
  <defs>
    <linearGradient id="gSick" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#67E8F9"/>
      <stop offset="55%" stop-color="#38BDF8"/>
      <stop offset="100%" stop-color="#FACC15"/>
    </linearGradient>
  </defs>
  <polygon points="18,14 22,24 32,28 22,32 18,42 14,32 4,28 14,24" fill="#FEF08A" stroke="#090D16" stroke-width="3"/>
  <polygon points="258,12 262,20 270,24 262,28 258,36 254,28 246,24 254,20" fill="#67E8F9" stroke="#090D16" stroke-width="3"/>
  <g stroke="#090D16" stroke-width="14" stroke-linejoin="round" stroke-linecap="round" fill="#090D16">
    <path d="M76 26 H46 L38 44 H68 L62 72 H32"/>
    <path d="M98 22 L92 74"/>
    <path d="M148 26 H120 L112 72 H142"/>
    <path d="M164 22 L158 74 M194 22 L164 48 L190 74"/>
    <path d="M220 20 L214 54 M212 68 L211 76"/>
  </g>
  <g stroke="url(#gSick)" stroke-width="7.5" stroke-linejoin="round" stroke-linecap="round" fill="none">
    <path d="M76 26 H46 L38 44 H68 L62 72 H32"/>
    <path d="M98 22 L92 74"/>
    <path d="M148 26 H120 L112 72 H142"/>
    <path d="M164 22 L158 74 M194 22 L164 48 L190 74"/>
    <path d="M220 20 L214 54 M212 68 L211 76"/>
  </g>
</svg>"##,
        RatingKind::Good => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 280 96">
  <defs>
    <linearGradient id="gGood" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#86EFAC"/>
      <stop offset="100%" stop-color="#10B981"/>
    </linearGradient>
  </defs>
  <g stroke="#090D16" stroke-width="14" stroke-linejoin="round" stroke-linecap="round" fill="none">
    <path d="M68 26 H38 L30 72 H62 L66 50 H52"/>
    <path d="M88 26 H116 L110 72 H82 Z"/>
    <path d="M138 26 H166 L160 72 H132 Z"/>
    <path d="M188 24 H210 L220 48 L206 72 H182 Z"/>
    <path d="M242 20 L236 54 M234 68 L233 76"/>
  </g>
  <g stroke="url(#gGood)" stroke-width="7.5" stroke-linejoin="round" stroke-linecap="round" fill="none">
    <path d="M68 26 H38 L30 72 H62 L66 50 H52"/>
    <path d="M88 26 H116 L110 72 H82 Z"/>
    <path d="M138 26 H166 L160 72 H132 Z"/>
    <path d="M188 24 H210 L220 48 L206 72 H182 Z"/>
    <path d="M242 20 L236 54 M234 68 L233 76"/>
  </g>
</svg>"##,
        RatingKind::Bad => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 280 96">
  <defs>
    <linearGradient id="gBad" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#FDE047"/>
      <stop offset="100%" stop-color="#F97316"/>
    </linearGradient>
  </defs>
  <g stroke="#090D16" stroke-width="14" stroke-linejoin="round" stroke-linecap="round" fill="none">
    <path d="M68 24 V74 H98 V52 H68 M68 24 H94 V48 H68"/>
    <path d="M118 74 L136 24 L154 74 M124 56 H148"/>
    <path d="M174 24 H198 L210 49 L198 74 H174 Z"/>
  </g>
  <g stroke="url(#gBad)" stroke-width="7.5" stroke-linejoin="round" stroke-linecap="round" fill="none">
    <path d="M68 24 V74 H98 V52 H68 M68 24 H94 V48 H68"/>
    <path d="M118 74 L136 24 L154 74 M124 56 H148"/>
    <path d="M174 24 H198 L210 49 L198 74 H174 Z"/>
  </g>
</svg>"##,
        RatingKind::Miss => r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 280 96">
  <defs>
    <linearGradient id="gMiss" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#FDA4AF"/>
      <stop offset="100%" stop-color="#DC2626"/>
    </linearGradient>
  </defs>
  <g stroke="#090D16" stroke-width="14" stroke-linejoin="round" stroke-linecap="round" fill="none">
    <path d="M32 74 L42 24 L60 54 L80 24 L86 74"/>
    <path d="M108 24 L104 74"/>
    <path d="M164 26 H134 L130 48 H158 L154 74 H124"/>
    <path d="M220 26 H190 L186 48 H214 L210 74 H180"/>
  </g>
  <g stroke="url(#gMiss)" stroke-width="7.5" stroke-linejoin="round" stroke-linecap="round" fill="none">
    <path d="M32 74 L42 24 L60 54 L80 24 L86 74"/>
    <path d="M108 24 L104 74"/>
    <path d="M164 26 H134 L130 48 H158 L154 74 H124"/>
    <path d="M220 26 H190 L186 48 H214 L210 74 H180"/>
  </g>
</svg>"##,
    }
}

/// Convierte cualquier flecha rasterizada (incluyendo `.svg` personalizados) a un receptor gris sin color.
pub fn desaturate_to_neutral_receptor(arrow: &RasterizedArrow) -> RasterizedArrow {
    let mut rgba = arrow.rgba.clone();
    for px in rgba.chunks_exact_mut(4) {
        if px[3] > 0 {
            let luma = (0.299 * (px[0] as f32) + 0.587 * (px[1] as f32) + 0.114 * (px[2] as f32))
                .round()
                .clamp(92.0, 212.0) as u8;
            px[0] = luma;
            px[1] = luma;
            px[2] = luma.saturating_add(6);
            px[3] = ((px[3] as f32) * 0.85).round().clamp(0.0, 255.0) as u8;
        }
    }
    RasterizedArrow {
        width: arrow.width,
        height: arrow.height,
        rgba,
    }
}

impl ArrowSkinSet {
    /// Rasteriza la versión sin color (receptor gris neutro) respetando SVGs personalizados.
    pub fn rasterize_neutral_receptor(
        &self,
        dir: NoteDirection,
    ) -> Result<RasterizedArrow, SvgSkinError> {
        if let Some(custom) = self.custom_svg_sources.get(&dir) {
            let colored = rasterize_svg_str(custom, self.note_size_px)?;
            Ok(desaturate_to_neutral_receptor(&colored))
        } else {
            rasterize_svg_str(default_receptor_svg_bundle(dir), self.note_size_px)
        }
    }
}

/// Rasteriza un string `.svg` cuadrado a RGBA8888 usando `resvg`.
pub fn rasterize_svg_str(svg_str: &str, target_px: u32) -> Result<RasterizedArrow, SvgSkinError> {
    let size = target_px.clamp(32, 512);
    rasterize_svg_rect(svg_str, size, size)
}

/// Rasteriza un string `.svg` rectangular a RGBA8888 usando `resvg` (ideal para barra de vida y ratings).
pub fn rasterize_svg_rect(
    svg_str: &str,
    target_w: u32,
    target_h: u32,
) -> Result<RasterizedArrow, SvgSkinError> {
    let w = target_w.clamp(24, 1024);
    let h = target_h.clamp(16, 1024);
    let opts = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(svg_str, &opts)
        .map_err(|e| SvgSkinError::SvgParse(e.to_string()))?;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)
        .ok_or(SvgSkinError::PixmapAlloc)?;

    let tree_size = tree.size();
    let sx = (w as f32) / tree_size.width().max(1.0);
    let sy = (h as f32) / tree_size.height().max(1.0);
    let transform = resvg::tiny_skia::Transform::from_scale(sx, sy);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // tiny-skia usa premultiplied RGBA; convertimos a RGBA recto para Bitmap / ImageBitmap.
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
        width: w,
        height: h,
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_receptor_is_uncolored_while_note_arrow_is_colored() {
        let skin = ArrowSkinSet::bundled_default();
        let receptor = skin.rasterize_neutral_receptor(NoteDirection::Down).unwrap();
        let note = rasterize_svg_str(&skin.svg_for(NoteDirection::Down), 128).unwrap();

        let rec_max_chroma = receptor
            .rgba
            .chunks_exact(4)
            .filter(|px| px[3] > 20)
            .map(|px| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]))
            .max()
            .unwrap_or(0);
        let note_max_chroma = note
            .rgba
            .chunks_exact(4)
            .filter(|px| px[3] > 20)
            .map(|px| px[0].max(px[1]).max(px[2]) - px[0].min(px[1]).min(px[2]))
            .max()
            .unwrap_or(0);

        assert!(rec_max_chroma <= 25);
        assert!(note_max_chroma >= 100);
    }
}
