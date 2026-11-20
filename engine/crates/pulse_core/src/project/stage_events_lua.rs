//! Archivo: engine/crates/pulse_core/src/project/stage_events_lua.rs
//! Propósito: Genera y sincroniza el script maestro `stage_events.lua`, compilando los eventos
//! de escenario del chart (`Stage Swap`, `Change Character`, `Flash Effect`, `Preload Sprite`)
//! en cues ordenados con precarga anticipada en IO y reciclaje de memoria.

use super::lua_helpers::{lua_escape_str, lua_str_array, parse_kv_params, split_csv_ids};
use crate::chart::ChartData;
use crate::stage::StageScene;

const STAGE_CUES_START_MARKER: &str = "-- [STAGE_CUES_START]";
const STAGE_CUES_END_MARKER: &str = "-- [STAGE_CUES_END]";

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
