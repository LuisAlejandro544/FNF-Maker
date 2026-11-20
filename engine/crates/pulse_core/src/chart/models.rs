//! Archivo: engine/crates/pulse_core/src/chart/models.rs
//! Propósito: Modelos de datos del chart (`ChartData`, `ChartSection`, `ChartNote`,
//! `ChartEvent`, `ChartIntroUiConfig`, `ChartStats`, `NoteDirection`, `NoteSpecialKind`,
//! `ChartError`), cálculo de estadísticas y chart demo integrado.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errores posibles al importar o validar un archivo chart JSON.
#[derive(Debug, Error)]
pub enum ChartError {
    #[error("Error leyendo JSON del chart: {0}")]
    Json(#[from] serde_json::Error),
    #[error("El archivo JSON no contiene un objeto 'song' ni una lista de notas válida")]
    InvalidStructure,
    #[error("El chart no contiene secciones ni notas válidas")]
    EmptyNotes,
}

/// Dirección cardinal de cada flecha en un chart de 4 carriles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NoteDirection {
    Left = 0,
    Down = 1,
    Up = 2,
    Right = 3,
}

impl NoteDirection {
    pub const ALL: [NoteDirection; 4] = [
        NoteDirection::Left,
        NoteDirection::Down,
        NoteDirection::Up,
        NoteDirection::Right,
    ];

    pub fn from_lane(lane: u8) -> Self {
        match lane % 4 {
            0 => NoteDirection::Left,
            1 => NoteDirection::Down,
            2 => NoteDirection::Up,
            _ => NoteDirection::Right,
        }
    }

    pub fn label_es(self) -> &'static str {
        match self {
            NoteDirection::Left => "Izquierda",
            NoteDirection::Down => "Abajo",
            NoteDirection::Up => "Arriba",
            NoteDirection::Right => "Derecha",
        }
    }
}

/// Clasificación de tipo de nota especial soportada por BeatForge Studio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum NoteSpecialKind {
    Normal = 0,
    Hurt = 1,
    Instakill = 2,
    Dodge = 3,
    AltAnim = 4,
    NoAnim = 5,
}

impl NoteSpecialKind {
    pub fn from_str_loose(raw: &str) -> Self {
        let clean = raw.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        match clean.as_str() {
            "" | "default" | "normal" | "gf_sing" | "gf" => NoteSpecialKind::Normal,
            "hurt" | "hurt_note" | "mine" | "mina" | "damage" | "danger" | "peligro" | "x" => {
                NoteSpecialKind::Hurt
            }
            "instakill" | "insta_kill" | "death" | "death_note" | "kill" | "kill_mine" | "muerte" => {
                NoteSpecialKind::Instakill
            }
            "dodge" | "dodge_note" | "esquive" | "warning" | "parry" => NoteSpecialKind::Dodge,
            "alt" | "alt_animation" | "alt_anim" | "hey" => NoteSpecialKind::AltAnim,
            "no_anim" | "no_animation" | "sin_anim" => NoteSpecialKind::NoAnim,
            _ if clean.contains("instakill") || clean.contains("death") || clean.contains("kill") => {
                NoteSpecialKind::Instakill
            }
            _ if clean.contains("hurt") || clean.contains("mine") || clean.contains("mina") => {
                NoteSpecialKind::Hurt
            }
            _ if clean.contains("dodge") || clean.contains("esquiv") => NoteSpecialKind::Dodge,
            _ if clean.contains("alt") => NoteSpecialKind::AltAnim,
            _ if clean.contains("no_anim") || clean.contains("noanim") => NoteSpecialKind::NoAnim,
            _ => NoteSpecialKind::Normal,
        }
    }

    pub fn code(self) -> u8 {
        self as u8
    }

    pub fn from_code(code: u8) -> Self {
        match code {
            1 => NoteSpecialKind::Hurt,
            2 => NoteSpecialKind::Instakill,
            3 => NoteSpecialKind::Dodge,
            4 => NoteSpecialKind::AltAnim,
            5 => NoteSpecialKind::NoAnim,
            _ => NoteSpecialKind::Normal,
        }
    }

    pub fn canonical_key(self) -> &'static str {
        match self {
            NoteSpecialKind::Normal => "default",
            NoteSpecialKind::Hurt => "hurt",
            NoteSpecialKind::Instakill => "instakill",
            NoteSpecialKind::Dodge => "dodge",
            NoteSpecialKind::AltAnim => "alt",
            NoteSpecialKind::NoAnim => "no_anim",
        }
    }

    pub fn is_hazard_x(self) -> bool {
        matches!(self, NoteSpecialKind::Hurt | NoteSpecialKind::Instakill)
    }
}

/// Evento cronológico dentro de `chart.json` (ej. cambios de cámara, zoom, flash o texto).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartEvent {
    pub time_ms: f64,
    pub name: String,
    pub value1: String,
    pub value2: String,
}

/// Nota individual ya normalizada con su tiempo en milisegundos, lado (Player u Opponent)
/// y duración de nota sostenida (hold/sustain).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartNote {
    pub id: u64,
    pub time_ms: f64,
    pub direction: NoteDirection,
    pub is_player: bool,
    pub sustain_ms: f64,
    pub note_type: String,
    pub hit: bool,
    pub missed: bool,
}

/// Sección rítmica del chart (típicamente 16 steps / 4 beats).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartSection {
    pub index: usize,
    pub start_ms: f64,
    pub length_in_steps: u32,
    pub must_hit_section: bool,
    pub bpm: f64,
    pub change_bpm: bool,
    pub alt_anim: bool,
    pub gf_section: bool,
    /// Ranura explícita de cámara (`Cam 0`, `Cam 1`, `Cam 2`, `Cam 3+`) asignada en el chart (`None` = automático).
    #[serde(default)]
    pub camera_slot: Option<usize>,
}

/// Configuración declarativa de inicio del mod (pantalla negra, HUD oculto, flechas ocultas)
/// persistida dentro de `chart.json` (`song.uiBlackout`) para activarse desde el frame 0 (`onCreate`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ChartIntroUiConfig {
    #[serde(default)]
    pub start_blackout: bool,
    #[serde(default = "default_intro_blackout_alpha")]
    pub blackout_alpha: f32,
    #[serde(default)]
    pub hide_hud: bool,
    #[serde(default)]
    pub hide_arrows: bool,
    #[serde(default = "default_true_flag")]
    pub reveal_on_song_start: bool,
    #[serde(default)]
    pub reveal_delay_ms: f64,
    #[serde(default = "default_intro_fade_sec")]
    pub fade_sec: f32,
}

fn default_intro_blackout_alpha() -> f32 {
    1.0
}

fn default_true_flag() -> bool {
    true
}

fn default_intro_fade_sec() -> f32 {
    0.65
}

impl Default for ChartIntroUiConfig {
    fn default() -> Self {
        Self {
            start_blackout: false,
            blackout_alpha: 1.0,
            hide_hud: false,
            hide_arrows: false,
            reveal_on_song_start: true,
            reveal_delay_ms: 0.0,
            fade_sec: 0.65,
        }
    }
}

impl ChartIntroUiConfig {
    pub fn normalized(mut self) -> Self {
        self.blackout_alpha = if self.blackout_alpha.is_finite() {
            self.blackout_alpha.clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.reveal_delay_ms = if self.reveal_delay_ms.is_finite() {
            self.reveal_delay_ms.clamp(0.0, 60_000.0)
        } else {
            0.0
        };
        self.fade_sec = if self.fade_sec.is_finite() {
            self.fade_sec.clamp(0.0, 25.0)
        } else {
            0.65
        };
        self
    }

    pub fn is_active(&self) -> bool {
        self.start_blackout || self.hide_hud || self.hide_arrows
    }
}

/// Estadísticas calculadas automáticamente al importar o editar un chart para mostrar en la UI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChartStats {
    pub total_notes: usize,
    pub player_notes: usize,
    pub opponent_notes: usize,
    pub sustain_notes: usize,
    pub duration_ms: f64,
    pub density_buckets: Vec<u16>,
}

/// Estructura completa de `chart.json` compilada en memoria y lista para editar o exportar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    pub title: String,
    pub bpm: f64,
    pub speed: f64,
    pub needs_voices: bool,
    pub player1: String,
    pub player2: String,
    pub gf_version: String,
    pub stage: String,
    pub arrow_skin: String,
    pub splash_skin: String,
    pub valid_score: bool,
    #[serde(default)]
    pub intro_ui: ChartIntroUiConfig,
    pub sections: Vec<ChartSection>,
    pub notes: Vec<ChartNote>,
    pub events: Vec<ChartEvent>,
    pub stats: ChartStats,
}

impl ChartData {
    pub(super) fn compute_stats(notes: &[ChartNote], fallback_duration_ms: f64) -> ChartStats {
        let mut player_notes = 0usize;
        let mut opponent_notes = 0usize;
        let mut sustain_notes = 0usize;
        let mut max_ms = fallback_duration_ms;

        for n in notes {
            if n.is_player {
                player_notes += 1;
            } else {
                opponent_notes += 1;
            }
            if n.sustain_ms > 1.0 {
                sustain_notes += 1;
            }
            let end_ms = n.time_ms + n.sustain_ms;
            if end_ms > max_ms {
                max_ms = end_ms;
            }
        }

        let bucket_count = 32usize;
        let mut buckets = vec![0_u16; bucket_count];
        let safe_max = max_ms.max(1.0);
        for n in notes {
            let idx = (((n.time_ms / safe_max) * (bucket_count as f64)) as usize)
                .min(bucket_count - 1);
            buckets[idx] = buckets[idx].saturating_add(1);
        }

        ChartStats {
            total_notes: notes.len(),
            player_notes,
            opponent_notes,
            sustain_notes,
            duration_ms: max_ms,
            density_buckets: buckets,
        }
    }

    /// Genera un chart de demostración incluido (libre de derechos) para probar el motor al instante.
    pub fn bundled_demo_chart() -> Self {
        let bpm = 128.0_f64;
        let step_ms = (60_000.0 / bpm) / 4.0;
        let mut notes = Vec::new();
        let mut id = 1_u64;

        let mut sections = Vec::new();
        for sec in 0..8_usize {
            let must_hit = sec % 2 == 1;
            let start_ms = (sec as f64) * 16.0 * step_ms;
            sections.push(ChartSection {
                index: sec,
                start_ms,
                length_in_steps: 16,
                must_hit_section: must_hit,
                bpm,
                change_bpm: false,
                alt_anim: false,
                gf_section: false,
                camera_slot: None,
            });
            for step in [0_u32, 2, 4, 6, 8, 10, 12, 14] {
                let dir = NoteDirection::from_lane(((sec + step as usize / 2) % 4) as u8);
                notes.push(ChartNote {
                    id,
                    time_ms: start_ms + (step as f64) * step_ms,
                    direction: dir,
                    is_player: must_hit,
                    sustain_ms: if step == 12 { step_ms * 2.0 } else { 0.0 },
                    note_type: "default".to_string(),
                    hit: false,
                    missed: false,
                });
                id += 1;
            }
        }

        let duration_ms = 8.0 * 16.0 * step_ms;
        let stats = Self::compute_stats(&notes, duration_ms);

        Self {
            title: "Neon Pulse (Demo Integrada)".to_string(),
            bpm,
            speed: 2.1,
            needs_voices: true,
            player1: "player_main".to_string(),
            player2: "opponent_main".to_string(),
            gf_version: "stage_speakers".to_string(),
            stage: "main_stage".to_string(),
            arrow_skin: String::new(),
            splash_skin: String::new(),
            valid_score: true,
            intro_ui: ChartIntroUiConfig::default(),
            sections,
            notes,
            events: vec![ChartEvent {
                time_ms: 4.0 * 16.0 * step_ms,
                name: "Add Camera Zoom".to_string(),
                value1: "0.04".to_string(),
                value2: "0.02".to_string(),
            }],
            stats,
        }
    }
}
