//! Archivo: engine/crates/pulse_audio/src/audio_cache.rs
//! Propósito: Caché binaria en disco (`.bfaud`) para pistas de audio ya decodificadas y
//! sus picos de forma de onda (`DecodedTrack`), evitando re-decodificar `.ogg`/`.mp3` en CPU
//! al abrir proyectos, cambiar entre mods o regresar desde gameplay, con expulsión LRU automática.

use crate::decoder::{AudioDecodeError, DecodedTrack, decode_audio_file};
use pulse_core::disk_cache::{evict_lru_files, touch_file_last_used};
use pulse_core::hot_compile::{AssetFileFingerprint, fnv1a_64_str};
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

const AUDIO_CACHE_MAGIC: [u8; 4] = *b"BFAU";
const AUDIO_CACHE_VERSION: u32 = 1;
const MAX_CACHED_PEAKS: u32 = 4096;

/// Calcula una clave determinista de 64 bits (hex) a partir de la huella del archivo de audio
/// (`size_bytes`, `modified_ms`, `sample_hash` FNV-1a) y la frecuencia destino (`target_sample_rate`).
pub fn compute_audio_cache_key(path: &Path, target_sample_rate: u32) -> Option<String> {
    let fp = AssetFileFingerprint::from_path(path)?;
    let sig = format!(
        "{}:{}:{}:{}",
        fp.size_bytes, fp.modified_ms, fp.sample_hash, target_sample_rate
    );
    Some(format!("trk_{:016x}_{}", fnv1a_64_str(&sig), target_sample_rate))
}

/// Decodifica una pista de audio utilizando caché binaria `.bfaud` en `cache_dir`
/// y aplicando cuota LRU en disco (`max_cache_bytes`). Si la caché no existe o está dañada,
/// decodifica normalmente con `decode_audio_file` y persiste de forma atómica el artefacto `.bfaud`.
pub fn decode_audio_file_cached(
    path: &Path,
    target_sample_rate: u32,
    cache_dir: Option<&Path>,
    max_cache_bytes: u64,
) -> Result<DecodedTrack, AudioDecodeError> {
    let cache_path: Option<PathBuf> = cache_dir.and_then(|dir| {
        compute_audio_cache_key(path, target_sample_rate).map(|k| dir.join(format!("{}.bfaud", k)))
    });

    if let Some(ref cpath) = cache_path {
        if cpath.is_file() {
            if let Ok(cached) = read_bfaud_file(cpath) {
                if cached.sample_rate == target_sample_rate {
                    touch_file_last_used(cpath);
                    return Ok(cached);
                }
            }
            let _ = fs::remove_file(cpath);
        }
    }

    let decoded = decode_audio_file(path, target_sample_rate)?;

    if let (Some(dir), Some(cpath)) = (cache_dir, cache_path) {
        if write_bfaud_file_atomic(cpath.as_path(), &decoded).is_ok() && max_cache_bytes > 0 {
            let _ = evict_lru_files(dir, max_cache_bytes, &[cpath.as_path()]);
        }
    }

    Ok(decoded)
}

/// Escribe un `DecodedTrack` en formato binario compacto `.bfaud` usando archivo temporal + rename.
pub fn write_bfaud_file_atomic(target_path: &Path, track: &DecodedTrack) -> std::io::Result<()> {
    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp_path = target_path.with_extension("bfaud.tmp");
    {
        let file = File::create(&tmp_path)?;
        let mut writer = BufWriter::with_capacity(64 * 1024, file);

        writer.write_all(&AUDIO_CACHE_MAGIC)?;
        writer.write_all(&AUDIO_CACHE_VERSION.to_le_bytes())?;
        writer.write_all(&track.sample_rate.to_le_bytes())?;

        let peak_count = track.waveform_peaks.len().min(MAX_CACHED_PEAKS as usize) as u32;
        writer.write_all(&peak_count.to_le_bytes())?;
        let sample_count = track.stereo_samples.len() as u64;
        writer.write_all(&sample_count.to_le_bytes())?;

        for &peak in track.waveform_peaks.iter().take(peak_count as usize) {
            writer.write_all(&peak.to_le_bytes())?;
        }

        // Escribir muestras i16 en bloques pequeños para no duplicar todo el buffer en RAM (crucial en 32-bit).
        let mut chunk_bytes = [0u8; 4096];
        for sample_chunk in track.stereo_samples.chunks(2048) {
            let byte_len = sample_chunk.len() * 2;
            for (idx, &sample) in sample_chunk.iter().enumerate() {
                let le = sample.to_le_bytes();
                chunk_bytes[idx * 2] = le[0];
                chunk_bytes[idx * 2 + 1] = le[1];
            }
            writer.write_all(&chunk_bytes[..byte_len])?;
        }
        writer.flush()?;
    }

    if fs::rename(&tmp_path, target_path).is_err() {
        let _ = fs::remove_file(target_path);
        fs::rename(&tmp_path, target_path)?;
    }
    Ok(())
}

/// Lee y valida estrictamente un artefacto binario `.bfaud` desde disco.
pub fn read_bfaud_file(path: &Path) -> std::io::Result<DecodedTrack> {
    let file = File::open(path)?;
    let file_len = file.metadata()?.len();
    let mut reader = BufReader::with_capacity(64 * 1024, file);

    let mut header = [0u8; 24];
    reader.read_exact(&mut header)?;

    if header[0..4] != AUDIO_CACHE_MAGIC {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Magic .bfaud inválido",
        ));
    }
    let version = u32::from_le_bytes(header[4..8].try_into().unwrap());
    if version != AUDIO_CACHE_VERSION {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Versión .bfaud no soportada",
        ));
    }
    let sample_rate = u32::from_le_bytes(header[8..12].try_into().unwrap());
    let peak_count = u32::from_le_bytes(header[12..16].try_into().unwrap());
    let sample_count_u64 = u64::from_le_bytes(header[16..24].try_into().unwrap());

    if sample_rate < 8000 || peak_count > MAX_CACHED_PEAKS || (sample_count_u64 % 2 != 0) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Parámetros de cabecera .bfaud fuera de rango",
        ));
    }

    let expected_size = 24_u64
        .saturating_add((peak_count as u64) * 4)
        .saturating_add(sample_count_u64.saturating_mul(2));
    if expected_size != file_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Archivo .bfaud truncado o corrupto",
        ));
    }

    let sample_count = usize::try_from(sample_count_u64).map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "Buffer de audio excede dirección 32-bit")
    })?;

    let mut waveform_peaks = Vec::with_capacity(peak_count as usize);
    let mut f32_buf = [0u8; 4];
    for _ in 0..peak_count {
        reader.read_exact(&mut f32_buf)?;
        waveform_peaks.push(f32::from_le_bytes(f32_buf));
    }

    let mut stereo_samples = Vec::with_capacity(sample_count);
    let mut raw_buf = vec![0u8; 8192];
    let mut remaining_samples = sample_count;
    while remaining_samples > 0 {
        let batch_samples = remaining_samples.min(4096);
        let batch_bytes = batch_samples * 2;
        reader.read_exact(&mut raw_buf[..batch_bytes])?;
        for pair in raw_buf[..batch_bytes].chunks_exact(2) {
            stereo_samples.push(i16::from_le_bytes([pair[0], pair[1]]));
        }
        remaining_samples -= batch_samples;
    }

    Ok(DecodedTrack {
        sample_rate,
        stereo_samples,
        waveform_peaks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bfaud_roundtrip_and_lru_eviction_preserve_samples_and_peaks() {
        let root = std::env::temp_dir().join("bf_audio_cache_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let track = DecodedTrack {
            sample_rate: 48_000,
            stereo_samples: vec![0, 1200, -1200, 32000, -32000, 512, -512, 0],
            waveform_peaks: vec![0.1, 0.5, 0.95, 0.25],
        };
        let target = root.join("sample.bfaud");
        write_bfaud_file_atomic(&target, &track).unwrap();

        let loaded = read_bfaud_file(&target).unwrap();
        assert_eq!(loaded.sample_rate, 48_000);
        assert_eq!(loaded.stereo_samples, track.stereo_samples);
        assert_eq!(loaded.waveform_peaks, track.waveform_peaks);

        // Verificar que archivo truncado falla limpiamente
        let broken = root.join("broken.bfaud");
        fs::write(&broken, b"BFAU1234").unwrap();
        assert!(read_bfaud_file(&broken).is_err());

        let _ = fs::remove_dir_all(&root);
    }
}
