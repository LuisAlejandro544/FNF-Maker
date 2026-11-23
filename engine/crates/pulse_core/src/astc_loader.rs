//! Archivo: engine/crates/pulse_core/src/astc_loader.rs
//! Propósito: Decodificador puro en Rust para texturas comprimidas `.astc` (Adaptive Scalable
//! Texture Compression) mediante `texture2ddecoder` (licencia MIT / Apache-2.0) y cargador
//! unificado de imágenes de sprites (`.png`, `.jpg`, `.webp` y `.astc`).
//! Permite importar mods móviles que traen spritesheets optimizados en `.astc` (ej. `spritemap1.astc`)
//! sin depender de hardware GPU específico ni librerías C++.

use image::codecs::webp::WebPEncoder;
use image::{DynamicImage, ExtendedColorType, ImageEncoder, RgbImage, RgbaImage};
use std::collections::HashSet;
use std::io::BufWriter;
use std::path::Path;

/// Política de formato y cuantización elegida por el compilador en caliente al guardar WebP VP8L.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebpPixelPolicy {
    /// Conserva RGBA8 íntegro sin pérdida.
    LosslessRgba,
    /// Si la imagen es 100 % opaca, aplica dithering Floyd-Steinberg hacia rejilla RGB565 y guarda RGB8
    /// para que Android decodifique en `Bitmap.Config.RGB_565` (50 % menos RAM/VRAM y sin banding).
    Rgb565DitheredIfOpaque,
    /// Cuantiza perceptualmente con `exoquant` hasta `max_colors` usando dithering Floyd-Steinberg
    /// (útil en gama baja / sprites grandes para acelerar decodificación y reducir peso en disco).
    QuantizedRgba { max_colors: usize },
}

/// Resultado real aplicado al codificar una textura compilada en WebP.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WebpSaveInfo {
    pub stored_opaque_rgb: bool,
    pub dithered_rgb565: bool,
    pub palette_quantized: bool,
}

/// Firma mágica de los primeros 4 bytes de cualquier archivo estándar `.astc`.
pub const ASTC_MAGIC: [u8; 4] = [0x13, 0xAB, 0xA1, 0x5C];

/// Metadatos extraídos de la cabecera de 16 bytes de un archivo `.astc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AstcHeader {
    pub block_x: usize,
    pub block_y: usize,
    pub block_z: usize,
    pub width: usize,
    pub height: usize,
    pub depth: usize,
}

impl AstcHeader {
    /// Lee y valida la cabecera de 16 bytes de un contenedor `.astc`.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 16 {
            return Err("Archivo .astc demasiado corto (< 16 bytes)".to_string());
        }
        if bytes[0..4] != ASTC_MAGIC {
            return Err("Cabecera mágica .astc inválida (se esperaba 13 AB A1 5C)".to_string());
        }

        let block_x = bytes[4] as usize;
        let block_y = bytes[5] as usize;
        let block_z = bytes[6] as usize;
        if !(1..=12).contains(&block_x) || !(1..=12).contains(&block_y) || block_z != 1 {
            return Err(format!(
                "Tamaño de bloque ASTC no soportado: {}x{}x{}",
                block_x, block_y, block_z
            ));
        }

        let width = u32::from_le_bytes([bytes[7], bytes[8], bytes[9], 0]) as usize;
        let height = u32::from_le_bytes([bytes[10], bytes[11], bytes[12], 0]) as usize;
        let depth = u32::from_le_bytes([bytes[13], bytes[14], bytes[15], 0]) as usize;

        if width == 0 || height == 0 || depth != 1 {
            return Err(format!(
                "Dimensiones .astc inválidas: {}x{}x{}",
                width, height, depth
            ));
        }
        if width > 8192 || height > 8192 {
            return Err(format!(
                "Textura .astc demasiado grande ({}x{}, máximo 8192px)",
                width, height
            ));
        }

        Ok(Self {
            block_x,
            block_y,
            block_z,
            width,
            height,
            depth,
        })
    }

    pub fn expected_payload_bytes(self) -> usize {
        let blocks_x = self.width.div_ceil(self.block_x);
        let blocks_y = self.height.div_ceil(self.block_y);
        blocks_x * blocks_y * 16
    }
}

/// Comprueba por firma mágica o extensión si los bytes o ruta corresponden a una textura `.astc`.
pub fn is_astc_path(path: &Path) -> bool {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("astc"))
        .unwrap_or(false)
    {
        return true;
    }
    if let Ok(mut f) = std::fs::File::open(path) {
        use std::io::Read;
        let mut magic = [0u8; 4];
        if f.read_exact(&mut magic).is_ok() {
            return magic == ASTC_MAGIC;
        }
    }
    false
}

/// Comprueba por extensión o cabecera RIFF/WEBP si la ruta ya es un archivo `.webp`.
pub fn is_webp_path(path: &Path) -> bool {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("webp"))
        .unwrap_or(false)
    {
        return true;
    }
    if let Ok(mut f) = std::fs::File::open(path) {
        use std::io::Read;
        let mut head = [0u8; 12];
        if f.read_exact(&mut head).is_ok() {
            return &head[0..4] == b"RIFF" && &head[8..12] == b"WEBP";
        }
    }
    false
}

/// Comprueba si todos los píxeles de la imagen tienen canal alfa opaco (`alpha >= 254`).
/// Permite codificar fondos opacos en `Rgb8` sin plano alfa inútil y ahorrar tamaño/decodificación.
pub fn is_image_opaque(img: &DynamicImage) -> bool {
    match img {
        DynamicImage::ImageRgb8(_) | DynamicImage::ImageLuma8(_) => true,
        _ => {
            let rgba = img.to_rgba8();
            rgba.as_raw().chunks_exact(4).all(|px| px[3] >= 254)
        }
    }
}

#[inline]
fn quantize_5bit(v: i32) -> u8 {
    let clamped = v.clamp(0, 255);
    let q = (clamped * 31 + 127) / 255;
    ((q << 3) | (q >> 2)) as u8
}

#[inline]
fn quantize_6bit(v: i32) -> u8 {
    let clamped = v.clamp(0, 255);
    let q = (clamped * 63 + 127) / 255;
    ((q << 2) | (q >> 4)) as u8
}

/// Aplica dithering Floyd-Steinberg hacia la rejilla exacta de 16 bits `RGB_565`
/// evitando bandas visibles (*color banding*) en degradados, cielos y luces antes de
/// almacenar el `.webp` que Android decodificará en `Bitmap.Config.RGB_565`.
pub fn dither_image_to_rgb565_grid(img: &DynamicImage) -> RgbImage {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let w_usize = w as usize;
    let h_usize = h as usize;
    if w_usize == 0 || h_usize == 0 {
        return rgb;
    }

    let raw = rgb.into_raw();
    // Acumuladores en punto fijo x16 para evitar asignaciones/flotantes dentro del bucle por píxel.
    let mut err_curr = vec![[0_i32; 3]; w_usize + 2];
    let mut err_next = vec![[0_i32; 3]; w_usize + 2];
    let mut out = vec![0_u8; w_usize * h_usize * 3];

    for y in 0..h_usize {
        err_next.fill([0, 0, 0]);
        for x in 0..w_usize {
            let idx = (y * w_usize + x) * 3;
            let ec = err_curr[x + 1];
            let r_in = (raw[idx] as i32) + (ec[0] / 16);
            let g_in = (raw[idx + 1] as i32) + (ec[1] / 16);
            let b_in = (raw[idx + 2] as i32) + (ec[2] / 16);

            let r_q = quantize_5bit(r_in);
            let g_q = quantize_6bit(g_in);
            let b_q = quantize_5bit(b_in);

            out[idx] = r_q;
            out[idx + 1] = g_q;
            out[idx + 2] = b_q;

            let er = r_in.clamp(0, 255) - (r_q as i32);
            let eg = g_in.clamp(0, 255) - (g_q as i32);
            let eb = b_in.clamp(0, 255) - (b_q as i32);

            // Difusión clásica Floyd-Steinberg: derecha 7/16, abajo-izq 3/16, abajo 5/16, abajo-der 1/16
            err_curr[x + 2][0] += er * 7;
            err_curr[x + 2][1] += eg * 7;
            err_curr[x + 2][2] += eb * 7;

            err_next[x][0] += er * 3;
            err_next[x][1] += eg * 3;
            err_next[x][2] += eb * 3;

            err_next[x + 1][0] += er * 5;
            err_next[x + 1][1] += eg * 5;
            err_next[x + 1][2] += eb * 5;

            err_next[x + 2][0] += er;
            err_next[x + 2][1] += eg;
            err_next[x + 2][2] += eb;
        }
        std::mem::swap(&mut err_curr, &mut err_next);
    }

    RgbImage::from_raw(w, h, out).unwrap_or_else(|| img.to_rgb8())
}

/// Cuantiza perceptualmente una imagen `RgbaImage` a `<= max_colors` usando `exoquant` (MIT)
/// con optimización K-Means y dithering Floyd-Steinberg. Devuelve `None` si la imagen ya tiene
/// menos colores únicos que `max_colors` (por ejemplo pixel-art), conservándola intacta 1:1.
pub fn quantize_rgba_with_exoquant(rgba: &RgbaImage, max_colors: usize) -> Option<RgbaImage> {
    use exoquant::optimizer::Optimizer;
    use exoquant::{Color, Histogram, Quantizer, Remapper, SimpleColorSpace, ditherer, optimizer};

    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 || max_colors < 16 {
        return None;
    }

    let raw = rgba.as_raw();
    let pixel_count = (w as usize) * (h as usize);
    let target_colors = max_colors.clamp(16, 256);

    // Muestreo rápido de colores únicos para no degradar sprites/pixel-art que ya tienen pocos colores
    let stride = (pixel_count / 32_768).max(1);
    let mut seen: HashSet<u32> = HashSet::with_capacity(target_colors + 32);
    for chunk in raw.chunks_exact(4).step_by(stride) {
        let packed = if chunk[3] == 0 {
            0
        } else {
            u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])
        };
        seen.insert(packed);
        if seen.len() > target_colors {
            break;
        }
    }
    if seen.len() <= target_colors {
        return None;
    }

    let mut pixels: Vec<Color> = Vec::with_capacity(pixel_count);
    for px in raw.chunks_exact(4) {
        if px[3] == 0 {
            pixels.push(Color {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            });
        } else {
            pixels.push(Color {
                r: px[0],
                g: px[1],
                b: px[2],
                a: px[3],
            });
        }
    }

    // Si la textura es muy grande (>1.2M px), muestreamos el histograma 2x2 para rapidez en móvil
    let hist_stride = if pixel_count > 1_200_000 { 2 } else { 1 };
    let histogram: Histogram = pixels.iter().step_by(hist_stride).cloned().collect();
    let colorspace = SimpleColorSpace::default();
    let mut quantizer = Quantizer::new(&histogram, &colorspace);
    while quantizer.num_colors() < target_colors {
        let prev = quantizer.num_colors();
        quantizer.step();
        if quantizer.num_colors() == prev {
            break;
        }
    }
    let palette = quantizer.colors(&colorspace);
    let palette = optimizer::KMeans.optimize_palette(&colorspace, &palette, &histogram, 4);
    let dither = ditherer::FloydSteinberg::new();
    let remapper = Remapper::new(&palette, &colorspace, &dither);
    let indexed = remapper.remap(&pixels, w as usize);

    let mut out = Vec::with_capacity(pixel_count * 4);
    for idx in indexed {
        let c = palette.get(idx as usize).copied().unwrap_or(Color {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        });
        if c.a == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            out.extend_from_slice(&[c.r, c.g, c.b, c.a]);
        }
    }

    RgbaImage::from_raw(w, h, out)
}

/// Codifica cualquier `DynamicImage` a `.webp` sin pérdida (VP8L con predictores, transformaciones
/// de color y compresión máxima sin pérdida de calidad ni alpha) de forma atómica en disco.
pub fn save_lossless_webp(img: &DynamicImage, path: &Path) -> Result<(), String> {
    save_optimized_webp(img, path, false).map(|_| ())
}

/// Codifica `DynamicImage` a `.webp` VP8L sin pixelación; si `strip_unused_alpha` es `true` y la
/// imagen es totalmente opaca, omite el canal alfa (`Rgb8`) reduciendo el peso del archivo.
/// Devuelve `true` si se almacenó como `Rgb8` opaco.
pub fn save_optimized_webp(
    img: &DynamicImage,
    path: &Path,
    strip_unused_alpha: bool,
) -> Result<bool, String> {
    let policy = if strip_unused_alpha {
        WebpPixelPolicy::Rgb565DitheredIfOpaque
    } else {
        WebpPixelPolicy::LosslessRgba
    };
    save_compiled_webp(img, path, policy).map(|info| info.stored_opaque_rgb)
}

/// Guarda un sprite compilado a `.webp` VP8L aplicando de manera atómica la política óptima
/// (`RGB_565` con dithering Floyd-Steinberg en fondos opacos o cuantización `exoquant` inteligente).
pub fn save_compiled_webp(
    img: &DynamicImage,
    path: &Path,
    policy: WebpPixelPolicy,
) -> Result<WebpSaveInfo, String> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp_path = path.with_extension("webp.tmp");
    let mut info = WebpSaveInfo::default();

    {
        let file = std::fs::File::create(&tmp_path).map_err(|e| {
            format!(
                "No se pudo crear archivo WebP temporal '{}': {}",
                tmp_path.display(),
                e
            )
        })?;
        let writer = BufWriter::new(file);
        let encoder = WebPEncoder::new_lossless(writer);

        let encode_res = match policy {
            WebpPixelPolicy::Rgb565DitheredIfOpaque if is_image_opaque(img) => {
                let dithered = dither_image_to_rgb565_grid(img);
                let (w, h) = dithered.dimensions();
                info.stored_opaque_rgb = true;
                info.dithered_rgb565 = true;
                encoder.write_image(dithered.as_raw(), w, h, ExtendedColorType::Rgb8)
            }
            WebpPixelPolicy::QuantizedRgba { max_colors } => {
                let rgba = img.to_rgba8();
                if let Some(quantized) = quantize_rgba_with_exoquant(&rgba, max_colors) {
                    let (w, h) = quantized.dimensions();
                    info.palette_quantized = true;
                    encoder.write_image(quantized.as_raw(), w, h, ExtendedColorType::Rgba8)
                } else {
                    let (w, h) = rgba.dimensions();
                    encoder.write_image(rgba.as_raw(), w, h, ExtendedColorType::Rgba8)
                }
            }
            _ => {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                encoder.write_image(rgba.as_raw(), w, h, ExtendedColorType::Rgba8)
            }
        };

        if let Err(e) = encode_res {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(format!("Error codificando WebP sin pérdida: {}", e));
        }
    }

    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(format!(
            "No se pudo mover WebP compilado a '{}': {}",
            path.display(),
            e
        ));
    }
    Ok(info)
}

/// Decodifica bytes completos de un archivo `.astc` (cabecera 16 bytes + bloques) a `DynamicImage` RGBA8.
pub fn decode_astc_file_bytes(bytes: &[u8]) -> Result<DynamicImage, String> {
    let header = AstcHeader::parse(bytes)?;
    let expected = header.expected_payload_bytes();
    let payload = &bytes[16..];
    if payload.len() < expected {
        return Err(format!(
            "Datos .astc incompletos: se esperaban {} bytes y hay {}",
            expected,
            payload.len()
        ));
    }

    let pixel_count = header.width * header.height;
    let mut bgra_u32 = vec![0_u32; pixel_count];
    texture2ddecoder::decode_astc(
        &payload[..expected],
        header.width,
        header.height,
        header.block_x,
        header.block_y,
        &mut bgra_u32,
    )
    .map_err(|e| format!("Error decodificando ASTC {}x{}: {}", header.block_x, header.block_y, e))?;

    // `texture2ddecoder` empaqueta cada pixel como `u32::from_le_bytes([b, g, r, a])`.
    let mut rgba = Vec::with_capacity(pixel_count * 4);
    for px in bgra_u32 {
        let [b, g, r, a] = px.to_le_bytes();
        rgba.push(r);
        rgba.push(g);
        rgba.push(b);
        rgba.push(a);
    }

    let img = RgbaImage::from_raw(header.width as u32, header.height as u32, rgba)
        .ok_or_else(|| "No se pudo construir buffer RGBA8 desde ASTC".to_string())?;
    Ok(DynamicImage::ImageRgba8(img))
}

/// Carga cualquier imagen de sprite soportada: `.astc`, `.png`, `.jpg`, `.webp`.
pub fn load_sprite_image(path: &Path) -> Result<DynamicImage, String> {
    let bytes = std::fs::read(path)
        .map_err(|e| format!("No se pudo leer la textura '{}': {}", path.display(), e))?;

    if bytes.starts_with(&ASTC_MAGIC) || is_astc_path(path) {
        return decode_astc_file_bytes(&bytes);
    }

    image::load_from_memory(&bytes)
        .map_err(|e| format!("Formato de imagen inválido '{}': {}", path.display(), e))
}

/// Obtiene dimensiones `(width, height)` sin decodificar toda la imagen cuando es posible.
pub fn read_sprite_dimensions(path: &Path) -> Option<(u32, u32)> {
    if is_astc_path(path) {
        use std::io::Read;
        let mut f = std::fs::File::open(path).ok()?;
        let mut head = [0u8; 16];
        f.read_exact(&mut head).ok()?;
        let h = AstcHeader::parse(&head).ok()?;
        return Some((h.width as u32, h.height as u32));
    }
    image::image_dimensions(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_astc_header_and_decodes_void_extent_block() {
        // Bloque ASTC 4x4 de extensión constante (void-extent RGBA blanco opaco)
        let mut bytes = vec![
            0x13, 0xAB, 0xA1, 0x5C, // magic
            4, 4, 1,                // 4x4x1 block
            4, 0, 0,                // width = 4
            4, 0, 0,                // height = 4
            1, 0, 0,                // depth = 1
        ];
        // Void-extent block (16 bytes): [0xFC, 0xFD, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, R_lo, R_hi, G_lo, G_hi, B_lo, B_hi, A_lo, A_hi]
        let block = [
            0xFC, 0xFD, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0x00, 0xFF, // R = 255
            0x00, 0x80, // G ~= 128
            0x00, 0x00, // B = 0
            0x00, 0xFF, // A = 255
        ];
        bytes.extend_from_slice(&block);

        let header = AstcHeader::parse(&bytes).expect("cabecera válida");
        assert_eq!(header.width, 4);
        assert_eq!(header.height, 4);
        assert_eq!(header.block_x, 4);
        assert_eq!(header.block_y, 4);

        let img = decode_astc_file_bytes(&bytes).expect("decodificación exitosa");
        assert_eq!(img.width(), 4);
        assert_eq!(img.height(), 4);
        let rgba = img.to_rgba8();
        let px = rgba.get_pixel(0, 0);
        assert_eq!(px[0], 255);
        assert_eq!(px[2], 0);
        assert_eq!(px[3], 255);
    }

    #[test]
    fn lossless_webp_roundtrip_preserves_exact_rgba_pixels() {
        let mut src = RgbaImage::new(4, 4);
        src.put_pixel(0, 0, image::Rgba([255, 16, 128, 255]));
        src.put_pixel(1, 0, image::Rgba([0, 224, 240, 120]));
        src.put_pixel(2, 1, image::Rgba([42, 99, 17, 0]));
        src.put_pixel(3, 3, image::Rgba([250, 250, 250, 255]));

        let dyn_img = DynamicImage::ImageRgba8(src.clone());
        let tmp_dir = std::env::temp_dir().join("beatforge_astc_webp_test");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let out_webp = tmp_dir.join("sample_sprite.webp");

        save_lossless_webp(&dyn_img, &out_webp).expect("guardado WebP sin pérdida");
        assert!(is_webp_path(&out_webp));

        let loaded = load_sprite_image(&out_webp).expect("lectura WebP");
        let loaded_rgba = loaded.to_rgba8();
        assert_eq!(loaded_rgba.dimensions(), (4, 4));
        assert_eq!(loaded_rgba.get_pixel(0, 0), src.get_pixel(0, 0));
        assert_eq!(loaded_rgba.get_pixel(1, 0), src.get_pixel(1, 0));
        assert_eq!(loaded_rgba.get_pixel(3, 3), src.get_pixel(3, 3));

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
