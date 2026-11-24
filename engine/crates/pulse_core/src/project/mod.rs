//! Archivo: engine/crates/pulse_core/src/project/mod.rs
//! Propósito: Punto de entrada modular del dominio `project` en `pulse_core`.
//! Reexporta la API pública completa (`ModProject`, `ScriptAsset`, `AudioTrackSlot`,
//! `CountdownAssetsConfig`, `CountdownStepAsset`, generadores/parsers de scripts maestros Lua
//! y constantes) preservando compatibilidad 100%.

mod camera_lua;
pub mod countdown;
mod lua_helpers;
mod models;
mod stage_events_lua;
mod ui_blackout_lua;

pub use camera_lua::{parse_camera_config_from_lua, render_camera_lua_script};
pub use countdown::{
    COUNTDOWN_STEP_COUNT, CountdownAssetsConfig, CountdownStepAsset, countdown_step_label,
    parse_countdown_sparrow_xml,
};
pub use models::{
    AudioTrackSlot, CAMERA_SCRIPT_ID, CAMERA_SCRIPT_NAME, MAX_AUDIO_TRACKS, MobileControlMode,
    ModProject, STAGE_EVENTS_SCRIPT_ID, STAGE_EVENTS_SCRIPT_NAME, ScriptAsset, ScriptLanguage,
    UI_BLACKOUT_SCRIPT_ID, UI_BLACKOUT_SCRIPT_NAME, current_epoch_ms,
};
pub use stage_events_lua::render_stage_events_lua_script;
pub use ui_blackout_lua::render_ui_blackout_lua_script;
