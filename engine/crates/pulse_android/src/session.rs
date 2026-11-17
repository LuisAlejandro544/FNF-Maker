//! Archivo: engine/crates/pulse_android/src/session.rs
//! Propósito: Estado de sesión puro del motor Rust (`EngineSession`) compartido por el puente JNI.
//! Concentra todas las operaciones reales de `pulse_core`, `pulse_audio` y `pulse_scripting`
//! (parseo/edición/exportación completa de `chart.json`, decodificación de hasta 3 pistas,
//! Sparrow XML, texturas `.astc` y atlas `.json` / Adobe Animate, escenario Z-order,
//! compilación en caliente `rayon`, rasterización SVG `resvg`, validación/ejecución Lua/TS/JS
//! y simulación frame-a-frame con cuenta regresiva BPM "3, 2, 1, ¡YA!").

use image::ImageFormat;
use pulse_audio::{AudioEngineHandle, DecodedTrack, decode_audio_file};
use pulse_core::astc_loader::{is_astc_path, load_sprite_image, read_sprite_dimensions};
use pulse_core::atlas_json::{DetectedSpriteJson, bake_animate_atlas, parse_sprite_json_str};
use pulse_core::hot_compile::{CompiledBundle, DeviceResourceTier, HotCompileReport, hot_compile_project};
use pulse_core::project::{ModProject, MAX_AUDIO_TRACKS};
use pulse_core::sparrow::CharacterAnimKind;
use pulse_core::svg_skin::{ArrowSkinSet, rasterize_svg_str};
use pulse_core::{ChartData, EntityRole, NoteDirection, ScriptAsset, ScriptLanguage, SparrowAtlas, StageEntity};
use pulse_scripting::{ModScriptRuntime, ScriptCommand, validate_script_syntax};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

const HIT_WINDOW_GOOD_MS: f64 = 155.0;
const HIT_WINDOW_PERFECT_MS: f64 = 55.0;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResultJson<T: Serialize> {
    pub ok: bool,
    pub message: String,
    pub data: Option<T>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshotJson {
    pub id: String,
    pub name: String,
    pub author: String,
    pub downscroll: bool,
    pub audio_offset_ms: f64,
    pub chart: ChartSnapshotJson,
    pub audio_tracks: Vec<AudioTrackSnapshotJson>,
    pub stage_entities: Vec<StageEntitySnapshotJson>,
    pub scripts: Vec<ScriptSnapshotJson>,
    pub arrow_skin_name: String,
    pub custom_arrow_paths: HashMap<String, String>,
    pub last_compile_report: Option<CompileReportJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartEventJson {
    pub time_ms: f64,
    pub name: String,
    pub value1: String,
    pub value2: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSectionSnapshotJson {
    pub index: usize,
    pub start_ms: f64,
    pub length_in_steps: u32,
    pub must_hit_section: bool,
    pub bpm: f64,
    pub change_bpm: bool,
    pub alt_anim: bool,
    pub gf_section: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSnapshotJson {
    pub title: String,
    pub bpm: f64,
    pub speed: f64,
    pub needs_voices: bool,
    pub player1: String,
    pub player2: String,
    pub gf_version: String,
    pub stage: String,
    pub section_count: usize,
    pub total_notes: usize,
    pub player_notes: usize,
    pub opponent_notes: usize,
    pub sustain_notes: usize,
    pub duration_ms: f64,
    pub density_buckets: Vec<u16>,
    pub source_path: Option<String>,
    pub sections: Vec<ChartSectionSnapshotJson>,
    pub notes: Vec<VisibleNoteJson>,
    pub events: Vec<ChartEventJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTrackSnapshotJson {
    pub slot_index: usize,
    pub role_label: String,
    pub file_name: Option<String>,
    pub gain: f32,
    pub muted: bool,
    pub duck_on_player_miss: bool,
    pub duration_ms: f64,
    pub waveform_peaks: Vec<f32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageEntitySnapshotJson {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub role_label: String,
    pub image_path: Option<String>,
    pub raw_image_path: Option<String>,
    pub xml_path: Option<String>,
    pub animate_json_path: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub opacity: f32,
    pub flip_x: bool,
    pub z_index: i32,
    pub visible: bool,
    pub animation_prefixes: Vec<String>,
    pub anim_bindings: HashMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptSnapshotJson {
    pub id: String,
    pub name: String,
    pub language: String,
    pub language_label: String,
    pub source_code: String,
    pub enabled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileReportJson {
    pub elapsed_ms: u64,
    pub kept_entities: usize,
    pub discarded_invisible_entities: Vec<String>,
    pub downscaled_textures: usize,
    pub estimated_vram_saved_kb: usize,
    pub compiled_texture_paths: HashMap<String, String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SparrowFrameJson {
    pub found: bool,
    pub raw_name: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub frame_x: i32,
    pub frame_y: i32,
    pub frame_width: u32,
    pub frame_height: u32,
    pub rotated: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplayEntityFrameJson {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub opacity: f32,
    pub flip_x: bool,
    pub visible: bool,
    pub texture_path: Option<String>,
    pub frame: Option<SparrowFrameJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisibleNoteJson {
    pub id: u64,
    pub time_ms: f64,
    pub direction: u8,
    pub is_player: bool,
    pub sustain_ms: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplayStepJson {
    pub active: bool,
    pub song_pos_ms: f64,
    pub duration_ms: f64,
    pub scroll_speed: f64,
    pub active_bpm: f64,
    pub downscroll: bool,
    pub countdown_stage: i32,
    pub countdown_label: String,
    pub countdown_progress: f32,
    pub camera_zoom: f32,
    pub camera_shake: f32,
    pub camera_x: f32,
    pub camera_y: f32,
    pub virtual_width: f32,
    pub virtual_height: f32,
    pub score: i64,
    pub combo: u32,
    pub max_combo: u32,
    pub hits: u32,
    pub misses: u32,
    pub health: f32,
    pub last_judgement: String,
    pub paused: bool,
    pub finished: bool,
    pub script_errors: Vec<String>,
    pub script_logs: Vec<String>,
    pub entities: Vec<GameplayEntityFrameJson>,
    pub visible_notes: Vec<VisibleNoteJson>,
}

pub struct GameplayRuntimeState {
    pub bundle: CompiledBundle,
    pub compiled_texture_paths: HashMap<String, String>,
    pub runtime: ModScriptRuntime,
    pub script_errors: Vec<String>,
    pub notes_hit_state: Vec<bool>,
    pub notes_miss_state: Vec<bool>,
    pub score: i64,
    pub combo: u32,
    pub max_combo: u32,
    pub hits: u32,
    pub misses: u32,
    pub health: f32,
    pub last_judgement: String,
    pub last_beat: i32,
    pub paused: bool,
    pub finished: bool,
    pub last_logs: Vec<String>,
}

pub struct EngineSession {
    pub cache_dir: PathBuf,
    pub project: ModProject,
    pub audio_engine: AudioEngineHandle,
    pub decoded_tracks: [Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS],
    pub compiled_bundle: Option<CompiledBundle>,
    pub compiled_texture_paths: HashMap<String, String>,
    pub last_compile_report: Option<HotCompileReport>,
    pub gameplay: Option<GameplayRuntimeState>,
}

impl EngineSession {
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            cache_dir,
            project: ModProject::new_demo(),
            audio_engine: AudioEngineHandle::new_deferred(),
            decoded_tracks: [None, None, None],
            compiled_bundle: None,
            compiled_texture_paths: HashMap::new(),
            last_compile_report: None,
            gameplay: None,
        }
    }

    pub fn snapshot_json_string(&self) -> String {
        let snap = self.build_snapshot();
        serde_json::to_string(&snap).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn build_snapshot(&self) -> ProjectSnapshotJson {
        let audio_tracks = (0..MAX_AUDIO_TRACKS)
            .map(|slot| {
                let cfg = &self.project.audio_tracks[slot];
                let decoded = self.decoded_tracks[slot].as_ref();
                AudioTrackSnapshotJson {
                    slot_index: slot,
                    role_label: cfg.role_label.clone(),
                    file_name: cfg.file_name.clone(),
                    gain: cfg.gain,
                    muted: cfg.muted,
                    duck_on_player_miss: cfg.duck_on_player_miss,
                    duration_ms: decoded.map(|d| d.duration_ms()).unwrap_or(0.0),
                    waveform_peaks: decoded.map(|d| d.waveform_peaks.clone()).unwrap_or_default(),
                }
            })
            .collect();

        let stage_entities = self
            .project
            .stage
            .entities
            .iter()
            .map(|ent| {
                let prefixes = ent
                    .atlas
                    .as_ref()
                    .map(|a| a.animations.keys().cloned().collect())
                    .unwrap_or_default();
                let mut bindings = HashMap::new();
                if let Some(atlas) = &ent.atlas {
                    for (kind, pref) in &atlas.bindings {
                        bindings.insert(anim_kind_key(*kind).to_string(), pref.clone());
                    }
                }
                StageEntitySnapshotJson {
                    id: ent.id.clone(),
                    display_name: ent.display_name.clone(),
                    role: role_to_key(ent.role).to_string(),
                    role_label: ent.role.label_es().to_string(),
                    image_path: ent.image_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    raw_image_path: ent.raw_image_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    xml_path: ent.xml_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    animate_json_path: ent.animate_json_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
                    x: ent.x,
                    y: ent.y,
                    width: ent.width,
                    height: ent.height,
                    scale: ent.scale,
                    opacity: ent.opacity,
                    flip_x: ent.flip_x,
                    z_index: ent.z_index,
                    visible: ent.visible,
                    animation_prefixes: prefixes,
                    anim_bindings: bindings,
                }
            })
            .collect();

        let scripts = self
            .project
            .scripts
            .iter()
            .map(|s| ScriptSnapshotJson {
                id: s.id.clone(),
                name: s.name.clone(),
                language: lang_to_key(s.language).to_string(),
                language_label: s.language.label().to_string(),
                source_code: s.source_code.clone(),
                enabled: s.enabled,
            })
            .collect();

        let mut custom_arrow_paths = HashMap::new();
        for (dir, path) in &self.project.arrow_skin.custom_svg_paths {
            custom_arrow_paths.insert(
                format!("{}", *dir as u8),
                path.to_string_lossy().into_owned(),
            );
        }

        let last_compile_report = self.last_compile_report.as_ref().map(|r| CompileReportJson {
            elapsed_ms: r.elapsed_ms,
            kept_entities: r.kept_entities,
            discarded_invisible_entities: r.discarded_invisible_entities.clone(),
            downscaled_textures: r.downscaled_textures,
            estimated_vram_saved_kb: r.estimated_vram_saved_kb,
            compiled_texture_paths: self.compiled_texture_paths.clone(),
        });

        ProjectSnapshotJson {
            id: self.project.id.clone(),
            name: self.project.name.clone(),
            author: self.project.author.clone(),
            downscroll: self.project.downscroll,
            audio_offset_ms: self.project.audio_offset_ms,
            chart: ChartSnapshotJson {
                title: self.project.chart.title.clone(),
                bpm: self.project.chart.bpm,
                speed: self.project.chart.speed,
                needs_voices: self.project.chart.needs_voices,
                player1: self.project.chart.player1.clone(),
                player2: self.project.chart.player2.clone(),
                gf_version: self.project.chart.gf_version.clone(),
                stage: self.project.chart.stage.clone(),
                section_count: self.project.chart.sections.len(),
                total_notes: self.project.chart.stats.total_notes,
                player_notes: self.project.chart.stats.player_notes,
                opponent_notes: self.project.chart.stats.opponent_notes,
                sustain_notes: self.project.chart.stats.sustain_notes,
                duration_ms: self.project.chart.stats.duration_ms,
                density_buckets: self.project.chart.stats.density_buckets.clone(),
                source_path: self
                    .project
                    .chart_source_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned()),
                sections: self
                    .project
                    .chart
                    .sections
                    .iter()
                    .map(|s| ChartSectionSnapshotJson {
                        index: s.index,
                        start_ms: s.start_ms,
                        length_in_steps: s.length_in_steps,
                        must_hit_section: s.must_hit_section,
                        bpm: s.bpm,
                        change_bpm: s.change_bpm,
                        alt_anim: s.alt_anim,
                        gf_section: s.gf_section,
                    })
                    .collect(),
                notes: self
                    .project
                    .chart
                    .notes
                    .iter()
                    .map(|n| VisibleNoteJson {
                        id: n.id,
                        time_ms: n.time_ms,
                        direction: n.direction as u8,
                        is_player: n.is_player,
                        sustain_ms: n.sustain_ms,
                    })
                    .collect(),
                events: self
                    .project
                    .chart
                    .events
                    .iter()
                    .map(|e| ChartEventJson {
                        time_ms: e.time_ms,
                        name: e.name.clone(),
                        value1: e.value1.clone(),
                        value2: e.value2.clone(),
                    })
                    .collect(),
            },
            audio_tracks,
            stage_entities,
            scripts,
            arrow_skin_name: self.project.arrow_skin.skin_name.clone(),
            custom_arrow_paths,
            last_compile_report,
        }
    }

    pub fn ok_response(&self, message: impl Into<String>) -> String {
        let res = ActionResultJson {
            ok: true,
            message: message.into(),
            data: Some(self.build_snapshot()),
        };
        serde_json::to_string(&res).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn err_response(&self, message: impl Into<String>) -> String {
        let res = ActionResultJson {
            ok: false,
            message: message.into(),
            data: Some(self.build_snapshot()),
        };
        serde_json::to_string(&res).unwrap_or_else(|_| "{}".to_string())
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
        self.ok_response("Metadatos completos de chart.json actualizados")
    }

    pub fn set_section_bpm(&mut self, sec_index: usize, bpm: f64, change_bpm: bool) -> String {
        self.project.chart.set_section_bpm(sec_index, bpm, change_bpm);
        self.ok_response(format!("BPM de sección {} actualizado", sec_index + 1))
    }

    pub fn add_chart_event(&mut self, time_ms: f64, name: &str, value1: &str, value2: &str) -> String {
        self.project.chart.add_event(time_ms, name, value1, value2);
        self.ok_response("Evento de chart.json añadido")
    }

    pub fn remove_chart_event(&mut self, index: usize) -> String {
        self.project.chart.remove_event(index);
        self.ok_response("Evento eliminado")
    }

    pub fn add_chart_section(&mut self, must_hit: bool) -> String {
        let idx = self.project.chart.add_section(must_hit);
        self.ok_response(format!("Sección {} añadida", idx + 1))
    }

    pub fn toggle_section_must_hit(&mut self, sec_index: usize) -> String {
        self.project.chart.toggle_section_must_hit(sec_index);
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
        }
        self.ok_response("Configuración de pista actualizada")
    }

    /// Si la entidad ya tiene `Animation.json` + `spritemap1.json` + textura (`.astc` o `.png`),
    /// hornea automáticamente los frames compuestos a un spritesheet RGBA en caché y actualiza su atlas.
    fn try_bake_animate_for_entity(&mut self, entity_id: &str) -> Result<Option<String>, String> {
        let (doc, spritemap, raw_img_path) = {
            let Some(ent) = self.project.stage.entities.iter().find(|e| e.id == entity_id) else {
                return Ok(None);
            };
            let (Some(doc), Some(sm)) = (ent.animate_doc.clone(), ent.animate_spritemap.clone()) else {
                return Ok(None);
            };
            let Some(img_path) = ent.raw_image_path.clone().or_else(|| ent.image_path.clone()) else {
                return Ok(None);
            };
            (doc, sm, img_path)
        };

        let src_img = load_sprite_image(&raw_img_path)?;
        let (baked_sheet, baked_atlas) = bake_animate_atlas(&doc, &spritemap, &src_img)?;
        let anim_count = baked_atlas.animations.len();

        let baked_dir = self.cache_dir.join("animate_baked");
        let _ = std::fs::create_dir_all(&baked_dir);
        let baked_path = baked_dir.join(format!("{}_baked.png", entity_id));
        baked_sheet
            .save_with_format(&baked_path, ImageFormat::Png)
            .map_err(|e| format!("Error guardando spritesheet Animate horneado: {}", e))?;

        if let Some(ent) = self.project.stage.find_mut(entity_id) {
            ent.image_path = Some(baked_path);
            ent.atlas = Some(baked_atlas);
        }

        Ok(Some(format!(
            "Rig Adobe Animate ensamblado en Rust: {} animaciones listas",
            anim_count
        )))
    }

    pub fn import_sprite_png(&mut self, entity_id: &str, path: PathBuf) -> String {
        if !path.exists() {
            return self.err_response("Archivo de textura no encontrado");
        }

        let is_astc = is_astc_path(&path);
        let display_png_path = if is_astc {
            match load_sprite_image(&path) {
                Ok(decoded) => {
                    let dec_dir = self.cache_dir.join("decoded_textures");
                    let _ = std::fs::create_dir_all(&dec_dir);
                    let out_png = dec_dir.join(format!("{}.png", entity_id));
                    if let Err(e) = decoded.save_with_format(&out_png, ImageFormat::Png) {
                        return self.err_response(format!("Error guardando caché ASTC->PNG: {}", e));
                    }
                    out_png
                }
                Err(e) => return self.err_response(format!("Error decodificando .astc: {}", e)),
            }
        } else {
            path.clone()
        };

        let dims = read_sprite_dimensions(&display_png_path).or_else(|| read_sprite_dimensions(&path));
        if let Some(ent) = self.project.stage.find_mut(entity_id) {
            ent.raw_image_path = Some(path);
            ent.image_path = Some(display_png_path);
            if let Some((w, h)) = dims {
                if ent.atlas.is_none() && ent.role != EntityRole::Background && w > 0 && h > 0 {
                    // Ajustar tamaño lógico inicial manteniendo proporción razonable dentro de 1280x720.
                    let max_side = w.max(h) as f32;
                    let norm = (320.0 / max_side).min(1.0);
                    ent.width = (w as f32 * norm).max(64.0);
                    ent.height = (h as f32 * norm).max(64.0);
                }
            }
        } else {
            return self.err_response("Objeto de escenario no encontrado");
        }

        match self.try_bake_animate_for_entity(entity_id) {
            Ok(Some(msg)) => self.ok_response(msg),
            Ok(None) => self.ok_response(if is_astc {
                "Textura .astc decodificada y vinculada en Rust"
            } else {
                "Hoja de textura vinculada en Rust"
            }),
            Err(e) => self.err_response(e),
        }
    }

    pub fn import_sparrow_xml(&mut self, entity_id: &str, path: PathBuf) -> String {
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => return self.err_response(format!("No se pudo leer el descriptor: {}", e)),
        };

        let is_json = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("json"))
            .unwrap_or(false)
            || raw.trim_start().starts_with('{')
            || raw.trim_start().starts_with('[');

        if is_json {
            match parse_sprite_json_str(&raw) {
                Ok(DetectedSpriteJson::DirectAtlas(atlas)) => {
                    let count = atlas.animations.len();
                    if let Some(ent) = self.project.stage.find_mut(entity_id) {
                        ent.xml_path = Some(path);
                        ent.animate_spritemap = None;
                        ent.animate_doc = None;
                        ent.animate_json_path = None;
                        ent.atlas = Some(atlas);
                        return self.ok_response(format!(
                            "Atlas JSON cargado automáticamente: {} animaciones",
                            count
                        ));
                    }
                    return self.err_response("Objeto de escenario no encontrado");
                }
                Ok(DetectedSpriteJson::AnimateSpritemap(sm)) => {
                    let piece_count = sm.sprites.len();
                    let fallback = sm.to_fallback_sparrow();
                    if let Some(ent) = self.project.stage.find_mut(entity_id) {
                        ent.xml_path = Some(path);
                        ent.animate_spritemap = Some(sm);
                        ent.atlas = Some(fallback);
                    } else {
                        return self.err_response("Objeto de escenario no encontrado");
                    }
                    return match self.try_bake_animate_for_entity(entity_id) {
                        Ok(Some(msg)) => self.ok_response(msg),
                        Ok(None) => self.ok_response(format!(
                            "spritemap1.json cargado ({} piezas). Falta Animation.json o textura para hornear el rig.",
                            piece_count
                        )),
                        Err(e) => self.err_response(e),
                    };
                }
                Ok(DetectedSpriteJson::AnimateDocument(doc)) => {
                    let anim_count = doc.detected_animations.len();
                    if let Some(ent) = self.project.stage.find_mut(entity_id) {
                        ent.animate_json_path = Some(path);
                        ent.animate_doc = Some(doc);
                    } else {
                        return self.err_response("Objeto de escenario no encontrado");
                    }
                    return match self.try_bake_animate_for_entity(entity_id) {
                        Ok(Some(msg)) => self.ok_response(msg),
                        Ok(None) => self.ok_response(format!(
                            "Animation.json cargado ({} animaciones). Importa spritemap1.json y la textura (.astc/.png).",
                            anim_count
                        )),
                        Err(e) => self.err_response(e),
                    };
                }
                Err(e) => return self.err_response(e),
            }
        }

        match SparrowAtlas::from_xml_str(&raw) {
            Ok(atlas) => {
                let count = atlas.animations.len();
                if let Some(ent) = self.project.stage.find_mut(entity_id) {
                    ent.xml_path = Some(path);
                    ent.animate_spritemap = None;
                    ent.animate_doc = None;
                    ent.animate_json_path = None;
                    ent.atlas = Some(atlas);
                    self.ok_response(format!(
                        "XML Sparrow ensamblado automáticamente: {} animaciones",
                        count
                    ))
                } else {
                    self.err_response("Objeto de escenario no encontrado")
                }
            }
            Err(e) => self.err_response(format!("Error XML Sparrow: {}", e)),
        }
    }

    pub fn set_sparrow_binding(&mut self, entity_id: &str, kind_key: &str, prefix: &str) -> String {
        let Some(kind) = parse_anim_kind(kind_key) else {
            return self.err_response("Tipo de animación desconocido");
        };
        if let Some(ent) = self.project.stage.find_mut(entity_id) {
            if let Some(atlas) = &mut ent.atlas {
                atlas.bindings.insert(kind, prefix.to_string());
                return self.ok_response("Animación asignada");
            }
        }
        self.err_response("El objeto no tiene un atlas cargado")
    }

    pub fn get_sparrow_frame_info(&self, entity_id: &str, kind_key: &str, elapsed_sec: f32) -> String {
        let kind = parse_anim_kind(kind_key).unwrap_or(CharacterAnimKind::Idle);
        let frame = self
            .project
            .stage
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .and_then(|e| e.atlas.as_ref())
            .and_then(|a| a.frame_at(kind, elapsed_sec));

        let resp = match frame {
            Some(f) => SparrowFrameJson {
                found: true,
                raw_name: f.raw_name.clone(),
                x: f.x,
                y: f.y,
                width: f.width,
                height: f.height,
                frame_x: f.frame_x,
                frame_y: f.frame_y,
                frame_width: f.frame_width,
                frame_height: f.frame_height,
                rotated: f.rotated,
            },
            None => SparrowFrameJson {
                found: false,
                raw_name: String::new(),
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                frame_x: 0,
                frame_y: 0,
                frame_width: 0,
                frame_height: 0,
                rotated: false,
            },
        };
        serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string())
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
            self.ok_response("Objeto actualizado")
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
        self.ok_response("Orden Z actualizado")
    }

    pub fn add_script(&mut self, lang_key: &str) -> String {
        let lang = parse_lang_key(lang_key);
        let idx = self.project.scripts.len() + 1;
        let id = format!("script_{}", idx);
        let name = format!("custom_{}.{}", idx, lang.extension());
        let template = match lang {
            ScriptLanguage::Lua => {
                "-- Nuevo script Lua 5.4\nfunction onBeatHit(beat)\n    setCameraZoom(1.03)\nend\n"
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
        });
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
                });
                self.ok_response("Script comunitario importado")
            }
            Err(e) => self.err_response(format!("No se pudo leer el script: {}", e)),
        }
    }

    pub fn update_script(&mut self, script_id: &str, enabled: bool, source_code: String) -> String {
        if let Some(script) = self.project.scripts.iter_mut().find(|s| s.id == script_id) {
            script.enabled = enabled;
            script.source_code = source_code;
            self.ok_response("Script guardado")
        } else {
            self.err_response("Script no encontrado")
        }
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
                self.ok_response(format!("Skin SVG actualizada para flecha {}", dir.label_es()))
            }
            Err(e) => self.err_response(format!("No se pudo leer SVG: {}", e)),
        }
    }

    pub fn restore_default_arrow_svg(&mut self) -> String {
        self.project.arrow_skin = ArrowSkinSet::bundled_default();
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

    pub fn hot_compile(&mut self, tier_code: i32) -> String {
        let tier = match tier_code {
            0 => DeviceResourceTier::LowRam32Bit,
            2 => DeviceResourceTier::HighQuality,
            _ => DeviceResourceTier::Balanced,
        };

        let bundle = hot_compile_project(&self.project, tier, |_pct, _msg| {});
        let out_dir = self.cache_dir.join("compiled_cache");
        let _ = std::fs::create_dir_all(&out_dir);

        let mut texture_paths = HashMap::new();
        for (entity_id, tex) in &bundle.textures {
            if let Some(rgba_img) = image::RgbaImage::from_raw(tex.width, tex.height, tex.rgba.clone()) {
                let target_file = out_dir.join(format!("{}.png", entity_id));
                if image::DynamicImage::ImageRgba8(rgba_img)
                    .save_with_format(&target_file, ImageFormat::Png)
                    .is_ok()
                {
                    texture_paths.insert(
                        entity_id.clone(),
                        target_file.to_string_lossy().into_owned(),
                    );
                }
            }
        }

        self.compiled_texture_paths = texture_paths;
        self.last_compile_report = Some(bundle.report.clone());
        self.compiled_bundle = Some(bundle);
        self.ok_response("Compilación en caliente completada")
    }

    pub fn start_gameplay(&mut self) -> String {
        self.gameplay = None;
        let bundle = self
            .compiled_bundle
            .take()
            .unwrap_or_else(|| hot_compile_project(&self.project, DeviceResourceTier::Balanced, |_, _| {}));

        let (mut runtime, script_errors) = ModScriptRuntime::load_from_scripts(&self.project.scripts);
        runtime.call_on_create();

        self.audio_engine.ensure_started();
        let any_audio = self.project.active_audio_count() > 0;
        self.audio_engine
            .mixer
            .use_demo_synth
            .store(!any_audio, Ordering::Relaxed);
        self.audio_engine.mixer.demo_bpm_bits.store(
            (self.project.chart.bpm as f32).to_bits(),
            Ordering::Relaxed,
        );
        for (slot, cfg) in self.project.audio_tracks.iter().enumerate() {
            self.audio_engine.mixer.configs[slot].set_gain(cfg.gain);
            self.audio_engine.mixer.configs[slot]
                .muted
                .store(cfg.muted, Ordering::Relaxed);
            self.audio_engine.mixer.configs[slot]
                .duck_on_miss
                .store(cfg.duck_on_player_miss, Ordering::Relaxed);
        }
        self.audio_engine
            .mixer
            .start_countdown(self.project.chart.bpm);
        self.audio_engine.mixer.play();

        let note_len = self.project.chart.notes.len();
        self.gameplay = Some(GameplayRuntimeState {
            bundle,
            compiled_texture_paths: self.compiled_texture_paths.clone(),
            runtime,
            script_errors,
            notes_hit_state: vec![false; note_len],
            notes_miss_state: vec![false; note_len],
            score: 0,
            combo: 0,
            max_combo: 0,
            hits: 0,
            misses: 0,
            health: 0.5,
            last_judgement: "¡Prepárate!".to_string(),
            last_beat: -10,
            paused: false,
            finished: false,
            last_logs: Vec::new(),
        });

        self.step_gameplay(0.0, 0, 0)
    }

    pub fn step_gameplay(
        &mut self,
        dt_sec: f32,
        _lane_pressed_mask: i32,
        lane_just_pressed_mask: i32,
    ) -> String {
        let Some(session) = self.gameplay.as_mut() else {
            return serde_json::to_string(&GameplayStepJson {
                active: false,
                song_pos_ms: 0.0,
                duration_ms: 0.0,
                scroll_speed: 1.8,
                active_bpm: 120.0,
                downscroll: false,
                countdown_stage: -1,
                countdown_label: String::new(),
                countdown_progress: 0.0,
                camera_zoom: 1.0,
                camera_shake: 0.0,
                camera_x: 640.0,
                camera_y: 360.0,
                virtual_width: 1280.0,
                virtual_height: 720.0,
                score: 0,
                combo: 0,
                max_combo: 0,
                hits: 0,
                misses: 0,
                health: 0.5,
                last_judgement: String::new(),
                paused: false,
                finished: false,
                script_errors: Vec::new(),
                script_logs: Vec::new(),
                entities: Vec::new(),
                visible_notes: Vec::new(),
            })
            .unwrap_or_default();
        };

        let dt = dt_sec.clamp(0.0, 0.05);
        let raw_mixer_ms = self.audio_engine.mixer.position_ms();
        let in_countdown = raw_mixer_ms < 0.0;
        let song_pos_ms = if in_countdown {
            raw_mixer_ms
        } else {
            raw_mixer_ms - self.project.audio_offset_ms
        };
        let active_bpm = self.project.chart.bpm_at_ms(song_pos_ms.max(0.0)).max(40.0);
        let beat_ms = 60_000.0 / active_bpm;
        let current_beat = (song_pos_ms / beat_ms).floor() as i32;

        let (countdown_stage, countdown_label, countdown_progress) = if in_countdown {
            let cd_beat_ms = 60_000.0 / self.project.chart.bpm.max(40.0);
            let total_cd_ms = cd_beat_ms * 4.0;
            let elapsed_cd = (total_cd_ms + raw_mixer_ms).clamp(0.0, total_cd_ms - 0.001);
            let stage = ((elapsed_cd / cd_beat_ms).floor() as i32).clamp(0, 3);
            let progress = ((elapsed_cd % cd_beat_ms) / cd_beat_ms) as f32;
            let label = match stage {
                0 => "3",
                1 => "2",
                2 => "1",
                _ => "¡YA!",
            };
            (stage, label.to_string(), progress.clamp(0.0, 1.0))
        } else {
            (-1, String::new(), 0.0)
        };

        let mut just_pressed = [
            (lane_just_pressed_mask & 1) != 0,
            (lane_just_pressed_mask & 2) != 0,
            (lane_just_pressed_mask & 4) != 0,
            (lane_just_pressed_mask & 8) != 0,
        ];

        if !session.paused && !session.finished {
            *session.runtime.state.song_pos_ms.lock() = song_pos_ms;
            if current_beat > session.last_beat {
                session.last_beat = current_beat;
                session.runtime.call_on_beat_hit(current_beat);
            }
            session.runtime.call_on_update(dt);

            for cmd in session.runtime.drain_commands() {
                match cmd {
                    ScriptCommand::SetCameraZoom(z) => {
                        self.project.stage.camera_zoom = z.clamp(0.5, 2.5);
                    }
                    ScriptCommand::ShakeCamera { intensity } => {
                        self.project.stage.camera_shake = intensity.clamp(0.0, 25.0);
                    }
                    ScriptCommand::SetEntityProp {
                        entity_id,
                        prop,
                        value,
                    } => {
                        if let Some(ent) = session
                            .bundle
                            .active_entities
                            .iter_mut()
                            .find(|e| e.id == entity_id)
                        {
                            match prop.as_str() {
                                "opacity" => ent.set_opacity(value),
                                "scale" => ent.set_scale(value),
                                "x" => ent.x = value,
                                "y" => ent.y = value,
                                _ => {}
                            }
                        }
                    }
                    ScriptCommand::PlayEntityAnim {
                        entity_id,
                        anim_name,
                    } => {
                        if let Some(ent) = session
                            .bundle
                            .active_entities
                            .iter_mut()
                            .find(|e| e.id == entity_id)
                        {
                            if let Some(kind) = parse_anim_kind(&anim_name.to_ascii_lowercase()) {
                                ent.active_anim = kind;
                                ent.anim_timer_sec = 0.0;
                            }
                        }
                    }
                    ScriptCommand::Log(msg) => {
                        session.last_logs.push(msg);
                        if session.last_logs.len() > 6 {
                            session.last_logs.remove(0);
                        }
                    }
                }
            }

            self.project.stage.camera_zoom += (1.0 - self.project.stage.camera_zoom) * (dt * 6.0);
            self.project.stage.camera_shake *= (1.0 - dt * 10.0).max(0.0);

            let mut any_recent_miss = false;
            for (idx, note) in self.project.chart.notes.iter().enumerate() {
                if session.notes_hit_state[idx] || session.notes_miss_state[idx] {
                    continue;
                }
                if !note.is_player {
                    if song_pos_ms >= note.time_ms {
                        session.notes_hit_state[idx] = true;
                        let dir_anim = dir_to_anim(note.direction);
                        trigger_role_anim(
                            &mut session.bundle.active_entities,
                            EntityRole::Opponent,
                            dir_anim,
                        );
                        session.runtime.call_on_note_hit(note.direction as i32, false);
                    }
                } else {
                    let diff = note.time_ms - song_pos_ms;
                    let lane_idx = note.direction as usize;
                    if just_pressed[lane_idx] && diff.abs() <= HIT_WINDOW_GOOD_MS {
                        session.notes_hit_state[idx] = true;
                        just_pressed[lane_idx] = false;

                        let pts = if diff.abs() <= HIT_WINDOW_PERFECT_MS {
                            session.last_judgement = "¡Perfecto!".to_string();
                            350
                        } else {
                            session.last_judgement = "¡Bien!".to_string();
                            200
                        };
                        session.score += pts;
                        session.combo += 1;
                        session.max_combo = session.max_combo.max(session.combo);
                        session.hits += 1;
                        session.health = (session.health + 0.035).min(1.0);

                        trigger_role_anim(
                            &mut session.bundle.active_entities,
                            EntityRole::Player,
                            dir_to_anim(note.direction),
                        );
                        session.runtime.call_on_note_hit(lane_idx as i32, true);
                    } else if song_pos_ms > note.time_ms + HIT_WINDOW_GOOD_MS {
                        session.notes_miss_state[idx] = true;
                        session.combo = 0;
                        session.misses += 1;
                        session.score -= 50;
                        session.health = (session.health - 0.06).max(0.0);
                        session.last_judgement = "Fallo".to_string();
                        any_recent_miss = true;
                        session.runtime.call_on_note_miss(lane_idx as i32, true);
                    }
                }
            }

            self.audio_engine
                .mixer
                .player_missed_recently
                .store(any_recent_miss, Ordering::Relaxed);

            for ent in &mut session.bundle.active_entities {
                ent.anim_timer_sec += dt;
                if ent.active_anim != CharacterAnimKind::Idle && ent.anim_timer_sec > 0.48 {
                    ent.active_anim = CharacterAnimKind::Idle;
                    ent.anim_timer_sec = 0.0;
                }
            }

            if song_pos_ms > self.project.chart.stats.duration_ms + 1200.0 {
                session.finished = true;
                self.audio_engine.mixer.pause();
            }
        }

        let entities = session
            .bundle
            .active_entities
            .iter()
            .map(|ent| {
                let tex_path = session
                    .compiled_texture_paths
                    .get(&ent.id)
                    .cloned()
                    .or_else(|| ent.image_path.as_ref().map(|p| p.to_string_lossy().into_owned()));
                let frame = ent
                    .atlas
                    .as_ref()
                    .and_then(|a| a.frame_at(ent.active_anim, ent.anim_timer_sec))
                    .map(|f| SparrowFrameJson {
                        found: true,
                        raw_name: f.raw_name.clone(),
                        x: f.x,
                        y: f.y,
                        width: f.width,
                        height: f.height,
                        frame_x: f.frame_x,
                        frame_y: f.frame_y,
                        frame_width: f.frame_width,
                        frame_height: f.frame_height,
                        rotated: f.rotated,
                    });
                GameplayEntityFrameJson {
                    id: ent.id.clone(),
                    display_name: ent.display_name.clone(),
                    role: role_to_key(ent.role).to_string(),
                    x: ent.x,
                    y: ent.y,
                    width: ent.width,
                    height: ent.height,
                    scale: ent.scale,
                    opacity: ent.opacity,
                    flip_x: ent.flip_x,
                    visible: ent.visible,
                    texture_path: tex_path,
                    frame,
                }
            })
            .collect();

        let visible_notes = self
            .project
            .chart
            .notes
            .iter()
            .enumerate()
            .filter_map(|(idx, note)| {
                if session.notes_hit_state[idx] {
                    return None;
                }
                let delta = note.time_ms - song_pos_ms;
                if !(-240.0..=1800.0).contains(&delta) {
                    return None;
                }
                Some(VisibleNoteJson {
                    id: note.id,
                    time_ms: note.time_ms,
                    direction: note.direction as u8,
                    is_player: note.is_player,
                    sustain_ms: note.sustain_ms,
                })
            })
            .collect();

        serde_json::to_string(&GameplayStepJson {
            active: true,
            song_pos_ms,
            duration_ms: self.project.chart.stats.duration_ms,
            scroll_speed: self.project.chart.speed,
            active_bpm,
            downscroll: self.project.downscroll,
            countdown_stage,
            countdown_label,
            countdown_progress,
            camera_zoom: self.project.stage.camera_zoom,
            camera_shake: self.project.stage.camera_shake,
            camera_x: self.project.stage.camera_x,
            camera_y: self.project.stage.camera_y,
            virtual_width: self.project.stage.virtual_width,
            virtual_height: self.project.stage.virtual_height,
            score: session.score,
            combo: session.combo,
            max_combo: session.max_combo,
            hits: session.hits,
            misses: session.misses,
            health: session.health,
            last_judgement: session.last_judgement.clone(),
            paused: session.paused,
            finished: session.finished,
            script_errors: session.script_errors.clone(),
            script_logs: session.last_logs.clone(),
            entities,
            visible_notes,
        })
        .unwrap_or_else(|_| "{}".to_string())
    }

    pub fn set_gameplay_paused(&mut self, paused: bool) -> String {
        if let Some(session) = self.gameplay.as_mut() {
            session.paused = paused;
            if paused {
                self.audio_engine.mixer.pause();
            } else {
                self.audio_engine.ensure_started();
                self.audio_engine.mixer.play();
            }
        }
        self.step_gameplay(0.0, 0, 0)
    }

    pub fn stop_gameplay(&mut self) -> String {
        self.audio_engine.mixer.pause();
        self.gameplay = None;
        self.ok_response("Partida finalizada")
    }
}

fn role_to_key(role: EntityRole) -> &'static str {
    match role {
        EntityRole::Background => "background",
        EntityRole::Player => "player",
        EntityRole::Opponent => "opponent",
        EntityRole::Decoration => "decoration",
    }
}

fn parse_role_key(key: &str) -> EntityRole {
    match key {
        "background" => EntityRole::Background,
        "player" => EntityRole::Player,
        "opponent" => EntityRole::Opponent,
        _ => EntityRole::Decoration,
    }
}

fn lang_to_key(lang: ScriptLanguage) -> &'static str {
    match lang {
        ScriptLanguage::Lua => "lua",
        ScriptLanguage::TypeScript => "ts",
        ScriptLanguage::JavaScript => "js",
    }
}

fn parse_lang_key(key: &str) -> ScriptLanguage {
    match key {
        "ts" | "typescript" => ScriptLanguage::TypeScript,
        "js" | "javascript" => ScriptLanguage::JavaScript,
        _ => ScriptLanguage::Lua,
    }
}

fn anim_kind_key(kind: CharacterAnimKind) -> &'static str {
    match kind {
        CharacterAnimKind::Idle => "idle",
        CharacterAnimKind::Left => "left",
        CharacterAnimKind::Down => "down",
        CharacterAnimKind::Up => "up",
        CharacterAnimKind::Right => "right",
        CharacterAnimKind::MissLeft => "miss_left",
        CharacterAnimKind::MissDown => "miss_down",
        CharacterAnimKind::MissUp => "miss_up",
        CharacterAnimKind::MissRight => "miss_right",
    }
}

fn parse_anim_kind(key: &str) -> Option<CharacterAnimKind> {
    match key {
        "idle" => Some(CharacterAnimKind::Idle),
        "left" => Some(CharacterAnimKind::Left),
        "down" => Some(CharacterAnimKind::Down),
        "up" => Some(CharacterAnimKind::Up),
        "right" => Some(CharacterAnimKind::Right),
        _ => None,
    }
}

fn dir_to_anim(dir: NoteDirection) -> CharacterAnimKind {
    match dir {
        NoteDirection::Left => CharacterAnimKind::Left,
        NoteDirection::Down => CharacterAnimKind::Down,
        NoteDirection::Up => CharacterAnimKind::Up,
        NoteDirection::Right => CharacterAnimKind::Right,
    }
}

fn trigger_role_anim(entities: &mut [StageEntity], role: EntityRole, anim: CharacterAnimKind) {
    for ent in entities.iter_mut() {
        if ent.role == role {
            ent.active_anim = anim;
            ent.anim_timer_sec = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_session_hot_compile_and_gameplay_step_work() {
        let tmp = std::env::temp_dir().join("beatforge_session_test");
        let mut session = EngineSession::new(tmp);
        let compile_json = session.hot_compile(1);
        assert!(compile_json.contains("\"ok\":true"));

        let gp_json = session.start_gameplay();
        assert!(gp_json.contains("\"active\":true"));

        let arrow_png = session.rasterize_arrow_png_bytes(0);
        assert!(arrow_png.starts_with(&[0x89, b'P', b'N', b'G']));
    }
}
