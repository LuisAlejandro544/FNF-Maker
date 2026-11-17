//! Archivo: engine/crates/pulse_core/src/sparrow.rs
//! Propósito: Analiza hojas de sprites `.png` acompañadas de su descriptor Sparrow `.xml`.
//! En lugar de obligar al usuario a recortar y armar cada frame manualmente en una web externa,
//! Rust lee los `<SubTexture>` del `.xml`, conserva los offsets (`frameX`, `frameY`, `frameWidth`,
//! `frameHeight`) y clasifica automáticamente las animaciones de personaje:
//! `Idle`, `Left` (Izquierda), `Down` (Abajo), `Up` (Arriba), `Right` (Derecha) y poses Miss.

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
    /// Parsea el contenido de un archivo `.xml` Sparrow y arma automáticamente todas las
    /// secuencias de sprites y sus enlaces a Idle / Arriba / Abajo / Izquierda / Derecha.
    pub fn from_xml_str(xml_text: &str) -> Result<Self, SparrowError> {
        let doc = roxmltree::Document::parse(xml_text)?;
        let root = doc.root_element();
        let root_name = root.tag_name().name();
        if !matches!(root_name, "TextureAtlas" | "Atlas" | "ATLAS" | "spritemap") {
            return Err(SparrowError::MissingAtlasRoot);
        }

        let image_path_hint = root
            .attribute("imagePath")
            .or_else(|| root.attribute("image"))
            .unwrap_or("spritesheet.png")
            .to_string();

        let mut grouped: BTreeMap<String, Vec<SparrowFrame>> = BTreeMap::new();

        for node in root
            .descendants()
            .filter(|n| n.is_element() && matches!(n.tag_name().name(), "SubTexture" | "SPRITE" | "sprite" | "Sprite"))
        {
            let raw_name = match node.attribute("name") {
                Some(n) if !n.trim().is_empty() => n.trim().to_string(),
                _ => continue,
            };

            let x = parse_u32(node.attribute("x"));
            let y = parse_u32(node.attribute("y"));
            let width = parse_u32(node.attribute("width").or_else(|| node.attribute("w"))).max(1);
            let height = parse_u32(node.attribute("height").or_else(|| node.attribute("h"))).max(1);
            let frame_x = parse_i32(node.attribute("frameX"));
            let frame_y = parse_i32(node.attribute("frameY"));
            let rotated = node
                .attribute("rotated")
                .map(|r| r.eq_ignore_ascii_case("true") || r.trim() == "1")
                .unwrap_or(false);
            let visual_w = if rotated { height } else { width };
            let visual_h = if rotated { width } else { height };
            let frame_width = parse_u32(node.attribute("frameWidth")).max(visual_w);
            let frame_height = parse_u32(node.attribute("frameHeight")).max(visual_h);

            let prefix = strip_trailing_frame_digits(&raw_name);

            grouped.entry(prefix).or_default().push(SparrowFrame {
                raw_name,
                x,
                y,
                width,
                height,
                frame_x,
                frame_y,
                frame_width,
                frame_height,
                rotated,
            });
        }

        if grouped.is_empty() {
            return Err(SparrowError::EmptyAtlas);
        }

        let mut animations = BTreeMap::new();
        for (prefix, mut frames) in grouped {
            frames.sort_by(|a, b| natural_frame_cmp(&a.raw_name, &b.raw_name));
            let lower = prefix.to_ascii_lowercase();
            let looped = lower.contains("idle") || lower.contains("dance") || lower.contains("loop");
            animations.insert(
                prefix.clone(),
                AnimSet {
                    prefix,
                    fps: 24.0,
                    looped,
                    frames,
                },
            );
        }

        let bindings = Self::auto_detect_bindings(&animations);

        Ok(Self {
            image_path_hint,
            animations,
            bindings,
        })
    }

    /// Detecta automáticamente a qué dirección corresponde cada prefijo de animación del XML.
    /// Soporta convenciones en inglés y español (ej. `singLEFT`, `left`, `izquierda`, `idle`, `dance`).
    pub fn auto_detect_bindings(
        animations: &BTreeMap<String, AnimSet>,
    ) -> HashMap<CharacterAnimKind, String> {
        let mut bindings = HashMap::new();
        let prefixes: Vec<&String> = animations.keys().collect();

        for prefix in &prefixes {
            let low = prefix.to_ascii_lowercase();
            let is_miss = low.contains("miss") || low.contains("fallo");

            if !is_miss && (low.contains("idle") || low.contains("dance") || low.contains("reposo")) {
                bindings.entry(CharacterAnimKind::Idle).or_insert_with(|| (*prefix).clone());
            } else if low.contains("left") || low.contains("izq") {
                let kind = if is_miss { CharacterAnimKind::MissLeft } else { CharacterAnimKind::Left };
                bindings.entry(kind).or_insert_with(|| (*prefix).clone());
            } else if low.contains("down") || low.contains("abajo") {
                let kind = if is_miss { CharacterAnimKind::MissDown } else { CharacterAnimKind::Down };
                bindings.entry(kind).or_insert_with(|| (*prefix).clone());
            } else if low.contains("up") || low.contains("arriba") {
                let kind = if is_miss { CharacterAnimKind::MissUp } else { CharacterAnimKind::Up };
                bindings.entry(kind).or_insert_with(|| (*prefix).clone());
            } else if low.contains("right") || low.contains("der") {
                let kind = if is_miss { CharacterAnimKind::MissRight } else { CharacterAnimKind::Right };
                bindings.entry(kind).or_insert_with(|| (*prefix).clone());
            }
        }

        // Si es un objeto de fondo o decoración con una sola animación, enlazarla como Idle.
        if !bindings.contains_key(&CharacterAnimKind::Idle) {
            if let Some(first) = prefixes.first() {
                bindings.insert(CharacterAnimKind::Idle, (*first).clone());
            }
        }

        bindings
    }

    /// Devuelve el fotograma correspondiente al tiempo transcurrido de animación.
    pub fn frame_at(&self, kind: CharacterAnimKind, elapsed_sec: f32) -> Option<&SparrowFrame> {
        let prefix = self
            .bindings
            .get(&kind)
            .or_else(|| self.bindings.get(&CharacterAnimKind::Idle))?;
        let anim = self.animations.get(prefix)?;
        if anim.frames.is_empty() {
            return None;
        }
        let raw_idx = (elapsed_sec.max(0.0) * anim.fps.max(1.0)) as usize;
        let idx = if anim.looped {
            raw_idx % anim.frames.len()
        } else {
            raw_idx.min(anim.frames.len() - 1)
        };
        Some(&anim.frames[idx])
    }
}

/// Ordena nombres de frames numéricamente para que `idle2` preceda a `idle10`.
pub fn natural_frame_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let strip_ext = |s: &str| -> &str {
        for ext in [".png", ".jpg", ".jpeg", ".webp", ".astc"] {
            if s.len() > ext.len() && s[s.len() - ext.len()..].eq_ignore_ascii_case(ext) {
                return &s[..s.len() - ext.len()];
            }
        }
        s
    };
    let sa = strip_ext(a);
    let sb = strip_ext(b);
    let split_num = |s: &str| -> (&str, Option<u64>) {
        let idx = s
            .rfind(|c: char| !c.is_ascii_digit())
            .map(|i| i + 1)
            .unwrap_or(0);
        let (pref, digits) = s.split_at(idx);
        let num = if digits.is_empty() {
            None
        } else {
            digits.parse::<u64>().ok()
        };
        (pref, num)
    };
    let (pa, na) = split_num(sa);
    let (pb, nb) = split_num(sb);
    match pa.cmp(pb) {
        std::cmp::Ordering::Equal => match (na, nb) {
            (Some(va), Some(vb)) => va.cmp(&vb).then_with(|| a.cmp(b)),
            _ => a.cmp(b),
        },
        other => other,
    }
}

/// Elimina extensión de imagen y contador numérico final (ej. `Hero idle0000.png` -> `Hero idle`).
pub fn strip_trailing_frame_digits(name: &str) -> String {
    let mut base = name.trim();
    for ext in [".png", ".jpg", ".jpeg", ".webp", ".astc"] {
        if base.len() > ext.len() && base[base.len() - ext.len()..].eq_ignore_ascii_case(ext) {
            base = &base[..base.len() - ext.len()];
            break;
        }
    }
    let trimmed = base.trim_end_matches(|c: char| c.is_ascii_digit());
    let cleaned = trimmed.trim_end_matches('_').trim();
    if cleaned.is_empty() {
        base.to_string()
    } else {
        cleaned.to_string()
    }
}

fn parse_u32(val: Option<&str>) -> u32 {
    val.and_then(|s| s.trim().parse::<f32>().ok())
        .map(|f| f.max(0.0).round() as u32)
        .unwrap_or(0)
}

fn parse_i32(val: Option<&str>) -> i32 {
    val.and_then(|s| s.trim().parse::<f32>().ok())
        .map(|f| f.round() as i32)
        .unwrap_or(0)
}
