//! Archivo: engine/crates/pulse_audio/src/adaptive_master/probe_plan.rs
//! Propósito: Fusiona la telemetría de `AudioManager`/`AudioDeviceInfo` con el
//! stream nativo `libaaudio.so` y diseña el `AdaptiveDspPlan` por ruta y espectro.

use super::models::{
    AdaptiveDspPlan, AndroidOutputRoute, DeviceAudioProbeInput, EffectiveDeviceAcoustics,
    EqBandAdjustment, SongAcousticAnalysis,
};
use crate::rust_audio_backend::AudioHardwareProbe;
use biquad::Q_BUTTERWORTH_F32;

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

    let frames_per_burst = if aaudio.frames_per_burst > 0 {
        aaudio.frames_per_burst
    } else {
        os.os_frames_per_buffer.max(64)
    };
    let low_latency_active = aaudio.low_latency_active || os.supports_low_latency_feature;
    let (estimated_output_latency_ms, recommended_audio_offset_ms) =
        estimate_output_latency_and_offset(
            os.route,
            effective_sr,
            frames_per_burst,
            aaudio.buffer_capacity_frames,
            low_latency_active,
            aaudio.aaudio_active,
        );

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
        frames_per_burst,
        buffer_capacity_frames: aaudio.buffer_capacity_frames,
        low_latency_active,
        aaudio_active: aaudio.aaudio_active,
        aaudio_device_id: aaudio.device_id,
        aaudio_matched: os.aaudio_matched && aaudio.device_id > 0,
        estimated_output_latency_ms,
        recommended_audio_offset_ms,
    }
}

/// Estima la latencia física total de salida (buffer AAudio + enlace inalámbrico Bluetooth/HDMI)
/// y calcula el desplazamiento automático recomendado en ms para compensar `audio_offset_ms`.
pub fn estimate_output_latency_and_offset(
    route: AndroidOutputRoute,
    effective_sr: u32,
    frames_per_burst: u32,
    buffer_capacity_frames: u32,
    low_latency_active: bool,
    aaudio_active: bool,
) -> (f32, f64) {
    let sr = effective_sr.max(8000) as f32;
    let active_buf_frames = if low_latency_active {
        frames_per_burst.max(64) * 2
    } else if buffer_capacity_frames > 0 {
        buffer_capacity_frames.max(frames_per_burst.max(64) * 2)
    } else {
        frames_per_burst.max(192) * 3
    };
    let buffer_ms = (active_buf_frames as f32 / sr) * 1000.0;
    let wireless_or_dac_ms = match route {
        AndroidOutputRoute::BluetoothA2dp => 165.0,
        AndroidOutputRoute::BluetoothLe => 50.0,
        AndroidOutputRoute::HdmiOrExternal => 42.0,
        AndroidOutputRoute::UsbAudio => {
            if low_latency_active {
                4.0
            } else {
                14.0
            }
        }
        AndroidOutputRoute::WiredHeadphones => {
            if aaudio_active && low_latency_active {
                0.0
            } else {
                8.0
            }
        }
        AndroidOutputRoute::BuiltinSpeaker | AndroidOutputRoute::Unknown => {
            if aaudio_active && low_latency_active {
                0.0
            } else {
                10.0
            }
        }
    };
    let total_latency_ms = (buffer_ms + wireless_or_dac_ms).clamp(2.0, 320.0);
    let recommended_offset_ms = match route {
        AndroidOutputRoute::BluetoothA2dp => total_latency_ms.clamp(140.0, 240.0).round() as f64,
        AndroidOutputRoute::BluetoothLe => total_latency_ms.clamp(40.0, 95.0).round() as f64,
        AndroidOutputRoute::HdmiOrExternal => total_latency_ms.clamp(25.0, 120.0).round() as f64,
        AndroidOutputRoute::UsbAudio => {
            if !low_latency_active && total_latency_ms > 28.0 {
                (total_latency_ms - 14.0).clamp(0.0, 35.0).round() as f64
            } else {
                0.0
            }
        }
        AndroidOutputRoute::WiredHeadphones
        | AndroidOutputRoute::BuiltinSpeaker
        | AndroidOutputRoute::Unknown => {
            if aaudio_active && !low_latency_active && total_latency_ms > 40.0 {
                (total_latency_ms - 20.0).clamp(0.0, 60.0).round() as f64
            } else {
                0.0
            }
        }
    };
    (total_latency_ms, recommended_offset_ms)
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
    let mut stereo_width = 1.0_f32;
    let mut crossfeed_strength = 0.0_f32;
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
                stereo_width = 0.0;
                crossfeed_strength = 0.0;
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
                // Doble altavoz estéreo: expandir suavemente separación Mid/Side en bocinas cercanas.
                highpass_hz = if sub > 0.24 { 105.0 } else { 88.0 };
                stereo_width = if song.stereo_correlation > 0.35 {
                    1.16
                } else if song.stereo_correlation > 0.10 {
                    1.08
                } else {
                    1.00
                };
                crossfeed_strength = 0.0;
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
                if stereo_width > 1.02 {
                    summary_notes.push("Amplitud estéreo M/S".to_string());
                }
                "Altavoz Estéreo Adaptativo".to_string()
            }
        }
        AndroidOutputRoute::WiredHeadphones => {
            target_lufs = -15.5;
            ceiling_dbtp = -1.0;
            highpass_hz = 28.0;
            stereo_width = if song.stereo_correlation > 0.85 { 1.06 } else { 1.0 };
            crossfeed_strength = 0.22;
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
            summary_notes.push("Crossfeed ligero".to_string());
            "Audífonos Cable Alta Fidelidad".to_string()
        }
        AndroidOutputRoute::UsbAudio => {
            target_lufs = -16.0;
            ceiling_dbtp = -1.2;
            highpass_hz = 24.0;
            stereo_width = 1.0;
            crossfeed_strength = 0.16;
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
            summary_notes.push("Crossfeed audiófilo".to_string());
            "DAC USB-C Referencia".to_string()
        }
        AndroidOutputRoute::BluetoothA2dp | AndroidOutputRoute::BluetoothLe => {
            target_lufs = -14.5;
            // Margen estricto de seguridad contra distorsión en códecs SBC/AAC/LC3
            ceiling_dbtp = -1.6;
            highpass_hz = 38.0;
            stereo_width = if song.stereo_correlation > 0.30 { 1.04 } else { 1.0 };
            crossfeed_strength = 0.12;
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
            summary_notes.push("Crossfeed BT".to_string());
            "Bluetooth Anti-Clipping".to_string()
        }
        AndroidOutputRoute::HdmiOrExternal | AndroidOutputRoute::Unknown => {
            target_lufs = -15.0;
            ceiling_dbtp = -1.0;
            highpass_hz = 34.0;
            stereo_width = 1.0;
            crossfeed_strength = 0.0;
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
    if device.recommended_audio_offset_ms >= 1.0 {
        summary_notes.push(format!("Sync +{:.0} ms", device.recommended_audio_offset_ms));
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
        stereo_width,
        crossfeed_strength,
        limiter_lookahead_ms: 4.0,
        dither_applied: true,
        high_precision_f32_path: true,
        eq_bands,
    }
}
