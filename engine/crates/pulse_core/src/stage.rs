//! Archivo: engine/crates/pulse_core/src/stage.rs
//! Propósito: Define el escenario horizontal 2D y cada entidad visual colocada en él.
//! Soporta:
//! - Identificadores de rol (`Background`, `Player`, `Opponent`, `Decoration`) para que
//!   el motor sepa quién canta cada carril, qué es fondo animado y qué es decoración.
//! - Posicionamiento con botones de flecha (`nudge`), escala (grande/pequeño), opacidad
//!   (0.0 a 1.0), rotación, volteo horizontal y orden Z (poner encima o debajo de otro objeto).

use crate::atlas_json::{AnimateDocument, AnimateSpritemap};
use crate::sparrow::{CharacterAnimKind, SparrowAtlas};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Identificador semántico de un asset dentro del escenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EntityRole {
    /// Fondo del escenario (puede ser imagen estática o animación Sparrow XML en bucle).
    Background,
    /// Personaje controlado por el jugador (reacciona a notas del jugador).
    Player,
    /// Personaje rival / enemigo (reacciona a notas del oponente).
    Opponent,
    /// Decoración pura (altavoces, luces, utilería encima o debajo de personajes).
    Decoration,
}

impl EntityRole {
    pub const ALL: [EntityRole; 4] = [
        EntityRole::Background,
        EntityRole::Player,
        EntityRole::Opponent,
        EntityRole::Decoration,
    ];

    pub fn label_es(self) -> &'static str {
        match self {
            EntityRole::Background => "Fondo",
            EntityRole::Player => "Player (Jugador)",
            EntityRole::Opponent => "Enemigo (Rival)",
            EntityRole::Decoration => "Decoración",
        }
    }

    pub fn default_z_index(self) -> i32 {
        match self {
            EntityRole::Background => 0,
            EntityRole::Decoration => 10,
            EntityRole::Opponent => 20,
            EntityRole::Player => 30,
        }
    }
}

/// Representa un objeto/personaje colocado en el escenario.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageEntity {
    pub id: String,
    pub display_name: String,
    pub role: EntityRole,
    pub image_path: Option<PathBuf>,
    pub raw_image_path: Option<PathBuf>,
    pub xml_path: Option<PathBuf>,
    pub animate_json_path: Option<PathBuf>,
    pub atlas: Option<SparrowAtlas>,
    #[serde(skip)]
    pub animate_spritemap: Option<AnimateSpritemap>,
    #[serde(skip)]
    pub animate_doc: Option<AnimateDocument>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub opacity: f32,
    pub rotation_deg: f32,
    pub flip_x: bool,
    pub z_index: i32,
    pub visible: bool,
    pub scroll_factor: f32,
    pub active_anim: CharacterAnimKind,
    pub anim_timer_sec: f32,
}

impl StageEntity {
    pub fn new(id: impl Into<String>, display_name: impl Into<String>, role: EntityRole) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            role,
            image_path: None,
            raw_image_path: None,
            xml_path: None,
            animate_json_path: None,
            atlas: None,
            animate_spritemap: None,
            animate_doc: None,
            x: 640.0,
            y: 360.0,
            width: 220.0,
            height: 260.0,
            scale: 1.0,
            opacity: 1.0,
            rotation_deg: 0.0,
            flip_x: false,
            z_index: role.default_z_index(),
            visible: true,
            scroll_factor: if role == EntityRole::Background { 0.85 } else { 1.0 },
            active_anim: CharacterAnimKind::Idle,
            anim_timer_sec: 0.0,
        }
    }

    /// Mueve el objeto con precisión usando las flechas del editor de escenario.
    pub fn nudge(&mut self, dx: f32, dy: f32) {
        self.x = (self.x + dx).clamp(-1000.0, 2600.0);
        self.y = (self.y + dy).clamp(-1000.0, 1800.0);
    }

    /// Ajusta la escala manteniendo límites seguros para 32-bit y 64-bit GPU textures.
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale.clamp(0.05, 6.0);
    }

    /// Ajusta la opacidad entre invisible (0.0) y sólido (1.0).
    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }
}

/// Modo de seguimiento automático de la cámara durante la partida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CameraFollowMode {
    /// Alterna entre Enemigo y Jugador cada N beats configurados.
    #[default]
    BeatsInterval,
    /// Sigue automáticamente `mustHitSection` definido en cada sección del `chart.json`.
    ChartSections,
}

impl CameraFollowMode {
    pub fn as_lua_str(self) -> &'static str {
        match self {
            CameraFollowMode::BeatsInterval => "beats",
            CameraFollowMode::ChartSections => "sections",
        }
    }

    pub fn from_str_loose(raw: &str) -> Self {
        let low = raw.trim().to_ascii_lowercase();
        if low.contains("section") || low == "chart" || low == "1" {
            CameraFollowMode::ChartSections
        } else {
            CameraFollowMode::BeatsInterval
        }
    }
}

/// Configuración persistente de cámara vinculada al script `camera_system.lua`.
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
}

impl Default for StageCameraConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            base_x: 640.0,
            base_y: 360.0,
            base_zoom: 1.0,
            follow_mode: CameraFollowMode::BeatsInterval,
            follow_interval_beats: 4,
            smooth_speed: 6.0,
            pan_strength: 0.72,
        }
    }
}

fn default_camera_base_zoom() -> f32 {
    1.0
}

/// Escena horizontal lógica (coordenadas virtuales 1280x720 independientes de la resolución).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageScene {
    pub virtual_width: f32,
    pub virtual_height: f32,
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    #[serde(default = "default_camera_base_zoom")]
    pub camera_base_zoom: f32,
    pub camera_shake: f32,
    #[serde(default)]
    pub camera: StageCameraConfig,
    pub entities: Vec<StageEntity>,
}

impl Default for StageScene {
    fn default() -> Self {
        let mut bg = StageEntity::new("stage_bg", "Fondo Escenario", EntityRole::Background);
        bg.x = 640.0;
        bg.y = 360.0;
        bg.width = 1280.0;
        bg.height = 720.0;

        let mut deco = StageEntity::new("stage_speakers", "Cabina de Sonido", EntityRole::Decoration);
        deco.x = 640.0;
        deco.y = 430.0;
        deco.width = 240.0;
        deco.height = 180.0;
        deco.z_index = 15;

        let mut opp = StageEntity::new("opponent_main", "Rival Principal", EntityRole::Opponent);
        opp.x = 300.0;
        opp.y = 420.0;
        opp.width = 210.0;
        opp.height = 280.0;
        opp.z_index = 20;

        let mut player = StageEntity::new("player_main", "Jugador Principal", EntityRole::Player);
        player.x = 980.0;
        player.y = 430.0;
        player.width = 210.0;
        player.height = 260.0;
        player.z_index = 30;

        let cam_cfg = StageCameraConfig::default();

        Self {
            virtual_width: 1280.0,
            virtual_height: 720.0,
            camera_x: cam_cfg.base_x,
            camera_y: cam_cfg.base_y,
            camera_zoom: cam_cfg.base_zoom,
            camera_base_zoom: cam_cfg.base_zoom,
            camera_shake: 0.0,
            camera: cam_cfg,
            entities: vec![bg, deco, opp, player],
        }
    }
}

impl StageScene {
    /// Aplica una nueva configuración de cámara validando límites y sincronizando la vista base.
    pub fn apply_camera_config(&mut self, mut cfg: StageCameraConfig) {
        cfg.base_x = if cfg.base_x.is_finite() {
            cfg.base_x.clamp(-1000.0, 2600.0)
        } else {
            640.0
        };
        cfg.base_y = if cfg.base_y.is_finite() {
            cfg.base_y.clamp(-1000.0, 1800.0)
        } else {
            360.0
        };
        cfg.base_zoom = if cfg.base_zoom.is_finite() {
            cfg.base_zoom.clamp(0.35, 2.5)
        } else {
            1.0
        };
        cfg.follow_interval_beats = cfg.follow_interval_beats.clamp(1, 32);
        cfg.smooth_speed = if cfg.smooth_speed.is_finite() {
            cfg.smooth_speed.clamp(1.0, 20.0)
        } else {
            6.0
        };
        cfg.pan_strength = if cfg.pan_strength.is_finite() {
            cfg.pan_strength.clamp(0.0, 1.0)
        } else {
            0.72
        };

        self.camera_x = cfg.base_x;
        self.camera_y = cfg.base_y;
        self.camera_base_zoom = cfg.base_zoom;
        self.camera_zoom = cfg.base_zoom;
        self.camera = cfg;
    }

    /// Ordena las entidades por `z_index` ascendente para dibujar correctamente qué está encima y qué debajo.
    pub fn sort_by_z_order(&mut self) {
        self.entities.sort_by_key(|e| e.z_index);
    }

    pub fn bring_forward(&mut self, entity_id: &str) {
        if let Some(target) = self.entities.iter_mut().find(|e| e.id == entity_id) {
            target.z_index += 1;
        }
        self.sort_by_z_order();
    }

    pub fn send_backward(&mut self, entity_id: &str) {
        if let Some(target) = self.entities.iter_mut().find(|e| e.id == entity_id) {
            target.z_index -= 1;
        }
        self.sort_by_z_order();
    }

    pub fn bring_to_front(&mut self, entity_id: &str) {
        let max_z = self.entities.iter().map(|e| e.z_index).max().unwrap_or(0);
        if let Some(target) = self.entities.iter_mut().find(|e| e.id == entity_id) {
            target.z_index = max_z + 1;
        }
        self.sort_by_z_order();
    }

    pub fn send_to_back(&mut self, entity_id: &str) {
        let min_z = self.entities.iter().map(|e| e.z_index).min().unwrap_or(0);
        if let Some(target) = self.entities.iter_mut().find(|e| e.id == entity_id) {
            target.z_index = min_z - 1;
        }
        self.sort_by_z_order();
    }

    pub fn find_mut(&mut self, entity_id: &str) -> Option<&mut StageEntity> {
        self.entities.iter_mut().find(|e| e.id == entity_id)
    }
}
