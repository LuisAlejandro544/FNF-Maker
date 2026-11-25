//! Archivo: engine/crates/pulse_audio/src/adaptive_master/mod.rs
//! Propósito: Orquestador modular del adaptador acústico de `pulse_audio` (100% Rust puro).
//! Reexporta los tipos públicos y coordina sondeo de hardware, caché incremental,
//! análisis EBU R128 + FFT, plan DSP, masterización con lookahead y guardado TPDF.

mod analyzer;
mod cache_io;
mod dsp_chain;
mod models;
mod probe_plan;

#[cfg(test)]
mod tests;

pub use analyzer::analyze_song_acoustics;
pub use dsp_chain::{process_pcm_with_plan, resample_stereo_with_rubato};
pub use models::{
    AdaptiveDspPlan, AndroidOutputRoute, AudioAdaptationReport, DeviceAudioProbeInput,
    EffectiveDeviceAcoustics, EqBandAdjustment, SongAcousticAnalysis, SpectralBandProfile,
};
pub use probe_plan::{build_adaptive_dsp_plan, fuse_device_and_aaudio_probe};

use crate::decoder::DecodedTrack;
use crate::rust_audio_backend::AudioHardwareProbe;
use cache_io::{
    compute_adaptation_cache_hash, mix_decoded_tracks_f32, mix_raw_source_files_f32,
    read_wav_stereo_f32, write_stereo_i16_wav_atomic,
};
use pulse_core::project::{AudioTrackSlot, MAX_AUDIO_TRACKS};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// Orquesta la fase independiente de Adaptación Acústica Adaptativa en Rust:
/// sondea el hardware real + AAudio, verifica caché por huella, decodifica en ruta `f32` nativa,
/// analiza con `ebur128` + `realfft`, construye el plan para este teléfono y salida, procesa con
/// `rubato` + `biquad` + Mid/Side + Crossfeed + Lookahead Limiter y guarda el WAV con dither TPDF.
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
        "Preparando señal de alta precisión...",
        "Decodificando audio nativo en f32 sin pérdida previa",
    );

    // Prioridad 1: ruta nativa en `f32` directo desde archivos fuente (sin `i16` ni `tanh()` previo).
    let mut used_high_precision_f32 = false;
    let loaded_source = if let Some((samples, sr)) = mix_raw_source_files_f32(configs)? {
        used_high_precision_f32 = true;
        Some((samples, sr))
    } else if let Some((samples, sr)) = mix_decoded_tracks_f32(tracks, configs) {
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
    let mut dsp_plan = build_adaptive_dsp_plan(&device, &song_analysis);
    dsp_plan.high_precision_f32_path = used_high_precision_f32;

    on_progress(
        0.78,
        "Masterizando con Lookahead y espacialidad...",
        &dsp_plan.adaptation_summary,
    );
    let (mastered_samples, output_true_peak_dbtp) = process_pcm_with_plan(&raw_samples, &dsp_plan)?;

    on_progress(
        0.92,
        "Guardando audio calibrado (TPDF Dither)...",
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
