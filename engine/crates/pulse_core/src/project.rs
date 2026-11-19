//! Archivo: engine/crates/pulse_core/src/project.rs
//! Propósito: Modela un proyecto completo de mod dentro de BeatForge Studio.
//! Controla:
//! - Chart activo
//! - Hasta máximo 3 pistas de audio simultáneas (Mix completo/Instrumental, Voces Player, Voces Rival)
//! - Escenario con todos los assets importados e identificados
//! - Scripts comunitarios en Lua, TypeScript o JavaScript
//! - Skin de flechas SVG seleccionada y opciones de gameplay (downscroll, offset Oboe)

use crate::chart::ChartData;
use crate::gfx_backend::RenderBackendKind;
use crate::stage::{CameraFollowMode, StageCameraConfig, StageScene};
use crate::svg_skin::ArrowSkinSet;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Máximo absoluto de pistas de música permitidas por proyecto según la especificación.
pub const MAX_AUDIO_TRACKS: usize = 3;

/// Identificador estable del script Lua que controla el sistema de cámara del escenario.
pub const CAMERA_SCRIPT_ID: &str = "script_camera_system";
/// Nombre visible del archivo Lua de cámara editable por el usuario en Script Studio.
pub const CAMERA_SCRIPT_NAME: &str = "camera_system.lua";

/// Identificador estable del script Lua que orquesta flashes, swaps de visibilidad, escenarios y personajes.
pub const STAGE_EVENTS_SCRIPT_ID: &str = "script_stage_events";
/// Nombre visible del archivo Lua de eventos de escenario y timeline en Script Studio.
pub const STAGE_EVENTS_SCRIPT_NAME: &str = "stage_events.lua";

/// Identificador estable del script Lua que controla pantalla en negro, visibilidad de HUD y flechas al iniciar o por eventos del chart.
pub const UI_BLACKOUT_SCRIPT_ID: &str = "script_ui_blackout";
/// Nombre visible del archivo Lua de pantalla en negro / visibilidad de UI en Script Studio.
pub const UI_BLACKOUT_SCRIPT_NAME: &str = "ui_blackout.lua";

const CAM_CFG_START_MARKER: &str = "-- [CAM_CFG_START]";
const CAM_CFG_END_MARKER: &str = "-- [CAM_CFG_END]";
const STAGE_CUES_START_MARKER: &str = "-- [STAGE_CUES_START]";
const STAGE_CUES_END_MARKER: &str = "-- [STAGE_CUES_END]";
const UI_BLACKOUT_START_MARKER: &str = "-- [UI_BLACKOUT_START]";
const UI_BLACKOUT_END_MARKER: &str = "-- [UI_BLACKOUT_END]";

const CAM_RUNTIME_V2_MARKER: &str = "-- [CAM_RUNTIME_V2]";

/// Genera o actualiza el bloque de parámetros del script `camera_system.lua` preservando
/// cualquier lógica personalizada que el usuario haya añadido debajo de los marcadores.
pub fn render_camera_lua_script(cfg: &StageCameraConfig, existing_source: Option<&str>) -> String {
    let mut normalized = cfg.clone();
    normalized.ensure_default_cameras();

    let slots_summary: Vec<String> = normalized
        .cameras
        .iter()
        .map(|s| {
            format!(
                "--   Cam {}: {} (target={}, x={:.0}, y={:.0}, zoom={:.2})",
                s.index,
                s.name,
                s.target_kind.as_key(),
                s.x,
                s.y,
                s.zoom
            )
        })
        .collect();

    let config_block = format!(
        "{}\n\
local CAM_ENABLED = {}\n\
local CAM_BASE_X = {:.1}\n\
local CAM_BASE_Y = {:.1}\n\
local CAM_BASE_ZOOM = {:.3}\n\
local CAM_MODE = \"{}\" -- \"fixed\" = Cam 0 fija | \"sections\" = chart.json + eventos | \"beats\" = cada N beats\n\
local CAM_INTERVAL_BEATS = {}\n\
local CAM_SMOOTH = {:.2}\n\
local CAM_PAN_STRENGTH = {:.2}\n\
local CAM_ACTIVE_INDEX = {}\n\
local CAM_COUNT = {}\n\
{}\n\
{}",
        CAM_CFG_START_MARKER,
        if normalized.enabled { "true" } else { "false" },
        normalized.base_x,
        normalized.base_y,
        normalized.base_zoom,
        normalized.follow_mode.as_lua_str(),
        normalized.follow_interval_beats.max(1),
        normalized.smooth_speed,
        normalized.pan_strength,
        normalized.active_camera_index,
        normalized.cameras.len(),
        slots_summary.join("\n"),
        CAM_CFG_END_MARKER
    );

    if let Some(src) = existing_source {
        let is_unmodified_v1 = !src.contains(CAM_RUNTIME_V2_MARKER)
            && src.contains("local roleId = isPlayerTurn and \"player\" or \"opponent\"");
        if !is_unmodified_v1 {
            if let (Some(start_idx), Some(end_idx)) =
                (src.find(CAM_CFG_START_MARKER), src.find(CAM_CFG_END_MARKER))
            {
                if end_idx >= start_idx {
                    let tail_start = end_idx + CAM_CFG_END_MARKER.len();
                    let prefix = &src[..start_idx];
                    let suffix = &src[tail_start..];
                    return format!("{}{}{}", prefix, config_block, suffix);
                }
            }
        }
    }

    format!(
        "-- Script real del Sistema Multicámara (BeatForge Studio)\n\
{}\n\
-- El motor nativo gestiona Cam 0 (Fija inicial), Cam 1 (Jugador), Cam 2 (Enemigo), Cam 3+\n\
-- y todos los eventos de chart.json (FocusCamera, ZoomCamera, Camera Follow Pos, Switch Camera).\n\
-- Cambia CAM_SCRIPT_OVERRIDE a true solo si deseas controlar la cámara manualmente desde Lua:\n\
{}\n\n\
local CAM_SCRIPT_OVERRIDE = false\n\n\
function onCreate()\n\
    if not CAM_ENABLED then\n\
        setCameraPos(CAM_BASE_X, CAM_BASE_Y)\n\
        setCameraBaseZoom(CAM_BASE_ZOOM)\n\
        setCameraZoom(CAM_BASE_ZOOM)\n\
        return\n\
    end\n\
    -- Arranca en Cámara 0 con su FOV/zoom configurado hasta recibir acciones de otras cámaras.\n\
    switchCamera(0)\n\
end\n\n\
function onBeatHit(beat)\n\
    if not CAM_ENABLED or not CAM_SCRIPT_OVERRIDE then return end\n\
    if CAM_MODE == \"sections\" then\n\
        switchCamera(getMustHitSection() and 1 or 2)\n\
    elseif CAM_MODE == \"beats\" then\n\
        local step = math.max(1, CAM_INTERVAL_BEATS)\n\
        switchCamera(((math.floor(beat / step) % 2) == 1) and 1 or 2)\n\
    end\n\
end\n",
        CAM_RUNTIME_V2_MARKER,
        config_block
    )
}

/// Extrae los parámetros de cámara editados manualmente dentro de `camera_system.lua`
/// para mantener sincronizado el rectángulo visual del escenario y el inspector.
pub fn parse_camera_config_from_lua(source: &str, current: &StageCameraConfig) -> StageCameraConfig {
    let mut next = current.clone();
    next.ensure_default_cameras();
    for line in source.lines() {
        let trimmed = line.trim();
        let no_comment = trimmed.split("--").next().unwrap_or("").trim();
        let Some((lhs, rhs)) = no_comment.split_once('=') else {
            continue;
        };
        let key = lhs.trim().trim_start_matches("local").trim();
        let val = rhs.trim();
        match key {
            "CAM_ENABLED" => {
                if val.eq_ignore_ascii_case("true") {
                    next.enabled = true;
                } else if val.eq_ignore_ascii_case("false") {
                    next.enabled = false;
                }
            }
            "CAM_BASE_X" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        next.base_x = v.clamp(-1000.0, 2600.0);
                    }
                }
            }
            "CAM_BASE_Y" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        next.base_y = v.clamp(-1000.0, 1800.0);
                    }
                }
            }
            "CAM_BASE_ZOOM" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        next.base_zoom = v.clamp(0.35, 2.5);
                    }
                }
            }
            "CAM_MODE" => {
                let clean = val.trim_matches('"').trim_matches('\'');
                next.follow_mode = CameraFollowMode::from_str_loose(clean);
            }
            "CAM_INTERVAL_BEATS" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        next.follow_interval_beats = (v.round() as i32).clamp(1, 32) as u32;
                    }
                }
            }
            "CAM_SMOOTH" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        next.smooth_speed = v.clamp(1.0, 20.0);
                    }
                }
            }
            "CAM_PAN_STRENGTH" => {
                if let Ok(v) = val.parse::<f32>() {
                    if v.is_finite() {
                        next.pan_strength = v.clamp(0.0, 1.0);
                    }
                }
            }
            "CAM_ACTIVE_INDEX" => {
                if let Ok(v) = val.parse::<i32>() {
                    let max_idx = next.cameras.len().saturating_sub(1);
                    next.active_camera_index = (v.max(0) as usize).min(max_idx);
                }
            }
            _ => {}
        }
    }
    next.sync_slot0_from_base();
    next
}

fn lua_escape_str(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 2);
    out.push('"');
    for ch in raw.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

fn lua_str_array(items: &[String]) -> String {
    if items.is_empty() {
        return "{}".to_string();
    }
    let joined: Vec<String> = items.iter().map(|s| lua_escape_str(s)).collect();
    format!("{{ {} }}", joined.join(", "))
}

fn split_csv_ids(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn parse_kv_params(raw: &str) -> Vec<(String, String)> {
    raw.split(';')
        .filter_map(|part| {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                return None;
            }
            trimmed
                .split_once('=')
                .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        })
        .collect()
}

/// Genera o actualiza el bloque `-- [STAGE_CUES_START] ... -- [STAGE_CUES_END]` dentro de
/// `stage_events.lua` a partir de los eventos del chart, preservando cualquier lógica Lua
/// personalizada añadida por el creador debajo de los marcadores.
pub fn render_stage_events_lua_script(
    chart: &ChartData,
    _stage: &StageScene,
    existing_source: Option<&str>,
) -> String {
    let mut sorted_events = chart.events.clone();
    sorted_events.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));

    let mut cue_lines = Vec::new();
    for ev in &sorted_events {
        let lower_name = ev.name.trim().to_ascii_lowercase();
        if lower_name == "stage swap" || lower_name == "swap stage" || lower_name == "change character"
        {
            let mut show_ids: Vec<String> = Vec::new();
            let mut hide_ids: Vec<String> = Vec::new();
            let mut dispose_old = false;
            let mut player_id = String::new();
            let mut opp_id = String::new();

            let p1 = parse_kv_params(&ev.value1);
            if p1.is_empty() && !ev.value1.trim().is_empty() {
                show_ids = split_csv_ids(&ev.value1);
            } else {
                for (k, v) in p1 {
                    match k.as_str() {
                        "show" | "mostrar" => show_ids.extend(split_csv_ids(&v)),
                        "hide" | "ocultar" => hide_ids.extend(split_csv_ids(&v)),
                        "dispose" | "reciclar" => {
                            dispose_old = matches!(v.as_str(), "1" | "true" | "si" | "yes");
                        }
                        "player" | "jugador" => player_id = v,
                        "opp" | "opponent" | "enemigo" | "rival" => opp_id = v,
                        _ => {}
                    }
                }
            }

            let p2 = parse_kv_params(&ev.value2);
            if p2.is_empty() && !ev.value2.trim().is_empty() {
                // Compatibilidad si value2 trae directo un flag o ID de jugador
                if matches!(ev.value2.trim(), "1" | "true") {
                    dispose_old = true;
                }
            } else {
                for (k, v) in p2 {
                    match k.as_str() {
                        "player" | "jugador" => player_id = v,
                        "opp" | "opponent" | "enemigo" | "rival" => opp_id = v,
                        "dispose" | "reciclar" => {
                            dispose_old = matches!(v.as_str(), "1" | "true" | "si" | "yes");
                        }
                        "show" => show_ids.extend(split_csv_ids(&v)),
                        "hide" => hide_ids.extend(split_csv_ids(&v)),
                        _ => {}
                    }
                }
            }

            cue_lines.push(format!(
                "    {{ timeMs = {:.2}, kind = \"swap\", show = {}, hide = {}, disposeOld = {}, player = {}, opp = {}, color = \"#FFFFFF\", durSec = 0.0, alpha = 0.0, overHud = false, preload = {{}} }},",
                ev.time_ms.max(0.0),
                lua_str_array(&show_ids),
                lua_str_array(&hide_ids),
                if dispose_old { "true" } else { "false" },
                lua_escape_str(&player_id),
                lua_escape_str(&opp_id),
            ));
        } else if matches!(
            lower_name.as_str(),
            "flash effect" | "flash camera" | "camera flash" | "flash hud"
        ) {
            let color = if ev.value1.trim().is_empty() {
                "#FFFFFF".to_string()
            } else {
                ev.value1.trim().to_string()
            };
            let mut dur_sec = 0.65_f64;
            let mut end_ms: Option<f64> = None;
            let mut alpha = 0.85_f64;
            let mut over_hud = lower_name == "flash hud";

            let params = parse_kv_params(&ev.value2);
            if params.is_empty() && !ev.value2.trim().is_empty() {
                if let Ok(parsed_dur) = ev.value2.trim().parse::<f64>() {
                    if parsed_dur.is_finite() && parsed_dur > 0.01 {
                        dur_sec = parsed_dur;
                    }
                }
            } else {
                for (k, v) in params {
                    match k.as_str() {
                        "dur" | "duration" => {
                            if let Ok(d) = v.parse::<f64>() {
                                if d.is_finite() && d > 0.01 {
                                    dur_sec = d;
                                }
                            }
                        }
                        "endms" | "end_ms" => {
                            if let Ok(em) = v.parse::<f64>() {
                                if em.is_finite() {
                                    end_ms = Some(em);
                                }
                            }
                        }
                        "alpha" | "opacity" => {
                            if let Ok(a) = v.parse::<f64>() {
                                if a.is_finite() {
                                    alpha = a.clamp(0.05, 1.0);
                                }
                            }
                        }
                        "hud" | "overhud" => {
                            over_hud = matches!(v.as_str(), "1" | "true" | "si" | "yes");
                        }
                        _ => {}
                    }
                }
            }
            if let Some(em) = end_ms {
                if em > ev.time_ms + 15.0 {
                    dur_sec = ((em - ev.time_ms) / 1000.0).clamp(0.04, 30.0);
                }
            }

            cue_lines.push(format!(
                "    {{ timeMs = {:.2}, kind = \"flash\", show = {{}}, hide = {{}}, disposeOld = false, player = \"\", opp = \"\", color = {}, durSec = {:.3}, alpha = {:.2}, overHud = {}, preload = {{}} }},",
                ev.time_ms.max(0.0),
                lua_escape_str(&color),
                dur_sec,
                alpha,
                if over_hud { "true" } else { "false" },
            ));
        } else if lower_name == "preload sprite" {
            let preload_ids = split_csv_ids(&ev.value1);
            if !preload_ids.is_empty() {
                cue_lines.push(format!(
                    "    {{ timeMs = {:.2}, kind = \"preload\", show = {{}}, hide = {{}}, disposeOld = false, player = \"\", opp = \"\", color = \"#FFFFFF\", durSec = 0.0, alpha = 0.0, overHud = false, preload = {} }},",
                    ev.time_ms.max(0.0),
                    lua_str_array(&preload_ids),
                ));
            }
        }
    }

    let cues_block = if cue_lines.is_empty() {
        format!(
            "{}\nlocal PRELOAD_LOOKAHEAD_MS = 4000.0\nlocal STAGE_TIMELINE_CUES = {{}}\n{}",
            STAGE_CUES_START_MARKER, STAGE_CUES_END_MARKER
        )
    } else {
        format!(
            "{}\nlocal PRELOAD_LOOKAHEAD_MS = 4000.0\nlocal STAGE_TIMELINE_CUES = {{\n{}\n}}\n{}",
            STAGE_CUES_START_MARKER,
            cue_lines.join("\n"),
            STAGE_CUES_END_MARKER
        )
    };

    if let Some(src) = existing_source {
        if let (Some(start_idx), Some(end_idx)) = (
            src.find(STAGE_CUES_START_MARKER),
            src.find(STAGE_CUES_END_MARKER),
        ) {
            if end_idx >= start_idx {
                let tail_start = end_idx + STAGE_CUES_END_MARKER.len();
                let prefix = &src[..start_idx];
                let suffix = &src[tail_start..];
                return format!("{}{}{}", prefix, cues_block, suffix);
            }
        }
    }

    format!(
        "-- Script maestro de Eventos de Escenario, Flashes y Cambios de Personaje (BeatForge Studio)\n\
-- Sincronizado automáticamente con las marcas '#' del Chart Editor.\n\
-- Gestiona precarga anticipada (4s antes en IO), swaps de visibilidad, flashes y reciclaje de RAM.\n\
{}\n\n\
local preloadCursor = 1\n\
local execCursor = 1\n\
local lastSongPosMs = -1.0\n\n\
local function preloadCueTargets(cue)\n\
    for i = 1, #cue.preload do\n\
        preloadEntity(cue.preload[i])\n\
    end\n\
    for i = 1, #cue.show do\n\
        preloadEntity(cue.show[i])\n\
    end\n\
    if cue.player ~= \"\" then\n\
        preloadEntity(cue.player)\n\
    end\n\
    if cue.opp ~= \"\" then\n\
        preloadEntity(cue.opp)\n\
    end\n\
end\n\n\
local function executeCue(cue)\n\
    if cue.kind == \"flash\" then\n\
        flashEffect(cue.color, cue.durSec, cue.alpha, cue.overHud)\n\
    elseif cue.kind == \"preload\" then\n\
        preloadCueTargets(cue)\n\
    elseif cue.kind == \"swap\" then\n\
        local pairCount = math.min(#cue.hide, #cue.show)\n\
        for i = 1, pairCount do\n\
            swapEntity(cue.hide[i], cue.show[i], cue.disposeOld)\n\
        end\n\
        for i = pairCount + 1, #cue.hide do\n\
            local hideId = cue.hide[i]\n\
            setEntityProp(hideId, \"visible\", false)\n\
            if cue.disposeOld then\n\
                retireEntity(hideId)\n\
            end\n\
        end\n\
        for i = pairCount + 1, #cue.show do\n\
            swapEntity(\"\", cue.show[i], false)\n\
        end\n\
        if cue.player ~= \"\" then\n\
            setActiveCharacter(\"player\", cue.player, cue.disposeOld)\n\
        end\n\
        if cue.opp ~= \"\" then\n\
            setActiveCharacter(\"opponent\", cue.opp, cue.disposeOld)\n\
        end\n\
    end\n\
end\n\n\
function onCreate()\n\
    preloadCursor = 1\n\
    execCursor = 1\n\
    lastSongPosMs = -1.0\n\
    -- Precarga anticipada para eventos que ocurren en los primeros segundos de la canción\n\
    while preloadCursor <= #STAGE_TIMELINE_CUES and STAGE_TIMELINE_CUES[preloadCursor].timeMs <= PRELOAD_LOOKAHEAD_MS do\n\
        preloadCueTargets(STAGE_TIMELINE_CUES[preloadCursor])\n\
        preloadCursor = preloadCursor + 1\n\
    end\n\
end\n\n\
function onUpdate(dt)\n\
    local songPos = getSongPosMs()\n\
    if songPos + 250.0 < lastSongPosMs then\n\
        preloadCursor = 1\n\
        execCursor = 1\n\
    end\n\
    lastSongPosMs = songPos\n\n\
    while preloadCursor <= #STAGE_TIMELINE_CUES and STAGE_TIMELINE_CUES[preloadCursor].timeMs <= (songPos + PRELOAD_LOOKAHEAD_MS) do\n\
        preloadCueTargets(STAGE_TIMELINE_CUES[preloadCursor])\n\
        preloadCursor = preloadCursor + 1\n\
    end\n\n\
    while execCursor <= #STAGE_TIMELINE_CUES and STAGE_TIMELINE_CUES[execCursor].timeMs <= songPos do\n\
        executeCue(STAGE_TIMELINE_CUES[execCursor])\n\
        execCursor = execCursor + 1\n\
    end\n\
end\n",
        cues_block
    )
}

fn parse_optional_bool_flag(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "si" | "sí" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Normaliza un evento de apagón / visibilidad de HUD y flechas proveniente tanto del editor `#`
/// como de archivos `chart.json` externos (ej. `"Screen Fade"`, `"Camera Fade"`, `"Hide UI"`).
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
    )
}

/// Genera o actualiza el bloque `-- [UI_BLACKOUT_START] ... -- [UI_BLACKOUT_END]` dentro de
/// `ui_blackout.lua`. Permite ocultar la interfaz, las flechas y poner la pantalla en negro
/// al comenzar la canción conservando intacta la cuenta regresiva `"3, 2, 1, ¡YA!"`, además de
/// ejecutar apagones o desvanecimientos en cualquier paso del chart.
pub fn render_ui_blackout_lua_script(chart: &ChartData, existing_source: Option<&str>) -> String {
    let mut sorted_events = chart.events.clone();
    sorted_events.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));

    let mut intro_black_enabled = false;
    let mut intro_black_alpha = 1.0_f64;
    let mut intro_hide_hud = true;
    let mut intro_hide_arrows = true;
    let mut intro_reveal_on_start = true;
    let mut intro_reveal_delay_sec = 0.0_f64;
    let mut intro_fade_sec = 0.65_f64;
    let mut cue_lines = Vec::new();

    for ev in &sorted_events {
        let lower_name = ev.name.trim().to_ascii_lowercase();
        if !is_ui_blackout_event_name(&lower_name) {
            continue;
        }

        let v1_lower = ev.value1.trim().to_ascii_lowercase();
        let mut action = if lower_name == "hide ui" || lower_name == "hide hud" {
            "hide".to_string()
        } else if lower_name == "show ui" || lower_name == "show hud" {
            "show".to_string()
        } else if lower_name == "toggle ui" {
            "toggle".to_string()
        } else if v1_lower == "intro"
            || v1_lower == "show"
            || v1_lower == "reveal"
            || v1_lower == "hide"
            || v1_lower == "toggle"
            || v1_lower == "fade"
        {
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
                if lower_name == "hide ui" || lower_name == "hide hud" {
                    -1.0
                } else {
                    1.0
                }
            }
            "show" => 0.0,
            _ => -1.0,
        };
        let mut hud_alpha = match action.as_str() {
            "intro" | "hide" => 0.0,
            "show" => 1.0,
            _ => -1.0,
        };
        let mut arrows_alpha = match action.as_str() {
            "intro" | "hide" => 0.0,
            "show" => 1.0,
            _ => -1.0,
        };
        let mut dur_sec = if action == "hide" && ev.time_ms <= 50.0 {
            0.0_f64
        } else {
            0.60_f64
        };
        let mut reveal_ms = 0.0_f64;
        let mut on_start = true;
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
                    "revealms" | "reveal_ms" | "delayms" => {
                        if let Ok(rm) = v.parse::<f64>() {
                            if rm.is_finite() && rm >= 0.0 {
                                reveal_ms = rm.clamp(0.0, 60_000.0);
                            }
                        }
                    }
                    "onstart" | "on_start" => {
                        if let Some(flag) = parse_optional_bool_flag(&v) {
                            on_start = flag;
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

        let is_intro_cue = action == "intro"
            || (ev.time_ms <= 120.0 && black_alpha >= 0.85 && hud_alpha >= 0.0 && hud_alpha <= 0.15);

        if is_intro_cue {
            intro_black_enabled = true;
            intro_black_alpha = if black_alpha >= 0.0 { black_alpha } else { 1.0 };
            intro_hide_hud = hud_alpha <= 0.05;
            intro_hide_arrows = arrows_alpha <= 0.05;
            intro_reveal_on_start = on_start || reveal_ms > 0.0;
            intro_reveal_delay_sec = (reveal_ms / 1000.0).clamp(0.0, 60.0);
            intro_fade_sec = if dur_sec > 0.01 { dur_sec } else { 0.65 };
            continue;
        }

        cue_lines.push(format!(
            "    {{ timeMs = {:.2}, action = {}, black = {:.2}, hud = {:.2}, arrows = {:.2}, durSec = {:.3}, ease = {} }},",
            ev.time_ms.max(0.0),
            lua_escape_str(&action),
            black_alpha,
            hud_alpha,
            arrows_alpha,
            dur_sec,
            lua_escape_str(&ease),
        ));
    }

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

    format!(
        "-- Script maestro de Pantalla en Negro y Visibilidad de UI (BeatForge Studio)\n\
-- Controla intros con pantalla negra y UI/flechas ocultas conservando visible el '3, 2, 1, ¡YA!'\n\
-- y apagones/desvanecimientos sincronizados con los eventos '#' del Chart Editor o chart.json externos.\n\
{}\n\n\
local cueCursor = 1\n\
local lastSongPosMs = -1.0\n\
local uiHiddenToggle = false\n\n\
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
    applyChannelTransition(0.0, 1.0, 1.0, INTRO_FADE_SEC, \"sineInOut\")\n\
    uiHiddenToggle = false\n\
end\n\n\
local function executeUiCue(cue)\n\
    if cue.action == \"toggle\" then\n\
        uiHiddenToggle = not uiHiddenToggle\n\
        local nextHud = uiHiddenToggle and 0.0 or 1.0\n\
        local nextArrows = uiHiddenToggle and 0.0 or 1.0\n\
        local nextBlack = (cue.black >= 0.0) and (uiHiddenToggle and cue.black or 0.0) or -1.0\n\
        applyChannelTransition(nextBlack, nextHud, nextArrows, cue.durSec, cue.ease)\n\
    else\n\
        if cue.hud >= 0.0 then\n\
            uiHiddenToggle = cue.hud <= 0.05\n\
        end\n\
        applyChannelTransition(cue.black, cue.hud, cue.arrows, cue.durSec, cue.ease)\n\
    end\n\
end\n\n\
function onCreate()\n\
    cueCursor = 1\n\
    lastSongPosMs = -1.0\n\
    uiHiddenToggle = false\n\
    if INTRO_BLACK_ENABLED then\n\
        -- Oscurece el escenario y oculta HUD/flechas al instante; el conteo '3, 2, 1, ¡YA!' sigue visible.\n\
        setScreenBlackout(INTRO_BLACKOUT_ALPHA)\n\
        if INTRO_HIDE_HUD then\n\
            setHudOpacity(0.0)\n\
            setHudVisible(false)\n\
        end\n\
        if INTRO_HIDE_ARROWS then\n\
            setArrowsOpacity(0.0)\n\
            setArrowsVisible(false)\n\
        end\n\
        uiHiddenToggle = INTRO_HIDE_HUD or INTRO_HIDE_ARROWS\n\
    else\n\
        setScreenBlackout(0.0)\n\
        setHudVisible(true)\n\
        setHudOpacity(1.0)\n\
        setArrowsVisible(true)\n\
        setArrowsOpacity(1.0)\n\
    end\n\
end\n\n\
function onSongStart()\n\
    if INTRO_BLACK_ENABLED and INTRO_REVEAL_AT_SONG_START then\n\
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
    local lower = string.lower(tostring(name or \"\"))\n\
    local v1 = tostring(value1 or \"\")\n\
    local v2 = tostring(value2 or \"\")\n\
    if lower == \"hide ui\" or lower == \"hide hud\" then\n\
        local dur = tonumber(v2) or 0.35\n\
        applyChannelTransition(-1.0, 0.0, 0.0, dur, \"sineInOut\")\n\
    elseif lower == \"show ui\" or lower == \"show hud\" then\n\
        local dur = tonumber(v2) or 0.35\n\
        applyChannelTransition(0.0, 1.0, 1.0, dur, \"sineInOut\")\n\
    elseif lower == \"toggle ui\" then\n\
        uiHiddenToggle = not uiHiddenToggle\n\
        local target = uiHiddenToggle and 0.0 or 1.0\n\
        local dur = tonumber(v2) or 0.35\n\
        applyChannelTransition(-1.0, target, target, dur, \"sineInOut\")\n\
    elseif lower == \"fade ui\" or lower == \"hud fade\" then\n\
        local target = tonumber(v1) or 0.0\n\
        local dur = tonumber(v2) or 0.5\n\
        applyChannelTransition(-1.0, target, target, dur, \"sineInOut\")\n\
    elseif lower == \"screen fade\" or lower == \"camera fade\" or lower == \"blackout\" then\n\
        local target = tonumber(v1) or 1.0\n\
        local dur = tonumber(v2) or 0.5\n\
        applyChannelTransition(target, -1.0, -1.0, dur, \"sineInOut\")\n\
    end\n\
end\n",
        cfg_and_cues_block
    )
}

/// Modo de control táctil en móvil:
/// - `TapArrows` (Por defecto): 100 % táctil directo tocando las 4 flechas receptoras.
/// - `ClassicHitbox`: Diseño clásico estilo Psych Engine Mobile (4 columnas hitbox verticales).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MobileControlMode {
    #[default]
    TapArrows,
    ClassicHitbox,
}

/// Lenguaje de un script comunitario integrado en el mod.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScriptLanguage {
    Lua,
    TypeScript,
    JavaScript,
}

impl ScriptLanguage {
    pub fn label(self) -> &'static str {
        match self {
            ScriptLanguage::Lua => "Lua 5.4",
            ScriptLanguage::TypeScript => "TypeScript",
            ScriptLanguage::JavaScript => "JavaScript",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            ScriptLanguage::Lua => "lua",
            ScriptLanguage::TypeScript => "ts",
            ScriptLanguage::JavaScript => "js",
        }
    }
}

/// Representa un archivo de script editable o importado en el proyecto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptAsset {
    pub id: String,
    pub name: String,
    pub language: ScriptLanguage,
    pub source_code: String,
    pub enabled: bool,
    /// JavaScript ya transpilado/validado durante `hot_compile` para omitir `oxc` al iniciar gameplay.
    #[serde(skip)]
    pub precompiled_js: Option<String>,
    /// Bytecode Lua 5.4 compilado con `dump(false)` en `hot_compile` para arranque instantáneo.
    #[serde(skip)]
    pub precompiled_bytecode: Option<Vec<u8>>,
}

/// Ranura de pista de audio (máximo 3: Principal/Instrumental, Voces Jugador, Voces Rival).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioTrackSlot {
    pub slot_index: usize,
    pub role_label: String,
    pub file_path: Option<PathBuf>,
    pub file_name: Option<String>,
    pub gain: f32,
    pub muted: bool,
    pub duck_on_player_miss: bool,
}

impl AudioTrackSlot {
    pub fn default_three_slots() -> [AudioTrackSlot; MAX_AUDIO_TRACKS] {
        [
            AudioTrackSlot {
                slot_index: 0,
                role_label: "Pista 1: Mix Único o Instrumental".to_string(),
                file_path: None,
                file_name: None,
                gain: 1.0,
                muted: false,
                duck_on_player_miss: false,
            },
            AudioTrackSlot {
                slot_index: 1,
                role_label: "Pista 2: Voces Player (Opcional)".to_string(),
                file_path: None,
                file_name: None,
                gain: 1.0,
                muted: false,
                duck_on_player_miss: true,
            },
            AudioTrackSlot {
                slot_index: 2,
                role_label: "Pista 3: Voces Enemigo (Opcional)".to_string(),
                file_path: None,
                file_name: None,
                gain: 1.0,
                muted: false,
                duck_on_player_miss: false,
            },
        ]
    }
}

/// Marca de tiempo actual en milisegundos Unix para ordenar proyectos recientes.
pub fn current_epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Proyecto completo del usuario listo para editar o compilar en caliente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModProject {
    pub id: String,
    pub name: String,
    pub author: String,
    #[serde(default)]
    pub updated_at_ms: u64,
    pub chart: ChartData,
    pub chart_source_path: Option<PathBuf>,
    pub audio_tracks: [AudioTrackSlot; MAX_AUDIO_TRACKS],
    pub stage: StageScene,
    pub arrow_skin: ArrowSkinSet,
    #[serde(default)]
    pub health_bar_svg_path: Option<PathBuf>,
    #[serde(default)]
    pub health_bar_svg_source: Option<String>,
    pub scripts: Vec<ScriptAsset>,
    pub downscroll: bool,
    pub control_mode: MobileControlMode,
    pub audio_offset_ms: f64,
    pub scroll_speed_override: Option<f64>,
    #[serde(default)]
    pub preferred_render_backend: RenderBackendKind,
}

impl ModProject {
    pub fn new_demo() -> Self {
        let stage = StageScene::default();
        let chart = ChartData::bundled_demo_chart();
        let camera_lua = render_camera_lua_script(&stage.camera, None);
        let stage_events_lua = render_stage_events_lua_script(&chart, &stage, None);
        let ui_blackout_lua = render_ui_blackout_lua_script(&chart, None);

        let default_lua = r#"-- Script Lua de ejemplo para BeatForge Studio
function onCreate()
    log("Escena Lua iniciada!")
end

function onBeatHit(beat)
    if beat % 2 == 0 then
        setCameraZoom(getCameraBaseZoom() + 0.04)
    end
end
"#;

        let default_ts = r#"// Script TypeScript de ejemplo (compilado por oxc en Rust)
interface NoteEvent {
    lane: number;
    isPlayer: boolean;
}

function onNoteHit(lane: number, isPlayer: boolean): void {
    const targetId: string = isPlayer ? "player_main" : "opponent_main";
    if (lane === 2) {
        setEntityProp(targetId, "opacity", 1.0);
    }
}
"#;

        Self {
            id: "proj_neon_pulse".to_string(),
            name: "Neon Pulse Demo".to_string(),
            author: "Creador Local".to_string(),
            updated_at_ms: current_epoch_ms(),
            chart,
            chart_source_path: None,
            audio_tracks: AudioTrackSlot::default_three_slots(),
            stage,
            arrow_skin: ArrowSkinSet::bundled_default(),
            health_bar_svg_path: None,
            health_bar_svg_source: None,
            scripts: vec![
                ScriptAsset {
                    id: CAMERA_SCRIPT_ID.to_string(),
                    name: CAMERA_SCRIPT_NAME.to_string(),
                    language: ScriptLanguage::Lua,
                    source_code: camera_lua,
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
                ScriptAsset {
                    id: STAGE_EVENTS_SCRIPT_ID.to_string(),
                    name: STAGE_EVENTS_SCRIPT_NAME.to_string(),
                    language: ScriptLanguage::Lua,
                    source_code: stage_events_lua,
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
                ScriptAsset {
                    id: UI_BLACKOUT_SCRIPT_ID.to_string(),
                    name: UI_BLACKOUT_SCRIPT_NAME.to_string(),
                    language: ScriptLanguage::Lua,
                    source_code: ui_blackout_lua,
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
                ScriptAsset {
                    id: "script_beat_zoom".to_string(),
                    name: "camera_bounce.lua".to_string(),
                    language: ScriptLanguage::Lua,
                    source_code: default_lua.to_string(),
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
                ScriptAsset {
                    id: "script_note_fx".to_string(),
                    name: "note_effects.ts".to_string(),
                    language: ScriptLanguage::TypeScript,
                    source_code: default_ts.to_string(),
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
            ],
            downscroll: false,
            control_mode: MobileControlMode::TapArrows,
            audio_offset_ms: 0.0,
            scroll_speed_override: None,
            preferred_render_backend: RenderBackendKind::Canvas2d,
        }
    }

    /// Garantiza que proyectos antiguos cargados desde disco cuenten con `camera_system.lua` actualizado (V2).
    pub fn ensure_camera_script(&mut self) {
        self.stage.camera.ensure_default_cameras();
        if let Some(script) = self
            .scripts
            .iter_mut()
            .find(|s| s.id == CAMERA_SCRIPT_ID || s.name == CAMERA_SCRIPT_NAME)
        {
            let rendered = render_camera_lua_script(&self.stage.camera, Some(&script.source_code));
            if rendered != script.source_code {
                script.source_code = rendered;
                script.precompiled_bytecode = None;
            }
        } else {
            let source_code = render_camera_lua_script(&self.stage.camera, None);
            self.scripts.insert(
                0,
                ScriptAsset {
                    id: CAMERA_SCRIPT_ID.to_string(),
                    name: CAMERA_SCRIPT_NAME.to_string(),
                    language: ScriptLanguage::Lua,
                    source_code,
                    enabled: self.stage.camera.enabled,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
            );
        }
    }

    /// Garantiza la presencia de `stage_events.lua` en proyectos nuevos o cargados desde disco.
    pub fn ensure_stage_events_script(&mut self) {
        let has_stage_events = self
            .scripts
            .iter()
            .any(|s| s.id == STAGE_EVENTS_SCRIPT_ID || s.name == STAGE_EVENTS_SCRIPT_NAME);
        if !has_stage_events {
            let source_code = render_stage_events_lua_script(&self.chart, &self.stage, None);
            let insert_idx = usize::from(
                self.scripts
                    .first()
                    .is_some_and(|s| s.id == CAMERA_SCRIPT_ID || s.name == CAMERA_SCRIPT_NAME),
            );
            self.scripts.insert(
                insert_idx.min(self.scripts.len()),
                ScriptAsset {
                    id: STAGE_EVENTS_SCRIPT_ID.to_string(),
                    name: STAGE_EVENTS_SCRIPT_NAME.to_string(),
                    language: ScriptLanguage::Lua,
                    source_code,
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
            );
        }
    }

    /// Resincroniza el bloque `-- [STAGE_CUES_START]..-- [STAGE_CUES_END]` de `stage_events.lua`
    /// tras editar cues en el Chart Editor o importar un chart nuevo.
    pub fn sync_stage_events_script(&mut self) {
        self.ensure_stage_events_script();
        if let Some(script) = self
            .scripts
            .iter_mut()
            .find(|s| s.id == STAGE_EVENTS_SCRIPT_ID || s.name == STAGE_EVENTS_SCRIPT_NAME)
        {
            script.source_code =
                render_stage_events_lua_script(&self.chart, &self.stage, Some(&script.source_code));
            script.precompiled_bytecode = None;
            script.precompiled_js = None;
        }
    }

    /// Garantiza la presencia de `ui_blackout.lua` en proyectos nuevos o cargados desde disco.
    pub fn ensure_ui_blackout_script(&mut self) {
        let has_ui_blackout = self
            .scripts
            .iter()
            .any(|s| s.id == UI_BLACKOUT_SCRIPT_ID || s.name == UI_BLACKOUT_SCRIPT_NAME);
        if !has_ui_blackout {
            let source_code = render_ui_blackout_lua_script(&self.chart, None);
            let mut insert_idx = 0usize;
            for (idx, s) in self.scripts.iter().enumerate() {
                if s.id == CAMERA_SCRIPT_ID
                    || s.name == CAMERA_SCRIPT_NAME
                    || s.id == STAGE_EVENTS_SCRIPT_ID
                    || s.name == STAGE_EVENTS_SCRIPT_NAME
                {
                    insert_idx = idx + 1;
                }
            }
            self.scripts.insert(
                insert_idx.min(self.scripts.len()),
                ScriptAsset {
                    id: UI_BLACKOUT_SCRIPT_ID.to_string(),
                    name: UI_BLACKOUT_SCRIPT_NAME.to_string(),
                    language: ScriptLanguage::Lua,
                    source_code,
                    enabled: true,
                    precompiled_js: None,
                    precompiled_bytecode: None,
                },
            );
        }
    }

    /// Resincroniza el bloque `-- [UI_BLACKOUT_START]..-- [UI_BLACKOUT_END]` de `ui_blackout.lua`
    /// tras editar cues del Chart Editor o importar un chart.json externo.
    pub fn sync_ui_blackout_script(&mut self) {
        self.ensure_ui_blackout_script();
        if let Some(script) = self
            .scripts
            .iter_mut()
            .find(|s| s.id == UI_BLACKOUT_SCRIPT_ID || s.name == UI_BLACKOUT_SCRIPT_NAME)
        {
            script.source_code =
                render_ui_blackout_lua_script(&self.chart, Some(&script.source_code));
            script.precompiled_bytecode = None;
            script.precompiled_js = None;
        }
    }

    /// Crea un nuevo proyecto limpio listo para personalizar desde cero.
    pub fn new_blank(id: impl Into<String>, name: impl Into<String>, author: impl Into<String>) -> Self {
        let mut proj = Self::new_demo();
        proj.id = id.into();
        let clean_name = name.into();
        proj.name = clean_name.clone();
        proj.author = author.into();
        proj.chart.title = clean_name;
        proj.updated_at_ms = current_epoch_ms();
        proj
    }

    /// Asigna una pista de audio respetando el límite estricto de máximo 3 pistas.
    pub fn assign_audio_track(&mut self, slot_index: usize, path: PathBuf) -> Result<(), String> {
        if slot_index >= MAX_AUDIO_TRACKS {
            return Err(format!(
                "Máximo {} pistas permitidas; índice {} fuera de rango",
                MAX_AUDIO_TRACKS, slot_index
            ));
        }
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio")
            .to_string();
        self.audio_tracks[slot_index].file_path = Some(path);
        self.audio_tracks[slot_index].file_name = Some(file_name);
        Ok(())
    }

    pub fn active_audio_count(&self) -> usize {
        self.audio_tracks
            .iter()
            .filter(|t| t.file_path.is_some())
            .count()
    }
}
