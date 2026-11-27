//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/simulation/camera_runtime.rs
//! Propósito: Lógica de cámara en tiempo real durante el gameplay:
//! - Cambio automático de ranura por sección del chart (`mustHitSection`, `gfSection`, `cameraSlot`)
//!   o por intervalo de beats (`BeatsInterval`).
//! - Aplicación de eventos de cámara del `chart.json` (`SelectCamera`, `FollowPos`, `Zoom`, `Vignette`).
//! - Seguimiento suave interpolado (`lerp`) con clamping obligatorio dentro del fondo del escenario.

use crate::session::{ActiveTweenState, GameplayRuntimeState, TweenTarget};
use pulse_core::chart::ParsedChartCameraAction;
use pulse_core::{CameraFollowMode, EntityRole, StageScene};

fn push_camera_pos_tween(
    session: &mut GameplayRuntimeState,
    stage: &StageScene,
    tx: f32,
    ty: f32,
    duration_sec: f32,
    ease: &str,
) {
    let (clamped_x, clamped_y) =
        stage.clamp_camera_to_background(tx, ty, stage.camera_base_zoom);
    session
        .active_tweens
        .retain(|t| !matches!(t.target, TweenTarget::CameraPosX | TweenTarget::CameraPosY));
    session.active_tweens.push(ActiveTweenState {
        tag: "__chart_cam_x".to_string(),
        target: TweenTarget::CameraPosX,
        start_val: stage.camera_x,
        end_val: clamped_x,
        elapsed_sec: 0.0,
        duration_sec: duration_sec.max(0.001),
        ease: ease.to_string(),
    });
    session.active_tweens.push(ActiveTweenState {
        tag: "__chart_cam_y".to_string(),
        target: TweenTarget::CameraPosY,
        start_val: stage.camera_y,
        end_val: clamped_y,
        elapsed_sec: 0.0,
        duration_sec: duration_sec.max(0.001),
        ease: ease.to_string(),
    });
}

fn push_camera_zoom_tween(
    session: &mut GameplayRuntimeState,
    stage: &StageScene,
    target_zoom: f32,
    duration_sec: f32,
    ease: &str,
) {
    session
        .active_tweens
        .retain(|t| !matches!(t.target, TweenTarget::CameraZoom));
    session.active_tweens.push(ActiveTweenState {
        tag: "__chart_cam_z".to_string(),
        target: TweenTarget::CameraZoom,
        start_val: stage.camera_zoom,
        end_val: target_zoom.clamp(0.35, 2.5),
        elapsed_sec: 0.0,
        duration_sec: duration_sec.max(0.001),
        ease: ease.to_string(),
    });
}

fn resolve_gf_target(stage: &StageScene) -> (f32, f32) {
    let cam0 = stage.resolve_camera_slot_target(0, "", "");
    if let Some(gf) = stage.entities.iter().find(|e| {
        e.role == EntityRole::Decoration
            && (e.id.eq_ignore_ascii_case("stage_speakers")
                || e.id.to_ascii_lowercase().contains("gf")
                || e.display_name.to_ascii_lowercase().contains("cabina"))
    }) {
        let dest_x = gf.x + gf.camera_offset[0];
        let dest_y = gf.y + gf.camera_offset[1] - 20.0;
        let pan = stage.camera.pan_strength.clamp(0.0, 1.0);
        let tx = (cam0.0 + (dest_x - cam0.0) * pan).clamp(-1000.0, 2600.0);
        let ty = (cam0.1 + (dest_y - cam0.1) * pan).clamp(-1000.0, 1800.0);
        (tx, ty)
    } else {
        (cam0.0, cam0.1)
    }
}

/// Actualiza la ranura de cámara activa al cambiar de sección del chart o al cruzar el intervalo de beats.
pub(super) fn update_section_and_beat_camera(
    session: &mut GameplayRuntimeState,
    stage: &StageScene,
    sec_idx: usize,
    must_hit_now: bool,
    gf_section_now: bool,
    section_cam_slot: Option<usize>,
    current_beat: i32,
) {
    if sec_idx as isize != session.last_section_idx {
        session.last_section_idx = sec_idx as isize;
        if let Some(explicit_slot) = section_cam_slot {
            let max_slot = stage.camera.cameras.len().saturating_sub(1);
            session.active_camera_slot = explicit_slot.min(max_slot);
            session.camera_follow_lock = None;
            session.camera_gf_override = false;
            session.camera_offset_override = [0.0, 0.0];
            session.camera_slot_zoom_override = None;
        } else if stage.camera.follow_mode == CameraFollowMode::ChartSections {
            session.camera_gf_override = gf_section_now;
            session.camera_offset_override = [0.0, 0.0];
            session.camera_slot_zoom_override = None;
            if !gf_section_now {
                session.active_camera_slot = if must_hit_now { 1 } else { 2 };
            }
        }
    }
    if stage.camera.follow_mode == CameraFollowMode::BeatsInterval
        && section_cam_slot.is_none()
        && session.camera_follow_lock.is_none()
    {
        let interval = stage.camera.follow_interval_beats.max(1) as i32;
        let bucket = current_beat.div_euclid(interval);
        if bucket != session.last_cam_beat_bucket {
            session.last_cam_beat_bucket = bucket;
            session.active_camera_slot = if bucket.rem_euclid(2) == 0 { 2 } else { 1 };
            session.camera_gf_override = false;
            session.camera_offset_override = [0.0, 0.0];
            session.camera_slot_zoom_override = None;
        }
    }
}

pub(super) fn apply_chart_camera_action(
    session: &mut GameplayRuntimeState,
    stage: &mut StageScene,
    action: ParsedChartCameraAction,
) {
    match action {
        ParsedChartCameraAction::SelectCamera {
            slot,
            focus_gf,
            offset_x,
            offset_y,
            zoom,
            duration_sec,
            ease,
        } => {
            session.camera_follow_lock = None;
            session.camera_gf_override = focus_gf;
            session.camera_offset_override = [offset_x, offset_y];

            let max_slot = stage.camera.cameras.len().saturating_sub(1);
            let target_slot = slot.unwrap_or(session.active_camera_slot).min(max_slot);
            if slot.is_some() {
                session.active_camera_slot = target_slot;
            }

            let (base_tx, base_ty, slot_zoom, _) = if focus_gf {
                let (gx, gy) = resolve_gf_target(stage);
                let (_, _, z, s) = stage.resolve_camera_slot_target(
                    session.active_camera_slot,
                    &session.active_player_id,
                    &session.active_opponent_id,
                );
                (gx, gy, z, s)
            } else {
                stage.resolve_camera_slot_target(
                    target_slot,
                    &session.active_player_id,
                    &session.active_opponent_id,
                )
            };

            let z_override = zoom.map(|z| z.clamp(0.35, 2.5));
            session.camera_slot_zoom_override = z_override;
            let final_zoom = z_override.unwrap_or(slot_zoom);
            stage.camera_base_zoom = final_zoom;
            let (tx, ty) = stage.clamp_camera_to_background(
                base_tx + offset_x,
                base_ty + offset_y,
                final_zoom,
            );

            if duration_sec > 0.01 {
                push_camera_pos_tween(session, stage, tx, ty, duration_sec, &ease);
                if z_override.is_some() {
                    push_camera_zoom_tween(session, stage, final_zoom, duration_sec, &ease);
                }
            } else {
                session
                    .active_tweens
                    .retain(|t| !matches!(t.target, TweenTarget::CameraPosX | TweenTarget::CameraPosY));
            }
        }
        ParsedChartCameraAction::FollowPos {
            x,
            y,
            zoom,
            duration_sec,
            ease,
        } => {
            let target_zoom = zoom.map(|z| z.clamp(0.35, 2.5)).unwrap_or(stage.camera_base_zoom);
            let (cx, cy) = stage.clamp_camera_to_background(x, y, target_zoom);
            session.camera_follow_lock = Some([cx, cy]);
            session.camera_gf_override = false;
            session.camera_offset_override = [0.0, 0.0];
            if let Some(z) = zoom {
                let cz = z.clamp(0.35, 2.5);
                session.camera_slot_zoom_override = Some(cz);
                stage.camera_base_zoom = cz;
                if duration_sec > 0.01 {
                    push_camera_zoom_tween(session, stage, cz, duration_sec, &ease);
                }
            }
            if duration_sec > 0.01 {
                push_camera_pos_tween(session, stage, cx, cy, duration_sec, &ease);
            }
        }
        ParsedChartCameraAction::ClearFollowPos => {
            session.camera_follow_lock = None;
            session.camera_offset_override = [0.0, 0.0];
        }
        ParsedChartCameraAction::SetZoom {
            zoom,
            is_direct_stage,
            duration_sec,
            ease,
        } => {
            let final_zoom = if is_direct_stage {
                (stage.camera.base_zoom * zoom).clamp(0.35, 2.5)
            } else {
                zoom.clamp(0.35, 2.5)
            };
            session.camera_slot_zoom_override = Some(final_zoom);
            stage.camera_base_zoom = final_zoom;
            if duration_sec > 0.01 {
                push_camera_zoom_tween(session, stage, final_zoom, duration_sec, &ease);
            } else {
                session
                    .active_tweens
                    .retain(|t| !matches!(t.target, TweenTarget::CameraZoom));
                stage.camera_zoom = final_zoom;
            }
        }
        ParsedChartCameraAction::AddZoomBump {
            cam_zoom_add,
            hud_zoom_add,
        } => {
            stage.camera_zoom = (stage.camera_zoom + cam_zoom_add).clamp(0.35, 2.5);
            session.hud_zoom = (session.hud_zoom + hud_zoom_add).clamp(0.35, 2.5);
        }
        ParsedChartCameraAction::SetVignette {
            enabled,
            color_argb,
            alpha,
            width,
            pulse,
            over_hud,
            duration_sec,
            ease,
        } => {
            session.active_tweens.retain(|t| {
                !matches!(
                    t.target,
                    TweenTarget::CameraVignetteAlpha | TweenTarget::CameraVignetteWidth
                )
            });
            if !enabled {
                if duration_sec <= 0.01 {
                    stage.vignette_alpha = 0.0;
                } else {
                    session.active_tweens.push(ActiveTweenState {
                        tag: "__chart_cam_vig_alpha".to_string(),
                        target: TweenTarget::CameraVignetteAlpha,
                        start_val: stage.vignette_alpha,
                        end_val: 0.0,
                        elapsed_sec: 0.0,
                        duration_sec: duration_sec.max(0.001),
                        ease,
                    });
                }
            } else {
                if let Some(c) = color_argb {
                    stage.vignette_color_argb = c;
                }
                if let Some(p) = pulse {
                    stage.vignette_pulse = p.clamp(0.0, 1.0);
                }
                if let Some(oh) = over_hud {
                    stage.vignette_over_hud = oh;
                }
                let target_alpha = alpha
                    .unwrap_or_else(|| {
                        if stage.camera.vignette_alpha > 0.01 {
                            stage.camera.vignette_alpha
                        } else {
                            0.58
                        }
                    })
                    .clamp(0.0, 1.0);
                let target_width = width.unwrap_or(stage.vignette_width).clamp(0.05, 0.50);
                if duration_sec <= 0.01 {
                    stage.vignette_alpha = target_alpha;
                    stage.vignette_width = target_width;
                } else {
                    session.active_tweens.push(ActiveTweenState {
                        tag: "__chart_cam_vig_alpha".to_string(),
                        target: TweenTarget::CameraVignetteAlpha,
                        start_val: stage.vignette_alpha,
                        end_val: target_alpha,
                        elapsed_sec: 0.0,
                        duration_sec: duration_sec.max(0.001),
                        ease: ease.clone(),
                    });
                    if (target_width - stage.vignette_width).abs() > 0.002 {
                        session.active_tweens.push(ActiveTweenState {
                            tag: "__chart_cam_vig_width".to_string(),
                            target: TweenTarget::CameraVignetteWidth,
                            start_val: stage.vignette_width,
                            end_val: target_width,
                            elapsed_sec: 0.0,
                            duration_sec: duration_sec.max(0.001),
                            ease,
                        });
                    }
                }
            }
        }
    }
}

pub(super) fn update_native_camera_follow(
    session: &mut GameplayRuntimeState,
    stage: &mut StageScene,
    dt: f32,
) {
    let (_, _, slot_zoom, slot_smooth) = stage.resolve_camera_slot_target(
        session.active_camera_slot,
        &session.active_player_id,
        &session.active_opponent_id,
    );
    let desired_base_zoom = session
        .camera_slot_zoom_override
        .unwrap_or(slot_zoom)
        .clamp(0.35, 2.5);
    stage.camera_base_zoom = desired_base_zoom;

    if stage.camera.enabled {
        let pos_tween_active = session
            .active_tweens
            .iter()
            .any(|t| matches!(t.target, TweenTarget::CameraPosX | TweenTarget::CameraPosY));
        if !pos_tween_active {
            let (raw_tx, raw_ty) = if let Some([lx, ly]) = session.camera_follow_lock {
                (lx, ly)
            } else if session.camera_gf_override {
                let (gx, gy) = resolve_gf_target(stage);
                (
                    gx + session.camera_offset_override[0],
                    gy + session.camera_offset_override[1],
                )
            } else {
                let (tx, ty, _, _) = stage.resolve_camera_slot_target(
                    session.active_camera_slot,
                    &session.active_player_id,
                    &session.active_opponent_id,
                );
                (
                    tx + session.camera_offset_override[0],
                    ty + session.camera_offset_override[1],
                )
            };
            let (target_x, target_y) =
                stage.clamp_camera_to_background(raw_tx, raw_ty, desired_base_zoom);
            let lerp_t = (dt * slot_smooth.clamp(1.0, 20.0)).min(1.0);
            stage.camera_x += (target_x - stage.camera_x) * lerp_t;
            stage.camera_y += (target_y - stage.camera_y) * lerp_t;
        }
    }

    let zoom_tween_active = session
        .active_tweens
        .iter()
        .any(|t| matches!(t.target, TweenTarget::CameraZoom));
    if !zoom_tween_active {
        stage.camera_zoom += (desired_base_zoom - stage.camera_zoom) * (dt * 6.0).min(1.0);
    }

    // Garantizar en cada frame que la cámara (incluyendo tweens o cambios de zoom) no descubra bordes negros fuera del fondo.
    if stage.camera.enabled {
        let (clamped_cam_x, clamped_cam_y) =
            stage.clamp_camera_to_background(stage.camera_x, stage.camera_y, stage.camera_zoom);
        stage.camera_x = clamped_cam_x;
        stage.camera_y = clamped_cam_y;
    }

    let hud_zoom_tween_active = session
        .active_tweens
        .iter()
        .any(|t| matches!(t.target, TweenTarget::HudProp { ref prop } if prop == "zoom"));
    if !hud_zoom_tween_active {
        session.hud_zoom += (1.0 - session.hud_zoom) * (dt * 6.0).min(1.0);
    }

    stage.camera_shake *= (1.0 - dt * 10.0).max(0.0);
}
