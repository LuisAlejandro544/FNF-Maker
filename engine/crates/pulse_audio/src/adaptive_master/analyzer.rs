//! Archivo: engine/crates/pulse_audio/src/adaptive_master/analyzer.rs
//! Propósito: Analizador acústico perceptual en Rust puro usando `ebur128`
//! (LUFS integrado, rango dinámico LRA, True Peak) y `realfft` con ventana Hann.

use super::models::{SongAcousticAnalysis, SpectralBandProfile};
use ebur128::{EbuR128, Mode};
use realfft::RealFftPlanner;

/// Analiza una mezcla estéreo `f32` entrelazada con `ebur128` (LUFS / LRA / True Peak) y `realfft` (FFT 5 bandas).
pub fn analyze_song_acoustics(samples_interleaved: &[f32], sample_rate: u32) -> SongAcousticAnalysis {
    let sr = sample_rate.max(8000);
    let total_frames = samples_interleaved.len() / 2;
    if total_frames == 0 {
        return SongAcousticAnalysis {
            sample_rate: sr,
            duration_sec: 0.0,
            integrated_lufs: -23.0,
            loudness_range_lu: 0.0,
            true_peak_dbtp: -90.0,
            crest_factor_db: 0.0,
            stereo_correlation: 1.0,
            bands: SpectralBandProfile {
                sub_bass_20_90_hz: 0.2,
                bass_90_220_hz: 0.2,
                low_mid_220_600_hz: 0.2,
                presence_2k_4k_hz: 0.2,
                air_6k_14k_hz: 0.2,
                spectral_tilt_db: 0.0,
            },
        };
    }

    // 1. Medición perceptual ITU-R BS.1770 / EBU R128 mediante crate `ebur128`
    let mut integrated_lufs = -23.0_f32;
    let mut loudness_range_lu = 4.0_f32;
    let mut true_peak_lin = 0.0001_f64;

    if let Ok(mut meter) = EbuR128::new(2, sr, Mode::I | Mode::LRA | Mode::TRUE_PEAK) {
        let chunk_floats = 4096 * 2;
        for chunk in samples_interleaved.chunks(chunk_floats) {
            let even_len = (chunk.len() / 2) * 2;
            if even_len > 0 {
                let _ = meter.add_frames_f32(&chunk[..even_len]);
            }
        }
        if let Ok(g) = meter.loudness_global() {
            if g.is_finite() && g > -70.0 {
                integrated_lufs = g as f32;
            }
        }
        if let Ok(lra) = meter.loudness_range() {
            if lra.is_finite() && lra >= 0.0 {
                loudness_range_lu = lra as f32;
            }
        }
        for ch in 0..2 {
            if let Ok(tp) = meter.true_peak(ch) {
                if tp.is_finite() && tp > true_peak_lin {
                    true_peak_lin = tp;
                }
            }
        }
    }

    // RMS, factor de cresta y correlación estéreo L/R reales
    let mut sum_sq = 0.0_f64;
    let mut max_abs = 0.0_f64;
    let mut dot_lr = 0.0_f64;
    let mut norm_l = 0.0_f64;
    let mut norm_r = 0.0_f64;
    for pair in samples_interleaved.chunks_exact(2) {
        let l = pair[0] as f64;
        let r = pair[1] as f64;
        let al = l.abs();
        let ar = r.abs();
        if al > max_abs {
            max_abs = al;
        }
        if ar > max_abs {
            max_abs = ar;
        }
        sum_sq += 0.5 * (l * l + r * r);
        dot_lr += l * r;
        norm_l += l * l;
        norm_r += r * r;
    }
    let rms = (sum_sq / total_frames as f64).sqrt().max(1e-9);
    let peak_for_crest = true_peak_lin.max(max_abs).max(rms);
    let true_peak_dbtp = (20.0 * true_peak_lin.max(max_abs).max(1e-9).log10()) as f32;
    let crest_factor_db = (20.0 * (peak_for_crest / rms).max(1.0).log10()) as f32;
    let corr_den = (norm_l * norm_r).sqrt();
    let stereo_correlation = if corr_den > 1e-12 {
        (dot_lr / corr_den).clamp(-1.0, 1.0) as f32
    } else {
        1.0
    };

    // 2. Análisis espectral real mediante `realfft` + ventana Hann distribuida por la canción
    let fft_len = 4096usize;
    let mut planner = RealFftPlanner::<f32>::new();
    let r2c = planner.plan_fft_forward(fft_len);
    let mut indata = r2c.make_input_vec();
    let mut spectrum = r2c.make_output_vec();

    let hann: Vec<f32> = (0..fft_len)
        .map(|i| {
            let phase = (2.0 * std::f32::consts::PI * i as f32) / ((fft_len - 1) as f32);
            0.5 * (1.0 - phase.cos())
        })
        .collect();

    let mut sub_energy = 0.0_f64;
    let mut bass_energy = 0.0_f64;
    let mut low_mid_energy = 0.0_f64;
    let mut presence_energy = 0.0_f64;
    let mut air_energy = 0.0_f64;
    let mut total_band_energy = 0.0_f64;

    let window_count = if total_frames <= fft_len {
        1
    } else {
        ((total_frames / fft_len).min(48)).max(1)
    };
    let stride = if window_count <= 1 || total_frames <= fft_len {
        0
    } else {
        (total_frames - fft_len) / (window_count - 1)
    };
    let bin_hz = (sr as f64) / (fft_len as f64);

    for w in 0..window_count {
        let start_frame = w * stride;
        indata.fill(0.0);
        let copy_frames = (total_frames - start_frame).min(fft_len);
        for i in 0..copy_frames {
            let base = (start_frame + i) * 2;
            let mono = 0.5 * (samples_interleaved[base] + samples_interleaved[base + 1]);
            indata[i] = mono * hann[i];
        }
        if r2c.process(&mut indata, &mut spectrum).is_ok() {
            for (bin, c) in spectrum.iter().enumerate() {
                let freq = bin as f64 * bin_hz;
                if freq < 20.0 || freq > 16_000.0 {
                    continue;
                }
                let p = (c.re as f64) * (c.re as f64) + (c.im as f64) * (c.im as f64);
                total_band_energy += p;
                if freq < 90.0 {
                    sub_energy += p;
                } else if freq < 220.0 {
                    bass_energy += p;
                } else if freq < 600.0 {
                    low_mid_energy += p;
                } else if (2000.0..=4000.0).contains(&freq) {
                    presence_energy += p;
                } else if (6000.0..=14_000.0).contains(&freq) {
                    air_energy += p;
                }
            }
        }
    }

    let denom = total_band_energy.max(1e-12);
    let sub_r = (sub_energy / denom).clamp(0.0, 1.0) as f32;
    let bass_r = (bass_energy / denom).clamp(0.0, 1.0) as f32;
    let low_mid_r = (low_mid_energy / denom).clamp(0.0, 1.0) as f32;
    let pres_r = (presence_energy / denom).clamp(0.0, 1.0) as f32;
    let air_r = (air_energy / denom).clamp(0.0, 1.0) as f32;

    let lows = (sub_energy + bass_energy).max(1e-12);
    let highs = (presence_energy + air_energy).max(1e-12);
    let spectral_tilt_db = (10.0 * (highs / lows).log10()).clamp(-24.0, 24.0) as f32;

    SongAcousticAnalysis {
        sample_rate: sr,
        duration_sec: total_frames as f32 / sr as f32,
        integrated_lufs,
        loudness_range_lu,
        true_peak_dbtp,
        crest_factor_db,
        stereo_correlation,
        bands: SpectralBandProfile {
            sub_bass_20_90_hz: sub_r,
            bass_90_220_hz: bass_r,
            low_mid_220_600_hz: low_mid_r,
            presence_2k_4k_hz: pres_r,
            air_6k_14k_hz: air_r,
            spectral_tilt_db,
        },
    }
}
