//! Archivo: engine/crates/pulse_core/src/vanilla/models.rs
//! Propósito: Define constantes de cabecera binaria, límites de seguridad 32-bit, modelos de
//! manifiesto/reporte Vanilla (`VanillaManifest`, `VanillaEntry`, etc.) y reglas de códec/rutas.

use crate::project::ModProject;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path};

/// Firma mágica de 4 bytes del contenedor Vanilla: `"VNLA"`.
pub const VANILLA_MAGIC: [u8; 4] = *b"VNLA";
/// Versión actual del formato binario Vanilla.
pub const VANILLA_FORMAT_VERSION: u16 = 1;
/// Tamaño fijo de la cabecera binaria Vanilla (bytes).
pub const VANILLA_HEADER_BYTES: usize = 24;
/// Extensión temporal acordada para pruebas de exportación/importación.
pub const VANILLA_FILE_EXTENSION: &str = "mod";
/// Flag de cabecera: el paquete aplicó conversión inteligente PNG -> WebP lossless.
pub const VANILLA_FLAG_OPTIMIZED_WEBP: u16 = 1 << 0;
/// Flag de cabecera: el paquete aplicó deduplicación de assets compartidos.
pub const VANILLA_FLAG_ASSET_DEDUP: u16 = 1 << 1;

/// Límites defensivos para evitar asignaciones excesivas en dispositivos 32-bit.
pub const MAX_VANILLA_MANIFEST_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_VANILLA_PROJECT_JSON_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_VANILLA_ENTRIES: usize = 4096;

/// Niveles Zstandard calibrados para máximo ahorro vs velocidad móvil.
pub const VANILLA_ZSTD_JSON_LEVEL: i32 = 12;
pub const VANILLA_ZSTD_DATA_LEVEL: i32 = 9;
pub const VANILLA_ZSTD_FALLBACK_LEVEL: i32 = 6;
pub(super) const STREAM_CHUNK_BYTES: usize = 64 * 1024;

/// Códec aplicado a cada entrada individual dentro de un paquete Vanilla.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VanillaEntryCodec {
    /// Almacenado sin re-comprimir (ideal para `.ogg`, `.mp3`, `.webp`, `.jpg`, `.png`).
    Raw,
    /// Comprimido con Zstandard (`zstd`), ideal para `project.json`, `.wav`, `.astc`, `.xml`, scripts y `.svg`.
    Zstd,
}

/// Entrada descrita en el índice del manifiesto Vanilla.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VanillaEntry {
    pub rel_path: String,
    pub kind: String,
    pub codec: VanillaEntryCodec,
    pub zstd_level: u8,
    pub offset: u64,
    pub source_bytes: u64,
    pub uncompressed_bytes: u64,
    pub compressed_bytes: u64,
    pub fnv1a64: u64,
}

/// Metadatos iniciales del contenedor Vanilla comprimidos justo después de la cabecera de 24 bytes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VanillaManifest {
    pub container_name: String,
    pub format_version: u16,
    pub extension_hint: String,
    pub project_name: String,
    pub author: String,
    pub chart_title: String,
    pub bpm: f64,
    pub total_notes: usize,
    pub track_count: usize,
    pub entity_count: usize,
    pub script_count: usize,
    pub created_at_ms: u64,
    pub optimized_webp: bool,
    pub deduplicated_assets: usize,
    pub png_converted_count: usize,
    pub total_source_bytes: u64,
    pub total_compressed_bytes: u64,
    pub entries: Vec<VanillaEntry>,
}

/// Resumen devuelto tras exportar un proyecto a `.mod` (Vanilla + Zstd).
#[derive(Debug, Clone)]
pub struct VanillaExportReport {
    pub manifest: VanillaManifest,
    pub archive_bytes: u64,
    pub source_bytes: u64,
    pub png_converted_count: usize,
    pub deduplicated_assets: usize,
    pub saved_bytes: u64,
    pub ratio_percent: f32,
}

/// Resultado de importar e instalar un `.mod` Vanilla en una carpeta de proyecto aislada.
#[derive(Debug, Clone)]
pub struct VanillaImportResult {
    pub manifest: VanillaManifest,
    pub project: ModProject,
    pub extracted_files: usize,
    pub decoded_astc_count: usize,
}

/// Decide la estrategia óptima de compresión para cada archivo dentro de Vanilla.
/// - JSON/XML/SVG/Scripts: Zstd nivel 12 (máximo ahorro en texto estructurado).
/// - Audio `.wav` y texturas `.astc`/`.bmp`: Zstd nivel 9 (muy alta reducción).
/// - Media ya entropía-codificada (`.ogg`, `.mp3`, `.webp`, `.jpg`, `.png`): Raw (sin gastar CPU ni inflar tamaño).
pub fn classify_vanilla_entry_codec(rel_path: &str) -> (VanillaEntryCodec, i32) {
    let ext = Path::new(rel_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "json" | "xml" | "svg" | "lua" | "ts" | "js" | "txt" | "md" => {
            (VanillaEntryCodec::Zstd, VANILLA_ZSTD_JSON_LEVEL)
        }
        "wav" | "astc" | "bmp" | "tga" | "raw" | "pcm" => {
            (VanillaEntryCodec::Zstd, VANILLA_ZSTD_DATA_LEVEL)
        }
        "ogg" | "oga" | "mp3" | "flac" | "m4a" | "aac" | "webp" | "jpg" | "jpeg" | "png" => {
            (VanillaEntryCodec::Raw, 0)
        }
        _ => (VanillaEntryCodec::Zstd, VANILLA_ZSTD_FALLBACK_LEVEL),
    }
}

/// Valida estrictamente una ruta relativa dentro de un archivo Vanilla `.mod` contra path traversal.
pub fn validate_vanilla_rel_path(rel_path: &str) -> Result<(), String> {
    if rel_path.trim().is_empty() {
        return Err("Ruta vacía dentro del contenedor Vanilla".to_string());
    }
    if rel_path.contains('\0')
        || rel_path.contains('\\')
        || rel_path.contains(':')
        || rel_path.starts_with('/')
    {
        return Err(format!("Ruta insegura en Vanilla: '{}'", rel_path));
    }
    let p = Path::new(rel_path);
    let mut comp_count = 0_usize;
    for comp in p.components() {
        match comp {
            Component::Normal(seg) => {
                let s = seg.to_string_lossy();
                if s.is_empty() || s == "." || s == ".." {
                    return Err(format!("Segmento inválido en ruta Vanilla: '{}'", rel_path));
                }
                comp_count += 1;
            }
            _ => {
                return Err(format!("Ruta relativa inválida en Vanilla: '{}'", rel_path));
            }
        }
    }
    if comp_count == 0 {
        return Err("Ruta sin componentes válidos en Vanilla".to_string());
    }
    Ok(())
}
