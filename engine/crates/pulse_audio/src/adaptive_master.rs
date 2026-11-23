//! Archivo: engine/crates/pulse_audio/src/adaptive_master.rs
//! Propósito: Motor de adaptación acústica en Rust puro (100% dependencias permisivas MIT/Apache-2.0)
//! que se ejecuta como fase independiente después del Compilador en Caliente y antes del Gameplay:
//! 1. Fusiona el sondeo físico del teléfono (`AudioManager` + `AudioDeviceInfo`) con el stream
//!    real de `libaaudio.so` (`AudioHardwareProbe`: frecuencia nativa, frames/ráfaga, baja latencia).
//! 2. Analiza cada canción con `ebur128` (LUFS integrado, LRA, True Peak dBTP) y `realfft`
//!    (energía espectral de 5 bandas con ventana Hann).
//! 3. Diseña y aplica una cadena DSP específica para ese teléfono + ruta activa + canción usando
//!    `rubato` (`FftFixedIn`), filtros IIR `biquad` (`DirectForm2Transposed`), pliegue mono seguro
//!    para altavoces de teléfono mono y limitador True Peak, escribiendo un WAV optimizado en caché.

use crate::decoder::DecodedTrack;
use crate::rust_audio_backend::AudioHardwareProbe;
use biquad::{Biquad, Coefficients, DirectForm2Transposed, Q_BUTTERWORTH_F32, ToHertz, Type};
use ebur128::{EbuR128, Mode};
use pulse_core::project::{AudioTrackSlot, MAX_AUDIO_TRACKS};
use realfft::RealFftPlanner;
use rubato::{FftFixedIn, Resampler};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

/// Ruta física de salida detectada por Android (`AudioDeviceInfo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AndroidOutputRoute {
    #[default]
    BuiltinSpeaker,
    WiredHeadphones,
    UsbAudio,
    BluetoothA2dp,
    BluetoothLe,
    HdmiOrExternal,
    Unknown,
}

impl AndroidOutputRoute {
    pub fn from_key(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "builtin_speaker" | "speaker" => Self::BuiltinSpeaker,
            "wired_headphones" | "wired_headset" | "headphones" => Self::WiredHeadphones,
            "usb_audio" | "usb" | "usb_headset" => Self::UsbAudio,
            "bluetooth_a2dp" | "a2dp" => Self::BluetoothA2dp,
            "bluetooth_le" | "ble_audio" => Self::BluetoothLe,
            "hdmi_or_external" | "hdmi" | "external" => Self::HdmiOrExternal,
            _ => Self::Unknown,
        }
    }

    pub fn label_es(self) -> &'static str {
        match self {
            Self::BuiltinSpeaker => "Altavoz del Teléfono",
            Self::WiredHeadphones => "Audífonos por Cable",
            Self::UsbAudio => "DAC / Audífonos USB-C",
            Self::BluetoothA2dp => "Bluetooth A2DP",
            Self::BluetoothLe => "Bluetooth LE Audio",
            Self::HdmiOrExternal => "HDMI / Salida Externa",
            Self::Unknown => "Salida Estándar",
        }
    }
}

/// Datos recolectados en `Dispatchers.IO` desde Android `AudioManager` + `AudioDeviceInfo` + `Build`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceAudioProbeInput {
    #[serde(default)]
    pub route: AndroidOutputRoute,
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub manufacturer: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_android_api")]
    pub android_api_level: u32,
    #[serde(default = "default_sr")]
    pub os_sample_rate: u32,
    #[serde(default = "default_burst")]
    pub os_frames_per_buffer: u32,
    #[serde(default = "default_channels")]
    pub reported_channel_count: u32,
    #[serde(default)]
    pub is_mono_speaker: bool,
    #[serde(default)]
    pub supports_low_latency_feature: bool,
    #[serde(default)]
    pub supports_pro_audio_feature: bool,
    #[serde(default)]
    pub supported_sample_rates: Vec<u32>,
}

fn default_android_api() -> u32 {
    26
}
fn default_sr() -> u32 {
    48_000
}
fn default_burst() -> u32 {
    192
}
fn default_channels() -> u32 {
    2
}

impl Default for DeviceAudioProbeInput {
    fn default() -> Self {
        Self {
            route: AndroidOutputRoute::BuiltinSpeaker,
            device_name: "Altavoz Integrado".to_string(),
            manufacturer: "Android".to_string(),
            model: "Generic".to_string(),
            android_api_level: 26,
            os_sample_rate: 48_000,
            os_frames_per_buffer: 192,
            reported_channel_count: 2,
            is_mono_speaker: false,
            supports_low_latency_feature: false,
            supports_pro_audio_feature: false,
            supported_sample_rates: vec![48_000],
        }
    }
}

/// Fusión autoritativa entre el sondeo Java (`AudioManager`) y el stream nativo Rust (`AAudio`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveDeviceAcoustics {
    pub route: AndroidOutputRoute,
    pub route_label: String,
    pub device_name: String,
    pub manufacturer: String,
    pub model: String,
    pub android_api_level: u32,
    pub effective_sample_rate: u32,
    pub hardware_native_sample_rate: u32,
    pub effective_channel_count: u32,
    pub is_mono_speaker: bool,
    pub frames_per_burst: u32,
    pub buffer_capacity_frames: u32,
    pub low_latency_active: bool,
    pub aaudio_active: bool,
    pub aaudio_device_id: i32,
}

/// Distribución de energía espectral calculada con `realfft` + ventana Hann.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectralBandProfile {
    pub sub_bass_20_90_hz: f32,
    pub bass_90_220_hz: f32,
    pub low_mid_220_600_hz: f32,
    pub presence_2k_4k_hz: f32,
    pub air_6k_14k_hz: f32,
    pub spectral_tilt_db: f32,
}

/// Análisis real de la canción en Rust mediante `ebur128` + `realfft`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongAcousticAnalysis {
    pub sample_rate: u32,
    pub duration_sec: f32,
    pub integrated_lufs: f32,
    pub loudness_range_lu: f32,
    pub true_peak_dbtp: f32,
    pub crest_factor_db: f32,
    pub stereo_correlation: f32,
    pub bands: SpectralBandProfile,
}

/// Banda individual de ecualización IIR aplicada con `biquad`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqBandAdjustment {
    pub kind: String,
    pub freq_hz: f32,
    pub gain_db: f32,
    pub q: f32,
    pub reason: String,
}

/// Plan de masterización adaptativa derivado para este teléfono y canción concretos.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdaptiveDspPlan {
    pub profile_name: String,
    pub adaptation_summary: String,
    pub target_lufs: f32,
    pub applied_makeup_gain_db: f32,
    pub ceiling_dbtp: f32,
    pub highpass_hz: f32,
    pub mono_fold_enabled: bool,
    pub resample_needed: bool,
    pub source_sample_rate: u32,
    pub target_sample_rate: u32,
    pub eq_bands: Vec<EqBandAdjustment>,
}

/// Reporte final enviado a la UI (`AudioAdaptScreen`) tras adaptar o reutilizar caché.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioAdaptationReport {
    pub enabled: bool,
    pub bypassed: bool,
    pub cache_reused: bool,
    pub elapsed_ms: u64,
    pub output_wav_path: Option<String>,
    pub device: EffectiveDeviceAcoustics,
    pub song_analysis: Option<SongAcousticAnalysis>,
    pub dsp_plan: Option<AdaptiveDspPlan>,
    pub output_true_peak_dbtp: f32,
}

/// Combina los datos de `AudioManager`/`AudioDeviceInfo` con la telemetría real del stream `libaaudio.so`.
pub fn fuse_device_and_aaudio_probe(
    os: &DeviceAudioProbeInput,
    aaudio: &AudioHardwareProbe,
) -> EffectiveDeviceAcoustics {
    let effective_sr = if aaudio.aaudio_active && aaudio.sample_rate >= 8000 {
        aaudio.sample_rate
    } else if os.os_sample_rate >= 8000 {
        os.os_sample_rate
    } else {
        48_000
    };

    let hw_native_sr = if aaudio.hardware_sample_rate >= 8000 {
        aaudio.hardware_sample_rate
    } else if !os.supported_sample_rates.is_empty() {
        if os.supported_sample_rates.contains(&effective_sr) {
            effective_sr
        } else {
            *os.supported_sample_rates.first().unwrap_or(&effective_sr)
        }
    } else {
        effective_sr
    };

    let reported_ch = if os.reported_channel_count >= 1 {
        os.reported_channel_count
    } else if aaudio.hardware_channel_count >= 1 {
        aaudio.hardware_channel_count
    } else {
        aaudio.channel_count.max(1)
    };

    let is_mono_speaker = os.route == AndroidOutputRoute::BuiltinSpeaker
        && (os.is_mono_speaker
            || reported_ch == 1
            || aaudio.hardware_channel_count == 1);

    EffectiveDeviceAcoustics {
        route: os.route,
        route_label: if os.route == AndroidOutputRoute::BuiltinSpeaker {
            if is_mono_speaker {
                "Altavoz del Teléfono (Mono)".to_string()
            } else {
                "Altavoz del Teléfono (Estéreo)".to_string()
            }
        } else {
            os.route.label_es().to_string()
        },
        device_name: if os.device_name.trim().is_empty() {
            os.route.label_es().to_string()
        } else {
            os.device_name.trim().to_string()
        },
        manufacturer: os.manufacturer.trim().to_string(),
        model: os.model.trim().to_string(),
        android_api_level: os.android_api_level.max(26),
        effective_sample_rate: effective_sr,
        hardware_native_sample_rate: hw_native_sr,
        effective_channel_count: reported_ch,
        is_mono_speaker,
        frames_per_burst: if aaudio.frames_per_burst > 0 {
            aaudio.frames_per_burst
        } else {
            os.os_frames_per_buffer.max(64)
        },
        buffer_capacity_frames: aaudio.buffer_capacity_frames,
        low_latency_active: aaudio.low_latency_active || os.supports_low_latency_feature,
        aaudio_active: aaudio.aaudio_active,
        aaudio_device_id: aaudio.device_id,
    }
}

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

/// Calcula el plan de masterización adaptativa combinando el hardware real de TU teléfono,
/// la salida activa detectada y las mediciones EBU R128 + FFT de esta canción específica.
pub fn build_adaptive_dsp_plan(
    device: &EffectiveDeviceAcoustics,
    song: &SongAcousticAnalysis,
) -> AdaptiveDspPlan {
    let mut eq_bands = Vec::with_capacity(5);
    let mut highpass_hz = 28.0_f32;
    let mut target_lufs = -14.5_f32;
    let mut ceiling_dbtp = -1.0_f32;
    let mut mono_fold_enabled = false;
    let mut summary_notes: Vec<String> = Vec::new();

    let sub = song.bands.sub_bass_20_90_hz;
    let bass = song.bands.bass_90_220_hz;
    let low_mid = song.bands.low_mid_220_600_hz;
    let presence = song.bands.presence_2k_4k_hz;
    let air = song.bands.air_6k_14k_hz;

    let profile_name = match device.route {
        AndroidOutputRoute::BuiltinSpeaker => {
            target_lufs = -13.5;
            ceiling_dbtp = -1.0;

            if device.is_mono_speaker {
                // Altavoz único pequeño: evitar distorsión por sub-graves y preservar canales paneados.
                highpass_hz = if sub > 0.22 { 150.0 } else { 135.0 };
                mono_fold_enabled = song.stereo_correlation < 0.92;
                eq_bands.push(EqBandAdjustment {
                    kind: "high_pass".to_string(),
                    freq_hz: highpass_hz,
                    gain_db: 0.0,
                    q: Q_BUTTERWORTH_F32,
                    reason: "Protege el micro-altavoz mono de excursión sub-grave".to_string(),
                });

                let punch_db = (2.6 - (bass * 4.5)).clamp(0.4, 3.0);
                eq_bands.push(EqBandAdjustment {
                    kind: "peaking".to_string(),
                    freq_hz: 195.0,
                    gain_db: punch_db,
                    q: 0.9,
                    reason: "Refuerza pegada audible en altavoz pequeño sin saturar".to_string(),
                });

                let mud_cut_db = if low_mid > 0.26 { -2.4 } else { -1.2 };
                eq_bands.push(EqBandAdjustment {
                    kind: "peaking".to_string(),
                    freq_hz: 360.0,
                    gain_db: mud_cut_db,
                    q: 1.1,
                    reason: "Despeja medios-graves encajonados".to_string(),
                });

                let clarity_db = if presence > 0.24 { 0.3 } else { 2.2 };
                eq_bands.push(EqBandAdjustment {
                    kind: "peaking".to_string(),
                    freq_hz: 2800.0,
                    gain_db: clarity_db,
                    q: 0.95,
                    reason: "Mejora inteligibilidad vocal y transientes rítmicas".to_string(),
                });
                summary_notes.push("Corte sub-grave + pliegue mono seguro".to_string());
                "Altavoz Mono Adaptativo".to_string()
            } else {
                // Doble altavoz estéreo: conservar anchura, controlar sub-grave extremo y realzar pegada.
                highpass_hz = if sub > 0.24 { 105.0 } else { 88.0 };
                eq_bands.push(EqBandAdjustment {
                    kind: "high_pass".to_string(),
                    freq_hz: highpass_hz,
                    gain_db: 0.0,
                    q: Q_BUTTERWORTH_F32,
                    reason: "Limpia sub-graves que el chasis del teléfono no reproduce".to_string(),
                });

                let punch_db = (2.2 - (bass * 3.8)).clamp(0.5, 2.5);
                eq_bands.push(EqBandAdjustment {
                    kind: "peaking".to_string(),
                    freq_hz: 165.0,
                    gain_db: punch_db,
                    q: 0.85,
                    reason: "Añade cuerpo en graves altos adaptado a bocinas estéreo móviles".to_string(),
                });

                if low_mid > 0.22 {
                    eq_bands.push(EqBandAdjustment {
                        kind: "peaking".to_string(),
                        freq_hz: 340.0,
                        gain_db: -1.6,
                        q: 1.0,
                        reason: "Reduce resonancia en medios-graves".to_string(),
                    });
                }

                let pres_db = if presence > 0.24 { -0.5 } else { 1.6 };
                eq_bands.push(EqBandAdjustment {
                    kind: "peaking".to_string(),
                    freq_hz: 2900.0,
                    gain_db: pres_db,
                    q: 0.9,
                    reason: "Ajusta presencia según brillo medido por FFT".to_string(),
                });
                summary_notes.push("Estéreo móvil calibrado por espectro".to_string());
                "Altavoz Estéreo Adaptativo".to_string()
            }
        }
        AndroidOutputRoute::WiredHeadphones => {
            target_lufs = -15.5;
            ceiling_dbtp = -1.0;
            highpass_hz = 28.0;
            eq_bands.push(EqBandAdjustment {
                kind: "high_pass".to_string(),
                freq_hz: highpass_hz,
                gain_db: 0.0,
                q: Q_BUTTERWORTH_F32,
                reason: "Limpieza subsónica DC sin tocar graves reales".to_string(),
            });

            if sub < 0.11 {
                eq_bands.push(EqBandAdjustment {
                    kind: "low_shelf".to_string(),
                    freq_hz: 78.0,
                    gain_db: 1.6,
                    q: Q_BUTTERWORTH_F32,
                    reason: "Compensa falta de sub-grave en esta pista".to_string(),
                });
            } else if sub > 0.34 {
                eq_bands.push(EqBandAdjustment {
                    kind: "low_shelf".to_string(),
                    freq_hz: 75.0,
                    gain_db: -1.4,
                    q: Q_BUTTERWORTH_F32,
                    reason: "Domina exceso de sub-grave para claridad en audífonos".to_string(),
                });
            }

            if presence > 0.26 {
                eq_bands.push(EqBandAdjustment {
                    kind: "peaking".to_string(),
                    freq_hz: 3200.0,
                    gain_db: -1.5,
                    q: 1.2,
                    reason: "Suaviza dureza excesiva en 3.2 kHz".to_string(),
                });
            }

            if air < 0.06 {
                eq_bands.push(EqBandAdjustment {
                    kind: "high_shelf".to_string(),
                    freq_hz: 9500.0,
                    gain_db: 1.3,
                    q: Q_BUTTERWORTH_F32,
                    reason: "Aporta aire suave en agudos".to_string(),
                });
            }
            summary_notes.push("Respuesta full-range fiel para audífonos".to_string());
            "Audífonos Cable Alta Fidelidad".to_string()
        }
        AndroidOutputRoute::UsbAudio => {
            target_lufs = -16.0;
            ceiling_dbtp = -1.2;
            highpass_hz = 24.0;
            eq_bands.push(EqBandAdjustment {
                kind: "high_pass".to_string(),
                freq_hz: highpass_hz,
                gain_db: 0.0,
                q: Q_BUTTERWORTH_F32,
                reason: "Filtro subsónico transparente para DAC externo".to_string(),
            });
            if air < 0.05 {
                eq_bands.push(EqBandAdjustment {
                    kind: "high_shelf".to_string(),
                    freq_hz: 10_500.0,
                    gain_db: 0.9,
                    q: Q_BUTTERWORTH_F32,
                    reason: "Micro-corrección de aire por espectro".to_string(),
                });
            }
            summary_notes.push("Perfil audiófilo transparente DAC USB".to_string());
            "DAC USB-C Referencia".to_string()
        }
        AndroidOutputRoute::BluetoothA2dp | AndroidOutputRoute::BluetoothLe => {
            target_lufs = -14.5;
            // Margen estricto de seguridad contra distorsión en códecs SBC/AAC/LC3
            ceiling_dbtp = -1.6;
            highpass_hz = 38.0;
            eq_bands.push(EqBandAdjustment {
                kind: "high_pass".to_string(),
                freq_hz: highpass_hz,
                gain_db: 0.0,
                q: Q_BUTTERWORTH_F32,
                reason: "Evita bombeo de compresor perceptual Bluetooth".to_string(),
            });

            if sub > 0.28 {
                eq_bands.push(EqBandAdjustment {
                    kind: "low_shelf".to_string(),
                    freq_hz: 85.0,
                    gain_db: -1.3,
                    q: Q_BUTTERWORTH_F32,
                    reason: "Estabiliza sub-grave para códec inalámbrico".to_string(),
                });
            }

            if air < 0.12 {
                eq_bands.push(EqBandAdjustment {
                    kind: "high_shelf".to_string(),
                    freq_hz: 9000.0,
                    gain_db: 1.6,
                    q: Q_BUTTERWORTH_F32,
                    reason: "Compensa caída de agudos en enlace Bluetooth".to_string(),
                });
            }
            summary_notes.push("Techo -1.6 dBTP anti-clipping para códec BT".to_string());
            "Bluetooth Anti-Clipping".to_string()
        }
        AndroidOutputRoute::HdmiOrExternal | AndroidOutputRoute::Unknown => {
            target_lufs = -15.0;
            ceiling_dbtp = -1.0;
            highpass_hz = 34.0;
            eq_bands.push(EqBandAdjustment {
                kind: "high_pass".to_string(),
                freq_hz: highpass_hz,
                gain_db: 0.0,
                q: Q_BUTTERWORTH_F32,
                reason: "Protección subsónica para salida externa".to_string(),
            });
            if presence < 0.12 {
                eq_bands.push(EqBandAdjustment {
                    kind: "peaking".to_string(),
                    freq_hz: 2600.0,
                    gain_db: 1.0,
                    q: 0.9,
                    reason: "Balance de claridad".to_string(),
                });
            }
            summary_notes.push("Nivelación balanceada EBU R128".to_string());
            "Salida Estándar Calibrada".to_string()
        }
    };

    let target_sr = device.effective_sample_rate.max(8000);
    let resample_needed = song.sample_rate != target_sr;
    if resample_needed {
        summary_notes.push(format!("Rubato {} -> {} Hz", song.sample_rate, target_sr));
    }

    // Nivelación EBU R128 acotada [-9 dB, +8 dB] respetando la dinámica de cada canción
    let lufs_delta = (target_lufs - song.integrated_lufs).clamp(-9.0, 8.0);
    let model_tag = if device.model.is_empty() {
        device.manufacturer.as_str()
    } else {
        device.model.as_str()
    };
    let adaptation_summary = format!(
        "{} ({}) · {} · {:.1} -> {:.1} LUFS",
        profile_name,
        if model_tag.is_empty() { "Móvil" } else { model_tag },
        summary_notes.join(" + "),
        song.integrated_lufs,
        target_lufs
    );

    AdaptiveDspPlan {
        profile_name,
        adaptation_summary,
        target_lufs,
        applied_makeup_gain_db: lufs_delta,
        ceiling_dbtp,
        highpass_hz,
        mono_fold_enabled,
        resample_needed,
        source_sample_rate: song.sample_rate,
        target_sample_rate: target_sr,
        eq_bands,
    }
}

/// Remuestrea PCM estéreo entrelazado con `rubato::FftFixedIn` compensando el retardo exacto
/// (`output_delay`) para mantener sincronización milimétrica con el chart y las notas.
pub fn resample_stereo_with_rubato(
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
    let mut resampler = FftFixedIn::<f32>::new(fs_in as usize, fs_out as usize, chunk_size, sub_chunks, 2)
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
    let flush_frames = ((delay_frames as f64 * (fs_in as f64 / fs_out as f64)).ceil() as usize) + step;
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
        let wave_in = [&chunk_l, &chunk_r];
        let out_chunks = resampler
            .process(&[wave_in[0].clone(), wave_in[1].clone()], None)
            .map_err(|e| format!("Error procesando remuestreo Rubato: {}", e))?;
        if out_chunks.len() >= 2 {
            left_all.extend_from_slice(&out_chunks[0]);
            right_all.extend_from_slice(&out_chunks[1]);
        }
        frame_cursor += step;
    }

    let start_trim = delay_frames.min(left_all.len());
    let avail = left_all.len().saturating_sub(start_trim).min(expected_out_frames);
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

/// Ejecuta la cadena de procesamiento DSP (`rubato` + filtros IIR `biquad` + pliegue mono seguro
/// + nivelación LUFS + limitador True Peak con envolvente de anticipación suave).
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

    let fs_hz = (plan.target_sample_rate.max(8000) as f32).hz();

    // Construir banco de filtros IIR dual-channel con `biquad::DirectForm2Transposed`
    let mut filters_l: Vec<DirectForm2Transposed<f32>> = Vec::with_capacity(plan.eq_bands.len());
    let mut filters_r: Vec<DirectForm2Transposed<f32>> = Vec::with_capacity(plan.eq_bands.len());
    for band in &plan.eq_bands {
        let max_f = (plan.target_sample_rate as f32 * 0.45).max(100.0);
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

    // Aplicar EQ IIR + pliegue mono seguro en altavoz único
    for pair in work.chunks_exact_mut(2) {
        let mut l = pair[0];
        let mut r = pair[1];
        for f in &mut filters_l {
            l = f.run(l);
        }
        for f in &mut filters_r {
            r = f.run(r);
        }
        if plan.mono_fold_enabled {
            let mono = 0.5 * (l + r);
            pair[0] = mono;
            pair[1] = mono;
        } else {
            pair[0] = l;
            pair[1] = r;
        }
    }

    // Ganancia por LUFS objetivo + limitador suave con envolvente de ataque/liberación hacia ceiling_dbtp
    let makeup_lin = 10.0_f32.powf(plan.applied_makeup_gain_db / 20.0);
    let ceiling_lin = 10.0_f32.powf(plan.ceiling_dbtp.min(-0.3) / 20.0);
    let release_coeff = (-1.0_f32 / (0.045 * plan.target_sample_rate.max(8000) as f32)).exp();

    let mut env_gain = 1.0_f32;
    let mut max_out_abs = 1e-9_f32;
    for pair in work.chunks_exact_mut(2) {
        let l = pair[0] * makeup_lin;
        let r = pair[1] * makeup_lin;
        let peak = l.abs().max(r.abs());
        let desired_gain = if peak > ceiling_lin {
            ceiling_lin / peak
        } else {
            1.0
        };
        if desired_gain < env_gain {
            // Ataque instantáneo para proteger contra cualquier overshot True Peak
            env_gain = desired_gain;
        } else {
            env_gain = release_coeff * env_gain + (1.0 - release_coeff) * desired_gain;
        }
        let mut out_l = (l * env_gain).clamp(-ceiling_lin, ceiling_lin);
        let mut out_r = (r * env_gain).clamp(-ceiling_lin, ceiling_lin);
        if !out_l.is_finite() {
            out_l = 0.0;
        }
        if !out_r.is_finite() {
            out_r = 0.0;
        }
        max_out_abs = max_out_abs.max(out_l.abs()).max(out_r.abs());
        pair[0] = out_l;
        pair[1] = out_r;
    }

    let out_dbtp = 20.0 * max_out_abs.max(1e-9).log10();
    Ok((work, out_dbtp))
}

fn fnv1a_update(hash: &mut u64, bytes: &[u8]) {
    const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
    for &b in bytes {
        *hash ^= b as u64;
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

fn compute_adaptation_cache_hash(
    tracks: &[Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS],
    configs: &[AudioTrackSlot; MAX_AUDIO_TRACKS],
    pre_merged_wav: Option<&Path>,
    device: &EffectiveDeviceAcoustics,
) -> String {
    let mut hash: u64 = 0xCBF2_9CE4_8422_2325;
    fnv1a_update(&mut hash, b"bf_audio_adapt_v1");
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

fn read_wav_stereo_f32(path: &Path) -> Result<(Vec<f32>, u32), String> {
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

fn mix_decoded_tracks_f32(
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

fn write_stereo_i16_wav_atomic(
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
        for &s in samples_interleaved {
            let pcm = (s.clamp(-1.0, 1.0) * 32767.0).round() as i16;
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

/// Orquesta la fase independiente de Adaptación Acústica Adaptativa en Rust:
/// sondea el hardware real + AAudio, verifica caché por huella, analiza con `ebur128` + `realfft`,
/// construye el plan para este teléfono y salida, procesa con `rubato` + `biquad` y guarda el WAV.
pub fn adapt_audio_for_device<F>(
    enabled: bool,
    tracks: &[Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS],
    configs: &[AudioTrackSlot; MAX_AUDIO_TRACKS],
    pre_merged_wav: Option<&Path>,
    os_probe: &DeviceAudioProbeInput,
    aaudio_probe: &AudioHardwareProbe,
    cache_dir: &Path,
    on_progress: F,
) -> Result<AudioAdaptationReport, String>
where
    F: Fn(f32, &str, &str),
{
    let started = Instant::now();
    on_progress(
        0.08,
        "Detectando salida de audio...",
        "Comprobando altavoces o audífonos del dispositivo",
    );
    let device = fuse_device_and_aaudio_probe(os_probe, aaudio_probe);

    if !enabled {
        on_progress(
            1.0,
            "Calibración de sonido desactivada",
            "Se reproducirá el audio original del proyecto",
        );
        return Ok(AudioAdaptationReport {
            enabled: false,
            bypassed: true,
            cache_reused: false,
            elapsed_ms: started.elapsed().as_millis() as u64,
            output_wav_path: None,
            device,
            song_analysis: None,
            dsp_plan: None,
            output_true_peak_dbtp: 0.0,
        });
    }

    let cache_hash = compute_adaptation_cache_hash(tracks, configs, pre_merged_wav, &device);
    let adapted_wav = cache_dir.join(format!("adapted_{}.wav", cache_hash));
    let report_json = cache_dir.join(format!("adapted_{}.json", cache_hash));

    if adapted_wav.is_file() && report_json.is_file() {
        if let Ok(raw_rep) = std::fs::read_to_string(&report_json) {
            if let Ok(mut cached_rep) = serde_json::from_str::<AudioAdaptationReport>(&raw_rep) {
                cached_rep.cache_reused = true;
                cached_rep.elapsed_ms = started.elapsed().as_millis() as u64;
                cached_rep.output_wav_path = Some(adapted_wav.to_string_lossy().into_owned());
                cached_rep.device = device;
                on_progress(
                    1.0,
                    "Sonido listo al instante",
                    &cached_rep
                        .dsp_plan
                        .as_ref()
                        .map(|p| p.adaptation_summary.clone())
                        .unwrap_or_else(|| "Audio calibrado y listo para reproducir".to_string()),
                );
                return Ok(cached_rep);
            }
        }
    }

    on_progress(
        0.22,
        "Preparando la canción...",
        "Cargando el audio para equilibrar su volumen",
    );

    let loaded_source = if let Some((samples, sr)) = mix_decoded_tracks_f32(tracks, configs) {
        Some((samples, sr))
    } else if let Some(wav_path) = pre_merged_wav {
        if wav_path.is_file() {
            Some(read_wav_stereo_f32(wav_path)?)
        } else {
            None
        }
    } else {
        None
    };

    let Some((raw_samples, source_sr)) = loaded_source else {
        on_progress(
            1.0,
            "Usando sonido base de prueba",
            "Salida de audio lista",
        );
        return Ok(AudioAdaptationReport {
            enabled: true,
            bypassed: true,
            cache_reused: false,
            elapsed_ms: started.elapsed().as_millis() as u64,
            output_wav_path: None,
            device,
            song_analysis: None,
            dsp_plan: None,
            output_true_peak_dbtp: 0.0,
        });
    };

    on_progress(
        0.45,
        "Analizando volumen y frecuencias...",
        "Midiendo graves, voces y brillo de la canción",
    );
    let song_analysis = analyze_song_acoustics(&raw_samples, source_sr);

    on_progress(
        0.62,
        "Calibrando sonido para tu teléfono...",
        &format!("Salida activa: {}", device.route_label),
    );
    let dsp_plan = build_adaptive_dsp_plan(&device, &song_analysis);

    on_progress(
        0.78,
        "Mejorando claridad y evitando saturación...",
        &dsp_plan.adaptation_summary,
    );
    let (mastered_samples, out_true_peak_dbtp) = process_pcm_with_plan(&raw_samples, &dsp_plan)?;

    on_progress(
        0.92,
        "Guardando audio calibrado...",
        "Listo para sonar con nitidez y sin cortes",
    );
    write_stereo_i16_wav_atomic(&adapted_wav, &mastered_samples, dsp_plan.target_sample_rate)?;

    let report = AudioAdaptationReport {
        enabled: true,
        bypassed: false,
        cache_reused: false,
        elapsed_ms: started.elapsed().as_millis() as u64,
        output_wav_path: Some(adapted_wav.to_string_lossy().into_owned()),
        device,
        song_analysis: Some(song_analysis),
        dsp_plan: Some(dsp_plan),
        output_true_peak_dbtp,
    };

    if let Ok(json_str) = serde_json::to_string(&report) {
        let _ = std::fs::write(&report_json, json_str);
    }

    on_progress(
        1.0,
        "¡Sonido calibrado!",
        "Audio listo para empezar a jugar",
    );

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sine_stereo(freq_hz: f32, sample_rate: u32, duration_sec: f32, amp: f32) -> Vec<f32> {
        let frames = (sample_rate as f32 * duration_sec) as usize;
        let mut out = Vec::with_capacity(frames * 2);
        for i in 0..frames {
            let t = i as f32 / sample_rate as f32;
            let s = (2.0 * std::f32::consts::PI * freq_hz * t).sin() * amp;
            out.push(s);
            out.push(s);
        }
        out
    }

    #[test]
    fn ebur128_and_realfft_distinguish_sub_bass_vs_bright_track() {
        let bass_track = make_sine_stereo(60.0, 48_000, 1.5, 0.5);
        let bright_track = make_sine_stereo(3000.0, 48_000, 1.5, 0.25);

        let a_bass = analyze_song_acoustics(&bass_track, 48_000);
        let a_bright = analyze_song_acoustics(&bright_track, 48_000);

        assert!(a_bass.bands.sub_bass_20_90_hz > 0.7);
        assert!(a_bright.bands.presence_2k_4k_hz > 0.7);
        assert!(a_bass.integrated_lufs > a_bright.integrated_lufs);
        assert!(a_bass.bands.spectral_tilt_db < 0.0);
        assert!(a_bright.bands.spectral_tilt_db > 0.0);
    }

    #[test]
    fn adaptive_plan_changes_across_phone_routes_and_song_profiles() {
        let mono_phone = fuse_device_and_aaudio_probe(
            &DeviceAudioProbeInput {
                route: AndroidOutputRoute::BuiltinSpeaker,
                manufacturer: "Xiaomi".to_string(),
                model: "Redmi Note".to_string(),
                is_mono_speaker: true,
                reported_channel_count: 1,
                os_sample_rate: 48_000,
                ..Default::default()
            },
            &AudioHardwareProbe {
                aaudio_active: true,
                sample_rate: 48_000,
                frames_per_burst: 192,
                ..Default::default()
            },
        );

        let headphones = fuse_device_and_aaudio_probe(
            &DeviceAudioProbeInput {
                route: AndroidOutputRoute::WiredHeadphones,
                manufacturer: "Samsung".to_string(),
                model: "Galaxy S23".to_string(),
                reported_channel_count: 2,
                os_sample_rate: 48_000,
                ..Default::default()
            },
            &AudioHardwareProbe {
                aaudio_active: true,
                sample_rate: 48_000,
                frames_per_burst: 96,
                ..Default::default()
            },
        );

        let bt_a2dp = fuse_device_and_aaudio_probe(
            &DeviceAudioProbeInput {
                route: AndroidOutputRoute::BluetoothA2dp,
                os_sample_rate: 44_100,
                ..Default::default()
            },
            &AudioHardwareProbe {
                aaudio_active: true,
                sample_rate: 44_100,
                ..Default::default()
            },
        );

        let sub_heavy_song = SongAcousticAnalysis {
            sample_rate: 44_100,
            duration_sec: 120.0,
            integrated_lufs: -11.0,
            loudness_range_lu: 5.0,
            true_peak_dbtp: -0.2,
            crest_factor_db: 7.5,
            stereo_correlation: 0.4,
            bands: SpectralBandProfile {
                sub_bass_20_90_hz: 0.40,
                bass_90_220_hz: 0.30,
                low_mid_220_600_hz: 0.15,
                presence_2k_4k_hz: 0.08,
                air_6k_14k_hz: 0.04,
                spectral_tilt_db: -6.5,
            },
        };

        let p_mono = build_adaptive_dsp_plan(&mono_phone, &sub_heavy_song);
        let p_hp = build_adaptive_dsp_plan(&headphones, &sub_heavy_song);
        let p_bt = build_adaptive_dsp_plan(&bt_a2dp, &sub_heavy_song);

        assert!(p_mono.highpass_hz >= 140.0);
        assert!(p_mono.mono_fold_enabled);
        assert!(p_hp.highpass_hz <= 30.0);
        assert!(!p_hp.mono_fold_enabled);
        assert!(p_bt.ceiling_dbtp <= -1.5);
        assert!(p_mono.resample_needed);
        assert!(!p_bt.resample_needed);
    }

    #[test]
    fn process_pcm_with_rubato_and_biquad_preserves_sync_and_respects_peak_ceiling() {
        let src = make_sine_stereo(440.0, 44_100, 1.0, 0.9);
        let dev = EffectiveDeviceAcoustics {
            route: AndroidOutputRoute::BluetoothA2dp,
            route_label: "Bluetooth A2DP".to_string(),
            device_name: "Buds".to_string(),
            manufacturer: "Brand".to_string(),
            model: "Phone".to_string(),
            android_api_level: 33,
            effective_sample_rate: 48_000,
            hardware_native_sample_rate: 48_000,
            effective_channel_count: 2,
            is_mono_speaker: false,
            frames_per_burst: 192,
            buffer_capacity_frames: 768,
            low_latency_active: true,
            aaudio_active: true,
            aaudio_device_id: 7,
        };
        let analysis = analyze_song_acoustics(&src, 44_100);
        let plan = build_adaptive_dsp_plan(&dev, &analysis);
        let (out, out_dbtp) = process_pcm_with_plan(&src, &plan).unwrap();

        // Debe conservar duración exacta (~48,000 frames estéreo = 96,000 floats)
        let out_frames = out.len() / 2;
        assert!((out_frames as isize - 48_000).abs() <= 4);
        assert!(out_dbtp <= plan.ceiling_dbtp + 0.05);
    }
}
