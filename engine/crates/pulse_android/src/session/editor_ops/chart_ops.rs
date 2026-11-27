//! Archivo: engine/crates/pulse_android/src/session/editor_ops/chart_ops.rs
//! Propósito: Operaciones de edición del chart en `EngineSession`: importación/exportación
//! de `chart.json`, metadatos, configuración inicial de apagón/HUD (`intro_ui`), BPM global
//! y por sección, eventos de escenario, ranuras de cámara por sección y notas especiales.

use crate::session::{EngineSession, friendly_import_name};
use pulse_core::ChartData;
use std::path::PathBuf;

impl EngineSession {
    pub fn import_chart(&mut self, path: PathBuf) -> String {
        match std::fs::read_to_string(&path) {
            Ok(raw) => match ChartData::from_json_str(&raw) {
                Ok(chart) => {
                    let msg = format!(
                        "Chart '{}' importado ({} notas, {:.1} BPM)",
                        chart.title, chart.stats.total_notes, chart.bpm
                    );
                    let display_name = friendly_import_name(&path);
                    self.project.chart = chart;
                    self.project.chart_source_path = Some(PathBuf::from(display_name));
                    self.project.sync_stage_events_script();
                    self.project.sync_ui_blackout_script();
                    self.invalidate_compiled_bundle();
                    self.sync_mixer_chart_duration();
                    self.save_active_project_now();
                    self.ok_response(msg)
                }
                Err(e) => self.err_response(format!("Chart inválido: {}", e)),
            },
            Err(e) => self.err_response(format!("No se pudo leer el chart: {}", e)),
        }
    }

    pub fn export_chart_json(&self) -> String {
        self.project.chart.to_json_str()
    }

    pub fn set_chart_bpm(&mut self, bpm: f64) -> String {
        self.project.chart.set_bpm(bpm);
        self.invalidate_compiled_bundle();
        self.sync_mixer_chart_duration();
        self.save_active_project_now();
        self.ok_response(format!("BPM actualizado a {:.1}", self.project.chart.bpm))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_chart_details(
        &mut self,
        title: &str,
        speed: f64,
        player1: &str,
        player2: &str,
        gf_version: &str,
        stage: &str,
        needs_voices: bool,
    ) -> String {
        self.project
            .chart
            .set_metadata(title, speed, player1, player2, gf_version, stage, needs_voices);
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Metadatos completos de chart.json actualizados")
    }

    /// Actualiza la configuración inicial de pantalla negra / ocultar HUD / ocultar flechas
    /// declarada directamente en `chart.json` y sincroniza `ui_blackout.lua`.
    #[allow(clippy::too_many_arguments)]
    pub fn update_chart_intro_ui(
        &mut self,
        start_blackout: bool,
        blackout_alpha: f32,
        hide_hud: bool,
        hide_arrows: bool,
        reveal_on_song_start: bool,
        reveal_delay_ms: f64,
        fade_sec: f32,
    ) -> String {
        self.project.chart.set_intro_ui(pulse_core::ChartIntroUiConfig {
            start_blackout,
            blackout_alpha,
            hide_hud,
            hide_arrows,
            reveal_on_song_start,
            reveal_delay_ms,
            fade_sec,
        });
        self.project.sync_ui_blackout_script();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Configuración inicial de pantalla negra, UI y flechas actualizada")
    }

    pub fn set_section_bpm(&mut self, sec_index: usize, bpm: f64, change_bpm: bool) -> String {
        self.project.chart.set_section_bpm(sec_index, bpm, change_bpm);
        self.invalidate_compiled_bundle();
        self.sync_mixer_chart_duration();
        self.save_active_project_now();
        self.ok_response(format!("BPM de sección {} actualizado", sec_index + 1))
    }

    pub fn add_chart_event(&mut self, time_ms: f64, name: &str, value1: &str, value2: &str) -> String {
        self.project.chart.add_event(time_ms, name, value1, value2);
        self.project.sync_stage_events_script();
        self.project.sync_ui_blackout_script();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Evento de chart.json y scripts de escena/UI actualizados")
    }

    pub fn remove_chart_event(&mut self, index: usize) -> String {
        self.project.chart.remove_event(index);
        self.project.sync_stage_events_script();
        self.project.sync_ui_blackout_script();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Evento eliminado y scripts de escena/UI actualizados")
    }

    pub fn add_chart_section(&mut self, must_hit: bool) -> String {
        let idx = self.project.chart.add_section(must_hit);
        self.invalidate_compiled_bundle();
        self.sync_mixer_chart_duration();
        self.save_active_project_now();
        self.ok_response(format!("Sección {} añadida", idx + 1))
    }

    pub fn toggle_section_must_hit(&mut self, sec_index: usize) -> String {
        self.project.chart.toggle_section_must_hit(sec_index);
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Foco de sección actualizado")
    }

    pub fn set_section_camera_slot(&mut self, sec_index: usize, camera_slot: i32) -> String {
        let slot_opt = if camera_slot < 0 {
            None
        } else {
            Some(camera_slot as usize)
        };
        self.project.chart.set_section_camera_slot(sec_index, slot_opt);
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        let label = match slot_opt {
            None => "Automático (mustHit / gfSection)".to_string(),
            Some(s) => format!("Cam {}", s),
        };
        self.ok_response(format!("Sección {} asignada a {}", sec_index + 1, label))
    }

    pub fn toggle_chart_note(
        &mut self,
        sec_index: usize,
        step_in_sec: u32,
        col_0_to_7: u8,
        sustain_steps: u32,
    ) -> String {
        self.toggle_chart_note_with_type(sec_index, step_in_sec, col_0_to_7, sustain_steps, "default")
    }

    pub fn toggle_chart_note_with_type(
        &mut self,
        sec_index: usize,
        step_in_sec: u32,
        col_0_to_7: u8,
        sustain_steps: u32,
        note_type: &str,
    ) -> String {
        self.project.chart.toggle_note_with_type(
            sec_index,
            step_in_sec,
            col_0_to_7,
            sustain_steps,
            note_type,
        );
        self.invalidate_compiled_bundle();
        self.sync_mixer_chart_duration();
        self.save_active_project_now();
        self.ok_response("Nota actualizada")
    }
}
