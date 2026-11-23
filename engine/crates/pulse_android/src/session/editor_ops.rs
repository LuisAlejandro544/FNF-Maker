//! Archivo: engine/crates/pulse_android/src/session/editor_ops.rs
//! Propósito: Operaciones de edición del proyecto en `EngineSession`:
//! - Importación, edición y exportación completa de `chart.json`.
//! - Decodificación y configuración de hasta 3 pistas de audio en `pulse_audio`.
//! - Gestión de entidades del escenario (`StageEntity`), scripts Lua/TS/JS y skins SVG (`resvg`).

use super::{
    ActionResultJson, EngineSession, friendly_import_name, parse_control_mode_key,
    parse_lang_key, parse_role_key,
};
use image::ImageFormat;
use pulse_audio::decode_audio_file_cached;
use pulse_core::mod_descriptors::{normalize_stage_world_pos, parse_stage_descriptor_str};
use pulse_core::project::{
    CAMERA_SCRIPT_ID, CAMERA_SCRIPT_NAME, MAX_AUDIO_TRACKS, parse_camera_config_from_lua,
    render_camera_lua_script,
};
use pulse_core::svg_skin::{
    ArrowSkinSet, default_hurt_arrow_x_svg, import_arrow_sparrow_atlas, rasterize_svg_str,
};
use pulse_core::{
    CameraFollowMode, CameraTargetKind, ChartData, EntityRole, NoteDirection, ScriptAsset,
    ScriptLanguage, StageCameraConfig, StageEntity,
};
use pulse_scripting::validate_script_syntax;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

impl EngineSession {
    pub fn update_project_meta(
        &mut self,
        name: String,
        author: String,
        downscroll: bool,
        control_mode_key: &str,
        audio_offset_ms: f64,
        scroll_speed: f64,
    ) -> String {
        self.project.name = name;
        self.project.author = author;
        self.project.downscroll = downscroll;
        self.project.control_mode = parse_control_mode_key(control_mode_key);
        self.project.audio_offset_ms = audio_offset_ms.clamp(-500.0, 500.0);
        self.project.chart.speed = scroll_speed.clamp(0.5, 5.0);
        if let Some(gp) = self.gameplay.as_mut() {
            gp.hud_revision = gp.hud_revision.wrapping_add(1);
        }
        self.invalidate_compiled_bundle();
        self.mark_dirty_and_maybe_save(false);
        self.ok_response("Proyecto actualizado")
    }

    pub fn import_chart(&mut self, path: PathBuf) -> String {
        match std::fs::read_to_string(&path) {
            Ok(raw) => match ChartData::from_json_str(&raw) {
                Ok(chart) => {
                    let msg = format!(
                        "Chart '{}' importado ({} notas, {:.1} BPM)",
                        chart.title, chart.stats.total_notes, chart.bpm
                    );
                    let display_name = friendly_import_name(&path);
                    self.project.chart = chart;
                    self.project.chart_source_path = Some(PathBuf::from(display_name));
                    self.project.sync_stage_events_script();
                    self.project.sync_ui_blackout_script();
                    self.invalidate_compiled_bundle();
                    self.sync_mixer_chart_duration();
                    self.save_active_project_now();
                    self.ok_response(msg)
                }
                Err(e) => self.err_response(format!("Chart inválido: {}", e)),
            },
            Err(e) => self.err_response(format!("No se pudo leer el chart: {}", e)),
        }
    }

    pub fn export_chart_json(&self) -> String {
        self.project.chart.to_json_str()
    }

    pub fn set_chart_bpm(&mut self, bpm: f64) -> String {
        self.project.chart.set_bpm(bpm);
        self.invalidate_compiled_bundle();
        self.sync_mixer_chart_duration();
        self.save_active_project_now();
        self.ok_response(format!("BPM actualizado a {:.1}", self.project.chart.bpm))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_chart_details(
        &mut self,
        title: &str,
        speed: f64,
        player1: &str,
        player2: &str,
        gf_version: &str,
        stage: &str,
        needs_voices: bool,
    ) -> String {
        self.project
            .chart
            .set_metadata(title, speed, player1, player2, gf_version, stage, needs_voices);
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Metadatos completos de chart.json actualizados")
    }

    /// Actualiza la configuración inicial de pantalla negra / ocultar HUD / ocultar flechas
    /// declarada directamente en `chart.json` y sincroniza `ui_blackout.lua`.
    #[allow(clippy::too_many_arguments)]
    pub fn update_chart_intro_ui(
        &mut self,
        start_blackout: bool,
        blackout_alpha: f32,
        hide_hud: bool,
        hide_arrows: bool,
        reveal_on_song_start: bool,
        reveal_delay_ms: f64,
        fade_sec: f32,
    ) -> String {
        self.project.chart.set_intro_ui(pulse_core::ChartIntroUiConfig {
            start_blackout,
            blackout_alpha,
            hide_hud,
            hide_arrows,
            reveal_on_song_start,
            reveal_delay_ms,
            fade_sec,
        });
        self.project.sync_ui_blackout_script();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Configuración inicial de pantalla negra, UI y flechas actualizada")
    }

    pub fn set_section_bpm(&mut self, sec_index: usize, bpm: f64, change_bpm: bool) -> String {
        self.project.chart.set_section_bpm(sec_index, bpm, change_bpm);
        self.invalidate_compiled_bundle();
        self.sync_mixer_chart_duration();
        self.save_active_project_now();
        self.ok_response(format!("BPM de sección {} actualizado", sec_index + 1))
    }

    pub fn add_chart_event(&mut self, time_ms: f64, name: &str, value1: &str, value2: &str) -> String {
        self.project.chart.add_event(time_ms, name, value1, value2);
        self.project.sync_stage_events_script();
        self.project.sync_ui_blackout_script();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Evento de chart.json y scripts de escena/UI actualizados")
    }

    pub fn remove_chart_event(&mut self, index: usize) -> String {
        self.project.chart.remove_event(index);
        self.project.sync_stage_events_script();
        self.project.sync_ui_blackout_script();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Evento eliminado y scripts de escena/UI actualizados")
    }

    pub fn add_chart_section(&mut self, must_hit: bool) -> String {
        let idx = self.project.chart.add_section(must_hit);
        self.invalidate_compiled_bundle();
        self.sync_mixer_chart_duration();
        self.save_active_project_now();
        self.ok_response(format!("Sección {} añadida", idx + 1))
    }

    pub fn toggle_section_must_hit(&mut self, sec_index: usize) -> String {
        self.project.chart.toggle_section_must_hit(sec_index);
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Foco de sección actualizado")
    }

    pub fn set_section_camera_slot(&mut self, sec_index: usize, camera_slot: i32) -> String {
        let slot_opt = if camera_slot < 0 {
            None
        } else {
            Some(camera_slot as usize)
        };
        self.project.chart.set_section_camera_slot(sec_index, slot_opt);
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        let label = match slot_opt {
            None => "Automático (mustHit / gfSection)".to_string(),
            Some(s) => format!("Cam {}", s),
        };
        self.ok_response(format!("Sección {} asignada a {}", sec_index + 1, label))
    }

    pub fn toggle_chart_note(
        &mut self,
        sec_index: usize,
        step_in_sec: u32,
        col_0_to_7: u8,
        sustain_steps: u32,
    ) -> String {
        self.toggle_chart_note_with_type(sec_index, step_in_sec, col_0_to_7, sustain_steps, "default")
    }

    pub fn toggle_chart_note_with_type(
        &mut self,
        sec_index: usize,
        step_in_sec: u32,
        col_0_to_7: u8,
        sustain_steps: u32,
        note_type: &str,
    ) -> String {
        self.project.chart.toggle_note_with_type(
            sec_index,
            step_in_sec,
            col_0_to_7,
            sustain_steps,
            note_type,
        );
        self.invalidate_compiled_bundle();
        self.sync_mixer_chart_duration();
        self.save_active_project_now();
        self.ok_response("Nota actualizada")
    }

    pub fn import_audio_track(&mut self, slot: usize, path: PathBuf) -> String {
        if slot >= MAX_AUDIO_TRACKS {
            return self.err_response(format!(
                "Máximo {} pistas permitidas; índice {} fuera de rango",
                MAX_AUDIO_TRACKS, slot
            ));
        }
        if !path.is_file() {
            return self.err_response("Archivo de audio no encontrado");
        }

        let friendly_name = friendly_import_name(&path);
        let stem_hint = PathBuf::from(&friendly_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("track")
            .to_string();

        let adopted_path = match self.adopt_permanent_asset(
            &path,
            "audio",
            &format!("slot{}_{}", slot, stem_hint),
        ) {
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
                let old_path = self.project.audio_tracks[slot].file_path.take();
                self.project.audio_tracks[slot].file_path = Some(adopted_path);
                self.project.audio_tracks[slot].file_name = Some(friendly_name);
                self.remove_managed_asset_if_unreferenced(old_path.as_deref());

                let arc_track = Arc::new(decoded);
                self.audio_engine
                    .mixer
                    .set_track(slot, Some(Arc::clone(&arc_track)));
                self.decoded_tracks[slot] = Some(arc_track);
                self.invalidate_compiled_bundle();
                self.invalidate_adapted_audio();
                self.save_active_project_now();
                self.ok_response(format!("Pista {} decodificada y lista en Rust AAudio", slot + 1))
            }
            Err(e) => {
                let _ = std::fs::remove_file(&adopted_path);
                self.err_response(format!("Error decodificando pista {}: {}", slot + 1, e))
            }
        }
    }

    pub fn remove_audio_track(&mut self, slot: usize) -> String {
        if slot < MAX_AUDIO_TRACKS {
            let old_path = self.project.audio_tracks[slot].file_path.take();
            self.project.audio_tracks[slot].file_name = None;
            self.decoded_tracks[slot] = None;
            self.audio_engine.mixer.set_track(slot, None);
            self.remove_managed_asset_if_unreferenced(old_path.as_deref());
            self.invalidate_compiled_bundle();
            self.invalidate_adapted_audio();
            self.save_active_project_now();
        }
        self.ok_response(format!("Pista {} liberada", slot + 1))
    }

    pub fn update_audio_track_config(
        &mut self,
        slot: usize,
        gain: f32,
        muted: bool,
        duck_on_miss: bool,
    ) -> String {
        if slot < MAX_AUDIO_TRACKS {
            self.project.audio_tracks[slot].gain = gain.clamp(0.0, 2.0);
            self.project.audio_tracks[slot].muted = muted;
            self.project.audio_tracks[slot].duck_on_player_miss = duck_on_miss;

            self.audio_engine.mixer.configs[slot].set_gain(gain);
            self.audio_engine.mixer.configs[slot]
                .muted
                .store(muted, Ordering::Relaxed);
            self.audio_engine.mixer.configs[slot]
                .duck_on_miss
                .store(duck_on_miss, Ordering::Relaxed);
            self.invalidate_compiled_bundle();
            self.invalidate_adapted_audio();
            self.mark_dirty_and_maybe_save(false);
        }
        self.ok_response("Configuración de pista actualizada")
    }

    pub fn set_audio_adapt_enabled(&mut self, enabled: bool) -> String {
        self.project.audio_adapt_enabled = enabled;
        self.invalidate_adapted_audio();
        self.save_active_project_now();
        self.ok_response(if enabled {
            "Adaptador acústico activado para tu teléfono"
        } else {
            "Adaptador acústico desactivado (audio original)"
        })
    }

    pub fn add_stage_entity(&mut self, name: &str, role_key: &str) -> String {
        let idx = self.project.stage.entities.len() + 1;
        let id = format!("asset_{}", idx);
        let role = parse_role_key(role_key);
        let clean_name = if name.trim().is_empty() {
            format!("Objeto {}", idx)
        } else {
            name.trim().to_string()
        };
        let ent = StageEntity::new(id, clean_name, role);
        self.project.stage.entities.push(ent);
        self.project.stage.sort_by_z_order();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Nuevo objeto añadido al escenario")
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_stage_entity(
        &mut self,
        entity_id: &str,
        name: &str,
        role_key: &str,
        dx: f32,
        dy: f32,
        abs_x: f32,
        abs_y: f32,
        use_abs_pos: bool,
        scale: f32,
        opacity: f32,
        visible: bool,
        flip_x: bool,
        commit_save: bool,
    ) -> String {
        if let Some(ent) = self.project.stage.find_mut(entity_id) {
            if !name.trim().is_empty() {
                ent.display_name = name.trim().to_string();
            }
            ent.role = parse_role_key(role_key);
            if use_abs_pos {
                ent.x = abs_x.clamp(-1000.0, 2600.0);
                ent.y = abs_y.clamp(-1000.0, 1800.0);
            } else if dx != 0.0 || dy != 0.0 {
                ent.nudge(dx, dy);
            }
            ent.set_scale(scale);
            ent.set_opacity(opacity);
            ent.visible = visible;
            ent.flip_x = flip_x;
            if commit_save {
                self.invalidate_compiled_bundle();
            }
            self.mark_dirty_and_maybe_save(commit_save);
            if commit_save {
                self.ok_response("Objeto actualizado")
            } else {
                "{\"ok\":true,\"message\":\"Objeto actualizado\",\"data\":null}".to_string()
            }
        } else {
            self.err_response("Objeto no encontrado")
        }
    }

    pub fn reorder_stage_entity(&mut self, entity_id: &str, action: &str) -> String {
        match action {
            "forward" => self.project.stage.bring_forward(entity_id),
            "backward" => self.project.stage.send_backward(entity_id),
            "front" => self.project.stage.bring_to_front(entity_id),
            "back" => self.project.stage.send_to_back(entity_id),
            _ => {}
        }
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Orden Z actualizado")
    }

    fn sync_camera_lua_script(&mut self) {
        self.project.ensure_camera_script();
        let applied_cfg = self.project.stage.camera.clone();
        if let Some(cam_script) = self
            .project
            .scripts
            .iter_mut()
            .find(|s| s.id == CAMERA_SCRIPT_ID || s.name == CAMERA_SCRIPT_NAME)
        {
            cam_script.enabled = applied_cfg.enabled;
            cam_script.source_code =
                render_camera_lua_script(&applied_cfg, Some(&cam_script.source_code));
            cam_script.precompiled_bytecode = None;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_stage_camera(
        &mut self,
        enabled: bool,
        base_x: f32,
        base_y: f32,
        base_zoom: f32,
        follow_mode_key: &str,
        follow_interval_beats: i32,
        smooth_speed: f32,
        pan_strength: f32,
        clamp_to_background: bool,
        commit_save: bool,
    ) -> String {
        let mut cfg = self.project.stage.camera.clone();
        cfg.enabled = enabled;
        cfg.base_x = base_x;
        cfg.base_y = base_y;
        cfg.base_zoom = base_zoom;
        cfg.follow_mode = CameraFollowMode::from_str_loose(follow_mode_key);
        cfg.follow_interval_beats = follow_interval_beats.clamp(1, 32) as u32;
        cfg.smooth_speed = smooth_speed;
        cfg.pan_strength = pan_strength;
        cfg.clamp_to_background = clamp_to_background;
        cfg.sync_slot0_from_base();
        self.project.stage.apply_camera_config(cfg);
        if commit_save {
            self.sync_camera_lua_script();
            self.invalidate_compiled_bundle();
        }

        self.mark_dirty_and_maybe_save(commit_save);
        if commit_save {
            self.ok_response("Cámara y camera_system.lua actualizados")
        } else {
            "{\"ok\":true,\"message\":\"Cámara actualizada\",\"data\":null}".to_string()
        }
    }

    pub fn select_stage_camera(&mut self, slot_index: usize) -> String {
        self.project.stage.camera.ensure_default_cameras();
        let max_idx = self.project.stage.camera.cameras.len().saturating_sub(1);
        let idx = slot_index.min(max_idx);
        self.project.stage.camera.active_camera_index = idx;
        let (rx, ry, rzoom, _) = self.project.stage.resolve_camera_slot_target(idx, "", "");
        self.project.stage.camera_x = rx;
        self.project.stage.camera_y = ry;
        self.project.stage.camera_base_zoom = rzoom;
        self.project.stage.camera_zoom = rzoom;
        self.sync_camera_lua_script();
        self.invalidate_compiled_bundle();
        self.mark_dirty_and_maybe_save(true);
        self.ok_response(format!("Previsualizando Cam {}", idx))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_camera_slot(
        &mut self,
        slot_index: usize,
        name: &str,
        target_kind_key: &str,
        target_entity_id: &str,
        x: f32,
        y: f32,
        zoom: f32,
        smooth_speed: f32,
        pan_strength: f32,
        commit_save: bool,
    ) -> String {
        let kind = CameraTargetKind::from_str_loose(target_kind_key);
        let entity_opt = if target_entity_id.trim().is_empty() {
            None
        } else {
            Some(target_entity_id.trim().to_string())
        };
        if !self.project.stage.update_camera_slot(
            slot_index,
            name,
            kind,
            entity_opt,
            x,
            y,
            zoom,
            smooth_speed,
            pan_strength,
        ) {
            return self.err_response("Ranura de cámara no encontrada");
        }
        if commit_save {
            self.sync_camera_lua_script();
            self.invalidate_compiled_bundle();
        }
        self.mark_dirty_and_maybe_save(commit_save);
        if commit_save {
            self.ok_response(format!("Cam {} actualizada", slot_index))
        } else {
            "{\"ok\":true,\"message\":\"Camara actualizada\",\"data\":null}".to_string()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_custom_camera(
        &mut self,
        name: &str,
        target_kind_key: &str,
        target_entity_id: &str,
        x: f32,
        y: f32,
        zoom: f32,
        smooth_speed: f32,
        pan_strength: f32,
    ) -> String {
        let kind = CameraTargetKind::from_str_loose(target_kind_key);
        let entity_opt = if target_entity_id.trim().is_empty() {
            None
        } else {
            Some(target_entity_id.trim().to_string())
        };
        let new_idx = self.project.stage.add_custom_camera(
            name,
            kind,
            entity_opt,
            x,
            y,
            zoom,
            smooth_speed,
            pan_strength,
        );
        self.sync_camera_lua_script();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response(format!("Nueva cámara Cam {} añadida", new_idx))
    }

    pub fn remove_custom_camera(&mut self, slot_index: usize) -> String {
        if !self.project.stage.remove_custom_camera(slot_index) {
            return self.err_response(
                "Cam 0, Cam 1 y Cam 2 son cámaras base; solo puedes eliminar Cam 3 o superiores",
            );
        }
        for sec in &mut self.project.chart.sections {
            if let Some(s) = sec.camera_slot {
                if s == slot_index {
                    sec.camera_slot = None;
                } else if s > slot_index {
                    sec.camera_slot = Some(s - 1);
                }
            }
        }
        self.sync_camera_lua_script();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response(format!("Cámara Cam {} eliminada", slot_index))
    }

    /// Importa un archivo de escenario (`stages/*.json` o `stages/*.xml`) para ubicar
    /// automáticamente al Player, Enemigo, Cabina/Decoración, zoom inicial y props declarados.
    pub fn import_stage_config(&mut self, path: PathBuf) -> String {
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                return self.err_response(format!("No se pudo leer el archivo de escenario: {}", e));
            }
        };

        let desc = match parse_stage_descriptor_str(&raw) {
            Ok(d) => d,
            Err(e) => return self.err_response(e),
        };

        let mut cam_cfg = self.project.stage.camera.clone();
        if let Some(zoom) = desc.default_zoom {
            cam_cfg.base_zoom = zoom.clamp(0.35, 2.5);
        }
        if let Some([cx, cy]) = desc.camera_start {
            if cx.is_finite() && cy.is_finite() {
                cam_cfg.base_x = cx.clamp(-1000.0, 2600.0);
                cam_cfg.base_y = cy.clamp(-1000.0, 1800.0);
            }
        }
        cam_cfg.sync_slot0_from_base();
        self.project.stage.apply_camera_config(cam_cfg);
        self.project.ensure_camera_script();
        let applied_cfg = self.project.stage.camera.clone();
        if let Some(cam_script) = self
            .project
            .scripts
            .iter_mut()
            .find(|s| s.id == CAMERA_SCRIPT_ID || s.name == CAMERA_SCRIPT_NAME)
        {
            cam_script.enabled = applied_cfg.enabled;
            cam_script.source_code =
                render_camera_lua_script(&applied_cfg, Some(&cam_script.source_code));
            cam_script.precompiled_bytecode = None;
        }

        let mut updated_slots = 0usize;
        let player_world = desc
            .player_pos
            .map(|p| normalize_stage_world_pos(p, EntityRole::Player, desc.is_bottom_center_coords));
        let opp_world = desc.opponent_pos.map(|p| {
            normalize_stage_world_pos(p, EntityRole::Opponent, desc.is_bottom_center_coords)
        });
        let gf_world = desc.gf_pos.map(|p| {
            normalize_stage_world_pos(p, EntityRole::Decoration, desc.is_bottom_center_coords)
        });

        let mut applied_gf = false;
        for ent in &mut self.project.stage.entities {
            match ent.role {
                EntityRole::Player => {
                    if let Some([wx, wy]) = player_world {
                        ent.x = wx;
                        ent.y = wy;
                        if let Some(pending) = &ent.pending_character_desc {
                            if let Some(cpos) = pending.position {
                                if pending.is_relative_position {
                                    ent.x = (ent.x + cpos[0] * 0.55).clamp(-1000.0, 2600.0);
                                    ent.y = (ent.y + cpos[1] * 0.25).clamp(-1000.0, 1800.0);
                                }
                            }
                        }
                        updated_slots += 1;
                    }
                    if let Some(z) = desc.player_z {
                        ent.z_index = z;
                    }
                    if let Some(cam_off) = desc.player_cam_offset {
                        ent.camera_offset = cam_off;
                    }
                }
                EntityRole::Opponent => {
                    if let Some([wx, wy]) = opp_world {
                        ent.x = wx;
                        ent.y = wy;
                        if let Some(pending) = &ent.pending_character_desc {
                            if let Some(cpos) = pending.position {
                                if pending.is_relative_position {
                                    ent.x = (ent.x + cpos[0] * 0.55).clamp(-1000.0, 2600.0);
                                    ent.y = (ent.y + cpos[1] * 0.25).clamp(-1000.0, 1800.0);
                                }
                            }
                        }
                        updated_slots += 1;
                    }
                    if let Some(z) = desc.opponent_z {
                        ent.z_index = z;
                    }
                    if let Some(cam_off) = desc.opponent_cam_offset {
                        ent.camera_offset = cam_off;
                    }
                }
                EntityRole::Decoration if !applied_gf => {
                    let is_gf_slot = ent.id == "stage_speakers"
                        || ent.display_name.to_ascii_lowercase().contains("cabina")
                        || ent.display_name.to_ascii_lowercase().contains("gf")
                        || ent.display_name.to_ascii_lowercase().contains("girlfriend");
                    if is_gf_slot {
                        applied_gf = true;
                        if desc.hide_gf {
                            ent.visible = false;
                        } else if let Some([wx, wy]) = gf_world {
                            ent.x = wx;
                            ent.y = wy;
                            ent.visible = true;
                            updated_slots += 1;
                        }
                        if let Some(z) = desc.gf_z {
                            ent.z_index = z;
                        }
                        if let Some(cam_off) = desc.gf_cam_offset {
                            ent.camera_offset = cam_off;
                        }
                    }
                }
                _ => {}
            }
        }

        let mut added_props = 0usize;
        for (i, prop) in desc.props.iter().enumerate() {
            let role = if i == 0 && prop.scroll_factor <= 0.95 {
                EntityRole::Background
            } else {
                EntityRole::Decoration
            };
            let [wx, wy] = normalize_stage_world_pos(
                prop.position,
                role,
                desc.is_bottom_center_coords,
            );
            let slug: String = prop
                .name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() {
                        c.to_ascii_lowercase()
                    } else {
                        '_'
                    }
                })
                .collect();
            let prop_id = if slug.trim_matches('_').is_empty() {
                format!("stage_prop_{}", i + 1)
            } else {
                format!("stage_prop_{}", slug.trim_matches('_'))
            };

            if let Some(existing) = self
                .project
                .stage
                .entities
                .iter_mut()
                .find(|e| e.id == prop_id || e.display_name.eq_ignore_ascii_case(&prop.name))
            {
                existing.x = wx;
                existing.y = wy;
                existing.set_scale(prop.scale);
                existing.scroll_factor = prop.scroll_factor;
                existing.z_index = prop.z_index;
                existing.flip_x = prop.flip_x;
                existing.set_opacity(prop.alpha);
            } else {
                let mut ent = StageEntity::new(prop_id, prop.name.clone(), role);
                ent.x = wx;
                ent.y = wy;
                ent.set_scale(prop.scale);
                ent.scroll_factor = prop.scroll_factor;
                ent.z_index = prop.z_index;
                ent.flip_x = prop.flip_x;
                ent.set_opacity(prop.alpha);
                self.project.stage.entities.push(ent);
                added_props += 1;
            }
        }

        self.project.stage.sort_by_z_order();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();

        self.ok_response(format!(
            "{} aplicado: zoom {:.2}x, {} posiciones actualizadas, {} props creados",
            desc.format_label, applied_cfg.base_zoom, updated_slots, added_props
        ))
    }

    pub fn add_script(&mut self, lang_key: &str) -> String {
        let lang = parse_lang_key(lang_key);
        let idx = self.project.scripts.len() + 1;
        let id = format!("script_{}", idx);
        let name = format!("custom_{}.{}", idx, lang.extension());
        let template = match lang {
            ScriptLanguage::Lua => {
                "-- Nuevo script Lua 5.4\nfunction onBeatHit(beat)\n    setCameraZoom(getCameraBaseZoom() + 0.03)\nend\n"
            }
            ScriptLanguage::TypeScript => {
                "// Nuevo script TypeScript (oxc)\nfunction onBeatHit(beat: number): void {\n    if (beat % 4 === 0) shakeCamera(4.0);\n}\n"
            }
            ScriptLanguage::JavaScript => {
                "// Nuevo script JavaScript ES2022\nfunction onNoteHit(lane, isPlayer) {\n    if (isPlayer) log('Hit lane ' + lane);\n}\n"
            }
        };
        self.project.scripts.push(ScriptAsset {
            id,
            name,
            language: lang,
            source_code: template.to_string(),
            enabled: true,
            precompiled_js: None,
            precompiled_bytecode: None,
        });
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Script creado")
    }

    pub fn import_script(&mut self, path: PathBuf) -> String {
        match std::fs::read_to_string(&path) {
            Ok(src) => {
                let name = friendly_import_name(&path);
                let lang = if name.ends_with(".ts") {
                    ScriptLanguage::TypeScript
                } else if name.ends_with(".js") {
                    ScriptLanguage::JavaScript
                } else {
                    ScriptLanguage::Lua
                };
                let id = format!("script_{}", self.project.scripts.len() + 1);
                self.project.scripts.push(ScriptAsset {
                    id,
                    name,
                    language: lang,
                    source_code: src,
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                });
                self.invalidate_compiled_bundle();
                self.save_active_project_now();
                self.ok_response("Script comunitario importado")
            }
            Err(e) => self.err_response(format!("No se pudo leer el script: {}", e)),
        }
    }

    pub fn update_script(&mut self, script_id: &str, enabled: bool, source_code: String) -> String {
        let mut is_camera_script = false;
        let mut parsed_cam_cfg: Option<StageCameraConfig> = None;

        if let Some(script) = self.project.scripts.iter_mut().find(|s| s.id == script_id) {
            script.enabled = enabled;
            script.source_code = source_code;
            script.precompiled_bytecode = None;
            script.precompiled_js = None;
            if script.id == CAMERA_SCRIPT_ID || script.name == CAMERA_SCRIPT_NAME {
                is_camera_script = true;
                let mut cfg =
                    parse_camera_config_from_lua(&script.source_code, &self.project.stage.camera);
                cfg.enabled = enabled;
                parsed_cam_cfg = Some(cfg);
            }
        } else {
            return self.err_response("Script no encontrado");
        }

        if is_camera_script {
            if let Some(cfg) = parsed_cam_cfg {
                self.project.stage.apply_camera_config(cfg);
            }
        }

        self.invalidate_compiled_bundle();
        self.mark_dirty_and_maybe_save(true);
        self.ok_response(if is_camera_script {
            "Script camera_system.lua y cámara del escenario sincronizados"
        } else {
            "Script guardado"
        })
    }

    pub fn validate_script(&self, script_id: &str) -> String {
        let Some(script) = self.project.scripts.iter().find(|s| s.id == script_id) else {
            return serde_json::to_string(&ActionResultJson::<String> {
                ok: false,
                message: "Script no encontrado".to_string(),
                data: None,
            })
            .unwrap_or_default();
        };

        match validate_script_syntax(script) {
            Ok(msg) => serde_json::to_string(&ActionResultJson {
                ok: true,
                message: msg,
                data: Some(script.id.clone()),
            })
            .unwrap_or_default(),
            Err(err) => serde_json::to_string(&ActionResultJson::<String> {
                ok: false,
                message: err,
                data: None,
            })
            .unwrap_or_default(),
        }
    }

    pub fn import_arrow_svg(&mut self, dir_code: u8, path: PathBuf) -> String {
        let dir = NoteDirection::from_lane(dir_code);
        match std::fs::read_to_string(&path) {
            Ok(svg_text) => {
                if let Err(e) = rasterize_svg_str(&svg_text, self.project.arrow_skin.note_size_px) {
                    return self.err_response(format!("SVG inválido: {}", e));
                }
                let display_name = friendly_import_name(&path);
                self.project
                    .arrow_skin
                    .custom_svg_paths
                    .insert(dir, PathBuf::from(display_name));
                self.project
                    .arrow_skin
                    .custom_svg_sources
                    .insert(dir, svg_text);
                self.save_active_project_now();
                self.ok_response(format!("Skin SVG actualizada para flecha {}", dir.label_es()))
            }
            Err(e) => self.err_response(format!("No se pudo leer SVG: {}", e)),
        }
    }

    pub fn import_arrow_atlas(&mut self, png_path: PathBuf, xml_path: PathBuf) -> String {
        let png_bytes = match std::fs::read(&png_path) {
            Ok(b) => b,
            Err(e) => return self.err_response(format!("No se pudo leer PNG de flechas: {}", e)),
        };
        let xml_text = match std::fs::read_to_string(&xml_path) {
            Ok(x) => x,
            Err(e) => return self.err_response(format!("No se pudo leer XML Sparrow de flechas: {}", e)),
        };
        let friendly_name = friendly_import_name(&png_path);
        match import_arrow_sparrow_atlas(&png_bytes, &xml_text, &friendly_name) {
            Ok(skin) => {
                let arrows_count = skin.custom_arrow_pngs.len();
                let receptors_count = skin.custom_receptor_pngs.len();
                let has_hurt = skin.custom_hurt_arrow_png.is_some();
                self.project.arrow_skin = skin;
                self.save_active_project_now();
                self.ok_response(format!(
                    "Skin PNG+XML '{}' importada ({} flechas, {} receptores{})",
                    friendly_name,
                    arrows_count,
                    receptors_count,
                    if has_hurt { ", +Nota X" } else { "" }
                ))
            }
            Err(e) => self.err_response(e),
        }
    }

    pub fn restore_default_arrow_svg(&mut self) -> String {
        self.project.arrow_skin = ArrowSkinSet::bundled_default();
        self.save_active_project_now();
        self.ok_response("Skin de flechas SVG originales restaurada")
    }

    pub fn rasterize_arrow_png_bytes(&self, dir_code: u8) -> Vec<u8> {
        let dir = NoteDirection::from_lane(dir_code);
        if let Some(png) = self.project.arrow_skin.custom_arrow_pngs.get(&dir) {
            if !png.is_empty() {
                return png.clone();
            }
        }
        let svg = self.project.arrow_skin.svg_for(dir);
        let Ok(raster) = rasterize_svg_str(&svg, self.project.arrow_skin.note_size_px) else {
            return Vec::new();
        };
        encode_raster_png(raster.width, raster.height, raster.rgba)
    }

    pub fn rasterize_receptor_png_bytes(&self, dir_code: u8) -> Vec<u8> {
        let dir = NoteDirection::from_lane(dir_code);
        if let Some(png) = self.project.arrow_skin.custom_receptor_pngs.get(&dir) {
            if !png.is_empty() {
                return png.clone();
            }
        }
        let Ok(raster) = self.project.arrow_skin.rasterize_neutral_receptor(dir) else {
            return Vec::new();
        };
        encode_raster_png(raster.width, raster.height, raster.rgba)
    }

    pub fn rasterize_hurt_arrow_png_bytes(&self) -> Vec<u8> {
        if let Some(png) = &self.project.arrow_skin.custom_hurt_arrow_png {
            if !png.is_empty() {
                return png.clone();
            }
        }
        let svg: &str = match self.project.arrow_skin.custom_hurt_arrow_svg.as_deref() {
            Some(custom) => custom,
            None => default_hurt_arrow_x_svg(),
        };
        let size = self.project.arrow_skin.note_size_px.max(128);
        let Ok(raster) = rasterize_svg_str(svg, size) else {
            return Vec::new();
        };
        encode_raster_png(raster.width, raster.height, raster.rgba)
    }

    pub fn import_health_bar_svg(&mut self, path: PathBuf) -> String {
        match std::fs::read_to_string(&path) {
            Ok(svg_text) => {
                if let Err(e) = pulse_core::svg_skin::rasterize_svg_rect(&svg_text, 600, 36) {
                    return self.err_response(format!("SVG de barra de vida inválido: {}", e));
                }
                let display_name = friendly_import_name(&path);
                self.project.health_bar_svg_path = Some(PathBuf::from(display_name));
                self.project.health_bar_svg_source = Some(svg_text);
                self.save_active_project_now();
                self.ok_response("Barra de vida SVG personalizada aplicada")
            }
            Err(e) => self.err_response(format!("No se pudo leer SVG de barra: {}", e)),
        }
    }

    pub fn restore_default_health_bar_svg(&mut self) -> String {
        self.project.health_bar_svg_path = None;
        self.project.health_bar_svg_source = None;
        self.save_active_project_now();
        self.ok_response("Barra de vida SVG estilo FNF restaurada")
    }

    pub fn rasterize_health_bar_png_bytes(&self) -> Vec<u8> {
        let svg: &str = match self.project.health_bar_svg_source.as_deref() {
            Some(custom) => custom,
            None => pulse_core::svg_skin::default_health_bar_frame_svg(),
        };
        let Ok(raster) = pulse_core::svg_skin::rasterize_svg_rect(svg, 600, 36) else {
            return Vec::new();
        };
        encode_raster_png(raster.width, raster.height, raster.rgba)
    }

    pub fn rasterize_rating_png_bytes(&self, kind_code: u8) -> Vec<u8> {
        let Some(kind) = pulse_core::svg_skin::RatingKind::from_code(kind_code) else {
            return Vec::new();
        };
        let svg = pulse_core::svg_skin::default_rating_svg(kind);
        let Ok(raster) = pulse_core::svg_skin::rasterize_svg_rect(svg, 280, 96) else {
            return Vec::new();
        };
        encode_raster_png(raster.width, raster.height, raster.rgba)
    }

    pub fn rasterize_default_icon_png_bytes(&self, is_player: bool, is_losing: bool) -> Vec<u8> {
        let svg = if is_player {
            pulse_core::svg_skin::default_player_icon_svg(is_losing)
        } else {
            pulse_core::svg_skin::default_opponent_icon_svg(is_losing)
        };
        let Ok(raster) = pulse_core::svg_skin::rasterize_svg_str(svg, 128) else {
            return Vec::new();
        };
        encode_raster_png(raster.width, raster.height, raster.rgba)
    }

    pub fn rasterize_note_splash_png_bytes(&self) -> Vec<u8> {
        let svg = pulse_core::svg_skin::default_note_splash_svg();
        let Ok(raster) = pulse_core::svg_skin::rasterize_svg_str(svg, 160) else {
            return Vec::new();
        };
        encode_raster_png(raster.width, raster.height, raster.rgba)
    }
}

fn encode_raster_png(width: u32, height: u32, rgba: Vec<u8>) -> Vec<u8> {
    let Some(rgba_img) = image::RgbaImage::from_raw(width, height, rgba) else {
        return Vec::new();
    };
    let mut png_bytes = Vec::new();
    if image::DynamicImage::ImageRgba8(rgba_img)
        .write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
        .is_ok()
    {
        png_bytes
    } else {
        Vec::new()
    }
}
