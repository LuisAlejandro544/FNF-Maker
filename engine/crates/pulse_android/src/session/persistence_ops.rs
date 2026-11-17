//! Archivo: engine/crates/pulse_android/src/session/persistence_ops.rs
//! Propósito: Persistencia automática en almacenamiento interno (`filesDir/projects/<id>/project.json`)
//! con escritura atómica (`.tmp` -> `rename`), recarga de pistas de audio al abrir un proyecto
//! y operaciones completas del menú de proyectos (listar, crear, abrir, duplicar, eliminar).

use super::{EngineSession, ProjectSummaryJson};
use pulse_audio::decode_audio_file;
use pulse_core::project::{MAX_AUDIO_TRACKS, ModProject, current_epoch_ms};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const AUTO_SAVE_THROTTLE_MS: u64 = 250;

pub(super) fn sanitize_project_id(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        format!("proj_{}", current_epoch_ms())
    } else {
        trimmed.to_string()
    }
}

impl EngineSession {
    pub fn projects_root_dir(&self) -> PathBuf {
        self.data_dir.join("projects")
    }

    pub fn project_dir_for(&self, project_id: &str) -> PathBuf {
        self.projects_root_dir().join(sanitize_project_id(project_id))
    }

    pub fn project_textures_dir(&self) -> PathBuf {
        self.project_dir_for(&self.project.id).join("textures")
    }

    fn active_project_marker(&self) -> PathBuf {
        self.projects_root_dir().join("active_project.txt")
    }

    /// Guarda inmediatamente el proyecto activo usando escritura atómica para evitar archivos corruptos.
    pub fn save_active_project_now(&mut self) {
        let root = self.projects_root_dir();
        if std::fs::create_dir_all(&root).is_err() {
            return;
        }
        let safe_id = sanitize_project_id(&self.project.id);
        if self.project.id != safe_id {
            self.project.id = safe_id.clone();
        }
        let proj_dir = self.project_dir_for(&safe_id);
        if std::fs::create_dir_all(&proj_dir).is_err() {
            return;
        }

        self.project.updated_at_ms = current_epoch_ms();
        let Ok(bytes) = serde_json::to_vec(&self.project) else {
            return;
        };

        let target = proj_dir.join("project.json");
        let tmp = proj_dir.join("project.json.tmp");
        if std::fs::write(&tmp, &bytes).is_ok() {
            if std::fs::rename(&tmp, &target).is_err() {
                let _ = std::fs::write(&target, &bytes);
                let _ = std::fs::remove_file(&tmp);
            }
        }

        let _ = std::fs::write(self.active_project_marker(), safe_id.as_bytes());
        self.dirty = false;
        self.last_save_at = Instant::now();
    }

    /// Guarda al instante para acciones discretas o aplica throttle suave de 250ms en arrastres 60Hz.
    pub fn mark_dirty_and_maybe_save(&mut self, force: bool) {
        self.dirty = true;
        if force || self.last_save_at.elapsed() >= Duration::from_millis(AUTO_SAVE_THROTTLE_MS) {
            self.save_active_project_now();
        }
    }

    /// Vuelca cualquier cambio pendiente a disco (usado al soltar gestos, cambiar pantalla o `onPause`).
    pub fn flush_pending_save(&mut self) {
        if self.dirty {
            self.save_active_project_now();
        }
    }

    /// Carga el último proyecto activo desde `filesDir/projects/` al arrancar la app.
    pub fn load_initial_project(&mut self) {
        let root = self.projects_root_dir();
        let _ = std::fs::create_dir_all(&root);

        if let Ok(active_id) = std::fs::read_to_string(self.active_project_marker()) {
            let trimmed = active_id.trim();
            if !trimmed.is_empty() && self.try_load_project_from_disk(trimmed).is_ok() {
                self.reload_project_audio_tracks();
                return;
            }
        }

        let mut latest: Option<ModProject> = None;
        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let proj_file = path.join("project.json");
                if let Ok(raw) = std::fs::read_to_string(&proj_file) {
                    if let Ok(proj) = serde_json::from_str::<ModProject>(&raw) {
                        if latest
                            .as_ref()
                            .map(|prev| proj.updated_at_ms > prev.updated_at_ms)
                            .unwrap_or(true)
                        {
                            latest = Some(proj);
                        }
                    }
                }
            }
        }

        if let Some(proj) = latest {
            self.project = proj;
            let _ = std::fs::write(
                self.active_project_marker(),
                sanitize_project_id(&self.project.id).as_bytes(),
            );
            self.reload_project_audio_tracks();
        } else {
            self.save_active_project_now();
        }
    }

    fn try_load_project_from_disk(&mut self, project_id: &str) -> Result<(), String> {
        let proj_file = self.project_dir_for(project_id).join("project.json");
        let raw = std::fs::read_to_string(&proj_file)
            .map_err(|e| format!("No se pudo leer el proyecto: {}", e))?;
        let proj = serde_json::from_str::<ModProject>(&raw)
            .map_err(|e| format!("Proyecto con formato inválido: {}", e))?;
        self.project = proj;
        Ok(())
    }

    /// Re-decodifica las pistas de audio guardadas del proyecto actual hacia `pulse_audio`.
    pub fn reload_project_audio_tracks(&mut self) {
        self.audio_engine.mixer.pause();
        for slot in 0..MAX_AUDIO_TRACKS {
            self.decoded_tracks[slot] = None;
            self.audio_engine.mixer.set_track(slot, None);
        }

        let has_any_file = self.project.audio_tracks.iter().any(|t| t.file_path.is_some());
        if has_any_file {
            self.audio_engine.ensure_started();
        }
        let target_sr = self.audio_engine.mixer.sample_rate.load(Ordering::Relaxed);

        for slot in 0..MAX_AUDIO_TRACKS {
            let cfg = &self.project.audio_tracks[slot];
            self.audio_engine.mixer.configs[slot].set_gain(cfg.gain);
            self.audio_engine.mixer.configs[slot]
                .muted
                .store(cfg.muted, Ordering::Relaxed);
            self.audio_engine.mixer.configs[slot]
                .duck_on_miss
                .store(cfg.duck_on_player_miss, Ordering::Relaxed);

            if let Some(path) = &cfg.file_path {
                if path.exists() {
                    if let Ok(decoded) = decode_audio_file(path, target_sr) {
                        let arc_track = Arc::new(decoded);
                        self.audio_engine
                            .mixer
                            .set_track(slot, Some(Arc::clone(&arc_track)));
                        self.decoded_tracks[slot] = Some(arc_track);
                    }
                }
            }
        }
    }

    pub fn list_projects_json(&self) -> String {
        let root = self.projects_root_dir();
        let mut summaries: Vec<ProjectSummaryJson> = Vec::new();
        let active_id = sanitize_project_id(&self.project.id);

        if let Ok(entries) = std::fs::read_dir(&root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let proj_file = path.join("project.json");
                let Ok(raw) = std::fs::read_to_string(&proj_file) else {
                    continue;
                };
                let Ok(proj) = serde_json::from_str::<ModProject>(&raw) else {
                    continue;
                };
                let sid = sanitize_project_id(&proj.id);
                summaries.push(ProjectSummaryJson {
                    id: sid.clone(),
                    name: proj.name.clone(),
                    author: proj.author.clone(),
                    chart_title: proj.chart.title.clone(),
                    bpm: proj.chart.bpm,
                    note_count: proj.chart.stats.total_notes,
                    track_count: proj.active_audio_count(),
                    entity_count: proj.stage.entities.len(),
                    script_count: proj.scripts.len(),
                    updated_at_ms: proj.updated_at_ms,
                    is_active: sid == active_id,
                });
            }
        }

        if summaries.is_empty() {
            summaries.push(ProjectSummaryJson {
                id: active_id,
                name: self.project.name.clone(),
                author: self.project.author.clone(),
                chart_title: self.project.chart.title.clone(),
                bpm: self.project.chart.bpm,
                note_count: self.project.chart.stats.total_notes,
                track_count: self.project.active_audio_count(),
                entity_count: self.project.stage.entities.len(),
                script_count: self.project.scripts.len(),
                updated_at_ms: self.project.updated_at_ms,
                is_active: true,
            });
        }

        summaries.sort_by(|a, b| {
            b.is_active
                .cmp(&a.is_active)
                .then_with(|| b.updated_at_ms.cmp(&a.updated_at_ms))
        });
        serde_json::to_string(&summaries).unwrap_or_else(|_| "[]".to_string())
    }

    pub fn create_project(&mut self, name: &str, author: &str) -> String {
        self.flush_pending_save();
        self.gameplay = None;
        self.compiled_bundle = None;
        self.compiled_texture_paths.clear();
        self.last_compile_report = None;

        let clean_name = if name.trim().is_empty() {
            "Nuevo Mod BeatForge".to_string()
        } else {
            name.trim().to_string()
        };
        let clean_author = if author.trim().is_empty() {
            "Creador Local".to_string()
        } else {
            author.trim().to_string()
        };
        let new_id = format!("proj_{}", current_epoch_ms());
        self.project = ModProject::new_blank(new_id, clean_name.clone(), clean_author);
        self.reload_project_audio_tracks();
        self.save_active_project_now();
        self.ok_response(format!("Proyecto '{}' creado y guardado", clean_name))
    }

    pub fn open_project(&mut self, project_id: &str) -> String {
        self.flush_pending_save();
        self.gameplay = None;
        self.compiled_bundle = None;
        self.compiled_texture_paths.clear();
        self.last_compile_report = None;

        if let Err(err) = self.try_load_project_from_disk(project_id) {
            return self.err_response(err);
        }
        let _ = std::fs::write(
            self.active_project_marker(),
            sanitize_project_id(&self.project.id).as_bytes(),
        );
        self.reload_project_audio_tracks();
        let name = self.project.name.clone();
        self.ok_response(format!("Proyecto '{}' abierto", name))
    }

    pub fn duplicate_project(&mut self, project_id: &str) -> String {
        self.flush_pending_save();
        let target_id = if project_id.trim().is_empty() {
            self.project.id.clone()
        } else {
            project_id.to_string()
        };

        let proj_file = self.project_dir_for(&target_id).join("project.json");
        let mut cloned = if let Ok(raw) = std::fs::read_to_string(&proj_file) {
            match serde_json::from_str::<ModProject>(&raw) {
                Ok(p) => p,
                Err(e) => return self.err_response(format!("No se pudo duplicar: {}", e)),
            }
        } else if sanitize_project_id(&self.project.id) == sanitize_project_id(&target_id) {
            self.project.clone()
        } else {
            return self.err_response("Proyecto a duplicar no encontrado");
        };

        let old_tex_dir = self.project_dir_for(&target_id).join("textures");
        let new_id = format!("proj_{}", current_epoch_ms());
        let new_tex_dir = self.project_dir_for(&new_id).join("textures");
        if old_tex_dir.exists() {
            for sub in ["decoded_textures", "animate_baked"] {
                let src_sub = old_tex_dir.join(sub);
                if let Ok(entries) = std::fs::read_dir(&src_sub) {
                    let dst_sub = new_tex_dir.join(sub);
                    let _ = std::fs::create_dir_all(&dst_sub);
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_file() {
                            if let Some(fname) = p.file_name() {
                                let _ = std::fs::copy(&p, dst_sub.join(fname));
                            }
                        }
                    }
                }
            }
            for ent in &mut cloned.stage.entities {
                if let Some(img_p) = &ent.image_path {
                    if let Ok(rel) = img_p.strip_prefix(&old_tex_dir) {
                        ent.image_path = Some(new_tex_dir.join(rel));
                    }
                }
            }
        }

        cloned.id = new_id.clone();
        cloned.name = format!("{} (Copia)", cloned.name);
        cloned.updated_at_ms = current_epoch_ms();

        self.gameplay = None;
        self.compiled_bundle = None;
        self.compiled_texture_paths.clear();
        self.last_compile_report = None;
        self.project = cloned;
        self.reload_project_audio_tracks();
        self.save_active_project_now();
        self.ok_response(format!("Proyecto duplicado: '{}'", self.project.name))
    }

    pub fn delete_project(&mut self, project_id: &str) -> String {
        let target_safe = sanitize_project_id(project_id);
        let active_safe = sanitize_project_id(&self.project.id);
        let target_dir = self.project_dir_for(&target_safe);

        if target_dir.exists() {
            if let Err(e) = std::fs::remove_dir_all(&target_dir) {
                return self.err_response(format!("Error eliminando proyecto: {}", e));
            }
        }

        if target_safe == active_safe {
            self.gameplay = None;
            self.compiled_bundle = None;
            self.compiled_texture_paths.clear();
            self.last_compile_report = None;

            // Buscar otro proyecto guardado o crear uno limpio si era el último
            let mut replacement: Option<ModProject> = None;
            if let Ok(entries) = std::fs::read_dir(self.projects_root_dir()) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if !p.is_dir() {
                        continue;
                    }
                    if let Ok(raw) = std::fs::read_to_string(p.join("project.json")) {
                        if let Ok(proj) = serde_json::from_str::<ModProject>(&raw) {
                            if replacement
                                .as_ref()
                                .map(|r| proj.updated_at_ms > r.updated_at_ms)
                                .unwrap_or(true)
                            {
                                replacement = Some(proj);
                            }
                        }
                    }
                }
            }

            self.project = replacement.unwrap_or_else(ModProject::new_demo);
            self.reload_project_audio_tracks();
            self.save_active_project_now();
        }

        self.ok_response("Proyecto eliminado")
    }
}
