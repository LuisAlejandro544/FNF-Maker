//! Archivo: engine/crates/pulse_ui/src/state.rs
//! Propósito: Administra el estado central de la aplicación Rust UI, incluyendo:
//! - Pantalla activa y popups
//! - Runtime asíncrono `tokio` para tareas pesadas (decodificación de audio, compilación en caliente)
//! - Caché de texturas GPU (`TextureHandle`) para spritesheets y flechas SVG
//! - Puente `PlatformFilePicker` para solicitar archivos reales en Android vía SAF.

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use parking_lot::Mutex;
use pulse_audio::{AudioEngineHandle, DecodedTrack, decode_audio_file};
use pulse_core::chart::NoteDirection;
use pulse_core::hot_compile::{CompiledBundle, DeviceResourceTier, hot_compile_project};
use pulse_core::project::{ModProject, MAX_AUDIO_TRACKS};
use pulse_core::sparrow::CharacterAnimKind;
use pulse_core::svg_skin::rasterize_arrow_skin;
use pulse_core::{ChartData, SparrowAtlas};
use pulse_scripting::ModScriptRuntime;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

/// Pantallas principales de BeatForge Studio (diseño horizontal multi-pantalla).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveScreen {
    Hub,
    ChartAndAudio,
    SpriteLab,
    StageEditor,
    ScriptStudio,
    HotCompile,
    Gameplay,
}

/// Tipo de archivo que el usuario desea importar desde su teléfono.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportKind {
    ChartJson,
    AudioTrack(usize),
    SpriteSheetPng,
    SpriteAtlasXml,
    ArrowSvg(NoteDirection),
    ScriptFile,
}

/// Evento enviado desde Android SAF (Kotlin coroutine `Dispatchers.IO`) hacia Rust.
#[derive(Debug, Clone)]
pub struct ImportedFileEvent {
    pub kind: ImportKind,
    pub path: PathBuf,
}

/// Interfaz abstracta para abrir el selector de archivos nativo del sistema operativo.
pub trait PlatformFilePicker: Send + Sync {
    fn request_import(&self, kind: ImportKind, tx: mpsc::UnboundedSender<ImportedFileEvent>);
}

/// Implementación por defecto para pruebas locales sin Android SAF.
pub struct NoopFilePicker;
impl PlatformFilePicker for NoopFilePicker {
    fn request_import(&self, _kind: ImportKind, _tx: mpsc::UnboundedSender<ImportedFileEvent>) {}
}

/// Mensajes internos enviados por los workers `tokio`/`rayon` al hilo de UI.
pub enum BackgroundEvent {
    AudioDecoded {
        slot: usize,
        track: Result<Arc<DecodedTrack>, String>,
    },
    CompileProgress {
        progress: f32,
        stage_label: String,
    },
    CompileFinished {
        bundle: CompiledBundle,
        script_errors: Vec<String>,
    },
}

/// Estado en tiempo real de una partida activa.
pub struct GameplaySession {
    pub bundle: CompiledBundle,
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
    pub lane_pressed: [bool; 4],
    pub lane_flash_timer: [f32; 4],
}

pub struct AppState {
    pub screen: ActiveScreen,
    pub project: ModProject,
    pub resource_tier: DeviceResourceTier,
    pub audio_engine: AudioEngineHandle,
    pub decoded_tracks: [Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS],
    pub selected_entity_id: String,
    pub selected_script_id: String,
    pub preview_anim_kind: CharacterAnimKind,
    pub preview_anim_timer: f32,
    pub move_step_px: f32,
    pub show_arrow_skin_popup: bool,
    pub script_check_status: Option<Result<String, String>>,
    pub toast_message: Option<String>,

    // Cachés GPU para preview y gameplay
    pub entity_textures: HashMap<String, TextureHandle>,
    pub arrow_textures: HashMap<NoteDirection, TextureHandle>,

    // Estado de compilación en caliente
    pub compile_progress: f32,
    pub compile_status_text: String,
    pub last_compile_report: Option<pulse_core::HotCompileReport>,
    pub gameplay: Option<GameplaySession>,

    // Concurrencia y puentes con Android
    pub tokio_rt: Runtime,
    pub file_picker: Arc<dyn PlatformFilePicker>,
    pub import_tx: mpsc::UnboundedSender<ImportedFileEvent>,
    pub import_rx: Mutex<mpsc::UnboundedReceiver<ImportedFileEvent>>,
    pub bg_tx: mpsc::UnboundedSender<BackgroundEvent>,
    pub bg_rx: Mutex<mpsc::UnboundedReceiver<BackgroundEvent>>,
}

impl AppState {
    pub fn new(file_picker: Arc<dyn PlatformFilePicker>) -> Self {
        let tokio_rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("No se pudo iniciar runtime Tokio");

        let (import_tx, import_rx) = mpsc::unbounded_channel();
        let (bg_tx, bg_rx) = mpsc::unbounded_channel();
        let project = ModProject::new_demo();
        let selected_entity_id = project
            .stage
            .entities
            .last()
            .map(|e| e.id.clone())
            .unwrap_or_default();
        let selected_script_id = project
            .scripts
            .first()
            .map(|s| s.id.clone())
            .unwrap_or_default();

        Self {
            screen: ActiveScreen::Hub,
            project,
            resource_tier: DeviceResourceTier::Balanced,
            audio_engine: AudioEngineHandle::start_default(),
            decoded_tracks: [None, None, None],
            selected_entity_id,
            selected_script_id,
            preview_anim_kind: CharacterAnimKind::Idle,
            preview_anim_timer: 0.0,
            move_step_px: 10.0,
            show_arrow_skin_popup: false,
            script_check_status: None,
            toast_message: None,
            entity_textures: HashMap::new(),
            arrow_textures: HashMap::new(),
            compile_progress: 0.0,
            compile_status_text: String::new(),
            last_compile_report: None,
            gameplay: None,
            tokio_rt,
            file_picker,
            import_tx,
            import_rx: Mutex::new(import_rx),
            bg_tx,
            bg_rx: Mutex::new(bg_rx),
        }
    }

    pub fn trigger_import(&self, kind: ImportKind) {
        self.file_picker.request_import(kind, self.import_tx.clone());
    }

    pub fn refresh_arrow_textures(&mut self, ctx: &egui::Context) {
        if let Ok(map) = rasterize_arrow_skin(&self.project.arrow_skin) {
            for (dir, img) in map {
                let cimg = ColorImage::from_rgba_unmultiplied(
                    [img.width as usize, img.height as usize],
                    &img.rgba,
                );
                let handle = ctx.load_texture(
                    format!("arrow_svg_{:?}", dir),
                    cimg,
                    TextureOptions::LINEAR,
                );
                self.arrow_textures.insert(dir, handle);
            }
        }
    }

    pub fn load_entity_texture_from_path(&mut self, ctx: &egui::Context, entity_id: &str, path: &PathBuf) {
        if let Ok(img) = image::open(path) {
            let rgba = img.to_rgba8();
            let size = [rgba.width() as usize, rgba.height() as usize];
            let cimg = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
            let handle = ctx.load_texture(
                format!("entity_tex_{}", entity_id),
                cimg,
                TextureOptions::LINEAR,
            );
            self.entity_textures.insert(entity_id.to_string(), handle);
        }
    }

    /// Inicia la compilación en caliente en segundo plano mientras muestra la pantalla de carga.
    pub fn start_hot_compile(&mut self) {
        self.audio_engine.mixer.pause();
        self.compile_progress = 0.02;
        self.compile_status_text = "Preparando compilación en caliente...".to_string();
        self.screen = ActiveScreen::HotCompile;

        let project_snapshot = self.project.clone();
        let tier = self.resource_tier;
        let bg_tx = self.bg_tx.clone();

        self.tokio_rt.spawn_blocking(move || {
            let tx_progress = bg_tx.clone();
            let bundle = hot_compile_project(&project_snapshot, tier, move |pct, msg| {
                let _ = tx_progress.send(BackgroundEvent::CompileProgress {
                    progress: pct,
                    stage_label: msg.to_string(),
                });
            });
            let _ = bg_tx.send(BackgroundEvent::CompileFinished {
                bundle,
                script_errors: Vec::new(),
            });
        });
    }

    pub fn poll_background_events(&mut self, ctx: &egui::Context) {
        let mut imported = Vec::new();
        if let Ok(mut rx) = self.import_rx.try_lock() {
            while let Ok(ev) = rx.try_recv() {
                imported.push(ev);
            }
        }
        for ev in imported {
            self.handle_imported_file(ctx, ev);
        }

        let mut bg_events = Vec::new();
        if let Ok(mut rx) = self.bg_rx.try_lock() {
            while let Ok(ev) = rx.try_recv() {
                bg_events.push(ev);
            }
        }
        for ev in bg_events {
            match ev {
                BackgroundEvent::AudioDecoded { slot, track } => match track {
                    Ok(decoded) => {
                        self.audio_engine.mixer.set_track(slot, Some(Arc::clone(&decoded)));
                        self.decoded_tracks[slot] = Some(decoded);
                        self.toast_message = Some(format!("Pista {} lista en Oboe", slot + 1));
                    }
                    Err(err) => {
                        self.toast_message = Some(format!("Error decodificando pista {}: {}", slot + 1, err));
                    }
                },
                BackgroundEvent::CompileProgress { progress, stage_label } => {
                    self.compile_progress = progress;
                    self.compile_status_text = stage_label;
                }
                BackgroundEvent::CompileFinished { bundle, .. } => {
                    self.finish_hot_compile_into_gameplay(ctx, bundle);
                }
            }
        }
    }

    fn finish_hot_compile_into_gameplay(&mut self, ctx: &egui::Context, bundle: CompiledBundle) {
        for (entity_id, tex) in &bundle.textures {
            let cimg = ColorImage::from_rgba_unmultiplied(
                [tex.width as usize, tex.height as usize],
                &tex.rgba,
            );
            let handle = ctx.load_texture(
                format!("compiled_{}", entity_id),
                cimg,
                TextureOptions::LINEAR,
            );
            self.entity_textures.insert(entity_id.clone(), handle);
        }

        for (dir, img) in &bundle.arrow_textures {
            let cimg = ColorImage::from_rgba_unmultiplied(
                [img.width as usize, img.height as usize],
                &img.rgba,
            );
            let handle = ctx.load_texture(
                format!("arrow_svg_{:?}", dir),
                cimg,
                TextureOptions::LINEAR,
            );
            self.arrow_textures.insert(*dir, handle);
        }

        let (mut runtime, script_errors) = ModScriptRuntime::load_from_scripts(&self.project.scripts);
        runtime.call_on_create();

        let note_len = self.project.chart.notes.len();
        self.last_compile_report = Some(bundle.report.clone());

        // Configurar mezclador Oboe
        let any_audio = self.project.active_audio_count() > 0;
        self.audio_engine
            .mixer
            .use_demo_synth
            .store(!any_audio, std::sync::atomic::Ordering::Relaxed);
        self.audio_engine.mixer.demo_bpm_bits.store(
            (self.project.chart.bpm as f32).to_bits(),
            std::sync::atomic::Ordering::Relaxed,
        );
        for (slot, cfg) in self.project.audio_tracks.iter().enumerate() {
            self.audio_engine.mixer.configs[slot].set_gain(cfg.gain);
            self.audio_engine.mixer.configs[slot]
                .muted
                .store(cfg.muted, std::sync::atomic::Ordering::Relaxed);
            self.audio_engine.mixer.configs[slot]
                .duck_on_miss
                .store(cfg.duck_on_player_miss, std::sync::atomic::Ordering::Relaxed);
        }
        self.audio_engine.mixer.seek_ms(0.0);
        self.audio_engine.mixer.play();

        self.gameplay = Some(GameplaySession {
            bundle,
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
            last_judgement: "¡Listo!".to_string(),
            last_beat: -1,
            paused: false,
            finished: false,
            lane_pressed: [false; 4],
            lane_flash_timer: [0.0; 4],
        });

        self.screen = ActiveScreen::Gameplay;
    }

    fn handle_imported_file(&mut self, ctx: &egui::Context, ev: ImportedFileEvent) {
        match ev.kind {
            ImportKind::ChartJson => match std::fs::read_to_string(&ev.path) {
                Ok(raw) => match ChartData::from_json_str(&raw) {
                    Ok(chart) => {
                        self.toast_message = Some(format!(
                            "Chart '{}' importado ({} notas, {} BPM)",
                            chart.title, chart.stats.total_notes, chart.bpm
                        ));
                        self.project.chart = chart;
                        self.project.chart_source_path = Some(ev.path);
                    }
                    Err(e) => {
                        self.toast_message = Some(format!("Chart inválido: {}", e));
                    }
                },
                Err(e) => {
                    self.toast_message = Some(format!("No se pudo leer el chart: {}", e));
                }
            },
            ImportKind::AudioTrack(slot) => {
                if let Err(e) = self.project.assign_audio_track(slot, ev.path.clone()) {
                    self.toast_message = Some(e);
                    return;
                }
                let target_sr = self
                    .audio_engine
                    .mixer
                    .sample_rate
                    .load(std::sync::atomic::Ordering::Relaxed);
                let bg_tx = self.bg_tx.clone();
                self.toast_message = Some(format!("Decodificando pista {}...", slot + 1));
                self.tokio_rt.spawn_blocking(move || {
                    let res = decode_audio_file(&ev.path, target_sr)
                        .map(Arc::new)
                        .map_err(|e| e.to_string());
                    let _ = bg_tx.send(BackgroundEvent::AudioDecoded { slot, track: res });
                });
            }
            ImportKind::SpriteSheetPng => {
                let entity_id = self.selected_entity_id.clone();
                self.load_entity_texture_from_path(ctx, &entity_id, &ev.path);
                if let Some(entity) = self.project.stage.find_mut(&entity_id) {
                    entity.image_path = Some(ev.path);
                }
                self.toast_message = Some("Hoja PNG cargada en el objeto seleccionado".to_string());
            }
            ImportKind::SpriteAtlasXml => {
                if let Ok(xml) = std::fs::read_to_string(&ev.path) {
                    match SparrowAtlas::from_xml_str(&xml) {
                        Ok(atlas) => {
                            let anim_count = atlas.animations.len();
                            let entity_id = self.selected_entity_id.clone();
                            if let Some(entity) = self.project.stage.find_mut(&entity_id) {
                                entity.xml_path = Some(ev.path);
                                entity.atlas = Some(atlas);
                            }
                            self.toast_message = Some(format!(
                                "XML armado automáticamente: {} animaciones detectadas",
                                anim_count
                            ));
                        }
                        Err(e) => {
                            self.toast_message = Some(format!("Error XML Sparrow: {}", e));
                        }
                    }
                }
            }
            ImportKind::ArrowSvg(dir) => {
                if let Ok(svg_text) = std::fs::read_to_string(&ev.path) {
                    self.project.arrow_skin.custom_svg_paths.insert(dir, ev.path);
                    self.project.arrow_skin.custom_svg_sources.insert(dir, svg_text);
                    self.refresh_arrow_textures(ctx);
                    self.toast_message = Some(format!(
                        "Skin SVG actualizada para flecha {}",
                        dir.label_es()
                    ));
                }
            }
            ImportKind::ScriptFile => {
                if let Ok(src) = std::fs::read_to_string(&ev.path) {
                    let name = ev
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("custom.lua")
                        .to_string();
                    let lang = if name.ends_with(".ts") {
                        pulse_core::ScriptLanguage::TypeScript
                    } else if name.ends_with(".js") {
                        pulse_core::ScriptLanguage::JavaScript
                    } else {
                        pulse_core::ScriptLanguage::Lua
                    };
                    let id = format!("script_{}", self.project.scripts.len() + 1);
                    self.project.scripts.push(pulse_core::ScriptAsset {
                        id: id.clone(),
                        name,
                        language: lang,
                        source_code: src,
                        enabled: true,
                    });
                    self.selected_script_id = id;
                    self.toast_message = Some("Script comunitario importado".to_string());
                }
            }
        }
    }
}
