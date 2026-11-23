//! Archivo: engine/crates/pulse_core/src/chart/camera_events.rs
//! Propósito: Intérprete nativo de eventos de cámara incluidos en `chart.json`
//! (Psych `Camera Follow Pos` / `Add Camera Zoom`, V-Slice `FocusCamera` / `ZoomCamera`,
//! Codename `Camera Movement` y cues `Switch Camera`).

use super::parser::json_as_f64;
use serde_json::Value;

/// Acción de cámara extraída directamente desde eventos estándar de `chart.json`
/// (Psych Engine `Camera Follow Pos` / `Add Camera Zoom`, V-Slice `FocusCamera` / `ZoomCamera`,
/// Codename Engine `Camera Movement` y cues `Switch Camera`).
#[derive(Debug, Clone, PartialEq)]
pub enum ParsedChartCameraAction {
    /// Cambia a una cámara numerada (`0=Cam0`, `1=Player`, `2=Enemigo`, `3+=Personalizada`) o apunta a GF.
    SelectCamera {
        slot: Option<usize>,
        focus_gf: bool,
        offset_x: f32,
        offset_y: f32,
        zoom: Option<f32>,
        duration_sec: f32,
        ease: String,
    },
    /// Bloquea la cámara en coordenadas fijas `(x, y)` del mundo (`Camera Follow Pos` o `FocusCamera char=-1`).
    FollowPos {
        x: f32,
        y: f32,
        zoom: Option<f32>,
        duration_sec: f32,
        ease: String,
    },
    /// Libera el bloqueo de posición fija (`Camera Follow Pos` con valores vacíos) y devuelve el control a la cámara activa.
    ClearFollowPos,
    /// Cambia el campo de visión (zoom) de cámara (`ZoomCamera`).
    SetZoom {
        zoom: f32,
        is_direct_stage: bool,
        duration_sec: f32,
        ease: String,
    },
    /// Añade un golpe/bop instantáneo al zoom de cámara y HUD (`Add Camera Zoom`).
    AddZoomBump {
        cam_zoom_add: f32,
        hud_zoom_add: f32,
    },
}

fn parse_inline_kv_pairs(raw: &str) -> Vec<(String, String)> {
    raw.split(';')
        .filter_map(|part| {
            let t = part.trim();
            if t.is_empty() {
                return None;
            }
            t.split_once('=')
                .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        })
        .collect()
}

/// Interpreta un evento del `chart.json` relacionado con movimiento, enfoque o zoom de cámara.
pub fn parse_chart_camera_event(
    name: &str,
    value1: &str,
    value2: &str,
    active_bpm: f64,
) -> Option<ParsedChartCameraAction> {
    let lower = name.trim().to_ascii_lowercase();
    let v1 = value1.trim();
    let v2 = value2.trim();
    let safe_bpm = active_bpm.max(40.0);
    let step_sec = ((60.0 / safe_bpm) / 4.0) as f32;

    match lower.as_str() {
        "switch camera" | "camera slot" | "set camera" | "cambiar camara" | "cambiar cámara" => {
            if let Ok(slot_num) = v1.parse::<i32>() {
                let zoom = v2.parse::<f32>().ok().filter(|z| z.is_finite() && *z > 0.1);
                return Some(ParsedChartCameraAction::SelectCamera {
                    slot: Some(slot_num.max(0) as usize),
                    focus_gf: false,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                });
            }
            let pairs = parse_inline_kv_pairs(v1);
            let mut slot: Option<usize> = None;
            let mut zoom: Option<f32> = None;
            let mut dur: f32 = 0.0;
            let mut ease = "sineOut".to_string();
            let mut clear_lock = false;
            let mut fx: Option<f32> = None;
            let mut fy: Option<f32> = None;
            for (k, v) in pairs {
                match k.as_str() {
                    "cam" | "slot" | "camera" => {
                        if let Ok(n) = v.parse::<i32>() {
                            slot = Some(n.max(0) as usize);
                        }
                    }
                    "zoom" | "fov" => {
                        zoom = v.parse::<f32>().ok().filter(|z| z.is_finite() && *z > 0.1);
                    }
                    "dur" | "duration" => {
                        dur = v.parse::<f32>().unwrap_or(0.0).max(0.0);
                    }
                    "ease" => {
                        if !v.is_empty() {
                            ease = v;
                        }
                    }
                    "x" => fx = v.parse::<f32>().ok().filter(|n| n.is_finite()),
                    "y" => fy = v.parse::<f32>().ok().filter(|n| n.is_finite()),
                    "clear" | "unlock" => {
                        clear_lock = matches!(v.as_str(), "1" | "true" | "yes" | "si");
                    }
                    _ => {}
                }
            }
            if !v2.is_empty() {
                for (k, v) in parse_inline_kv_pairs(v2) {
                    match k.as_str() {
                        "zoom" | "fov" => {
                            zoom = v.parse::<f32>().ok().filter(|z| z.is_finite() && *z > 0.1);
                        }
                        "dur" | "duration" => dur = v.parse::<f32>().unwrap_or(dur).max(0.0),
                        "ease" if !v.is_empty() => ease = v,
                        _ => {}
                    }
                }
                if zoom.is_none() && !v2.contains('=') {
                    zoom = v2.parse::<f32>().ok().filter(|z| z.is_finite() && *z > 0.1);
                }
            }
            if clear_lock && slot.is_none() && fx.is_none() && fy.is_none() {
                return Some(ParsedChartCameraAction::ClearFollowPos);
            }
            if let (Some(x), Some(y)) = (fx, fy) {
                if slot.is_none() {
                    return Some(ParsedChartCameraAction::FollowPos {
                        x,
                        y,
                        zoom,
                        duration_sec: dur,
                        ease,
                    });
                }
            }
            Some(ParsedChartCameraAction::SelectCamera {
                slot: Some(slot.unwrap_or(0)),
                focus_gf: false,
                offset_x: fx.unwrap_or(0.0),
                offset_y: fy.unwrap_or(0.0),
                zoom,
                duration_sec: dur,
                ease,
            })
        }
        "camera follow pos" | "camerafollowpos" => {
            if v1.is_empty() && v2.is_empty() {
                return Some(ParsedChartCameraAction::ClearFollowPos);
            }
            let x = v1.parse::<f32>().ok().filter(|n| n.is_finite())?;
            let y = v2.parse::<f32>().ok().filter(|n| n.is_finite())?;
            Some(ParsedChartCameraAction::FollowPos {
                x,
                y,
                zoom: None,
                duration_sec: 0.0,
                ease: "sineOut".to_string(),
            })
        }
        "add camera zoom" | "addcamera zoom" | "add cam zoom" => {
            let cam_add = if v1.is_empty() {
                0.015
            } else {
                v1.parse::<f32>().unwrap_or(0.015)
            };
            let hud_add = if v2.is_empty() {
                0.03
            } else {
                v2.parse::<f32>().unwrap_or(0.03)
            };
            Some(ParsedChartCameraAction::AddZoomBump {
                cam_zoom_add: if cam_add.is_finite() { cam_add } else { 0.0 },
                hud_zoom_add: if hud_add.is_finite() { hud_add } else { 0.0 },
            })
        }
        "focuscamera" | "focus camera" => {
            if v1.starts_with('{') {
                if let Ok(obj) = serde_json::from_str::<Value>(v1) {
                    let explicit_cam = obj
                    .get("cam")
                    .or_else(|| obj.get("slot"))
                    .and_then(json_as_f64)
                    .filter(|n| n.is_finite() && *n >= 0.0)
                    .map(|n| n.round() as usize);
                let char_id = obj.get("char").and_then(json_as_f64).map(|n| n.round() as i32);
                let ox = obj.get("x").and_then(json_as_f64).unwrap_or(0.0) as f32;
                let oy = obj.get("y").and_then(json_as_f64).unwrap_or(0.0) as f32;
                let dur_steps = obj.get("duration").and_then(json_as_f64).unwrap_or(0.0).max(0.0) as f32;
                let ease = obj
                    .get("ease")
                    .and_then(|v| v.as_str())
                    .unwrap_or("sineOut")
                    .to_string();
                let zoom = obj
                    .get("zoom")
                    .and_then(json_as_f64)
                    .map(|z| z as f32)
                    .filter(|z| z.is_finite() && *z > 0.1);
                let duration_sec = if dur_steps > 0.0 { dur_steps * step_sec } else { 0.0 };

                if let Some(slot) = explicit_cam {
                    return Some(ParsedChartCameraAction::SelectCamera {
                        slot: Some(slot),
                        focus_gf: false,
                        offset_x: ox,
                        offset_y: oy,
                        zoom,
                        duration_sec,
                        ease,
                    });
                }

                return match char_id {
                    // V-Slice: char 0 = BF/Player -> Cam 1; 1 = Dad/Opponent -> Cam 2; 2 = GF; -1 = coordenadas stage
                    Some(0) => Some(ParsedChartCameraAction::SelectCamera {
                        slot: Some(1),
                        focus_gf: false,
                        offset_x: ox,
                        offset_y: oy,
                        zoom,
                        duration_sec,
                        ease,
                    }),
                    Some(1) => Some(ParsedChartCameraAction::SelectCamera {
                        slot: Some(2),
                        focus_gf: false,
                        offset_x: ox,
                        offset_y: oy,
                        zoom,
                        duration_sec,
                        ease,
                    }),
                    Some(2) => Some(ParsedChartCameraAction::SelectCamera {
                        slot: None,
                        focus_gf: true,
                        offset_x: ox,
                        offset_y: oy,
                        zoom,
                        duration_sec,
                        ease,
                    }),
                    Some(-1) => {
                        if ox.abs() < 0.01 && oy.abs() < 0.01 && obj.get("x").is_none() {
                            Some(ParsedChartCameraAction::SelectCamera {
                                slot: Some(0),
                                focus_gf: false,
                                offset_x: 0.0,
                                offset_y: 0.0,
                                zoom,
                                duration_sec,
                                ease,
                            })
                        } else {
                            Some(ParsedChartCameraAction::FollowPos {
                                x: ox,
                                y: oy,
                                zoom,
                                duration_sec,
                                ease,
                            })
                        }
                    }
                    _ => Some(ParsedChartCameraAction::FollowPos {
                        x: ox,
                        y: oy,
                        zoom,
                        duration_sec,
                        ease,
                    }),
                };
                }
            }
            // Formato escalar directo: v1 = "0"|"1"|"2"|"bf"|"dad"|"gf"
            match v1.to_ascii_lowercase().as_str() {
                "0" | "bf" | "boyfriend" | "player" | "jugador" => Some(ParsedChartCameraAction::SelectCamera {
                    slot: Some(1),
                    focus_gf: false,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom: None,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                }),
                "1" | "dad" | "opponent" | "enemigo" | "rival" => Some(ParsedChartCameraAction::SelectCamera {
                    slot: Some(2),
                    focus_gf: false,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom: None,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                }),
                "2" | "gf" | "girlfriend" => Some(ParsedChartCameraAction::SelectCamera {
                    slot: None,
                    focus_gf: true,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom: None,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                }),
                "-1" | "cam0" | "stage" => Some(ParsedChartCameraAction::SelectCamera {
                    slot: Some(0),
                    focus_gf: false,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom: None,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                }),
                _ => None,
            }
        }
        "zoomcamera" | "zoom camera" => {
            if v1.starts_with('{') {
                if let Ok(obj) = serde_json::from_str::<Value>(v1) {
                    let zoom = obj.get("zoom").and_then(json_as_f64)? as f32;
                    let dur_steps = obj.get("duration").and_then(json_as_f64).unwrap_or(4.0).max(0.0) as f32;
                    let mode = obj.get("mode").and_then(|v| v.as_str()).unwrap_or("direct");
                    let ease = obj
                        .get("ease")
                        .and_then(|v| v.as_str())
                        .unwrap_or("sineOut")
                        .to_string();
                    if !zoom.is_finite() {
                        return None;
                    }
                    return Some(ParsedChartCameraAction::SetZoom {
                        zoom,
                        is_direct_stage: mode.eq_ignore_ascii_case("stage"),
                        duration_sec: dur_steps * step_sec,
                        ease,
                    });
                }
            }
            let zoom = v1.parse::<f32>().ok().filter(|z| z.is_finite() && *z > 0.1)?;
            let dur = v2.parse::<f32>().ok().filter(|d| d.is_finite() && *d >= 0.0).unwrap_or(0.0);
            Some(ParsedChartCameraAction::SetZoom {
                zoom,
                is_direct_stage: false,
                duration_sec: dur,
                ease: "sineOut".to_string(),
            })
        }
        "camera movement" | "cameramovement" => {
            // Codename Engine: 0 = Opponent (Cam 2), 1 = Player (Cam 1), 2 = GF
            let target = v1.to_ascii_lowercase();
            if target == "1" || target == "boyfriend" || target == "bf" || target == "player" {
                Some(ParsedChartCameraAction::SelectCamera {
                    slot: Some(1),
                    focus_gf: false,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom: None,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                })
            } else if target == "2" || target == "gf" || target == "girlfriend" {
                Some(ParsedChartCameraAction::SelectCamera {
                    slot: None,
                    focus_gf: true,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom: None,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                })
            } else {
                Some(ParsedChartCameraAction::SelectCamera {
                    slot: Some(2),
                    focus_gf: false,
                    offset_x: 0.0,
                    offset_y: 0.0,
                    zoom: None,
                    duration_sec: 0.0,
                    ease: "sineOut".to_string(),
                })
            }
        }
        _ => None,
    }
}
