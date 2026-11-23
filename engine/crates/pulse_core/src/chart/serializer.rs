//! Archivo: engine/crates/pulse_core/src/chart/serializer.rs
//! Propósito: Exportador completo de `ChartData` hacia formato estándar `chart.json`
//! agrupando notas por sección (`mustHitSection`, `cameraSlot`) y eventos por milisegundo.

use super::models::{ChartData, ChartNote};
use serde_json::{Value, json};

impl ChartData {
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
            if let Some(last_arr) = grouped_events.last_mut().and_then(|v| v.as_array_mut()) {
                if last_arr.first().and_then(|v| v.as_f64()) == Some(t) {
                    if let Some(arr) = last_arr.get_mut(1).and_then(|v| v.as_array_mut()) {
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
                "uiBlackout": self.intro_ui,
                "events": grouped_events,
                "notes": json_sections
            }
        });

        serde_json::to_string_pretty(&root).unwrap_or_else(|_| "{}".to_string())
    }
}

fn round_ms(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
