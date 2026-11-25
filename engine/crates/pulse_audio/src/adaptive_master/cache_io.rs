//! Archivo: engine/crates/pulse_audio/src/adaptive_master/cache_io.rs
//! Propósito: Cálculo de huella FNV-1a para caché incremental, lectura/mezcla de
//! pistas en precisión `f32` y escritura atómica WAV de 16 bits con dither TPDF.

use super::models::EffectiveDeviceAcoustics;
use crate::decoder::{DecodedTrack, decode_audio_file_raw_f32, resample_stereo_f32_rubato};
use pulse_core::project::{AudioTrackSlot, MAX_AUDIO_TRACKS};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::Arc;

fn fnv1a_update(hash: &mut u64, bytes: &[u8]) {
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    for &b in bytes {
        *hash ^= b as u64;
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

pub(super) fn compute_adaptation_cache_hash(
    tracks: &[Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS],
    configs: &[AudioTrackSlot; MAX_AUDIO_TRACKS],
    pre_merged_wav: Option<&Path>,
    device: &EffectiveDeviceAcoustics,
) -> String {
    let mut hash: u64 = 0xCBF2_9CE4_8422_2325;
    fnv1a_update(&mut hash, b"bf_audio_adapt_v2");
    fnv1a_update(&mut hash, device.route_label.as_bytes());
    fnv1a_update(&mut hash, device.manufacturer.as_bytes());
    fnv1a_update(&mut hash, device.model.as_bytes());
    fnv1a_update(&mut hash, &device.effective_sample_rate.to_le_bytes());
    fnv1a_update(&mut hash, &[u8::from(device.is_mono_speaker)]);

    if let Some(wav_path) = pre_merged_wav {
        if let Ok(meta) = std::fs::metadata(wav_path) {
            fnv1a_update(&mut hash, wav_path.to_string_lossy().as_bytes());
            fnv1a_update(&mut hash, &meta.len().to_le_bytes());
            let mod_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            fnv1a_update(&mut hash, &mod_ms.to_le_bytes());
        }
    }

    for slot in 0..MAX_AUDIO_TRACKS {
        let cfg = &configs[slot];
        fnv1a_update(&mut hash, &[slot as u8, u8::from(cfg.muted)]);
        fnv1a_update(&mut hash, &cfg.gain.to_bits().to_le_bytes());
        if let Some(path) = &cfg.file_path {
            if let Ok(meta) = std::fs::metadata(path) {
                fnv1a_update(&mut hash, path.to_string_lossy().as_bytes());
                fnv1a_update(&mut hash, &meta.len().to_le_bytes());
                let mod_ms = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                fnv1a_update(&mut hash, &mod_ms.to_le_bytes());
            }
        }
        if let Some(track) = &tracks[slot] {
            fnv1a_update(&mut hash, &track.sample_rate.to_le_bytes());
            fnv1a_update(&mut hash, &(track.stereo_samples.len() as u64).to_le_bytes());
            let step = (track.stereo_samples.len() / 64).max(1);
            let mut idx = 0usize;
            while idx < track.stereo_samples.len() {
                fnv1a_update(&mut hash, &track.stereo_samples[idx].to_le_bytes());
                idx += step;
            }
        }
    }

    format!("{:016x}", hash)
}

pub(super) fn read_wav_stereo_f32(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let mut reader = hound::WavReader::open(path)
        .map_err(|e| format!("No se pudo abrir WAV fuente para adaptar: {}", e))?;
    let spec = reader.spec();
    let ch = spec.channels.max(1) as usize;
    let sr = spec.sample_rate.max(8000);

    let mono_or_multi: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .filter_map(Result::ok)
            .collect(),
        hound::SampleFormat::Int => {
            let max_val = ((1_i64 << (spec.bits_per_sample.saturating_sub(1).max(1))) - 1) as f32;
            reader
                .samples::<i32>()
                .filter_map(Result::ok)
                .map(|s| s as f32 / max_val)
                .collect()
        }
    };

    if ch == 2 {
        return Ok((mono_or_multi, sr));
    }
    let frames = mono_or_multi.len() / ch;
    let mut stereo = Vec::with_capacity(frames * 2);
    for f in 0..frames {
        let base = f * ch;
        let l = mono_or_multi[base];
        let r = if ch >= 2 { mono_or_multi[base + 1] } else { l };
        stereo.push(l);
        stereo.push(r);
    }
    Ok((stereo, sr))
}

/// Decodifica las pistas fuente activas desde disco directamente en `f32` a su frecuencia nativa
/// (evitando `i16` previo y remuestreo lineal), mezclándolas con su ganancia en punto flotante.
pub(super) fn mix_raw_source_files_f32(
    configs: &[AudioTrackSlot; MAX_AUDIO_TRACKS],
) -> Result<Option<(Vec<f32>, u32)>, String> {
    let active_slots: Vec<usize> = (0..MAX_AUDIO_TRACKS)
        .filter(|&slot| {
            !configs[slot].muted
                && configs[slot]
                    .file_path
                    .as_ref()
                    .is_some_and(|p| p.is_file())
        })
        .collect();

    if active_slots.is_empty() {
        return Ok(None);
    }

    let first_slot = active_slots[0];
    let first_path = configs[first_slot].file_path.as_ref().unwrap();
    let (base_sr, mut mix_buf) = decode_audio_file_raw_f32(first_path)
        .map_err(|e| format!("Error decodificando pista {} en f32: {}", first_slot + 1, e))?;
    let first_gain = configs[first_slot].gain.clamp(0.0, 2.0);
    if (first_gain - 1.0).abs() > 1e-5 {
        for s in &mut mix_buf {
            *s *= first_gain;
        }
    }

    for &slot in &active_slots[1..] {
        let path = configs[slot].file_path.as_ref().unwrap();
        let (track_sr, raw_samples) = decode_audio_file_raw_f32(path)
            .map_err(|e| format!("Error decodificando pista {} en f32: {}", slot + 1, e))?;
        let aligned = if track_sr != base_sr {
            resample_stereo_f32_rubato(&raw_samples, track_sr, base_sr)?
        } else {
            raw_samples
        };
        if aligned.len() > mix_buf.len() {
            mix_buf.resize(aligned.len(), 0.0);
        }
        let gain = configs[slot].gain.clamp(0.0, 2.0);
        for (dst, &src) in mix_buf.iter_mut().zip(aligned.iter()) {
            *dst += src * gain;
        }
    }

    if mix_buf.is_empty() {
        return Ok(None);
    }
    Ok(Some((mix_buf, base_sr)))
}

pub(super) fn mix_decoded_tracks_f32(
    tracks: &[Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS],
    configs: &[AudioTrackSlot; MAX_AUDIO_TRACKS],
) -> Option<(Vec<f32>, u32)> {
    let mut active = Vec::with_capacity(MAX_AUDIO_TRACKS);
    let mut max_frames = 0usize;
    let mut sample_rate = 48_000_u32;

    for slot in 0..MAX_AUDIO_TRACKS {
        if let Some(track) = &tracks[slot] {
            if !configs[slot].muted && track.frame_count() > 0 {
                max_frames = max_frames.max(track.frame_count());
                sample_rate = track.sample_rate.max(8000);
                active.push(slot);
            }
        }
    }

    if active.is_empty() || max_frames == 0 {
        return None;
    }

    let mut out = vec![0.0_f32; max_frames * 2];
    for slot in active {
        if let Some(track) = &tracks[slot] {
            let gain = configs[slot].gain.clamp(0.0, 2.0) / 32768.0;
            let len = track.stereo_samples.len().min(out.len());
            for i in 0..len {
                out[i] += (track.stereo_samples[i] as f32) * gain;
            }
        }
    }
    Some((out, sample_rate))
}

#[inline]
fn next_tpdf_lsb_unit(rng_state: &mut u64) -> f32 {
    *rng_state = rng_state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *rng_state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    ((z >> 40) as f32) * (1.0 / 16_777_216.0)
}

pub(super) fn write_stereo_i16_wav_atomic(
    out_path: &Path,
    samples_interleaved: &[f32],
    sample_rate: u32,
) -> Result<(), String> {
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp_path = out_path.with_extension("wav.tmp");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sample_rate.max(8000),
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    {
        let file = File::create(&tmp_path)
            .map_err(|e| format!("No se pudo crear WAV adaptado temporal: {}", e))?;
        let buf_writer = BufWriter::with_capacity(64 * 1024, file);
        let mut writer = hound::WavWriter::new(buf_writer, spec)
            .map_err(|e| format!("Error abriendo cabecera WAV adaptado: {}", e))?;
        let mut rng_state: u64 = 0x9E37_79B9_7F4A_7C15 ^ (sample_rate as u64);
        for &s in samples_interleaved {
            let clean = if s.is_finite() { s.clamp(-1.0, 1.0) } else { 0.0 };
            // Dither triangular TPDF de ±1 LSB para cuantización limpia de 32f a 16-bit
            let tpdf = next_tpdf_lsb_unit(&mut rng_state) - next_tpdf_lsb_unit(&mut rng_state);
            let pcm = (clean * 32767.0 + tpdf).round().clamp(-32768.0, 32767.0) as i16;
            writer
                .write_sample(pcm)
                .map_err(|e| format!("Error escribiendo muestra WAV adaptada: {}", e))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("Error cerrando WAV adaptado: {}", e))?;
    }

    if std::fs::rename(&tmp_path, out_path).is_err() {
        std::fs::copy(&tmp_path, out_path)
            .map_err(|e| format!("Error moviendo WAV adaptado final: {}", e))?;
        let _ = std::fs::remove_file(&tmp_path);
    }
    Ok(())
}
