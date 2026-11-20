//! Archivo: engine/crates/pulse_core/src/chart/parser.rs
//! Propósito: Parser universal multi-formato de `chart.json` para `ChartData`.
//! Soporta formato clásico (`song.notes`), doble envoltura, V-Slice 2.0.0 (`notes.{diff}`),
//! Codename Engine (`strumLines`), listas planas de notas, eventos `-1` inline y `uiBlackout`.

use super::models::{
    ChartData, ChartError, ChartEvent, ChartIntroUiConfig, ChartNote, ChartSection, NoteDirection,
};
use serde_json::Value;

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

        let intro_ui = parse_chart_intro_ui(song_obj, &root);
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
            intro_ui,
            sections,
            notes,
            events,
            stats,
        })
    }
}

pub(super) fn json_as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
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

fn json_as_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => n.as_f64().map(|x| x > 0.0),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "si" | "sí" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn first_bool_opt(candidates: &[Option<&Value>]) -> Option<bool> {
    for c in candidates.iter().flatten() {
        if let Some(b) = json_as_bool(c) {
            return Some(b);
        }
    }
    None
}

fn parse_chart_intro_ui(song_obj: &Value, root: &Value) -> ChartIntroUiConfig {
    let mut cfg = ChartIntroUiConfig::default();
    let block = song_obj
        .get("uiBlackout")
        .or_else(|| song_obj.get("introUi"))
        .or_else(|| song_obj.get("intro_ui"))
        .or_else(|| root.get("uiBlackout"))
        .or_else(|| root.get("introUi"))
        .or_else(|| root.get("intro_ui"));

    let b_get = |key: &str| block.and_then(|v| v.get(key));

    let explicit_alpha = first_valid_f64(
        &[
            b_get("blackoutAlpha"),
            b_get("blackout_alpha"),
            b_get("alpha"),
            song_obj.get("blackoutAlpha"),
            root.get("blackoutAlpha"),
        ],
        -0.001,
    );
    if let Some(a) = explicit_alpha {
        cfg.blackout_alpha = (a as f32).clamp(0.0, 1.0);
    }

    if let Some(b) = first_bool_opt(&[
        b_get("startBlackout"),
        b_get("start_blackout"),
        b_get("blackout"),
        b_get("enabled"),
        song_obj.get("startBlackout"),
        song_obj.get("blackoutOnStart"),
        root.get("startBlackout"),
        root.get("blackoutOnStart"),
    ]) {
        cfg.start_blackout = b;
    } else if explicit_alpha.is_some_and(|a| a > 0.01) {
        cfg.start_blackout = true;
    }

    if let Some(h) = first_bool_opt(&[
        b_get("hideHud"),
        b_get("hide_hud"),
        b_get("hideUi"),
        b_get("hide_ui"),
        b_get("hideHudOnStart"),
        song_obj.get("hideHud"),
        song_obj.get("hideHudOnStart"),
        song_obj.get("hideUi"),
        root.get("hideHud"),
        root.get("hideHudOnStart"),
        root.get("hideUi"),
    ]) {
        cfg.hide_hud = h;
    }

    if let Some(ar) = first_bool_opt(&[
        b_get("hideArrows"),
        b_get("hide_arrows"),
        b_get("hideStrums"),
        b_get("hide_strums"),
        b_get("hideArrowsOnStart"),
        song_obj.get("hideArrows"),
        song_obj.get("hideArrowsOnStart"),
        song_obj.get("hideStrums"),
        root.get("hideArrows"),
        root.get("hideArrowsOnStart"),
        root.get("hideStrums"),
    ]) {
        cfg.hide_arrows = ar;
    }

    if let Some(rev) = first_bool_opt(&[
        b_get("revealOnSongStart"),
        b_get("reveal_on_song_start"),
        b_get("revealOnStart"),
        b_get("onStart"),
        song_obj.get("introRevealOnStart"),
        root.get("introRevealOnStart"),
    ]) {
        cfg.reveal_on_song_start = rev;
    }

    if let Some(delay) = first_valid_f64(
        &[
            b_get("revealDelayMs"),
            b_get("reveal_delay_ms"),
            b_get("delayMs"),
            song_obj.get("introRevealDelayMs"),
            root.get("introRevealDelayMs"),
        ],
        -0.001,
    ) {
        cfg.reveal_delay_ms = delay;
    }

    if let Some(fade) = first_valid_f64(
        &[
            b_get("fadeSec"),
            b_get("fade_sec"),
            b_get("durSec"),
            song_obj.get("introFadeSec"),
            root.get("introFadeSec"),
        ],
        -0.001,
    ) {
        cfg.fade_sec = fade as f32;
    }

    cfg.normalized()
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
