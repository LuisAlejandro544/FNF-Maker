//! Archivo: engine/crates/pulse_core/src/chart.rs
//! Propósito: Analiza e importa archivos de chart `.json` compatibles con el formato
//! estándar de mods de ritmo (secciones con `sectionNotes`, `mustHitSection`, `bpm`,
//! `speed` y notas `[tiempoMs, carril, duracionSustainMs]`).
//! Convierte cualquier variante de JSON comunitario en una línea temporal ordenada
//! lista para ser jugada y sincronizada con el reloj de audio de Oboe.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Errores posibles al importar o validar un archivo chart JSON.
#[derive(Debug, Error)]
pub enum ChartError {
    #[error("Error leyendo JSON del chart: {0}")]
    Json(#[from] serde_json::Error),
    #[error("El archivo JSON no contiene un objeto 'song' ni una lista de notas válida")]
    InvalidStructure,
    #[error("El chart no contiene notas válidas")]
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
}

/// Estadísticas calculadas automáticamente al importar un chart para mostrar en la UI.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChartStats {
    pub total_notes: usize,
    pub player_notes: usize,
    pub opponent_notes: usize,
    pub sustain_notes: usize,
    pub duration_ms: f64,
    pub density_buckets: Vec<u16>,
}

/// Estructura completa del chart compilado en memoria.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartData {
    pub title: String,
    pub bpm: f64,
    pub speed: f64,
    pub needs_voices: bool,
    pub sections: Vec<ChartSection>,
    pub notes: Vec<ChartNote>,
    pub stats: ChartStats,
}

impl ChartData {
    /// Parsea un archivo JSON de chart (soporta tanto formato anidado `{ "song": { ... } }`
    /// como formato plano `{ "notes": [...], "bpm": ... }`).
    pub fn from_json_str(raw: &str) -> Result<Self, ChartError> {
        let root: Value = serde_json::from_str(raw)?;
        let song_obj = root
            .get("song")
            .filter(|v| v.is_object())
            .unwrap_or(&root);

        let title = song_obj
            .get("song")
            .and_then(|v| v.as_str())
            .or_else(|| song_obj.get("songName").and_then(|v| v.as_str()))
            .unwrap_or("Pista Importada")
            .to_string();

        let base_bpm = song_obj
            .get("bpm")
            .and_then(|v| v.as_f64())
            .filter(|b| *b > 10.0)
            .unwrap_or(120.0);

        let speed = song_obj
            .get("speed")
            .and_then(|v| v.as_f64())
            .filter(|s| *s > 0.1)
            .unwrap_or(1.8);

        let needs_voices = song_obj
            .get("needsVoices")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let raw_sections = song_obj
            .get("notes")
            .and_then(|v| v.as_array())
            .ok_or(ChartError::InvalidStructure)?;

        let mut sections = Vec::with_capacity(raw_sections.len());
        let mut notes = Vec::new();
        let mut current_bpm = base_bpm;
        let mut cursor_ms = 0.0_f64;
        let mut next_id = 1_u64;

        for (sec_idx, sec_val) in raw_sections.iter().enumerate() {
            let change_bpm = sec_val
                .get("changeBPM")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if change_bpm {
                if let Some(new_bpm) = sec_val.get("bpm").and_then(|v| v.as_f64()) {
                    if new_bpm > 10.0 {
                        current_bpm = new_bpm;
                    }
                }
            }

            let length_in_steps = sec_val
                .get("lengthInSteps")
                .and_then(|v| v.as_u64())
                .unwrap_or(16) as u32;

            let must_hit = sec_val
                .get("mustHitSection")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            sections.push(ChartSection {
                index: sec_idx,
                start_ms: cursor_ms,
                length_in_steps,
                must_hit_section: must_hit,
                bpm: current_bpm,
            });

            if let Some(sec_notes) = sec_val.get("sectionNotes").and_then(|v| v.as_array()) {
                for raw_note in sec_notes {
                    if let Some(arr) = raw_note.as_array() {
                        if arr.len() < 2 {
                            continue;
                        }
                        let time_ms = arr[0].as_f64().unwrap_or(0.0).max(0.0);
                        let raw_lane = arr[1].as_i64().unwrap_or(0);
                        if raw_lane < 0 {
                            continue;
                        }
                        let lane = (raw_lane as u8) % 8;
                        let sustain_ms = arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0);
                        let note_type = arr
                            .get(3)
                            .and_then(|v| v.as_str())
                            .unwrap_or("default")
                            .to_string();

                        // En el formato estándar, si `mustHitSection` es true:
                        // carriles 0..3 son Player y 4..7 son Opponent.
                        // Si `mustHitSection` es false:
                        // carriles 0..3 son Opponent y 4..7 son Player.
                        let is_first_group = lane < 4;
                        let is_player = if must_hit { is_first_group } else { !is_first_group };

                        notes.push(ChartNote {
                            id: next_id,
                            time_ms,
                            direction: NoteDirection::from_lane(lane),
                            is_player,
                            sustain_ms,
                            note_type,
                            hit: false,
                            missed: false,
                        });
                        next_id += 1;
                    }
                }
            }

            let step_ms = (60_000.0 / current_bpm) / 4.0;
            cursor_ms += step_ms * (length_in_steps as f64);
        }

        if notes.is_empty() {
            return Err(ChartError::EmptyNotes);
        }

        // Ordenar cronológicamente para búsqueda binaria/puntero O(1) durante gameplay.
        notes.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));

        let stats = Self::compute_stats(&notes, cursor_ms);

        Ok(Self {
            title,
            bpm: base_bpm,
            speed,
            needs_voices,
            sections,
            notes,
            stats,
        })
    }

    fn compute_stats(notes: &[ChartNote], fallback_duration_ms: f64) -> ChartStats {
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

        // 8 secciones alternando entre Rival (Opponent) y Jugador (Player).
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
            needs_voices: false,
            sections,
            notes,
            stats,
        }
    }
}
