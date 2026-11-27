//! Archivo: engine/crates/pulse_audio/src/adaptive_master/analyzer.rs
//! Propósito: Analizador acústico perceptual en Rust puro usando `ebur128`
//! (LUFS integrado, rango dinámico LRA, True Peak) y `realfft` con ventana Hann.
//! Soporta análisis por bloques (`StreamingAcousticAnalyzer`) para no retener canciones
//! completas en RAM en dispositivos móviles.

use super::models::{SongAcousticAnalysis, SpectralBandProfile};
use ebur128::{EbuR128, Mode};
use realfft::{RealFftPlanner, RealToComplex};
use std::sync::Arc;

const FFT_LEN: usize = 4096;
const MAX_FFT_RESERVOIR: usize = 48;

/// Analizador acústico incremental de bajo consumo de RAM (`~32 KB` de buffers internos)
/// que acumula mediciones ITU-R BS.1770 (`ebur128`) y muestreo espectral `realfft` por chunks.
pub struct StreamingAcousticAnalyzer {
    sr: u32,
    meter: Option<EbuR128>,
    total_frames: usize,
    sum_sq: f64,
    max_abs: f64,
    dot_lr: f64,
    norm_l: f64,
    norm_r: f64,
    r2c: Arc<dyn RealToComplex<f32>>,
    hann: Vec<f32>,
    indata: Vec<f32>,
    spectrum: Vec<realfft::num_complex::Complex<f32>>,
    window_mono: Vec<f32>,
    snapshots: Vec<[f64; 6]>,
    windows_seen: usize,
}

impl StreamingAcousticAnalyzer {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(8000);
        let meter = EbuR128::new(2, sr, Mode::I | Mode::LRA | Mode::TRUE_PEAK).ok();
        let mut planner = RealFftPlanner::<f32>::new();
        let r2c = planner.plan_fft_forward(FFT_LEN);
        let indata = r2c.make_input_vec();
        let spectrum = r2c.make_output_vec();
        let hann: Vec<f32> = (0..FFT_LEN)
            .map(|i| {
                let phase = (2.0 * std::f32::consts::PI * i as f32) / ((FFT_LEN - 1) as f32);
                0.5 * (1.0 - phase.cos())
            })
            .collect();

        Self {
            sr,
            meter,
            total_frames: 0,
            sum_sq: 0.0,
            max_abs: 0.0,
            dot_lr: 0.0,
            norm_l: 0.0,
            norm_r: 0.0,
            r2c,
            hann,
            indata,
            spectrum,
            window_mono: Vec::with_capacity(FFT_LEN),
            snapshots: Vec::with_capacity(MAX_FFT_RESERVOIR),
            windows_seen: 0,
        }
    }

    pub fn push_chunk(&mut self, samples_interleaved: &[f32]) {
        let even_len = (samples_interleaved.len() / 2) * 2;
        if even_len == 0 {
            return;
        }
        let slice = &samples_interleaved[..even_len];

        if let Some(meter) = &mut self.meter {
            for sub in slice.chunks(4096 * 2) {
                let sub_even = (sub.len() / 2) * 2;
                if sub_even > 0 {
                    let _ = meter.add_frames_f32(&sub[..sub_even]);
                }
            }
        }

        for pair in slice.chunks_exact(2) {
            let l_raw = if pair[0].is_finite() { pair[0] } else { 0.0 };
            let r_raw = if pair[1].is_finite() { pair[1] } else { 0.0 };
            let l = l_raw as f64;
            let r = r_raw as f64;
            let al = l.abs();
            let ar = r.abs();
            if al > self.max_abs {
                self.max_abs = al;
            }
            if ar > self.max_abs {
                self.max_abs = ar;
            }
            self.sum_sq += 0.5 * (l * l + r * r);
            self.dot_lr += l * r;
            self.norm_l += l * l;
            self.norm_r += r * r;
            self.total_frames += 1;

            self.window_mono.push(0.5 * (l_raw + r_raw));
            if self.window_mono.len() >= FFT_LEN {
                self.evaluate_current_window();
                self.window_mono.clear();
            }
        }
    }

    fn evaluate_current_window(&mut self) {
        let should_sample = self.windows_seen < MAX_FFT_RESERVOIR || self.windows_seen % 4 == 0;
        let idx = self.windows_seen;
        self.windows_seen += 1;
        if !should_sample {
            return;
        }

        self.indata.fill(0.0);
        let copy_frames = self.window_mono.len().min(FFT_LEN);
        for i in 0..copy_frames {
            self.indata[i] = self.window_mono[i] * self.hann[i];
        }
        if self.r2c.process(&mut self.indata, &mut self.spectrum).is_ok() {
            let bin_hz = (self.sr as f64) / (FFT_LEN as f64);
            let mut sub_energy = 0.0_f64;
            let mut bass_energy = 0.0_f64;
            let mut low_mid_energy = 0.0_f64;
            let mut presence_energy = 0.0_f64;
            let mut air_energy = 0.0_f64;
            let mut total_band_energy = 0.0_f64;

            for (bin, c) in self.spectrum.iter().enumerate() {
                let freq = bin as f64 * bin_hz;
                if !(20.0..=16_000.0).contains(&freq) {
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

            let snap = [
                sub_energy,
                bass_energy,
                low_mid_energy,
                presence_energy,
                air_energy,
                total_band_energy,
            ];
            if self.snapshots.len() < MAX_FFT_RESERVOIR {
                self.snapshots.push(snap);
            } else {
                let slot = (idx.wrapping_mul(2_654_435_761)) % MAX_FFT_RESERVOIR;
                self.snapshots[slot] = snap;
            }
        }
    }

    pub fn finish(mut self) -> SongAcousticAnalysis {
        if self.total_frames == 0 {
            return SongAcousticAnalysis {
                sample_rate: self.sr,
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

        if self.snapshots.is_empty() && !self.window_mono.is_empty() {
            self.evaluate_current_window();
        }

        let mut integrated_lufs = -23.0_f32;
        let mut loudness_range_lu = 4.0_f32;
        let mut true_peak_lin = 0.0001_f64;

        if let Some(meter) = &self.meter {
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

        let rms = (self.sum_sq / self.total_frames as f64).sqrt().max(1e-9);
        let peak_for_crest = true_peak_lin.max(self.max_abs).max(rms);
        let true_peak_dbtp = (20.0 * true_peak_lin.max(self.max_abs).max(1e-9).log10()) as f32;
        let crest_factor_db = (20.0 * (peak_for_crest / rms).max(1.0).log10()) as f32;
        let corr_den = (self.norm_l * self.norm_r).sqrt();
        let stereo_correlation = if corr_den > 1e-12 {
            (self.dot_lr / corr_den).clamp(-1.0, 1.0) as f32
        } else {
            1.0
        };

        let mut sub_energy = 0.0_f64;
        let mut bass_energy = 0.0_f64;
        let mut low_mid_energy = 0.0_f64;
        let mut presence_energy = 0.0_f64;
        let mut air_energy = 0.0_f64;
        let mut total_band_energy = 0.0_f64;
        for snap in &self.snapshots {
            sub_energy += snap[0];
            bass_energy += snap[1];
            low_mid_energy += snap[2];
            presence_energy += snap[3];
            air_energy += snap[4];
            total_band_energy += snap[5];
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
            sample_rate: self.sr,
            duration_sec: self.total_frames as f32 / self.sr as f32,
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
}

/// Analiza una mezcla estéreo `f32` entrelazada con `ebur128` (LUFS / LRA / True Peak) y `realfft` (FFT 5 bandas).
pub fn analyze_song_acoustics(samples_interleaved: &[f32], sample_rate: u32) -> SongAcousticAnalysis {
    let mut analyzer = StreamingAcousticAnalyzer::new(sample_rate);
    for chunk in samples_interleaved.chunks(8192 * 2) {
        analyzer.push_chunk(chunk);
    }
    analyzer.finish()
}
