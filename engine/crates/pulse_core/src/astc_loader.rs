//! Archivo: engine/crates/pulse_core/src/astc_loader.rs
//! Propósito: Decodificador puro en Rust para texturas comprimidas `.astc` (Adaptive Scalable
//! Texture Compression) mediante `texture2ddecoder` (licencia MIT / Apache-2.0) y cargador
//! unificado de imágenes de sprites (`.png`, `.jpg`, `.webp` y `.astc`).
//! Permite importar mods móviles que traen spritesheets optimizados en `.astc` (ej. `spritemap1.astc`)
//! sin depender de hardware GPU específico ni librerías C++.

use image::{DynamicImage, RgbaImage};
use std::path::Path;

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
}
