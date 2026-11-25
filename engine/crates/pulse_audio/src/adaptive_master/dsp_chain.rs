//! Archivo: engine/crates/pulse_audio/src/adaptive_master/dsp_chain.rs
//! Propósito: Cadena DSP de masterización adaptativa en punto flotante (`rubato`,
//! filtros IIR `biquad`, Mid/Side, crossfeed psicoacústico y limitador Lookahead).

use super::models::AdaptiveDspPlan;
use crate::decoder::resample_stereo_f32_rubato;
use biquad::{Biquad, Coefficients, DirectForm2Transposed, ToHertz, Type};

/// Remuestrea PCM estéreo entrelazado con `rubato::FftFixedIn` compensando el retardo exacto
/// (`output_delay`) para mantener sincronización milimétrica con el chart y las notas.
pub fn resample_stereo_with_rubato(
    interleaved_in: &[f32],
    fs_in: u32,
    fs_out: u32,
) -> Result<Vec<f32>, String> {
    resample_stereo_f32_rubato(interleaved_in, fs_in, fs_out)
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

    let mut work = if plan.resample_needed && plan.source_sample_rate != plan.target_sample_rate {
        resample_stereo_with_rubato(
            samples_interleaved,
            plan.source_sample_rate,
            plan.target_sample_rate,
        )?
    } else {
        samples_interleaved.to_vec()
    };

    let target_sr = plan.target_sample_rate.max(8000);
    let fs_f32 = target_sr as f32;
    let fs_hz = fs_f32.hz();

    // 1. Construir banco de filtros IIR dual-channel con `biquad::DirectForm2Transposed`
    let mut filters_l: Vec<DirectForm2Transposed<f32>> = Vec::with_capacity(plan.eq_bands.len());
    let mut filters_r: Vec<DirectForm2Transposed<f32>> = Vec::with_capacity(plan.eq_bands.len());
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

    // 2. Estado de Crossfeed psicoacústico ligero (LP ~700 Hz + retardo interaural ~0.25 ms)
    let crossfeed = if plan.mono_fold_enabled || plan.stereo_width <= 0.001 {
        0.0_f32
    } else {
        plan.crossfeed_strength.clamp(0.0, 0.35)
    };
    let width = if plan.mono_fold_enabled {
        0.0_f32
    } else {
        plan.stereo_width.clamp(0.0, 1.35)
    };
    let alpha_lp = 1.0_f32 - (-2.0_f32 * std::f32::consts::PI * 700.0_f32 / fs_f32).exp();
    let delay_frames = ((0.00025_f32 * fs_f32).round() as usize).clamp(1, 64);
    let mut delay_l = vec![0.0_f32; delay_frames];
    let mut delay_r = vec![0.0_f32; delay_frames];
    let mut delay_idx = 0usize;
    let mut lp_l = 0.0_f32;
    let mut lp_r = 0.0_f32;

    // Aplicar EQ IIR + Mid/Side + Crossfeed antes del limitador
    for pair in work.chunks_exact_mut(2) {
        let mut l = if pair[0].is_finite() { pair[0] } else { 0.0 };
        let mut r = if pair[1].is_finite() { pair[1] } else { 0.0 };
        for f in &mut filters_l {
            l = f.run(l);
        }
        for f in &mut filters_r {
            r = f.run(r);
        }
        if plan.mono_fold_enabled || width <= 0.001 {
            let mono = 0.5 * (l + r);
            pair[0] = mono;
            pair[1] = mono;
        } else {
            if (width - 1.0).abs() > 0.001 {
                let mid = 0.5 * (l + r);
                let side = 0.5 * (l - r) * width;
                l = mid + side;
                r = mid - side;
            }
            if crossfeed > 0.001 {
                lp_l += alpha_lp * (l - lp_l);
                lp_r += alpha_lp * (r - lp_r);
                let delayed_lp_l = delay_l[delay_idx];
                let delayed_lp_r = delay_r[delay_idx];
                delay_l[delay_idx] = lp_l;
                delay_r[delay_idx] = lp_r;
                delay_idx = (delay_idx + 1) % delay_frames;

                let k = 0.5 * crossfeed;
                let high_l = l - lp_l;
                let high_r = r - lp_r;
                l = high_l + (1.0 - k) * lp_l + k * delayed_lp_r;
                r = high_r + (1.0 - k) * lp_r + k * delayed_lp_l;
            }
            pair[0] = if l.is_finite() { l } else { 0.0 };
            pair[1] = if r.is_finite() { r } else { 0.0 };
        }
    }

    // 3. Ganancia por LUFS objetivo + Limitador Lookahead (~4 ms en anillo O(L) de pocos KB)
    let total_frames = work.len() / 2;
    let makeup_lin = 10.0_f32.powf(plan.applied_makeup_gain_db / 20.0);
    let ceiling_lin = 10.0_f32.powf(plan.ceiling_dbtp.min(-0.3) / 20.0);
    let lookahead_sec = plan.limiter_lookahead_ms.clamp(0.0, 10.0) * 0.001;
    let lookahead_frames = if lookahead_sec > 0.0 {
        ((lookahead_sec * fs_f32).round() as usize).clamp(16, 512)
    } else {
        1
    };
    let release_coeff = (-1.0_f32 / (0.065 * fs_f32)).exp();

    let mut ring_l = vec![0.0_f32; lookahead_frames];
    let mut ring_r = vec![0.0_f32; lookahead_frames];
    let mut ring_req = vec![1.0_f32; lookahead_frames];
    let mut ring_pos = 0usize;
    let mut below_unity_count = 0usize;

    let mut env_gain = 1.0_f32;
    let mut max_out_abs = 1e-9_f32;

    for step in 0..(total_frames + lookahead_frames) {
        let (in_l, in_r, req_in) = if step < total_frames {
            let base = step * 2;
            let sl = work[base] * makeup_lin;
            let sr = work[base + 1] * makeup_lin;
            let sl_c = if sl.is_finite() { sl } else { 0.0 };
            let sr_c = if sr.is_finite() { sr } else { 0.0 };
            let peak = sl_c.abs().max(sr_c.abs());
            let req = if peak > ceiling_lin {
                ceiling_lin / peak
            } else {
                1.0
            };
            (sl_c, sr_c, req)
        } else {
            (0.0, 0.0, 1.0)
        };

        let out_l_raw = ring_l[ring_pos];
        let out_r_raw = ring_r[ring_pos];
        let req_now = ring_req[ring_pos];
        if req_now < 0.999_999 {
            below_unity_count = below_unity_count.saturating_sub(1);
        }

        ring_l[ring_pos] = in_l;
        ring_r[ring_pos] = in_r;
        ring_req[ring_pos] = req_in;
        if req_in < 0.999_999 {
            below_unity_count += 1;
        }

        if step >= lookahead_frames {
            if below_unity_count == 0 && req_now >= 1.0 {
                env_gain = release_coeff * env_gain + (1.0 - release_coeff);
                if env_gain > 0.999_99 {
                    env_gain = 1.0;
                }
            } else {
                // Buscar pendiente máxima de pre-atenuación en los próximos `L` frames + tope mínimo futuro
                let mut max_slope = 0.0_f32;
                let mut min_future = 1.0_f32;
                for d in 1..=lookahead_frames {
                    let idx = (ring_pos + d) % lookahead_frames;
                    let r_d = ring_req[idx];
                    if r_d < min_future {
                        min_future = r_d;
                    }
                    if r_d < env_gain {
                        let slope = (env_gain - r_d) / (d as f32);
                        if slope > max_slope {
                            max_slope = slope;
                        }
                    }
                }
                if max_slope > 0.0 {
                    env_gain = (env_gain - max_slope).min(req_now).max(0.0);
                } else {
                    let released = release_coeff * env_gain + (1.0 - release_coeff);
                    env_gain = released.min(min_future).min(req_now);
                }
            }

            let mut out_l = (out_l_raw * env_gain).clamp(-ceiling_lin, ceiling_lin);
            let mut out_r = (out_r_raw * env_gain).clamp(-ceiling_lin, ceiling_lin);
            if !out_l.is_finite() {
                out_l = 0.0;
            }
            if !out_r.is_finite() {
                out_r = 0.0;
            }
            max_out_abs = max_out_abs.max(out_l.abs()).max(out_r.abs());

            let out_base = (step - lookahead_frames) * 2;
            work[out_base] = out_l;
            work[out_base + 1] = out_r;
        }

        ring_pos = (ring_pos + 1) % lookahead_frames;
    }

    let out_dbtp = 20.0 * max_out_abs.max(1e-9).log10();
    Ok((work, out_dbtp))
}
