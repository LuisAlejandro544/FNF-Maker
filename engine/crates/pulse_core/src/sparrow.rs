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

        let mut atlas = Self {
            image_path_hint,
            animations,
            bindings,
        };
        atlas.normalize_and_center_animations();
        Ok(atlas)
    }

    /// Normaliza y centra automáticamente todas las animaciones del atlas dentro de un lienzo
    /// lógico unificado `(unified_w, unified_h)` compartido por todo el spritesheet.
    /// Elimina márgenes vacíos asimétricos entre poses (`Idle` derecho vs `Left`/`Down`/`Up`/`Right`
    /// movidos), mantiene intacta la fluidez relativa interna entre frames de una misma animación,
    /// alinea horizontalmente al centro y verticalmente al suelo, y garantiza que el escalado (`fit`)
    /// no cambie de tamaño al alternar poses. Es idempotente y funciona en todos los formatos.
    pub fn normalize_and_center_animations(&mut self) {
        if self.animations.is_empty() {
            return;
        }

        struct AnimFrameLayout {
            prefix: String,
            span_w: u32,
            span_h: u32,
            rel_offsets: Vec<(i32, i32)>,
        }

        let mut layouts: Vec<AnimFrameLayout> = Vec::with_capacity(self.animations.len());
        let mut unified_w = 1_u32;
        let mut unified_h = 1_u32;

        for (prefix, anim) in &self.animations {
            if anim.frames.is_empty() {
                continue;
            }

            let vis_dims: Vec<(u32, u32)> = anim
                .frames
                .iter()
                .map(|f| {
                    let vw = if f.rotated { f.height } else { f.width }.max(1);
                    let vh = if f.rotated { f.width } else { f.height }.max(1);
                    (vw, vh)
                })
                .collect();

            let has_offsets = anim.frames.iter().zip(vis_dims.iter()).any(|(f, &(vw, vh))| {
                f.frame_x != 0 || f.frame_y != 0 || f.frame_width > vw || f.frame_height > vh
            });

            let (span_w, span_h, rel_offsets) = if !has_offsets {
                // Atlas sin frameX/frameY: centrar cada frame horizontalmente y alinearlo a la base.
                let mut max_w = 1_u32;
                let mut max_h = 1_u32;
                for &(vw, vh) in &vis_dims {
                    max_w = max_w.max(vw);
                    max_h = max_h.max(vh);
                }
                let rels = vis_dims
                    .iter()
                    .map(|&(vw, vh)| {
                        let rx = ((max_w.saturating_sub(vw)) as i32) / 2;
                        let ry = (max_h.saturating_sub(vh)) as i32;
                        (rx, ry)
                    })
                    .collect();
                (max_w, max_h, rels)
            } else {
                // Atlas con offsets: calcular la caja visual ajustada de la animación conservando
                // los desplazamientos relativos entre sus propios frames.
                let mut min_x = i32::MAX;
                let mut min_y = i32::MAX;
                let mut max_x = i32::MIN;
                let mut max_y = i32::MIN;
                let mut raw_coords = Vec::with_capacity(anim.frames.len());

                for (f, &(vw, vh)) in anim.frames.iter().zip(vis_dims.iter()) {
                    let vx0 = -f.frame_x;
                    let vy0 = -f.frame_y;
                    let vx1 = vx0.saturating_add(vw as i32);
                    let vy1 = vy0.saturating_add(vh as i32);
                    min_x = min_x.min(vx0);
                    min_y = min_y.min(vy0);
                    max_x = max_x.max(vx1);
                    max_y = max_y.max(vy1);
                    raw_coords.push((vx0, vy0));
                }

                let sw = (max_x.saturating_sub(min_x)).max(1) as u32;
                let sh = (max_y.saturating_sub(min_y)).max(1) as u32;
                let rels = raw_coords
                    .into_iter()
                    .map(|(vx0, vy0)| (vx0.saturating_sub(min_x), vy0.saturating_sub(min_y)))
                    .collect();
                (sw, sh, rels)
            };

            unified_w = unified_w.max(span_w);
            unified_h = unified_h.max(span_h);
            layouts.push(AnimFrameLayout {
                prefix: prefix.clone(),
                span_w,
                span_h,
                rel_offsets,
            });
        }

        for layout in layouts {
            let Some(anim) = self.animations.get_mut(&layout.prefix) else {
                continue;
            };
            let pad_x = ((unified_w.saturating_sub(layout.span_w)) as i32) / 2;
            let pad_y = (unified_h.saturating_sub(layout.span_h)) as i32;
            for (frame, (rx, ry)) in anim.frames.iter_mut().zip(layout.rel_offsets.into_iter()) {
                frame.frame_x = -(rx.saturating_add(pad_x));
                frame.frame_y = -(ry.saturating_add(pad_y));
                frame.frame_width = unified_w;
                frame.frame_height = unified_h;
            }
        }
    }

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
            let is_death = low.contains("death") || low.contains("dies") || low.contains("gameover") || low.contains("muerte");

            if is_death {
                if low.contains("confirm") || low.contains("retry") || low.contains("reintent") {
                    bindings.entry(CharacterAnimKind::DeathConfirm).or_insert_with(|| (*prefix).clone());
                } else if low.contains("loop") || low.contains("bucle") || low.contains("dead") {
                    bindings.entry(CharacterAnimKind::DeathLoop).or_insert_with(|| (*prefix).clone());
                } else {
                    bindings.entry(CharacterAnimKind::DeathStart).or_insert_with(|| (*prefix).clone());
                }
            } else if !is_miss && (low.contains("idle") || low.contains("dance") || low.contains("reposo")) {
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

/// Ordena nombres de frames numéricamente para que `idle2` preceda a `idle10`.
pub fn natural_frame_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    fn strip_ext(s: &str) -> &str {
        for ext in [".png", ".jpg", ".jpeg", ".webp", ".astc"] {
            if s.len() > ext.len() && s[s.len() - ext.len()..].eq_ignore_ascii_case(ext) {
                return &s[..s.len() - ext.len()];
            }
        }
        s
    }

    fn split_num(s: &str) -> (&str, Option<u64>) {
        let bytes = s.as_bytes();
        let mut idx = bytes.len();
        while idx > 0 && bytes[idx - 1].is_ascii_digit() {
            idx -= 1;
        }
        let (pref, digits) = s.split_at(idx);
        let num = if digits.is_empty() {
            None
        } else {
            digits.parse::<u64>().ok()
        };
        (pref, num)
    }

    let sa = strip_ext(a);
    let sb = strip_ext(b);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_centers_misaligned_sparrow_animations_idempotently() {
        let xml = r#"
        <TextureAtlas imagePath="hero.png">
            <SubTexture name="hero idle0000" x="0" y="0" width="200" height="300" frameX="-20" frameY="-10" frameWidth="240" frameHeight="320" />
            <SubTexture name="hero idle0001" x="200" y="0" width="196" height="304" frameX="-22" frameY="-6" frameWidth="240" frameHeight="320" />
            <SubTexture name="hero singLEFT0000" x="400" y="0" width="260" height="290" frameX="-260" frameY="-80" frameWidth="620" frameHeight="420" />
            <SubTexture name="hero singUP0000" x="700" y="0" width="180" height="360" />
        </TextureAtlas>
        "#;

        let mut atlas = SparrowAtlas::from_xml_str(xml).unwrap();
        let idle = &atlas.animations["hero idle"].frames;
        let left = &atlas.animations["hero singLEFT"].frames;
        let up = &atlas.animations["hero singUP"].frames;

        // Todos comparten el mismo lienzo lógico unificado (260x360).
        assert_eq!(idle[0].frame_width, 260);
        assert_eq!(idle[0].frame_height, 360);
        assert_eq!(left[0].frame_width, 260);
        assert_eq!(left[0].frame_height, 360);
        assert_eq!(up[0].frame_width, 260);
        assert_eq!(up[0].frame_height, 360);

        // singLEFT queda centrado en X (0..260 -> frame_x=0) y al suelo en Y (360-290=70 -> frame_y=-70)
        assert_eq!(left[0].frame_x, 0);
        assert_eq!(left[0].frame_y, -70);

        // singUP sin frameX/Y queda centrado en X ((260-180)/2=40 -> frame_x=-40) y Y=0
        assert_eq!(up[0].frame_x, -40);
        assert_eq!(up[0].frame_y, 0);

        // Idle queda centrado horizontalmente ((260-200)/2=30 -> frame_x=-30) y pies abajo
        assert_eq!(idle[0].frame_x, -30);
        assert_eq!(idle[0].frame_y, -60);
        assert_eq!(idle[1].frame_y, -56);

        // Idempotencia al volver a ejecutar sobre el atlas ya normalizado
        let before_fx = idle[0].frame_x;
        let before_fy = idle[0].frame_y;
        atlas.normalize_and_center_animations();
        assert_eq!(atlas.animations["hero idle"].frames[0].frame_x, before_fx);
        assert_eq!(atlas.animations["hero idle"].frames[0].frame_y, before_fy);
    }
}

