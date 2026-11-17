//! Archivo: engine/crates/pulse_core/src/project.rs
//! Propósito: Modela un proyecto completo de mod dentro de BeatForge Studio.
//! Controla:
//! - Chart activo
//! - Hasta máximo 3 pistas de audio simultáneas (Mix completo/Instrumental, Voces Player, Voces Rival)
//! - Escenario con todos los assets importados e identificados
//! - Scripts comunitarios en Lua, TypeScript o JavaScript
//! - Skin de flechas SVG seleccionada y opciones de gameplay (downscroll, offset Oboe)

use crate::chart::ChartData;
use crate::stage::{CameraFollowMode, StageCameraConfig, StageScene};
use crate::svg_skin::ArrowSkinSet;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Máximo absoluto de pistas de música permitidas por proyecto según la especificación.
pub const MAX_AUDIO_TRACKS: usize = 3;

/// Identificador estable del script Lua que controla el sistema de cámara del escenario.
pub const CAMERA_SCRIPT_ID: &str = "script_camera_system";
/// Nombre visible del archivo Lua de cámara editable por el usuario en Script Studio.
pub const CAMERA_SCRIPT_NAME: &str = "camera_system.lua";

const CAM_CFG_START_MARKER: &str = "-- [CAM_CFG_START]";
const CAM_CFG_END_MARKER: &str = "-- [CAM_CFG_END]";

/// Genera o actualiza el bloque de parámetros del script `camera_system.lua` preservando
/// cualquier lógica personalizada que el usuario haya añadido debajo de los marcadores.
pub fn render_camera_lua_script(cfg: &StageCameraConfig, existing_source: Option<&str>) -> String {
    let config_block = format!(
        "{}\n\
local CAM_ENABLED = {}\n\
local CAM_BASE_X = {:.1}\n\
local CAM_BASE_Y = {:.1}\n\
local CAM_BASE_ZOOM = {:.3}\n\
local CAM_MODE = \"{}\" -- \"beats\" = cada N beats | \"sections\" = chart.json mustHitSection\n\
local CAM_INTERVAL_BEATS = {}\n\
local CAM_SMOOTH = {:.2}\n\
local CAM_PAN_STRENGTH = {:.2}\n\
{}",
        CAM_CFG_START_MARKER,
        if cfg.enabled { "true" } else { "false" },
        cfg.base_x,
        cfg.base_y,
        cfg.base_zoom,
        cfg.follow_mode.as_lua_str(),
        cfg.follow_interval_beats.max(1),
        cfg.smooth_speed,
        cfg.pan_strength,
        CAM_CFG_END_MARKER
    );

    if let Some(src) = existing_source {
        if let (Some(start_idx), Some(end_idx)) =
            (src.find(CAM_CFG_START_MARKER), src.find(CAM_CFG_END_MARKER))
        {
            if end_idx >= start_idx {
                let tail_start = end_idx + CAM_CFG_END_MARKER.len();
                let prefix = &src[..start_idx];
                let suffix = &src[tail_start..];
                return format!("{}{}{}", prefix, config_block, suffix);
            }
        }
    }

    format!(
        "-- Script real del Sistema de Cámara (BeatForge Studio)\n\
-- Sincronizado automáticamente con el editor visual del Escenario.\n\
-- Puedes editar estos valores o personalizar el comportamiento abajo en Lua 5.4:\n\
{}\n\n\
local targetX = CAM_BASE_X\n\
local targetY = CAM_BASE_Y\n\
local focusPlayer = false\n\n\
local function updateCameraTarget(isPlayerTurn)\n\
    local roleId = isPlayerTurn and \"player\" or \"opponent\"\n\
    local charX = getEntityX(roleId)\n\
    local charY = getEntityY(roleId)\n\
    if charX ~= nil and charY ~= nil then\n\
        targetX = CAM_BASE_X + (charX - CAM_BASE_X) * CAM_PAN_STRENGTH\n\
        targetY = CAM_BASE_Y + ((charY - 24.0) - CAM_BASE_Y) * CAM_PAN_STRENGTH\n\
    else\n\
        targetX = CAM_BASE_X\n\
        targetY = CAM_BASE_Y\n\
    end\n\
end\n\n\
function onCreate()\n\
    setCameraBaseZoom(CAM_BASE_ZOOM)\n\
    setCameraZoom(CAM_BASE_ZOOM)\n\
    if not CAM_ENABLED then\n\
        setCameraPos(CAM_BASE_X, CAM_BASE_Y)\n\
        return\n\
    end\n\
    if CAM_MODE == \"sections\" then\n\
        focusPlayer = getMustHitSection()\n\
    else\n\
        focusPlayer = false\n\
    end\n\
    updateCameraTarget(focusPlayer)\n\
    setCameraPos(targetX, targetY)\n\
end\n\n\
function onBeatHit(beat)\n\
    if not CAM_ENABLED then return end\n\
    if CAM_MODE == \"sections\" then\n\
        focusPlayer = getMustHitSection()\n\
    else\n\
        local step = math.max(1, CAM_INTERVAL_BEATS)\n\
        focusPlayer = (math.floor(beat / step) % 2) == 1\n\
    end\n\
    updateCameraTarget(focusPlayer)\n\
end\n\n\
function onUpdate(dt)\n\
    if not CAM_ENABLED then\n\
        setCameraPos(CAM_BASE_X, CAM_BASE_Y)\n\
        return\n\
    end\n\
    local cx = getCameraX()\n\
    local cy = getCameraY()\n\
    local t = 1.0 - math.exp(-CAM_SMOOTH * math.max(0.0, dt))\n\
    local nx = cx + (targetX - cx) * t\n\
    local ny = cy + (targetY - cy) * t\n\
    setCameraPos(nx, ny)\n\
end\n",
        config_block
    )
}

/// Extrae los parámetros de cámara editados manualmente dentro de `camera_system.lua`
/// para mantener sincronizado el rectángulo visual del escenario y el inspector.
pub fn parse_camera_config_from_lua(source: &str, current: &StageCameraConfig) -> StageCameraConfig {
    let mut next = current.clone();
    for line in source.lines() {
        let trimmed = line.trim();
        let no_comment = trimmed.split("--").next().unwrap_or("").trim();
        let Some((lhs, rhs)) = no_comment.split_once('=') else {
            continue;
        };
        let key = lhs.trim().trim_start_matches("local").trim();
        let val = rhs.trim();
        match key {
            "CAM_ENABLED" => {
                if val.eq_ignore_ascii_case("true") {
                    next.enabled = true;
                } else if val.eq_ignore_ascii_case("false") {
                    next.enabled = false;
                }
            }
            "CAM_BASE_X" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        next.base_x = v.clamp(-1000.0, 2600.0);
                    }
                }
            }
            "CAM_BASE_Y" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        next.base_y = v.clamp(-1000.0, 1800.0);
                    }
                }
            }
            "CAM_BASE_ZOOM" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        next.base_zoom = v.clamp(0.35, 2.5);
                    }
                }
            }
            "CAM_MODE" => {
                let clean = val.trim_matches('"').trim_matches('\'');
                next.follow_mode = CameraFollowMode::from_str_loose(clean);
            }
            "CAM_INTERVAL_BEATS" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        next.follow_interval_beats = (v.round() as i32).clamp(1, 32) as u32;
                    }
                }
            }
            "CAM_SMOOTH" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        next.smooth_speed = v.clamp(1.0, 20.0);
                    }
                }
            }
            "CAM_PAN_STRENGTH" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        next.pan_strength = v.clamp(0.0, 1.0);
                    }
                }
            }
            _ => {}
        }
    }
    next
}

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

/// Proyecto completo del usuario listo para editar o compilar en caliente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModProject {
    pub id: String,
    pub name: String,
    pub author: String,
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
}

impl ModProject {
    pub fn new_demo() -> Self {
        let stage = StageScene::default();
        let camera_lua = render_camera_lua_script(&stage.camera, None);

        let default_lua = r#"-- Script Lua de ejemplo para BeatForge Studio
function onCreate()
    log("Escena Lua iniciada!")
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
            updated_at_ms: current_epoch_ms(),
            chart: ChartData::bundled_demo_chart(),
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
        }
    }

    /// Garantiza que proyectos antiguos cargados desde disco cuenten con `camera_system.lua`.
    pub fn ensure_camera_script(&mut self) {
        let has_camera_script = self
            .scripts
            .iter()
            .any(|s| s.id == CAMERA_SCRIPT_ID || s.name == CAMERA_SCRIPT_NAME);
        if !has_camera_script {
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
