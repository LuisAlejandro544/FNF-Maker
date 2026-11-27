//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/script_sync/tweens_timers.rs
//! Propósito: Evaluación de curvas de interpolación (`quad`, `cubic`, `sine`, `back`, `elastic`,
//! `bounce`), avance de temporizadores (`onTimerCompleted`) y actualización por frame de tweens
//! activos sobre entidades, cámara, receptores, HUD, flechas, blackout y viñeta (`onTweenCompleted`).

use super::props::{apply_entity_prop, apply_strum_prop};
use crate::session::{GameplayRuntimeState, TweenTarget};
use pulse_core::StageScene;

pub(super) fn eval_ease(name: &str, raw_t: f32) -> f32 {
    let t = raw_t.clamp(0.0, 1.0);
    match name.trim().to_ascii_lowercase().as_str() {
        "quadin" => t * t,
        "quadout" => 1.0 - (1.0 - t) * (1.0 - t),
        "quadinout" => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
            }
        }
        "cubicin" => t * t * t,
        "cubicout" => 1.0 - (1.0 - t).powi(3),
        "cubicinout" => {
            if t < 0.5 {
                4.0 * t * t * t
            } else {
                1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
            }
        }
        "sinein" => 1.0 - ((t * std::f32::consts::FRAC_PI_2).cos()),
        "sineout" => (t * std::f32::consts::FRAC_PI_2).sin(),
        "sineinout" => -((std::f32::consts::PI * t).cos() - 1.0) / 2.0,
        "backout" => {
            let c1 = 1.70158;
            let c3 = c1 + 1.0;
            1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
        }
        "elasticout" => {
            if t <= 0.0 || t >= 1.0 {
                t
            } else {
                let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                2.0f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
            }
        }
        "bounceout" => {
            let n1 = 7.5625;
            let d1 = 2.75;
            if t < 1.0 / d1 {
                n1 * t * t
            } else if t < 2.0 / d1 {
                let u = t - 1.5 / d1;
                n1 * u * u + 0.75
            } else if t < 2.5 / d1 {
                let u = t - 2.25 / d1;
                n1 * u * u + 0.9375
            } else {
                let u = t - 2.625 / d1;
                n1 * u * u + 0.984375
            }
        }
        _ => t,
    }
}

/// Avanza temporizadores activos y dispara `onTimerCompleted` al expirar.
pub(in crate::session::gameplay_ops) fn update_active_timers(
    session: &mut GameplayRuntimeState,
    dt: f32,
) {
    if session.active_timers.is_empty() {
        return;
    }
    let mut completed_timers: Vec<String> = Vec::new();
    session.active_timers.retain_mut(|timer| {
        timer.remaining_sec -= dt;
        if timer.remaining_sec <= 0.0 {
            completed_timers.push(timer.tag.clone());
            false
        } else {
            true
        }
    });
    for tag in completed_timers {
        session.runtime.call_on_timer_completed(&tag);
    }
}

/// Avanza tweens activos sobre entidades, cámara y receptores, disparando `onTweenCompleted`.
pub(in crate::session::gameplay_ops) fn update_active_tweens(
    session: &mut GameplayRuntimeState,
    stage: &mut StageScene,
    dt: f32,
) {
    if session.active_tweens.is_empty() {
        return;
    }
    let mut completed_tweens: Vec<String> = Vec::new();
    let mut sort_z = false;
    session.active_tweens.retain_mut(|tw| {
        tw.elapsed_sec += dt;
        let raw_t = if tw.duration_sec <= 0.0001 {
            1.0
        } else {
            (tw.elapsed_sec / tw.duration_sec).clamp(0.0, 1.0)
        };
        let eased = eval_ease(&tw.ease, raw_t);
        let val = tw.start_val + (tw.end_val - tw.start_val) * eased;
        match &tw.target {
            TweenTarget::EntityProp { entity_id, prop } => {
                if apply_entity_prop(
                    &mut session.bundle.active_entities,
                    &mut session.dynamic_items,
                    entity_id,
                    prop,
                    val,
                ) {
                    sort_z = true;
                }
            }
            TweenTarget::CameraPosX => {
                stage.camera_x = val.clamp(-1000.0, 2600.0);
            }
            TweenTarget::CameraPosY => {
                stage.camera_y = val.clamp(-1000.0, 1800.0);
            }
            TweenTarget::CameraZoom => {
                stage.camera_zoom = val.clamp(0.35, 2.5);
            }
            TweenTarget::StrumProp { strum_idx, prop } => {
                apply_strum_prop(&mut session.strum_overrides, *strum_idx, prop, val);
            }
            TweenTarget::HudProp { prop } => match prop.as_str() {
                "opacity" | "alpha" => {
                    let clamped = val.clamp(0.0, 1.0);
                    session.hud_opacity = clamped;
                    if raw_t >= 1.0 {
                        session.hud_visible = clamped > 0.01;
                    }
                }
                "zoom" => session.hud_zoom = val.clamp(0.3, 2.5),
                "visible" => session.hud_visible = val > 0.0,
                _ => {}
            },
            TweenTarget::ArrowsProp { prop } => match prop.as_str() {
                "opacity" | "alpha" => {
                    let clamped = val.clamp(0.0, 1.0);
                    session.arrows_opacity = clamped;
                    if raw_t >= 1.0 {
                        session.arrows_visible = clamped > 0.01;
                    }
                }
                "visible" => session.arrows_visible = val > 0.0,
                _ => {}
            },
            TweenTarget::ScreenBlackout => {
                session.screen_blackout_alpha = val.clamp(0.0, 1.0);
            }
            TweenTarget::CameraVignetteAlpha => {
                stage.vignette_alpha = val.clamp(0.0, 1.0);
            }
            TweenTarget::CameraVignetteWidth => {
                stage.vignette_width = val.clamp(0.05, 0.50);
            }
        }
        if raw_t >= 1.0 {
            completed_tweens.push(tw.tag.clone());
            false
        } else {
            true
        }
    });
    if sort_z {
        session.bundle.active_entities.sort_by_key(|e| e.z_index);
    }
    for tag in completed_tweens {
        session.runtime.call_on_tween_completed(&tag);
    }
}
