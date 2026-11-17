//! Archivo: engine/crates/pulse_audio/src/decoder.rs
//! Propósito: Decodifica archivos de audio `.ogg` (lewton), `.wav` (hound) y `.mp3` (rmp3)
//! a PCM estéreo compacto de 16 bits (`Vec<i16>` intercalado L/R, 50% menos RAM que `f32`)
//! y los remuestrea automáticamente a la frecuencia nativa de salida (típicamente 48000 Hz)
//! para mantener las hasta 3 pistas sincronizadas sin deriva en teléfonos 32 y 64 bits.

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

/// Decodifica cualquier archivo `.ogg`, `.wav` o `.mp3` y lo remuestrea a `target_sample_rate`.
pub fn decode_audio_file(
    path: &Path,
    target_sample_rate: u32,
) -> Result<DecodedTrack, AudioDecodeError> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let (src_rate, raw_stereo) = match ext.as_str() {
        "ogg" | "oga" => decode_ogg(path)?,
        "wav" => decode_wav(path)?,
        "mp3" => decode_mp3(path)?,
        _ => return Err(AudioDecodeError::UnsupportedFormat),
    };

    let stereo_samples = if src_rate == target_sample_rate {
        raw_stereo
    } else {
        resample_stereo_linear(&raw_stereo, src_rate, target_sample_rate)
    };

    let waveform_peaks = compute_waveform_peaks(&stereo_samples, 64);

    Ok(DecodedTrack {
        sample_rate: target_sample_rate,
        stereo_samples,
        waveform_peaks,
    })
}

fn decode_ogg(path: &Path) -> Result<(u32, Vec<i16>), AudioDecodeError> {
    let file = File::open(path)?;
    let mut reader = lewton::inside_ogg::OggStreamReader::new(BufReader::new(file))
        .map_err(|e| AudioDecodeError::Ogg(e.to_string()))?;
    let rate = reader.ident_hdr.audio_sample_rate;
    let channels = reader.ident_hdr.audio_channels as usize;
    let mut out = Vec::new();

    while let Some(packet) = reader
        .read_dec_packet_itl()
        .map_err(|e| AudioDecodeError::Ogg(e.to_string()))?
    {
        if channels <= 1 {
            out.reserve(packet.len() * 2);
            for s in packet {
                out.push(s);
                out.push(s);
            }
        } else {
            out.reserve((packet.len() / channels) * 2);
            for chunk in packet.chunks_exact(channels) {
                out.push(chunk[0]);
                out.push(chunk[1]);
            }
        }
    }

    Ok((rate, out))
}

fn decode_wav(path: &Path) -> Result<(u32, Vec<i16>), AudioDecodeError> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| AudioDecodeError::Wav(e.to_string()))?;
    let spec = reader.spec();
    let channels = (spec.channels as usize).max(1);
    let mut mono_or_multi: Vec<i16> = Vec::new();

    match spec.sample_format {
        hound::SampleFormat::Float => {
            for s in reader.samples::<f32>() {
                let f = s.map_err(|e| AudioDecodeError::Wav(e.to_string()))?;
                let q = (f.clamp(-1.0, 1.0) * 32767.0).round() as i16;
                mono_or_multi.push(q);
            }
        }
        hound::SampleFormat::Int => {
            if spec.bits_per_sample == 16 {
                for s in reader.samples::<i16>() {
                    mono_or_multi.push(s.map_err(|e| AudioDecodeError::Wav(e.to_string()))?);
                }
            } else {
                let max_val = ((1_i64 << (spec.bits_per_sample.saturating_sub(1))) as f32).max(1.0);
                for s in reader.samples::<i32>() {
                    let v = s.map_err(|e| AudioDecodeError::Wav(e.to_string()))?;
                    let norm = ((v as f32) / max_val).clamp(-1.0, 1.0);
                    mono_or_multi.push((norm * 32767.0).round() as i16);
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

    Ok((spec.sample_rate, stereo))
}

fn decode_mp3(path: &Path) -> Result<(u32, Vec<i16>), AudioDecodeError> {
    let bytes = std::fs::read(path)?;
    let mut decoder = rmp3::Decoder::new(&bytes);
    let mut out = Vec::new();
    let mut detected_rate = 44100_u32;

    while let Some(frame) = decoder.next() {
        if let rmp3::Frame::Audio(audio) = frame {
            detected_rate = audio.sample_rate();
            let ch = audio.channels() as usize;
            let samples = audio.samples();
            if ch <= 1 {
                out.reserve(samples.len() * 2);
                for &s in samples {
                    out.push(s);
                    out.push(s);
                }
            } else {
                out.reserve((samples.len() / ch) * 2);
                for pair in samples.chunks_exact(ch) {
                    out.push(pair[0]);
                    out.push(pair[1]);
                }
            }
        }
    }

    if out.is_empty() {
        return Err(AudioDecodeError::Mp3Empty);
    }

    Ok((detected_rate, out))
}

/// Remuestreo lineal estéreo rápido en `i16` para unificar las 3 pistas al sample rate del hardware.
fn resample_stereo_linear(src: &[i16], src_rate: u32, dst_rate: u32) -> Vec<i16> {
    let src_frames = src.len() / 2;
    if src_frames < 2 || src_rate == 0 || dst_rate == 0 {
        return src.to_vec();
    }
    let ratio = (src_rate as f64) / (dst_rate as f64);
    let dst_frames = (((src_frames as f64) / ratio).ceil() as usize).max(1);
    let mut out = Vec::with_capacity(dst_frames * 2);

    for i in 0..dst_frames {
        let pos = (i as f64) * ratio;
        let idx = pos.floor() as usize;
        let frac = (pos - (idx as f64)) as f32;
        let i0 = idx.min(src_frames - 1);
        let i1 = (idx + 1).min(src_frames - 1);

        let l0 = src[i0 * 2] as f32;
        let l1 = src[i1 * 2] as f32;
        let r0 = src[i0 * 2 + 1] as f32;
        let r1 = src[i1 * 2 + 1] as f32;

        let l = (l0 + (l1 - l0) * frac).round().clamp(-32768.0, 32767.0) as i16;
        let r = (r0 + (r1 - r0) * frac).round().clamp(-32768.0, 32767.0) as i16;
        out.push(l);
        out.push(r);
    }

    out
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
