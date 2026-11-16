//! Archivo: engine/crates/pulse_audio/src/decoder.rs
//! Propósito: Decodifica archivos de audio `.ogg` (lewton), `.wav` (hound) y `.mp3` (rmp3)
//! a PCM estéreo flotante (`Vec<f32>` intercalado L/R) y los remuestrea automáticamente
//! a la frecuencia nativa de salida de Oboe (típicamente 48000 Hz) para mantener
//! las hasta 3 pistas perfectamente sincronizadas sin deriva.

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

/// Pista decodificada en memoria lista para mezclar en tiempo real por el hilo de Oboe.
#[derive(Debug, Clone)]
pub struct DecodedTrack {
    pub sample_rate: u32,
    /// Muestras estéreo intercaladas `[L0, R0, L1, R1, ...]`.
    pub stereo_samples: Vec<f32>,
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

fn decode_ogg(path: &Path) -> Result<(u32, Vec<f32>), AudioDecodeError> {
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
        if channels == 1 {
            for s in packet {
                let f = (s as f32) / 32768.0;
                out.push(f);
                out.push(f);
            }
        } else {
            for chunk in packet.chunks_exact(channels) {
                out.push((chunk[0] as f32) / 32768.0);
                out.push((chunk[1] as f32) / 32768.0);
            }
        }
    }

    Ok((rate, out))
}

fn decode_wav(path: &Path) -> Result<(u32, Vec<f32>), AudioDecodeError> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| AudioDecodeError::Wav(e.to_string()))?;
    let spec = reader.spec();
    let channels = spec.channels as usize;
    let mut mono_or_multi: Vec<f32> = Vec::new();

    match spec.sample_format {
        hound::SampleFormat::Float => {
            for s in reader.samples::<f32>() {
                mono_or_multi.push(s.map_err(|e| AudioDecodeError::Wav(e.to_string()))?);
            }
        }
        hound::SampleFormat::Int => {
            let max_val = ((1_i64 << (spec.bits_per_sample.saturating_sub(1))) as f32).max(1.0);
            for s in reader.samples::<i32>() {
                let v = s.map_err(|e| AudioDecodeError::Wav(e.to_string()))?;
                mono_or_multi.push((v as f32) / max_val);
            }
        }
    }

    let mut stereo = Vec::with_capacity((mono_or_multi.len() / channels.max(1)) * 2);
    if channels <= 1 {
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

fn decode_mp3(path: &Path) -> Result<(u32, Vec<f32>), AudioDecodeError> {
    let bytes = std::fs::read(path)?;
    let mut decoder = rmp3::Decoder::new(&bytes);
    let mut out = Vec::new();
    let mut detected_rate = 44100_u32;

    while let Some(frame) = decoder.next() {
        if let rmp3::Frame::Audio(audio) = frame {
            detected_rate = audio.sample_rate();
            let ch = audio.channels() as usize;
            let samples = audio.samples();
            if ch == 1 {
                for &s in samples {
                    let f = (s as f32) / 32768.0;
                    out.push(f);
                    out.push(f);
                }
            } else {
                for pair in samples.chunks_exact(ch) {
                    out.push((pair[0] as f32) / 32768.0);
                    out.push((pair[1] as f32) / 32768.0);
                }
            }
        }
    }

    if out.is_empty() {
        return Err(AudioDecodeError::Mp3Empty);
    }

    Ok((detected_rate, out))
}

/// Remuestreo lineal estéreo rápido para unificar las 3 pistas al sample rate del hardware.
fn resample_stereo_linear(src: &[f32], src_rate: u32, dst_rate: u32) -> Vec<f32> {
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

        let l = src[i0 * 2] + (src[i1 * 2] - src[i0 * 2]) * frac;
        let r = src[i0 * 2 + 1] + (src[i1 * 2 + 1] - src[i0 * 2 + 1]) * frac;
        out.push(l);
        out.push(r);
    }

    out
}

fn compute_waveform_peaks(stereo: &[f32], bar_count: usize) -> Vec<f32> {
    let frames = stereo.len() / 2;
    if frames == 0 || bar_count == 0 {
        return vec![0.0; bar_count];
    }
    let step = (frames / bar_count).max(1);
    let mut peaks = Vec::with_capacity(bar_count);
    for b in 0..bar_count {
        let start = b * step;
        let end = ((b + 1) * step).min(frames);
        let mut max_amp = 0.0_f32;
        for f in start..end {
            let amp = stereo[f * 2].abs().max(stereo[f * 2 + 1].abs());
            if amp > max_amp {
                max_amp = amp;
            }
        }
        peaks.push(max_amp.clamp(0.0, 1.0));
    }
    peaks
}
