//! Archivo: engine/crates/pulse_core/src/vanilla.rs
//! Propósito: Implementación 100% Rust del formato contenedor portátil **Vanilla** (extensión
//! temporal `.mod`) apoyado en **Zstandard (`zstd`)**, optimización automática de imágenes `.png`
//! a **WebP sin pérdida (VP8L)** cuando reduce peso, deduplicación de assets repetidos,
//! compresión por streaming de bajo consumo RAM (apto para 32 y 64 bits) y verificación de
//! integridad FNV-1a de 64 bits contra corrupción y path traversal.

use crate::astc_loader::{is_astc_path, load_sprite_image, save_lossless_webp};
use crate::hot_compile::fnv1a_64_bytes;
use crate::project::{MAX_AUDIO_TRACKS, ModProject, current_epoch_ms};
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

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
const STREAM_CHUNK_BYTES: usize = 64 * 1024;

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

/// Hasher incremental FNV-1a 64-bit equivalente a `fnv1a_64_bytes` para streaming sin cargar archivos enteros en RAM.
struct Fnv1a64Hasher {
    state: u64,
}

impl Fnv1a64Hasher {
    fn new() -> Self {
        Self {
            state: 0xcbf29ce484222325,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.state ^= b as u64;
            self.state = self.state.wrapping_mul(0x100000001b3);
        }
    }

    fn finish(self) -> u64 {
        self.state
    }
}

/// Limpiador RAII de directorio temporal para evitar residuos si falla una exportación o importación.
struct TempDirGuard {
    path: PathBuf,
    armed: bool,
}

impl TempDirGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
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
struct TempFileGuard {
    path: PathBuf,
    armed: bool,
}

impl TempFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
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

fn sanitize_slug(raw: &str, fallback: &str, max_len: usize) -> String {
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

fn allocate_unique_rel_path(
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

fn canonical_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn strip_to_file_name(path_opt: &mut Option<PathBuf>) {
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
fn try_convert_to_smaller_lossless_webp(src_path: &Path, src_bytes: u64) -> Option<Vec<u8>> {
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

enum PayloadSource {
    File(PathBuf),
    Bytes(Vec<u8>),
}

struct PlannedAsset {
    rel_path: String,
    kind: String,
    source_bytes: u64,
    payload: PayloadSource,
}

/// Copia una fuente hacia el escritor secuencial aplicando `Raw` o `Zstd` y calculando su hash FNV-1a.
fn write_entry_to_blob_stream(
    blob_writer: &mut BufWriter<File>,
    blob_offset: &mut u64,
    rel_path: &str,
    kind: &str,
    source_bytes: u64,
    payload: PayloadSource,
) -> Result<VanillaEntry, String> {
    let (codec, level) = classify_vanilla_entry_codec(rel_path);
    let start_offset = *blob_offset;
    let mut hasher = Fnv1a64Hasher::new();
    let mut uncompressed_bytes = 0_u64;
    let mut buf = vec![0u8; STREAM_CHUNK_BYTES];

    let start_pos = blob_writer
        .stream_position()
        .map_err(|e| format!("Error consultando posición del stream Vanilla: {}", e))?;

    match codec {
        VanillaEntryCodec::Raw => {
            match payload {
                PayloadSource::Bytes(bytes) => {
                    hasher.update(&bytes);
                    uncompressed_bytes = bytes.len() as u64;
                    blob_writer
                        .write_all(&bytes)
                        .map_err(|e| format!("Error escribiendo '{}' en Vanilla: {}", rel_path, e))?;
                }
                PayloadSource::File(path) => {
                    let f = File::open(&path).map_err(|e| {
                        format!("No se pudo abrir asset '{}': {}", path.display(), e)
                    })?;
                    let mut reader = BufReader::new(f);
                    loop {
                        let n = reader
                            .read(&mut buf)
                            .map_err(|e| format!("Error leyendo '{}': {}", rel_path, e))?;
                        if n == 0 {
                            break;
                        }
                        hasher.update(&buf[..n]);
                        uncompressed_bytes += n as u64;
                        blob_writer
                            .write_all(&buf[..n])
                            .map_err(|e| format!("Error escribiendo '{}': {}", rel_path, e))?;
                    }
                }
            }
            blob_writer
                .flush()
                .map_err(|e| format!("Error vaciando buffer de '{}': {}", rel_path, e))?;
        }
        VanillaEntryCodec::Zstd => {
            let mut encoder = zstd::stream::Encoder::new(&mut *blob_writer, level)
                .map_err(|e| format!("Error iniciando Zstd para '{}': {}", rel_path, e))?;
            match payload {
                PayloadSource::Bytes(bytes) => {
                    hasher.update(&bytes);
                    uncompressed_bytes = bytes.len() as u64;
                    encoder
                        .write_all(&bytes)
                        .map_err(|e| format!("Error comprimiendo '{}' con Zstd: {}", rel_path, e))?;
                }
                PayloadSource::File(path) => {
                    let f = File::open(&path).map_err(|e| {
                        format!("No se pudo abrir asset '{}': {}", path.display(), e)
                    })?;
                    let mut reader = BufReader::new(f);
                    loop {
                        let n = reader
                            .read(&mut buf)
                            .map_err(|e| format!("Error leyendo '{}': {}", rel_path, e))?;
                        if n == 0 {
                            break;
                        }
                        hasher.update(&buf[..n]);
                        uncompressed_bytes += n as u64;
                        encoder
                            .write_all(&buf[..n])
                            .map_err(|e| format!("Error comprimiendo '{}': {}", rel_path, e))?;
                    }
                }
            }
            let returned_writer = encoder
                .finish()
                .map_err(|e| format!("Error cerrando frame Zstd de '{}': {}", rel_path, e))?;
            returned_writer
                .flush()
                .map_err(|e| format!("Error vaciando stream Zstd de '{}': {}", rel_path, e))?;
        }
    }

    let end_pos = blob_writer
        .stream_position()
        .map_err(|e| format!("Error midiendo bytes escritos de '{}': {}", rel_path, e))?;
    let compressed_bytes = end_pos.saturating_sub(start_pos);
    *blob_offset = end_pos;

    Ok(VanillaEntry {
        rel_path: rel_path.to_string(),
        kind: kind.to_string(),
        codec,
        zstd_level: if codec == VanillaEntryCodec::Zstd {
            level.clamp(0, 22) as u8
        } else {
            0
        },
        offset: start_offset,
        source_bytes,
        uncompressed_bytes,
        compressed_bytes,
        fnv1a64: hasher.finish(),
    })
}

/// Empaqueta y comprime un `ModProject` completo en el contenedor **Vanilla** (`.mod`) usando **Zstd**.
/// Realiza escritura por streaming hacia un archivo temporal antes de renombrarlo atómicamente.
pub fn export_project_vanilla_mod(
    project: &ModProject,
    out_path: &Path,
    optimize_png_to_webp: bool,
) -> Result<VanillaExportReport, String> {
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("No se pudo preparar directorio de exportación: {}", e))?;
    }

    let mut cloned = project.clone();
    cloned.id = "vanilla_export".to_string();
    strip_to_file_name(&mut cloned.chart_source_path);
    strip_to_file_name(&mut cloned.health_bar_svg_path);
    for ent in &mut cloned.stage.entities {
        strip_to_file_name(&mut ent.xml_path);
        strip_to_file_name(&mut ent.animate_json_path);
    }

    let mut used_rel_paths: HashSet<String> = HashSet::new();
    used_rel_paths.insert("project.json".to_string());
    // Mapa de deduplicación: ruta canónica -> (rel_path, source_bytes)
    let mut dedup_map: HashMap<PathBuf, String> = HashMap::new();
    let mut planned_assets: Vec<PlannedAsset> = Vec::new();
    let mut png_converted_count = 0_usize;
    let mut deduplicated_assets = 0_usize;

    let mut stage_binary_asset = |src_path: &Path,
                                  dir_prefix: &str,
                                  stem_hint: &str,
                                  kind: &str,
                                  allow_webp_opt: bool|
     -> Result<String, String> {
        let key = canonical_key(src_path);
        if let Some(existing_rel) = dedup_map.get(&key) {
            deduplicated_assets += 1;
            return Ok(existing_rel.clone());
        }

        let meta = std::fs::metadata(src_path)
            .map_err(|e| format!("No se pudo leer '{}': {}", src_path.display(), e))?;
        let orig_bytes = meta.len();
        let orig_ext = src_path
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| !e.trim().is_empty())
            .unwrap_or("bin");

        let (final_ext, payload) = if allow_webp_opt && optimize_png_to_webp {
            if let Some(webp_bytes) = try_convert_to_smaller_lossless_webp(src_path, orig_bytes) {
                png_converted_count += 1;
                ("webp".to_string(), PayloadSource::Bytes(webp_bytes))
            } else {
                (orig_ext.to_string(), PayloadSource::File(src_path.to_path_buf()))
            }
        } else {
            (orig_ext.to_string(), PayloadSource::File(src_path.to_path_buf()))
        };

        let rel_path = allocate_unique_rel_path(&mut used_rel_paths, dir_prefix, stem_hint, &final_ext);
        dedup_map.insert(key, rel_path.clone());
        planned_assets.push(PlannedAsset {
            rel_path: rel_path.clone(),
            kind: kind.to_string(),
            source_bytes: orig_bytes,
            payload,
        });
        Ok(rel_path)
    };

    // 1. Pistas de audio (1..3)
    for slot in 0..MAX_AUDIO_TRACKS {
        if let Some(src_path) = cloned.audio_tracks[slot].file_path.clone() {
            if src_path.is_file() {
                let stem = src_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("audio");
                let rel = stage_binary_asset(
                    &src_path,
                    "assets/audio",
                    &format!("track{}_{}", slot, stem),
                    "audio",
                    false,
                )?;
                cloned.audio_tracks[slot].file_path = Some(PathBuf::from(rel));
            } else {
                cloned.audio_tracks[slot].file_path = None;
            }
        }
    }

    // 2. Sprites, texturas `.astc` / Animate e íconos de escenario
    for ent in &mut cloned.stage.entities {
        let ent_stem = sanitize_slug(&ent.id, "entity", 32);

        // Si el origen es `.astc` puro (sin rig Animate), evitamos guardar también el `.webp` decodificado
        // porque `.astc + Zstd` es compacto y al importar reconstruimos `decoded_textures/<id>.webp`.
        let skip_redundant_astc_decoded_webp = ent.animate_doc.is_none()
            && ent
                .raw_image_path
                .as_ref()
                .is_some_and(|p| p.is_file() && is_astc_path(p));

        if let Some(raw_p) = ent.raw_image_path.clone() {
            if raw_p.is_file() {
                let is_astc = is_astc_path(&raw_p);
                let rel = stage_binary_asset(
                    &raw_p,
                    "assets/sprites",
                    &ent_stem,
                    "sprite",
                    !is_astc,
                )?;
                ent.raw_image_path = Some(PathBuf::from(rel));
            } else {
                ent.raw_image_path = None;
            }
        }

        if skip_redundant_astc_decoded_webp {
            ent.image_path = None;
        } else if let Some(img_p) = ent.image_path.clone() {
            if img_p.is_file() {
                let dir_prefix = if ent.animate_doc.is_some()
                    && img_p
                        .to_string_lossy()
                        .contains("animate_baked")
                {
                    "textures/animate_baked"
                } else {
                    "assets/sprites"
                };
                let is_astc = is_astc_path(&img_p);
                let rel = stage_binary_asset(
                    &img_p,
                    dir_prefix,
                    &format!("{}_sheet", ent_stem),
                    "sprite",
                    !is_astc,
                )?;
                ent.image_path = Some(PathBuf::from(rel));
            } else {
                ent.image_path = None;
            }
        }

        if let Some(icon_n) = ent.icon_normal_path.clone() {
            if icon_n.is_file() {
                let rel = stage_binary_asset(
                    &icon_n,
                    "assets/icons",
                    &format!("{}_normal", ent_stem),
                    "icon",
                    true,
                )?;
                ent.icon_normal_path = Some(PathBuf::from(rel));
            } else {
                ent.icon_normal_path = None;
            }
        }

        if let Some(icon_l) = ent.icon_losing_path.clone() {
            if icon_l.is_file() {
                let rel = stage_binary_asset(
                    &icon_l,
                    "assets/icons",
                    &format!("{}_losing", ent_stem),
                    "icon",
                    true,
                )?;
                ent.icon_losing_path = Some(PathBuf::from(rel));
            } else {
                ent.icon_losing_path = None;
            }
        }
    }

    // 3. Serializar `project.json` compacto al principio del stream de blobs
    let project_json_bytes = serde_json::to_vec(&cloned)
        .map_err(|e| format!("Error serializando proyecto para Vanilla: {}", e))?;
    let project_json_len = project_json_bytes.len() as u64;

    let blobs_tmp_path = out_path.with_extension("mod.blobs.tmp");
    let out_tmp_path = out_path.with_extension("mod.tmp");
    let blobs_guard = TempFileGuard::new(blobs_tmp_path.clone());
    let mut out_guard = TempFileGuard::new(out_tmp_path.clone());

    let mut entries: Vec<VanillaEntry> = Vec::with_capacity(1 + planned_assets.len());
    {
        let blobs_file = File::create(&blobs_tmp_path)
            .map_err(|e| format!("No se pudo crear buffer temporal Vanilla: {}", e))?;
        let mut blob_writer = BufWriter::new(blobs_file);
        let mut blob_offset = 0_u64;

        let proj_entry = write_entry_to_blob_stream(
            &mut blob_writer,
            &mut blob_offset,
            "project.json",
            "project_json",
            project_json_len,
            PayloadSource::Bytes(project_json_bytes),
        )?;
        entries.push(proj_entry);

        for item in planned_assets {
            let entry = write_entry_to_blob_stream(
                &mut blob_writer,
                &mut blob_offset,
                &item.rel_path,
                &item.kind,
                item.source_bytes,
                item.payload,
            )?;
            entries.push(entry);
        }
    }

    let total_source_bytes: u64 = entries.iter().map(|e| e.source_bytes).sum();
    let total_compressed_bytes: u64 = entries.iter().map(|e| e.compressed_bytes).sum();

    let manifest = VanillaManifest {
        container_name: "Vanilla".to_string(),
        format_version: VANILLA_FORMAT_VERSION,
        extension_hint: VANILLA_FILE_EXTENSION.to_string(),
        project_name: cloned.name.clone(),
        author: cloned.author.clone(),
        chart_title: cloned.chart.title.clone(),
        bpm: cloned.chart.bpm,
        total_notes: cloned.chart.stats.total_notes,
        track_count: cloned.active_audio_count(),
        entity_count: cloned.stage.entities.len(),
        script_count: cloned.scripts.len(),
        created_at_ms: current_epoch_ms(),
        optimized_webp: optimize_png_to_webp,
        deduplicated_assets,
        png_converted_count,
        total_source_bytes,
        total_compressed_bytes,
        entries,
    };

    let manifest_bytes = serde_json::to_vec(&manifest)
        .map_err(|e| format!("Error serializando manifiesto Vanilla: {}", e))?;
    if manifest_bytes.len() > MAX_VANILLA_MANIFEST_BYTES {
        return Err("El manifiesto del proyecto excede el límite seguro Vanilla".to_string());
    }
    let manifest_fnv1a64 = fnv1a_64_bytes(&manifest_bytes);

    let mut manifest_compressed = Vec::new();
    {
        let mut enc = zstd::stream::Encoder::new(&mut manifest_compressed, VANILLA_ZSTD_JSON_LEVEL)
            .map_err(|e| format!("Error iniciando compresión Zstd del manifiesto: {}", e))?;
        enc.write_all(&manifest_bytes)
            .map_err(|e| format!("Error comprimiendo manifiesto Vanilla: {}", e))?;
        enc.finish()
            .map_err(|e| format!("Error finalizando manifiesto Vanilla: {}", e))?;
    }

    let manifest_comp_u32 = u32::try_from(manifest_compressed.len())
        .map_err(|_| "Manifiesto Vanilla demasiado grande".to_string())?;
    let manifest_uncomp_u32 = u32::try_from(manifest_bytes.len())
        .map_err(|_| "Manifiesto Vanilla demasiado grande".to_string())?;

    let mut flags = 0_u16;
    if optimize_png_to_webp {
        flags |= VANILLA_FLAG_OPTIMIZED_WEBP;
    }
    if deduplicated_assets > 0 {
        flags |= VANILLA_FLAG_ASSET_DEDUP;
    }

    // 4. Ensamblar cabecera (24B) + manifiesto Zstd + blobs en `.mod.tmp` -> `rename` atómico
    {
        let out_file = File::create(&out_tmp_path)
            .map_err(|e| format!("No se pudo crear archivo .mod temporal: {}", e))?;
        let mut out_writer = BufWriter::new(out_file);

        let mut header = [0u8; VANILLA_HEADER_BYTES];
        header[0..4].copy_from_slice(&VANILLA_MAGIC);
        header[4..6].copy_from_slice(&VANILLA_FORMAT_VERSION.to_le_bytes());
        header[6..8].copy_from_slice(&flags.to_le_bytes());
        header[8..12].copy_from_slice(&manifest_comp_u32.to_le_bytes());
        header[12..16].copy_from_slice(&manifest_uncomp_u32.to_le_bytes());
        header[16..24].copy_from_slice(&manifest_fnv1a64.to_le_bytes());

        out_writer
            .write_all(&header)
            .map_err(|e| format!("Error escribiendo cabecera Vanilla: {}", e))?;
        out_writer
            .write_all(&manifest_compressed)
            .map_err(|e| format!("Error escribiendo manifiesto Vanilla: {}", e))?;

        let blobs_in = File::open(&blobs_tmp_path)
            .map_err(|e| format!("Error abriendo blobs temporales Vanilla: {}", e))?;
        let mut blobs_reader = BufReader::new(blobs_in);
        std::io::copy(&mut blobs_reader, &mut out_writer)
            .map_err(|e| format!("Error copiando blobs hacia paquete Vanilla: {}", e))?;
        out_writer
            .flush()
            .map_err(|e| format!("Error vaciando archivo Vanilla final: {}", e))?;
    }

    if let Err(e) = std::fs::rename(&out_tmp_path, out_path) {
        std::fs::copy(&out_tmp_path, out_path)
            .map_err(|copy_err| format!("No se pudo finalizar archivo .mod ({} / {})", e, copy_err))?;
        let _ = std::fs::remove_file(&out_tmp_path);
    }
    out_guard.disarm();
    drop(blobs_guard);
    let _ = std::fs::remove_file(&blobs_tmp_path);

    let archive_bytes = std::fs::metadata(out_path)
        .map(|m| m.len())
        .unwrap_or(VANILLA_HEADER_BYTES as u64 + manifest_compressed.len() as u64 + total_compressed_bytes);
    let saved_bytes = total_source_bytes.saturating_sub(archive_bytes);
    let ratio_percent = if total_source_bytes > 0 {
        (saved_bytes as f64 * 100.0 / total_source_bytes as f64) as f32
    } else {
        0.0
    };

    Ok(VanillaExportReport {
        manifest,
        archive_bytes,
        source_bytes: total_source_bytes,
        png_converted_count,
        deduplicated_assets,
        saved_bytes,
        ratio_percent,
    })
}

fn read_manifest_from_reader(file: &mut File) -> Result<(VanillaManifest, u64), String> {
    let total_file_len = file
        .metadata()
        .map_err(|e| format!("Error leyendo tamaño del archivo .mod: {}", e))?
        .len();
    if total_file_len < VANILLA_HEADER_BYTES as u64 {
        return Err("Archivo .mod inválido o truncado (< 24 bytes)".to_string());
    }

    let mut header = [0u8; VANILLA_HEADER_BYTES];
    file.read_exact(&mut header)
        .map_err(|e| format!("No se pudo leer cabecera Vanilla: {}", e))?;

    if header[0..4] != VANILLA_MAGIC {
        return Err("Formato no compatible: falta firma mágica Vanilla (VNLA)".to_string());
    }
    let version = u16::from_le_bytes([header[4], header[5]]);
    if version != VANILLA_FORMAT_VERSION {
        return Err(format!("Versión de contenedor Vanilla no soportada: v{}", version));
    }

    let manifest_comp_len = u32::from_le_bytes([header[8], header[9], header[10], header[11]]) as usize;
    let manifest_uncomp_len =
        u32::from_le_bytes([header[12], header[13], header[14], header[15]]) as usize;
    let expected_fnv = u64::from_le_bytes([
        header[16], header[17], header[18], header[19], header[20], header[21], header[22],
        header[23],
    ]);

    if manifest_comp_len == 0
        || manifest_comp_len > MAX_VANILLA_MANIFEST_BYTES
        || manifest_uncomp_len == 0
        || manifest_uncomp_len > MAX_VANILLA_MANIFEST_BYTES
    {
        return Err("Tamaño de manifiesto Vanilla fuera de límites seguros".to_string());
    }
    if (VANILLA_HEADER_BYTES as u64) + (manifest_comp_len as u64) > total_file_len {
        return Err("Archivo .mod truncado dentro del manifiesto".to_string());
    }

    let mut comp_buf = vec![0u8; manifest_comp_len];
    file.read_exact(&mut comp_buf)
        .map_err(|e| format!("Error leyendo manifiesto comprimido Vanilla: {}", e))?;
    let data_section_start = (VANILLA_HEADER_BYTES + manifest_comp_len) as u64;

    let mut dec = zstd::stream::Decoder::new(Cursor::new(&comp_buf))
        .map_err(|e| format!("Error inicializando decoder Zstd del manifiesto: {}", e))?;
    let mut manifest_bytes = Vec::with_capacity(manifest_uncomp_len.min(1024 * 1024));
    (&mut dec)
        .take((MAX_VANILLA_MANIFEST_BYTES + 1) as u64)
        .read_to_end(&mut manifest_bytes)
        .map_err(|e| format!("Error descomprimiendo manifiesto Vanilla: {}", e))?;

    if manifest_bytes.len() != manifest_uncomp_len {
        return Err("Longitud descomprimida del manifiesto Vanilla no coincide".to_string());
    }
    if fnv1a_64_bytes(&manifest_bytes) != expected_fnv {
        return Err("Manifiesto Vanilla corrupto: checksum FNV-1a inválido".to_string());
    }

    let manifest: VanillaManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|e| format!("Manifiesto JSON Vanilla inválido: {}", e))?;
    if manifest.entries.len() > MAX_VANILLA_ENTRIES {
        return Err("El paquete Vanilla excede el máximo de archivos permitidos".to_string());
    }

    for entry in &manifest.entries {
        validate_vanilla_rel_path(&entry.rel_path)?;
        let end = entry
            .offset
            .checked_add(entry.compressed_bytes)
            .and_then(|v| data_section_start.checked_add(v))
            .ok_or_else(|| format!("Overflow de offset en entrada '{}'", entry.rel_path))?;
        if end > total_file_len {
            return Err(format!(
                "Archivo .mod truncado en entrada '{}'",
                entry.rel_path
            ));
        }
    }

    Ok((manifest, data_section_start))
}

/// Inspecciona únicamente la cabecera + manifiesto Zstd de un archivo `.mod` sin extraer assets.
pub fn inspect_vanilla_mod(mod_path: &Path) -> Result<VanillaManifest, String> {
    let mut file = File::open(mod_path)
        .map_err(|e| format!("No se pudo abrir archivo .mod '{}': {}", mod_path.display(), e))?;
    let (manifest, _) = read_manifest_from_reader(&mut file)?;
    Ok(manifest)
}

fn extract_entry_to_writer<W: Write>(
    file: &mut File,
    data_section_start: u64,
    entry: &VanillaEntry,
    writer: &mut W,
) -> Result<(), String> {
    file.seek(SeekFrom::Start(data_section_start + entry.offset))
        .map_err(|e| format!("Error buscando offset de '{}': {}", entry.rel_path, e))?;

    let mut hasher = Fnv1a64Hasher::new();
    let mut total_written = 0_u64;
    let mut buf = vec![0u8; STREAM_CHUNK_BYTES];

    let mut limited = file.take(entry.compressed_bytes);
    match entry.codec {
        VanillaEntryCodec::Raw => loop {
            let n = limited
                .read(&mut buf)
                .map_err(|e| format!("Error leyendo entrada '{}': {}", entry.rel_path, e))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            total_written += n as u64;
            writer
                .write_all(&buf[..n])
                .map_err(|e| format!("Error extrayendo '{}': {}", entry.rel_path, e))?;
        },
        VanillaEntryCodec::Zstd => {
            let mut dec = zstd::stream::Decoder::new(limited)
                .map_err(|e| format!("Error iniciando Zstd en '{}': {}", entry.rel_path, e))?;
            loop {
                let n = dec
                    .read(&mut buf)
                    .map_err(|e| format!("Error descomprimiendo '{}': {}", entry.rel_path, e))?;
                if n == 0 {
                    break;
                }
                total_written += n as u64;
                if total_written > entry.uncompressed_bytes {
                    return Err(format!(
                        "Entrada '{}' excede tamaño declarado en manifiesto Vanilla",
                        entry.rel_path
                    ));
                }
                hasher.update(&buf[..n]);
                writer
                    .write_all(&buf[..n])
                    .map_err(|e| format!("Error extrayendo '{}': {}", entry.rel_path, e))?;
            }
        }
    }

    if total_written != entry.uncompressed_bytes {
        return Err(format!(
            "Tamaño extraído no coincide para '{}' ({} vs {})",
            entry.rel_path, total_written, entry.uncompressed_bytes
        ));
    }
    if hasher.finish() != entry.fnv1a64 {
        return Err(format!(
            "Checksum FNV-1a corrupto en '{}' dentro del archivo .mod",
            entry.rel_path
        ));
    }
    writer
        .flush()
        .map_err(|e| format!("Error vaciando '{}': {}", entry.rel_path, e))?;
    Ok(())
}

fn remap_imported_rel_path(slot: &mut Option<PathBuf>, target_project_dir: &Path) -> Result<(), String> {
    let Some(cur) = slot.as_ref() else {
        return Ok(());
    };
    let s = cur.to_string_lossy().replace('\\', "/");
    if s.is_empty() {
        *slot = None;
        return Ok(());
    }
    validate_vanilla_rel_path(&s)?;
    let resolved = target_project_dir.join(&s);
    if resolved.is_file() {
        *slot = Some(resolved);
    } else {
        *slot = None;
    }
    Ok(())
}

/// Importa y descomprime un archivo `.mod` (Vanilla + Zstd) hacia una carpeta temporal, verifica todos
/// los checksums y reescribe rutas hacia `target_project_dir` antes de publicarlo de forma atómica.
pub fn import_project_vanilla_mod(
    mod_path: &Path,
    target_project_id: &str,
    target_project_dir: &Path,
) -> Result<VanillaImportResult, String> {
    let mut file = File::open(mod_path)
        .map_err(|e| format!("No se pudo abrir paquete .mod '{}': {}", mod_path.display(), e))?;
    let (manifest, data_section_start) = read_manifest_from_reader(&mut file)?;

    if let Some(parent) = target_project_dir.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Error creando directorio raíz de proyectos: {}", e))?;
    }

    let staging_dir = target_project_dir.with_extension("vanilla_import_tmp");
    if staging_dir.exists() {
        let _ = std::fs::remove_dir_all(&staging_dir);
    }
    std::fs::create_dir_all(&staging_dir)
        .map_err(|e| format!("Error creando carpeta temporal de importación: {}", e))?;
    let mut staging_guard = TempDirGuard::new(staging_dir.clone());

    let mut project_json_bytes: Option<Vec<u8>> = None;
    let mut extracted_files = 0_usize;

    for entry in &manifest.entries {
        if entry.rel_path == "project.json" {
            if entry.uncompressed_bytes > MAX_VANILLA_PROJECT_JSON_BYTES as u64 {
                return Err("project.json dentro del .mod excede límite seguro (64 MiB)".to_string());
            }
            let mut buf = Vec::with_capacity(entry.uncompressed_bytes as usize);
            extract_entry_to_writer(&mut file, data_section_start, entry, &mut buf)?;
            project_json_bytes = Some(buf);
        } else {
            let dest_path = staging_dir.join(&entry.rel_path);
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!("No se pudo crear carpeta para '{}': {}", entry.rel_path, e)
                })?;
            }
            let out_f = File::create(&dest_path).map_err(|e| {
                format!("No se pudo crear archivo '{}': {}", entry.rel_path, e)
            })?;
            let mut out_writer = BufWriter::new(out_f);
            extract_entry_to_writer(&mut file, data_section_start, entry, &mut out_writer)?;
            extracted_files += 1;
        }
    }

    let json_bytes = project_json_bytes
        .ok_or_else(|| "El paquete Vanilla .mod no contiene 'project.json'".to_string())?;
    let mut proj: ModProject = serde_json::from_slice(&json_bytes)
        .map_err(|e| format!("Error leyendo project.json dentro de Vanilla: {}", e))?;

    proj.id = target_project_id.to_string();
    proj.updated_at_ms = current_epoch_ms();

    // Reescribir rutas hacia la ruta temporal primero para verificar existencia y decodificar ASTC,
    // luego apuntarán finalmente a `target_project_dir` tras el rename atómico.
    for track in &mut proj.audio_tracks {
        remap_imported_rel_path(&mut track.file_path, &staging_dir)?;
    }

    let mut decoded_astc_count = 0_usize;
    for ent in &mut proj.stage.entities {
        remap_imported_rel_path(&mut ent.raw_image_path, &staging_dir)?;
        remap_imported_rel_path(&mut ent.image_path, &staging_dir)?;
        remap_imported_rel_path(&mut ent.icon_normal_path, &staging_dir)?;
        remap_imported_rel_path(&mut ent.icon_losing_path, &staging_dir)?;

        // Reconstruir `.webp` lossless para texturas `.astc` almacenadas de forma compacta
        if ent.image_path.is_none() {
            if let Some(raw_p) = ent.raw_image_path.as_ref() {
                if raw_p.is_file() && is_astc_path(raw_p) {
                    let decoded = load_sprite_image(raw_p)?;
                    let dec_dir = staging_dir.join("textures").join("decoded_textures");
                    let safe_ent = sanitize_slug(&ent.id, "entity", 32);
                    let out_webp = dec_dir.join(format!("{}.webp", safe_ent));
                    save_lossless_webp(&decoded, &out_webp)?;
                    ent.image_path = Some(out_webp);
                    decoded_astc_count += 1;
                }
            }
        }
    }

    // Reemplazar prefijo `staging_dir` -> `target_project_dir` antes de grabar `project.json`
    let swap_prefix = |slot: &mut Option<PathBuf>| {
        if let Some(cur) = slot.as_ref() {
            if let Ok(rel) = cur.strip_prefix(&staging_dir) {
                *slot = Some(target_project_dir.join(rel));
            }
        }
    };
    for track in &mut proj.audio_tracks {
        swap_prefix(&mut track.file_path);
    }
    for ent in &mut proj.stage.entities {
        swap_prefix(&mut ent.raw_image_path);
        swap_prefix(&mut ent.image_path);
        swap_prefix(&mut ent.icon_normal_path);
        swap_prefix(&mut ent.icon_losing_path);
    }

    proj.ensure_camera_script();
    proj.ensure_stage_events_script();

    let final_json = serde_json::to_vec(&proj)
        .map_err(|e| format!("Error guardando project.json importado: {}", e))?;
    std::fs::write(staging_dir.join("project.json"), final_json)
        .map_err(|e| format!("Error escribiendo project.json final: {}", e))?;

    if target_project_dir.exists() {
        std::fs::remove_dir_all(target_project_dir)
            .map_err(|e| format!("No se pudo preparar destino de proyecto: {}", e))?;
    }
    std::fs::rename(&staging_dir, target_project_dir)
        .map_err(|e| format!("No se pudo instalar proyecto importado: {}", e))?;
    staging_guard.disarm();

    Ok(VanillaImportResult {
        manifest,
        project: proj,
        extracted_files,
        decoded_astc_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    #[test]
    fn vanilla_codec_classification_compresses_text_wav_astc_and_stores_compressed_media() {
        assert_eq!(
            classify_vanilla_entry_codec("project.json"),
            (VanillaEntryCodec::Zstd, VANILLA_ZSTD_JSON_LEVEL)
        );
        assert_eq!(
            classify_vanilla_entry_codec("assets/audio/song.wav"),
            (VanillaEntryCodec::Zstd, VANILLA_ZSTD_DATA_LEVEL)
        );
        assert_eq!(
            classify_vanilla_entry_codec("assets/sprites/hero.astc"),
            (VanillaEntryCodec::Zstd, VANILLA_ZSTD_DATA_LEVEL)
        );
        assert_eq!(
            classify_vanilla_entry_codec("assets/audio/inst.ogg"),
            (VanillaEntryCodec::Raw, 0)
        );
        assert_eq!(
            classify_vanilla_entry_codec("assets/sprites/hero.webp"),
            (VanillaEntryCodec::Raw, 0)
        );
    }

    #[test]
    fn vanilla_roundtrip_exports_and_imports_preserving_pixels_audio_and_dedup() {
        let tmp_root = std::env::temp_dir().join(format!("beatforge_vanilla_rt_{}", current_epoch_ms()));
        let src_assets = tmp_root.join("src_assets");
        let _ = std::fs::create_dir_all(&src_assets);

        // 1. Crear PNG con patrón sólido repetitivo (PNG -> WebP lossless encoge muy bien)
        let png_path = src_assets.join("stage_bg.png");
        let mut img = RgbaImage::new(128, 128);
        for (x, y, px) in img.enumerate_pixels_mut() {
            *px = image::Rgba([((x / 8) * 16) as u8, ((y / 8) * 16) as u8, 220, 255]);
        }
        image::DynamicImage::ImageRgba8(img.clone())
            .save(&png_path)
            .expect("guardar PNG fuente");

        // 2. Crear WAV silencioso/repetitivo (Zstd logra ratio brutal)
        let wav_path = src_assets.join("inst.wav");
        let mut wav_bytes = Vec::from(b"RIFF\x24\x40\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x02\x00\x80\xbb\x00\x00\x00\xee\x02\x00\x04\x00\x10\x00data\x00\x40\x00\x00".as_slice());
        wav_bytes.resize(wav_bytes.len() + 16384, 0u8);
        std::fs::write(&wav_path, &wav_bytes).expect("guardar WAV");

        // 3. Crear OGG simulado (debe guardarse Raw)
        let ogg_path = src_assets.join("voices.ogg");
        std::fs::write(&ogg_path, b"OggS_fake_compressed_stream_payload").expect("guardar OGG");

        let mut proj = ModProject::new_blank("proj_src", "Test Vanilla Mod", "Tester");
        proj.assign_audio_track(0, wav_path).unwrap();
        proj.assign_audio_track(1, ogg_path).unwrap();
        if let Some(bg) = proj.stage.entities.first_mut() {
            bg.raw_image_path = Some(png_path.clone());
            bg.image_path = Some(png_path);
        }

        let mod_file = tmp_root.join("test_vanilla_mod.mod");
        let report = export_project_vanilla_mod(&proj, &mod_file, true).expect("exportación Vanilla");
        assert!(mod_file.is_file());
        assert!(report.archive_bytes < report.source_bytes);
        assert_eq!(report.deduplicated_assets, 1); // raw_image_path == image_path deduplicado

        let inspected = inspect_vanilla_mod(&mod_file).expect("inspección rápida");
        assert_eq!(inspected.container_name, "Vanilla");
        assert_eq!(inspected.project_name, "Test Vanilla Mod");

        let dst_dir = tmp_root.join("installed_proj");
        let imported = import_project_vanilla_mod(&mod_file, "proj_dst", &dst_dir)
            .expect("importación Vanilla");
        assert_eq!(imported.project.id, "proj_dst");
        assert_eq!(imported.project.name, "Test Vanilla Mod");
        assert!(imported.project.audio_tracks[0].file_path.as_ref().unwrap().is_file());
        assert!(imported.project.audio_tracks[1].file_path.as_ref().unwrap().is_file());

        let imported_bg = imported.project.stage.entities[0]
            .image_path
            .as_ref()
            .expect("ruta imagen importada");
        let reloaded_img = load_sprite_image(imported_bg).expect("recarga WebP/PNG");
        assert_eq!(reloaded_img.to_rgba8().get_pixel(10, 20), img.get_pixel(10, 20));

        let _ = std::fs::remove_dir_all(&tmp_root);
    }

    #[test]
    fn vanilla_rejects_corrupted_checksum_and_unsafe_paths() {
        assert!(validate_vanilla_rel_path("../escape.txt").is_err());
        assert!(validate_vanilla_rel_path("/abs/path.png").is_err());
        assert!(validate_vanilla_rel_path("C:\\win\\a.png").is_err());
        assert!(validate_vanilla_rel_path("assets/sprites/bf.webp").is_ok());

        let tmp_root = std::env::temp_dir().join(format!("beatforge_vanilla_corrupt_{}", current_epoch_ms()));
        let _ = std::fs::create_dir_all(&tmp_root);
        let proj = ModProject::new_demo();
        let mod_file = tmp_root.join("corrupt_test.mod");
        export_project_vanilla_mod(&proj, &mod_file, true).expect("export");

        let mut bytes = std::fs::read(&mod_file).expect("leer mod");
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF; // Corromper último byte de datos
        std::fs::write(&mod_file, &bytes).expect("escribir mod corrupto");

        let res = import_project_vanilla_mod(&mod_file, "proj_bad", &tmp_root.join("bad_out"));
        assert!(res.is_err());

        let _ = std::fs::remove_dir_all(&tmp_root);
    }
}
