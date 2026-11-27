//! Archivo: engine/crates/pulse_audio/src/adaptive_master/mod.rs
//! Propósito: Orquestador modular del adaptador acústico de `pulse_audio` (100% Rust puro).
//! Reexporta los tipos públicos y coordina sondeo de hardware, caché incremental sin
//! invalidaciones innecesarias, poda de `audio_adapted/`, análisis EBU R128 + FFT por bloques,
//! plan DSP con compensación de latencia, masterización con lookahead en streaming y TPDF.

mod analyzer;
mod cache_io;
mod dsp_chain;
mod models;
mod probe_plan;

#[cfg(test)]
mod tests;

pub use analyzer::{StreamingAcousticAnalyzer, analyze_song_acoustics};
pub use dsp_chain::{AdaptiveDspProcessor, process_pcm_with_plan, resample_stereo_with_rubato};
pub use models::{
    AdaptiveDspPlan, AndroidOutputRoute, AudioAdaptationReport, DeviceAudioProbeInput,
    EffectiveDeviceAcoustics, EqBandAdjustment, SongAcousticAnalysis, SpectralBandProfile,
};
pub use probe_plan::{
    build_adaptive_dsp_plan, estimate_output_latency_and_offset, fuse_device_and_aaudio_probe,
};

use crate::decoder::DecodedTrack;
use crate::rust_audio_backend::AudioHardwareProbe;
use cache_io::{
    ADAPTED_CACHE_MAX_BYTES, ADAPTED_CACHE_MAX_PAIRS, StreamingWavTpdfWriter,
    compute_adaptation_cache_hash, prune_audio_adapted_cache, stream_source_audio_chunks,
    stream_source_audio_chunks_with_sr,
};
use pulse_core::project::{AudioTrackSlot, MAX_AUDIO_TRACKS};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

/// Orquesta la fase independiente de Adaptación Acústica Adaptativa en Rust:
/// sondea el hardware real + AAudio, verifica caché por huella, analiza en streaming `f32`
/// con `ebur128` + `realfft`, construye el plan para este teléfono y salida, procesa por bloques
/// con `rubato` + `biquad` + Mid/Side + Crossfeed + Lookahead Limiter directo a WAV TPDF,
/// poda caché obsoleta y calcula la compensación de latencia (`recommended_audio_offset_ms`).
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
    let applied_auto_offset_ms = device.recommended_audio_offset_ms;

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
            applied_auto_offset_ms,
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
                cached_rep.applied_auto_offset_ms = applied_auto_offset_ms;
                prune_audio_adapted_cache(
                    cache_dir,
                    Some(&adapted_wav),
                    Some(&report_json),
                    ADAPTED_CACHE_MAX_PAIRS,
                    ADAPTED_CACHE_MAX_BYTES,
                );
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
        "Analizando volumen y frecuencias en streaming...",
        "Decodificando audio nativo en f32 por bloques de baja RAM",
    );

    // Pasada 1 (streaming): medir LUFS / LRA / True Peak / FFT sin cargar la canción entera en RAM
    let mut active_analyzer: Option<StreamingAcousticAnalyzer> = None;
    let stream_res =
        stream_source_audio_chunks_with_sr(tracks, configs, pre_merged_wav, |sr, chunk| {
            let analyzer =
                active_analyzer.get_or_insert_with(|| StreamingAcousticAnalyzer::new(sr));
            analyzer.push_chunk(chunk);
            Ok(())
        })?;

    let Some((source_sr, used_high_precision_f32)) = stream_res else {
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
            applied_auto_offset_ms,
        });
    };

    on_progress(
        0.52,
        "Calibrando sonido para tu teléfono...",
        &format!("Salida activa: {}", device.route_label),
    );
    let song_analysis = active_analyzer
        .take()
        .unwrap_or_else(|| StreamingAcousticAnalyzer::new(source_sr))
        .finish();

    let mut dsp_plan = build_adaptive_dsp_plan(&device, &song_analysis);
    dsp_plan.high_precision_f32_path = used_high_precision_f32;

    on_progress(
        0.72,
        "Masterizando en streaming (Lookahead + TPDF)...",
        &dsp_plan.adaptation_summary,
    );

    // Pasada 2 (streaming): aplicar Rubato + EQ Biquad + M/S + Crossfeed + Lookahead directo al WAV
    let mut processor = AdaptiveDspProcessor::new(&dsp_plan)?;
    let mut wav_writer =
        StreamingWavTpdfWriter::create(&adapted_wav, dsp_plan.target_sample_rate)?;
    let mut dsp_chunk_out = Vec::with_capacity(16_384);

    let _ = stream_source_audio_chunks(tracks, configs, pre_merged_wav, |chunk| {
        processor.process_chunk(chunk, &mut dsp_chunk_out)?;
        if !dsp_chunk_out.is_empty() {
            wav_writer.write_samples(&dsp_chunk_out)?;
        }
        Ok(())
    })?;

    let output_true_peak_dbtp = processor.finish(&mut dsp_chunk_out)?;
    if !dsp_chunk_out.is_empty() {
        wav_writer.write_samples(&dsp_chunk_out)?;
    }

    on_progress(
        0.94,
        "Finalizando audio calibrado (TPDF Dither)...",
        "Listo para sonar con nitidez y baja RAM",
    );
    wav_writer.finalize()?;

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
        applied_auto_offset_ms,
    };

    if let Ok(json_str) = serde_json::to_string(&report) {
        let _ = std::fs::write(&report_json, json_str);
    }
    prune_audio_adapted_cache(
        cache_dir,
        Some(&adapted_wav),
        Some(&report_json),
        ADAPTED_CACHE_MAX_PAIRS,
        ADAPTED_CACHE_MAX_BYTES,
    );

    on_progress(
        1.0,
        "¡Sonido calibrado!",
        "Audio listo para empezar a jugar",
    );

    Ok(report)
}
