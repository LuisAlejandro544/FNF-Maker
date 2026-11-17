//! Archivo: engine/crates/pulse_scripting/src/js_vm.rs
//! Propósito: Máquina virtual JavaScript (QuickJS-NG vía `rquickjs`, licencia MIT)
//! que ejecuta tanto scripts `.js` puros como scripts `.ts` previamente transpilados por `oxc`.
//! Expone exactamente la misma API global ilimitada que Lua 5.4 y reporta errores al motor.

use crate::bridge::{ScriptCommand, ScriptHostState, parse_color_argb};
use rquickjs::{Context, Function, Runtime};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Instant;
use thiserror::Error;

const JS_INIT_BUDGET_MS: u64 = 250;
const JS_FRAME_BUDGET_MS: u64 = 25;
const JS_MAX_WATCHDOG_TRIPS: u32 = 3;

#[derive(Debug, Error)]
pub enum JsVmError {
    #[error("Error creando runtime QuickJS: {0}")]
    Runtime(String),
    #[error("Error ejecutando script JS: {0}")]
    Eval(String),
}

pub struct JsScriptVm {
    _runtime: Runtime,
    context: Context,
    host: Arc<ScriptHostState>,
    script_name: String,
    watchdog_origin: Instant,
    watchdog_deadline_us: Arc<AtomicU64>,
    watchdog_tripped: Arc<AtomicBool>,
    watchdog_trips: u32,
    disabled_by_watchdog: bool,
}

impl JsScriptVm {
    pub fn new(js_source: &str, host: Arc<ScriptHostState>) -> Result<Self, JsVmError> {
        Self::new_named("script.js", js_source, host)
    }

    pub fn new_named(
        script_name: &str,
        js_source: &str,
        host: Arc<ScriptHostState>,
    ) -> Result<Self, JsVmError> {
        let runtime = Runtime::new().map_err(|e| JsVmError::Runtime(e.to_string()))?;
        let watchdog_origin = Instant::now();
        let watchdog_deadline_us = Arc::new(AtomicU64::new(0));
        let watchdog_tripped = Arc::new(AtomicBool::new(false));

        {
            let origin = watchdog_origin;
            let deadline_ref = Arc::clone(&watchdog_deadline_us);
            let tripped_ref = Arc::clone(&watchdog_tripped);
            runtime.set_interrupt_handler(Some(Box::new(move || -> bool {
                let limit_us = deadline_ref.load(Ordering::Relaxed);
                if limit_us == 0 {
                    return false;
                }
                let elapsed = origin.elapsed().as_micros() as u64;
                if elapsed > limit_us {
                    tripped_ref.store(true, Ordering::Relaxed);
                    return true;
                }
                false
            })));
        }

        let context = Context::full(&runtime).map_err(|e| JsVmError::Runtime(e.to_string()))?;

        context.with(|ctx| -> Result<(), JsVmError> {
            let globals = ctx.globals();

            macro_rules! bind_js {
                ($name:expr, $closure:expr) => {{
                    let f = Function::new(ctx.clone(), $closure)
                        .map_err(|e| JsVmError::Eval(e.to_string()))?;
                    globals
                        .set($name, f)
                        .map_err(|e| JsVmError::Eval(e.to_string()))?;
                }};
            }

            {
                let h = Arc::clone(&host);
                bind_js!("log", move |msg: String| {
                    h.push_command(ScriptCommand::Log(msg));
                });
            }

            // --- Cámara ---
            {
                let h = Arc::clone(&host);
                bind_js!("setCameraPos", move |x: f32, y: f32| {
                    if x.is_finite() && y.is_finite() {
                        *h.camera_x.lock() = x;
                        *h.camera_y.lock() = y;
                        h.push_command(ScriptCommand::SetCameraPos { x, y });
                    }
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("setCameraZoom", move |zoom: f32| {
                    if zoom.is_finite() {
                        *h.camera_zoom.lock() = zoom;
                        h.push_command(ScriptCommand::SetCameraZoom(zoom));
                    }
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("setCameraBaseZoom", move |zoom: f32| {
                    if zoom.is_finite() {
                        *h.camera_base_zoom.lock() = zoom;
                        h.push_command(ScriptCommand::SetCameraBaseZoom(zoom));
                    }
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getCameraX", move || -> f32 { *h.camera_x.lock() });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getCameraY", move || -> f32 { *h.camera_y.lock() });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getCameraZoom", move || -> f32 { *h.camera_zoom.lock() });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getCameraBaseZoom", move || -> f32 { *h.camera_base_zoom.lock() });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("shakeCamera", move |intensity: f32| {
                    h.push_command(ScriptCommand::ShakeCamera { intensity });
                });
            }

            // --- Entidades ---
            {
                let h = Arc::clone(&host);
                bind_js!("getEntityX", move |id_or_role: String| -> Option<f32> {
                    h.entity_positions.lock().get(&id_or_role).map(|p| p.0)
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getEntityY", move |id_or_role: String| -> Option<f32> {
                    h.entity_positions.lock().get(&id_or_role).map(|p| p.1)
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getEntityProp", move |entity_id: String, prop: String| -> Option<f32> {
                    h.entity_props
                        .lock()
                        .get(&entity_id)
                        .and_then(|snap| snap.get(&prop))
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("setEntityProp", move |entity_id: String, prop: String, value: f32| {
                    if let Some(snap) = h.entity_props.lock().get_mut(&entity_id) {
                        snap.set(&prop, value);
                    }
                    h.push_command(ScriptCommand::SetEntityProp {
                        entity_id,
                        prop,
                        value,
                    });
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("playAnim", move |entity_id: String, anim_name: String| {
                    h.push_command(ScriptCommand::PlayEntityAnim {
                        entity_id,
                        anim_name,
                    });
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("playEntityAnim", move |entity_id: String, anim_name: String| {
                    h.push_command(ScriptCommand::PlayEntityAnim {
                        entity_id,
                        anim_name,
                    });
                });
            }

            // --- Strums / Modcharts ---
            {
                let h = Arc::clone(&host);
                bind_js!("setStrumProp", move |strum_idx: i32, prop: String, value: f32| {
                    let idx = strum_idx.clamp(0, 7) as usize;
                    h.strum_props.lock()[idx].set(&prop, value);
                    h.push_command(ScriptCommand::SetStrumProp {
                        strum_idx: idx,
                        prop,
                        value,
                    });
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getStrumProp", move |strum_idx: i32, prop: String| -> f32 {
                    let idx = strum_idx.clamp(0, 7) as usize;
                    h.strum_props.lock()[idx].get(&prop).unwrap_or(0.0)
                });
            }

            // --- Estado del Chart / Gameplay / HUD ---
            {
                let h = Arc::clone(&host);
                bind_js!("getMustHitSection", move || -> bool { *h.must_hit_section.lock() });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getActiveBpm", move || -> f64 { *h.current_bpm.lock() });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getSongPosMs", move || -> f64 { *h.song_pos_ms.lock() });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getHealth", move || -> f32 { *h.health.lock() });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("setHealth", move |value: f32| {
                    if value.is_finite() {
                        *h.health.lock() = value.clamp(0.0, 1.0);
                        h.push_command(ScriptCommand::SetHealth(value));
                    }
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("addHealth", move |delta: f32| {
                    if delta.is_finite() {
                        let mut hp = h.health.lock();
                        *hp = (*hp + delta).clamp(0.0, 1.0);
                        h.push_command(ScriptCommand::AddHealth(delta));
                    }
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getScore", move || -> f64 { *h.score.lock() as f64 });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("setScore", move |val: f64| {
                    if val.is_finite() {
                        let v = val.round() as i64;
                        *h.score.lock() = v;
                        h.push_command(ScriptCommand::SetScore(v));
                    }
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("addScore", move |delta: f64| {
                    if delta.is_finite() {
                        let d = delta.round() as i64;
                        *h.score.lock() += d;
                        h.push_command(ScriptCommand::AddScore(d));
                    }
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getCombo", move || -> i32 { *h.combo.lock() as i32 });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getMisses", move || -> i32 { *h.misses.lock() as i32 });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getHits", move || -> i32 { *h.hits.lock() as i32 });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getAccuracy", move || -> f32 { *h.accuracy.lock() });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getScrollSpeed", move || -> f64 { *h.scroll_speed.lock() });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("setScrollSpeed", move |speed: f64| {
                    if speed.is_finite() {
                        let clamped = speed.clamp(0.3, 6.0);
                        *h.scroll_speed.lock() = clamped;
                        h.push_command(ScriptCommand::SetScrollSpeed(clamped));
                    }
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("getDownscroll", move || -> bool { *h.downscroll.lock() });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("setDownscroll", move |enabled: bool| {
                    *h.downscroll.lock() = enabled;
                    h.push_command(ScriptCommand::SetDownscroll(enabled));
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("setHudVisible", move |visible: bool| {
                    h.push_command(ScriptCommand::SetHudProp {
                        prop: "visible".to_string(),
                        value: if visible { 1.0 } else { 0.0 },
                    });
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("setHudOpacity", move |alpha: f32| {
                    h.push_command(ScriptCommand::SetHudProp {
                        prop: "opacity".to_string(),
                        value: alpha,
                    });
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("setHudZoom", move |zoom: f32| {
                    h.push_command(ScriptCommand::SetHudProp {
                        prop: "zoom".to_string(),
                        value: zoom,
                    });
                });
            }

            // --- Gráficos, Textos y Clonación dinámica ---
            {
                let h = Arc::clone(&host);
                bind_js!("makeGraphic", move |id: String, x: f32, y: f32, width: f32, height: f32, color: String| {
                    h.push_command(ScriptCommand::MakeGraphic {
                        id,
                        x,
                        y,
                        width,
                        height,
                        color_argb: parse_color_argb(&color),
                        is_hud: false,
                    });
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("makeHudGraphic", move |id: String, x: f32, y: f32, width: f32, height: f32, color: String| {
                    h.push_command(ScriptCommand::MakeGraphic {
                        id,
                        x,
                        y,
                        width,
                        height,
                        color_argb: parse_color_argb(&color),
                        is_hud: true,
                    });
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("makeText", move |id: String, text: String, x: f32, y: f32, font_size: f32, color: String| {
                    h.push_command(ScriptCommand::MakeText {
                        id,
                        text,
                        x,
                        y,
                        font_size,
                        color_argb: parse_color_argb(&color),
                        is_hud: true,
                    });
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("setTextString", move |id: String, text: String| {
                    h.push_command(ScriptCommand::SetTextString { id, text });
                });
            }
            {
                let h = Arc::clone(&host);
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
                let h = Arc::clone(&host);
                bind_js!("removeSprite", move |id: String| {
                    h.push_command(ScriptCommand::RemoveSprite(id));
                });
            }

            // --- Tweens Nativos ---
            {
                let h = Arc::clone(&host);
                bind_js!("tweenEntityProp", move |tag: String, entity_id: String, prop: String, target: f32, dur: f32, ease: String| {
                    h.push_command(ScriptCommand::TweenEntityProp {
                        tag,
                        entity_id,
                        prop,
                        target,
                        duration_sec: dur,
                        ease,
                    });
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("tweenCameraPos", move |tag: String, x: f32, y: f32, dur: f32, ease: String| {
                    h.push_command(ScriptCommand::TweenCameraPos {
                        tag,
                        x,
                        y,
                        duration_sec: dur,
                        ease,
                    });
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("tweenCameraZoom", move |tag: String, zoom: f32, dur: f32, ease: String| {
                    h.push_command(ScriptCommand::TweenCameraZoom {
                        tag,
                        zoom,
                        duration_sec: dur,
                        ease,
                    });
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("tweenStrumProp", move |tag: String, strum_idx: i32, prop: String, target: f32, dur: f32, ease: String| {
                    h.push_command(ScriptCommand::TweenStrumProp {
                        tag,
                        strum_idx: strum_idx.clamp(0, 7) as usize,
                        prop,
                        target,
                        duration_sec: dur,
                        ease,
                    });
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("cancelTween", move |tag: String| {
                    h.push_command(ScriptCommand::CancelTween(tag));
                });
            }

            // --- Temporizadores y Audio ---
            {
                let h = Arc::clone(&host);
                bind_js!("runTimer", move |tag: String, duration_sec: f32| {
                    h.push_command(ScriptCommand::RunTimer { tag, duration_sec });
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("cancelTimer", move |tag: String| {
                    h.push_command(ScriptCommand::CancelTimer(tag));
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("setTrackVolume", move |slot: i32, gain: f32| {
                    h.push_command(ScriptCommand::SetTrackVolume {
                        slot: slot.max(0) as usize,
                        gain,
                    });
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("setTrackMute", move |slot: i32, muted: bool| {
                    h.push_command(ScriptCommand::SetTrackMute {
                        slot: slot.max(0) as usize,
                        muted,
                    });
                });
            }

            // --- Flashes, Precarga IO, Swaps de Escenario y Personajes Activos ---
            {
                let h = Arc::clone(&host);
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
                let h = Arc::clone(&host);
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
                let h = Arc::clone(&host);
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
                let h = Arc::clone(&host);
                bind_js!("preloadEntity", move |entity_id: String| {
                    h.push_command(ScriptCommand::PreloadEntity(entity_id));
                });
            }
            {
                let h = Arc::clone(&host);
                bind_js!("retireEntity", move |entity_id: String| {
                    h.push_command(ScriptCommand::RetireEntity(entity_id));
                });
            }
            {
                let h = Arc::clone(&host);
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
                let h = Arc::clone(&host);
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

            Self::arm_deadline(
                &watchdog_origin,
                &watchdog_deadline_us,
                &watchdog_tripped,
                JS_INIT_BUDGET_MS,
            );
            let eval_res = ctx.eval::<(), _>(js_source);
            watchdog_deadline_us.store(0, Ordering::Relaxed);
            if let Err(e) = eval_res {
                if watchdog_tripped.load(Ordering::Relaxed) {
                    return Err(JsVmError::Eval(format!(
                        "Watchdog QuickJS: inicialización excedió {}ms y fue abortada",
                        JS_INIT_BUDGET_MS
                    )));
                }
                return Err(JsVmError::Eval(e.to_string()));
            }

            Ok(())
        })?;

        Ok(Self {
            _runtime: runtime,
            context,
            host,
            script_name: script_name.to_string(),
            watchdog_origin,
            watchdog_deadline_us,
            watchdog_tripped,
            watchdog_trips: 0,
            disabled_by_watchdog: false,
        })
    }

    fn arm_deadline(
        origin: &Instant,
        deadline: &AtomicU64,
        tripped: &AtomicBool,
        budget_ms: u64,
    ) {
        tripped.store(false, Ordering::Relaxed);
        let now_us = origin.elapsed().as_micros() as u64;
        deadline.store(now_us.saturating_add(budget_ms * 1000), Ordering::Relaxed);
    }

    fn handle_js_error(&mut self, fn_name: &str, err_msg: String) {
        if self.watchdog_tripped.swap(false, Ordering::Relaxed) {
            self.watchdog_trips += 1;
            if self.watchdog_trips >= JS_MAX_WATCHDOG_TRIPS {
                self.disabled_by_watchdog = true;
                self.host.push_runtime_error(format!(
                    "Watchdog JS/TS [{}]: script en cuarentena tras {} excesos en {}",
                    self.script_name, self.watchdog_trips, fn_name
                ));
            } else {
                self.host.push_runtime_error(format!(
                    "Watchdog JS/TS [{}] ({}): interrumpido por superar {}ms",
                    self.script_name, fn_name, JS_FRAME_BUDGET_MS
                ));
            }
        } else {
            self.host
                .push_runtime_error(format!("JS [{}] {}: {}", self.script_name, fn_name, err_msg));
        }
    }

    pub fn is_quarantined(&self) -> bool {
        self.disabled_by_watchdog
    }

    pub fn call_void(&mut self, fn_name: &str) {
        if self.disabled_by_watchdog {
            return;
        }
        let mut call_err: Option<String> = None;
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>(fn_name) {
                Self::arm_deadline(
                    &self.watchdog_origin,
                    &self.watchdog_deadline_us,
                    &self.watchdog_tripped,
                    JS_FRAME_BUDGET_MS,
                );
                let res = func.call::<_, ()>(());
                self.watchdog_deadline_us.store(0, Ordering::Relaxed);
                if let Err(e) = res {
                    call_err = Some(e.to_string());
                }
            }
        });
        if let Some(err) = call_err {
            self.handle_js_error(fn_name, err);
        }
    }

    pub fn call_f32(&mut self, fn_name: &str, arg: f32) {
        if self.disabled_by_watchdog {
            return;
        }
        let mut call_err: Option<String> = None;
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>(fn_name) {
                Self::arm_deadline(
                    &self.watchdog_origin,
                    &self.watchdog_deadline_us,
                    &self.watchdog_tripped,
                    JS_FRAME_BUDGET_MS,
                );
                let res = func.call::<_, ()>((arg,));
                self.watchdog_deadline_us.store(0, Ordering::Relaxed);
                if let Err(e) = res {
                    call_err = Some(e.to_string());
                }
            }
        });
        if let Some(err) = call_err {
            self.handle_js_error(fn_name, err);
        }
    }

    pub fn call_i32(&mut self, fn_name: &str, arg: i32) {
        if self.disabled_by_watchdog {
            return;
        }
        let mut call_err: Option<String> = None;
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>(fn_name) {
                Self::arm_deadline(
                    &self.watchdog_origin,
                    &self.watchdog_deadline_us,
                    &self.watchdog_tripped,
                    JS_FRAME_BUDGET_MS,
                );
                let res = func.call::<_, ()>((arg,));
                self.watchdog_deadline_us.store(0, Ordering::Relaxed);
                if let Err(e) = res {
                    call_err = Some(e.to_string());
                }
            }
        });
        if let Some(err) = call_err {
            self.handle_js_error(fn_name, err);
        }
    }

    pub fn call_str(&mut self, fn_name: &str, arg: &str) {
        if self.disabled_by_watchdog {
            return;
        }
        let mut call_err: Option<String> = None;
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>(fn_name) {
                Self::arm_deadline(
                    &self.watchdog_origin,
                    &self.watchdog_deadline_us,
                    &self.watchdog_tripped,
                    JS_FRAME_BUDGET_MS,
                );
                let res = func.call::<_, ()>((arg,));
                self.watchdog_deadline_us.store(0, Ordering::Relaxed);
                if let Err(e) = res {
                    call_err = Some(e.to_string());
                }
            }
        });
        if let Some(err) = call_err {
            self.handle_js_error(fn_name, err);
        }
    }

    pub fn call_note_event(&mut self, fn_name: &str, lane: i32, is_player: bool) {
        if self.disabled_by_watchdog {
            return;
        }
        let mut call_err: Option<String> = None;
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>(fn_name) {
                Self::arm_deadline(
                    &self.watchdog_origin,
                    &self.watchdog_deadline_us,
                    &self.watchdog_tripped,
                    JS_FRAME_BUDGET_MS,
                );
                let res = func.call::<_, ()>((lane, is_player));
                self.watchdog_deadline_us.store(0, Ordering::Relaxed);
                if let Err(e) = res {
                    call_err = Some(e.to_string());
                }
            }
        });
        if let Some(err) = call_err {
            self.handle_js_error(fn_name, err);
        }
    }

    pub fn call_chart_event(&mut self, name: &str, val1: &str, val2: &str) {
        if self.disabled_by_watchdog {
            return;
        }
        let mut call_err: Option<String> = None;
        self.context.with(|ctx| {
            if let Ok(func) = ctx.globals().get::<_, Function>("onEvent") {
                Self::arm_deadline(
                    &self.watchdog_origin,
                    &self.watchdog_deadline_us,
                    &self.watchdog_tripped,
                    JS_FRAME_BUDGET_MS,
                );
                let res = func.call::<_, ()>((name, val1, val2));
                self.watchdog_deadline_us.store(0, Ordering::Relaxed);
                if let Err(e) = res {
                    call_err = Some(e.to_string());
                }
            }
        });
        if let Some(err) = call_err {
            self.handle_js_error("onEvent", err);
        }
    }
}

enum JsWorkerCmd {
    CallVoid {
        fn_name: &'static str,
        ack: std::sync::mpsc::SyncSender<()>,
    },
    CallF32 {
        fn_name: &'static str,
        arg: f32,
        ack: std::sync::mpsc::SyncSender<()>,
    },
    CallI32 {
        fn_name: &'static str,
        arg: i32,
        ack: std::sync::mpsc::SyncSender<()>,
    },
    CallStr {
        fn_name: &'static str,
        arg: String,
        ack: std::sync::mpsc::SyncSender<()>,
    },
    CallNoteEvent {
        fn_name: &'static str,
        lane: i32,
        is_player: bool,
        ack: std::sync::mpsc::SyncSender<()>,
    },
    CallChartEvent {
        name: String,
        val1: String,
        val2: String,
        ack: std::sync::mpsc::SyncSender<()>,
    },
}

/// Manejador `Send` que encapsula todas las instancias `JsScriptVm` (!Send) dentro de un único
/// hilo dedicado de QuickJS, evitando cruzar `JSRuntime`/`JSContext` entre hilos de Android JNI.
pub struct JsWorkerHandle {
    tx: Option<std::sync::mpsc::Sender<JsWorkerCmd>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl JsWorkerHandle {
    pub fn spawn(
        scripts: Vec<(String, String)>,
        host: Arc<ScriptHostState>,
    ) -> (Self, Vec<String>) {
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<JsWorkerCmd>();
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel::<Vec<String>>(1);

        let handle = std::thread::Builder::new()
            .name("pulse_quickjs_worker".to_string())
            .spawn(move || {
                let mut vms = Vec::new();
                let mut errors = Vec::new();

                for (name, js_source) in scripts {
                    match JsScriptVm::new_named(&name, &js_source, Arc::clone(&host)) {
                        Ok(vm) => vms.push(vm),
                        Err(e) => errors.push(format!("[{}] {}", name, e)),
                    }
                }

                let _ = init_tx.send(errors);

                while let Ok(cmd) = cmd_rx.recv() {
                    match cmd {
                        JsWorkerCmd::CallVoid { fn_name, ack } => {
                            for vm in &mut vms {
                                vm.call_void(fn_name);
                            }
                            let _ = ack.send(());
                        }
                        JsWorkerCmd::CallF32 { fn_name, arg, ack } => {
                            for vm in &mut vms {
                                vm.call_f32(fn_name, arg);
                            }
                            let _ = ack.send(());
                        }
                        JsWorkerCmd::CallI32 { fn_name, arg, ack } => {
                            for vm in &mut vms {
                                vm.call_i32(fn_name, arg);
                            }
                            let _ = ack.send(());
                        }
                        JsWorkerCmd::CallStr { fn_name, arg, ack } => {
                            for vm in &mut vms {
                                vm.call_str(fn_name, &arg);
                            }
                            let _ = ack.send(());
                        }
                        JsWorkerCmd::CallNoteEvent {
                            fn_name,
                            lane,
                            is_player,
                            ack,
                        } => {
                            for vm in &mut vms {
                                vm.call_note_event(fn_name, lane, is_player);
                            }
                            let _ = ack.send(());
                        }
                        JsWorkerCmd::CallChartEvent {
                            name,
                            val1,
                            val2,
                            ack,
                        } => {
                            for vm in &mut vms {
                                vm.call_chart_event(&name, &val1, &val2);
                            }
                            let _ = ack.send(());
                        }
                    }
                }
            });

        match handle {
            Ok(join_handle) => {
                let init_errors = init_rx.recv().unwrap_or_default();
                (
                    Self {
                        tx: Some(cmd_tx),
                        thread: Some(join_handle),
                    },
                    init_errors,
                )
            }
            Err(e) => (
                Self {
                    tx: None,
                    thread: None,
                },
                vec![format!("No se pudo iniciar hilo QuickJS: {}", e)],
            ),
        }
    }

    pub fn call_void(&self, fn_name: &'static str) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
            if tx.send(JsWorkerCmd::CallVoid { fn_name, ack: ack_tx }).is_ok() {
                let _ = ack_rx.recv();
            }
        }
    }

    pub fn call_f32(&self, fn_name: &'static str, arg: f32) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
            if tx
                .send(JsWorkerCmd::CallF32 {
                    fn_name,
                    arg,
                    ack: ack_tx,
                })
                .is_ok()
            {
                let _ = ack_rx.recv();
            }
        }
    }

    pub fn call_i32(&self, fn_name: &'static str, arg: i32) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
            if tx
                .send(JsWorkerCmd::CallI32 {
                    fn_name,
                    arg,
                    ack: ack_tx,
                })
                .is_ok()
            {
                let _ = ack_rx.recv();
            }
        }
    }

    pub fn call_str(&self, fn_name: &'static str, arg: String) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
            if tx
                .send(JsWorkerCmd::CallStr {
                    fn_name,
                    arg,
                    ack: ack_tx,
                })
                .is_ok()
            {
                let _ = ack_rx.recv();
            }
        }
    }

    pub fn call_note_event(&self, fn_name: &'static str, lane: i32, is_player: bool) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
            if tx
                .send(JsWorkerCmd::CallNoteEvent {
                    fn_name,
                    lane,
                    is_player,
                    ack: ack_tx,
                })
                .is_ok()
            {
                let _ = ack_rx.recv();
            }
        }
    }

    pub fn call_chart_event(&self, name: String, val1: String, val2: String) {
        if let Some(tx) = &self.tx {
            let (ack_tx, ack_rx) = std::sync::mpsc::sync_channel(1);
            if tx
                .send(JsWorkerCmd::CallChartEvent {
                    name,
                    val1,
                    val2,
                    ack: ack_tx,
                })
                .is_ok()
            {
                let _ = ack_rx.recv();
            }
        }
    }
}

impl Drop for JsWorkerHandle {
    fn drop(&mut self) {
        self.tx.take();
        if let Some(join) = self.thread.take() {
            let _ = join.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quickjs_watchdog_interrupts_infinite_loop_and_quarantines() {
        let js = r#"
            function onUpdate(dt) {
                while (true) {}
            }
        "#;
        let host = Arc::new(ScriptHostState::default());
        let mut vm = JsScriptVm::new_named("loop.js", js, Arc::clone(&host))
            .expect("carga inicial válida");

        for _ in 0..3 {
            vm.call_f32("onUpdate", 0.016);
        }
        assert!(vm.is_quarantined());
        let errs = host.drain_runtime_errors();
        assert!(errs.iter().any(|e| e.contains("cuarentena")));
    }
}
