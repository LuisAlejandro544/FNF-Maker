//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/script_sync/camera_commands.rs
//! Propósito: Procesamiento de comandos `ScriptCommand` relacionados con la cámara multicámara
//! (`SwitchCameraSlot`, `SetCameraPos`, `SetCameraZoom`, `SetCameraFollowLock`, clamping) y
//! efectos de viñeta lateral (`SetCameraVignette`, `TweenCameraVignette`, `ClearCameraVignette`).

use crate::session::{ActiveTweenState, GameplayRuntimeState, TweenTarget};
use pulse_core::StageScene;
use pulse_scripting::ScriptCommand;

/// Ejecuta los comandos de cámara y viñeta; devuelve `Some(cmd)` si el comando pertenece a otro dominio.
pub(super) fn try_apply_camera_command(
    session: &mut GameplayRuntimeState,
    stage: &mut StageScene,
    cmd: ScriptCommand,
) -> Option<ScriptCommand> {
    match cmd {
        ScriptCommand::SetCameraPos { x, y } => {
            let (cx, cy) = stage.clamp_camera_to_background(x, y, stage.camera_zoom);
            stage.camera_x = cx;
            stage.camera_y = cy;
            None
        }
        ScriptCommand::SetCameraZoom(z) => {
            stage.camera_zoom = z.clamp(0.35, 2.5);
            None
        }
        ScriptCommand::SetCameraBaseZoom(z) => {
            let clamped = z.clamp(0.35, 2.5);
            stage.camera_base_zoom = clamped;
            session.camera_slot_zoom_override = Some(clamped);
            None
        }
        ScriptCommand::ShakeCamera { intensity } => {
            stage.camera_shake = intensity.clamp(0.0, 25.0);
            None
        }
        ScriptCommand::SwitchCameraSlot {
            slot,
            zoom,
            duration_sec,
            ease,
        } => {
            let max_slot = stage.camera.cameras.len().saturating_sub(1);
            let clamped_slot = slot.min(max_slot);
            session.active_camera_slot = clamped_slot;
            session.camera_follow_lock = None;
            session.camera_gf_override = false;
            session.camera_offset_override = [0.0, 0.0];
            let (tx, ty, slot_zoom, _) = stage.resolve_camera_slot_target(
                clamped_slot,
                &session.active_player_id,
                &session.active_opponent_id,
            );
            let z_override = zoom.map(|z| z.clamp(0.35, 2.5));
            session.camera_slot_zoom_override = z_override;
            let final_zoom = z_override.unwrap_or(slot_zoom);
            stage.camera_base_zoom = final_zoom;
            if duration_sec > 0.01 {
                session.active_tweens.retain(|t| {
                    !matches!(
                        t.target,
                        TweenTarget::CameraPosX | TweenTarget::CameraPosY | TweenTarget::CameraZoom
                    )
                });
                session.active_tweens.push(ActiveTweenState {
                    tag: "__cam_slot_x".to_string(),
                    target: TweenTarget::CameraPosX,
                    start_val: stage.camera_x,
                    end_val: tx,
                    elapsed_sec: 0.0,
                    duration_sec,
                    ease: ease.clone(),
                });
                session.active_tweens.push(ActiveTweenState {
                    tag: "__cam_slot_y".to_string(),
                    target: TweenTarget::CameraPosY,
                    start_val: stage.camera_y,
                    end_val: ty,
                    elapsed_sec: 0.0,
                    duration_sec,
                    ease: ease.clone(),
                });
                session.active_tweens.push(ActiveTweenState {
                    tag: "__cam_slot_z".to_string(),
                    target: TweenTarget::CameraZoom,
                    start_val: stage.camera_zoom,
                    end_val: final_zoom,
                    elapsed_sec: 0.0,
                    duration_sec,
                    ease,
                });
            }
            None
        }
        ScriptCommand::SetCameraFollowLock { x, y } => {
            let (cx, cy) = stage.clamp_camera_to_background(x, y, stage.camera_base_zoom);
            session.camera_follow_lock = Some([cx, cy]);
            None
        }
        ScriptCommand::ClearCameraFollowLock => {
            session.camera_follow_lock = None;
            None
        }
        ScriptCommand::SetCameraClamp { enabled } => {
            stage.camera.clamp_to_background = enabled;
            let (cx, cy) =
                stage.clamp_camera_to_background(stage.camera_x, stage.camera_y, stage.camera_zoom);
            stage.camera_x = cx;
            stage.camera_y = cy;
            None
        }
        ScriptCommand::SetCameraVignette {
            color_argb,
            alpha,
            width,
            pulse,
            over_hud,
        } => {
            session.active_tweens.retain(|t| {
                !matches!(
                    t.target,
                    TweenTarget::CameraVignetteAlpha | TweenTarget::CameraVignetteWidth
                )
            });
            stage.vignette_color_argb = color_argb;
            stage.vignette_alpha = alpha.clamp(0.0, 1.0);
            stage.vignette_width = width.clamp(0.05, 0.50);
            stage.vignette_pulse = pulse.clamp(0.0, 1.0);
            stage.vignette_over_hud = over_hud;
            None
        }
        ScriptCommand::TweenCameraVignette {
            tag,
            target_alpha,
            duration_sec,
            ease,
            color_argb,
            target_width,
            pulse,
            over_hud,
        } => {
            if let Some(c) = color_argb {
                stage.vignette_color_argb = c;
            }
            if let Some(p) = pulse {
                stage.vignette_pulse = p.clamp(0.0, 1.0);
            }
            if let Some(oh) = over_hud {
                stage.vignette_over_hud = oh;
            }
            let end_alpha = target_alpha.clamp(0.0, 1.0);
            let tag_w = format!("{}__w", tag);
            session
                .active_tweens
                .retain(|t| t.tag != tag && t.tag != tag_w);
            if duration_sec <= 0.01 {
                stage.vignette_alpha = end_alpha;
                if let Some(w) = target_width {
                    stage.vignette_width = w.clamp(0.05, 0.50);
                }
            } else {
                session.active_tweens.push(ActiveTweenState {
                    tag,
                    target: TweenTarget::CameraVignetteAlpha,
                    start_val: stage.vignette_alpha,
                    end_val: end_alpha,
                    elapsed_sec: 0.0,
                    duration_sec: duration_sec.max(0.001),
                    ease: ease.clone(),
                });
                if let Some(w) = target_width {
                    session.active_tweens.push(ActiveTweenState {
                        tag: tag_w,
                        target: TweenTarget::CameraVignetteWidth,
                        start_val: stage.vignette_width,
                        end_val: w.clamp(0.05, 0.50),
                        elapsed_sec: 0.0,
                        duration_sec: duration_sec.max(0.001),
                        ease,
                    });
                }
            }
            None
        }
        ScriptCommand::ClearCameraVignette { duration_sec, ease } => {
            session.active_tweens.retain(|t| {
                !matches!(
                    t.target,
                    TweenTarget::CameraVignetteAlpha | TweenTarget::CameraVignetteWidth
                )
            });
            if duration_sec <= 0.01 {
                stage.vignette_alpha = 0.0;
            } else {
                session.active_tweens.push(ActiveTweenState {
                    tag: "__cam_vignette_clear".to_string(),
                    target: TweenTarget::CameraVignetteAlpha,
                    start_val: stage.vignette_alpha,
                    end_val: 0.0,
                    elapsed_sec: 0.0,
                    duration_sec: duration_sec.max(0.001),
                    ease,
                });
            }
            None
        }
        other => Some(other),
    }
}
