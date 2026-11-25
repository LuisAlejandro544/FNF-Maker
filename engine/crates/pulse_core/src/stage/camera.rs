//! Archivo: engine/crates/pulse_core/src/stage/camera.rs
//! Propósito: Define el sistema multicámara (`Cam 0`, `Cam 1`, `Cam 2`, `Cam 3+`), modos de
//! seguimiento automático y configuración/sanitización de la viñeta lateral de cámara.

use serde::{Deserialize, Serialize};

/// Objetivo al que apunta una cámara individual (`Cam 0`, `Cam 1`, `Cam 2`, `Cam 3+`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CameraTargetKind {
    /// Coordenadas fijas en el mundo (`x, y`) sin seguir automáticamente a un personaje.
    #[default]
    Fixed,
    /// Apunta al Jugador activo aplicando `(x, y)` como desplazamiento de encuadre.
    Player,
    /// Apunta al Enemigo activo aplicando `(x, y)` como desplazamiento de encuadre.
    Opponent,
    /// Apunta a la Cabina / GF aplicando `(x, y)` como desplazamiento de encuadre.
    Gf,
    /// Apunta a una entidad específica del escenario (`target_entity_id`) con desplazamiento `(x, y)`.
    Entity,
}

impl CameraTargetKind {
    pub fn as_key(self) -> &'static str {
        match self {
            CameraTargetKind::Fixed => "fixed",
            CameraTargetKind::Player => "player",
            CameraTargetKind::Opponent => "opponent",
            CameraTargetKind::Gf => "gf",
            CameraTargetKind::Entity => "entity",
        }
    }

    pub fn from_str_loose(raw: &str) -> Self {
        let low = raw.trim().to_ascii_lowercase();
        match low.as_str() {
            "player" | "jugador" | "bf" | "boyfriend" | "1" => CameraTargetKind::Player,
            "opponent" | "enemigo" | "rival" | "dad" | "2" => CameraTargetKind::Opponent,
            "gf" | "girlfriend" | "cabina" | "speakers" => CameraTargetKind::Gf,
            "entity" | "entidad" | "objeto" | "target" => CameraTargetKind::Entity,
            _ => CameraTargetKind::Fixed,
        }
    }
}

/// Modo de seguimiento automático de la cámara durante la partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CameraFollowMode {
    /// Inicia en Cámara 0 fija y solo cambia cuando hay acciones/eventos de cámara en el chart o scripts.
    Camera0Fixed,
    /// Alterna entre Enemigo (Cam 2) y Jugador (Cam 1) cada N beats configurados.
    BeatsInterval,
    /// Sigue automáticamente `mustHitSection`, `gfSection`, `cameraSlot` y eventos del `chart.json`.
    #[default]
    ChartSections,
}

impl CameraFollowMode {
    pub fn as_lua_str(self) -> &'static str {
        match self {
            CameraFollowMode::Camera0Fixed => "fixed",
            CameraFollowMode::BeatsInterval => "beats",
            CameraFollowMode::ChartSections => "sections",
        }
    }

    pub fn from_str_loose(raw: &str) -> Self {
        let low = raw.trim().to_ascii_lowercase();
        if low.contains("fix") || low == "fija" || low == "cam0" || low == "camera0" {
            CameraFollowMode::Camera0Fixed
        } else if low.contains("beat") || low == "0" {
            CameraFollowMode::BeatsInterval
        } else {
            CameraFollowMode::ChartSections
        }
    }
}

/// Ranura individual dentro del sistema multicámara (`Cam 0` Fija, `Cam 1` Player, `Cam 2` Enemigo, `Cam 3+` personalizadas).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageCameraSlot {
    pub index: usize,
    pub name: String,
    #[serde(default)]
    pub target_kind: CameraTargetKind,
    #[serde(default)]
    pub target_entity_id: Option<String>,
    /// Coordenada X del mundo en modo `Fixed` o desplazamiento X de encuadre sobre el objetivo.
    pub x: f32,
    /// Coordenada Y del mundo en modo `Fixed` o desplazamiento Y de encuadre sobre el objetivo.
    pub y: f32,
    /// Campo de visión (zoom) específico para esta cámara (`0.35..2.5`).
    pub zoom: f32,
    /// Velocidad de transición suave al enfocarse esta cámara (`1.0..20.0`).
    pub smooth_speed: f32,
    /// Fuerza de seguimiento sobre el objetivo (`0.0` = permanece en Cámara 0, `1.0` = directo al objetivo).
    pub pan_strength: f32,
}

impl StageCameraSlot {
    pub fn sanitize(&mut self) {
        self.x = if self.x.is_finite() {
            self.x.clamp(-1000.0, 2600.0)
        } else {
            0.0
        };
        self.y = if self.y.is_finite() {
            self.y.clamp(-1000.0, 1800.0)
        } else {
            0.0
        };
        self.zoom = if self.zoom.is_finite() {
            self.zoom.clamp(0.35, 2.5)
        } else {
            1.0
        };
        self.smooth_speed = if self.smooth_speed.is_finite() {
            self.smooth_speed.clamp(1.0, 20.0)
        } else {
            6.0
        };
        self.pan_strength = if self.pan_strength.is_finite() {
            self.pan_strength.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if self.target_entity_id.as_deref().is_some_and(|s| s.trim().is_empty()) {
            self.target_entity_id = None;
        }
    }
}

/// Construye las 3 cámaras base (`Cam 0`, `Cam 1`, `Cam 2`).
pub fn default_camera_slots(
    base_x: f32,
    base_y: f32,
    base_zoom: f32,
    smooth_speed: f32,
) -> Vec<StageCameraSlot> {
    vec![
        StageCameraSlot {
            index: 0,
            name: "Cam 0 · Inicial Fija".to_string(),
            target_kind: CameraTargetKind::Fixed,
            target_entity_id: None,
            x: base_x.clamp(-1000.0, 2600.0),
            y: base_y.clamp(-1000.0, 1800.0),
            zoom: base_zoom.clamp(0.35, 2.5),
            smooth_speed: smooth_speed.clamp(1.0, 20.0),
            pan_strength: 0.0,
        },
        StageCameraSlot {
            index: 1,
            name: "Cam 1 · Jugador".to_string(),
            target_kind: CameraTargetKind::Player,
            target_entity_id: None,
            x: 0.0,
            y: -24.0,
            zoom: base_zoom.clamp(0.35, 2.5),
            smooth_speed: smooth_speed.clamp(1.0, 20.0),
            pan_strength: 1.0,
        },
        StageCameraSlot {
            index: 2,
            name: "Cam 2 · Enemigo".to_string(),
            target_kind: CameraTargetKind::Opponent,
            target_entity_id: None,
            x: 0.0,
            y: -24.0,
            zoom: base_zoom.clamp(0.35, 2.5),
            smooth_speed: smooth_speed.clamp(1.0, 20.0),
            pan_strength: 1.0,
        },
    ]
}

fn default_true() -> bool {
    true
}

fn default_vignette_color() -> String {
    "#EF4444".to_string()
}

pub(super) fn default_vignette_color_argb() -> u32 {
    0xFFEF4444
}

fn default_vignette_alpha() -> f32 {
    0.58
}

pub(super) fn default_vignette_width() -> f32 {
    0.24
}

pub(super) fn default_vignette_pulse() -> f32 {
    0.35
}

/// Convierte un color hexadecimal (`#RRGGBB`, `#AARRGGBB`, `0x...`) o nombre común en ARGB32.
pub fn parse_vignette_color_argb(raw: &str) -> u32 {
    let s = raw.trim();
    match s.to_ascii_lowercase().as_str() {
        "red" | "rojo" => return 0xFFEF4444,
        "crimson" | "carmesi" | "carmesí" => return 0xFFDC2626,
        "blue" | "azul" => return 0xFF3B82F6,
        "cyan" | "cian" => return 0xFF06B6D4,
        "purple" | "morado" | "violet" | "violeta" => return 0xFFA855F7,
        "amber" | "ambar" | "ámbar" | "orange" | "naranja" => return 0xFFF59E0B,
        "green" | "verde" => return 0xFF10B981,
        "pink" | "rosa" | "magenta" => return 0xFFEC4899,
        "yellow" | "amarillo" => return 0xFFFACC15,
        "white" | "blanco" => return 0xFFFFFFFF,
        "black" | "negro" => return 0xFF09090B,
        _ => {}
    }
    let hex = s
        .strip_prefix('#')
        .or_else(|| s.strip_prefix("0x"))
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    if hex.len() == 6 {
        if let Ok(rgb) = u32::from_str_radix(hex, 16) {
            return 0xFF00_0000 | rgb;
        }
    } else if hex.len() == 8 {
        if let Ok(argb) = u32::from_str_radix(hex, 16) {
            return argb;
        }
    }
    0xFFEF4444
}

/// Configuración persistente del sistema multicámara vinculada a `camera_system.lua`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageCameraConfig {
    pub enabled: bool,
    pub base_x: f32,
    pub base_y: f32,
    pub base_zoom: f32,
    pub follow_mode: CameraFollowMode,
    pub follow_interval_beats: u32,
    pub smooth_speed: f32,
    pub pan_strength: f32,
    #[serde(default)]
    pub active_camera_index: usize,
    #[serde(default = "default_true")]
    pub clamp_to_background: bool,
    #[serde(default)]
    pub cameras: Vec<StageCameraSlot>,
    #[serde(default)]
    pub vignette_enabled: bool,
    #[serde(default = "default_vignette_color")]
    pub vignette_color: String,
    #[serde(default = "default_vignette_alpha")]
    pub vignette_alpha: f32,
    #[serde(default = "default_vignette_width")]
    pub vignette_width: f32,
    #[serde(default = "default_vignette_pulse")]
    pub vignette_pulse: f32,
    #[serde(default)]
    pub vignette_over_hud: bool,
}

impl Default for StageCameraConfig {
    fn default() -> Self {
        let base_x = 640.0;
        let base_y = 360.0;
        let base_zoom = 1.0;
        let smooth_speed = 6.0;
        Self {
            enabled: true,
            base_x,
            base_y,
            base_zoom,
            follow_mode: CameraFollowMode::ChartSections,
            follow_interval_beats: 4,
            smooth_speed,
            pan_strength: 0.85,
            active_camera_index: 0,
            clamp_to_background: true,
            cameras: default_camera_slots(base_x, base_y, base_zoom, smooth_speed),
            vignette_enabled: false,
            vignette_color: default_vignette_color(),
            vignette_alpha: default_vignette_alpha(),
            vignette_width: default_vignette_width(),
            vignette_pulse: default_vignette_pulse(),
            vignette_over_hud: false,
        }
    }
}

impl StageCameraConfig {
    /// Garantiza que existan al menos `Cam 0` (Fija), `Cam 1` (Jugador) y `Cam 2` (Enemigo)
    /// y normaliza índices y límites numéricos en todas las ranuras.
    pub fn ensure_default_cameras(&mut self) {
        let defaults = default_camera_slots(self.base_x, self.base_y, self.base_zoom, self.smooth_speed);
        if self.cameras.is_empty() {
            self.cameras = defaults;
        } else {
            while self.cameras.len() < 3 {
                let next_idx = self.cameras.len();
                self.cameras.push(defaults[next_idx].clone());
            }
        }
        for (idx, cam) in self.cameras.iter_mut().enumerate() {
            cam.index = idx;
            if cam.name.trim().is_empty() {
                cam.name = match idx {
                    0 => "Cam 0 · Inicial Fija".to_string(),
                    1 => "Cam 1 · Jugador".to_string(),
                    2 => "Cam 2 · Enemigo".to_string(),
                    _ => format!("Cam {} · Personalizada", idx),
                };
            }
            cam.sanitize();
        }
        if self.active_camera_index >= self.cameras.len() {
            self.active_camera_index = 0;
        }
    }

    /// Normaliza color y parámetros de la viñeta/brillo lateral de cámara.
    pub fn sanitize_vignette(&mut self) {
        let trimmed = self.vignette_color.trim();
        if trimmed.is_empty() {
            self.vignette_color = default_vignette_color();
        } else {
            let argb = parse_vignette_color_argb(trimmed);
            self.vignette_color = format!("#{:06X}", argb & 0x00FF_FFFF);
        }
        self.vignette_alpha = if self.vignette_alpha.is_finite() {
            self.vignette_alpha.clamp(0.0, 1.0)
        } else {
            default_vignette_alpha()
        };
        self.vignette_width = if self.vignette_width.is_finite() {
            self.vignette_width.clamp(0.05, 0.50)
        } else {
            default_vignette_width()
        };
        self.vignette_pulse = if self.vignette_pulse.is_finite() {
            self.vignette_pulse.clamp(0.0, 1.0)
        } else {
            default_vignette_pulse()
        };
    }

    /// Sincroniza `Cam 0` desde `base_x`, `base_y`, `base_zoom` y `smooth_speed` cuando provienen de legacy/Lua.
    pub fn sync_slot0_from_base(&mut self) {
        self.ensure_default_cameras();
        self.sanitize_vignette();
        if let Some(cam0) = self.cameras.first_mut() {
            cam0.x = self.base_x.clamp(-1000.0, 2600.0);
            cam0.y = self.base_y.clamp(-1000.0, 1800.0);
            cam0.zoom = self.base_zoom.clamp(0.35, 2.5);
            cam0.smooth_speed = self.smooth_speed.clamp(1.0, 20.0);
        }
    }
}
