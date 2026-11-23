//! Archivo: engine/crates/pulse_core/src/stage.rs
//! Propósito: Define el escenario horizontal 2D y cada entidad visual colocada en él.
//! Soporta:
//! - Identificadores de rol (`Background`, `Player`, `Opponent`, `Decoration`) para que
//!   el motor sepa quién canta cada carril, qué es fondo animado y qué es decoración.
//! - Posicionamiento con botones de flecha (`nudge`), escala (grande/pequeño), opacidad
//!   (0.0 a 1.0), rotación, volteo horizontal y orden Z (poner encima o debajo de otro objeto).

use crate::atlas_json::{AnimateDocument, AnimateSpritemap};
use crate::mod_descriptors::{CharacterDescriptor, apply_character_position};
use crate::sparrow::{CharacterAnimKind, SparrowAtlas};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    #[serde(default)]
    pub character_config_path: Option<PathBuf>,
    #[serde(default)]
    pub icon_normal_path: Option<PathBuf>,
    #[serde(default)]
    pub icon_losing_path: Option<PathBuf>,
    pub atlas: Option<SparrowAtlas>,
    #[serde(skip)]
    pub animate_spritemap: Option<AnimateSpritemap>,
    #[serde(skip)]
    pub animate_doc: Option<AnimateDocument>,
    #[serde(default)]
    pub anim_offsets: HashMap<CharacterAnimKind, [f32; 2]>,
    #[serde(default)]
    pub camera_offset: [f32; 2],
    #[serde(default)]
    pub pending_character_desc: Option<CharacterDescriptor>,
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
        let (def_w, def_h) = match role {
            EntityRole::Background => (1280.0, 720.0),
            EntityRole::Player | EntityRole::Opponent => (280.0, 380.0),
            EntityRole::Decoration => (320.0, 280.0),
        };
        Self {
            id: id.into(),
            display_name: display_name.into(),
            role,
            image_path: None,
            raw_image_path: None,
            xml_path: None,
            animate_json_path: None,
            character_config_path: None,
            icon_normal_path: None,
            icon_losing_path: None,
            atlas: None,
            animate_spritemap: None,
            animate_doc: None,
            anim_offsets: HashMap::new(),
            camera_offset: [0.0, 0.0],
            pending_character_desc: None,
            x: 640.0,
            y: 360.0,
            width: def_w,
            height: def_h,
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

    /// Ajusta el desplazamiento visual (`offset_x`, `offset_y`) de una pose específica.
    pub fn set_anim_offset(&mut self, kind: CharacterAnimKind, offset_x: f32, offset_y: f32) {
        let ox = if offset_x.is_finite() {
            offset_x.clamp(-500.0, 500.0)
        } else {
            0.0
        };
        let oy = if offset_y.is_finite() {
            offset_y.clamp(-500.0, 500.0)
        } else {
            0.0
        };
        self.anim_offsets.insert(kind, [ox, oy]);
    }

    /// Ajusta `width` y `height` lógicos (en 1280x720) para una imagen suelta sin atlas,
    /// asegurando que los sprites importados en escenarios tengan tamaño visible y proporcionado.
    pub fn fit_dimensions_to_image(&mut self, img_w: u32, img_h: u32) {
        if img_w == 0 || img_h == 0 {
            return;
        }
        let w = img_w as f32;
        let h = img_h as f32;
        match self.role {
            EntityRole::Background => {
                let cover = (1280.0 / w).max(720.0 / h);
                self.width = (w * cover).clamp(1280.0, 2560.0);
                self.height = (h * cover).clamp(720.0, 1440.0);
            }
            EntityRole::Player | EntityRole::Opponent => {
                let fit = (520.0 / w).min(380.0 / h);
                self.width = (w * fit).clamp(140.0, 560.0);
                self.height = (h * fit).clamp(160.0, 460.0);
            }
            EntityRole::Decoration => {
                let fit = (560.0 / w).min(320.0 / h);
                self.width = (w * fit).clamp(120.0, 640.0);
                self.height = (h * fit).clamp(120.0, 420.0);
            }
        }
    }

    /// Ajusta `width` y `height` lógicos a partir de las dimensiones reales del frame unificado y
    /// la pose `Idle` del atlas, evitando que el sprite quede diminuto o aplastado por el tamaño
    /// total de la hoja spritesheet.
    pub fn fit_dimensions_to_atlas(&mut self) {
        let Some((unified_w, unified_h, _ref_w, ref_h)) = self
            .atlas
            .as_ref()
            .and_then(|a| a.reference_frame_dimensions())
        else {
            return;
        };
        let uw = unified_w.max(1) as f32;
        let uh = unified_h.max(1) as f32;
        let idle_h = ref_h.max(1) as f32;

        match self.role {
            EntityRole::Background => {
                let cover = (1280.0 / uw).max(720.0 / uh);
                self.width = (uw * cover).clamp(1280.0, 2560.0);
                self.height = (uh * cover).clamp(720.0, 1440.0);
            }
            EntityRole::Player | EntityRole::Opponent => {
                let fit = (380.0 / idle_h).min(640.0 / uw).min(520.0 / uh);
                self.width = (uw * fit).max(140.0);
                self.height = (uh * fit).max(160.0);
            }
            EntityRole::Decoration => {
                let fit = (320.0 / idle_h).min(640.0 / uw).min(460.0 / uh);
                self.width = (uw * fit).max(120.0);
                self.height = (uh * fit).max(120.0);
            }
        }
    }

    /// Aplica un descriptor `.json` / `.xml` de personaje sobre la entidad, enlazando prefijos,
    /// FPS, loops, offsets por animación, escala, volteo y (opcionalmente) posición en el escenario.
    /// Devuelve `(animaciones_enlazadas, offsets_aplicados)`.
    pub fn apply_character_descriptor(
        &mut self,
        desc: &CharacterDescriptor,
        apply_position: bool,
    ) -> (usize, usize) {
        if apply_position {
            if let Some(pos) = desc.position {
                let [nx, ny] =
                    apply_character_position([self.x, self.y], pos, desc.is_relative_position, self.role);
                self.x = nx;
                self.y = ny;
            }
        }
        if let Some(cam) = desc.camera_position {
            self.camera_offset = [
                if cam[0].is_finite() { cam[0] } else { 0.0 },
                if cam[1].is_finite() { cam[1] } else { 0.0 },
            ];
        }
        if let Some(scale) = desc.scale {
            self.set_scale(scale);
        }
        if let Some(flip_x) = desc.flip_x {
            self.flip_x = flip_x;
        }

        self.pending_character_desc = Some(desc.clone());

        let mut mapped_anims = 0usize;
        let mut applied_offsets = 0usize;

        for anim_desc in &desc.animations {
            let Some(kind) = anim_desc.anim_key else {
                continue;
            };
            self.set_anim_offset(kind, anim_desc.offset[0], anim_desc.offset[1]);
            applied_offsets += 1;

            let p_trim = anim_desc.prefix.trim();
            if p_trim.is_empty() {
                continue;
            }
            if let Some(atlas) = &mut self.atlas {
                let p_low = p_trim.to_ascii_lowercase();
                let matched_key = atlas
                    .animations
                    .keys()
                    .find(|k| *k == p_trim)
                    .or_else(|| atlas.animations.keys().find(|k| k.eq_ignore_ascii_case(p_trim)))
                    .or_else(|| {
                        atlas.animations.keys().find(|k| {
                            let kl = k.to_ascii_lowercase();
                            kl.starts_with(&p_low) || p_low.starts_with(&kl)
                        })
                    })
                    .cloned();

                if let Some(key) = matched_key {
                    atlas.bindings.insert(kind, key.clone());
                    if let Some(anim_set) = atlas.animations.get_mut(&key) {
                        anim_set.fps = anim_desc.fps.clamp(1.0, 120.0);
                        anim_set.looped = anim_desc.looped;
                    }
                    mapped_anims += 1;
                }
            }
        }

        (mapped_anims, applied_offsets)
    }
}

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

    /// Sincroniza `Cam 0` desde `base_x`, `base_y`, `base_zoom` y `smooth_speed` cuando provienen de legacy/Lua.
    pub fn sync_slot0_from_base(&mut self) {
        self.ensure_default_cameras();
        if let Some(cam0) = self.cameras.first_mut() {
            cam0.x = self.base_x.clamp(-1000.0, 2600.0);
            cam0.y = self.base_y.clamp(-1000.0, 1800.0);
            cam0.zoom = self.base_zoom.clamp(0.35, 2.5);
            cam0.smooth_speed = self.smooth_speed.clamp(1.0, 20.0);
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
        deco.y = 415.0;
        deco.width = 320.0;
        deco.height = 240.0;
        deco.z_index = 15;

        let mut opp = StageEntity::new("opponent_main", "Rival Principal", EntityRole::Opponent);
        opp.x = 300.0;
        opp.y = 415.0;
        opp.width = 280.0;
        opp.height = 380.0;
        opp.z_index = 20;

        let mut player = StageEntity::new("player_main", "Jugador Principal", EntityRole::Player);
        player.x = 980.0;
        player.y = 420.0;
        player.width = 280.0;
        player.height = 370.0;
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
    /// Aplica una nueva configuración de cámara validando límites, asegurando `Cam 0/1/2` y sincronizando la vista base.
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
            0.85
        };

        cfg.ensure_default_cameras();
        let (rx, ry, rzoom, _) =
            Self::resolve_slot_against_entities(&cfg, &self.entities, cfg.active_camera_index, "", "");

        self.camera_x = rx;
        self.camera_y = ry;
        self.camera_base_zoom = rzoom;
        self.camera_zoom = rzoom;
        self.camera = cfg;
    }

    fn resolve_entity_anchor<'a>(
        entities: &'a [StageEntity],
        kind: CameraTargetKind,
        target_entity_id: Option<&str>,
        active_player_id: &str,
        active_opponent_id: &str,
    ) -> Option<&'a StageEntity> {
        match kind {
            CameraTargetKind::Fixed => None,
            CameraTargetKind::Player => {
                if !active_player_id.is_empty() {
                    if let Some(e) = entities.iter().find(|e| e.id == active_player_id) {
                        return Some(e);
                    }
                }
                entities
                    .iter()
                    .find(|e| e.role == EntityRole::Player && e.visible)
                    .or_else(|| entities.iter().find(|e| e.role == EntityRole::Player))
            }
            CameraTargetKind::Opponent => {
                if !active_opponent_id.is_empty() {
                    if let Some(e) = entities.iter().find(|e| e.id == active_opponent_id) {
                        return Some(e);
                    }
                }
                entities
                    .iter()
                    .find(|e| e.role == EntityRole::Opponent && e.visible)
                    .or_else(|| entities.iter().find(|e| e.role == EntityRole::Opponent))
            }
            CameraTargetKind::Gf => entities
                .iter()
                .find(|e| {
                    e.role == EntityRole::Decoration
                        && (e.id.eq_ignore_ascii_case("stage_speakers")
                            || e.id.to_ascii_lowercase().contains("gf")
                            || e.display_name.to_ascii_lowercase().contains("cabina"))
                })
                .or_else(|| {
                    entities
                        .iter()
                        .find(|e| e.role == EntityRole::Decoration && e.visible)
                }),
            CameraTargetKind::Entity => target_entity_id
                .and_then(|id| entities.iter().find(|e| e.id == id))
                .or_else(|| entities.first()),
        }
    }

    /// Calcula el rectángulo límite `[min_left, min_top, max_right, max_bottom]` formado por las
    /// capas de fondo (`EntityRole::Background`) para evitar que la cámara muestre bordes negros vacíos.
    fn compute_background_bounds(entities: &[StageEntity]) -> Option<[f32; 4]> {
        let mut candidates: Vec<&StageEntity> = entities
            .iter()
            .filter(|e| e.role == EntityRole::Background && e.visible && e.opacity > 0.01)
            .collect();
        if candidates.is_empty() {
            candidates = entities
                .iter()
                .filter(|e| e.role == EntityRole::Background)
                .collect();
        }
        if candidates.is_empty() {
            return None;
        }

        let large_only: Vec<&StageEntity> = candidates
            .iter()
            .copied()
            .filter(|e| (e.width * e.scale).abs() >= 320.0 && (e.height * e.scale).abs() >= 180.0)
            .collect();
        let sized = if large_only.is_empty() {
            &candidates
        } else {
            &large_only
        };

        // Si existen capas de fondo con imagen/atlas real, ignoramos placeholders vacíos
        // (como el `stage_bg` sin textura) para que no amplíen los límites con huecos oscuros.
        let with_media: Vec<&StageEntity> = sized
            .iter()
            .copied()
            .filter(|e| {
                e.image_path.is_some()
                    || e.raw_image_path.is_some()
                    || e.xml_path.is_some()
                    || e.animate_json_path.is_some()
                    || e.atlas.is_some()
                    || e.animate_spritemap.is_some()
            })
            .collect();
        let chosen = if with_media.is_empty() {
            sized
        } else {
            &with_media
        };

        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for bg in chosen {
            let hw = (bg.width * bg.scale).abs() * 0.5;
            let hh = (bg.height * bg.scale).abs() * 0.5;
            min_x = min_x.min(bg.x - hw);
            max_x = max_x.max(bg.x + hw);
            min_y = min_y.min(bg.y - hh);
            max_y = max_y.max(bg.y + hh);
        }

        if min_x.is_finite() && min_y.is_finite() && max_x > min_x && max_y > min_y {
            Some([min_x, min_y, max_x, max_y])
        } else {
            None
        }
    }

    fn clamp_point_to_bounds(
        x: f32,
        y: f32,
        zoom: f32,
        virtual_w: f32,
        virtual_h: f32,
        bounds: Option<[f32; 4]>,
    ) -> (f32, f32) {
        let cx_world = if x.is_finite() {
            x.clamp(-1000.0, 2600.0)
        } else {
            640.0
        };
        let cy_world = if y.is_finite() {
            y.clamp(-1000.0, 1800.0)
        } else {
            360.0
        };
        let Some([min_x, min_y, max_x, max_y]) = bounds else {
            return (cx_world, cy_world);
        };
        let z = if zoom.is_finite() {
            zoom.clamp(0.35, 2.5)
        } else {
            1.0
        };
        let half_vw = (virtual_w.max(320.0) * 0.5) / z;
        let half_vh = (virtual_h.max(180.0) * 0.5) / z;

        let clamped_x = if min_x + half_vw <= max_x - half_vw {
            cx_world.clamp(min_x + half_vw, max_x - half_vw)
        } else {
            (min_x + max_x) * 0.5
        };
        let clamped_y = if min_y + half_vh <= max_y - half_vh {
            cy_world.clamp(min_y + half_vh, max_y - half_vh)
        } else {
            (min_y + max_y) * 0.5
        };
        (
            clamped_x.clamp(-1000.0, 2600.0),
            clamped_y.clamp(-1000.0, 1800.0),
        )
    }

    /// Devuelve el rectángulo delimitador del fondo actual (`[min_x, min_y, max_x, max_y]`).
    pub fn background_bounds(&self) -> Option<[f32; 4]> {
        Self::compute_background_bounds(&self.entities)
    }

    /// Devuelve el rectángulo delimitador del fondo a partir de una lista viva de entidades
    /// (por ejemplo `active_entities` durante `Stage Swap` en gameplay).
    pub fn background_bounds_for_entities(entities: &[StageEntity]) -> Option<[f32; 4]> {
        Self::compute_background_bounds(entities)
    }

    /// Restringe unas coordenadas `(x, y)` al área cubierta por el fondo del escenario según el `zoom`
    /// activo cuando `clamp_to_background` está habilitado.
    pub fn clamp_camera_to_background(&self, x: f32, y: f32, zoom: f32) -> (f32, f32) {
        self.clamp_camera_to_entities_background(x, y, zoom, &self.entities)
    }

    /// Restringe unas coordenadas `(x, y)` al área cubierta por el fondo vivo de `entities`
    /// (evitando franjas negras cuando scripts/eventos ocultan, reemplazan o mueven fondos).
    pub fn clamp_camera_to_entities_background(
        &self,
        x: f32,
        y: f32,
        zoom: f32,
        entities: &[StageEntity],
    ) -> (f32, f32) {
        if !self.camera.clamp_to_background {
            return (
                if x.is_finite() { x.clamp(-1000.0, 2600.0) } else { 640.0 },
                if y.is_finite() { y.clamp(-1000.0, 1800.0) } else { 360.0 },
            );
        }
        Self::clamp_point_to_bounds(
            x,
            y,
            zoom,
            self.virtual_width,
            self.virtual_height,
            Self::compute_background_bounds(entities),
        )
    }

    fn resolve_slot_against_entities(
        cfg: &StageCameraConfig,
        entities: &[StageEntity],
        slot_idx: usize,
        active_player_id: &str,
        active_opponent_id: &str,
    ) -> (f32, f32, f32, f32) {
        let cam0_x = cfg
            .cameras
            .first()
            .filter(|c| c.target_kind == CameraTargetKind::Fixed)
            .map(|c| c.x)
            .unwrap_or(cfg.base_x);
        let cam0_y = cfg
            .cameras
            .first()
            .filter(|c| c.target_kind == CameraTargetKind::Fixed)
            .map(|c| c.y)
            .unwrap_or(cfg.base_y);

        let bg_bounds = if cfg.clamp_to_background {
            Self::compute_background_bounds(entities)
        } else {
            None
        };

        let Some(slot) = cfg.cameras.get(slot_idx).or_else(|| cfg.cameras.first()) else {
            let z = cfg.base_zoom.clamp(0.35, 2.5);
            let (cx, cy) = Self::clamp_point_to_bounds(cfg.base_x, cfg.base_y, z, 1280.0, 720.0, bg_bounds);
            return (cx, cy, z, cfg.smooth_speed.clamp(1.0, 20.0));
        };

        let zoom = slot.zoom.clamp(0.35, 2.5);
        let smooth = slot.smooth_speed.clamp(1.0, 20.0);

        let (raw_x, raw_y) = if slot.target_kind == CameraTargetKind::Fixed {
            (slot.x.clamp(-1000.0, 2600.0), slot.y.clamp(-1000.0, 1800.0))
        } else if let Some(ent) = Self::resolve_entity_anchor(
            entities,
            slot.target_kind,
            slot.target_entity_id.as_deref(),
            active_player_id,
            active_opponent_id,
        ) {
            let dest_x = ent.x + ent.camera_offset[0] + slot.x;
            let dest_y = ent.y + ent.camera_offset[1] + slot.y;
            let pan = slot.pan_strength.clamp(0.0, 1.0);
            (
                (cam0_x + (dest_x - cam0_x) * pan).clamp(-1000.0, 2600.0),
                (cam0_y + (dest_y - cam0_y) * pan).clamp(-1000.0, 1800.0),
            )
        } else {
            (
                (cam0_x + slot.x).clamp(-1000.0, 2600.0),
                (cam0_y + slot.y).clamp(-1000.0, 1800.0),
            )
        };

        let (final_x, final_y) = if cfg.clamp_to_background {
            Self::clamp_point_to_bounds(raw_x, raw_y, zoom, 1280.0, 720.0, bg_bounds)
        } else {
            (raw_x, raw_y)
        };
        (final_x, final_y, zoom, smooth)
    }

    /// Resuelve coordenadas `(x, y, zoom, smooth_speed)` reales en escena para la cámara `slot_idx`.
    pub fn resolve_camera_slot_target(
        &self,
        slot_idx: usize,
        active_player_id: &str,
        active_opponent_id: &str,
    ) -> (f32, f32, f32, f32) {
        self.resolve_camera_slot_target_with_entities(
            slot_idx,
            &self.entities,
            active_player_id,
            active_opponent_id,
        )
    }

    /// Resuelve coordenadas `(x, y, zoom, smooth_speed)` usando el snapshot vivo `entities`
    /// (imprescindible durante gameplay tras eventos `Stage Swap` / `Change Character` / tweens).
    pub fn resolve_camera_slot_target_with_entities(
        &self,
        slot_idx: usize,
        entities: &[StageEntity],
        active_player_id: &str,
        active_opponent_id: &str,
    ) -> (f32, f32, f32, f32) {
        Self::resolve_slot_against_entities(
            &self.camera,
            entities,
            slot_idx,
            active_player_id,
            active_opponent_id,
        )
    }

    /// Actualiza los parámetros de una ranura de cámara (`Cam 0/1/2/3+`) y sincroniza los campos base si es `Cam 0`.
    #[allow(clippy::too_many_arguments)]
    pub fn update_camera_slot(
        &mut self,
        slot_index: usize,
        name: &str,
        target_kind: CameraTargetKind,
        target_entity_id: Option<String>,
        x: f32,
        y: f32,
        zoom: f32,
        smooth_speed: f32,
        pan_strength: f32,
    ) -> bool {
        self.camera.ensure_default_cameras();
        let Some(slot) = self.camera.cameras.get_mut(slot_index) else {
            return false;
        };
        if !name.trim().is_empty() {
            slot.name = name.trim().to_string();
        }
        slot.target_kind = target_kind;
        slot.target_entity_id = target_entity_id.filter(|s| !s.trim().is_empty());
        slot.x = x;
        slot.y = y;
        slot.zoom = zoom;
        slot.smooth_speed = smooth_speed;
        slot.pan_strength = pan_strength;
        slot.sanitize();

        if slot_index == 0 {
            self.camera.base_x = slot.x;
            self.camera.base_y = slot.y;
            self.camera.base_zoom = slot.zoom;
            self.camera.smooth_speed = slot.smooth_speed;
        }
        self.camera.active_camera_index = slot_index;
        let (rx, ry, rzoom, _) = self.resolve_camera_slot_target(slot_index, "", "");
        self.camera_x = rx;
        self.camera_y = ry;
        self.camera_base_zoom = rzoom;
        self.camera_zoom = rzoom;
        true
    }

    /// Añade una nueva cámara personalizada (`Cam 3`, `Cam 4`, ...) y devuelve su índice asignado.
    #[allow(clippy::too_many_arguments)]
    pub fn add_custom_camera(
        &mut self,
        name: &str,
        target_kind: CameraTargetKind,
        target_entity_id: Option<String>,
        x: f32,
        y: f32,
        zoom: f32,
        smooth_speed: f32,
        pan_strength: f32,
    ) -> usize {
        self.camera.ensure_default_cameras();
        let idx = self.camera.cameras.len();
        let clean_name = if name.trim().is_empty() {
            format!("Cam {} · Personalizada", idx)
        } else {
            name.trim().to_string()
        };
        let mut slot = StageCameraSlot {
            index: idx,
            name: clean_name,
            target_kind,
            target_entity_id: target_entity_id.filter(|s| !s.trim().is_empty()),
            x,
            y,
            zoom,
            smooth_speed,
            pan_strength,
        };
        slot.sanitize();
        self.camera.cameras.push(slot);
        self.camera.active_camera_index = idx;
        let (rx, ry, rzoom, _) = self.resolve_camera_slot_target(idx, "", "");
        self.camera_x = rx;
        self.camera_y = ry;
        self.camera_base_zoom = rzoom;
        self.camera_zoom = rzoom;
        idx
    }

    /// Elimina una cámara personalizada (`index >= 3`) preservando intactas `Cam 0`, `Cam 1` y `Cam 2`.
    pub fn remove_custom_camera(&mut self, slot_index: usize) -> bool {
        self.camera.ensure_default_cameras();
        if slot_index < 3 || slot_index >= self.camera.cameras.len() {
            return false;
        }
        self.camera.cameras.remove(slot_index);
        for (i, cam) in self.camera.cameras.iter_mut().enumerate() {
            cam.index = i;
        }
        if self.camera.active_camera_index >= self.camera.cameras.len() {
            self.camera.active_camera_index = self.camera.cameras.len().saturating_sub(1);
        }
        let active_idx = self.camera.active_camera_index;
        let (rx, ry, rzoom, _) = self.resolve_camera_slot_target(active_idx, "", "");
        self.camera_x = rx;
        self.camera_y = ry;
        self.camera_base_zoom = rzoom;
        self.camera_zoom = rzoom;
        true
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sparrow::SparrowAtlas;

    #[test]
    fn stage_import_sizes_characters_and_props_generously_without_sheet_squashing() {
        let mut char_ent = StageEntity::new("opp", "Oponente", EntityRole::Opponent);
        // Primero importa un spritesheet ancho 4096x2048
        char_ent.fit_dimensions_to_image(4096, 2048);
        assert!(char_ent.width >= 400.0);

        // Luego importa su XML con pose Idle de 240x360 y ataque ancho de 480x380
        let xml = r#"
        <TextureAtlas imagePath="opp.png">
            <SubTexture name="opp idle0000" x="0" y="0" width="240" height="360" />
            <SubTexture name="opp singLEFT0000" x="240" y="0" width="480" height="380" />
        </TextureAtlas>
        "#;
        char_ent.atlas = Some(SparrowAtlas::from_xml_str(xml).unwrap());
        char_ent.fit_dimensions_to_atlas();

        // Debe conservar altura cómoda (~380px para el Idle) y proporción real del lienzo unificado 480x380
        assert!(char_ent.height >= 360.0 && char_ent.height <= 420.0);
        assert!(char_ent.width > char_ent.height);

        // Un fondo importado siempre cubre mínimo 1280x720
        let mut bg = StageEntity::new("bg2", "Fondo", EntityRole::Background);
        bg.fit_dimensions_to_image(800, 600);
        assert!(bg.width >= 1280.0 && bg.height >= 720.0);
    }

    #[test]
    fn multi_camera_slots_resolve_targets_and_protect_base_three() {
        let mut stage = StageScene::default();
        stage.camera.ensure_default_cameras();
        assert_eq!(stage.camera.cameras.len(), 3);

        // Cam 0 fija en (640, 360)
        let (x0, y0, z0, _) = stage.resolve_camera_slot_target(0, "", "");
        assert_eq!((x0, y0, z0), (640.0, 360.0, 1.0));

        // Cam 1 sigue al Player (980, 420) + offset (0, -24)
        let (x1, y1, _, _) = stage.resolve_camera_slot_target(1, "player_main", "opponent_main");
        assert!((x1 - 980.0).abs() < 0.01);
        assert!((y1 - 396.0).abs() < 0.01);

        // Cam 2 sigue al Enemigo (300, 415) + offset (0, -24)
        let (x2, y2, _, _) = stage.resolve_camera_slot_target(2, "player_main", "opponent_main");
        assert!((x2 - 300.0).abs() < 0.01);
        assert!((y2 - 391.0).abs() < 0.01);

        // Crear Cam 3 personalizada apuntando a la Cabina/GF con zoom 1.35
        let idx3 = stage.add_custom_camera(
            "Cam 3 · Cabina",
            CameraTargetKind::Gf,
            None,
            0.0,
            -30.0,
            1.35,
            8.0,
            1.0,
        );
        assert_eq!(idx3, 3);
        let (x3, y3, z3, _) = stage.resolve_camera_slot_target(3, "", "");
        assert!((x3 - 640.0).abs() < 0.01);
        assert!((y3 - 385.0).abs() < 0.01);
        assert!((z3 - 1.35).abs() < 0.01);

        // No permite borrar Cam 0..2, sí permite borrar Cam 3
        assert!(!stage.remove_custom_camera(0));
        assert!(!stage.remove_custom_camera(2));
        assert!(stage.remove_custom_camera(3));
        assert_eq!(stage.camera.cameras.len(), 3);
    }

    #[test]
    fn swapped_active_entities_clamp_camera_without_exposing_black_bars() {
        let mut stage = StageScene::default();
        let mut bg_alt = StageEntity::new("bg_red", "Fondo Alterno", EntityRole::Background);
        bg_alt.x = 980.0;
        bg_alt.y = 360.0;
        bg_alt.width = 1600.0;
        bg_alt.height = 720.0;
        bg_alt.visible = false;
        bg_alt.image_path = Some(PathBuf::from("sprites/bg_red.png"));
        stage.entities.push(bg_alt);

        // Simula `active_entities` durante gameplay tras `Stage Swap`:
        // se oculta `stage_bg` ([0..1280]) y se muestra `bg_red` ([180..1780]).
        let mut active_entities = stage.entities.clone();
        active_entities[0].visible = false;
        active_entities.last_mut().unwrap().visible = true;

        let (clamped_x, clamped_y) =
            stage.clamp_camera_to_entities_background(240.0, 360.0, 1.0, &active_entities);
        // Con zoom 1.0 (half_vw = 640), el límite izquierdo de bg_red (180) exige camera_x >= 820.
        assert!((clamped_x - 820.0).abs() < 0.01);
        assert!((clamped_y - 360.0).abs() < 0.01);
    }
}


