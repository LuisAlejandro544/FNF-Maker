//! Archivo: engine/crates/pulse_scripting/src/js_vm/bindings_hud_fx.rs
//! Propósito: Registra en el objeto global de QuickJS los bindings de estado de chart/gameplay,
//! HUD y visibilidad de flechas, gráficos/textos dinámicos, tweens, timers, audio, flashes,
//! precarga IO, swaps de escenario y game over.

use super::JsVmError;
use crate::bridge::{ScriptCommand, ScriptHostState, parse_color_argb};
use rquickjs::{Ctx, Function, Object};
use std::sync::Arc;

pub(super) fn register_hud_fx_bindings<'js>(
    ctx: &Ctx<'js>,
    globals: &Object<'js>,
    host: &Arc<ScriptHostState>,
) -> Result<(), JsVmError> {
    macro_rules! bind_js {
        ($name:expr, $closure:expr) => {{
            let f = Function::new(ctx.clone(), $closure)
                .map_err(|e| JsVmError::Eval(e.to_string()))?;
            globals
                .set($name, f)
                .map_err(|e| JsVmError::Eval(e.to_string()))?;
        }};
    }

    // --- Estado del Chart / Gameplay / HUD ---
    {
        let h = Arc::clone(host);
        bind_js!("getMustHitSection", move || -> bool { *h.must_hit_section.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getGfSection", move || -> bool { *h.gf_section.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getActiveBpm", move || -> f64 { *h.current_bpm.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getSongPosMs", move || -> f64 { *h.song_pos_ms.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getHealth", move || -> f32 { *h.health.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setHealth", move |value: f32| {
            if value.is_finite() {
                *h.health.lock() = value.clamp(0.0, 1.0);
                h.push_command(ScriptCommand::SetHealth(value));
            }
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("addHealth", move |delta: f32| {
            if delta.is_finite() {
                let mut hp = h.health.lock();
                *hp = (*hp + delta).clamp(0.0, 1.0);
                h.push_command(ScriptCommand::AddHealth(delta));
            }
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getScore", move || -> f64 { *h.score.lock() as f64 });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setScore", move |val: f64| {
            if val.is_finite() {
                let v = val.round() as i64;
                *h.score.lock() = v;
                h.push_command(ScriptCommand::SetScore(v));
            }
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("addScore", move |delta: f64| {
            if delta.is_finite() {
                let d = delta.round() as i64;
                *h.score.lock() += d;
                h.push_command(ScriptCommand::AddScore(d));
            }
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getCombo", move || -> i32 { *h.combo.lock() as i32 });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getMisses", move || -> i32 { *h.misses.lock() as i32 });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getHits", move || -> i32 { *h.hits.lock() as i32 });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getAccuracy", move || -> f32 { *h.accuracy.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getScrollSpeed", move || -> f64 { *h.scroll_speed.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setScrollSpeed", move |speed: f64| {
            if speed.is_finite() {
                let clamped = speed.clamp(0.3, 6.0);
                *h.scroll_speed.lock() = clamped;
                h.push_command(ScriptCommand::SetScrollSpeed(clamped));
            }
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getDownscroll", move || -> bool { *h.downscroll.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setDownscroll", move |enabled: bool| {
            *h.downscroll.lock() = enabled;
            h.push_command(ScriptCommand::SetDownscroll(enabled));
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setHudVisible", move |visible: bool| {
            *h.hud_visible.lock() = visible;
            h.push_command(ScriptCommand::SetHudProp {
                prop: "visible".to_string(),
                value: if visible { 1.0 } else { 0.0 },
            });
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getHudVisible", move || -> bool { *h.hud_visible.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setHudOpacity", move |alpha: f32| {
            if alpha.is_finite() {
                let clamped = alpha.clamp(0.0, 1.0);
                *h.hud_opacity.lock() = clamped;
                h.push_command(ScriptCommand::SetHudProp {
                    prop: "opacity".to_string(),
                    value: clamped,
                });
            }
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getHudOpacity", move || -> f32 { *h.hud_opacity.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setHudZoom", move |zoom: f32| {
            if zoom.is_finite() {
                let clamped = zoom.clamp(0.4, 2.5);
                *h.hud_zoom.lock() = clamped;
                h.push_command(ScriptCommand::SetHudProp {
                    prop: "zoom".to_string(),
                    value: clamped,
                });
            }
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getHudZoom", move || -> f32 { *h.hud_zoom.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setArrowsVisible", move |visible: bool| {
            *h.arrows_visible.lock() = visible;
            h.push_command(ScriptCommand::SetArrowsProp {
                prop: "visible".to_string(),
                value: if visible { 1.0 } else { 0.0 },
            });
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getArrowsVisible", move || -> bool { *h.arrows_visible.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setArrowsOpacity", move |alpha: f32| {
            if alpha.is_finite() {
                let clamped = alpha.clamp(0.0, 1.0);
                *h.arrows_opacity.lock() = clamped;
                h.push_command(ScriptCommand::SetArrowsProp {
                    prop: "opacity".to_string(),
                    value: clamped,
                });
            }
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getArrowsOpacity", move || -> f32 { *h.arrows_opacity.lock() });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setScreenBlackout", move |alpha: f32| {
            if alpha.is_finite() {
                let clamped = alpha.clamp(0.0, 1.0);
                *h.screen_blackout_alpha.lock() = clamped;
                h.push_command(ScriptCommand::SetScreenBlackout(clamped));
            }
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("getScreenBlackout", move || -> f32 { *h.screen_blackout_alpha.lock() });
    }

    // --- Gráficos, Textos y Clonación dinámica ---
    {
        let h = Arc::clone(host);
        bind_js!(
            "makeGraphic",
            move |id: String, x: f32, y: f32, width: f32, height: f32, color: String| {
                h.push_command(ScriptCommand::MakeGraphic {
                    id,
                    x,
                    y,
                    width,
                    height,
                    color_argb: parse_color_argb(&color),
                    is_hud: false,
                });
            }
        );
    }
    {
        let h = Arc::clone(host);
        bind_js!(
            "makeHudGraphic",
            move |id: String, x: f32, y: f32, width: f32, height: f32, color: String| {
                h.push_command(ScriptCommand::MakeGraphic {
                    id,
                    x,
                    y,
                    width,
                    height,
                    color_argb: parse_color_argb(&color),
                    is_hud: true,
                });
            }
        );
    }
    {
        let h = Arc::clone(host);
        bind_js!(
            "makeText",
            move |id: String, text: String, x: f32, y: f32, font_size: f32, color: String| {
                h.push_command(ScriptCommand::MakeText {
                    id,
                    text,
                    x,
                    y,
                    font_size,
                    color_argb: parse_color_argb(&color),
                    is_hud: true,
                });
            }
        );
    }
    {
        let h = Arc::clone(host);
        bind_js!("setTextString", move |id: String, text: String| {
            h.push_command(ScriptCommand::SetTextString { id, text });
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("cloneSprite", move |id: String, source_id: String, x: f32, y: f32| {
            h.push_command(ScriptCommand::CloneSprite {
                id,
                source_id,
                x,
                y,
            });
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("removeSprite", move |id: String| {
            h.push_command(ScriptCommand::RemoveSprite(id));
        });
    }

    // --- Tweens Nativos ---
    {
        let h = Arc::clone(host);
        bind_js!(
            "tweenEntityProp",
            move |tag: String, entity_id: String, prop: String, target: f32, dur: f32, ease: String| {
                h.push_command(ScriptCommand::TweenEntityProp {
                    tag,
                    entity_id,
                    prop,
                    target,
                    duration_sec: dur,
                    ease,
                });
            }
        );
    }
    {
        let h = Arc::clone(host);
        bind_js!(
            "tweenCameraPos",
            move |tag: String, x: f32, y: f32, dur: f32, ease: String| {
                h.push_command(ScriptCommand::TweenCameraPos {
                    tag,
                    x,
                    y,
                    duration_sec: dur,
                    ease,
                });
            }
        );
    }
    {
        let h = Arc::clone(host);
        bind_js!(
            "tweenCameraZoom",
            move |tag: String, zoom: f32, dur: f32, ease: String| {
                h.push_command(ScriptCommand::TweenCameraZoom {
                    tag,
                    zoom,
                    duration_sec: dur,
                    ease,
                });
            }
        );
    }
    {
        let h = Arc::clone(host);
        bind_js!(
            "tweenStrumProp",
            move |tag: String, strum_idx: i32, prop: String, target: f32, dur: f32, ease: String| {
                h.push_command(ScriptCommand::TweenStrumProp {
                    tag,
                    strum_idx: strum_idx.clamp(0, 7) as usize,
                    prop,
                    target,
                    duration_sec: dur,
                    ease,
                });
            }
        );
    }
    {
        let h = Arc::clone(host);
        bind_js!(
            "tweenHudProp",
            move |tag: String, prop: String, target: f32, dur: f32, ease: String| {
                h.push_command(ScriptCommand::TweenHudProp {
                    tag,
                    prop,
                    target,
                    duration_sec: dur,
                    ease,
                });
            }
        );
    }
    {
        let h = Arc::clone(host);
        bind_js!(
            "tweenHudOpacity",
            move |tag: String, target: f32, dur: f32, ease: String| {
                h.push_command(ScriptCommand::TweenHudProp {
                    tag,
                    prop: "opacity".to_string(),
                    target,
                    duration_sec: dur,
                    ease,
                });
            }
        );
    }
    {
        let h = Arc::clone(host);
        bind_js!(
            "tweenArrowsOpacity",
            move |tag: String, target: f32, dur: f32, ease: String| {
                h.push_command(ScriptCommand::TweenArrowsProp {
                    tag,
                    prop: "opacity".to_string(),
                    target,
                    duration_sec: dur,
                    ease,
                });
            }
        );
    }
    {
        let h = Arc::clone(host);
        bind_js!(
            "tweenScreenBlackout",
            move |tag: String, target: f32, dur: f32, ease: String| {
                h.push_command(ScriptCommand::TweenScreenBlackout {
                    tag,
                    target,
                    duration_sec: dur,
                    ease,
                });
            }
        );
    }
    {
        let h = Arc::clone(host);
        bind_js!("cancelTween", move |tag: String| {
            h.push_command(ScriptCommand::CancelTween(tag));
        });
    }

    // --- Temporizadores y Audio ---
    {
        let h = Arc::clone(host);
        bind_js!("runTimer", move |tag: String, duration_sec: f32| {
            h.push_command(ScriptCommand::RunTimer { tag, duration_sec });
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("cancelTimer", move |tag: String| {
            h.push_command(ScriptCommand::CancelTimer(tag));
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setTrackVolume", move |slot: i32, gain: f32| {
            h.push_command(ScriptCommand::SetTrackVolume {
                slot: slot.max(0) as usize,
                gain,
            });
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("setTrackMute", move |slot: i32, muted: bool| {
            h.push_command(ScriptCommand::SetTrackMute {
                slot: slot.max(0) as usize,
                muted,
            });
        });
    }

    // --- Flashes, Precarga IO, Swaps de Escenario y Personajes Activos ---
    {
        let h = Arc::clone(host);
        bind_js!("flashCamera", move |color: String, duration_sec: f32| {
            h.push_command(ScriptCommand::FlashEffect {
                color_argb: parse_color_argb(&color),
                duration_sec: duration_sec.max(0.02),
                start_alpha: 1.0,
                over_hud: false,
            });
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("flashHud", move |color: String, duration_sec: f32| {
            h.push_command(ScriptCommand::FlashEffect {
                color_argb: parse_color_argb(&color),
                duration_sec: duration_sec.max(0.02),
                start_alpha: 1.0,
                over_hud: true,
            });
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!(
            "flashEffect",
            move |color: String, duration_sec: f32, start_alpha: f32, over_hud: bool| {
                h.push_command(ScriptCommand::FlashEffect {
                    color_argb: parse_color_argb(&color),
                    duration_sec: duration_sec.max(0.02),
                    start_alpha: start_alpha.clamp(0.0, 1.0),
                    over_hud,
                });
            }
        );
    }
    {
        let h = Arc::clone(host);
        bind_js!("preloadEntity", move |entity_id: String| {
            h.push_command(ScriptCommand::PreloadEntity(entity_id));
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("retireEntity", move |entity_id: String| {
            h.push_command(ScriptCommand::RetireEntity(entity_id));
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!(
            "swapEntity",
            move |old_id: String, new_id: String, dispose_old: bool| {
                h.push_command(ScriptCommand::SwapEntity {
                    old_id,
                    new_id,
                    dispose_old,
                });
            }
        );
    }
    {
        let h = Arc::clone(host);
        bind_js!(
            "setActiveCharacter",
            move |role: String, entity_id: String, dispose_old: bool| {
                h.push_command(ScriptCommand::SetActiveCharacter {
                    role,
                    entity_id,
                    dispose_old,
                });
            }
        );
    }
    {
        let h = Arc::clone(host);
        bind_js!("triggerGameOver", move || {
            *h.game_over.lock() = true;
            h.push_command(ScriptCommand::TriggerGameOver);
        });
    }
    {
        let h = Arc::clone(host);
        bind_js!("isGameOver", move || -> bool { *h.game_over.lock() });
    }

    Ok(())
}
