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

/// Escena horizontal lógica (coordenadas virtuales 1280x720 independientes de la resolución).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageScene {
    pub virtual_width: f32,
    pub virtual_height: f32,
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub camera_shake: f32,
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

        Self {
            virtual_width: 1280.0,
            virtual_height: 720.0,
            camera_x: 640.0,
            camera_y: 360.0,
            camera_zoom: 1.0,
            camera_shake: 0.0,
            entities: vec![bg, deco, opp, player],
        }
    }
}

impl StageScene {
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
