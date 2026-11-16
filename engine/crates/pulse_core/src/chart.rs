//! Archivo: engine/crates/pulse_core/src/chart.rs
//! Propósito: Analiza, edita y exporta archivos completos de `chart.json` compatibles con el
//! formato estándar comunitario de mods de ritmo (`song`, `bpm`, `speed`, `needsVoices`,
//! `player1`, `player2`, `gfVersion`, `stage`, `arrowSkin`, `splashSkin`, `validScore`,
//! `events`, secciones con `sectionNotes`, `sectionBeats`, `lengthInSteps`, `mustHitSection`,
//! `changeBPM`, `altAnim`, `gfSection` y notas `[tiempoMs, carril, duracionSustainMs, tipoNota]`).

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
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
    pub sections: Vec<ChartSection>,
    pub notes: Vec<ChartNote>,
    pub events: Vec<ChartEvent>,
    pub stats: ChartStats,
}

impl ChartData {
    /// Parsea un archivo `chart.json` completo (soporta formato `{ "song": { ... } }` y plano).
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

        let player1 = song_obj
            .get("player1")
            .and_then(|v| v.as_str())
            .unwrap_or("player_main")
            .to_string();
        let player2 = song_obj
            .get("player2")
            .and_then(|v| v.as_str())
            .unwrap_or("opponent_main")
            .to_string();
        let gf_version = song_obj
            .get("gfVersion")
            .or_else(|| song_obj.get("player3"))
            .and_then(|v| v.as_str())
            .unwrap_or("stage_speakers")
            .to_string();
        let stage = song_obj
            .get("stage")
            .and_then(|v| v.as_str())
            .unwrap_or("main_stage")
            .to_string();
        let arrow_skin = song_obj
            .get("arrowSkin")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let splash_skin = song_obj
            .get("splashSkin")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let valid_score = song_obj
            .get("validScore")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let raw_sections = song_obj
            .get("notes")
            .and_then(|v| v.as_array())
            .ok_or(ChartError::InvalidStructure)?;

        let mut sections = Vec::with_capacity(raw_sections.len());
        let mut notes = Vec::new();
        let mut events = Vec::new();
        let mut current_bpm = base_bpm;
        let mut cursor_ms = 0.0_f64;
        let mut next_id = 1_u64;

        // Parsear arreglo global `events` (formato estándar `[[timeMs, [[name, v1, v2], ...]], ...]`)
        if let Some(raw_events) = song_obj
            .get("events")
            .or_else(|| root.get("events"))
            .and_then(|v| v.as_array())
        {
            for ev_group in raw_events {
                if let Some(arr) = ev_group.as_array() {
                    let time_ms = arr.first().and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0);
                    if let Some(inner_list) = arr.get(1).and_then(|v| v.as_array()) {
                        for entry in inner_list {
                            if let Some(e_arr) = entry.as_array() {
                                let name = e_arr.first().and_then(|v| v.as_str()).unwrap_or("").to_string();
                                if name.is_empty() {
                                    continue;
                                }
                                let value1 = value_to_compact_string(e_arr.get(1));
                                let value2 = value_to_compact_string(e_arr.get(2));
                                events.push(ChartEvent {
                                    time_ms,
                                    name,
                                    value1,
                                    value2,
                                });
                            }
                        }
                    }
                }
            }
        }

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
                .map(|s| s as u32)
                .or_else(|| {
                    sec_val
                        .get("sectionBeats")
                        .and_then(|v| v.as_f64())
                        .map(|b| (b * 4.0).round().max(4.0) as u32)
                })
                .unwrap_or(16)
                .max(4);

            let must_hit = sec_val
                .get("mustHitSection")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let alt_anim = sec_val
                .get("altAnim")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let gf_section = sec_val
                .get("gfSection")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            sections.push(ChartSection {
                index: sec_idx,
                start_ms: cursor_ms,
                length_in_steps,
                must_hit_section: must_hit,
                bpm: current_bpm,
                change_bpm,
                alt_anim,
                gf_section,
            });

            if let Some(sec_notes) = sec_val.get("sectionNotes").and_then(|v| v.as_array()) {
                for raw_note in sec_notes {
                    if let Some(arr) = raw_note.as_array() {
                        if arr.len() < 2 {
                            continue;
                        }
                        let time_ms = arr[0].as_f64().unwrap_or(0.0).max(0.0);
                        let raw_lane = arr[1].as_i64().unwrap_or(0);
                        // En algunos charts, `lane == -1` codifica un evento en línea `[time, -1, name, v1, v2]`
                        if raw_lane == -1 {
                            let ev_name = arr.get(2).and_then(|v| v.as_str()).unwrap_or("").to_string();
                            if !ev_name.is_empty() {
                                events.push(ChartEvent {
                                    time_ms,
                                    name: ev_name,
                                    value1: value_to_compact_string(arr.get(3)),
                                    value2: value_to_compact_string(arr.get(4)),
                                });
                            }
                            continue;
                        }
                        if raw_lane < 0 {
                            continue;
                        }
                        let lane = (raw_lane as u8) % 8;
                        let sustain_ms = arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0).max(0.0);
                        let note_type = match arr.get(3) {
                            Some(Value::String(s)) if !s.trim().is_empty() => s.clone(),
                            Some(Value::Bool(true)) => "Alt Animation".to_string(),
                            Some(Value::Number(n)) => format!("type_{}", n),
                            _ => "default".to_string(),
                        };

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

        if sections.is_empty() && notes.is_empty() {
            return Err(ChartError::EmptyNotes);
        }

        notes.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        events.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));

        let stats = Self::compute_stats(&notes, cursor_ms);

        Ok(Self {
            title,
            bpm: base_bpm,
            speed,
            needs_voices,
            player1,
            player2,
            gf_version,
            stage,
            arrow_skin,
            splash_skin,
            valid_score,
            sections,
            notes,
            events,
            stats,
        })
    }

    /// Serializa el chart completo en formato estándar `chart.json` listo para compartir o usar en otro motor.
    pub fn to_json_str(&self) -> String {
        let mut section_buckets: Vec<Vec<&ChartNote>> = vec![Vec::new(); self.sections.len()];
        for note in &self.notes {
            let mut target_idx = self.sections.len().saturating_sub(1);
            for (idx, sec) in self.sections.iter().enumerate() {
                let next_start = self
                    .sections
                    .get(idx + 1)
                    .map(|s| s.start_ms)
                    .unwrap_or(f64::INFINITY);
                if note.time_ms >= sec.start_ms - 1.0 && note.time_ms < next_start - 1.0 {
                    target_idx = idx;
                    break;
                }
            }
            if let Some(bucket) = section_buckets.get_mut(target_idx) {
                bucket.push(note);
            }
        }

        let json_sections: Vec<Value> = self
            .sections
            .iter()
            .enumerate()
            .map(|(idx, sec)| {
                let sec_notes: Vec<Value> = section_buckets
                    .get(idx)
                    .into_iter()
                    .flatten()
                    .map(|n| {
                        let dir_idx = n.direction as u8;
                        // Convención estándar de `mustHitSection`:
                        // true -> 0..3 Jugador, 4..7 Oponente; false -> 0..3 Oponente, 4..7 Jugador.
                        let lane = if sec.must_hit_section {
                            if n.is_player { dir_idx } else { dir_idx + 4 }
                        } else if n.is_player {
                            dir_idx + 4
                        } else {
                            dir_idx
                        };
                        if n.note_type != "default" && !n.note_type.is_empty() {
                            json!([
                                round_ms(n.time_ms),
                                lane,
                                round_ms(n.sustain_ms),
                                n.note_type
                            ])
                        } else {
                            json!([round_ms(n.time_ms), lane, round_ms(n.sustain_ms)])
                        }
                    })
                    .collect();

                json!({
                    "lengthInSteps": sec.length_in_steps,
                    "sectionBeats": (sec.length_in_steps as f64) / 4.0,
                    "mustHitSection": sec.must_hit_section,
                    "bpm": sec.bpm,
                    "changeBPM": sec.change_bpm,
                    "altAnim": sec.alt_anim,
                    "gfSection": sec.gf_section,
                    "typeOfSection": 0,
                    "sectionNotes": sec_notes
                })
            })
            .collect();

        // Agrupar eventos por tiempo (en ms) al estilo estándar `[[timeMs, [[name, v1, v2], ...]]]`
        let mut grouped_events: Vec<Value> = Vec::new();
        for ev in &self.events {
            let t = round_ms(ev.time_ms);
            let entry = json!([ev.name, ev.value1, ev.value2]);
            if let Some(last) = grouped_events.last_mut() {
                if last.get(0).and_then(|v| v.as_f64()) == Some(t) {
                    if let Some(arr) = last.get_mut(1).and_then(|v| v.as_array_mut()) {
                        arr.push(entry);
                        continue;
                    }
                }
            }
            grouped_events.push(json!([t, [entry]]));
        }

        let root = json!({
            "song": {
                "song": self.title,
                "bpm": self.bpm,
                "speed": self.speed,
                "needsVoices": self.needs_voices,
                "player1": self.player1,
                "player2": self.player2,
                "gfVersion": self.gf_version,
                "stage": self.stage,
                "arrowSkin": self.arrow_skin,
                "splashSkin": self.splash_skin,
                "validScore": self.valid_score,
                "events": grouped_events,
                "notes": json_sections
            }
        });

        serde_json::to_string_pretty(&root).unwrap_or_else(|_| "{}".to_string())
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

    /// Recalcula los tiempos `start_ms` de todas las secciones respetando cambios individuales de BPM.
    pub fn recompute_stats(&mut self) {
        let mut cursor_ms = 0.0_f64;
        let mut active_bpm = self.bpm.max(40.0);
        for (idx, sec) in self.sections.iter_mut().enumerate() {
            sec.index = idx;
            if sec.change_bpm && sec.bpm >= 40.0 {
                active_bpm = sec.bpm;
            } else {
                sec.bpm = active_bpm;
            }
            sec.start_ms = cursor_ms;
            let step_ms = (60_000.0 / sec.bpm.max(40.0)) / 4.0;
            cursor_ms += step_ms * (sec.length_in_steps.max(4) as f64);
        }
        self.notes.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        self.events.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        self.stats = Self::compute_stats(&self.notes, cursor_ms);
    }

    /// Cambia el BPM base del chart y actualiza las secciones que no tuvieron cambio manual de BPM.
    pub fn set_bpm(&mut self, new_bpm: f64) {
        let clamped = new_bpm.clamp(40.0, 320.0);
        self.bpm = clamped;
        for sec in &mut self.sections {
            if !sec.change_bpm {
                sec.bpm = clamped;
            }
        }
        self.recompute_stats();
    }

    /// Cambia el BPM de una sección específica (habilitando o deshabilitando `changeBPM`).
    pub fn set_section_bpm(&mut self, sec_index: usize, new_bpm: f64, change_bpm: bool) {
        if let Some(sec) = self.sections.get_mut(sec_index) {
            sec.change_bpm = change_bpm;
            sec.bpm = if change_bpm {
                new_bpm.clamp(40.0, 320.0)
            } else {
                self.bpm.max(40.0)
            };
        }
        self.recompute_stats();
    }

    /// Actualiza los metadatos completos de `chart.json` (`song`, `speed`, personajes, escenario, voces).
    pub fn set_metadata(
        &mut self,
        title: &str,
        speed: f64,
        player1: &str,
        player2: &str,
        gf_version: &str,
        stage: &str,
        needs_voices: bool,
    ) {
        if !title.trim().is_empty() {
            self.title = title.trim().to_string();
        }
        self.speed = speed.clamp(0.5, 5.0);
        if !player1.trim().is_empty() {
            self.player1 = player1.trim().to_string();
        }
        if !player2.trim().is_empty() {
            self.player2 = player2.trim().to_string();
        }
        if !gf_version.trim().is_empty() {
            self.gf_version = gf_version.trim().to_string();
        }
        if !stage.trim().is_empty() {
            self.stage = stage.trim().to_string();
        }
        self.needs_voices = needs_voices;
    }

    pub fn add_event(&mut self, time_ms: f64, name: &str, value1: &str, value2: &str) {
        let clean = name.trim();
        if clean.is_empty() {
            return;
        }
        self.events.push(ChartEvent {
            time_ms: time_ms.max(0.0),
            name: clean.to_string(),
            value1: value1.trim().to_string(),
            value2: value2.trim().to_string(),
        });
        self.recompute_stats();
    }

    pub fn remove_event(&mut self, index: usize) {
        if index < self.events.len() {
            self.events.remove(index);
            self.recompute_stats();
        }
    }

    /// Añade una nueva sección vacía de 16 steps al final para seguir charteando canciones largas.
    pub fn add_section(&mut self, must_hit_section: bool) -> usize {
        let idx = self.sections.len();
        let last_bpm = self.sections.last().map(|s| s.bpm).unwrap_or(self.bpm).max(40.0);
        let start_ms = self
            .sections
            .last()
            .map(|s| {
                let step_ms = (60_000.0 / s.bpm.max(40.0)) / 4.0;
                s.start_ms + step_ms * (s.length_in_steps.max(4) as f64)
            })
            .unwrap_or(0.0);
        self.sections.push(ChartSection {
            index: idx,
            start_ms,
            length_in_steps: 16,
            must_hit_section,
            bpm: last_bpm,
            change_bpm: false,
            alt_anim: false,
            gf_section: false,
        });
        self.recompute_stats();
        idx
    }

    /// Alterna quién tiene el foco principal de la sección (`must_hit_section`).
    pub fn toggle_section_must_hit(&mut self, sec_index: usize) {
        if let Some(sec) = self.sections.get_mut(sec_index) {
            sec.must_hit_section = !sec.must_hit_section;
        }
    }

    /// Coloca o quita una nota manualmente en `(sec_index, step_in_sec, col_0_to_7)`:
    /// columnas `0..=3` corresponden a Oponente y `4..=7` al Jugador.
    pub fn toggle_note_at(
        &mut self,
        sec_index: usize,
        step_in_sec: u32,
        col_0_to_7: u8,
        sustain_steps: u32,
    ) -> bool {
        if sec_index >= self.sections.len() {
            return false;
        }
        let sec = &self.sections[sec_index];
        let step_ms = (60_000.0 / sec.bpm.max(40.0)) / 4.0;
        let target_ms = sec.start_ms + (step_in_sec as f64) * step_ms;
        let is_player = col_0_to_7 >= 4;
        let dir = NoteDirection::from_lane(col_0_to_7 % 4);
        let tolerance_ms = step_ms * 0.45;

        if let Some(existing_idx) = self.notes.iter().position(|n| {
            n.is_player == is_player
                && n.direction == dir
                && (n.time_ms - target_ms).abs() <= tolerance_ms
        }) {
            self.notes.remove(existing_idx);
            self.recompute_stats();
            false
        } else {
            let next_id = self.notes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
            let sustain_ms = (sustain_steps as f64) * step_ms;
            self.notes.push(ChartNote {
                id: next_id,
                time_ms: target_ms,
                direction: dir,
                is_player,
                sustain_ms,
                note_type: "default".to_string(),
                hit: false,
                missed: false,
            });
            self.recompute_stats();
            true
        }
    }

    /// Devuelve el BPM activo en un instante dado de la canción (respetando cambios de BPM por sección).
    pub fn bpm_at_ms(&self, song_pos_ms: f64) -> f64 {
        let mut active = self.bpm.max(40.0);
        for sec in &self.sections {
            if song_pos_ms >= sec.start_ms {
                active = sec.bpm.max(40.0);
            } else {
                break;
            }
        }
        active
    }
}

fn value_to_compact_string(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::Bool(b)) => b.to_string(),
        _ => String::new(),
    }
}

fn round_ms(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod manual_chart_tests {
    use super::*;

    #[test]
    fn manual_toggle_and_add_section_updates_stats() {
        let mut chart = ChartData::bundled_demo_chart();
        let initial_sections = chart.sections.len();
        let new_idx = chart.add_section(true);
        assert_eq!(new_idx, initial_sections);

        let placed = chart.toggle_note_at(new_idx, 4, 5, 2);
        assert!(placed);
        assert!(chart.notes.iter().any(|n| n.is_player && n.direction == NoteDirection::Down && n.sustain_ms > 0.0));

        let removed = chart.toggle_note_at(new_idx, 4, 5, 0);
        assert!(!removed);
    }

    #[test]
    fn complete_chart_json_round_trip_preserves_metadata_events_and_lanes() {
        let mut chart = ChartData::bundled_demo_chart();
        chart.set_metadata("Smoke Track", 2.6, "bf_custom", "garcello_custom", "gf_none", "alley", true);
        chart.set_section_bpm(1, 160.0, true);
        chart.add_event(2500.0, "Camera Flash", "0.5", "ffffff");

        let exported = chart.to_json_str();
        let reimported = ChartData::from_json_str(&exported).expect("reimportación válida");

        assert_eq!(reimported.title, "Smoke Track");
        assert_eq!(reimported.player1, "bf_custom");
        assert_eq!(reimported.player2, "garcello_custom");
        assert_eq!(reimported.gf_version, "gf_none");
        assert_eq!(reimported.stage, "alley");
        assert_eq!(reimported.sections[1].bpm, 160.0);
        assert!(reimported.sections[1].change_bpm);
        assert_eq!(reimported.notes.len(), chart.notes.len());
        assert!(reimported.events.iter().any(|e| e.name == "Camera Flash"));
    }
}
