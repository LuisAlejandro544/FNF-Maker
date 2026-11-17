//! Archivo: engine/crates/pulse_scripting/src/bridge.rs
//! Propósito: Puente seguro entre las máquinas virtuales (Lua / QuickJS) y el motor Rust.
//! En lugar de mutar el escenario desde múltiples hilos, los scripts encolan `ScriptCommand`
//! y el hilo de juego los aplica determinísticamente cada frame.

use parking_lot::Mutex;
use std::collections::{HashMap, VecDeque};

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
    pub entity_positions: Mutex<HashMap<String, (f32, f32)>>,
    pub commands: Mutex<VecDeque<ScriptCommand>>,
    pub recent_logs: Mutex<VecDeque<String>>,
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
            entity_positions: Mutex::new(HashMap::new()),
            commands: Mutex::new(VecDeque::new()),
            recent_logs: Mutex::new(VecDeque::new()),
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

    pub fn drain_commands(&self) -> Vec<ScriptCommand> {
        self.commands.lock().drain(..).collect()
    }
}
