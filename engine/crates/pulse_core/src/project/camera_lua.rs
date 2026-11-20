//! Archivo: engine/crates/pulse_core/src/project/camera_lua.rs
//! Propósito: Generación y parseo bidireccional del script maestro `camera_system.lua`,
//! manteniendo sincronizados los parámetros del bloque `-- [CAM_CFG_START]..-- [CAM_CFG_END]`
//! con `StageCameraConfig` sin sobrescribir la lógica Lua personalizada del usuario.

use crate::stage::{CameraFollowMode, StageCameraConfig};

const CAM_CFG_START_MARKER: &str = "-- [CAM_CFG_START]";
const CAM_CFG_END_MARKER: &str = "-- [CAM_CFG_END]";
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
local CAM_CLAMP_BG = {}\n\
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
        if normalized.clamp_to_background { "true" } else { "false" },
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
            "CAM_CLAMP_BG" => {
                if val.eq_ignore_ascii_case("true") {
                    next.clamp_to_background = true;
                } else if val.eq_ignore_ascii_case("false") {
                    next.clamp_to_background = false;
                }
            }
            _ => {}
        }
    }
    next.sync_slot0_from_base();
    next
}
