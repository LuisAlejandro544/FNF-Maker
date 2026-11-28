//! Archivo: engine/crates/pulse_core/src/project/ui_blackout_lua.rs
//! Propósito: Genera y actualiza el script maestro `ui_blackout.lua`, encargado de la
//! cortina de pantalla en negro y la visibilidad/opacidad de HUD y flechas al inicio de la
//! canción (`chart.intro_ui`) y en eventos del chart.

use super::lua_helpers::{lua_escape_str, parse_kv_params, parse_optional_bool_flag};
use crate::chart::ChartData;

const UI_BLACKOUT_START_MARKER: &str = "-- [UI_BLACKOUT_START]";
const UI_BLACKOUT_END_MARKER: &str = "-- [UI_BLACKOUT_END]";
pub(super) const UI_BLACKOUT_RUNTIME_V2_MARKER: &str = "-- [UI_BLACKOUT_RUNTIME_V2]";

/// Normaliza un evento de apagón / visibilidad de HUD y flechas proveniente tanto del editor `#`
/// como de archivos `chart.json` externos (ej. `"Screen Fade"`, `"Camera Fade"`, `"Hide UI"`, `"Hide Arrows"`).
fn is_ui_blackout_event_name(lower_name: &str) -> bool {
    matches!(
        lower_name,
        "ui blackout"
            | "blackout"
            | "screen fade"
            | "camera fade"
            | "hide ui"
            | "show ui"
            | "toggle ui"
            | "fade ui"
            | "hud fade"
            | "hide hud"
            | "show hud"
            | "hide arrows"
            | "show arrows"
            | "toggle arrows"
            | "fade arrows"
            | "arrows fade"
            | "hide strums"
            | "show strums"
            | "ocultar flechas"
            | "mostrar flechas"
    )
}

/// Genera o actualiza el bloque `-- [UI_BLACKOUT_START] ... -- [UI_BLACKOUT_END]` dentro de
/// `ui_blackout.lua`. Permite ocultar la interfaz, las flechas y poner la pantalla en negro
/// de forma independiente o combinada al comenzar la canción (conservando visible `"2, 1, ¡YA!"`)
/// y ejecutar apagones o desvanecimientos en cualquier paso del chart.
pub fn render_ui_blackout_lua_script(chart: &ChartData, existing_source: Option<&str>) -> String {
    let mut sorted_events = chart.events.clone();
    sorted_events.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));

    let mut intro_black_enabled = chart.intro_ui.start_blackout;
    let mut intro_black_alpha = chart.intro_ui.blackout_alpha.clamp(0.0, 1.0) as f64;
    let mut intro_hide_hud = chart.intro_ui.hide_hud;
    let mut intro_hide_arrows = chart.intro_ui.hide_arrows;
    let mut intro_reveal_on_start = chart.intro_ui.reveal_on_song_start;
    let mut intro_reveal_delay_sec =
        (chart.intro_ui.reveal_delay_ms / 1000.0).clamp(0.0, 600.0);
    let mut intro_fade_sec = if chart.intro_ui.fade_sec > 0.01 {
        (chart.intro_ui.fade_sec as f64).clamp(0.0, 25.0)
    } else {
        0.65_f64
    };

    struct CompiledBlackoutCue {
        time_ms: f64,
        action: String,
        black: f64,
        hud: f64,
        arrows: f64,
        dur_sec: f64,
        ease: String,
    }
    let mut compiled_cues: Vec<CompiledBlackoutCue> = Vec::new();
    let mut pending_auto_ends: Vec<(f64, f64, String)> = Vec::new();

    for ev in &sorted_events {
        let lower_name = ev.name.trim().to_ascii_lowercase();
        if !is_ui_blackout_event_name(&lower_name) {
            continue;
        }

        let is_arrow_only_ev = matches!(
            lower_name.as_str(),
            "hide arrows"
                | "show arrows"
                | "toggle arrows"
                | "fade arrows"
                | "arrows fade"
                | "hide strums"
                | "show strums"
                | "ocultar flechas"
                | "mostrar flechas"
        );
        let is_hud_only_ev = matches!(lower_name.as_str(), "hide hud" | "show hud");
        let is_ui_both_ev = matches!(
            lower_name.as_str(),
            "hide ui" | "show ui" | "toggle ui" | "fade ui" | "hud fade"
        );

        let v1_lower = ev.value1.trim().to_ascii_lowercase();
        let mut action = if matches!(
            lower_name.as_str(),
            "hide ui" | "hide hud" | "hide arrows" | "hide strums" | "ocultar flechas"
        ) {
            "hide".to_string()
        } else if matches!(
            lower_name.as_str(),
            "show ui" | "show hud" | "show arrows" | "show strums" | "mostrar flechas"
        ) {
            "show".to_string()
        } else if matches!(lower_name.as_str(), "toggle ui" | "toggle arrows") {
            "toggle".to_string()
        } else if matches!(
            v1_lower.as_str(),
            "intro" | "show" | "reveal" | "hide" | "toggle" | "fade"
        ) {
            if v1_lower == "reveal" {
                "show".to_string()
            } else {
                v1_lower.clone()
            }
        } else {
            "fade".to_string()
        };

        let mut black_alpha = match action.as_str() {
            "intro" | "hide" => {
                if is_arrow_only_ev || is_hud_only_ev || is_ui_both_ev {
                    -1.0
                } else {
                    1.0
                }
            }
            "show" => {
                if is_arrow_only_ev || is_hud_only_ev {
                    -1.0
                } else {
                    0.0
                }
            }
            _ => -1.0,
        };
        let mut hud_alpha = match action.as_str() {
            "intro" | "hide" => {
                if is_arrow_only_ev {
                    -1.0
                } else {
                    0.0
                }
            }
            "show" => {
                if is_arrow_only_ev {
                    -1.0
                } else {
                    1.0
                }
            }
            _ => -1.0,
        };
        let mut arrows_alpha = match action.as_str() {
            "intro" | "hide" => {
                if is_hud_only_ev {
                    -1.0
                } else {
                    0.0
                }
            }
            "show" => {
                if is_hud_only_ev {
                    -1.0
                } else {
                    1.0
                }
            }
            _ => -1.0,
        };
        let mut dur_sec = if action == "hide" && ev.time_ms <= 120.0 {
            0.0_f64
        } else {
            0.60_f64
        };
        let mut reveal_ms = 0.0_f64;
        let mut end_ms: Option<f64> = None;
        let mut end_dur_sec: Option<f64> = None;
        let mut explicit_on_start: Option<bool> = None;
        let mut ease = "sineInOut".to_string();

        // Parseo flexible estilo "Screen Fade" / "Camera Fade" externo (CSV o número en value1/value2)
        if matches!(lower_name.as_str(), "screen fade" | "camera fade") {
            let v1_parts: Vec<&str> = ev.value1.split(',').map(|s| s.trim()).collect();
            if v1_parts.len() == 1 {
                if let Ok(a) = v1_parts[0].parse::<f64>() {
                    if a.is_finite() {
                        black_alpha = a.clamp(0.0, 1.0);
                    }
                }
            } else if v1_parts.len() >= 2 {
                if let Ok(a) = v1_parts.last().unwrap_or(&"").parse::<f64>() {
                    if a.is_finite() && a <= 1.0 {
                        black_alpha = a.clamp(0.0, 1.0);
                    }
                }
            }
            if black_alpha < 0.0 {
                black_alpha = 1.0;
            }
            if let Ok(d) = ev.value2.trim().parse::<f64>() {
                if d.is_finite() && d >= 0.0 {
                    dur_sec = d.clamp(0.0, 20.0);
                }
            }
        } else if matches!(lower_name.as_str(), "fade ui" | "hud fade") {
            if let Ok(a) = ev.value1.trim().parse::<f64>() {
                if a.is_finite() {
                    let clamped = a.clamp(0.0, 1.0);
                    hud_alpha = clamped;
                    arrows_alpha = clamped;
                }
            }
            if let Ok(d) = ev.value2.trim().parse::<f64>() {
                if d.is_finite() && d >= 0.0 {
                    dur_sec = d.clamp(0.0, 20.0);
                }
            }
        } else if matches!(lower_name.as_str(), "fade arrows" | "arrows fade") {
            if let Ok(a) = ev.value1.trim().parse::<f64>() {
                if a.is_finite() {
                    arrows_alpha = a.clamp(0.0, 1.0);
                }
            }
            if let Ok(d) = ev.value2.trim().parse::<f64>() {
                if d.is_finite() && d >= 0.0 {
                    dur_sec = d.clamp(0.0, 20.0);
                }
            }
        } else if is_arrow_only_ev || is_hud_only_ev || matches!(lower_name.as_str(), "hide ui" | "show ui" | "toggle ui") {
            if let Ok(d) = ev.value2.trim().parse::<f64>() {
                if d.is_finite() && d >= 0.0 {
                    dur_sec = d.clamp(0.0, 20.0);
                }
            }
        }

        // Parseo KV estructurado BeatForge (tanto en value1 como en value2)
        for kv_source in [&ev.value1, &ev.value2] {
            for (k, v) in parse_kv_params(kv_source) {
                match k.as_str() {
                    "action" | "mode" => {
                        let m = v.to_ascii_lowercase();
                        action = if m == "reveal" { "show".to_string() } else { m };
                    }
                    "black" | "blackout" => {
                        if let Ok(b) = v.parse::<f64>() {
                            if b.is_finite() {
                                black_alpha = if b < 0.0 { -1.0 } else { b.clamp(0.0, 1.0) };
                            }
                        } else if let Some(flag) = parse_optional_bool_flag(&v) {
                            black_alpha = if flag { 1.0 } else { 0.0 };
                        }
                    }
                    "hud" => {
                        if let Ok(h) = v.parse::<f64>() {
                            if h.is_finite() {
                                hud_alpha = if h < 0.0 { -1.0 } else { h.clamp(0.0, 1.0) };
                            }
                        } else if let Some(flag) = parse_optional_bool_flag(&v) {
                            hud_alpha = if flag { 1.0 } else { 0.0 };
                        }
                    }
                    "arrows" | "strums" | "flechas" => {
                        if let Ok(ar) = v.parse::<f64>() {
                            if ar.is_finite() {
                                arrows_alpha = if ar < 0.0 { -1.0 } else { ar.clamp(0.0, 1.0) };
                            }
                        } else if let Some(flag) = parse_optional_bool_flag(&v) {
                            arrows_alpha = if flag { 1.0 } else { 0.0 };
                        }
                    }
                    "dur" | "duration" => {
                        if let Ok(d) = v.parse::<f64>() {
                            if d.is_finite() && d >= 0.0 {
                                dur_sec = d.clamp(0.0, 25.0);
                            }
                        }
                    }
                    "enddur" | "end_dur" | "fadeout" | "outdur" => {
                        if let Ok(d) = v.parse::<f64>() {
                            if d.is_finite() && d >= 0.0 {
                                end_dur_sec = Some(d.clamp(0.0, 25.0));
                            }
                        }
                    }
                    "endms" | "end_ms" => {
                        if let Ok(em) = v.parse::<f64>() {
                            if em.is_finite() && em > 0.0 {
                                end_ms = Some(em.clamp(0.0, 600_000.0));
                            }
                        }
                    }
                    "revealms" | "reveal_ms" | "delayms" => {
                        if let Ok(rm) = v.parse::<f64>() {
                            if rm.is_finite() && rm >= 0.0 {
                                reveal_ms = rm.clamp(0.0, 600_000.0);
                            }
                        }
                    }
                    "onstart" | "on_start" => {
                        if let Some(flag) = parse_optional_bool_flag(&v) {
                            explicit_on_start = Some(flag);
                        }
                    }
                    "ease" => {
                        if !v.trim().is_empty() {
                            ease = v.trim().to_string();
                        }
                    }
                    _ => {}
                }
            }
        }

        let hides_at_start = ev.time_ms <= 120.0
            && (action == "hide"
                || (action == "fade"
                    && (black_alpha >= 0.85
                        || (hud_alpha >= 0.0 && hud_alpha <= 0.15)
                        || (arrows_alpha >= 0.0 && arrows_alpha <= 0.15))));
        let is_intro_cue = action == "intro" || hides_at_start;

        if let Some(em) = end_ms {
            if em > ev.time_ms.max(0.0) + 5.0 {
                pending_auto_ends.push((em, end_dur_sec.unwrap_or(dur_sec), ease.clone()));
            }
        }

        if is_intro_cue {
            if black_alpha > 0.01 {
                intro_black_enabled = true;
                intro_black_alpha = black_alpha.clamp(0.0, 1.0);
            }
            if hud_alpha >= 0.0 && hud_alpha <= 0.05 {
                intro_hide_hud = true;
            }
            if arrows_alpha >= 0.0 && arrows_alpha <= 0.05 {
                intro_hide_arrows = true;
            }
            if let Some(flag) = explicit_on_start {
                intro_reveal_on_start = flag || reveal_ms > 0.0;
            } else if end_ms.is_some() {
                intro_reveal_on_start = false;
            } else if reveal_ms > 0.0 || action == "intro" {
                intro_reveal_on_start = true;
            } else if !chart.intro_ui.is_active() {
                intro_reveal_on_start = false;
            }
            if reveal_ms > 0.0 {
                intro_reveal_delay_sec = (reveal_ms / 1000.0).clamp(0.0, 600.0);
            }
            if dur_sec > 0.01 {
                intro_fade_sec = dur_sec;
            }
            continue;
        }

        compiled_cues.push(CompiledBlackoutCue {
            time_ms: ev.time_ms.max(0.0),
            action,
            black: black_alpha,
            hud: hud_alpha,
            arrows: arrows_alpha,
            dur_sec,
            ease,
        });
    }

    for (end_ms, end_dur, end_ease) in pending_auto_ends {
        let already_has_show = compiled_cues
            .iter()
            .any(|c| c.action == "show" && (c.time_ms - end_ms).abs() <= 50.0);
        if !already_has_show {
            compiled_cues.push(CompiledBlackoutCue {
                time_ms: end_ms,
                action: "show".to_string(),
                black: 0.0,
                hud: 1.0,
                arrows: 1.0,
                dur_sec: end_dur,
                ease: end_ease,
            });
        }
    }
    compiled_cues.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));

    let cue_lines: Vec<String> = compiled_cues
        .iter()
        .map(|c| {
            format!(
                "    {{ timeMs = {:.2}, action = {}, black = {:.2}, hud = {:.2}, arrows = {:.2}, durSec = {:.3}, ease = {} }},",
                c.time_ms,
                lua_escape_str(&c.action),
                c.black,
                c.hud,
                c.arrows,
                c.dur_sec,
                lua_escape_str(&c.ease),
            )
        })
        .collect();

    let cfg_and_cues_block = if cue_lines.is_empty() {
        format!(
            "{}\n\
local INTRO_BLACK_ENABLED = {}\n\
local INTRO_BLACKOUT_ALPHA = {:.2}\n\
local INTRO_HIDE_HUD = {}\n\
local INTRO_HIDE_ARROWS = {}\n\
local INTRO_REVEAL_AT_SONG_START = {}\n\
local INTRO_REVEAL_DELAY_SEC = {:.3}\n\
local INTRO_FADE_SEC = {:.3}\n\
local UI_BLACKOUT_CUES = {{}}\n\
{}",
            UI_BLACKOUT_START_MARKER,
            if intro_black_enabled { "true" } else { "false" },
            intro_black_alpha,
            if intro_hide_hud { "true" } else { "false" },
            if intro_hide_arrows { "true" } else { "false" },
            if intro_reveal_on_start { "true" } else { "false" },
            intro_reveal_delay_sec,
            intro_fade_sec,
            UI_BLACKOUT_END_MARKER
        )
    } else {
        format!(
            "{}\n\
local INTRO_BLACK_ENABLED = {}\n\
local INTRO_BLACKOUT_ALPHA = {:.2}\n\
local INTRO_HIDE_HUD = {}\n\
local INTRO_HIDE_ARROWS = {}\n\
local INTRO_REVEAL_AT_SONG_START = {}\n\
local INTRO_REVEAL_DELAY_SEC = {:.3}\n\
local INTRO_FADE_SEC = {:.3}\n\
local UI_BLACKOUT_CUES = {{\n{}\n}}\n\
{}",
            UI_BLACKOUT_START_MARKER,
            if intro_black_enabled { "true" } else { "false" },
            intro_black_alpha,
            if intro_hide_hud { "true" } else { "false" },
            if intro_hide_arrows { "true" } else { "false" },
            if intro_reveal_on_start { "true" } else { "false" },
            intro_reveal_delay_sec,
            intro_fade_sec,
            cue_lines.join("\n"),
            UI_BLACKOUT_END_MARKER
        )
    };

    if let Some(src) = existing_source {
        if src.contains(UI_BLACKOUT_RUNTIME_V2_MARKER) {
            if let (Some(start_idx), Some(end_idx)) = (
                src.find(UI_BLACKOUT_START_MARKER),
                src.find(UI_BLACKOUT_END_MARKER),
            ) {
                if end_idx >= start_idx {
                    let tail_start = end_idx + UI_BLACKOUT_END_MARKER.len();
                    let prefix = &src[..start_idx];
                    let suffix = &src[tail_start..];
                    return format!("{}{}{}", prefix, cfg_and_cues_block, suffix);
                }
            }
        }
    }

    format!(
        "-- Script maestro de Pantalla en Negro y Visibilidad de UI (BeatForge Studio)\n\
{}\n\
-- Controla intros con pantalla negra y UI/flechas ocultas conservando visible el '2, 1, ¡YA!'\n\
-- y apagones/desvanecimientos sincronizados con chart.json (uiBlackout) y eventos '#'.\n\
{}\n\n\
local cueCursor = 1\n\
local lastSongPosMs = -1.0\n\
local uiHiddenToggle = false\n\
local arrowsHiddenToggle = false\n\n\
local function applyChannelTransition(blackTarget, hudTarget, arrowsTarget, durSec, easeName)\n\
    local ease = easeName or \"sineInOut\"\n\
    local d = math.max(0.0, durSec or 0.0)\n\
    if blackTarget >= 0.0 then\n\
        if d <= 0.01 then\n\
            setScreenBlackout(blackTarget)\n\
        else\n\
            tweenScreenBlackout(\"ui_blackout_curtain\", blackTarget, d, ease)\n\
        end\n\
    end\n\
    if hudTarget >= 0.0 then\n\
        if hudTarget > 0.01 then\n\
            setHudVisible(true)\n\
            if d <= 0.01 then\n\
                setHudOpacity(hudTarget)\n\
            else\n\
                tweenHudOpacity(\"ui_blackout_hud\", hudTarget, d, ease)\n\
            end\n\
        else\n\
            if d <= 0.01 then\n\
                setHudOpacity(0.0)\n\
                setHudVisible(false)\n\
            else\n\
                tweenHudOpacity(\"ui_blackout_hud\", 0.0, d, ease)\n\
            end\n\
        end\n\
    end\n\
    if arrowsTarget >= 0.0 then\n\
        if arrowsTarget > 0.01 then\n\
            setArrowsVisible(true)\n\
            if d <= 0.01 then\n\
                setArrowsOpacity(arrowsTarget)\n\
            else\n\
                tweenArrowsOpacity(\"ui_blackout_arrows\", arrowsTarget, d, ease)\n\
            end\n\
        else\n\
            if d <= 0.01 then\n\
                setArrowsOpacity(0.0)\n\
                setArrowsVisible(false)\n\
            else\n\
                tweenArrowsOpacity(\"ui_blackout_arrows\", 0.0, d, ease)\n\
            end\n\
        end\n\
    end\n\
end\n\n\
local function revealFromIntro()\n\
    local b = INTRO_BLACK_ENABLED and 0.0 or -1.0\n\
    local h = INTRO_HIDE_HUD and 1.0 or -1.0\n\
    local a = INTRO_HIDE_ARROWS and 1.0 or -1.0\n\
    applyChannelTransition(b, h, a, INTRO_FADE_SEC, \"sineInOut\")\n\
    if INTRO_HIDE_HUD then uiHiddenToggle = false end\n\
    if INTRO_HIDE_ARROWS then arrowsHiddenToggle = false end\n\
end\n\n\
local function hasTimelineCueNear(songPos)\n\
    for i = 1, #UI_BLACKOUT_CUES do\n\
        if math.abs(UI_BLACKOUT_CUES[i].timeMs - songPos) <= 45.0 then\n\
            return true\n\
        end\n\
    end\n\
    return false\n\
end\n\n\
local function executeUiCue(cue)\n\
    if cue.action == \"toggle\" then\n\
        if cue.arrows >= 0.0 and cue.hud < 0.0 then\n\
            arrowsHiddenToggle = not arrowsHiddenToggle\n\
            local nextArrows = arrowsHiddenToggle and 0.0 or 1.0\n\
            applyChannelTransition(-1.0, -1.0, nextArrows, cue.durSec, cue.ease)\n\
        else\n\
            uiHiddenToggle = not uiHiddenToggle\n\
            local nextHud = uiHiddenToggle and 0.0 or 1.0\n\
            local nextArrows = (cue.arrows >= 0.0) and nextHud or -1.0\n\
            if nextArrows >= 0.0 then arrowsHiddenToggle = uiHiddenToggle end\n\
            local nextBlack = (cue.black >= 0.0) and (uiHiddenToggle and cue.black or 0.0) or -1.0\n\
            applyChannelTransition(nextBlack, nextHud, nextArrows, cue.durSec, cue.ease)\n\
        end\n\
    else\n\
        if cue.hud >= 0.0 then\n\
            uiHiddenToggle = cue.hud <= 0.05\n\
        end\n\
        if cue.arrows >= 0.0 then\n\
            arrowsHiddenToggle = cue.arrows <= 0.05\n\
        end\n\
        applyChannelTransition(cue.black, cue.hud, cue.arrows, cue.durSec, cue.ease)\n\
    end\n\
end\n\n\
function onCreate()\n\
    cueCursor = 1\n\
    lastSongPosMs = -1.0\n\
    if INTRO_BLACK_ENABLED then\n\
        setScreenBlackout(INTRO_BLACKOUT_ALPHA)\n\
    else\n\
        setScreenBlackout(0.0)\n\
    end\n\
    if INTRO_HIDE_HUD then\n\
        setHudOpacity(0.0)\n\
        setHudVisible(false)\n\
    else\n\
        setHudVisible(true)\n\
        setHudOpacity(1.0)\n\
    end\n\
    if INTRO_HIDE_ARROWS then\n\
        setArrowsOpacity(0.0)\n\
        setArrowsVisible(false)\n\
    else\n\
        setArrowsVisible(true)\n\
        setArrowsOpacity(1.0)\n\
    end\n\
    uiHiddenToggle = INTRO_HIDE_HUD\n\
    arrowsHiddenToggle = INTRO_HIDE_ARROWS\n\
end\n\n\
function onSongStart()\n\
    if (INTRO_BLACK_ENABLED or INTRO_HIDE_HUD or INTRO_HIDE_ARROWS) and INTRO_REVEAL_AT_SONG_START then\n\
        if INTRO_REVEAL_DELAY_SEC > 0.02 then\n\
            runTimer(\"ui_blackout_intro_reveal\", INTRO_REVEAL_DELAY_SEC)\n\
        else\n\
            revealFromIntro()\n\
        end\n\
    end\n\
end\n\n\
function onTimerCompleted(tag)\n\
    if tag == \"ui_blackout_intro_reveal\" then\n\
        revealFromIntro()\n\
    end\n\
end\n\n\
function onUpdate(dt)\n\
    local songPos = getSongPosMs()\n\
    if songPos + 250.0 < lastSongPosMs then\n\
        cueCursor = 1\n\
    end\n\
    lastSongPosMs = songPos\n\
    if songPos < 0.0 then\n\
        return\n\
    end\n\
    while cueCursor <= #UI_BLACKOUT_CUES and UI_BLACKOUT_CUES[cueCursor].timeMs <= songPos do\n\
        executeUiCue(UI_BLACKOUT_CUES[cueCursor])\n\
        cueCursor = cueCursor + 1\n\
    end\n\
end\n\n\
function onEvent(name, value1, value2)\n\
    local songPos = getSongPosMs()\n\
    if songPos <= 120.0 or hasTimelineCueNear(songPos) then\n\
        return\n\
    end\n\
    local lower = string.lower(tostring(name or \"\"))\n\
    local v1 = tostring(value1 or \"\")\n\
    local v2 = tostring(value2 or \"\")\n\
    if lower == \"hide ui\" or lower == \"hide hud\" then\n\
        local dur = tonumber(v2) or 0.35\n\
        uiHiddenToggle = true\n\
        if lower == \"hide ui\" then arrowsHiddenToggle = true end\n\
        applyChannelTransition(-1.0, 0.0, lower == \"hide ui\" and 0.0 or -1.0, dur, \"sineInOut\")\n\
    elseif lower == \"show ui\" or lower == \"show hud\" then\n\
        local dur = tonumber(v2) or 0.35\n\
        uiHiddenToggle = false\n\
        if lower == \"show ui\" then arrowsHiddenToggle = false end\n\
        applyChannelTransition(0.0, 1.0, lower == \"show ui\" and 1.0 or -1.0, dur, \"sineInOut\")\n\
    elseif lower == \"hide arrows\" or lower == \"hide strums\" or lower == \"ocultar flechas\" then\n\
        local dur = tonumber(v2) or 0.35\n\
        arrowsHiddenToggle = true\n\
        applyChannelTransition(-1.0, -1.0, 0.0, dur, \"sineInOut\")\n\
    elseif lower == \"show arrows\" or lower == \"show strums\" or lower == \"mostrar flechas\" then\n\
        local dur = tonumber(v2) or 0.35\n\
        arrowsHiddenToggle = false\n\
        applyChannelTransition(-1.0, -1.0, 1.0, dur, \"sineInOut\")\n\
    elseif lower == \"toggle arrows\" then\n\
        arrowsHiddenToggle = not arrowsHiddenToggle\n\
        local target = arrowsHiddenToggle and 0.0 or 1.0\n\
        local dur = tonumber(v2) or 0.35\n\
        applyChannelTransition(-1.0, -1.0, target, dur, \"sineInOut\")\n\
    elseif lower == \"fade arrows\" or lower == \"arrows fade\" then\n\
        local target = tonumber(v1) or 0.0\n\
        local dur = tonumber(v2) or 0.5\n\
        arrowsHiddenToggle = target <= 0.05\n\
        applyChannelTransition(-1.0, -1.0, target, dur, \"sineInOut\")\n\
    elseif lower == \"toggle ui\" then\n\
        uiHiddenToggle = not uiHiddenToggle\n\
        arrowsHiddenToggle = uiHiddenToggle\n\
        local target = uiHiddenToggle and 0.0 or 1.0\n\
        local dur = tonumber(v2) or 0.35\n\
        applyChannelTransition(-1.0, target, target, dur, \"sineInOut\")\n\
    elseif lower == \"fade ui\" or lower == \"hud fade\" then\n\
        local target = tonumber(v1) or 0.0\n\
        local dur = tonumber(v2) or 0.5\n\
        uiHiddenToggle = target <= 0.05\n\
        arrowsHiddenToggle = target <= 0.05\n\
        applyChannelTransition(-1.0, target, target, dur, \"sineInOut\")\n\
    elseif lower == \"screen fade\" or lower == \"camera fade\" or lower == \"blackout\" then\n\
        local target = tonumber(v1) or 1.0\n\
        local dur = tonumber(v2) or 0.5\n\
        applyChannelTransition(target, -1.0, -1.0, dur, \"sineInOut\")\n\
    end\n\
end\n",
        UI_BLACKOUT_RUNTIME_V2_MARKER,
        cfg_and_cues_block
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_intro_blackout_range_and_auto_synthesizes_mid_song_end_cue() {
        let mut chart = ChartData::bundled_demo_chart();
        chart.add_event(
            0.0,
            "UI Blackout",
            "intro",
            "black=1.00;hud=0.00;arrows=0.00;dur=0.50;onStart=0;endMs=12000;fadeOut=0.75",
        );
        chart.add_event(
            18_000.0,
            "UI Blackout",
            "hide",
            "black=0.90;hud=0.00;arrows=1.00;dur=0.40;endMs=24000;fadeOut=0.60",
        );

        let lua_src = render_ui_blackout_lua_script(&chart, None);
        assert!(lua_src.contains("local INTRO_BLACK_ENABLED = true"));
        assert!(lua_src.contains("local INTRO_REVEAL_AT_SONG_START = false"));
        assert!(lua_src.contains("timeMs = 12000.00, action = \"show\", black = 0.00"));
        assert!(lua_src.contains("timeMs = 18000.00, action = \"hide\", black = 0.90"));
        assert!(lua_src.contains("timeMs = 24000.00, action = \"show\", black = 0.00"));
    }
}
