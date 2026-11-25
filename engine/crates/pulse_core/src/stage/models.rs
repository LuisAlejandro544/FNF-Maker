//! Archivo: engine/crates/pulse_core/src/stage/models.rs
//! Propósito: Define los roles semánticos (`EntityRole`) y el modelo de entidad visual
//! (`StageEntity`) junto con sus transformaciones espaciales, ajuste automático a imagen/atlas
//! y enlace de descriptores `.json`/`.xml` de personaje.

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
