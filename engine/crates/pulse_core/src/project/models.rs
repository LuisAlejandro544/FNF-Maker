//! Archivo: engine/crates/pulse_core/src/project/models.rs
//! Propósito: Define las estructuras de datos del proyecto (`ModProject`, `ScriptAsset`,
//! `AudioTrackSlot`, `ScriptLanguage`, `MobileControlMode`), constantes de límite/scripts
//! y métodos de inicialización, sincronización de scripts y asignación de pistas de audio.

use super::camera_lua::render_camera_lua_script;
use super::countdown::CountdownAssetsConfig;
use super::dev_api::{CURRENT_DEV_API_VERSION, LEGACY_DEV_API_VERSION};
use super::stage_events_lua::render_stage_events_lua_script;
use super::ui_blackout_lua::{UI_BLACKOUT_RUNTIME_V2_MARKER, render_ui_blackout_lua_script};
use crate::chart::ChartData;
use crate::gfx_backend::RenderBackendKind;
use crate::stage::StageScene;
use crate::svg_skin::ArrowSkinSet;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Máximo absoluto de pistas de música permitidas por proyecto según la especificación.
pub const MAX_AUDIO_TRACKS: usize = 3;

/// Identificador estable del script Lua que controla el sistema de cámara del escenario.
pub const CAMERA_SCRIPT_ID: &str = "script_camera_system";
/// Nombre visible del archivo Lua de cámara editable por el usuario en Script Studio.
pub const CAMERA_SCRIPT_NAME: &str = "camera_system.lua";

/// Identificador estable del script Lua que orquesta flashes, swaps de visibilidad, escenarios y personajes.
pub const STAGE_EVENTS_SCRIPT_ID: &str = "script_stage_events";
/// Nombre visible del archivo Lua de eventos de escenario y timeline en Script Studio.
pub const STAGE_EVENTS_SCRIPT_NAME: &str = "stage_events.lua";

/// Identificador estable del script Lua que controla pantalla en negro, visibilidad de HUD y flechas al iniciar o por eventos del chart.
pub const UI_BLACKOUT_SCRIPT_ID: &str = "script_ui_blackout";
/// Nombre visible del archivo Lua de pantalla en negro / visibilidad de UI en Script Studio.
pub const UI_BLACKOUT_SCRIPT_NAME: &str = "ui_blackout.lua";

/// Modo de control táctil en móvil:
/// - `TapArrows` (Por defecto): 100 % táctil directo tocando las 4 flechas receptoras.
/// - `ClassicHitbox`: Diseño clásico estilo Psych Engine Mobile (4 columnas hitbox verticales).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MobileControlMode {
    #[default]
    TapArrows,
    ClassicHitbox,
}

/// Lenguaje de un script comunitario integrado en el mod.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptLanguage {
    Lua,
    TypeScript,
    JavaScript,
}

impl ScriptLanguage {
    pub fn label(self) -> &'static str {
        match self {
            ScriptLanguage::Lua => "Lua 5.4",
            ScriptLanguage::TypeScript => "TypeScript",
            ScriptLanguage::JavaScript => "JavaScript",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            ScriptLanguage::Lua => "lua",
            ScriptLanguage::TypeScript => "ts",
            ScriptLanguage::JavaScript => "js",
        }
    }
}

/// Representa un archivo de script editable o importado en el proyecto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptAsset {
    pub id: String,
    pub name: String,
    pub language: ScriptLanguage,
    pub source_code: String,
    pub enabled: bool,
    /// JavaScript ya transpilado/validado durante `hot_compile` para omitir `oxc` al iniciar gameplay.
    #[serde(skip)]
    pub precompiled_js: Option<String>,
    /// Bytecode Lua 5.4 compilado con `dump(false)` en `hot_compile` para arranque instantáneo.
    #[serde(skip)]
    pub precompiled_bytecode: Option<Vec<u8>>,
}

/// Ranura de pista de audio (máximo 3: Principal/Instrumental, Voces Jugador, Voces Rival).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTrackSlot {
    pub slot_index: usize,
    pub role_label: String,
    pub file_path: Option<PathBuf>,
    pub file_name: Option<String>,
    pub gain: f32,
    pub muted: bool,
    pub duck_on_player_miss: bool,
}

impl AudioTrackSlot {
    pub fn default_three_slots() -> [AudioTrackSlot; MAX_AUDIO_TRACKS] {
        [
            AudioTrackSlot {
                slot_index: 0,
                role_label: "Pista 1: Mix Único o Instrumental".to_string(),
                file_path: None,
                file_name: None,
                gain: 1.0,
                muted: false,
                duck_on_player_miss: false,
            },
            AudioTrackSlot {
                slot_index: 1,
                role_label: "Pista 2: Voces Player (Opcional)".to_string(),
                file_path: None,
                file_name: None,
                gain: 1.0,
                muted: false,
                duck_on_player_miss: true,
            },
            AudioTrackSlot {
                slot_index: 2,
                role_label: "Pista 3: Voces Enemigo (Opcional)".to_string(),
                file_path: None,
                file_name: None,
                gain: 1.0,
                muted: false,
                duck_on_player_miss: false,
            },
        ]
    }
}

/// Marca de tiempo actual en milisegundos Unix para ordenar proyectos recientes.
pub fn current_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn default_audio_adapt_enabled() -> bool {
    true
}

fn default_legacy_dev_api_version() -> u32 {
    LEGACY_DEV_API_VERSION
}

/// Proyecto completo del usuario listo para editar o compilar en caliente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModProject {
    pub id: String,
    pub name: String,
    pub author: String,
    #[serde(default = "default_legacy_dev_api_version")]
    pub dev_api_version: u32,
    #[serde(default)]
    pub updated_at_ms: u64,
    pub chart: ChartData,
    pub chart_source_path: Option<PathBuf>,
    pub audio_tracks: [AudioTrackSlot; MAX_AUDIO_TRACKS],
    pub stage: StageScene,
    pub arrow_skin: ArrowSkinSet,
    #[serde(default)]
    pub health_bar_svg_path: Option<PathBuf>,
    #[serde(default)]
    pub health_bar_svg_source: Option<String>,
    pub scripts: Vec<ScriptAsset>,
    pub downscroll: bool,
    pub control_mode: MobileControlMode,
    pub audio_offset_ms: f64,
    pub scroll_speed_override: Option<f64>,
    #[serde(default)]
    pub preferred_render_backend: RenderBackendKind,
    #[serde(default)]
    pub render_backend_user_selected: bool,
    #[serde(default = "default_audio_adapt_enabled")]
    pub audio_adapt_enabled: bool,
    #[serde(default)]
    pub countdown: CountdownAssetsConfig,
}

impl ModProject {
    pub fn new_demo() -> Self {
        let stage = StageScene::default();
        let chart = ChartData::bundled_demo_chart();
        let camera_lua = render_camera_lua_script(&stage.camera, None);
        let stage_events_lua = render_stage_events_lua_script(&chart, &stage, None);
        let ui_blackout_lua = render_ui_blackout_lua_script(&chart, None);

        let default_lua = r#"-- Script Lua de ejemplo para BeatForge Studio
function onCreate()
    -- Escena Lua lista
end

function onBeatHit(beat)
    if beat % 2 == 0 then
        setCameraZoom(getCameraBaseZoom() + 0.04)
    end
end
"#;

        let default_ts = r#"// Script TypeScript de ejemplo (compilado por oxc en Rust)
interface NoteEvent {
    lane: number;
    isPlayer: boolean;
}

function onNoteHit(lane: number, isPlayer: boolean): void {
    const targetId: string = isPlayer ? "player_main" : "opponent_main";
    if (lane === 2) {
        setEntityProp(targetId, "opacity", 1.0);
    }
}
"#;

        Self {
            id: "proj_neon_pulse".to_string(),
            name: "Neon Pulse Demo".to_string(),
            author: "Creador Local".to_string(),
            dev_api_version: CURRENT_DEV_API_VERSION,
            updated_at_ms: current_epoch_ms(),
            chart,
            chart_source_path: None,
            audio_tracks: AudioTrackSlot::default_three_slots(),
            stage,
            arrow_skin: ArrowSkinSet::bundled_default(),
            health_bar_svg_path: None,
            health_bar_svg_source: None,
            scripts: vec![
                ScriptAsset {
                    id: CAMERA_SCRIPT_ID.to_string(),
                    name: CAMERA_SCRIPT_NAME.to_string(),
                    language: ScriptLanguage::Lua,
                    source_code: camera_lua,
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
                ScriptAsset {
                    id: STAGE_EVENTS_SCRIPT_ID.to_string(),
                    name: STAGE_EVENTS_SCRIPT_NAME.to_string(),
                    language: ScriptLanguage::Lua,
                    source_code: stage_events_lua,
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
                ScriptAsset {
                    id: UI_BLACKOUT_SCRIPT_ID.to_string(),
                    name: UI_BLACKOUT_SCRIPT_NAME.to_string(),
                    language: ScriptLanguage::Lua,
                    source_code: ui_blackout_lua,
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
                ScriptAsset {
                    id: "script_beat_zoom".to_string(),
                    name: "camera_bounce.lua".to_string(),
                    language: ScriptLanguage::Lua,
                    source_code: default_lua.to_string(),
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
                ScriptAsset {
                    id: "script_note_fx".to_string(),
                    name: "note_effects.ts".to_string(),
                    language: ScriptLanguage::TypeScript,
                    source_code: default_ts.to_string(),
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
            ],
            downscroll: false,
            control_mode: MobileControlMode::TapArrows,
            audio_offset_ms: 0.0,
            scroll_speed_override: None,
            preferred_render_backend: RenderBackendKind::Gles3,
            render_backend_user_selected: false,
            audio_adapt_enabled: true,
            countdown: CountdownAssetsConfig::default(),
        }
    }

    /// Garantiza que proyectos antiguos cargados desde disco cuenten con `camera_system.lua` actualizado (V2).
    pub fn ensure_camera_script(&mut self) {
        self.stage.camera.ensure_default_cameras();
        if let Some(script) = self
            .scripts
            .iter_mut()
            .find(|s| s.id == CAMERA_SCRIPT_ID || s.name == CAMERA_SCRIPT_NAME)
        {
            let rendered = render_camera_lua_script(&self.stage.camera, Some(&script.source_code));
            if rendered != script.source_code {
                script.source_code = rendered;
                script.precompiled_bytecode = None;
            }
        } else {
            let source_code = render_camera_lua_script(&self.stage.camera, None);
            self.scripts.insert(
                0,
                ScriptAsset {
                    id: CAMERA_SCRIPT_ID.to_string(),
                    name: CAMERA_SCRIPT_NAME.to_string(),
                    language: ScriptLanguage::Lua,
                    source_code,
                    enabled: self.stage.camera.enabled,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
            );
        }
    }

    /// Garantiza la presencia de `stage_events.lua` en proyectos nuevos o cargados desde disco.
    pub fn ensure_stage_events_script(&mut self) {
        let has_stage_events = self
            .scripts
            .iter()
            .any(|s| s.id == STAGE_EVENTS_SCRIPT_ID || s.name == STAGE_EVENTS_SCRIPT_NAME);
        if !has_stage_events {
            let source_code = render_stage_events_lua_script(&self.chart, &self.stage, None);
            let insert_idx = usize::from(
                self.scripts
                    .first()
                    .is_some_and(|s| s.id == CAMERA_SCRIPT_ID || s.name == CAMERA_SCRIPT_NAME),
            );
            self.scripts.insert(
                insert_idx.min(self.scripts.len()),
                ScriptAsset {
                    id: STAGE_EVENTS_SCRIPT_ID.to_string(),
                    name: STAGE_EVENTS_SCRIPT_NAME.to_string(),
                    language: ScriptLanguage::Lua,
                    source_code,
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
            );
        }
    }

    /// Resincroniza el bloque `-- [STAGE_CUES_START]..-- [STAGE_CUES_END]` de `stage_events.lua`
    /// tras editar cues en el Chart Editor o importar un chart nuevo.
    pub fn sync_stage_events_script(&mut self) {
        self.ensure_stage_events_script();
        if let Some(script) = self
            .scripts
            .iter_mut()
            .find(|s| s.id == STAGE_EVENTS_SCRIPT_ID || s.name == STAGE_EVENTS_SCRIPT_NAME)
        {
            script.source_code =
                render_stage_events_lua_script(&self.chart, &self.stage, Some(&script.source_code));
            script.precompiled_bytecode = None;
            script.precompiled_js = None;
        }
    }

    /// Garantiza la presencia de `ui_blackout.lua` actualizado (V2) en proyectos nuevos o cargados desde disco.
    pub fn ensure_ui_blackout_script(&mut self) {
        if let Some(script) = self
            .scripts
            .iter_mut()
            .find(|s| s.id == UI_BLACKOUT_SCRIPT_ID || s.name == UI_BLACKOUT_SCRIPT_NAME)
        {
            if !script.source_code.contains(UI_BLACKOUT_RUNTIME_V2_MARKER) {
                let upgraded = render_ui_blackout_lua_script(&self.chart, Some(&script.source_code));
                if upgraded != script.source_code {
                    script.source_code = upgraded;
                    script.precompiled_bytecode = None;
                    script.precompiled_js = None;
                }
            }
        } else {
            let source_code = render_ui_blackout_lua_script(&self.chart, None);
            let mut insert_idx = 0usize;
            for (idx, s) in self.scripts.iter().enumerate() {
                if s.id == CAMERA_SCRIPT_ID
                    || s.name == CAMERA_SCRIPT_NAME
                    || s.id == STAGE_EVENTS_SCRIPT_ID
                    || s.name == STAGE_EVENTS_SCRIPT_NAME
                {
                    insert_idx = idx + 1;
                }
            }
            self.scripts.insert(
                insert_idx.min(self.scripts.len()),
                ScriptAsset {
                    id: UI_BLACKOUT_SCRIPT_ID.to_string(),
                    name: UI_BLACKOUT_SCRIPT_NAME.to_string(),
                    language: ScriptLanguage::Lua,
                    source_code,
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
            );
        }
    }

    /// Resincroniza el bloque `-- [UI_BLACKOUT_START]..-- [UI_BLACKOUT_END]` de `ui_blackout.lua`
    /// tras editar `chart.intro_ui`, cues del Chart Editor o importar un chart.json externo.
    pub fn sync_ui_blackout_script(&mut self) {
        self.ensure_ui_blackout_script();
        if let Some(script) = self
            .scripts
            .iter_mut()
            .find(|s| s.id == UI_BLACKOUT_SCRIPT_ID || s.name == UI_BLACKOUT_SCRIPT_NAME)
        {
            script.source_code =
                render_ui_blackout_lua_script(&self.chart, Some(&script.source_code));
            script.precompiled_bytecode = None;
            script.precompiled_js = None;
        }
    }

    /// Crea un nuevo proyecto limpio listo para personalizar desde cero.
    pub fn new_blank(id: impl Into<String>, name: impl Into<String>, author: impl Into<String>) -> Self {
        let mut proj = Self::new_demo();
        proj.id = id.into();
        let clean_name = name.into();
        proj.name = clean_name.clone();
        proj.author = author.into();
        proj.chart.title = clean_name;
        proj.updated_at_ms = current_epoch_ms();
        proj
    }

    /// Asigna una pista de audio respetando el límite estricto de máximo 3 pistas.
    pub fn assign_audio_track(&mut self, slot_index: usize, path: PathBuf) -> Result<(), String> {
        if slot_index >= MAX_AUDIO_TRACKS {
            return Err(format!(
                "Máximo {} pistas permitidas; índice {} fuera de rango",
                MAX_AUDIO_TRACKS, slot_index
            ));
        }
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio")
            .to_string();
        self.audio_tracks[slot_index].file_path = Some(path);
        self.audio_tracks[slot_index].file_name = Some(file_name);
        Ok(())
    }

    pub fn active_audio_count(&self) -> usize {
        self.audio_tracks
            .iter()
            .filter(|t| t.file_path.is_some())
            .count()
    }
}
