//! Archivo: engine/crates/pulse_core/src/sparrow/models.rs
//! Propósito: Modelos de datos (`SparrowError`, `SparrowFrame`, `CharacterAnimKind`,
//! `AnimSet`, `SparrowAtlas`) y métodos de consulta por tiempo y dimensiones de referencia.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use thiserror::Error;

/// Errores al interpretar un archivo Sparrow XML.
#[derive(Debug, Error)]
pub enum SparrowError {
    #[error("Error leyendo XML Sparrow: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("El XML no contiene un nodo raíz <TextureAtlas> válido")]
    MissingAtlasRoot,
    #[error("No se encontraron nodos <SubTexture> en el XML")]
    EmptyAtlas,
}

/// Un fotograma individual recortado de la hoja de sprites `.png`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparrowFrame {
    pub raw_name: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub frame_x: i32,
    pub frame_y: i32,
    pub frame_width: u32,
    pub frame_height: u32,
    pub rotated: bool,
    #[serde(default)]
    pub page_index: u16,
}

/// Roles de animación estándar que el ensamblador automático detecta para un personaje.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CharacterAnimKind {
    Idle,
    Left,
    Down,
    Up,
    Right,
    MissLeft,
    MissDown,
    MissUp,
    MissRight,
    DeathStart,
    DeathLoop,
    DeathConfirm,
}

impl CharacterAnimKind {
    pub const CORE_FIVE: [CharacterAnimKind; 5] = [
        CharacterAnimKind::Idle,
        CharacterAnimKind::Left,
        CharacterAnimKind::Down,
        CharacterAnimKind::Up,
        CharacterAnimKind::Right,
    ];

    pub fn label_es(self) -> &'static str {
        match self {
            CharacterAnimKind::Idle => "Reposo (Idle)",
            CharacterAnimKind::Left => "Izquierda (Left)",
            CharacterAnimKind::Down => "Abajo (Down)",
            CharacterAnimKind::Up => "Arriba (Up)",
            CharacterAnimKind::Right => "Derecha (Right)",
            CharacterAnimKind::MissLeft => "Fallo Izquierda",
            CharacterAnimKind::MissDown => "Fallo Abajo",
            CharacterAnimKind::MissUp => "Fallo Arriba",
            CharacterAnimKind::MissRight => "Fallo Derecha",
            CharacterAnimKind::DeathStart => "Muerte Inicio (Dies)",
            CharacterAnimKind::DeathLoop => "Muerte Bucle (Loop)",
            CharacterAnimKind::DeathConfirm => "Muerte Reintentar (Confirm)",
        }
    }
}

/// Secuencia de animación agrupada por prefijo del XML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnimSet {
    pub prefix: String,
    pub fps: f32,
    pub looped: bool,
    pub frames: Vec<SparrowFrame>,
}

/// Atlas completo ya procesado y con asignaciones automáticas listas para previsualizar o jugar.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparrowAtlas {
    pub image_path_hint: String,
    pub animations: BTreeMap<String, AnimSet>,
    pub bindings: HashMap<CharacterAnimKind, String>,
}

impl SparrowAtlas {
    /// Devuelve `(unified_w, unified_h, ref_idle_w, ref_idle_h)` para dimensionar entidades
    /// en el escenario usando la altura visual real del `Idle` sin encoger el personaje por poses amplias.
    pub fn reference_frame_dimensions(&self) -> Option<(u32, u32, u32, u32)> {
        let first_frame = self.animations.values().flat_map(|a| a.frames.iter()).next()?;
        let first_vw = if first_frame.rotated {
            first_frame.height
        } else {
            first_frame.width
        }
        .max(1);
        let first_vh = if first_frame.rotated {
            first_frame.width
        } else {
            first_frame.height
        }
        .max(1);
        let unified_w = first_frame.frame_width.max(first_vw).max(1);
        let unified_h = first_frame.frame_height.max(first_vh).max(1);

        let ref_anim = self
            .resolve_prefix(CharacterAnimKind::Idle)
            .and_then(|p| self.animations.get(p))
            .or_else(|| self.animations.values().find(|a| !a.frames.is_empty()))?;

        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;
        for f in &ref_anim.frames {
            let vw = if f.rotated { f.height } else { f.width }.max(1) as i32;
            let vh = if f.rotated { f.width } else { f.height }.max(1) as i32;
            let vx0 = -f.frame_x;
            let vy0 = -f.frame_y;
            min_x = min_x.min(vx0);
            min_y = min_y.min(vy0);
            max_x = max_x.max(vx0 + vw);
            max_y = max_y.max(vy0 + vh);
        }

        let ref_w = if max_x > min_x {
            (max_x - min_x) as u32
        } else {
            unified_w
        };
        let ref_h = if max_y > min_y {
            (max_y - min_y) as u32
        } else {
            unified_h
        };
        Some((unified_w, unified_h, ref_w.max(1), ref_h.max(1)))
    }

    fn resolve_prefix(&self, kind: CharacterAnimKind) -> Option<&String> {
        self.bindings
            .get(&kind)
            .or_else(|| match kind {
                CharacterAnimKind::MissLeft => self.bindings.get(&CharacterAnimKind::Left),
                CharacterAnimKind::MissDown => self.bindings.get(&CharacterAnimKind::Down),
                CharacterAnimKind::MissUp => self.bindings.get(&CharacterAnimKind::Up),
                CharacterAnimKind::MissRight => self.bindings.get(&CharacterAnimKind::Right),
                CharacterAnimKind::DeathLoop => self.bindings.get(&CharacterAnimKind::DeathStart),
                CharacterAnimKind::DeathConfirm => self.bindings.get(&CharacterAnimKind::DeathLoop)
                    .or_else(|| self.bindings.get(&CharacterAnimKind::DeathStart)),
                _ => None,
            })
            .or_else(|| self.bindings.get(&CharacterAnimKind::Idle))
    }

    /// Devuelve la cantidad de frames asignados a una animación.
    pub fn frame_count(&self, kind: CharacterAnimKind) -> usize {
        self.resolve_prefix(kind)
            .and_then(|prefix| self.animations.get(prefix))
            .map(|anim| anim.frames.len())
            .unwrap_or(0)
    }

    /// Devuelve la duración total en segundos de una reproducción de la animación.
    pub fn anim_duration_sec(&self, kind: CharacterAnimKind) -> f32 {
        let Some(prefix) = self.resolve_prefix(kind) else {
            return 0.0;
        };
        let Some(anim) = self.animations.get(prefix) else {
            return 0.0;
        };
        if anim.frames.is_empty() {
            return 0.0;
        }
        (anim.frames.len() as f32) / anim.fps.max(1.0)
    }

    /// Devuelve el fotograma correspondiente al tiempo transcurrido de animación.
    pub fn frame_at(&self, kind: CharacterAnimKind, elapsed_sec: f32) -> Option<&SparrowFrame> {
        self.frame_at_synced(kind, elapsed_sec, false)
    }

    /// Devuelve el fotograma permitiendo sujetar (`force_clamp`) el último frame entre beats
    /// cuando el `Idle` se dispara al ritmo del BPM en gameplay.
    pub fn frame_at_synced(
        &self,
        kind: CharacterAnimKind,
        elapsed_sec: f32,
        force_clamp: bool,
    ) -> Option<&SparrowFrame> {
        let prefix = self.resolve_prefix(kind)?;
        let anim = self.animations.get(prefix)?;
        if anim.frames.is_empty() {
            return None;
        }
        let raw_idx = (elapsed_sec.max(0.0) * anim.fps.max(1.0)) as usize;
        let idx = if anim.looped && !force_clamp {
            raw_idx % anim.frames.len()
        } else {
            raw_idx.min(anim.frames.len() - 1)
        };
        Some(&anim.frames[idx])
    }
}
