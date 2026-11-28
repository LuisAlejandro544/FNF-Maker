//! Archivo: engine/crates/pulse_android/src/session/editor_ops/mod.rs
//! Propósito: Coordinador modular de las operaciones de edición del proyecto en `EngineSession`:
//! - `chart_ops`: Importación, exportación y edición de `chart.json` (BPM, secciones, eventos, notas).
//! - `audio_ops`: Decodificación, volumen, mute y adaptador acústico de hasta 3 pistas en `pulse_audio`.
//! - `stage_ops`: Gestión de entidades del escenario (`StageEntity`), Z-order e importación `stages/*`.
//! - `camera_ops`: Multicámara (`Cam 0..3+`), viñeta lateral y sincronización hacia `camera_system.lua`.
//! - `script_skin_ops`: Scripts Lua/TS/JS y skins SVG/Sparrow (`NOTE_assets`, barra de vida, íconos, splash).

mod audio_ops;
mod camera_ops;
mod chart_ops;
mod script_skin_ops;
mod stage_ops;

use crate::session::{EngineSession, parse_control_mode_key};
use pulse_core::project::{CURRENT_DEV_API_VERSION, dev_api_label, migrate_project_to_dev_api};

impl EngineSession {
    pub fn update_project_meta(
        &mut self,
        name: String,
        author: String,
        downscroll: bool,
        control_mode_key: &str,
        audio_offset_ms: f64,
        scroll_speed: f64,
    ) -> String {
        self.project.name = name;
        self.project.author = author;
        self.project.downscroll = downscroll;
        self.project.control_mode = parse_control_mode_key(control_mode_key);
        self.project.audio_offset_ms = audio_offset_ms.clamp(-500.0, 500.0);
        self.project.chart.speed = scroll_speed.clamp(0.5, 5.0);
        if let Some(gp) = self.gameplay.as_mut() {
            gp.active_downscroll = downscroll
                || self.project.control_mode == pulse_core::project::MobileControlMode::TapArrows;
            gp.hud_revision = gp.hud_revision.wrapping_add(1);
        }
        self.invalidate_compiled_bundle();
        self.mark_dirty_and_maybe_save(false);
        self.ok_response("Proyecto actualizado")
    }

    pub fn upgrade_dev_api_version(&mut self) -> String {
        let (prev, next) = migrate_project_to_dev_api(&mut self.project, CURRENT_DEV_API_VERSION);
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        if prev == next {
            self.ok_response(format!(
                "El proyecto ya usa la API de desarrollo {}",
                dev_api_label(next)
            ))
        } else {
            self.ok_response(format!(
                "Proyecto migrado de API v{} a {}",
                prev,
                dev_api_label(next)
            ))
        }
    }
}
