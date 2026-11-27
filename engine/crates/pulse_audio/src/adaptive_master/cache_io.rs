//! Archivo: engine/crates/pulse_audio/src/adaptive_master/cache_io.rs
//! Propósito: Cálculo de huella FNV-1a para caché incremental (sin invalidación innecesaria por
//! `compiled_mix.wav`), limpieza/poda del directorio `audio_adapted/`, lectura/mezcla por bloques
//! (`8192` frames) en precisión `f32` y escritura atómica WAV en streaming con dither TPDF.

use super::models::EffectiveDeviceAcoustics;
use crate::decoder::{DecodedTrack, StreamingStereoResampler, StreamingTrackDecoder};
use pulse_core::project::{AudioTrackSlot, MAX_AUDIO_TRACKS};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub(super) const ADAPTED_CACHE_MAX_PAIRS: usize = 2;
pub(super) const ADAPTED_CACHE_MAX_BYTES: u64 = 96 * 1024 * 1024;
const STREAM_BLOCK_FRAMES: usize = 8192;

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
    fnv1a_update(&mut hash, b"bf_audio_adapt_v3");
    fnv1a_update(&mut hash, &[device.route as u8]);
    fnv1a_update(&mut hash, device.route_label.as_bytes());
    fnv1a_update(&mut hash, device.manufacturer.as_bytes());
    fnv1a_update(&mut hash, device.model.as_bytes());
    fnv1a_update(&mut hash, &device.effective_sample_rate.to_le_bytes());
    fnv1a_update(&mut hash, &[u8::from(device.is_mono_speaker)]);
    fnv1a_update(&mut hash, &device.aaudio_device_id.to_le_bytes());

    let has_raw_source = (0..MAX_AUDIO_TRACKS).any(|slot| {
        !configs[slot].muted
            && configs[slot]
                .file_path
                .as_ref()
                .is_some_and(|p| p.is_file())
    });

    // Solo incluir `pre_merged_wav` y buffers del editor si no existen archivos fuente crudos en disco,
    // evitando que reconstruir o limpiar `compiled_mix.wav` invalide sin razón `audio_adapted/`.
    if !has_raw_source {
        if let Some(wav_path) = pre_merged_wav {
            if let Ok(meta) = std::fs::metadata(wav_path) {
                fnv1a_update(&mut hash, wav_path.to_string_lossy().as_bytes());
                fnv1a_update(&mut hash, &meta.len().to_le_bytes());
                let mod_ms = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                fnv1a_update(&mut hash, &mod_ms.to_le_bytes());
            }
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
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64)
                    .unwrap_or(0);
                fnv1a_update(&mut hash, &mod_ms.to_le_bytes());
            }
        }
        if !has_raw_source {
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
    }

    format!("{:016x}", hash)
}

/// Limpia archivos `.tmp` huérfanos y poda entradas antiguas dentro de `audio_adapted/`
/// manteniendo protegida la pista adaptada activa actual.
pub(super) fn prune_audio_adapted_cache(
    cache_dir: &Path,
    keep_wav: Option<&Path>,
    keep_json: Option<&Path>,
    max_pairs: usize,
    max_total_bytes: u64,
) {
    let Ok(entries) = std::fs::read_dir(cache_dir) else {
        return;
    };

    struct AdaptedEntry {
        wav_path: PathBuf,
        json_path: PathBuf,
        bytes: u64,
        modified: SystemTime,
        protected: bool,
    }

    let mut pairs: Vec<AdaptedEntry> = Vec::new();
    let mut json_files: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if ext == "tmp" {
            let _ = std::fs::remove_file(&path);
            continue;
        }

        if ext == "wav" && name.starts_with("adapted_") {
            let json_path = path.with_extension("json");
            let wav_meta = std::fs::metadata(&path).ok();
            let json_meta = std::fs::metadata(&json_path).ok();
            let bytes = wav_meta.as_ref().map(|m| m.len()).unwrap_or(0)
                + json_meta.as_ref().map(|m| m.len()).unwrap_or(0);
            let modified = wav_meta
                .and_then(|m| m.modified().ok())
                .unwrap_or(UNIX_EPOCH);
            let protected = keep_wav.is_some_and(|k| k == path.as_path());
            pairs.push(AdaptedEntry {
                wav_path: path,
                json_path,
                bytes,
                modified,
                protected,
            });
        } else if ext == "json" && name.starts_with("adapted_") {
            json_files.push(path);
        }
    }

    // Borrar archivos `.json` huérfanos sin `.wav` correspondiente
    for jp in json_files {
        let protected = keep_json.is_some_and(|k| k == jp.as_path());
        if !protected && !jp.with_extension("wav").is_file() {
            let _ = std::fs::remove_file(jp);
        }
    }

    // Ordenar poniendo primero el protegido y luego los más recientes
    pairs.sort_by(|a, b| {
        b.protected
            .cmp(&a.protected)
            .then_with(|| b.modified.cmp(&a.modified))
    });

    let mut kept_pairs = 0usize;
    let mut kept_bytes = 0u64;
    for item in pairs {
        let over_count = kept_pairs >= max_pairs.max(1);
        let over_bytes = kept_bytes.saturating_add(item.bytes) > max_total_bytes;
        if !item.protected && (over_count || over_bytes) {
            let _ = std::fs::remove_file(&item.wav_path);
            let _ = std::fs::remove_file(&item.json_path);
        } else {
            kept_pairs += 1;
            kept_bytes = kept_bytes.saturating_add(item.bytes);
        }
    }
}

enum SourceMixerStream {
    RawFiles {
        slots: Vec<(usize, f32, StreamingTrackDecoder, Option<StreamingStereoResampler>)>,
        base_sr: u32,
        slot_chunks: Vec<Vec<f32>>,
        slot_resampled: Vec<Vec<f32>>,
        slot_eof: Vec<bool>,
    },
    DecodedTracks {
        active: Vec<(Arc<DecodedTrack>, f32)>,
        frame_cursor: usize,
        max_frames: usize,
        sample_rate: u32,
    },
    PreMergedWav {
        decoder: StreamingTrackDecoder,
    },
}

impl SourceMixerStream {
    fn open(
        tracks: &[Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS],
        configs: &[AudioTrackSlot; MAX_AUDIO_TRACKS],
        pre_merged_wav: Option<&Path>,
    ) -> Result<Option<(Self, u32, bool)>, String> {
        let active_raw: Vec<usize> = (0..MAX_AUDIO_TRACKS)
            .filter(|&slot| {
                !configs[slot].muted
                    && configs[slot]
                        .file_path
                        .as_ref()
                        .is_some_and(|p| p.is_file())
            })
            .collect();

        if !active_raw.is_empty() {
            let mut decoders = Vec::with_capacity(active_raw.len());
            let first_slot = active_raw[0];
            let first_path = configs[first_slot].file_path.as_ref().unwrap();
            let first_dec = StreamingTrackDecoder::open(first_path)
                .map_err(|e| format!("Error abriendo pista {} en f32: {}", first_slot + 1, e))?;
            let base_sr = first_dec.sample_rate().max(8000);
            decoders.push((first_slot, configs[first_slot].gain.clamp(0.0, 2.0), first_dec, None));

            for &slot in &active_raw[1..] {
                let path = configs[slot].file_path.as_ref().unwrap();
                let dec = StreamingTrackDecoder::open(path)
                    .map_err(|e| format!("Error abriendo pista {} en f32: {}", slot + 1, e))?;
                let sr = dec.sample_rate().max(8000);
                let resampler = if sr != base_sr {
                    Some(StreamingStereoResampler::new(sr, base_sr)?)
                } else {
                    None
                };
                decoders.push((slot, configs[slot].gain.clamp(0.0, 2.0), dec, resampler));
            }

            let count = decoders.len();
            return Ok(Some((
                Self::RawFiles {
                    slots: decoders,
                    base_sr,
                    slot_chunks: vec![Vec::with_capacity(STREAM_BLOCK_FRAMES * 2); count],
                    slot_resampled: vec![Vec::with_capacity(STREAM_BLOCK_FRAMES * 2); count],
                    slot_eof: vec![false; count],
                },
                base_sr,
                true,
            )));
        }

        let mut active_mem = Vec::with_capacity(MAX_AUDIO_TRACKS);
        let mut max_frames = 0usize;
        let mut mem_sr = 48_000_u32;
        for slot in 0..MAX_AUDIO_TRACKS {
            if let Some(track) = &tracks[slot] {
                if !configs[slot].muted && track.frame_count() > 0 {
                    max_frames = max_frames.max(track.frame_count());
                    mem_sr = track.sample_rate.max(8000);
                    active_mem.push((Arc::clone(track), configs[slot].gain.clamp(0.0, 2.0)));
                }
            }
        }
        if !active_mem.is_empty() && max_frames > 0 {
            return Ok(Some((
                Self::DecodedTracks {
                    active: active_mem,
                    frame_cursor: 0,
                    max_frames,
                    sample_rate: mem_sr,
                },
                mem_sr,
                false,
            )));
        }

        if let Some(wav_path) = pre_merged_wav {
            if wav_path.is_file() {
                let decoder = StreamingTrackDecoder::open(wav_path).map_err(|e| {
                    format!("No se pudo abrir WAV fuente para adaptar: {}", e)
                })?;
                let sr = decoder.sample_rate().max(8000);
                return Ok(Some((Self::PreMergedWav { decoder }, sr, false)));
            }
        }

        Ok(None)
    }

    fn next_chunk(
        &mut self,
        max_frames: usize,
        out_interleaved: &mut Vec<f32>,
    ) -> Result<usize, String> {
        out_interleaved.clear();
        match self {
            Self::RawFiles {
                slots,
                base_sr: _,
                slot_chunks,
                slot_resampled,
                slot_eof,
            } => {
                let mut max_floats = 0usize;
                for (i, (slot_idx, _gain, dec, resampler_opt)) in slots.iter_mut().enumerate() {
                    let buf = &mut slot_resampled[i];
                    if buf.is_empty() && !slot_eof[i] {
                        let raw = &mut slot_chunks[i];
                        loop {
                            let frames = dec
                                .read_next_frames(max_frames, raw)
                                .map_err(|e| {
                                    format!(
                                        "Error decodificando bloque pista {}: {}",
                                        *slot_idx + 1,
                                        e
                                    )
                                })?;
                            if let Some(resampler) = resampler_opt {
                                if frames > 0 {
                                    resampler.push_interleaved(raw, buf)?;
                                    if !buf.is_empty() {
                                        break;
                                    }
                                } else {
                                    resampler.finish(buf)?;
                                    slot_eof[i] = true;
                                    break;
                                }
                            } else {
                                if frames == 0 {
                                    slot_eof[i] = true;
                                } else {
                                    buf.extend_from_slice(raw);
                                }
                                break;
                            }
                        }
                    }
                    let want = (max_frames * 2).min(buf.len());
                    max_floats = max_floats.max(want);
                }

                if max_floats == 0 {
                    return Ok(0);
                }

                out_interleaved.resize(max_floats, 0.0);
                for (i, (_slot_idx, gain, _dec, _res)) in slots.iter().enumerate() {
                    let buf = &mut slot_resampled[i];
                    let take_floats = buf.len().min(max_floats);
                    if take_floats > 0 {
                        let g = *gain;
                        for j in 0..take_floats {
                            out_interleaved[j] += buf[j] * g;
                        }
                        buf.drain(..take_floats);
                    }
                }
                Ok(max_floats / 2)
            }
            Self::DecodedTracks {
                active,
                frame_cursor,
                max_frames: total_frames,
                sample_rate: _,
            } => {
                if *frame_cursor >= *total_frames {
                    return Ok(0);
                }
                let take_frames = (*total_frames - *frame_cursor).min(max_frames);
                out_interleaved.resize(take_frames * 2, 0.0);
                for (track, gain) in active.iter() {
                    let track_frames = track.frame_count();
                    if *frame_cursor >= track_frames {
                        continue;
                    }
                    let avail = (track_frames - *frame_cursor).min(take_frames);
                    let scale = (*gain) / 32768.0;
                    let base = (*frame_cursor) * 2;
                    for i in 0..(avail * 2) {
                        out_interleaved[i] += (track.stereo_samples[base + i] as f32) * scale;
                    }
                }
                *frame_cursor += take_frames;
                Ok(take_frames)
            }
            Self::PreMergedWav { decoder } => {
                let frames = decoder
                    .read_next_frames(max_frames, out_interleaved)
                    .map_err(|e| format!("Error leyendo WAV fuente en bloques: {}", e))?;
                Ok(frames)
            }
        }
    }
}

/// Transmite por bloques de `~8192` frames las fuentes activas entregando el `sample_rate`
/// real de la fuente junto a cada bloque para inicializar `StreamingAcousticAnalyzer` en una sola pasada.
pub(super) fn stream_source_audio_chunks_with_sr<F>(
    tracks: &[Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS],
    configs: &[AudioTrackSlot; MAX_AUDIO_TRACKS],
    pre_merged_wav: Option<&Path>,
    mut on_chunk: F,
) -> Result<Option<(u32, bool)>, String>
where
    F: FnMut(u32, &[f32]) -> Result<(), String>,
{
    let Some((mut stream, source_sr, used_high_precision_f32)) =
        SourceMixerStream::open(tracks, configs, pre_merged_wav)?
    else {
        return Ok(None);
    };

    let mut chunk_buf = Vec::with_capacity(STREAM_BLOCK_FRAMES * 2);
    loop {
        let frames = stream.next_chunk(STREAM_BLOCK_FRAMES, &mut chunk_buf)?;
        if frames == 0 {
            break;
        }
        on_chunk(source_sr, &chunk_buf)?;
    }

    Ok(Some((source_sr, used_high_precision_f32)))
}

/// Transmite por bloques de `~8192` frames las fuentes activas (crudos `f32`, pistas en memoria o
/// `compiled_mix.wav`) sin cargar nunca la canción completa en RAM.
pub(super) fn stream_source_audio_chunks<F>(
    tracks: &[Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS],
    configs: &[AudioTrackSlot; MAX_AUDIO_TRACKS],
    pre_merged_wav: Option<&Path>,
    mut on_chunk: F,
) -> Result<Option<(u32, bool)>, String>
where
    F: FnMut(&[f32]) -> Result<(), String>,
{
    stream_source_audio_chunks_with_sr(tracks, configs, pre_merged_wav, |_sr, chunk| {
        on_chunk(chunk)
    })
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

/// Escritor WAV estéreo de 16 bits en streaming directo hacia `.wav.tmp` con dither TPDF
/// y renombrado atómico al finalizar, manteniendo un consumo constante mínimo de RAM.
pub(super) struct StreamingWavTpdfWriter {
    writer: hound::WavWriter<BufWriter<File>>,
    tmp_path: PathBuf,
    out_path: PathBuf,
    rng_state: u64,
}

impl StreamingWavTpdfWriter {
    pub(super) fn create(out_path: &Path, sample_rate: u32) -> Result<Self, String> {
        if let Some(parent) = out_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp_path = out_path.with_extension("wav.tmp");
        let sr = sample_rate.max(8000);
        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: sr,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let file = File::create(&tmp_path)
            .map_err(|e| format!("No se pudo crear WAV adaptado temporal: {}", e))?;
        let buf_writer = BufWriter::with_capacity(64 * 1024, file);
        let writer = hound::WavWriter::new(buf_writer, spec)
            .map_err(|e| format!("Error abriendo cabecera WAV adaptado: {}", e))?;

        Ok(Self {
            writer,
            tmp_path,
            out_path: out_path.to_path_buf(),
            rng_state: 0x9E37_79B9_7F4A_7C15 ^ (sr as u64),
        })
    }

    pub(super) fn write_samples(&mut self, samples_interleaved: &[f32]) -> Result<(), String> {
        for &s in samples_interleaved {
            let clean = if s.is_finite() { s.clamp(-1.0, 1.0) } else { 0.0 };
            let tpdf = next_tpdf_lsb_unit(&mut self.rng_state)
                - next_tpdf_lsb_unit(&mut self.rng_state);
            let pcm = (clean * 32767.0 + tpdf).round().clamp(-32768.0, 32767.0) as i16;
            self.writer
                .write_sample(pcm)
                .map_err(|e| format!("Error escribiendo muestra WAV adaptada: {}", e))?;
        }
        Ok(())
    }

    pub(super) fn finalize(self) -> Result<(), String> {
        self.writer
            .finalize()
            .map_err(|e| format!("Error cerrando WAV adaptado: {}", e))?;
        if std::fs::rename(&self.tmp_path, &self.out_path).is_err() {
            std::fs::copy(&self.tmp_path, &self.out_path)
                .map_err(|e| format!("Error moviendo WAV adaptado final: {}", e))?;
            let _ = std::fs::remove_file(&self.tmp_path);
        }
        Ok(())
    }
}
