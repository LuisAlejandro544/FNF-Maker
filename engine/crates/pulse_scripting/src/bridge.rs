//! Archivo: engine/crates/pulse_scripting/src/bridge.rs
//! Propósito: Puente seguro entre las máquinas virtuales (Lua / QuickJS) y el motor Rust.
//! Permite manipulación completa de entidades, strums (modcharts), cámara, HUD,
//! gráficos/textos dinámicos, tweens, temporizadores y pistas de audio,
//! manteniendo espejos sincronizados para lecturas instantáneas desde scripts.

use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};

/// Instantánea de propiedades de una entidad visible para getters síncronos en Lua/JS.
#[derive(Debug, Clone, Copy)]
pub struct EntityPropSnapshot {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub opacity: f32,
    pub angle: f32,
    pub visible: f32,
    pub flip_x: f32,
    pub z_index: f32,
    pub scroll_factor: f32,
}

impl Default for EntityPropSnapshot {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            scale: 1.0,
            opacity: 1.0,
            angle: 0.0,
            visible: 1.0,
            flip_x: 0.0,
            z_index: 0.0,
            scroll_factor: 1.0,
        }
    }
}

impl EntityPropSnapshot {
    pub fn get(&self, prop: &str) -> Option<f32> {
        match prop {
            "x" => Some(self.x),
            "y" => Some(self.y),
            "width" => Some(self.width),
            "height" => Some(self.height),
            "scale" => Some(self.scale),
            "opacity" | "alpha" => Some(self.opacity),
            "angle" | "rotation" | "rotationDeg" => Some(self.angle),
            "visible" => Some(self.visible),
            "flipX" | "flip_x" => Some(self.flip_x),
            "zIndex" | "z_index" | "z" => Some(self.z_index),
            "scrollFactor" | "scroll_factor" => Some(self.scroll_factor),
            _ => None,
        }
    }

    pub fn set(&mut self, prop: &str, value: f32) {
        if !value.is_finite() {
            return;
        }
        match prop {
            "x" => self.x = value,
            "y" => self.y = value,
            "width" => self.width = value.max(1.0),
            "height" => self.height = value.max(1.0),
            "scale" => self.scale = value.clamp(0.01, 10.0),
            "opacity" | "alpha" => self.opacity = value.clamp(0.0, 1.0),
            "angle" | "rotation" | "rotationDeg" => self.angle = value,
            "visible" => self.visible = if value > 0.0 { 1.0 } else { 0.0 },
            "flipX" | "flip_x" => self.flip_x = if value > 0.0 { 1.0 } else { 0.0 },
            "zIndex" | "z_index" | "z" => self.z_index = value,
            "scrollFactor" | "scroll_factor" => self.scroll_factor = value.clamp(0.0, 4.0),
            _ => {}
        }
    }
}

/// Instantánea de transformaciones de un receptor/carril (0..3 Rival, 4..7 Jugador).
#[derive(Debug, Clone, Copy)]
pub struct StrumPropSnapshot {
    pub offset_x: f32,
    pub offset_y: f32,
    pub angle: f32,
    pub scale: f32,
    pub alpha: f32,
}

impl Default for StrumPropSnapshot {
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            angle: 0.0,
            scale: 1.0,
            alpha: 1.0,
        }
    }
}

impl StrumPropSnapshot {
    pub fn get(&self, prop: &str) -> Option<f32> {
        match prop {
            "x" | "offsetX" | "offset_x" => Some(self.offset_x),
            "y" | "offsetY" | "offset_y" => Some(self.offset_y),
            "angle" | "rotation" => Some(self.angle),
            "scale" => Some(self.scale),
            "alpha" | "opacity" => Some(self.alpha),
            _ => None,
        }
    }

    pub fn set(&mut self, prop: &str, value: f32) {
        if !value.is_finite() {
            return;
        }
        match prop {
            "x" | "offsetX" | "offset_x" => self.offset_x = value.clamp(-1200.0, 1200.0),
            "y" | "offsetY" | "offset_y" => self.offset_y = value.clamp(-800.0, 800.0),
            "angle" | "rotation" => self.angle = value,
            "scale" => self.scale = value.clamp(0.05, 4.0),
            "alpha" | "opacity" => self.alpha = value.clamp(0.0, 1.0),
            _ => {}
        }
    }
}

/// Convierte un color hexadecimal o nombre simple en ARGB32 (`0xAARRGGBB`).
pub fn parse_color_argb(raw: &str) -> u32 {
    let s = raw.trim();
    match s.to_ascii_lowercase().as_str() {
        "red" | "rojo" => return 0xFFEF4444,
        "green" | "verde" => return 0xFF10B981,
        "blue" | "azul" => return 0xFF3B82F6,
        "cyan" | "cian" => return 0xFF22D3EE,
        "pink" | "rosa" => return 0xFFF472B6,
        "yellow" | "amarillo" => return 0xFFFACC15,
        "white" | "blanco" => return 0xFFFFFFFF,
        "black" | "negro" => return 0xFF000000,
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
    0xFFFFFFFF
}

/// Comandos que un script Lua, TS o JS puede enviar al motor en tiempo real.
#[derive(Debug, Clone)]
pub enum ScriptCommand {
    Log(String),
    SetCameraPos { x: f32, y: f32 },
    SetCameraZoom(f32),
    SetCameraBaseZoom(f32),
    ShakeCamera { intensity: f32 },
    SetEntityProp {
        entity_id: String,
        prop: String,
        value: f32,
    },
    PlayEntityAnim {
        entity_id: String,
        anim_name: String,
    },
    SetStrumProp {
        strum_idx: usize,
        prop: String,
        value: f32,
    },
    SetHealth(f32),
    AddHealth(f32),
    SetScore(i64),
    AddScore(i64),
    SetScrollSpeed(f64),
    SetDownscroll(bool),
    SetHudProp {
        prop: String,
        value: f32,
    },
    MakeGraphic {
        id: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color_argb: u32,
        is_hud: bool,
    },
    MakeText {
        id: String,
        text: String,
        x: f32,
        y: f32,
        font_size: f32,
        color_argb: u32,
        is_hud: bool,
    },
    SetTextString {
        id: String,
        text: String,
    },
    CloneSprite {
        id: String,
        source_id: String,
        x: f32,
        y: f32,
    },
    RemoveSprite(String),
    TweenEntityProp {
        tag: String,
        entity_id: String,
        prop: String,
        target: f32,
        duration_sec: f32,
        ease: String,
    },
    TweenCameraPos {
        tag: String,
        x: f32,
        y: f32,
        duration_sec: f32,
        ease: String,
    },
    TweenCameraZoom {
        tag: String,
        zoom: f32,
        duration_sec: f32,
        ease: String,
    },
    TweenStrumProp {
        tag: String,
        strum_idx: usize,
        prop: String,
        target: f32,
        duration_sec: f32,
        ease: String,
    },
    CancelTween(String),
    RunTimer {
        tag: String,
        duration_sec: f32,
    },
    CancelTimer(String),
    SetTrackVolume {
        slot: usize,
        gain: f32,
    },
    SetTrackMute {
        slot: usize,
        muted: bool,
    },
    FlashEffect {
        color_argb: u32,
        duration_sec: f32,
        start_alpha: f32,
        over_hud: bool,
    },
    PreloadEntity(String),
    RetireEntity(String),
    SwapEntity {
        old_id: String,
        new_id: String,
        dispose_old: bool,
    },
    SetActiveCharacter {
        role: String,
        entity_id: String,
        dispose_old: bool,
    },
}

/// Estado compartido accesible por todas las funciones nativas registradas en Lua y JS.
#[derive(Debug)]
pub struct ScriptHostState {
    pub song_pos_ms: Mutex<f64>,
    pub current_bpm: Mutex<f64>,
    pub camera_x: Mutex<f32>,
    pub camera_y: Mutex<f32>,
    pub camera_zoom: Mutex<f32>,
    pub camera_base_zoom: Mutex<f32>,
    pub must_hit_section: Mutex<bool>,
    pub health: Mutex<f32>,
    pub score: Mutex<i64>,
    pub combo: Mutex<u32>,
    pub misses: Mutex<u32>,
    pub hits: Mutex<u32>,
    pub accuracy: Mutex<f32>,
    pub scroll_speed: Mutex<f64>,
    pub downscroll: Mutex<bool>,
    pub entity_positions: Mutex<HashMap<String, (f32, f32)>>,
    pub entity_props: Mutex<HashMap<String, EntityPropSnapshot>>,
    pub strum_props: Mutex<[StrumPropSnapshot; 8]>,
    pub commands: Mutex<VecDeque<ScriptCommand>>,
    pub recent_logs: Mutex<VecDeque<String>>,
    pub runtime_errors: Mutex<VecDeque<String>>,
}

impl Default for ScriptHostState {
    fn default() -> Self {
        Self {
            song_pos_ms: Mutex::new(0.0),
            current_bpm: Mutex::new(120.0),
            camera_x: Mutex::new(640.0),
            camera_y: Mutex::new(360.0),
            camera_zoom: Mutex::new(1.0),
            camera_base_zoom: Mutex::new(1.0),
            must_hit_section: Mutex::new(false),
            health: Mutex::new(0.5),
            score: Mutex::new(0),
            combo: Mutex::new(0),
            misses: Mutex::new(0),
            hits: Mutex::new(0),
            accuracy: Mutex::new(100.0),
            scroll_speed: Mutex::new(1.0),
            downscroll: Mutex::new(false),
            entity_positions: Mutex::new(HashMap::new()),
            entity_props: Mutex::new(HashMap::new()),
            strum_props: Mutex::new([StrumPropSnapshot::default(); 8]),
            commands: Mutex::new(VecDeque::new()),
            recent_logs: Mutex::new(VecDeque::new()),
            runtime_errors: Mutex::new(VecDeque::new()),
        }
    }
}

impl ScriptHostState {
    pub fn push_command(&self, cmd: ScriptCommand) {
        if let ScriptCommand::Log(ref msg) = cmd {
            let mut logs = self.recent_logs.lock();
            if logs.len() >= 32 {
                logs.pop_front();
            }
            logs.push_back(msg.clone());
        }
        self.commands.lock().push_back(cmd);
    }

    pub fn push_runtime_error(&self, err: String) {
        let mut errs = self.runtime_errors.lock();
        if errs.iter().any(|existing| existing == &err) {
            return;
        }
        if errs.len() >= 12 {
            errs.pop_front();
        }
        errs.push_back(err);
    }

    pub fn drain_runtime_errors(&self) -> Vec<String> {
        self.runtime_errors.lock().drain(..).collect()
    }

    pub fn drain_commands(&self) -> Vec<ScriptCommand> {
        self.commands.lock().drain(..).collect()
    }
}
