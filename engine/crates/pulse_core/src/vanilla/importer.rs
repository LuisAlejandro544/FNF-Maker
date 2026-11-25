//! Archivo: engine/crates/pulse_core/src/vanilla/importer.rs
//! Propósito: Lector, verificador e importador por streaming de paquetes **Vanilla** (`.mod`),
//! validando cabecera, hashes FNV-1a, prevención de path traversal y decodificación ASTC->WebP.

use super::helpers::{Fnv1a64Hasher, TempDirGuard, sanitize_slug};
use super::models::{
    MAX_VANILLA_ENTRIES, MAX_VANILLA_MANIFEST_BYTES, MAX_VANILLA_PROJECT_JSON_BYTES,
    STREAM_CHUNK_BYTES, VANILLA_FORMAT_VERSION, VANILLA_HEADER_BYTES, VANILLA_MAGIC, VanillaEntry,
    VanillaEntryCodec, VanillaImportResult, VanillaManifest, validate_vanilla_rel_path,
};
use crate::astc_loader::{is_astc_path, load_sprite_image, save_lossless_webp};
use crate::hot_compile::fnv1a_64_bytes;
use crate::project::{ModProject, current_epoch_ms};
use std::fs::File;
use std::io::{BufWriter, Cursor, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

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

    for (slot_idx, cd) in proj.countdown.slots.iter_mut().enumerate() {
        remap_imported_rel_path(&mut cd.raw_image_path, &staging_dir)?;
        remap_imported_rel_path(&mut cd.image_path, &staging_dir)?;
        remap_imported_rel_path(&mut cd.audio_path, &staging_dir)?;

        if cd.image_path.is_none() {
            if let Some(raw_p) = cd.raw_image_path.as_ref() {
                if raw_p.is_file() && is_astc_path(raw_p) {
                    let decoded = load_sprite_image(raw_p)?;
                    let dec_dir = staging_dir.join("textures").join("decoded_textures");
                    let out_webp = dec_dir.join(format!("countdown_{}.webp", slot_idx));
                    save_lossless_webp(&decoded, &out_webp)?;
                    cd.image_path = Some(out_webp);
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
    for cd in &mut proj.countdown.slots {
        swap_prefix(&mut cd.raw_image_path);
        swap_prefix(&mut cd.image_path);
        swap_prefix(&mut cd.audio_path);
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
