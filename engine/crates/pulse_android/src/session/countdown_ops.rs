//! Archivo: engine/crates/pulse_android/src/session/countdown_ops.rs
//! Propósito: Gestión en Rust de los recursos personalizados de cuenta regresiva
//! ("2 / Ready?", "1 / Set?", "¡YA! / Go"): importación de imágenes independientes
//! (PNG/WebP/ASTC), atlas Sparrow `.xml` opcionales por paso, audios/voces separados
//! (`intro2`, `intro1`, `introGo`) y preescucha inmediata en el mezclador AAudio.

use super::{EngineSession, friendly_import_name};
use pulse_audio::decode_audio_file_cached;
use pulse_core::astc_loader::{
    is_astc_path, load_sprite_image, read_sprite_dimensions, save_lossless_webp,
};
use pulse_core::project::{
    COUNTDOWN_STEP_COUNT, CountdownAssetsConfig, countdown_step_label,
    parse_countdown_sparrow_xml,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

impl EngineSession {
    /// Importa una imagen independiente (`.png`, `.webp`, `.jpg`, `.astc`) para el paso `slot` (`0..=2`).
    pub fn import_countdown_image(&mut self, slot: usize, path: PathBuf) -> String {
        if slot >= COUNTDOWN_STEP_COUNT {
            return self.err_response(format!(
                "Paso de cuenta regresiva inválido ({}): solo existen 0..2",
                slot
            ));
        }
        if !path.is_file() {
            return self.err_response("Archivo de imagen de cuenta regresiva no encontrado");
        }

        let friendly_name = friendly_import_name(&path);
        let stem_hint = format!("countdown_step_{}", slot + 1);
        let is_astc = is_astc_path(&path);

        let (adopted_raw_path, display_img_path) = if is_astc {
            let decoded = match load_sprite_image(&path) {
                Ok(img) => img,
                Err(e) => return self.err_response(format!("Error decodificando .astc: {}", e)),
            };
            let adopted_raw = match self.adopt_permanent_asset(&path, "countdown", &stem_hint) {
                Ok(p) => p,
                Err(e) => return self.err_response(e),
            };
            let dec_dir = self.project_textures_dir().join("decoded_textures");
            let _ = std::fs::create_dir_all(&dec_dir);
            let out_webp = dec_dir.join(format!("countdown_{}.webp", slot));
            if let Err(e) = save_lossless_webp(&decoded, &out_webp) {
                let _ = std::fs::remove_file(&adopted_raw);
                return self.err_response(format!("Error guardando caché ASTC->WebP: {}", e));
            }
            (adopted_raw, out_webp)
        } else {
            if read_sprite_dimensions(&path).is_none() {
                return self.err_response("Imagen de cuenta regresiva inválida o corrupta");
            }
            let adopted_img = match self.adopt_permanent_asset(&path, "countdown", &stem_hint) {
                Ok(p) => p,
                Err(e) => return self.err_response(e),
            };
            (adopted_img.clone(), adopted_img)
        };

        let old_raw = self.project.countdown.slots[slot].raw_image_path.take();
        let old_img = self.project.countdown.slots[slot].image_path.take();
        self.project.countdown.slots[slot].slot_index = slot;
        self.project.countdown.slots[slot].raw_image_path = Some(adopted_raw_path);
        self.project.countdown.slots[slot].image_path = Some(display_img_path);
        self.project.countdown.slots[slot].image_name = Some(friendly_name.clone());

        self.remove_managed_asset_if_unreferenced(old_raw.as_deref());
        if old_img.as_deref() != old_raw.as_deref() {
            self.remove_managed_asset_if_unreferenced(old_img.as_deref());
        }

        self.save_active_project_now();
        self.ok_response(format!(
            "Sprite '{}' asignado a {}",
            friendly_name,
            countdown_step_label(slot)
        ))
    }

    /// Importa y parsea un atlas Sparrow `.xml` opcional para animar el paso `slot` (`0..=2`).
    pub fn import_countdown_xml(&mut self, slot: usize, path: PathBuf) -> String {
        if slot >= COUNTDOWN_STEP_COUNT {
            return self.err_response(format!(
                "Paso de cuenta regresiva inválido ({}): solo existen 0..2",
                slot
            ));
        }
        let xml_text = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => return self.err_response(format!("No se pudo leer el archivo XML: {}", e)),
        };

        let frames = match parse_countdown_sparrow_xml(&xml_text, slot) {
            Ok(f) => f,
            Err(e) => return self.err_response(e),
        };
        let frame_count = frames.len();
        let friendly_name = friendly_import_name(&path);

        self.project.countdown.slots[slot].slot_index = slot;
        self.project.countdown.slots[slot].xml_path = Some(PathBuf::from(&friendly_name));
        self.project.countdown.slots[slot].frames = frames;
        self.project.countdown.sanitize();

        self.save_active_project_now();
        self.ok_response(format!(
            "XML '{}' ({} frames) aplicado a {}",
            friendly_name,
            frame_count,
            countdown_step_label(slot)
        ))
    }

    /// Quita la imagen y XML personalizados del paso `slot` para volver a la tarjeta vectorial en ese paso.
    pub fn clear_countdown_sprite(&mut self, slot: usize) -> String {
        if slot >= COUNTDOWN_STEP_COUNT {
            return self.err_response("Paso de cuenta regresiva fuera de rango");
        }
        let old_raw = self.project.countdown.slots[slot].raw_image_path.take();
        let old_img = self.project.countdown.slots[slot].image_path.take();
        self.project.countdown.slots[slot].clear_sprite();

        self.remove_managed_asset_if_unreferenced(old_raw.as_deref());
        if old_img.as_deref() != old_raw.as_deref() {
            self.remove_managed_asset_if_unreferenced(old_img.as_deref());
        }

        self.save_active_project_now();
        self.ok_response(format!(
            "Sprite de {} restaurado al estilo vectorial",
            countdown_step_label(slot)
        ))
    }

    /// Importa un clip de audio/voz separado (`.ogg`, `.wav`, `.mp3`) para el paso `slot` (`0..=2`).
    pub fn import_countdown_audio(&mut self, slot: usize, path: PathBuf) -> String {
        if slot >= COUNTDOWN_STEP_COUNT {
            return self.err_response(format!(
                "Paso de audio inválido ({}): solo existen 0..2",
                slot
            ));
        }
        if !path.is_file() {
            return self.err_response("Archivo de audio de cuenta regresiva no encontrado");
        }

        let friendly_name = friendly_import_name(&path);
        let stem_hint = format!("countdown_voice_{}", slot + 1);
        let adopted_path = match self.adopt_permanent_asset(&path, "countdown", &stem_hint) {
            Ok(p) => p,
            Err(e) => return self.err_response(e),
        };

        self.audio_engine.ensure_started();
        let target_sr = self
            .audio_engine
            .mixer
            .sample_rate
            .load(Ordering::Relaxed)
            .max(8000);
        let audio_cache_dir = self.audio_cache_dir();
        let audio_quota = self.cache_budget().audio_cache_bytes;

        match decode_audio_file_cached(&adopted_path, target_sr, Some(&audio_cache_dir), audio_quota)
        {
            Ok(decoded) => {
                let old_path = self.project.countdown.slots[slot].audio_path.take();
                self.project.countdown.slots[slot].slot_index = slot;
                self.project.countdown.slots[slot].audio_path = Some(adopted_path);
                self.project.countdown.slots[slot].audio_name = Some(friendly_name.clone());
                self.remove_managed_asset_if_unreferenced(old_path.as_deref());

                let arc_track = Arc::new(decoded);
                self.audio_engine
                    .mixer
                    .set_countdown_cue_track(slot, Some(Arc::clone(&arc_track)));
                self.decoded_countdown_cues[slot] = Some(arc_track);
                self.save_active_project_now();

                self.ok_response(format!(
                    "Voz/SFX '{}' asignada a {}",
                    friendly_name,
                    countdown_step_label(slot)
                ))
            }
            Err(e) => {
                let _ = std::fs::remove_file(&adopted_path);
                self.err_response(format!(
                    "Error decodificando audio de {}: {}",
                    countdown_step_label(slot),
                    e
                ))
            }
        }
    }

    /// Quita la voz/SFX personalizada del paso `slot` para usar el sintetizador limpio por defecto.
    pub fn clear_countdown_audio(&mut self, slot: usize) -> String {
        if slot >= COUNTDOWN_STEP_COUNT {
            return self.err_response("Paso de audio fuera de rango");
        }
        let old_path = self.project.countdown.slots[slot].audio_path.take();
        self.project.countdown.slots[slot].clear_audio();
        self.decoded_countdown_cues[slot] = None;
        self.audio_engine.mixer.set_countdown_cue_track(slot, None);
        self.remove_managed_asset_if_unreferenced(old_path.as_deref());

        self.save_active_project_now();
        self.ok_response(format!(
            "Voz de {} restaurada al sintetizador integrado",
            countdown_step_label(slot)
        ))
    }

    /// Dispara la preescucha inmediata del paso `slot` (`0..=2`) en AAudio (personalizada o sinte).
    pub fn preview_countdown_audio(&mut self, slot: usize) -> String {
        if slot >= COUNTDOWN_STEP_COUNT {
            return self.err_response("Paso de audio fuera de rango");
        }
        self.audio_engine.ensure_started();
        self.audio_engine.mixer.pause();
        self.audio_engine.mixer.trigger_countdown_cue_preview(slot);
        self.ok_response(format!("Probando sonido de {}", countdown_step_label(slot)))
    }

    /// Restaura los 3 pasos ("2 / Ready?", "1 / Set?", "¡YA! / Go") al estilo visual y sonoro integrado por defecto.
    pub fn restore_default_countdown_assets(&mut self) -> String {
        let mut old_paths: Vec<PathBuf> = Vec::new();
        for cd in &mut self.project.countdown.slots {
            if let Some(p) = cd.raw_image_path.take() {
                old_paths.push(p);
            }
            if let Some(p) = cd.image_path.take() {
                old_paths.push(p);
            }
            if let Some(p) = cd.audio_path.take() {
                old_paths.push(p);
            }
        }
        self.project.countdown = CountdownAssetsConfig::default();
        self.decoded_countdown_cues = [None, None, None];
        self.audio_engine.mixer.clear_countdown_cues();

        for p in old_paths {
            self.remove_managed_asset_if_unreferenced(Some(p.as_path()));
        }

        self.save_active_project_now();
        self.ok_response("Cuenta regresiva (sprites y voces) restaurada por defecto")
    }
}
