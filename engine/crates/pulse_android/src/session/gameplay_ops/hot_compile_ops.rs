//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/hot_compile_ops.rs
//! Propósito: Compilación en caliente incremental (`rayon`) de sprites WebP sin pérdida,
//! precompilación en caché de scripts (Lua `.luac`, TypeScript/JS `.js`) y fusión de pistas
//! de audio a `compiled_mix.wav` para reproducción por streaming de baja memoria.

use crate::session::EngineSession;
use crate::session::persistence_ops::sanitize_project_id;
use pulse_audio::merge_tracks_to_wav;
use pulse_core::disk_cache::{
    cleanup_legacy_flat_compiled_cache, evict_lru_subdirs, touch_dir_last_used,
};
use pulse_core::hot_compile::{
    AudioMergeFingerprint, CachedAudioEntry, DeviceResourceTier, HotCompileManifest,
    hot_compile_project_to_dir_with_manifest,
};
use pulse_scripting::precompile_scripts_with_cache;
use std::collections::HashMap;
use std::sync::atomic::Ordering;

impl EngineSession {
    /// Ejecuta compilación en caliente usando el perfil de dispositivo indicado sin callback de progreso.
    pub fn hot_compile(&mut self, tier_code: i32) -> String {
        self.hot_compile_with_progress(tier_code, |_pct, _phase, _detail| {})
    }

    /// Ejecuta compilación en caliente con caché incremental por huella (`manifest.json`),
    /// conversión WebP sin pérdida, precompilación Lua/TS/JS y reporte continuo de progreso.
    pub fn hot_compile_with_progress<F>(&mut self, tier_code: i32, on_progress: F) -> String
    where
        F: Fn(f32, &str, &str) + Send + Sync,
    {
        let tier = match tier_code {
            0 => DeviceResourceTier::LowRam32Bit,
            2 => DeviceResourceTier::HighQuality,
            _ => DeviceResourceTier::Balanced,
        };

        let cache_root = self.compiled_cache_root_dir();
        cleanup_legacy_flat_compiled_cache(&cache_root);
        let out_dir = self.active_compiled_cache_dir();
        let _ = std::fs::create_dir_all(&out_dir);
        touch_dir_last_used(&out_dir);

        on_progress(
            0.04,
            "Leyendo caché incremental...",
            "Verificando huellas de sprites, audio y scripts",
        );

        let mut manifest = HotCompileManifest::load_or_default(&out_dir, &self.project.id);

        let mut bundle = hot_compile_project_to_dir_with_manifest(
            &self.project,
            tier,
            Some(&out_dir),
            &mut manifest,
            |pct, detail| {
                on_progress(pct, "Optimizando sprites a WebP sin pérdida", detail);
            },
        );

        on_progress(
            0.75,
            "Precompilando scripts Lua / TypeScript / JS",
            "Generando bytecode Lua 5.4 y JS transpilado con oxc",
        );
        let scripts_cache_dir = out_dir.join("scripts");
        let script_precompile = precompile_scripts_with_cache(
            &mut bundle.optimized_scripts,
            &mut manifest,
            Some(&scripts_cache_dir),
        );
        bundle.report.precompiled_scripts = script_precompile.precompiled_count;
        bundle.report.cached_scripts_reused = script_precompile.cached_reused_count;
        bundle.report.script_precompile_errors = script_precompile.errors;

        let mut texture_paths = HashMap::with_capacity(bundle.compiled_texture_paths.len());
        for (entity_id, path) in &bundle.compiled_texture_paths {
            texture_paths.insert(
                entity_id.clone(),
                path.to_string_lossy().into_owned(),
            );
        }

        let target_sr = self
            .audio_engine
            .mixer
            .sample_rate
            .load(Ordering::Relaxed)
            .max(8000);
        let compiled_wav = out_dir.join("compiled_mix.wav");
        let audio_fp_opt = AudioMergeFingerprint::compute(&self.project.audio_tracks, target_sr);

        let reused_audio = if let (Some(fp), Some(cached_audio)) = (&audio_fp_opt, &manifest.audio) {
            if &cached_audio.fingerprint == fp && compiled_wav.exists() {
                on_progress(
                    0.85,
                    "Reutilizando mezcla de audio en caché",
                    "Streaming WAV listo sin recodificar",
                );
                bundle.report.merged_audio_tracks = cached_audio.merged_tracks;
                bundle.report.compiled_audio_path = Some(compiled_wav.clone());
                bundle.report.estimated_audio_ram_saved_kb = cached_audio.estimated_ram_saved_kb;
                bundle.report.cached_audio_reused = true;
                self.compiled_audio_path = Some(compiled_wav.clone());
                true
            } else {
                false
            }
        } else {
            false
        };

        if !reused_audio {
            if let Some(fp) = audio_fp_opt {
                on_progress(
                    0.82,
                    "Fusionando pistas de audio...",
                    "Creando mezcla streaming WAV de baja memoria",
                );
                if self.project.active_audio_count() > 0
                    && self.decoded_tracks.iter().all(|t| t.is_none())
                {
                    self.reload_project_audio_tracks();
                }
                match merge_tracks_to_wav(
                    &self.decoded_tracks,
                    &self.project.audio_tracks,
                    target_sr,
                    &compiled_wav,
                ) {
                    Ok(Some(merge_rep)) => {
                        bundle.report.merged_audio_tracks = merge_rep.merged_tracks;
                        bundle.report.compiled_audio_path = Some(merge_rep.output_path.clone());
                        bundle.report.estimated_audio_ram_saved_kb =
                            merge_rep.estimated_ram_saved_kb;
                        self.compiled_audio_path = Some(merge_rep.output_path.clone());
                        manifest.audio = Some(CachedAudioEntry {
                            fingerprint: fp,
                            compiled_wav_path: merge_rep.output_path,
                            merged_tracks: merge_rep.merged_tracks,
                            estimated_ram_saved_kb: merge_rep.estimated_ram_saved_kb,
                        });
                    }
                    _ => {
                        let _ = std::fs::remove_file(&compiled_wav);
                        manifest.audio = None;
                        self.compiled_audio_path = None;
                    }
                }
            } else {
                // 0 o 1 pista: no requiere mezcla WAV combinada; asegurar pistas decodificadas listas
                if self.project.active_audio_count() > 0
                    && self.decoded_tracks.iter().all(|t| t.is_none())
                {
                    self.reload_project_audio_tracks();
                }
                let _ = std::fs::remove_file(&compiled_wav);
                manifest.audio = None;
                self.compiled_audio_path = None;
            }
        }

        let _ = manifest.save_to_dir(&out_dir);
        touch_dir_last_used(&out_dir);
        let safe_active = sanitize_project_id(&self.project.id);
        let _ = evict_lru_subdirs(
            &cache_root,
            self.cache_budget().compiled_cache_bytes,
            &[&safe_active],
        );

        on_progress(
            0.90,
            "Manifiesto incremental guardado",
            "Preparando texturas WebP y entorno de juego",
        );

        self.compiled_texture_paths = texture_paths;
        self.last_compile_report = Some(bundle.report.clone());
        self.compiled_bundle = Some(bundle);
        self.ok_response("Compilación en caliente inteligente completada")
    }
}
