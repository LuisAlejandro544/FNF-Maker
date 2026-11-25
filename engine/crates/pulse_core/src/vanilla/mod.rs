//! Archivo: engine/crates/pulse_core/src/vanilla/mod.rs
//! Propósito: Punto de entrada modular del formato contenedor portátil **Vanilla** (`.mod`)
//! con compresión **Zstandard (`zstd`)**, conversión WebP VP8L sin pérdida, deduplicación de assets,
//! exportación/importación por streaming y verificación de integridad FNV-1a de 64 bits.

mod exporter;
mod helpers;
mod importer;
mod models;

#[cfg(test)]
mod tests;

pub use exporter::export_project_vanilla_mod;
pub use importer::{import_project_vanilla_mod, inspect_vanilla_mod};
pub use models::{
    MAX_VANILLA_ENTRIES, MAX_VANILLA_MANIFEST_BYTES, MAX_VANILLA_PROJECT_JSON_BYTES,
    VANILLA_FILE_EXTENSION, VANILLA_FLAG_ASSET_DEDUP, VANILLA_FLAG_OPTIMIZED_WEBP,
    VANILLA_FORMAT_VERSION, VANILLA_HEADER_BYTES, VANILLA_MAGIC, VANILLA_ZSTD_DATA_LEVEL,
    VANILLA_ZSTD_FALLBACK_LEVEL, VANILLA_ZSTD_JSON_LEVEL, VanillaEntry, VanillaEntryCodec,
    VanillaExportReport, VanillaImportResult, VanillaManifest, classify_vanilla_entry_codec,
    validate_vanilla_rel_path,
};
