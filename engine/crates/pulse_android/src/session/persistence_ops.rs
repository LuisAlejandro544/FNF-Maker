//! Archivo: engine/crates/pulse_android/src/session/persistence_ops.rs
//! Propósito: Persistencia automática en almacenamiento interno (`filesDir/projects/<id>/project.json`)
//! con escritura atómica (`.tmp` -> `rename`), adopción y recolección de basura de assets binarios
//! por proyecto (`filesDir/projects/<id>/assets/`), recarga acelerada con caché `.bfaud` en `cacheDir`
//! y limpieza de cachés multi-proyecto al eliminar o cambiar de proyecto.

//! - `persistence_ops`: Persistencia automática instantánea en `filesDir/projects/<id>/project.json`,
//! limpieza LRU de cachés multi-proyecto y empaquetado portátil **Vanilla (`.mod`)** con Zstd.

use super::{ActionResultJson, EngineSession, ProjectSummaryJson};
use pulse_audio::decode_audio_file_cached;
use pulse_core::disk_cache::{cleanup_legacy_flat_compiled_cache, touch_dir_last_used};
use pulse_core::project::{MAX_AUDIO_TRACKS, ModProject, current_epoch_ms};
use pulse_core::vanilla::{export_project_vanilla_mod, import_project_vanilla_mod};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

const AUTO_SAVE_THROTTLE_MS: u64 = 250;

fn normalize_loaded_project(proj: &mut ModProject) {
    proj.ensure_camera_script();
    proj.ensure_stage_events_script();
    proj.ensure_ui_blackout_script();
    for ent in &mut proj.stage.entities {
        if let Some(atlas) = ent.atlas.as_mut() {
            atlas.normalize_and_center_animations();
        }
    }
}

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

fn sanitize_asset_stem(raw: &str) -> String {
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
        "asset".to_string()
    } else {
        trimmed.chars().take(40).collect()
    }
}

fn normalize_path_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn collect_project_binary_paths(project: &ModProject, out: &mut HashSet<PathBuf>) {
    for track in &project.audio_tracks {
        if let Some(p) = &track.file_path {
            out.insert(normalize_path_key(p));
        }
    }
    for ent in &project.stage.entities {
        if let Some(p) = &ent.image_path {
            out.insert(normalize_path_key(p));
        }
        if let Some(p) = &ent.raw_image_path {
            out.insert(normalize_path_key(p));
        }
        if let Some(p) = &ent.icon_normal_path {
            out.insert(normalize_path_key(p));
        }
        if let Some(p) = &ent.icon_losing_path {
            out.insert(normalize_path_key(p));
        }
    }
}

fn rewrite_prefix_if_matched(
    slot: &mut Option<PathBuf>,
    old_root: &Path,
    new_root: &Path,
) {
    if let Some(cur) = slot.as_ref() {
        if let Ok(rel) = cur.strip_prefix(old_root) {
            *slot = Some(new_root.join(rel));
        }
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

    pub fn project_assets_dir_for(&self, project_id: &str) -> PathBuf {
        self.project_dir_for(project_id).join("assets")
    }

    pub fn project_assets_dir(&self) -> PathBuf {
        self.project_assets_dir_for(&self.project.id)
    }

    fn legacy_mod_imports_dir(&self) -> PathBuf {
        self.data_dir.join("mod_imports")
    }

    fn active_project_marker(&self) -> PathBuf {
        self.projects_root_dir().join("active_project.txt")
    }

    /// Adopta un asset binario permanente (audio, sprite `.png`/`.astc`, ícono) desde staging
    /// hacia `filesDir/projects/<id>/assets/<subdir>/` con nombre único y seguro.
    pub(super) fn adopt_permanent_asset(
        &self,
        src_path: &Path,
        subdir: &str,
        stem_hint: &str,
    ) -> Result<PathBuf, String> {
        let assets_dir = self.project_assets_dir();
        if src_path.starts_with(&assets_dir) && src_path.is_file() {
            return Ok(src_path.to_path_buf());
        }

        let target_dir = assets_dir.join(subdir);
        std::fs::create_dir_all(&target_dir)
            .map_err(|e| format!("Error creando carpeta de assets: {}", e))?;

        let ext = src_path
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| !e.trim().is_empty())
            .unwrap_or("bin");
        let clean_stem = sanitize_asset_stem(stem_hint);
        let stamp = current_epoch_ms();
        let mut dest = target_dir.join(format!("{}_{}.{}", clean_stem, stamp, ext));
        let mut suffix = 1_u32;
        while dest.exists() {
            dest = target_dir.join(format!("{}_{}_{}.{}", clean_stem, stamp, suffix, ext));
            suffix += 1;
        }

        std::fs::copy(src_path, &dest)
            .map_err(|e| format!("Error copiando asset al proyecto: {}", e))?;
        Ok(dest)
    }

    fn collect_all_referenced_binary_paths(&self) -> HashSet<PathBuf> {
        let mut refs = HashSet::new();
        collect_project_binary_paths(&self.project, &mut refs);
        if let Ok(entries) = std::fs::read_dir(self.projects_root_dir()) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                if let Ok(raw) = std::fs::read_to_string(path.join("project.json")) {
                    if let Ok(proj) = serde_json::from_str::<ModProject>(&raw) {
                        collect_project_binary_paths(&proj, &mut refs);
                    }
                }
            }
        }
        refs
    }

    /// Elimina un asset antiguo de `assets/`, `textures/` o `mod_imports/` únicamente
    /// cuando deja de estar referenciado por las entidades/pistas activas o por otros proyectos.
    pub(super) fn remove_managed_asset_if_unreferenced(&self, old_path: Option<&Path>) {
        let Some(path) = old_path else {
            return;
        };
        if !path.is_file() {
            return;
        }
        let assets_dir = self.project_assets_dir();
        let textures_dir = self.project_textures_dir();
        let legacy_dir = self.legacy_mod_imports_dir();

        let in_project_storage = path.starts_with(&assets_dir) || path.starts_with(&textures_dir);
        let in_legacy_imports = path.starts_with(&legacy_dir);
        if !in_project_storage && !in_legacy_imports {
            return;
        }

        let norm = normalize_path_key(path);
        if in_legacy_imports {
            let all_refs = self.collect_all_referenced_binary_paths();
            if !all_refs.contains(&norm) {
                let _ = std::fs::remove_file(path);
            }
        } else {
            let mut active_refs = HashSet::new();
            collect_project_binary_paths(&self.project, &mut active_refs);
            if !active_refs.contains(&norm) {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    /// Recolecta y limpia archivos huérfanos en `filesDir/mod_imports/` (incluidos charts, XML,
    /// scripts y SVG de versiones previas) así como archivos huérfanos del proyecto activo.
    pub fn prune_unreferenced_user_storage(&self) {
        let all_refs = self.collect_all_referenced_binary_paths();
        let legacy_dir = self.legacy_mod_imports_dir();
        if legacy_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&legacy_dir) {
                let mut remaining = 0_usize;
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        let norm = normalize_path_key(&p);
                        if !all_refs.contains(&norm) {
                            let _ = std::fs::remove_file(&p);
                        } else {
                            remaining += 1;
                        }
                    }
                }
                if remaining == 0 {
                    let _ = std::fs::remove_dir(&legacy_dir);
                }
            }
        }

        let mut active_refs = HashSet::new();
        collect_project_binary_paths(&self.project, &mut active_refs);

        for sub in ["audio", "sprites", "icons"] {
            let dir = self.project_assets_dir().join(sub);
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() && !active_refs.contains(&normalize_path_key(&p)) {
                        let _ = std::fs::remove_file(&p);
                    }
                }
            }
        }
        for sub in ["decoded_textures", "animate_baked"] {
            let dir = self.project_textures_dir().join(sub);
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() && !active_refs.contains(&normalize_path_key(&p)) {
                        let _ = std::fs::remove_file(&p);
                    }
                }
            }
        }
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
        if std::fs::write(&tmp, &bytes).is_ok() && std::fs::rename(&tmp, &target).is_err() {
            let _ = std::fs::write(&target, &bytes);
            let _ = std::fs::remove_file(&tmp);
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
        cleanup_legacy_flat_compiled_cache(&self.compiled_cache_root_dir());

        if let Ok(active_id) = std::fs::read_to_string(self.active_project_marker()) {
            let trimmed = active_id.trim();
            if !trimmed.is_empty() && self.try_load_project_from_disk(trimmed).is_ok() {
                let comp_dir = self.active_compiled_cache_dir();
                if comp_dir.is_dir() {
                    touch_dir_last_used(&comp_dir);
                }
                self.prune_unreferenced_user_storage();
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

        if let Some(mut proj) = latest {
            normalize_loaded_project(&mut proj);
            self.project = proj;
            let _ = std::fs::write(
                self.active_project_marker(),
                sanitize_project_id(&self.project.id).as_bytes(),
            );
            let comp_dir = self.active_compiled_cache_dir();
            if comp_dir.is_dir() {
                touch_dir_last_used(&comp_dir);
            }
            self.prune_unreferenced_user_storage();
            self.reload_project_audio_tracks();
        } else {
            self.save_active_project_now();
            self.prune_unreferenced_user_storage();
        }
    }

    fn try_load_project_from_disk(&mut self, project_id: &str) -> Result<(), String> {
        let proj_file = self.project_dir_for(project_id).join("project.json");
        let raw = std::fs::read_to_string(&proj_file)
            .map_err(|e| format!("No se pudo leer el proyecto: {}", e))?;
        let mut proj = serde_json::from_str::<ModProject>(&raw)
            .map_err(|e| format!("Proyecto con formato inválido: {}", e))?;
        normalize_loaded_project(&mut proj);
        self.project = proj;
        Ok(())
    }

    /// Carga o recarga las pistas de audio del proyecto usando caché binaria `.bfaud` en `cacheDir/audio_cache/`
    /// para abrir proyectos y regresar de gameplay casi al instante sin re-decodificar OGG/MP3.
    pub fn reload_project_audio_tracks(&mut self) {
        self.audio_engine.mixer.pause();
        self.audio_engine.mixer.clear_stream_source();
        self.compiled_audio_path = None;
        for slot in 0..MAX_AUDIO_TRACKS {
            self.decoded_tracks[slot] = None;
            self.audio_engine.mixer.set_track(slot, None);
        }

        let has_any_file = self.project.audio_tracks.iter().any(|t| t.file_path.is_some());
        if has_any_file {
            self.audio_engine.ensure_started();
        }
        let target_sr = self
            .audio_engine
            .mixer
            .sample_rate
            .load(Ordering::Relaxed)
            .max(8000);
        let audio_cache_dir = self.audio_cache_dir();
        let audio_quota = self.cache_budget().audio_cache_bytes;

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
                    if let Ok(decoded) = decode_audio_file_cached(
                        path,
                        target_sr,
                        Some(&audio_cache_dir),
                        audio_quota,
                    ) {
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
        let comp_dir = self.active_compiled_cache_dir();
        if comp_dir.is_dir() {
            touch_dir_last_used(&comp_dir);
        }
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

        let old_proj_dir = self.project_dir_for(&target_id);
        let old_tex_dir = old_proj_dir.join("textures");
        let old_assets_dir = old_proj_dir.join("assets");
        let new_id = format!("proj_{}", current_epoch_ms());
        let new_proj_dir = self.project_dir_for(&new_id);
        let new_tex_dir = new_proj_dir.join("textures");
        let new_assets_dir = new_proj_dir.join("assets");

        if old_tex_dir.exists() {
            for sub in ["decoded_textures", "animate_baked"] {
                copy_flat_dir_files(&old_tex_dir.join(sub), &new_tex_dir.join(sub));
            }
        }
        if old_assets_dir.exists() {
            for sub in ["audio", "sprites", "icons"] {
                copy_flat_dir_files(&old_assets_dir.join(sub), &new_assets_dir.join(sub));
            }
        }

        for track in &mut cloned.audio_tracks {
            rewrite_prefix_if_matched(&mut track.file_path, &old_assets_dir, &new_assets_dir);
        }
        for ent in &mut cloned.stage.entities {
            rewrite_prefix_if_matched(&mut ent.image_path, &old_tex_dir, &new_tex_dir);
            rewrite_prefix_if_matched(&mut ent.image_path, &old_assets_dir, &new_assets_dir);
            rewrite_prefix_if_matched(&mut ent.raw_image_path, &old_assets_dir, &new_assets_dir);
            rewrite_prefix_if_matched(&mut ent.icon_normal_path, &old_assets_dir, &new_assets_dir);
            rewrite_prefix_if_matched(&mut ent.icon_losing_path, &old_assets_dir, &new_assets_dir);
        }

        cloned.id = new_id.clone();
        cloned.name = format!("{} (Copia)", cloned.name);
        cloned.updated_at_ms = current_epoch_ms();
        cloned.ensure_camera_script();
        cloned.ensure_stage_events_script();
        cloned.ensure_ui_blackout_script();

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
        let target_cache_dir = self.compiled_cache_dir_for(&target_safe);
        if target_cache_dir.exists() {
            let _ = std::fs::remove_dir_all(&target_cache_dir);
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

        self.prune_unreferenced_user_storage();
        self.ok_response("Proyecto eliminado")
    }

    /// Exporta cualquier proyecto guardado (o el activo) al formato contenedor **Vanilla** (`.mod`)
    /// apoyado en compresión Zstandard y conversión inteligente PNG -> WebP sin pérdida.
    pub fn export_project_vanilla_mod(
        &mut self,
        project_id: &str,
        out_path: &Path,
        optimize_png_to_webp: bool,
    ) -> String {
        self.flush_pending_save();

        let target_safe = if project_id.trim().is_empty() {
            sanitize_project_id(&self.project.id)
        } else {
            sanitize_project_id(project_id)
        };
        let active_safe = sanitize_project_id(&self.project.id);

        let target_proj = if target_safe == active_safe {
            self.project.clone()
        } else {
            let proj_file = self.project_dir_for(&target_safe).join("project.json");
            let Ok(raw) = std::fs::read_to_string(&proj_file) else {
                return self.err_response("No se encontró el proyecto a exportar");
            };
            match serde_json::from_str::<ModProject>(&raw) {
                Ok(p) => p,
                Err(e) => {
                    return self.err_response(format!("Proyecto inválido para exportar: {}", e));
                }
            }
        };

        match export_project_vanilla_mod(&target_proj, out_path, optimize_png_to_webp) {
            Ok(report) => {
                let src_mb = report.source_bytes as f64 / (1024.0 * 1024.0);
                let out_mb = report.archive_bytes as f64 / (1024.0 * 1024.0);
                let webp_note = if report.png_converted_count > 0 {
                    format!(" · {} img WebP lossless", report.png_converted_count)
                } else {
                    String::new()
                };
                let msg = format!(
                    "Vanilla .mod listo: {:.2} MB -> {:.2} MB (-{:.0}%{})",
                    src_mb,
                    out_mb,
                    report.ratio_percent.max(0.0),
                    webp_note
                );
                let res = ActionResultJson::<()> {
                    ok: true,
                    message: msg,
                    data: None,
                };
                serde_json::to_string(&res)
                    .unwrap_or_else(|_| "{\"ok\":true,\"message\":\"Exportado\"}".to_string())
            }
            Err(err) => self.err_response(format!("Error exportando Vanilla .mod: {}", err)),
        }
    }

    /// Importa un archivo `.mod` (contenedor Vanilla + Zstd), verifica checksums FNV-1a,
    /// instala el proyecto en una carpeta aislada y lo abre como proyecto activo.
    pub fn import_project_vanilla_mod(&mut self, mod_path: PathBuf) -> String {
        if !mod_path.is_file() {
            return self.err_response("Archivo .mod no encontrado");
        }
        self.flush_pending_save();

        let base_stamp = current_epoch_ms();
        let mut new_id = format!("proj_{}", base_stamp);
        let mut suffix = 1_u32;
        while self.project_dir_for(&new_id).exists() {
            new_id = format!("proj_{}_{}", base_stamp, suffix);
            suffix += 1;
        }
        let target_dir = self.project_dir_for(&new_id);

        let imported = match import_project_vanilla_mod(&mod_path, &new_id, &target_dir) {
            Ok(res) => res,
            Err(err) => return self.err_response(format!("Error importando .mod: {}", err)),
        };

        self.gameplay = None;
        self.compiled_bundle = None;
        self.compiled_texture_paths.clear();
        self.last_compile_report = None;
        let mut imported_proj = imported.project;
        normalize_loaded_project(&mut imported_proj);
        self.project = imported_proj;
        self.dirty = false;
        self.last_save_at = Instant::now();

        let _ = std::fs::write(
            self.active_project_marker(),
            sanitize_project_id(&self.project.id).as_bytes(),
        );
        self.reload_project_audio_tracks();

        let name = self.project.name.clone();
        self.ok_response(format!(
            "Mod '{}' importado desde Vanilla .mod ({} archivos)",
            name, imported.extracted_files
        ))
    }
}

fn copy_flat_dir_files(src: &Path, dst: &Path) {
    if let Ok(entries) = std::fs::read_dir(src) {
        let _ = std::fs::create_dir_all(dst);
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                if let Some(fname) = p.file_name() {
                    let _ = std::fs::copy(&p, dst.join(fname));
                }
            }
        }
    }
}
