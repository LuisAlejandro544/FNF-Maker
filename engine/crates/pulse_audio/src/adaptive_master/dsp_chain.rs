//! Archivo: engine/crates/pulse_audio/src/adaptive_master/dsp_chain.rs
//! Propósito: Cadena DSP de masterización adaptativa en punto flotante (`rubato`,
//! filtros IIR `biquad`, Mid/Side, crossfeed psicoacústico y limitador Lookahead).
//! Provee `AdaptiveDspProcessor` por bloques pequeños (streaming) para procesar canciones
//! completas sin picos de memoria en dispositivos móviles de 32 bits.

use super::models::AdaptiveDspPlan;
use crate::decoder::{StreamingStereoResampler, resample_stereo_f32_rubato};
use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Type};
use std::collections::VecDeque;

/// Remuestrea PCM estéreo entrelazado con `rubato::FftFixedIn` compensando el retardo exacto
/// (`output_delay`) para mantener sincronización milimétrica con el chart y las notas.
pub fn resample_stereo_with_rubato(
    interleaved_in: &[f32],
    fs_in: u32,
    fs_out: u32,
) -> Result<Vec<f32>, String> {
    resample_stereo_f32_rubato(interleaved_in, fs_in, fs_out)
}

/// Procesador DSP con estado por bloques (`rubato` incremental + IIR `biquad` + Mid/Side
/// + crossfeed psicoacústico ligero + nivelación LUFS + limitador Lookahead ~4 ms).
pub struct AdaptiveDspProcessor {
    resampler: Option<StreamingStereoResampler>,
    filters_l: Vec<DirectForm2Transposed<f32>>,
    filters_r: Vec<DirectForm2Transposed<f32>>,
    mono_fold_enabled: bool,
    width: f32,
    crossfeed: f32,
    alpha_lp: f32,
    delay_l: Vec<f32>,
    delay_r: Vec<f32>,
    delay_idx: usize,
    lp_l: f32,
    lp_r: f32,
    makeup_lin: f32,
    ceiling_lin: f32,
    lookahead_frames: usize,
    release_coeff: f32,
    ring_l: Vec<f32>,
    ring_r: Vec<f32>,
    ring_req: Vec<f32>,
    ring_pos: usize,
    below_unity_count: usize,
    min_deque: VecDeque<(u64, f32)>,
    step_idx: u64,
    env_gain: f32,
    max_out_abs: f32,
    scratch_resampled: Vec<f32>,
    has_seen_input: bool,
}

impl AdaptiveDspProcessor {
    pub fn new(plan: &AdaptiveDspPlan) -> Result<Self, String> {
        let resampler = if plan.resample_needed && plan.source_sample_rate != plan.target_sample_rate
        {
            Some(StreamingStereoResampler::new(
                plan.source_sample_rate,
                plan.target_sample_rate,
            )?)
        } else {
            None
        };

        let target_sr = plan.target_sample_rate.max(8000);
        let fs_f32 = target_sr as f32;
        let fs_hz = fs_f32.hz();

        let mut filters_l: Vec<DirectForm2Transposed<f32>> =
            Vec::with_capacity(plan.eq_bands.len());
        let mut filters_r: Vec<DirectForm2Transposed<f32>> =
            Vec::with_capacity(plan.eq_bands.len());
        for band in &plan.eq_bands {
            let max_f = (fs_f32 * 0.45).max(100.0);
            let f0 = band.freq_hz.clamp(20.0, max_f).hz();
            let q = band.q.clamp(0.25, 4.0);
            let filter_type = match band.kind.as_str() {
                "high_pass" => Type::HighPass,
                "low_shelf" => Type::LowShelf(band.gain_db.clamp(-12.0, 12.0)),
                "high_shelf" => Type::HighShelf(band.gain_db.clamp(-12.0, 12.0)),
                _ => Type::PeakingEQ(band.gain_db.clamp(-12.0, 12.0)),
            };
            if let Ok(coeffs) = Coefficients::<f32>::from_params(filter_type, fs_hz, f0, q) {
                filters_l.push(DirectForm2Transposed::<f32>::new(coeffs));
                filters_r.push(DirectForm2Transposed::<f32>::new(coeffs));
            }
        }

        let width = if plan.mono_fold_enabled {
            0.0_f32
        } else {
            plan.stereo_width.clamp(0.0, 1.35)
        };
        let crossfeed = if plan.mono_fold_enabled || width <= 0.001 {
            0.0_f32
        } else {
            plan.crossfeed_strength.clamp(0.0, 0.35)
        };
        let alpha_lp = 1.0_f32 - (-2.0_f32 * std::f32::consts::PI * 700.0_f32 / fs_f32).exp();
        let delay_frames = ((0.00025_f32 * fs_f32).round() as usize).clamp(1, 64);

        let makeup_lin = 10.0_f32.powf(plan.applied_makeup_gain_db / 20.0);
        let ceiling_lin = 10.0_f32.powf(plan.ceiling_dbtp.min(-0.3) / 20.0);
        let lookahead_sec = plan.limiter_lookahead_ms.clamp(0.0, 10.0) * 0.001;
        let lookahead_frames = if lookahead_sec > 0.0 {
            ((lookahead_sec * fs_f32).round() as usize).clamp(16, 512)
        } else {
            1
        };
        let release_coeff = (-1.0_f32 / (0.065 * fs_f32)).exp();

        Ok(Self {
            resampler,
            filters_l,
            filters_r,
            mono_fold_enabled: plan.mono_fold_enabled,
            width,
            crossfeed,
            alpha_lp,
            delay_l: vec![0.0_f32; delay_frames],
            delay_r: vec![0.0_f32; delay_frames],
            delay_idx: 0,
            lp_l: 0.0,
            lp_r: 0.0,
            makeup_lin,
            ceiling_lin,
            lookahead_frames,
            release_coeff,
            ring_l: vec![0.0_f32; lookahead_frames],
            ring_r: vec![0.0_f32; lookahead_frames],
            ring_req: vec![1.0_f32; lookahead_frames],
            ring_pos: 0,
            below_unity_count: 0,
            min_deque: VecDeque::with_capacity(lookahead_frames + 1),
            step_idx: 0,
            env_gain: 1.0,
            max_out_abs: 1e-9,
            scratch_resampled: Vec::with_capacity(16_384),
            has_seen_input: false,
        })
    }

    pub fn process_chunk(
        &mut self,
        interleaved_in: &[f32],
        out: &mut Vec<f32>,
    ) -> Result<(), String> {
        out.clear();
        if interleaved_in.len() < 2 {
            return Ok(());
        }
        self.has_seen_input = true;

        if let Some(resampler) = &mut self.resampler {
            self.scratch_resampled.clear();
            resampler.push_interleaved(interleaved_in, &mut self.scratch_resampled)?;
            let mut tmp = Vec::new();
            std::mem::swap(&mut tmp, &mut self.scratch_resampled);
            self.process_target_rate_frames(&tmp, out);
            std::mem::swap(&mut tmp, &mut self.scratch_resampled);
        } else {
            self.process_target_rate_frames(interleaved_in, out);
        }
        Ok(())
    }

    pub fn finish(&mut self, out: &mut Vec<f32>) -> Result<f32, String> {
        out.clear();
        if !self.has_seen_input {
            return Ok(-90.0);
        }

        if let Some(mut resampler) = self.resampler.take() {
            self.scratch_resampled.clear();
            resampler.finish(&mut self.scratch_resampled)?;
            let mut tmp = Vec::new();
            std::mem::swap(&mut tmp, &mut self.scratch_resampled);
            self.process_target_rate_frames(&tmp, out);
            std::mem::swap(&mut tmp, &mut self.scratch_resampled);
        }

        // Drenar los últimos `lookahead_frames` almacenados en la línea de retardo del limitador.
        for _ in 0..self.lookahead_frames {
            self.step_limiter(0.0, 0.0, 1.0, out);
        }

        Ok(20.0 * self.max_out_abs.max(1e-9).log10())
    }

    fn process_target_rate_frames(&mut self, interleaved: &[f32], out: &mut Vec<f32>) {
        let delay_frames = self.delay_l.len().max(1);
        for pair in interleaved.chunks_exact(2) {
            let mut l = if pair[0].is_finite() { pair[0] } else { 0.0 };
            let mut r = if pair[1].is_finite() { pair[1] } else { 0.0 };
            for f in &mut self.filters_l {
                l = f.run(l);
            }
            for f in &mut self.filters_r {
                r = f.run(r);
            }

            if self.mono_fold_enabled || self.width <= 0.001 {
                let mono = 0.5 * (l + r);
                l = mono;
                r = mono;
            } else {
                if (self.width - 1.0).abs() > 0.001 {
                    let mid = 0.5 * (l + r);
                    let side = 0.5 * (l - r) * self.width;
                    l = mid + side;
                    r = mid - side;
                }
                if self.crossfeed > 0.001 {
                    self.lp_l += self.alpha_lp * (l - self.lp_l);
                    self.lp_r += self.alpha_lp * (r - self.lp_r);
                    let delayed_lp_l = self.delay_l[self.delay_idx];
                    let delayed_lp_r = self.delay_r[self.delay_idx];
                    self.delay_l[self.delay_idx] = self.lp_l;
                    self.delay_r[self.delay_idx] = self.lp_r;
                    self.delay_idx = (self.delay_idx + 1) % delay_frames;

                    let k = 0.5 * self.crossfeed;
                    let high_l = l - self.lp_l;
                    let high_r = r - self.lp_r;
                    l = high_l + (1.0 - k) * self.lp_l + k * delayed_lp_r;
                    r = high_r + (1.0 - k) * self.lp_r + k * delayed_lp_l;
                }
                if !l.is_finite() {
                    l = 0.0;
                }
                if !r.is_finite() {
                    r = 0.0;
                }
            }

            let sl = l * self.makeup_lin;
            let sr = r * self.makeup_lin;
            let sl_c = if sl.is_finite() { sl } else { 0.0 };
            let sr_c = if sr.is_finite() { sr } else { 0.0 };
            let peak = sl_c.abs().max(sr_c.abs());
            let req = if peak > self.ceiling_lin {
                self.ceiling_lin / peak
            } else {
                1.0
            };
            self.step_limiter(sl_c, sr_c, req, out);
        }
    }

    #[inline]
    fn step_limiter(&mut self, in_l: f32, in_r: f32, req_in: f32, out: &mut Vec<f32>) {
        let l_frames = self.lookahead_frames;
        let out_l_raw = self.ring_l[self.ring_pos];
        let out_r_raw = self.ring_r[self.ring_pos];
        let req_now = self.ring_req[self.ring_pos];
        if req_now < 0.999_999 {
            self.below_unity_count = self.below_unity_count.saturating_sub(1);
        }

        self.ring_l[self.ring_pos] = in_l;
        self.ring_r[self.ring_pos] = in_r;
        self.ring_req[self.ring_pos] = req_in;
        if req_in < 0.999_999 {
            self.below_unity_count += 1;
        }

        // Mantener deque monótona mínima O(1) para conocer el menor `req` en los próximos `L` frames.
        let min_keep_step = self.step_idx.saturating_sub(l_frames as u64);
        while let Some(&(idx, _)) = self.min_deque.front() {
            if idx <= min_keep_step && self.step_idx >= l_frames as u64 {
                self.min_deque.pop_front();
            } else {
                break;
            }
        }
        while let Some(&(_, back_req)) = self.min_deque.back() {
            if back_req >= req_in {
                self.min_deque.pop_back();
            } else {
                break;
            }
        }
        self.min_deque.push_back((self.step_idx, req_in));

        if self.step_idx >= l_frames as u64 {
            if self.below_unity_count == 0 && req_now >= 1.0 {
                self.env_gain = self.release_coeff * self.env_gain + (1.0 - self.release_coeff);
                if self.env_gain > 0.999_99 {
                    self.env_gain = 1.0;
                }
            } else {
                let min_future = self.min_deque.front().map(|x| x.1).unwrap_or(1.0);
                // Solo necesitamos explorar pendientes si existe algún futuro por debajo de `env_gain`
                let mut max_slope = 0.0_f32;
                if min_future < self.env_gain {
                    for d in 1..=l_frames {
                        let idx = (self.ring_pos + d) % l_frames;
                        let r_d = self.ring_req[idx];
                        if r_d < self.env_gain {
                            let slope = (self.env_gain - r_d) / (d as f32);
                            if slope > max_slope {
                                max_slope = slope;
                            }
                        }
                    }
                }
                if max_slope > 0.0 {
                    self.env_gain = (self.env_gain - max_slope).min(req_now).max(0.0);
                } else {
                    let released =
                        self.release_coeff * self.env_gain + (1.0 - self.release_coeff);
                    self.env_gain = released.min(min_future).min(req_now);
                }
            }

            let mut out_l = (out_l_raw * self.env_gain).clamp(-self.ceiling_lin, self.ceiling_lin);
            let mut out_r = (out_r_raw * self.env_gain).clamp(-self.ceiling_lin, self.ceiling_lin);
            if !out_l.is_finite() {
                out_l = 0.0;
            }
            if !out_r.is_finite() {
                out_r = 0.0;
            }
            self.max_out_abs = self.max_out_abs.max(out_l.abs()).max(out_r.abs());
            out.push(out_l);
            out.push(out_r);
        }

        self.ring_pos = (self.ring_pos + 1) % l_frames;
        self.step_idx += 1;
    }
}

/// Ejecuta la cadena de procesamiento DSP (`rubato` + filtros IIR `biquad` + pliegue mono
/// / anchura Mid-Side + crossfeed psicoacústico ligero + nivelación LUFS + limitador
/// True Peak con Lookahead ~4 ms y envolvente suave de ganancia).
pub fn process_pcm_with_plan(
    samples_interleaved: &[f32],
    plan: &AdaptiveDspPlan,
) -> Result<(Vec<f32>, f32), String> {
    if samples_interleaved.is_empty() {
        return Ok((Vec::new(), -90.0));
    }

    let expected_frames = if plan.resample_needed && plan.source_sample_rate != plan.target_sample_rate
    {
        ((samples_interleaved.len() / 2) as f64 * (plan.target_sample_rate as f64)
            / (plan.source_sample_rate.max(1) as f64))
            .round() as usize
    } else {
        samples_interleaved.len() / 2
    };

    let mut processor = AdaptiveDspProcessor::new(plan)?;
    let mut out = Vec::with_capacity(expected_frames * 2);
    let mut chunk_out = Vec::with_capacity(8192 * 2);

    for chunk in samples_interleaved.chunks(8192 * 2) {
        processor.process_chunk(chunk, &mut chunk_out)?;
        out.extend_from_slice(&chunk_out);
    }
    let out_dbtp = processor.finish(&mut chunk_out)?;
    out.extend_from_slice(&chunk_out);
    Ok((out, out_dbtp))
}
