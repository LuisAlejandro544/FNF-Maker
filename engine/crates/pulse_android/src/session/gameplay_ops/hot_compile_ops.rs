//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/hot_compile_ops.rs
//! Propósito: Compilación en caliente incremental (`rayon`) de sprites WebP sin pérdida o con
//! cuantización perceptual (`exoquant` / `RGB_565` con dithering), pre-escalado inteligente (`Lanczos3`)
//! al viewport real del teléfono y zoom máximo de cámara, paginación automática de mega-atlas,
//! planificación temporal de ciclo de vida, precompilación en caché de scripts y fusión WAV.

use crate::session::EngineSession;
use crate::session::persistence_ops::sanitize_project_id;
use pulse_audio::merge_tracks_to_wav;
use pulse_core::disk_cache::{
    cleanup_legacy_flat_compiled_cache, evict_lru_subdirs, touch_dir_last_used,
};
use pulse_core::hot_compile::{
    AudioMergeFingerprint, CachedAudioEntry, DeviceResourceTier, HotCompileManifest,
    ScreenViewportMetrics, hot_compile_project_to_dir_with_manifest_viewport,
};
use pulse_scripting::precompile_scripts_with_cache;
use std::collections::HashMap;
use std::sync::atomic::Ordering;

impl EngineSession {
    /// Ejecuta compilación en caliente usando el perfil de dispositivo indicado sin callback de progreso.
    pub fn hot_compile(&mut self, tier_code: i32) -> String {
        self.hot_compile_with_viewport(tier_code, 1280, 720)
    }

    /// Ejecuta compilación en caliente adaptada a las dimensiones reales en píxeles de pantalla.
    pub fn hot_compile_with_viewport(
        &mut self,
        tier_code: i32,
        viewport_w: i32,
        viewport_h: i32,
    ) -> String {
        self.hot_compile_with_progress_and_viewport(
            tier_code,
            viewport_w,
            viewport_h,
            |_pct, _phase, _detail| {},
        )
    }

    /// Ejecuta compilación en caliente con caché incremental por huella (`manifest.json`),
    /// conversión WebP sin pérdida, precompilación Lua/TS/JS y reporte continuo de progreso.
    pub fn hot_compile_with_progress<F>(&mut self, tier_code: i32, on_progress: F) -> String
    where
        F: Fn(f32, &str, &str) + Send + Sync,
    {
        self.hot_compile_with_progress_and_viewport(tier_code, 1280, 720, on_progress)
    }

    /// Ejecuta compilación en caliente tomando en cuenta el tamaño real del viewport horizontal
    /// del teléfono y el zoom máximo detectado en cámaras y eventos del chart (mip-downscale `Lanczos3`).
    pub fn hot_compile_with_progress_and_viewport<F>(
        &mut self,
        tier_code: i32,
        viewport_w: i32,
        viewport_h: i32,
        on_progress: F,
    ) -> String
    where
        F: Fn(f32, &str, &str) + Send + Sync,
    {
        let tier = match tier_code {
            0 => DeviceResourceTier::LowRam32Bit,
            2 => DeviceResourceTier::HighQuality,
            _ => DeviceResourceTier::Balanced,
        };
        let viewport = ScreenViewportMetrics {
            pixel_width: viewport_w.max(320) as u32,
            pixel_height: viewport_h.max(240) as u32,
        }
        .normalized();

        let cache_root = self.compiled_cache_root_dir();
        cleanup_legacy_flat_compiled_cache(&cache_root);
        let out_dir = self.active_compiled_cache_dir();
        let _ = std::fs::create_dir_all(&out_dir);
        touch_dir_last_used(&out_dir);

        on_progress(
            0.04,
            "Revisando proyecto...",
            "Comprobando imágenes, escenario y scripts guardados",
        );

        let mut manifest = HotCompileManifest::load_or_default(&out_dir, &self.project.id);

        let mut bundle = hot_compile_project_to_dir_with_manifest_viewport(
            &self.project,
            tier,
            viewport,
            Some(&out_dir),
            &mut manifest,
            |pct, detail| {
                on_progress(pct, "Optimizando personajes y fondos", detail);
            },
        );

        on_progress(
            0.75,
            "Preparando scripts y eventos",
            "Activando animaciones y efectos personalizados",
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
        let mut texture_pages: HashMap<String, Vec<String>> =
            HashMap::with_capacity(bundle.compiled_texture_pages.len());
        for (entity_id, pages) in &bundle.compiled_texture_pages {
            texture_pages.insert(
                entity_id.clone(),
                pages
                    .iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect(),
            );
        }
        let mut pixel_formats: HashMap<String, String> =
            HashMap::with_capacity(bundle.texture_pixel_formats.len());
        for (entity_id, fmt) in &bundle.texture_pixel_formats {
            pixel_formats.insert(entity_id.clone(), fmt.as_key().to_string());
        }
        let mut countdown_paths: HashMap<String, String> =
            HashMap::with_capacity(bundle.compiled_countdown_paths.len());
        for (slot_idx, path) in &bundle.compiled_countdown_paths {
            countdown_paths.insert(slot_idx.to_string(), path.to_string_lossy().into_owned());
        }
        let mut icon_normal_paths: HashMap<String, String> =
            HashMap::with_capacity(bundle.compiled_icon_normal_paths.len());
        for (entity_id, path) in &bundle.compiled_icon_normal_paths {
            icon_normal_paths.insert(entity_id.clone(), path.to_string_lossy().into_owned());
        }
        let mut icon_losing_paths: HashMap<String, String> =
            HashMap::with_capacity(bundle.compiled_icon_losing_paths.len());
        for (entity_id, path) in &bundle.compiled_icon_losing_paths {
            icon_losing_paths.insert(entity_id.clone(), path.to_string_lossy().into_owned());
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
                    "Mezcla de música lista",
                    "Audio preparado al instante sin volver a convertir",
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
                    "Preparando pistas musicales...",
                    "Uniendo música y voces para sonar sin cortes",
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
            "Escenario guardado y listo",
            "Cargando todo para empezar la partida",
        );

        self.compiled_texture_paths = texture_paths;
        self.compiled_texture_pages = texture_pages;
        self.texture_pixel_formats = pixel_formats;
        self.compiled_countdown_paths = countdown_paths;
        self.compiled_icon_normal_paths = icon_normal_paths;
        self.compiled_icon_losing_paths = icon_losing_paths;
        self.last_compile_report = Some(bundle.report.clone());
        self.last_timeline_windows = bundle.timeline_windows.clone();
        self.compiled_bundle = Some(bundle);
        self.ok_response("Compilación en caliente inteligente completada")
    }
}
