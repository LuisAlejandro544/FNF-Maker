//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/script_sync/mod.rs
//! Propósito: Coordinador modular de la sincronización bidireccional entre el estado de
//! gameplay en Rust y las máquinas virtuales Lua/QuickJS:
//! - `props`: Lectura/escritura de propiedades (`StageEntity`, `DynamicDrawableState`, `StrumOverrideState`) y snapshot `sync_script_host_state`.
//! - `tweens_timers`: Evaluación de curvas de interpolación (`eval_ease`), temporizadores y tweens activos.
//! - `camera_commands`: Ejecución de comandos de cámara multicámara y viñeta lateral emitidos por scripts.
//! - `command_handlers`: Drenaje de la cola `ScriptCommand` (HUD, tweens, swaps RAM, audio y Game Over).

mod camera_commands;
mod command_handlers;
mod props;
mod tweens_timers;

pub(super) use command_handlers::drain_and_apply_script_commands;
pub(super) use props::sync_script_host_state;
pub(super) use tweens_timers::{update_active_timers, update_active_tweens};
