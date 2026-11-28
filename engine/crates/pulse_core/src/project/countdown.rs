//! Archivo: engine/crates/pulse_core/src/project/countdown.rs
//! Propósito: Modelo y utilidades para personalizar los 3 recursos de la cuenta regresiva
//! inicial estilo FNF (`2 / Ready?`, `1 / Set?`, `¡YA! / Go`): imágenes independientes
//! (PNG/WebP/ASTC), atlas Sparrow `.xml` opcionales por paso (o un `.xml` combinado) y pistas de
//! voz/SFX separadas (`intro2`, `intro1`, `introGo`), manteniendo fallback automático paso a paso
//! y migración compatible si se abre un proyecto antiguo de 4 slots.

use crate::sparrow::{SparrowAtlas, SparrowFrame};
use serde::{Deserialize, Deserializer, Serialize};
use std::cmp::Ordering;
use std::path::PathBuf;

/// Cantidad fija de pasos en la cuenta regresiva estilo FNF: `0 = 2 (Ready?)`, `1 = 1 (Set?)`, `2 = ¡YA! (Go)`.
pub const COUNTDOWN_STEP_COUNT: usize = 3;

/// Nombres legibles en español para cada paso de la cuenta regresiva.
pub fn countdown_step_label(slot_index: usize) -> &'static str {
    match slot_index {
        0 => "Paso 1 · \"2\" (Ready?)",
        1 => "Paso 2 · \"1\" (Set?)",
        _ => "Paso 3 · \"¡YA!\" (Go)",
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

/// Deserializa `slots` de 3 pasos (`2`, `1`, `¡YA!`) y migra automáticamente arreglos legados
/// de 4 pasos (`[3, Ready, Set, Go]` -> `[Ready, Set, Go]`) sin romper proyectos guardados.
fn deserialize_countdown_slots<'de, D>(
    deserializer: D,
) -> Result<[CountdownStepAsset; COUNTDOWN_STEP_COUNT], D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Vec::<CountdownStepAsset>::deserialize(deserializer)?;
    let mut out = [
        CountdownStepAsset::empty(0),
        CountdownStepAsset::empty(1),
        CountdownStepAsset::empty(2),
    ];

    if raw.len() >= 4 {
        for (dst_idx, src_idx) in (1..=3).enumerate() {
            let mut item = raw[src_idx].clone();
            item.slot_index = dst_idx;
            out[dst_idx] = item;
        }
    } else {
        for (idx, mut item) in raw.into_iter().take(COUNTDOWN_STEP_COUNT).enumerate() {
            item.slot_index = idx;
            out[idx] = item;
        }
    }
    Ok(out)
}

/// Contenedor serializable de los 3 pasos (`2 / Ready?`, `1 / Set?`, `¡YA! / Go`) dentro de `ModProject`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountdownAssetsConfig {
    #[serde(deserialize_with = "deserialize_countdown_slots")]
    pub slots: [CountdownStepAsset; COUNTDOWN_STEP_COUNT],
}

impl Default for CountdownAssetsConfig {
    fn default() -> Self {
        Self {
            slots: [
                CountdownStepAsset::empty(0),
                CountdownStepAsset::empty(1),
                CountdownStepAsset::empty(2),
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

/// Parsea un archivo `.xml` Sparrow para el paso `slot_index` (`0..=2`):
/// - Si el XML contiene múltiples animaciones (atlas combinado), prioriza los prefijos del paso
///   (`ready/two/2/dos`, `set/one/1/uno`, `go/ya/start`).
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
        0 => &["ready", "two", "dos", "listos", "intro2", "count2", "2"],
        1 => &["set", "one", "uno", "casi", "intro1", "count1", "1"],
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
        let frames = parse_countdown_sparrow_xml(xml, 0).expect("parse xml");
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

        let ready_frames = parse_countdown_sparrow_xml(xml, 0).unwrap();
        assert_eq!(ready_frames.len(), 2);
        assert!(ready_frames.iter().all(|f| f.raw_name.starts_with("ready")));

        let set_frames = parse_countdown_sparrow_xml(xml, 1).unwrap();
        assert_eq!(set_frames.len(), 1);
        assert_eq!(set_frames[0].raw_name, "set0000");

        let go_frames = parse_countdown_sparrow_xml(xml, 2).unwrap();
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
        let mut step = CountdownStepAsset::empty(2);
        step.frames = parse_countdown_sparrow_xml(xml, 2).unwrap();

        assert_eq!(step.frame_at_progress(0.0).unwrap().raw_name, "go0");
        assert_eq!(step.frame_at_progress(0.3).unwrap().raw_name, "go1");
        assert_eq!(step.frame_at_progress(0.6).unwrap().raw_name, "go2");
        assert_eq!(step.frame_at_progress(1.0).unwrap().raw_name, "go3");
        assert_eq!(step.frame_at_progress(f32::NAN).unwrap().raw_name, "go0");
    }

    #[test]
    fn migrates_legacy_four_slot_countdown_json_to_three_steps() {
        let legacy_json = r#"{
            "slots": [
                {"slot_index": 0, "image_name": "three.png", "audio_name": "intro3.ogg"},
                {"slot_index": 1, "image_name": "ready.png", "audio_name": "intro2.ogg"},
                {"slot_index": 2, "image_name": "set.png", "audio_name": "intro1.ogg"},
                {"slot_index": 3, "image_name": "go.png", "audio_name": "introGo.ogg"}
            ]
        }"#;
        let parsed: CountdownAssetsConfig =
            serde_json::from_str(legacy_json).expect("deserializa legado 4 slots");
        assert_eq!(parsed.slots.len(), 3);
        assert_eq!(parsed.slots[0].slot_index, 0);
        assert_eq!(parsed.slots[0].image_name.as_deref(), Some("ready.png"));
        assert_eq!(parsed.slots[0].audio_name.as_deref(), Some("intro2.ogg"));
        assert_eq!(parsed.slots[1].slot_index, 1);
        assert_eq!(parsed.slots[1].image_name.as_deref(), Some("set.png"));
        assert_eq!(parsed.slots[2].slot_index, 2);
        assert_eq!(parsed.slots[2].image_name.as_deref(), Some("go.png"));
        assert_eq!(parsed.slots[2].audio_name.as_deref(), Some("introGo.ogg"));
    }
}
