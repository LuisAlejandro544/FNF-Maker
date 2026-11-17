//! Archivo: engine/crates/pulse_core/src/project.rs
//! Propósito: Modela un proyecto completo de mod dentro de BeatForge Studio.
//! Controla:
//! - Chart activo
//! - Hasta máximo 3 pistas de audio simultáneas (Mix completo/Instrumental, Voces Player, Voces Rival)
//! - Escenario con todos los assets importados e identificados
//! - Scripts comunitarios en Lua, TypeScript o JavaScript
//! - Skin de flechas SVG seleccionada y opciones de gameplay (downscroll, offset Oboe)

use crate::chart::ChartData;
use crate::stage::StageScene;
use crate::svg_skin::ArrowSkinSet;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Máximo absoluto de pistas de música permitidas por proyecto según la especificación.
pub const MAX_AUDIO_TRACKS: usize = 3;

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
    pub scripts: Vec<ScriptAsset>,
    pub downscroll: bool,
    pub control_mode: MobileControlMode,
    pub audio_offset_ms: f64,
    pub scroll_speed_override: Option<f64>,
}

impl ModProject {
    pub fn new_demo() -> Self {
        let default_lua = r#"-- Script Lua de ejemplo para BeatForge Studio
function onCreate()
    log("Escena Lua iniciada!")
end

function onBeatHit(beat)
    if beat % 2 == 0 then
        setCameraZoom(1.04)
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
            stage: StageScene::default(),
            arrow_skin: ArrowSkinSet::bundled_default(),
            scripts: vec![
                ScriptAsset {
                    id: "script_beat_zoom".to_string(),
                    name: "camera_bounce.lua".to_string(),
                    language: ScriptLanguage::Lua,
                    source_code: default_lua.to_string(),
                    enabled: true,
                },
                ScriptAsset {
                    id: "script_note_fx".to_string(),
                    name: "note_effects.ts".to_string(),
                    language: ScriptLanguage::TypeScript,
                    source_code: default_ts.to_string(),
                    enabled: true,
                },
            ],
            downscroll: false,
            control_mode: MobileControlMode::TapArrows,
            audio_offset_ms: 0.0,
            scroll_speed_override: None,
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
