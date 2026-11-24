//! Archivo: engine/crates/pulse_core/src/project/countdown.rs
//! Propósito: Modelo y utilidades para personalizar los 4 recursos de la cuenta regresiva
//! inicial (`3`, `2`, `1`, `¡YA!`): imágenes independientes (PNG/WebP/ASTC), atlas Sparrow `.xml`
//! opcionales por paso (o un `.xml` combinado) y pistas de voz/SFX separadas (`intro3`, `intro2`,
//! `intro1`, `introGo`), manteniendo fallback automático paso a paso si alguno falta.

use crate::sparrow::{SparrowAtlas, SparrowFrame};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::path::PathBuf;

/// Cantidad fija de pasos en la cuenta regresiva estilo FNF: `0 = 3`, `1 = 2`, `2 = 1`, `3 = ¡YA!`.
pub const COUNTDOWN_STEP_COUNT: usize = 4;

/// Nombres legibles en español para cada paso de la cuenta regresiva.
pub fn countdown_step_label(slot_index: usize) -> &'static str {
    match slot_index {
        0 => "Paso 1 · \"3\"",
        1 => "Paso 2 · \"2\" (Listos)",
        2 => "Paso 3 · \"1\" (Ya casi)",
        _ => "Paso 4 · \"¡YA!\" (Go)",
    }
}

/// Configuración de imagen, animación XML opcional y audio para un paso individual de cuenta regresiva.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountdownStepAsset {
    pub slot_index: usize,
    #[serde(default)]
    pub image_path: Option<PathBuf>,
    #[serde(default)]
    pub raw_image_path: Option<PathBuf>,
    #[serde(default)]
    pub image_name: Option<String>,
    #[serde(default)]
    pub xml_path: Option<PathBuf>,
    #[serde(default)]
    pub frames: Vec<SparrowFrame>,
    #[serde(default = "default_countdown_fps")]
    pub fps: f32,
    #[serde(default)]
    pub audio_path: Option<PathBuf>,
    #[serde(default)]
    pub audio_name: Option<String>,
}

fn default_countdown_fps() -> f32 {
    24.0
}

impl CountdownStepAsset {
    pub fn empty(slot_index: usize) -> Self {
        Self {
            slot_index: slot_index.min(COUNTDOWN_STEP_COUNT - 1),
            image_path: None,
            raw_image_path: None,
            image_name: None,
            xml_path: None,
            frames: Vec::new(),
            fps: default_countdown_fps(),
            audio_path: None,
            audio_name: None,
        }
    }

    pub fn has_custom_sprite(&self) -> bool {
        self.image_path.is_some()
    }

    pub fn has_custom_audio(&self) -> bool {
        self.audio_path.is_some()
    }

    /// Devuelve el fotograma del XML correspondiente al progreso normalizado `0.0..=1.0` dentro del beat.
    pub fn frame_at_progress(&self, progress: f32) -> Option<&SparrowFrame> {
        if self.frames.is_empty() {
            return None;
        }
        let clamped = if progress.is_finite() {
            progress.clamp(0.0, 0.9999)
        } else {
            0.0
        };
        let idx = ((clamped * self.frames.len() as f32).floor() as usize)
            .min(self.frames.len().saturating_sub(1));
        self.frames.get(idx)
    }

    /// Limpia únicamente la imagen y XML del paso, manteniendo el audio si existiera.
    pub fn clear_sprite(&mut self) {
        self.image_path = None;
        self.raw_image_path = None;
        self.image_name = None;
        self.xml_path = None;
        self.frames.clear();
        self.fps = default_countdown_fps();
    }

    /// Limpia únicamente el efecto de voz/audio del paso.
    pub fn clear_audio(&mut self) {
        self.audio_path = None;
        self.audio_name = None;
    }
}

/// Contenedor serializable de los 4 pasos (`3`, `2`, `1`, `¡YA!`) dentro de `ModProject`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountdownAssetsConfig {
    pub slots: [CountdownStepAsset; COUNTDOWN_STEP_COUNT],
}

impl Default for CountdownAssetsConfig {
    fn default() -> Self {
        Self {
            slots: [
                CountdownStepAsset::empty(0),
                CountdownStepAsset::empty(1),
                CountdownStepAsset::empty(2),
                CountdownStepAsset::empty(3),
            ],
        }
    }
}

impl CountdownAssetsConfig {
    pub fn sanitize(&mut self) {
        for (idx, slot) in self.slots.iter_mut().enumerate() {
            slot.slot_index = idx;
            if !slot.fps.is_finite() || slot.fps <= 0.5 {
                slot.fps = default_countdown_fps();
            } else {
                slot.fps = slot.fps.clamp(1.0, 120.0);
            }
        }
    }

    pub fn active_sprite_count(&self) -> usize {
        self.slots.iter().filter(|s| s.has_custom_sprite()).count()
    }

    pub fn active_audio_count(&self) -> usize {
        self.slots.iter().filter(|s| s.has_custom_audio()).count()
    }
}

/// Parsea un archivo `.xml` Sparrow para el paso `slot_index` (`0..=3`):
/// - Si el XML contiene múltiples animaciones (atlas combinado), prioriza los prefijos del paso
///   (`three/3/tres`, `two/ready/2/dos`, `one/set/1/uno`, `go/ya/start`).
/// - Si contiene una sola animación o ningún prefijo coincide con palabras clave, aplana todos
///   los `<SubTexture>` ordenados de forma natural por nombre.
pub fn parse_countdown_sparrow_xml(
    xml_text: &str,
    slot_index: usize,
) -> Result<Vec<SparrowFrame>, String> {
    let atlas = SparrowAtlas::from_xml_str(xml_text)
        .map_err(|e| format!("Error leyendo XML Sparrow de cuenta regresiva: {}", e))?;

    if atlas.animations.is_empty() {
        return Err("El archivo XML no contiene fotogramas <SubTexture>".to_string());
    }

    let keywords: &[&str] = match slot_index {
        0 => &["three", "tres", "intro3", "count3", "3"],
        1 => &["ready", "two", "dos", "listos", "intro2", "count2", "2"],
        2 => &["set", "one", "uno", "casi", "intro1", "count1", "1"],
        _ => &["go", "ya", "start", "introgo", "countgo"],
    };

    if atlas.animations.len() > 1 {
        let mut matched: Vec<SparrowFrame> = Vec::new();
        for (prefix, anim) in &atlas.animations {
            let lower = prefix.to_ascii_lowercase();
            let matches_kw = keywords.iter().any(|kw| {
                if kw.len() == 1 {
                    lower == *kw
                        || lower.ends_with(kw)
                        || lower.contains(&format!("_{}", kw))
                        || lower.contains(&format!(" {}", kw))
                        || lower.contains(&format!("-{}", kw))
                } else {
                    lower.contains(kw)
                }
            });
            if matches_kw {
                matched.extend(anim.frames.clone());
            }
        }
        if !matched.is_empty() {
            matched.sort_by(|a, b| natural_name_cmp(&a.raw_name, &b.raw_name));
            return Ok(matched);
        }
    }

    let mut all_frames: Vec<SparrowFrame> = atlas
        .animations
        .into_values()
        .flat_map(|anim| anim.frames.into_iter())
        .collect();
    all_frames.sort_by(|a, b| natural_name_cmp(&a.raw_name, &b.raw_name));
    Ok(all_frames)
}

fn natural_name_cmp(a: &str, b: &str) -> Ordering {
    let mut ai = a.as_bytes().iter().peekable();
    let mut bi = b.as_bytes().iter().peekable();

    while let (Some(&ca), Some(&cb)) = (ai.peek(), bi.peek()) {
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let mut na: u64 = 0;
            let mut la: usize = 0;
            while let Some(&&d) = ai.peek() {
                if !d.is_ascii_digit() {
                    break;
                }
                na = na.saturating_mul(10).saturating_add((d - b'0') as u64);
                la += 1;
                ai.next();
            }
            let mut nb: u64 = 0;
            let mut lb: usize = 0;
            while let Some(&&d) = bi.peek() {
                if !d.is_ascii_digit() {
                    break;
                }
                nb = nb.saturating_mul(10).saturating_add((d - b'0') as u64);
                lb += 1;
                bi.next();
            }
            match na.cmp(&nb) {
                Ordering::Equal => match la.cmp(&lb) {
                    Ordering::Equal => continue,
                    non_eq => return non_eq,
                },
                non_eq => return non_eq,
            }
        } else {
            let la = ca.to_ascii_lowercase();
            let lb = cb.to_ascii_lowercase();
            if la != lb {
                return la.cmp(&lb);
            }
            ai.next();
            bi.next();
        }
    }

    a.len().cmp(&b.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_step_countdown_sparrow_xml_in_natural_order() {
        let xml = r#"
        <TextureAtlas imagePath="ready.png">
            <SubTexture name="ready_0010" x="100" y="0" width="120" height="80" frameX="-2" frameY="-4" frameWidth="130" frameHeight="90"/>
            <SubTexture name="ready_0002" x="0" y="0" width="100" height="80" frameX="0" frameY="0" frameWidth="130" frameHeight="90"/>
            <SubTexture name="ready_0001" x="0" y="80" width="100" height="80" frameX="0" frameY="0" frameWidth="130" frameHeight="90"/>
        </TextureAtlas>
        "#;
        let frames = parse_countdown_sparrow_xml(xml, 1).expect("parse xml");
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].raw_name, "ready_0001");
        assert_eq!(frames[1].raw_name, "ready_0002");
        assert_eq!(frames[2].raw_name, "ready_0010");
    }

    #[test]
    fn selects_matching_prefix_from_combined_countdown_xml() {
        let xml = r#"
        <TextureAtlas imagePath="countdown_all.png">
            <SubTexture name="ready0000" x="0" y="0" width="100" height="60"/>
            <SubTexture name="ready0001" x="100" y="0" width="100" height="60"/>
            <SubTexture name="set0000" x="0" y="60" width="110" height="60"/>
            <SubTexture name="go0000" x="0" y="120" width="140" height="80"/>
            <SubTexture name="go0001" x="140" y="120" width="140" height="80"/>
        </TextureAtlas>
        "#;

        let ready_frames = parse_countdown_sparrow_xml(xml, 1).unwrap();
        assert_eq!(ready_frames.len(), 2);
        assert!(ready_frames.iter().all(|f| f.raw_name.starts_with("ready")));

        let set_frames = parse_countdown_sparrow_xml(xml, 2).unwrap();
        assert_eq!(set_frames.len(), 1);
        assert_eq!(set_frames[0].raw_name, "set0000");

        let go_frames = parse_countdown_sparrow_xml(xml, 3).unwrap();
        assert_eq!(go_frames.len(), 2);
        assert!(go_frames.iter().all(|f| f.raw_name.starts_with("go")));
    }

    #[test]
    fn frame_at_progress_maps_normalized_beat_range_safely() {
        let xml = r#"
        <TextureAtlas imagePath="go.png">
            <SubTexture name="go0" x="0" y="0" width="50" height="50"/>
            <SubTexture name="go1" x="50" y="0" width="50" height="50"/>
            <SubTexture name="go2" x="100" y="0" width="50" height="50"/>
            <SubTexture name="go3" x="150" y="0" width="50" height="50"/>
        </TextureAtlas>
        "#;
        let mut step = CountdownStepAsset::empty(3);
        step.frames = parse_countdown_sparrow_xml(xml, 3).unwrap();

        assert_eq!(step.frame_at_progress(0.0).unwrap().raw_name, "go0");
        assert_eq!(step.frame_at_progress(0.3).unwrap().raw_name, "go1");
        assert_eq!(step.frame_at_progress(0.6).unwrap().raw_name, "go2");
        assert_eq!(step.frame_at_progress(1.0).unwrap().raw_name, "go3");
        assert_eq!(step.frame_at_progress(f32::NAN).unwrap().raw_name, "go0");
    }
}
