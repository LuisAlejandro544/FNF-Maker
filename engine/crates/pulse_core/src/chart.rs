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
    /// Parsea un archivo `chart.json` completo.
    /// Soporta formato estándar `{ "song": { ... } }`, doble envoltura `{ "song": { "song": { ... } } }`,
    /// envolturas `{ "chart": { ... } }` / `{ "data": { ... } }`, formato moderno V-Slice
    /// (`"notes": { "hard": [...] }`), formato Codename Engine (`"strumLines": [...]`),
    /// arreglo `"sections"` o listas planas de notas, además de limpiar BOM UTF-8.
    pub fn from_json_str(raw: &str) -> Result<Self, ChartError> {
        let cleaned = raw.trim_start_matches('\u{FEFF}').trim();
        let root: Value = serde_json::from_str(cleaned)?;
        let song_obj = unwrap_chart_container(&root);

        let title = first_non_empty_str(&[
            song_obj.get("song"),
            song_obj.get("songName"),
            song_obj.get("title"),
            song_obj.get("name"),
            root.get("songName"),
            root.get("song"),
            root.get("title"),
        ])
        .unwrap_or_else(|| "Pista Importada".to_string());

        let base_bpm = first_valid_f64(
            &[
                song_obj.get("bpm"),
                song_obj.get("BPM"),
                root.get("bpm"),
                root.get("BPM"),
                song_obj
                    .get("timeChanges")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|tc| tc.get("bpm")),
                root.get("timeChanges")
                    .and_then(|v| v.as_array())
                    .and_then(|a| a.first())
                    .and_then(|tc| tc.get("bpm")),
            ],
            10.0,
        )
        .unwrap_or(120.0);

        let speed = extract_scroll_speed(song_obj)
            .or_else(|| extract_scroll_speed(&root))
            .unwrap_or(1.8);

        let needs_voices = song_obj
            .get("needsVoices")
            .or_else(|| root.get("needsVoices"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let play_chars = song_obj
            .get("playData")
            .or_else(|| root.get("playData"))
            .and_then(|pd| pd.get("characters"));

        let player1 = first_non_empty_str(&[
            song_obj.get("player1"),
            play_chars.and_then(|c| c.get("player")),
            root.get("player1"),
        ])
        .unwrap_or_else(|| "player_main".to_string());

        let player2 = first_non_empty_str(&[
            song_obj.get("player2"),
            play_chars.and_then(|c| c.get("opponent")),
            root.get("player2"),
        ])
        .unwrap_or_else(|| "opponent_main".to_string());

        let gf_version = first_non_empty_str(&[
            song_obj.get("gfVersion"),
            song_obj.get("player3"),
            play_chars.and_then(|c| c.get("girlfriend")),
            root.get("gfVersion"),
            root.get("player3"),
        ])
        .unwrap_or_else(|| "stage_speakers".to_string());

        let stage = first_non_empty_str(&[
            song_obj.get("stage"),
            song_obj
                .get("playData")
                .or_else(|| root.get("playData"))
                .and_then(|pd| pd.get("stage")),
            root.get("stage"),
        ])
        .unwrap_or_else(|| "main_stage".to_string());

        let arrow_skin = first_non_empty_str(&[
            song_obj.get("arrowSkin"),
            root.get("arrowSkin"),
        ])
        .unwrap_or_default();

        let splash_skin = first_non_empty_str(&[
            song_obj.get("splashSkin"),
            root.get("splashSkin"),
        ])
        .unwrap_or_default();

        let valid_score = song_obj
            .get("validScore")
            .or_else(|| root.get("validScore"))
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let mut events = Vec::new();
        if let Some(raw_events) = song_obj
            .get("events")
            .or_else(|| root.get("events"))
            .and_then(|v| v.as_array())
        {
            parse_chart_events_array(raw_events, &mut events);
        }

        let mut sections = Vec::new();
        let mut notes = Vec::new();
        let mut next_id = 1_u64;
        let mut cursor_ms = 0.0_f64;

        let notes_val = song_obj
            .get("notes")
            .or_else(|| song_obj.get("sections"))
            .or_else(|| root.get("notes"))
            .or_else(|| root.get("sections"));

        let strum_lines_val = song_obj
            .get("strumLines")
            .or_else(|| root.get("strumLines"))
            .and_then(|v| v.as_array());

        if let Some(raw_arr) = notes_val.and_then(|v| v.as_array()) {
            let looks_like_sections = raw_arr.is_empty()
                || raw_arr.iter().any(|el| {
                    el.is_object()
                        && (el.get("sectionNotes").is_some()
                            || el.get("notes").is_some()
                            || el.get("mustHitSection").is_some()
                            || el.get("mustHit").is_some()
                            || el.get("lengthInSteps").is_some()
                            || el.get("sectionBeats").is_some())
                });

            if looks_like_sections {
                let mut current_bpm = base_bpm;
                sections.reserve(raw_arr.len());

                for (sec_idx, sec_val) in raw_arr.iter().enumerate() {
                    let change_bpm = sec_val
                        .get("changeBPM")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if change_bpm {
                        if let Some(new_bpm) =
                            first_valid_f64(&[sec_val.get("bpm"), sec_val.get("BPM")], 10.0)
                        {
                            current_bpm = new_bpm;
                        }
                    }

                    let length_in_steps = sec_val
                        .get("lengthInSteps")
                        .and_then(json_as_f64)
                        .map(|s| s.round().max(4.0) as u32)
                        .or_else(|| {
                            sec_val
                                .get("sectionBeats")
                                .and_then(json_as_f64)
                                .map(|b| (b * 4.0).round().max(4.0) as u32)
                        })
                        .unwrap_or(16)
                        .max(4);

                    let must_hit = sec_val
                        .get("mustHitSection")
                        .or_else(|| sec_val.get("mustHit"))
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
                    let camera_slot = sec_val
                        .get("cameraSlot")
                        .or_else(|| sec_val.get("camera"))
                        .or_else(|| sec_val.get("camSlot"))
                        .and_then(json_as_f64)
                        .filter(|n| n.is_finite() && *n >= 0.0)
                        .map(|n| n.round() as usize);

                    sections.push(ChartSection {
                        index: sec_idx,
                        start_ms: cursor_ms,
                        length_in_steps,
                        must_hit_section: must_hit,
                        bpm: current_bpm,
                        change_bpm,
                        alt_anim,
                        gf_section,
                        camera_slot,
                    });

                    if let Some(sec_notes) = sec_val
                        .get("sectionNotes")
                        .or_else(|| sec_val.get("notes"))
                        .and_then(|v| v.as_array())
                    {
                        for raw_note in sec_notes {
                            parse_section_note_entry(
                                raw_note,
                                must_hit,
                                &mut next_id,
                                &mut notes,
                                &mut events,
                            );
                        }
                    }

                    let step_ms = (60_000.0 / current_bpm) / 4.0;
                    cursor_ms += step_ms * (length_in_steps as f64);
                }
            } else {
                // Lista plana directa de notas (sin secciones envolventes)
                for raw_note in raw_arr {
                    parse_flat_note_entry(raw_note, true, &mut next_id, &mut notes, &mut events);
                }
                cursor_ms = build_synthetic_sections(base_bpm, &notes, &events, &mut sections);
            }
        } else if let Some(notes_map) = notes_val.and_then(|v| v.as_object()) {
            // Formato V-Slice 2.0.0: `"notes": { "hard": [...], "normal": [...], "easy": [...] }`
            if let Some(diff_notes) = pick_vslice_difficulty_array(notes_map) {
                for raw_note in diff_notes {
                    parse_flat_note_entry(raw_note, true, &mut next_id, &mut notes, &mut events);
                }
            }
            if !notes.is_empty() || !events.is_empty() {
                cursor_ms = build_synthetic_sections(base_bpm, &notes, &events, &mut sections);
            }
        } else if let Some(strum_lines) = strum_lines_val {
            // Formato Codename Engine: `"strumLines": [ { "type": 0|1|2, "notes": [...] }, ... ]`
            for line in strum_lines {
                let line_type = line.get("type").and_then(json_as_f64).map(|v| v.round() as i64);
                let pos_str = line
                    .get("position")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let is_player = match line_type {
                    Some(1) => true,
                    Some(0) | Some(2) => false,
                    _ => matches!(pos_str.as_str(), "boyfriend" | "bf" | "player"),
                };

                if let Some(line_notes) = line.get("notes").and_then(|v| v.as_array()) {
                    for raw_note in line_notes {
                        parse_codename_note_entry(raw_note, is_player, &mut next_id, &mut notes);
                    }
                }
            }
            if !notes.is_empty() || !events.is_empty() || !strum_lines.is_empty() {
                cursor_ms = build_synthetic_sections(base_bpm, &notes, &events, &mut sections);
            }
        } else {
            return Err(ChartError::InvalidStructure);
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

                let mut sec_obj = json!({
                    "lengthInSteps": sec.length_in_steps,
                    "sectionBeats": (sec.length_in_steps as f64) / 4.0,
                    "mustHitSection": sec.must_hit_section,
                    "bpm": sec.bpm,
                    "changeBPM": sec.change_bpm,
                    "altAnim": sec.alt_anim,
                    "gfSection": sec.gf_section,
                    "typeOfSection": 0,
                    "sectionNotes": sec_notes
                });
                if let Some(slot) = sec.camera_slot {
                    sec_obj["cameraSlot"] = json!(slot);
                }
                sec_obj
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
            camera_slot: None,
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

    /// Asigna una cámara específica (`Cam 0`, `Cam 1`, `Cam 2`, `Cam 3+`) o vuelve a modo automático (`None`).
    pub fn set_section_camera_slot(&mut self, sec_index: usize, camera_slot: Option<usize>) {
        if let Some(sec) = self.sections.get_mut(sec_index) {
            sec.camera_slot = camera_slot;
            if let Some(slot) = camera_slot {
                if slot == 1 {
                    sec.must_hit_section = true;
                    sec.gf_section = false;
                } else if slot == 2 {
                    sec.must_hit_section = false;
                    sec.gf_section = false;
                }
            }
        }
    }

    /// Coloca o quita una nota manualmente en `(sec_index, step_in_sec, col_0_to_7)` con tipo `"default"`.
    pub fn toggle_note_at(
        &mut self,
        sec_index: usize,
        step_in_sec: u32,
        col_0_to_7: u8,
        sustain_steps: u32,
    ) -> bool {
        self.toggle_note_with_type(sec_index, step_in_sec, col_0_to_7, sustain_steps, "default")
    }

    /// Coloca o quita una nota manualmente indicando `note_type` (`default`, `hurt`, `instakill`, `dodge`, `alt`, `no_anim`).
    pub fn toggle_note_with_type(
        &mut self,
        sec_index: usize,
        step_in_sec: u32,
        col_0_to_7: u8,
        sustain_steps: u32,
        note_type: &str,
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
        let clean_type = {
            let trimmed = note_type.trim();
            if trimmed.is_empty() {
                "default".to_string()
            } else {
                trimmed.to_string()
            }
        };

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
                note_type: clean_type,
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

    /// Devuelve si el foco principal del chart (`mustHitSection`) pertenece al Jugador en `song_pos_ms`.
    pub fn must_hit_at_ms(&self, song_pos_ms: f64) -> bool {
        let mut must_hit = self
            .sections
            .first()
            .map(|s| s.must_hit_section)
            .unwrap_or(false);
        for sec in &self.sections {
            if song_pos_ms >= sec.start_ms {
                must_hit = sec.must_hit_section;
            } else {
                break;
            }
        }
        must_hit
    }

    /// Devuelve `(sec_idx, must_hit_section, gf_section, camera_slot)` en el instante `song_pos_ms`.
    pub fn camera_state_at_ms(&self, song_pos_ms: f64) -> (usize, bool, bool, Option<usize>) {
        let mut active_idx = 0usize;
        let mut must_hit = self
            .sections
            .first()
            .map(|s| s.must_hit_section)
            .unwrap_or(false);
        let mut gf_sec = self
            .sections
            .first()
            .map(|s| s.gf_section)
            .unwrap_or(false);
        let mut cam_slot = self.sections.first().and_then(|s| s.camera_slot);
        for (idx, sec) in self.sections.iter().enumerate() {
            if song_pos_ms >= sec.start_ms {
                active_idx = idx;
                must_hit = sec.must_hit_section;
                gf_sec = sec.gf_section;
                cam_slot = sec.camera_slot;
            } else {
                break;
            }
        }
        (active_idx, must_hit, gf_sec, cam_slot)
    }
}

/// Acción de cámara extraída directamente desde eventos estándar de `chart.json`
/// (Psych Engine `Camera Follow Pos` / `Add Camera Zoom`, V-Slice `FocusCamera` / `ZoomCamera`,
/// Codename Engine `Camera Movement` y cues `Switch Camera`).
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedChartCameraAction {
    /// Cambia a una cámara numerada (`0=Cam0`, `1=Player`, `2=Enemigo`, `3+=Personalizada`) o apunta a GF.
    SelectCamera {
        slot: Option<usize>,
        focus_gf: bool,
        offset_x: f32,
        offset_y: f32,
        zoom: Option<f32>,
        duration_sec: f32,
        ease: String,
    },
    /// Bloquea la cámara en coordenadas fijas `(x, y)` del mundo (`Camera Follow Pos` o `FocusCamera char=-1`).
    FollowPos {
        x: f32,
        y: f32,
        zoom: Option<f32>,
        duration_sec: f32,
        ease: String,
    },
    /// Libera el bloqueo de posición fija (`Camera Follow Pos` con valores vacíos) y devuelve el control a la cámara activa.
    ClearFollowPos,
    /// Cambia el campo de visión (zoom) de cámara (`ZoomCamera`).
    SetZoom {
        zoom: f32,
        is_direct_stage: bool,
        duration_sec: f32,
        ease: String,
    },
    /// Añade un golpe/bop instantáneo al zoom de cámara y HUD (`Add Camera Zoom`).
    AddZoomBump {
        cam_zoom_add: f32,
        hud_zoom_add: f32,
    },
}

fn parse_inline_kv_pairs(raw: &str) -> Vec<(String, String)> {
    raw.split(';')
        .filter_map(|part| {
            let t = part.trim();
            if t.is_empty() {
                return None;
            }
            t.split_once('=')
                .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        })
        .collect()
}

/// Interpreta un evento del `chart.json` relacionado con movimiento, enfoque o zoom de cámara.
pub fn parse_chart_camera_event(
    name: &str,
    value1: &str,
    value2: &str,
    active_bpm: f64,
) -> Option<ParsedChartCameraAction> {
    let lower = name.trim().to_ascii_lowercase();
    let v1 = value1.trim();
    let v2 = value2.trim();
    let safe_bpm = active_bpm.max(40.0);
    let step_sec = ((60.0 / safe_bpm) / 4.0) as f32;

    match lower.as_str() {
        "switch camera" | "camera slot" | "set camera" | "cambiar camara" | "cambiar cámara" => {
            if let Ok(slot_num) = v1.parse::<i32>() {
                let zoom = v2.parse::<f32>().ok().filter(|z| z.is_finite() && *z > 0.1);
                return Some(ParsedChartCameraAction::SelectCamera {
                    slot: Some(slot_num.max(0) as usize),
                    focus_gf: false,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                });
            }
            let pairs = parse_inline_kv_pairs(v1);
            let mut slot: Option<usize> = None;
            let mut zoom: Option<f32> = None;
            let mut dur: f32 = 0.0;
            let mut ease = "sineOut".to_string();
            let mut clear_lock = false;
            let mut fx: Option<f32> = None;
            let mut fy: Option<f32> = None;
            for (k, v) in pairs {
                match k.as_str() {
                    "cam" | "slot" | "camera" => {
                        if let Ok(n) = v.parse::<i32>() {
                            slot = Some(n.max(0) as usize);
                        }
                    }
                    "zoom" | "fov" => {
                        zoom = v.parse::<f32>().ok().filter(|z| z.is_finite() && *z > 0.1);
                    }
                    "dur" | "duration" => {
                        dur = v.parse::<f32>().unwrap_or(0.0).max(0.0);
                    }
                    "ease" => {
                        if !v.is_empty() {
                            ease = v;
                        }
                    }
                    "x" => fx = v.parse::<f32>().ok().filter(|n| n.is_finite()),
                    "y" => fy = v.parse::<f32>().ok().filter(|n| n.is_finite()),
                    "clear" | "unlock" => {
                        clear_lock = matches!(v.as_str(), "1" | "true" | "yes" | "si");
                    }
                    _ => {}
                }
            }
            if !v2.is_empty() {
                for (k, v) in parse_inline_kv_pairs(v2) {
                    match k.as_str() {
                        "zoom" | "fov" => {
                            zoom = v.parse::<f32>().ok().filter(|z| z.is_finite() && *z > 0.1);
                        }
                        "dur" | "duration" => dur = v.parse::<f32>().unwrap_or(dur).max(0.0),
                        "ease" if !v.is_empty() => ease = v,
                        _ => {}
                    }
                }
                if zoom.is_none() && !v2.contains('=') {
                    zoom = v2.parse::<f32>().ok().filter(|z| z.is_finite() && *z > 0.1);
                }
            }
            if clear_lock && slot.is_none() && fx.is_none() && fy.is_none() {
                return Some(ParsedChartCameraAction::ClearFollowPos);
            }
            if let (Some(x), Some(y)) = (fx, fy) {
                if slot.is_none() {
                    return Some(ParsedChartCameraAction::FollowPos {
                        x,
                        y,
                        zoom,
                        duration_sec: dur,
                        ease,
                    });
                }
            }
            Some(ParsedChartCameraAction::SelectCamera {
                slot: Some(slot.unwrap_or(0)),
                focus_gf: false,
                offset_x: fx.unwrap_or(0.0),
                offset_y: fy.unwrap_or(0.0),
                zoom,
                duration_sec: dur,
                ease,
            })
        }
        "camera follow pos" | "camerafollowpos" => {
            if v1.is_empty() && v2.is_empty() {
                return Some(ParsedChartCameraAction::ClearFollowPos);
            }
            let x = v1.parse::<f32>().ok().filter(|n| n.is_finite())?;
            let y = v2.parse::<f32>().ok().filter(|n| n.is_finite())?;
            Some(ParsedChartCameraAction::FollowPos {
                x,
                y,
                zoom: None,
                duration_sec: 0.0,
                ease: "sineOut".to_string(),
            })
        }
        "add camera zoom" | "addcamera zoom" | "add cam zoom" => {
            let cam_add = if v1.is_empty() {
                0.015
            } else {
                v1.parse::<f32>().unwrap_or(0.015)
            };
            let hud_add = if v2.is_empty() {
                0.03
            } else {
                v2.parse::<f32>().unwrap_or(0.03)
            };
            Some(ParsedChartCameraAction::AddZoomBump {
                cam_zoom_add: if cam_add.is_finite() { cam_add } else { 0.0 },
                hud_zoom_add: if hud_add.is_finite() { hud_add } else { 0.0 },
            })
        }
        "focuscamera" | "focus camera" => {
            if let Ok(obj) = serde_json::from_str::<Value>(v1) {
                let explicit_cam = obj
                    .get("cam")
                    .or_else(|| obj.get("slot"))
                    .and_then(json_as_f64)
                    .filter(|n| n.is_finite() && *n >= 0.0)
                    .map(|n| n.round() as usize);
                let char_id = obj.get("char").and_then(json_as_f64).map(|n| n.round() as i32);
                let ox = obj.get("x").and_then(json_as_f64).unwrap_or(0.0) as f32;
                let oy = obj.get("y").and_then(json_as_f64).unwrap_or(0.0) as f32;
                let dur_steps = obj.get("duration").and_then(json_as_f64).unwrap_or(0.0).max(0.0) as f32;
                let ease = obj
                    .get("ease")
                    .and_then(|v| v.as_str())
                    .unwrap_or("sineOut")
                    .to_string();
                let zoom = obj
                    .get("zoom")
                    .and_then(json_as_f64)
                    .map(|z| z as f32)
                    .filter(|z| z.is_finite() && *z > 0.1);
                let duration_sec = if dur_steps > 0.0 { dur_steps * step_sec } else { 0.0 };

                if let Some(slot) = explicit_cam {
                    return Some(ParsedChartCameraAction::SelectCamera {
                        slot: Some(slot),
                        focus_gf: false,
                        offset_x: ox,
                        offset_y: oy,
                        zoom,
                        duration_sec,
                        ease,
                    });
                }

                return match char_id {
                    // V-Slice: char 0 = BF/Player -> Cam 1; 1 = Dad/Opponent -> Cam 2; 2 = GF; -1 = coordenadas stage
                    Some(0) => Some(ParsedChartCameraAction::SelectCamera {
                        slot: Some(1),
                        focus_gf: false,
                        offset_x: ox,
                        offset_y: oy,
                        zoom,
                        duration_sec,
                        ease,
                    }),
                    Some(1) => Some(ParsedChartCameraAction::SelectCamera {
                        slot: Some(2),
                        focus_gf: false,
                        offset_x: ox,
                        offset_y: oy,
                        zoom,
                        duration_sec,
                        ease,
                    }),
                    Some(2) => Some(ParsedChartCameraAction::SelectCamera {
                        slot: None,
                        focus_gf: true,
                        offset_x: ox,
                        offset_y: oy,
                        zoom,
                        duration_sec,
                        ease,
                    }),
                    Some(-1) => {
                        if ox.abs() < 0.01 && oy.abs() < 0.01 && obj.get("x").is_none() {
                            Some(ParsedChartCameraAction::SelectCamera {
                                slot: Some(0),
                                focus_gf: false,
                                offset_x: 0.0,
                                offset_y: 0.0,
                                zoom,
                                duration_sec,
                                ease,
                            })
                        } else {
                            Some(ParsedChartCameraAction::FollowPos {
                                x: ox,
                                y: oy,
                                zoom,
                                duration_sec,
                                ease,
                            })
                        }
                    }
                    _ => Some(ParsedChartCameraAction::FollowPos {
                        x: ox,
                        y: oy,
                        zoom,
                        duration_sec,
                        ease,
                    }),
                };
            }
            // Formato escalar directo: v1 = "0"|"1"|"2"|"bf"|"dad"|"gf"
            match v1.to_ascii_lowercase().as_str() {
                "0" | "bf" | "boyfriend" | "player" | "jugador" => Some(ParsedChartCameraAction::SelectCamera {
                    slot: Some(1),
                    focus_gf: false,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom: None,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                }),
                "1" | "dad" | "opponent" | "enemigo" | "rival" => Some(ParsedChartCameraAction::SelectCamera {
                    slot: Some(2),
                    focus_gf: false,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom: None,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                }),
                "2" | "gf" | "girlfriend" => Some(ParsedChartCameraAction::SelectCamera {
                    slot: None,
                    focus_gf: true,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom: None,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                }),
                "-1" | "cam0" | "stage" => Some(ParsedChartCameraAction::SelectCamera {
                    slot: Some(0),
                    focus_gf: false,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom: None,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                }),
                _ => None,
            }
        }
        "zoomcamera" | "zoom camera" => {
            if let Ok(obj) = serde_json::from_str::<Value>(v1) {
                let zoom = obj.get("zoom").and_then(json_as_f64)? as f32;
                let dur_steps = obj.get("duration").and_then(json_as_f64).unwrap_or(4.0).max(0.0) as f32;
                let mode = obj.get("mode").and_then(|v| v.as_str()).unwrap_or("direct");
                let ease = obj
                    .get("ease")
                    .and_then(|v| v.as_str())
                    .unwrap_or("sineOut")
                    .to_string();
                if !zoom.is_finite() {
                    return None;
                }
                return Some(ParsedChartCameraAction::SetZoom {
                    zoom,
                    is_direct_stage: mode.eq_ignore_ascii_case("stage"),
                    duration_sec: dur_steps * step_sec,
                    ease,
                });
            }
            let zoom = v1.parse::<f32>().ok().filter(|z| z.is_finite() && *z > 0.1)?;
            let dur = v2.parse::<f32>().ok().filter(|d| d.is_finite() && *d >= 0.0).unwrap_or(0.0);
            Some(ParsedChartCameraAction::SetZoom {
                zoom,
                is_direct_stage: false,
                duration_sec: dur,
                ease: "sineOut".to_string(),
            })
        }
        "camera movement" | "cameramovement" => {
            // Codename Engine: 0 = Opponent (Cam 2), 1 = Player (Cam 1), 2 = GF
            let target = v1.to_ascii_lowercase();
            if target == "1" || target == "boyfriend" || target == "bf" || target == "player" {
                Some(ParsedChartCameraAction::SelectCamera {
                    slot: Some(1),
                    focus_gf: false,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom: None,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                })
            } else if target == "2" || target == "gf" || target == "girlfriend" {
                Some(ParsedChartCameraAction::SelectCamera {
                    slot: None,
                    focus_gf: true,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom: None,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                })
            } else {
                Some(ParsedChartCameraAction::SelectCamera {
                    slot: Some(2),
                    focus_gf: false,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom: None,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                })
            }
        }
        _ => None,
    }
}

fn unwrap_chart_container(root: &Value) -> &Value {
    let mut curr = root;
    for _ in 0..4 {
        if curr.get("notes").is_some()
            || curr.get("sections").is_some()
            || curr.get("strumLines").is_some()
        {
            break;
        }
        if let Some(next) = curr
            .get("song")
            .filter(|v| v.is_object())
            .or_else(|| curr.get("chart").filter(|v| v.is_object()))
            .or_else(|| curr.get("data").filter(|v| v.is_object()))
        {
            curr = next;
        } else {
            break;
        }
    }
    curr
}

fn json_as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn first_valid_f64(candidates: &[Option<&Value>], min_exclusive: f64) -> Option<f64> {
    for c in candidates.iter().flatten() {
        if let Some(val) = json_as_f64(c) {
            if val.is_finite() && val > min_exclusive {
                return Some(val);
            }
        }
    }
    None
}

fn first_non_empty_str(candidates: &[Option<&Value>]) -> Option<String> {
    for c in candidates.iter().flatten() {
        if let Some(s) = c.as_str() {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

fn extract_scroll_speed(obj: &Value) -> Option<f64> {
    for key in ["speed", "scrollSpeed"] {
        if let Some(val) = obj.get(key) {
            if let Some(num) = json_as_f64(val) {
                if num.is_finite() && num > 0.1 {
                    return Some(num);
                }
            }
            if let Some(map) = val.as_object() {
                for diff in ["hard", "normal", "easy", "erect", "nightmare"] {
                    if let Some(num) = map.get(diff).and_then(json_as_f64) {
                        if num.is_finite() && num > 0.1 {
                            return Some(num);
                        }
                    }
                }
                for any_val in map.values() {
                    if let Some(num) = json_as_f64(any_val) {
                        if num.is_finite() && num > 0.1 {
                            return Some(num);
                        }
                    }
                }
            }
        }
    }
    None
}

fn pick_vslice_difficulty_array<'a>(
    notes_map: &'a serde_json::Map<String, Value>,
) -> Option<&'a Vec<Value>> {
    for diff in ["hard", "normal", "easy", "erect", "nightmare"] {
        if let Some(arr) = notes_map.get(diff).and_then(|v| v.as_array()) {
            if !arr.is_empty() {
                return Some(arr);
            }
        }
    }
    for val in notes_map.values() {
        if let Some(arr) = val.as_array() {
            if !arr.is_empty() {
                return Some(arr);
            }
        }
    }
    for val in notes_map.values() {
        if let Some(arr) = val.as_array() {
            return Some(arr);
        }
    }
    None
}

fn parse_chart_events_array(raw_events: &[Value], events: &mut Vec<ChartEvent>) {
    for ev_item in raw_events {
        if let Some(arr) = ev_item.as_array() {
            let time_ms = arr.first().and_then(json_as_f64).unwrap_or(0.0).max(0.0);
            if let Some(inner_list) = arr.get(1).and_then(|v| v.as_array()) {
                for entry in inner_list {
                    if let Some(e_arr) = entry.as_array() {
                        let name = e_arr
                            .first()
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        if name.is_empty() {
                            continue;
                        }
                        events.push(ChartEvent {
                            time_ms,
                            name,
                            value1: value_to_compact_string(e_arr.get(1)),
                            value2: value_to_compact_string(e_arr.get(2)),
                        });
                    }
                }
            } else if let Some(name) = arr.get(1).and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    events.push(ChartEvent {
                        time_ms,
                        name: name.to_string(),
                        value1: value_to_compact_string(arr.get(2)),
                        value2: value_to_compact_string(arr.get(3)),
                    });
                }
            }
        } else if let Some(obj) = ev_item.as_object() {
            // Formato V-Slice `{ "t": ms, "e": name, "v": val }` o Codename `{ "time": ms, "name": name, "params": [...] }`
            let time_ms = obj
                .get("t")
                .or_else(|| obj.get("time"))
                .and_then(json_as_f64)
                .unwrap_or(0.0)
                .max(0.0);
            let name = obj
                .get("e")
                .or_else(|| obj.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            if name.is_empty() {
                continue;
            }
            let (value1, value2) = if let Some(params) = obj.get("params").and_then(|v| v.as_array())
            {
                (
                    value_to_compact_string(params.first()),
                    value_to_compact_string(params.get(1)),
                )
            } else if let Some(v_obj) = obj.get("v").and_then(|v| v.as_object()) {
                (
                    serde_json::to_string(v_obj).unwrap_or_default(),
                    String::new(),
                )
            } else {
                (
                    value_to_compact_string(obj.get("v").or_else(|| obj.get("value1"))),
                    value_to_compact_string(obj.get("value2")),
                )
            };
            events.push(ChartEvent {
                time_ms,
                name,
                value1,
                value2,
            });
        }
    }
}

fn parse_section_note_entry(
    raw_note: &Value,
    must_hit: bool,
    next_id: &mut u64,
    notes: &mut Vec<ChartNote>,
    events: &mut Vec<ChartEvent>,
) {
    if let Some(arr) = raw_note.as_array() {
        if arr.len() < 2 {
            return;
        }
        let time_ms = json_as_f64(&arr[0]).unwrap_or(0.0).max(0.0);
        let raw_lane = json_as_f64(&arr[1]).map(|v| v.round() as i64).unwrap_or(0);
        // En algunos charts, `lane == -1` codifica un evento en línea `[time, -1, name, v1, v2]`
        if raw_lane == -1 {
            let ev_name = arr
                .get(2)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !ev_name.is_empty() {
                events.push(ChartEvent {
                    time_ms,
                    name: ev_name,
                    value1: value_to_compact_string(arr.get(3)),
                    value2: value_to_compact_string(arr.get(4)),
                });
            }
            return;
        }
        if raw_lane < 0 {
            return;
        }
        let lane = (raw_lane as u8) % 8;
        let sustain_ms = arr.get(2).and_then(json_as_f64).unwrap_or(0.0).max(0.0);
        let note_type = parse_note_type_value(arr.get(3));

        let is_first_group = lane < 4;
        let is_player = if must_hit {
            is_first_group
        } else {
            !is_first_group
        };

        notes.push(ChartNote {
            id: *next_id,
            time_ms,
            direction: NoteDirection::from_lane(lane),
            is_player,
            sustain_ms,
            note_type,
            hit: false,
            missed: false,
        });
        *next_id += 1;
    } else if raw_note.is_object() {
        parse_flat_note_entry(raw_note, must_hit, next_id, notes, events);
    }
}

fn parse_flat_note_entry(
    raw_note: &Value,
    player_first_group: bool,
    next_id: &mut u64,
    notes: &mut Vec<ChartNote>,
    events: &mut Vec<ChartEvent>,
) {
    if let Some(arr) = raw_note.as_array() {
        parse_section_note_entry(raw_note, player_first_group, next_id, notes, events);
        let _ = arr;
        return;
    }
    let Some(obj) = raw_note.as_object() else {
        return;
    };
    let time_ms = obj
        .get("t")
        .or_else(|| obj.get("time"))
        .or_else(|| obj.get("strumTime"))
        .and_then(json_as_f64)
        .unwrap_or(0.0)
        .max(0.0);
    let raw_lane = obj
        .get("d")
        .or_else(|| obj.get("lane"))
        .or_else(|| obj.get("noteData"))
        .or_else(|| obj.get("id"))
        .and_then(json_as_f64)
        .map(|v| v.round() as i64)
        .unwrap_or(-1);
    if raw_lane < 0 {
        return;
    }
    let lane = (raw_lane as u8) % 8;
    let sustain_ms = obj
        .get("l")
        .or_else(|| obj.get("sustain"))
        .or_else(|| obj.get("sustainLength"))
        .or_else(|| obj.get("sLen"))
        .and_then(json_as_f64)
        .unwrap_or(0.0)
        .max(0.0);
    let note_type = parse_note_type_value(
        obj.get("k")
            .or_else(|| obj.get("noteType"))
            .or_else(|| obj.get("type")),
    );
    let explicit_player = obj
        .get("mustPress")
        .or_else(|| obj.get("isPlayer"))
        .and_then(|v| v.as_bool());
    let is_player = explicit_player.unwrap_or_else(|| {
        let is_first_group = lane < 4;
        if player_first_group {
            is_first_group
        } else {
            !is_first_group
        }
    });

    notes.push(ChartNote {
        id: *next_id,
        time_ms,
        direction: NoteDirection::from_lane(lane),
        is_player,
        sustain_ms,
        note_type,
        hit: false,
        missed: false,
    });
    *next_id += 1;
}

fn parse_codename_note_entry(
    raw_note: &Value,
    is_player: bool,
    next_id: &mut u64,
    notes: &mut Vec<ChartNote>,
) {
    if let Some(arr) = raw_note.as_array() {
        if arr.len() < 2 {
            return;
        }
        let time_ms = json_as_f64(&arr[0]).unwrap_or(0.0).max(0.0);
        let raw_lane = json_as_f64(&arr[1]).map(|v| v.round() as i64).unwrap_or(-1);
        if raw_lane < 0 {
            return;
        }
        let sustain_ms = arr.get(2).and_then(json_as_f64).unwrap_or(0.0).max(0.0);
        let note_type = parse_note_type_value(arr.get(3));
        notes.push(ChartNote {
            id: *next_id,
            time_ms,
            direction: NoteDirection::from_lane((raw_lane as u8) % 4),
            is_player,
            sustain_ms,
            note_type,
            hit: false,
            missed: false,
        });
        *next_id += 1;
        return;
    }

    let Some(obj) = raw_note.as_object() else {
        return;
    };
    let time_ms = obj
        .get("time")
        .or_else(|| obj.get("t"))
        .and_then(json_as_f64)
        .unwrap_or(0.0)
        .max(0.0);
    let raw_lane = obj
        .get("id")
        .or_else(|| obj.get("d"))
        .or_else(|| obj.get("lane"))
        .and_then(json_as_f64)
        .map(|v| v.round() as i64)
        .unwrap_or(-1);
    if raw_lane < 0 {
        return;
    }
    let sustain_ms = obj
        .get("sLen")
        .or_else(|| obj.get("l"))
        .or_else(|| obj.get("sustain"))
        .and_then(json_as_f64)
        .unwrap_or(0.0)
        .max(0.0);
    let note_type = parse_note_type_value(obj.get("type").or_else(|| obj.get("k")));

    notes.push(ChartNote {
        id: *next_id,
        time_ms,
        direction: NoteDirection::from_lane((raw_lane as u8) % 4),
        is_player,
        sustain_ms,
        note_type,
        hit: false,
        missed: false,
    });
    *next_id += 1;
}

fn parse_note_type_value(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) if !s.trim().is_empty() => s.clone(),
        Some(Value::Bool(true)) => "Alt Animation".to_string(),
        Some(Value::Number(n)) => format!("type_{}", n),
        _ => "default".to_string(),
    }
}

fn build_synthetic_sections(
    base_bpm: f64,
    notes: &[ChartNote],
    events: &[ChartEvent],
    sections: &mut Vec<ChartSection>,
) -> f64 {
    let safe_bpm = base_bpm.max(40.0);
    let step_ms = (60_000.0 / safe_bpm) / 4.0;
    let sec_ms = step_ms * 16.0;
    let mut max_ms = sec_ms * 4.0;
    for n in notes {
        max_ms = max_ms.max(n.time_ms + n.sustain_ms);
    }
    for ev in events {
        max_ms = max_ms.max(ev.time_ms);
    }
    let total_secs = ((max_ms / sec_ms).ceil() as usize).clamp(4, 2000);
    for idx in 0..total_secs {
        let start_ms = (idx as f64) * sec_ms;
        let end_ms = start_ms + sec_ms;
        let mut player_count = 0usize;
        let mut opp_count = 0usize;
        for n in notes {
            if n.time_ms >= start_ms - 1.0 && n.time_ms < end_ms - 1.0 {
                if n.is_player {
                    player_count += 1;
                } else {
                    opp_count += 1;
                }
            }
        }
        let must_hit = if player_count == 0 && opp_count == 0 {
            true
        } else {
            player_count >= opp_count
        };
        sections.push(ChartSection {
            index: idx,
            start_ms,
            length_in_steps: 16,
            must_hit_section: must_hit,
            bpm: safe_bpm,
            change_bpm: false,
            alt_anim: false,
            gf_section: false,
            camera_slot: None,
        });
    }
    (total_secs as f64) * sec_ms
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

    #[test]
    fn parses_vslice_difficulty_map_and_double_wrapped_and_bom_charts() {
        let vslice = "\u{FEFF}{\n  \"version\": \"2.0.0\",\n  \"songName\": \"V-Slice Track\",\n  \"bpm\": 150,\n  \"scrollSpeed\": { \"hard\": 2.8, \"normal\": 2.2 },\n  \"events\": [{ \"t\": 1200, \"e\": \"FocusCamera\", \"v\": { \"char\": 1 } }],\n  \"notes\": {\n    \"hard\": [\n      { \"t\": 500, \"d\": 0, \"l\": 0, \"k\": \"\" },\n      { \"t\": 1000, \"d\": 5, \"l\": 200, \"k\": \"alt\" }\n    ]\n  }\n}";
        let parsed = ChartData::from_json_str(vslice).expect("V-Slice válido");
        assert_eq!(parsed.title, "V-Slice Track");
        assert_eq!(parsed.bpm, 150.0);
        assert_eq!(parsed.speed, 2.8);
        assert_eq!(parsed.notes.len(), 2);
        assert!(parsed.notes[0].is_player);
        assert_eq!(parsed.notes[0].direction, NoteDirection::Left);
        assert!(!parsed.notes[1].is_player);
        assert_eq!(parsed.notes[1].direction, NoteDirection::Down);
        assert_eq!(parsed.notes[1].sustain_ms, 200.0);
        assert_eq!(parsed.events.len(), 1);
        assert!(parsed.sections.len() >= 4);

        let double_wrapped = r#"{
          "song": {
            "song": {
              "song": "Double Wrapped",
              "bpm": "140",
              "notes": [
                { "mustHit": true, "sectionNotes": [[400, 2, 0]] }
              ]
            }
          }
        }"#;
        let parsed_dw = ChartData::from_json_str(double_wrapped).expect("Double wrapped válido");
        assert_eq!(parsed_dw.title, "Double Wrapped");
        assert_eq!(parsed_dw.bpm, 140.0);
        assert_eq!(parsed_dw.notes.len(), 1);
        assert!(parsed_dw.notes[0].is_player);
    }

    #[test]
    fn parses_codename_strumlines_chart() {
        let codename = r#"{
          "codenameChart": true,
          "bpm": 132,
          "scrollSpeed": 2.4,
          "events": [{ "name": "Camera Movement", "time": 800, "params": [1] }],
          "strumLines": [
            { "type": 0, "position": "dad", "notes": [{ "time": 500, "id": 1, "sLen": 0 }] },
            { "type": 1, "position": "boyfriend", "notes": [{ "time": 1000, "id": 3, "sLen": 150 }] }
          ]
        }"#;
        let parsed = ChartData::from_json_str(codename).expect("Codename válido");
        assert_eq!(parsed.bpm, 132.0);
        assert_eq!(parsed.speed, 2.4);
        assert_eq!(parsed.notes.len(), 2);
        assert!(!parsed.notes[0].is_player);
        assert_eq!(parsed.notes[0].direction, NoteDirection::Down);
        assert!(parsed.notes[1].is_player);
        assert_eq!(parsed.notes[1].direction, NoteDirection::Right);
        assert_eq!(parsed.notes[1].sustain_ms, 150.0);
        assert_eq!(parsed.events.len(), 1);
    }

    #[test]
    fn chart_section_camera_slot_and_camera_events_parse_cleanly() {
        let mut chart = ChartData::bundled_demo_chart();
        chart.set_section_camera_slot(0, Some(3));
        assert_eq!(chart.camera_state_at_ms(10.0), (0, false, false, Some(3)));

        let exported = chart.to_json_str();
        let reimported = ChartData::from_json_str(&exported).expect("chart con cameraSlot");
        assert_eq!(reimported.sections[0].camera_slot, Some(3));

        assert_eq!(
            parse_chart_camera_event("Switch Camera", "cam=3;zoom=1.40;dur=0.5;ease=quadOut", "", 120.0),
            Some(ParsedChartCameraAction::SelectCamera {
                slot: Some(3),
                focus_gf: false,
                offset_x: 0.0,
                offset_y: 0.0,
                zoom: Some(1.4),
                duration_sec: 0.5,
                ease: "quadOut".to_string(),
            })
        );
        assert_eq!(
            parse_chart_camera_event("Camera Follow Pos", "710", "320", 120.0),
            Some(ParsedChartCameraAction::FollowPos {
                x: 710.0,
                y: 320.0,
                zoom: None,
                duration_sec: 0.0,
                ease: "sineOut".to_string(),
            })
        );
        assert_eq!(
            parse_chart_camera_event("Camera Follow Pos", "", "", 120.0),
            Some(ParsedChartCameraAction::ClearFollowPos)
        );
        assert_eq!(
            parse_chart_camera_event("FocusCamera", r#"{"char":1,"x":20,"y":-10,"duration":4}"#, "", 120.0),
            Some(ParsedChartCameraAction::SelectCamera {
                slot: Some(2),
                focus_gf: false,
                offset_x: 20.0,
                offset_y: -10.0,
                zoom: None,
                duration_sec: 0.5,
                ease: "sineOut".to_string(),
            })
        );
    }
}
