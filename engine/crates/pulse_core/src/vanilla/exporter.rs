//! Archivo: engine/crates/pulse_core/src/vanilla/exporter.rs
//! Propósito: Empaquetador y exportador por streaming de un `ModProject` completo al formato
//! portátil **Vanilla** (`.mod`) con deduplicación, optimización PNG->WebP VP8L y compresión Zstd.

use super::helpers::{
    Fnv1a64Hasher, TempFileGuard, allocate_unique_rel_path, canonical_key, sanitize_slug,
    strip_to_file_name, try_convert_to_smaller_lossless_webp,
};
use super::models::{
    MAX_VANILLA_MANIFEST_BYTES, STREAM_CHUNK_BYTES, VANILLA_FILE_EXTENSION, VANILLA_FLAG_ASSET_DEDUP,
    VANILLA_FLAG_OPTIMIZED_WEBP, VANILLA_FORMAT_VERSION, VANILLA_HEADER_BYTES, VANILLA_MAGIC,
    VANILLA_ZSTD_JSON_LEVEL, VanillaEntry, VanillaEntryCodec, VanillaExportReport, VanillaManifest,
    classify_vanilla_entry_codec,
};
use crate::astc_loader::is_astc_path;
use crate::hot_compile::fnv1a_64_bytes;
use crate::project::{MAX_AUDIO_TRACKS, ModProject, current_epoch_ms};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, Write};
use std::path::{Path, PathBuf};

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

    // 2b. Recursos de cuenta regresiva ("2 / Ready?", "1 / Set?", "¡YA! / Go"): imagen, XML opcional y voz/SFX
    for (slot_idx, cd) in cloned.countdown.slots.iter_mut().enumerate() {
        let slot_stem = format!("countdown_step_{}", slot_idx + 1);
        let skip_redundant_astc_decoded = cd
            .raw_image_path
            .as_ref()
            .is_some_and(|p| p.is_file() && is_astc_path(p));

        if let Some(raw_p) = cd.raw_image_path.clone() {
            if raw_p.is_file() {
                let is_astc = is_astc_path(&raw_p);
                let rel = stage_binary_asset(
                    &raw_p,
                    "assets/countdown",
                    &slot_stem,
                    "countdown_sprite",
                    !is_astc,
                )?;
                cd.raw_image_path = Some(PathBuf::from(rel));
            } else {
                cd.raw_image_path = None;
            }
        }

        if skip_redundant_astc_decoded {
            cd.image_path = None;
        } else if let Some(img_p) = cd.image_path.clone() {
            if img_p.is_file() {
                let is_astc = is_astc_path(&img_p);
                let rel = stage_binary_asset(
                    &img_p,
                    "assets/countdown",
                    &format!("{}_img", slot_stem),
                    "countdown_sprite",
                    !is_astc,
                )?;
                cd.image_path = Some(PathBuf::from(rel));
            } else {
                cd.image_path = None;
            }
        }

        if let Some(xml_p) = cd.xml_path.clone() {
            let clean_name = xml_p
                .file_name()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(format!("{}.xml", slot_stem)));
            cd.xml_path = Some(clean_name);
        }

        if let Some(aud_p) = cd.audio_path.clone() {
            if aud_p.is_file() {
                let rel = stage_binary_asset(
                    &aud_p,
                    "assets/countdown",
                    &format!("{}_voice", slot_stem),
                    "countdown_audio",
                    false,
                )?;
                cd.audio_path = Some(PathBuf::from(rel));
            } else {
                cd.audio_path = None;
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
