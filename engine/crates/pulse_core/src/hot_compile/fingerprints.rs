//! Archivo: engine/crates/pulse_core/src/hot_compile/fingerprints.rs
//! Propósito: Define los perfiles de hardware (`DeviceResourceTier`), funciones hash FNV-1a
//! deterministas y estructuras de huellas incrementales (`AssetFileFingerprint`,
//! `TextureFingerprint`, `AudioMergeFingerprint`) para invalidación precisa de caché en disco.

use crate::project::AudioTrackSlot;
use crate::stage::StageEntity;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::UNIX_EPOCH;

/// Perfil de optimización según la memoria/arquitectura del teléfono del usuario.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceResourceTier {
    /// Teléfonos 32-bit (`armeabi-v7a`) o 2–3 GB RAM: atlas hasta 2560px y fondos estáticos hasta 1536px.
    LowRam32Bit,
    /// Teléfonos gama media 64-bit: protege atlas hasta 4096px y fondos hasta 2560px.
    Balanced,
    /// Gama alta: hasta 4096px sin pérdida visible.
    HighQuality,
}

impl DeviceResourceTier {
    pub fn as_code(self) -> u8 {
        match self {
            DeviceResourceTier::LowRam32Bit => 0,
            DeviceResourceTier::Balanced => 1,
            DeviceResourceTier::HighQuality => 2,
        }
    }

    pub fn max_texture_dim(self) -> u32 {
        match self {
            DeviceResourceTier::LowRam32Bit => 1536,
            DeviceResourceTier::Balanced => 2560,
            DeviceResourceTier::HighQuality => 4096,
        }
    }

    /// Los atlas multi-frame (Sparrow/Animate) dividen la hoja entre decenas de cuadros,
    /// y los fondos de escenario (`Background`) necesitan al menos Full HD (1920px) para evitar
    /// pixelación en texto fino o al hacer zoom con la cámara.
    pub fn max_dim_for_role(self, has_atlas: bool, is_background: bool) -> u32 {
        if has_atlas {
            match self {
                DeviceResourceTier::LowRam32Bit => 2560,
                DeviceResourceTier::Balanced => 4096,
                DeviceResourceTier::HighQuality => 4096,
            }
        } else if is_background {
            match self {
                DeviceResourceTier::LowRam32Bit => 1920,
                DeviceResourceTier::Balanced => 2560,
                DeviceResourceTier::HighQuality => 3840,
            }
        } else {
            self.max_texture_dim()
        }
    }

    /// Compatibilidad con llamadas existentes que no distinguen entre fondo y prop estático.
    pub fn max_dim_for_entity(self, has_atlas: bool) -> u32 {
        self.max_dim_for_role(has_atlas, false)
    }

    /// Presupuesto de memoria RAM recomendado para caché auxiliar según la gama del dispositivo.
    pub fn lru_cache_budget_bytes(self) -> usize {
        match self {
            DeviceResourceTier::LowRam32Bit => 24 * 1024 * 1024,
            DeviceResourceTier::Balanced => 64 * 1024 * 1024,
            DeviceResourceTier::HighQuality => 128 * 1024 * 1024,
        }
    }

    pub fn label_es(self) -> &'static str {
        match self {
            DeviceResourceTier::LowRam32Bit => "Económico (32-bit / <=1536–2560px)",
            DeviceResourceTier::Balanced => "Balanceado HD (Atlas <=4096px)",
            DeviceResourceTier::HighQuality => "Máxima Calidad (<=4096px)",
        }
    }
}

/// Hash determinista FNV-1a de 64 bits (sin dependencias externas ni colisiones triviales).
pub fn fnv1a_64_bytes(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
    }
    hash
}

/// Hash determinista FNV-1a de 64 bits sobre una cadena UTF-8.
pub fn fnv1a_64_str(s: &str) -> u64 {
    fnv1a_64_bytes(s.as_bytes())
}

/// Huella ligera de un archivo en disco combinando tamaño, fecha de modificación y hash FNV-1a
/// de bloques inicial/medio/final (o completo si pesa <= 192 KB) para máxima velocidad en ARM32.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetFileFingerprint {
    pub path: String,
    pub size_bytes: u64,
    pub modified_ms: u64,
    pub sample_hash: u64,
}

impl AssetFileFingerprint {
    pub fn from_path(path: &Path) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        let size_bytes = meta.len();
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let mut f = std::fs::File::open(path).ok()?;
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut feed = |chunk: &[u8]| {
            for &b in chunk {
                hash ^= b as u64;
                hash = hash.wrapping_mul(0x0000_0100_0000_01B3);
            }
        };

        const CHUNK: usize = 64 * 1024;
        if size_bytes <= (CHUNK as u64) * 3 {
            let mut buf = Vec::with_capacity(size_bytes as usize);
            f.read_to_end(&mut buf).ok()?;
            feed(&buf);
        } else {
            let mut buf = vec![0u8; CHUNK];
            let n1 = f.read(&mut buf).ok()?;
            feed(&buf[..n1]);

            let mid_off = size_bytes / 2;
            f.seek(SeekFrom::Start(mid_off)).ok()?;
            let n2 = f.read(&mut buf).ok()?;
            feed(&buf[..n2]);

            let tail_off = size_bytes.saturating_sub(CHUNK as u64);
            f.seek(SeekFrom::Start(tail_off)).ok()?;
            let n3 = f.read(&mut buf).ok()?;
            feed(&buf[..n3]);
        }

        Some(Self {
            path: path.to_string_lossy().into_owned(),
            size_bytes,
            modified_ms,
            sample_hash: hash,
        })
    }
}

/// Dimensiones reales en píxeles de la pantalla horizontal del dispositivo para ajustar mip-downscale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenViewportMetrics {
    pub pixel_width: u32,
    pub pixel_height: u32,
}

impl Default for ScreenViewportMetrics {
    fn default() -> Self {
        Self {
            pixel_width: 1280,
            pixel_height: 720,
        }
    }
}

impl ScreenViewportMetrics {
    pub fn normalized(self) -> Self {
        let a = self.pixel_width.clamp(320, 4096);
        let b = self.pixel_height.clamp(180, 4096);
        Self {
            pixel_width: a.max(b),
            pixel_height: a.min(b),
        }
    }

    /// Relación de escala entre el mundo lógico (`1280x720`) y los píxeles reales del teléfono.
    pub fn virtual_to_pixel_scale(self, virtual_w: f32, virtual_h: f32) -> f32 {
        let norm = self.normalized();
        let vw = virtual_w.max(320.0);
        let vh = virtual_h.max(180.0);
        let sx = (norm.pixel_width as f32) / vw;
        let sy = (norm.pixel_height as f32) / vh;
        sx.max(sy).clamp(0.45, 3.2)
    }
}

/// Huella completa de una textura en el escenario + configuración del dispositivo y viewport real.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextureFingerprint {
    pub file: AssetFileFingerprint,
    pub tier_code: u8,
    pub has_atlas: bool,
    pub entity_w_bits: u32,
    pub entity_h_bits: u32,
    pub scale_bits: u32,
    pub atlas_frame_hash: u64,
    #[serde(default)]
    pub referenced_anim_hash: u64,
    #[serde(default)]
    pub viewport_w: u32,
    #[serde(default)]
    pub viewport_h: u32,
    #[serde(default)]
    pub max_zoom_bits: u32,
}

impl TextureFingerprint {
    pub fn compute(entity: &StageEntity, path: &Path, tier: DeviceResourceTier) -> Option<Self> {
        Self::compute_with_refs(entity, path, tier, None)
    }

    pub fn compute_with_refs(
        entity: &StageEntity,
        path: &Path,
        tier: DeviceResourceTier,
        referenced_anim_prefixes: Option<&HashSet<String>>,
    ) -> Option<Self> {
        Self::compute_with_context(
            entity,
            path,
            tier,
            referenced_anim_prefixes,
            ScreenViewportMetrics::default(),
            1.0,
        )
    }

    pub fn compute_with_context(
        entity: &StageEntity,
        path: &Path,
        tier: DeviceResourceTier,
        referenced_anim_prefixes: Option<&HashSet<String>>,
        viewport: ScreenViewportMetrics,
        max_camera_zoom: f32,
    ) -> Option<Self> {
        let file = AssetFileFingerprint::from_path(path)?;
        let norm_vp = viewport.normalized();
        let norm_zoom = if max_camera_zoom.is_finite() {
            max_camera_zoom.clamp(0.55, 2.5)
        } else {
            1.0
        };
        let mut atlas_hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut ref_anim_hash: u64 = 0xcbf2_9ce4_8422_2325;
        if let Some(atlas) = &entity.atlas {
            let mut max_frame_w = 0u32;
            let mut max_frame_h = 0u32;
            let mut count = 0u64;
            for (prefix, anim) in &atlas.animations {
                for fr in &anim.frames {
                    max_frame_w = max_frame_w.max(fr.width);
                    max_frame_h = max_frame_h.max(fr.height);
                    count += 1;
                }
                if referenced_anim_prefixes.is_some_and(|set| set.contains(prefix)) {
                    for &b in prefix.as_bytes() {
                        ref_anim_hash ^= b as u64;
                        ref_anim_hash = ref_anim_hash.wrapping_mul(0x0000_0100_0000_01B3);
                    }
                    ref_anim_hash ^= 0xFF;
                    ref_anim_hash = ref_anim_hash.wrapping_mul(0x0000_0100_0000_01B3);
                }
            }
            for bytes in [
                max_frame_w.to_le_bytes().as_slice(),
                max_frame_h.to_le_bytes().as_slice(),
                count.to_le_bytes().as_slice(),
                (atlas.bindings.len() as u64).to_le_bytes().as_slice(),
            ] {
                for &b in bytes {
                    atlas_hash ^= b as u64;
                    atlas_hash = atlas_hash.wrapping_mul(0x0000_0100_0000_01B3);
                }
            }
        }
        Some(Self {
            file,
            tier_code: tier.as_code(),
            has_atlas: entity.atlas.is_some(),
            entity_w_bits: entity.width.to_bits(),
            entity_h_bits: entity.height.to_bits(),
            scale_bits: entity.scale.to_bits(),
            atlas_frame_hash: atlas_hash,
            referenced_anim_hash: ref_anim_hash,
            viewport_w: norm_vp.pixel_width,
            viewport_h: norm_vp.pixel_height,
            max_zoom_bits: norm_zoom.to_bits(),
        })
    }
}

/// Huella de una ranura de audio para invalidar mezcla WAV solo cuando cambia archivo, ganancia o mute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioSlotFingerprint {
    pub slot_index: usize,
    pub file: AssetFileFingerprint,
    pub gain_bits: u32,
    pub muted: bool,
    pub duck_on_miss: bool,
}

/// Huella global de mezcla de pistas hacia `compiled_mix.wav`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioMergeFingerprint {
    pub target_sample_rate: u32,
    pub slots: Vec<AudioSlotFingerprint>,
}

impl AudioMergeFingerprint {
    pub fn compute(slots: &[AudioTrackSlot], target_sample_rate: u32) -> Option<Self> {
        let mut active = Vec::new();
        for slot in slots {
            let Some(path) = slot.file_path.as_ref() else {
                continue;
            };
            let file = AssetFileFingerprint::from_path(path)?;
            active.push(AudioSlotFingerprint {
                slot_index: slot.slot_index,
                file,
                gain_bits: slot.gain.to_bits(),
                muted: slot.muted,
                duck_on_miss: slot.duck_on_player_miss,
            });
        }
        if active.len() <= 1 {
            return None;
        }
        Some(Self {
            target_sample_rate,
            slots: active,
        })
    }
}
