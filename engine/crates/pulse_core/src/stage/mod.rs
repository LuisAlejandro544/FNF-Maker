//! Archivo: engine/crates/pulse_core/src/stage/mod.rs
//! Propósito: Punto de entrada modular del escenario 2D (`StageScene`), sus entidades visuales
//! (`StageEntity`, `EntityRole`) y el sistema multicámara con viñeta (`StageCameraConfig`).

mod camera;
mod models;
mod scene;

#[cfg(test)]
mod tests;

pub use camera::{
    CameraFollowMode, CameraTargetKind, StageCameraConfig, StageCameraSlot, default_camera_slots,
    parse_vignette_color_argb,
};
pub use models::{EntityRole, StageEntity};
pub use scene::StageScene;
