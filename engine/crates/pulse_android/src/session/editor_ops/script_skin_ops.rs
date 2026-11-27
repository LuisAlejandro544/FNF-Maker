//! Archivo: engine/crates/pulse_android/src/session/editor_ops/script_skin_ops.rs
//! Propósito: Gestión de scripts comunitarios Lua/TypeScript/JavaScript (alta, importación,
//! guardado sincronizado con `camera_system.lua` y validación sintáctica) y rasterización/importación
//! de skins SVG y `NOTE_assets.png+.xml` (flechas, receptores, Nota X, barra de vida, ratings,
//! íconos predeterminados y Note Splash) mediante `resvg`.

use crate::session::{ActionResultJson, EngineSession, friendly_import_name, parse_lang_key};
use image::ImageFormat;
use pulse_core::project::{CAMERA_SCRIPT_ID, CAMERA_SCRIPT_NAME, parse_camera_config_from_lua};
use pulse_core::svg_skin::{
    ArrowSkinSet, default_hurt_arrow_x_svg, import_arrow_sparrow_atlas, rasterize_svg_str,
};
use pulse_core::{NoteDirection, ScriptAsset, ScriptLanguage, StageCameraConfig};
use pulse_scripting::validate_script_syntax;
use std::io::Cursor;
use std::path::PathBuf;

impl EngineSession {
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
