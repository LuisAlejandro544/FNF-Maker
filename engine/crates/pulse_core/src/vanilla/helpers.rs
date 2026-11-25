//! Archivo: engine/crates/pulse_core/src/vanilla/helpers.rs
//! Propósito: Utilidades compartidas de streaming y manejo de archivos para Vanilla (`.mod`):
//! hasher incremental FNV-1a, guardas RAII temporales, sanitización de nombres y conversión VP8L.

use crate::astc_loader::load_sprite_image;
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder};
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Hasher incremental FNV-1a 64-bit equivalente a `fnv1a_64_bytes` para streaming sin cargar archivos enteros en RAM.
pub(super) struct Fnv1a64Hasher {
    state: u64,
}

impl Fnv1a64Hasher {
    pub(super) fn new() -> Self {
        Self {
            state: 0xcbf29ce484222325,
        }
    }

    pub(super) fn update(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.state ^= b as u64;
            self.state = self.state.wrapping_mul(0x100000001b3);
        }
    }

    pub(super) fn finish(self) -> u64 {
        self.state
    }
}

/// Limpiador RAII de directorio temporal para evitar residuos si falla una exportación o importación.
pub(super) struct TempDirGuard {
    path: PathBuf,
    armed: bool,
}

impl TempDirGuard {
    pub(super) fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    pub(super) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        if self.armed && self.path.exists() {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

/// Limpiador RAII para archivos temporales de exportación.
pub(super) struct TempFileGuard {
    path: PathBuf,
    armed: bool,
}

impl TempFileGuard {
    pub(super) fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    pub(super) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if self.armed && self.path.is_file() {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

pub(super) fn sanitize_slug(raw: &str, fallback: &str, max_len: usize) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.chars().take(max_len).collect()
    }
}

pub(super) fn allocate_unique_rel_path(
    used: &mut HashSet<String>,
    dir_prefix: &str,
    stem: &str,
    ext: &str,
) -> String {
    let clean_stem = sanitize_slug(stem, "asset", 40);
    let clean_ext = sanitize_slug(ext, "bin", 12).to_ascii_lowercase();
    let base = format!("{}/{}.{}", dir_prefix, clean_stem, clean_ext);
    if used.insert(base.clone()) {
        return base;
    }
    let mut idx = 2_u32;
    loop {
        let candidate = format!("{}/{}_{}.{}", dir_prefix, clean_stem, idx, clean_ext);
        if used.insert(candidate.clone()) {
            return candidate;
        }
        idx += 1;
    }
}

pub(super) fn canonical_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub(super) fn strip_to_file_name(path_opt: &mut Option<PathBuf>) {
    if let Some(p) = path_opt.take() {
        if let Some(name) = p.file_name() {
            *path_opt = Some(PathBuf::from(name));
        }
    }
}

fn is_png_or_bmp(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        if ext.eq_ignore_ascii_case("png") || ext.eq_ignore_ascii_case("bmp") {
            return true;
        }
    }
    if let Ok(mut f) = File::open(path) {
        let mut head = [0u8; 8];
        if f.read_exact(&mut head).is_ok() {
            return head == [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1A, b'\n']
                || &head[0..2] == b"BM";
        }
    }
    false
}

/// Convierte imágenes `.png`/`.bmp` a **WebP sin pérdida (VP8L)** si el resultado es realmente más pequeño.
pub(super) fn try_convert_to_smaller_lossless_webp(src_path: &Path, src_bytes: u64) -> Option<Vec<u8>> {
    if !is_png_or_bmp(src_path) {
        return None;
    }
    let img = load_sprite_image(src_path).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let mut webp_buf = Vec::with_capacity((src_bytes as usize).min(4 * 1024 * 1024));
    let encoder = WebPEncoder::new_lossless(&mut webp_buf);
    encoder
        .write_image(rgba.as_raw(), w, h, ExtendedColorType::Rgba8)
        .ok()?;

    if (webp_buf.len() as u64) + 64 < src_bytes {
        Some(webp_buf)
    } else {
        None
    }
}
