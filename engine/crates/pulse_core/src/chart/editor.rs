//! Archivo: engine/crates/pulse_core/src/chart/editor.rs
//! Propósito: Operaciones de edición y consulta cronológica sobre `ChartData`
//! (recalculo de estadísticas, BPM por canción/sección, metadatos, secciones,
//! notas especiales, eventos y consulta de cámara por milisegundo).

use super::models::{ChartData, ChartEvent, ChartIntroUiConfig, ChartNote, ChartSection, NoteDirection};

impl ChartData {
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
    #[allow(clippy::too_many_arguments)]
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

    /// Actualiza la configuración declarativa de pantalla en negro / HUD oculto / flechas ocultas al iniciar.
    pub fn set_intro_ui(&mut self, cfg: ChartIntroUiConfig) {
        self.intro_ui = cfg.normalized();
    }

    /// Devuelve la duración total del timeline incluyendo secciones vacías al final del chart.
    pub fn total_timeline_duration_ms(&self) -> f64 {
        let sections_end_ms = self
            .sections
            .last()
            .map(|sec| {
                let step_ms = (60_000.0 / sec.bpm.max(40.0)) / 4.0;
                sec.start_ms + (sec.length_in_steps.max(4) as f64) * step_ms
            })
            .unwrap_or(0.0);
        sections_end_ms.max(self.stats.duration_ms).max(0.0)
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
