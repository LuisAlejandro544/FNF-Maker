//! Archivo: engine/crates/pulse_android/src/session/editor_ops.rs
//! Propósito: Operaciones de edición del proyecto en `EngineSession`:
//! - Importación, edición y exportación completa de `chart.json`.
//! - Decodificación y configuración de hasta 3 pistas de audio en `pulse_audio`.
//! - Gestión de entidades del escenario (`StageEntity`), scripts Lua/TS/JS y skins SVG (`resvg`).

use super::{ActionResultJson, EngineSession, parse_control_mode_key, parse_lang_key, parse_role_key};
use image::ImageFormat;
use pulse_audio::decode_audio_file;
use pulse_core::project::{
    CAMERA_SCRIPT_ID, CAMERA_SCRIPT_NAME, MAX_AUDIO_TRACKS, parse_camera_config_from_lua,
    render_camera_lua_script,
};
use pulse_core::svg_skin::{ArrowSkinSet, rasterize_svg_str};
use pulse_core::{
    CameraFollowMode, ChartData, NoteDirection, ScriptAsset, ScriptLanguage, StageCameraConfig,
    StageEntity,
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
                    self.project.chart = chart;
                    self.project.chart_source_path = Some(path);
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
        self.save_active_project_now();
        self.ok_response("Metadatos completos de chart.json actualizados")
    }

    pub fn set_section_bpm(&mut self, sec_index: usize, bpm: f64, change_bpm: bool) -> String {
        self.project.chart.set_section_bpm(sec_index, bpm, change_bpm);
        self.save_active_project_now();
        self.ok_response(format!("BPM de sección {} actualizado", sec_index + 1))
    }

    pub fn add_chart_event(&mut self, time_ms: f64, name: &str, value1: &str, value2: &str) -> String {
        self.project.chart.add_event(time_ms, name, value1, value2);
        self.save_active_project_now();
        self.ok_response("Evento de chart.json añadido")
    }

    pub fn remove_chart_event(&mut self, index: usize) -> String {
        self.project.chart.remove_event(index);
        self.save_active_project_now();
        self.ok_response("Evento eliminado")
    }

    pub fn add_chart_section(&mut self, must_hit: bool) -> String {
        let idx = self.project.chart.add_section(must_hit);
        self.save_active_project_now();
        self.ok_response(format!("Sección {} añadida", idx + 1))
    }

    pub fn toggle_section_must_hit(&mut self, sec_index: usize) -> String {
        self.project.chart.toggle_section_must_hit(sec_index);
        self.save_active_project_now();
        self.ok_response("Foco de sección actualizado")
    }

    pub fn toggle_chart_note(
        &mut self,
        sec_index: usize,
        step_in_sec: u32,
        col_0_to_7: u8,
        sustain_steps: u32,
    ) -> String {
        self.project
            .chart
            .toggle_note_at(sec_index, step_in_sec, col_0_to_7, sustain_steps);
        self.save_active_project_now();
        self.ok_response("Nota actualizada")
    }

    pub fn import_audio_track(&mut self, slot: usize, path: PathBuf) -> String {
        if let Err(e) = self.project.assign_audio_track(slot, path.clone()) {
            return self.err_response(e);
        }
        self.audio_engine.ensure_started();
        let target_sr = self.audio_engine.mixer.sample_rate.load(Ordering::Relaxed);
        match decode_audio_file(&path, target_sr) {
            Ok(decoded) => {
                let arc_track = Arc::new(decoded);
                self.audio_engine
                    .mixer
                    .set_track(slot, Some(Arc::clone(&arc_track)));
                self.decoded_tracks[slot] = Some(arc_track);
                self.save_active_project_now();
                self.ok_response(format!("Pista {} decodificada y lista en Rust AAudio", slot + 1))
            }
            Err(e) => self.err_response(format!("Error decodificando pista {}: {}", slot + 1, e)),
        }
    }

    pub fn remove_audio_track(&mut self, slot: usize) -> String {
        if slot < MAX_AUDIO_TRACKS {
            self.project.audio_tracks[slot].file_path = None;
            self.project.audio_tracks[slot].file_name = None;
            self.decoded_tracks[slot] = None;
            self.audio_engine.mixer.set_track(slot, None);
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
            self.mark_dirty_and_maybe_save(false);
        }
        self.ok_response("Configuración de pista actualizada")
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
        self.save_active_project_now();
        self.ok_response("Orden Z actualizado")
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
        commit_save: bool,
    ) -> String {
        let cfg = StageCameraConfig {
            enabled,
            base_x,
            base_y,
            base_zoom,
            follow_mode: CameraFollowMode::from_str_loose(follow_mode_key),
            follow_interval_beats: follow_interval_beats.clamp(1, 32) as u32,
            smooth_speed,
            pan_strength,
        };
        self.project.stage.apply_camera_config(cfg);
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
        }

        self.mark_dirty_and_maybe_save(commit_save);
        if commit_save {
            self.ok_response("Cámara y camera_system.lua actualizados")
        } else {
            "{\"ok\":true,\"message\":\"Cámara actualizada\",\"data\":null}".to_string()
        }
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
        self.save_active_project_now();
        self.ok_response("Script creado")
    }

    pub fn import_script(&mut self, path: PathBuf) -> String {
        match std::fs::read_to_string(&path) {
            Ok(src) => {
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("custom.lua")
                    .to_string();
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
                self.project.arrow_skin.custom_svg_paths.insert(dir, path);
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

    pub fn restore_default_arrow_svg(&mut self) -> String {
        self.project.arrow_skin = ArrowSkinSet::bundled_default();
        self.save_active_project_now();
        self.ok_response("Skin de flechas SVG originales restaurada")
    }

    pub fn rasterize_arrow_png_bytes(&self, dir_code: u8) -> Vec<u8> {
        let dir = NoteDirection::from_lane(dir_code);
        let svg = self.project.arrow_skin.svg_for(dir);
        let Ok(raster) = rasterize_svg_str(&svg, self.project.arrow_skin.note_size_px) else {
            return Vec::new();
        };
        let Some(rgba_img) = image::RgbaImage::from_raw(raster.width, raster.height, raster.rgba) else {
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

    pub fn rasterize_receptor_png_bytes(&self, dir_code: u8) -> Vec<u8> {
        let dir = NoteDirection::from_lane(dir_code);
        let Ok(raster) = self.project.arrow_skin.rasterize_neutral_receptor(dir) else {
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
                self.project.health_bar_svg_path = Some(path);
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
