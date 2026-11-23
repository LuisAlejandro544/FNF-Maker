//! Archivo: engine/crates/pulse_audio/src/decoder.rs
//! Propósito: Decodifica archivos de audio `.ogg` (lewton), `.wav` (hound) y `.mp3` (rmp3)
//! conservando una ruta nativa de alta precisión en `f32` sin pasar por `resample_stereo_linear`:
//! - `decode_audio_file_raw_f32`: entrega PCM estéreo `f32` a su frecuencia original (para masterización
//!   acústica `adaptive_master` y medición EBU R128/FFT sin pérdida previa).
//! - `resample_stereo_f32_rubato`: remuestrea con `rubato::FftFixedIn` compensando `output_delay`.
//! - `decode_audio_file`: genera `DecodedTrack` compacto (`i16`, 50% menos RAM) para el editor y `.bfaud`.

use rubato::{FftFixedIn, Resampler};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioDecodeError {
    #[error("Error de lectura de archivo: {0}")]
    Io(#[from] std::io::Error),
    #[error("Formato no soportado (usa .ogg, .wav o .mp3)")]
    UnsupportedFormat,
    #[error("Error decodificando OGG Vorbis: {0}")]
    Ogg(String),
    #[error("Error decodificando WAV: {0}")]
    Wav(String),
    #[error("El archivo MP3 no contiene frames válidos")]
    Mp3Empty,
    #[error("Error remuestreando audio con Rubato: {0}")]
    Resample(String),
}

/// Pista decodificada en memoria compacta (`i16`) para previsualización en estudio y horneado.
#[derive(Debug, Clone)]
pub struct DecodedTrack {
    pub sample_rate: u32,
    /// Muestras estéreo intercaladas en 16 bits `[L0, R0, L1, R1, ...]` (mitad de RAM que `f32`).
    pub stereo_samples: Vec<i16>,
    /// Picos normalizados (64 barras) para dibujar la forma de onda en la UI.
    pub waveform_peaks: Vec<f32>,
}

impl DecodedTrack {
    pub fn frame_count(&self) -> usize {
        self.stereo_samples.len() / 2
    }

    pub fn duration_ms(&self) -> f64 {
        let frames = self.frame_count() as f64;
        (frames / (self.sample_rate.max(1) as f64)) * 1000.0
    }

    pub fn memory_bytes(&self) -> usize {
        self.stereo_samples.len() * std::mem::size_of::<i16>()
    }
}

/// Decodifica cualquier `.ogg`, `.wav` o `.mp3` en `f32` estéreo entrelazado a su frecuencia
/// de muestreo nativa original, evitando cuantización temprana a `i16` y remuestreo lineal.
pub fn decode_audio_file_raw_f32(path: &Path) -> Result<(u32, Vec<f32>), AudioDecodeError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "ogg" | "oga" => decode_ogg_f32(path),
        "wav" => decode_wav_f32(path),
        "mp3" => decode_mp3_f32(path),
        _ => Err(AudioDecodeError::UnsupportedFormat),
    }
}

/// Remuestrea PCM estéreo `f32` entrelazado con `rubato::FftFixedIn` compensando el retardo exacto
/// (`output_delay`) para mantener sincronización milimétrica con el chart y las notas.
pub fn resample_stereo_f32_rubato(
    interleaved_in: &[f32],
    fs_in: u32,
    fs_out: u32,
) -> Result<Vec<f32>, String> {
    let in_frames = interleaved_in.len() / 2;
    if in_frames == 0 || fs_in == fs_out || fs_in == 0 || fs_out == 0 {
        return Ok(interleaved_in.to_vec());
    }

    let chunk_size = 1024usize;
    let sub_chunks = 2usize;
    let mut resampler =
        FftFixedIn::<f32>::new(fs_in as usize, fs_out as usize, chunk_size, sub_chunks, 2)
            .map_err(|e| format!("Error creando remuestreador Rubato: {}", e))?;

    let delay_frames = resampler.output_delay();
    let expected_out_frames =
        ((in_frames as f64) * (fs_out as f64) / (fs_in as f64)).round() as usize;

    let mut left_all = Vec::with_capacity(expected_out_frames + chunk_size * 2);
    let mut right_all = Vec::with_capacity(expected_out_frames + chunk_size * 2);

    let step = resampler.input_frames_next().max(1);
    let mut chunk_l = vec![0.0_f32; step];
    let mut chunk_r = vec![0.0_f32; step];

    let mut frame_cursor = 0usize;
    // Añadir frames de cola para drenar completamente `output_delay` al final de la canción.
    let flush_frames =
        ((delay_frames as f64 * (fs_in as f64 / fs_out as f64)).ceil() as usize) + step;
    let total_feed_frames = in_frames + flush_frames;

    while frame_cursor < total_feed_frames {
        chunk_l.fill(0.0);
        chunk_r.fill(0.0);
        for i in 0..step {
            let src_idx = frame_cursor + i;
            if src_idx < in_frames {
                let base = src_idx * 2;
                chunk_l[i] = interleaved_in[base];
                chunk_r[i] = interleaved_in[base + 1];
            }
        }
        let out_chunks = resampler
            .process(&[chunk_l.as_slice(), chunk_r.as_slice()], None)
            .map_err(|e| format!("Error procesando remuestreo Rubato: {}", e))?;
        if out_chunks.len() >= 2 {
            left_all.extend_from_slice(&out_chunks[0]);
            right_all.extend_from_slice(&out_chunks[1]);
        }
        frame_cursor += step;
    }

    let start_trim = delay_frames.min(left_all.len());
    let avail = left_all
        .len()
        .saturating_sub(start_trim)
        .min(expected_out_frames);
    let mut interleaved_out = Vec::with_capacity(expected_out_frames * 2);
    for i in 0..avail {
        let idx = start_trim + i;
        interleaved_out.push(left_all[idx]);
        interleaved_out.push(right_all[idx]);
    }
    while interleaved_out.len() < expected_out_frames * 2 {
        interleaved_out.push(0.0);
        interleaved_out.push(0.0);
    }
    Ok(interleaved_out)
}

/// Decodifica cualquier archivo `.ogg`, `.wav` o `.mp3` en `f32`, lo remuestrea con Rubato FFT
/// hacia `target_sample_rate` y genera el `DecodedTrack` compacto (`i16`) para el editor.
pub fn decode_audio_file(
    path: &Path,
    target_sample_rate: u32,
) -> Result<DecodedTrack, AudioDecodeError> {
    let target_sr = target_sample_rate.max(8000);
    let (src_rate, raw_f32) = decode_audio_file_raw_f32(path)?;

    let resampled_f32 = if src_rate == target_sr {
        raw_f32
    } else {
        resample_stereo_f32_rubato(&raw_f32, src_rate, target_sr)
            .map_err(AudioDecodeError::Resample)?
    };

    let mut stereo_samples = Vec::with_capacity(resampled_f32.len());
    for &s in &resampled_f32 {
        let q = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        stereo_samples.push(q);
    }

    let waveform_peaks = compute_waveform_peaks(&stereo_samples, 64);

    Ok(DecodedTrack {
        sample_rate: target_sr,
        stereo_samples,
        waveform_peaks,
    })
}

fn decode_ogg_f32(path: &Path) -> Result<(u32, Vec<f32>), AudioDecodeError> {
    let file = File::open(path)?;
    let mut reader = lewton::inside_ogg::OggStreamReader::new(BufReader::new(file))
        .map_err(|e| AudioDecodeError::Ogg(e.to_string()))?;
    let rate = reader.ident_hdr.audio_sample_rate.max(8000);
    let channels = reader.ident_hdr.audio_channels as usize;
    let mut out = Vec::new();

    while let Some(packet) = reader
        .read_dec_packet_generic::<Vec<Vec<f32>>>()
        .map_err(|e| AudioDecodeError::Ogg(e.to_string()))?
    {
        let ch_count = packet.len();
        if ch_count == 0 {
            continue;
        }
        let frame_len = packet[0].len();
        if frame_len == 0 {
            continue;
        }
        out.reserve(frame_len * 2);
        if ch_count == 1 || channels <= 1 {
            for &s in &packet[0] {
                let clean = if s.is_finite() { s } else { 0.0 };
                out.push(clean);
                out.push(clean);
            }
        } else {
            let r_ch = &packet[1];
            for i in 0..frame_len {
                let l = packet[0][i];
                let r = r_ch.get(i).copied().unwrap_or(l);
                out.push(if l.is_finite() { l } else { 0.0 });
                out.push(if r.is_finite() { r } else { 0.0 });
            }
        }
    }

    Ok((rate, out))
}

fn decode_wav_f32(path: &Path) -> Result<(u32, Vec<f32>), AudioDecodeError> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| AudioDecodeError::Wav(e.to_string()))?;
    let spec = reader.spec();
    let sr = spec.sample_rate.max(8000);
    let channels = (spec.channels as usize).max(1);
    let mut mono_or_multi: Vec<f32> = Vec::new();

    match spec.sample_format {
        hound::SampleFormat::Float => {
            for s in reader.samples::<f32>() {
                let f = s.map_err(|e| AudioDecodeError::Wav(e.to_string()))?;
                mono_or_multi.push(if f.is_finite() { f } else { 0.0 });
            }
        }
        hound::SampleFormat::Int => {
            if spec.bits_per_sample == 16 {
                for s in reader.samples::<i16>() {
                    let v = s.map_err(|e| AudioDecodeError::Wav(e.to_string()))?;
                    mono_or_multi.push((v as f32) / 32768.0);
                }
            } else {
                let denom = (1_i64 << spec.bits_per_sample.saturating_sub(1).max(1)) as f64;
                for s in reader.samples::<i32>() {
                    let v = s.map_err(|e| AudioDecodeError::Wav(e.to_string()))?;
                    mono_or_multi.push(((v as f64) / denom) as f32);
                }
            }
        }
    }

    let mut stereo = Vec::with_capacity((mono_or_multi.len() / channels) * 2);
    if channels == 1 {
        for s in mono_or_multi {
            stereo.push(s);
            stereo.push(s);
        }
    } else {
        for chunk in mono_or_multi.chunks_exact(channels) {
            stereo.push(chunk[0]);
            stereo.push(chunk[1]);
        }
    }

    Ok((sr, stereo))
}

fn decode_mp3_f32(path: &Path) -> Result<(u32, Vec<f32>), AudioDecodeError> {
    let bytes = std::fs::read(path)?;
    let mut decoder = rmp3::Decoder::new(&bytes);
    let mut out = Vec::new();
    let mut detected_rate = 44100_u32;

    while let Some(frame) = decoder.next() {
        if let rmp3::Frame::Audio(audio) = frame {
            detected_rate = audio.sample_rate().max(8000);
            let ch = audio.channels() as usize;
            let samples: &[f32] = audio.samples();
            if ch <= 1 {
                out.reserve(samples.len() * 2);
                for &s in samples {
                    let clean = if s.is_finite() { s } else { 0.0 };
                    out.push(clean);
                    out.push(clean);
                }
            } else {
                out.reserve((samples.len() / ch) * 2);
                for pair in samples.chunks_exact(ch) {
                    let l = if pair[0].is_finite() { pair[0] } else { 0.0 };
                    let r = if pair[1].is_finite() { pair[1] } else { 0.0 };
                    out.push(l);
                    out.push(r);
                }
            }
        }
    }

    if out.is_empty() {
        return Err(AudioDecodeError::Mp3Empty);
    }

    Ok((detected_rate, out))
}

fn compute_waveform_peaks(stereo: &[i16], bar_count: usize) -> Vec<f32> {
    let frames = stereo.len() / 2;
    if frames == 0 || bar_count == 0 {
        return vec![0.0; bar_count];
    }
    let step = (frames / bar_count).max(1);
    let mut peaks = Vec::with_capacity(bar_count);
    for b in 0..bar_count {
        let start = b * step;
        let end = ((b + 1) * step).min(frames);
        let mut max_amp = 0_i32;
        for f in start..end {
            let l = (stereo[f * 2] as i32).abs();
            let r = (stereo[f * 2 + 1] as i32).abs();
            let amp = l.max(r);
            if amp > max_amp {
                max_amp = amp;
            }
        }
        peaks.push(((max_amp as f32) / 32768.0).clamp(0.0, 1.0));
    }
    peaks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_f32_wav_preserves_native_sample_rate_and_24bit_precision() {
        let tmp = std::env::temp_dir().join("beatforge_raw_f32_24bit_test.wav");
        let _ = std::fs::remove_file(&tmp);

        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 44_100,
            bits_per_sample: 24,
            sample_format: hound::SampleFormat::Int,
        };
        {
            let mut writer = hound::WavWriter::create(&tmp, spec).unwrap();
            // Señal sutil por debajo de 1 LSB de 16-bit (~1/65536 = 0.000015) que se perdería en i16
            let subtle_24bit = 128_i32; // 128 / 8388608 ~= 1.525e-5
            for _ in 0..512 {
                writer.write_sample(subtle_24bit).unwrap();
                writer.write_sample(-subtle_24bit).unwrap();
            }
            writer.finalize().unwrap();
        }

        let (sr, samples) = decode_audio_file_raw_f32(&tmp).unwrap();
        assert_eq!(sr, 44_100);
        assert_eq!(samples.len(), 1024);
        assert!(samples[0] > 1.0e-5 && samples[0] < 2.0e-5);
        assert!((samples[0] + samples[1]).abs() < 1e-7);

        let resampled = resample_stereo_f32_rubato(&samples, 44_100, 48_000).unwrap();
        let expected_frames = ((512.0_f64 * 48_000.0) / 44_100.0).round() as usize;
        assert_eq!(resampled.len() / 2, expected_frames);

        let _ = std::fs::remove_file(&tmp);
    }
}
