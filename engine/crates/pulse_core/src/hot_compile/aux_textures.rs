//! Archivo: engine/crates/pulse_core/src/hot_compile/aux_textures.rs
//! Propósito: Compilación incremental en caliente y caché en disco (WebP VP8L) para
//! sprites de cuenta regresiva (`2 / Ready?`, `1 / Set?`, `¡YA! / Go`) e íconos de personaje (`Normal`/`Perdiendo`),
//! incluyendo registro PassThrough en manifiesto, escalado uniforme `Lanczos3` y ahorro de VRAM.

use super::fingerprints::{AssetFileFingerprint, DeviceResourceTier, fnv1a_64_str};
use super::manifest::{AuxTextureFingerprint, CachedAuxTextureEntry};
use crate::astc_loader::{
    WebpPixelPolicy, is_astc_path, is_webp_path, load_sprite_image, read_sprite_dimensions,
    save_compiled_webp,
};
use crate::project::ModProject;
use crate::stage::StageEntity;
use image::imageops::FilterType;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Límite máximo de lado para sprites de cuenta regresiva según perfil de dispositivo.
#[inline]
pub fn countdown_max_side_for_tier(tier: DeviceResourceTier) -> u32 {
    match tier {
        DeviceResourceTier::LowRam32Bit => 1024,
        DeviceResourceTier::Balanced | DeviceResourceTier::HighQuality => 1536,
    }
}

/// Límite máximo de lado para íconos de barra de vida según perfil y si es tira doble FNF (`width >= height * 1.85`).
#[inline]
pub fn icon_max_side_for_tier(tier: DeviceResourceTier, is_dual_strip: bool) -> u32 {
    if is_dual_strip {
        match tier {
            DeviceResourceTier::LowRam32Bit => 512,
            DeviceResourceTier::Balanced => 512,
            DeviceResourceTier::HighQuality => 640,
        }
    } else {
        match tier {
            DeviceResourceTier::LowRam32Bit => 256,
            DeviceResourceTier::Balanced => 384,
            DeviceResourceTier::HighQuality => 512,
        }
    }
}

/// Resultado de compilar de forma incremental todos los sprites de cuenta regresiva e íconos.
#[derive(Debug, Default)]
pub struct AuxTexturesCompileResult {
    pub countdown_paths: HashMap<usize, PathBuf>,
    pub icon_normal_paths: HashMap<String, PathBuf>,
    pub icon_losing_paths: HashMap<String, PathBuf>,
    pub next_cached_countdown: HashMap<usize, CachedAuxTextureEntry>,
    pub next_cached_icons: HashMap<String, CachedAuxTextureEntry>,
    pub compiled_countdown_count: usize,
    pub compiled_icon_count: usize,
    pub vram_saved_bytes: usize,
}

#[derive(Debug, Clone)]
enum AuxTarget {
    Countdown {
        slot_index: usize,
        src_path: PathBuf,
        has_frames: bool,
    },
    Icon {
        entity_id: String,
        is_losing: bool,
        src_path: PathBuf,
    },
}

#[derive(Debug)]
enum AuxOutcome {
    Countdown {
        slot_index: usize,
        out_path: PathBuf,
        entry: Option<CachedAuxTextureEntry>,
    },
    Icon {
        entity_id: String,
        is_losing: bool,
        key: String,
        out_path: PathBuf,
        entry: Option<CachedAuxTextureEntry>,
    },
}

/// Sanea un identificador para generar nombres de archivo `.webp` seguros sin colisiones.
fn sanitize_cache_token(raw: &str) -> String {
    let mut slug: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if slug.is_empty() {
        slug.push_str("asset");
    }
    if slug.len() > 36 {
        slug.truncate(36);
    }
    format!("{}_{:08x}", slug, (fnv1a_64_str(raw) & 0xffff_ffff) as u32)
}

/// Compila en paralelo (`rayon`) y cachea los sprites de cuenta regresiva (`2-1-¡YA!`) y los
/// íconos de personaje presentes en `active_entities`, registrando tanto WebPs codificados como
/// entradas `PassThrough` en los mapas del manifiesto incremental.
pub fn compile_countdown_and_icons_with_manifest(
    project: &ModProject,
    active_entities: &[StageEntity],
    tier: DeviceResourceTier,
    out_dir: Option<&Path>,
    prev_countdown: &HashMap<usize, CachedAuxTextureEntry>,
    prev_icons: &HashMap<String, CachedAuxTextureEntry>,
) -> AuxTexturesCompileResult {
    compile_countdown_and_icons_with_manifest_progress(
        project,
        active_entities,
        tier,
        out_dir,
        prev_countdown,
        prev_icons,
        |_, _| {},
    )
}

/// Variante de `compile_countdown_and_icons_with_manifest` con callback de progreso incremental
/// (`(completados, total)`) invocado tras procesar cada sprite de cuenta regresiva o ícono.
pub fn compile_countdown_and_icons_with_manifest_progress<F>(
    project: &ModProject,
    active_entities: &[StageEntity],
    tier: DeviceResourceTier,
    out_dir: Option<&Path>,
    prev_countdown: &HashMap<usize, CachedAuxTextureEntry>,
    prev_icons: &HashMap<String, CachedAuxTextureEntry>,
    on_item_done: F,
) -> AuxTexturesCompileResult
where
    F: Fn(usize, usize) + Send + Sync,
{
    let mut targets: Vec<AuxTarget> = Vec::new();

    for slot in &project.countdown.slots {
        if let Some(path) = slot.image_path.as_ref() {
            targets.push(AuxTarget::Countdown {
                slot_index: slot.slot_index,
                src_path: path.clone(),
                has_frames: !slot.frames.is_empty(),
            });
        }
    }

    for ent in active_entities {
        if let Some(normal) = ent.icon_normal_path.as_ref() {
            targets.push(AuxTarget::Icon {
                entity_id: ent.id.clone(),
                is_losing: false,
                src_path: normal.clone(),
            });
        }
        if let Some(losing) = ent.icon_losing_path.as_ref() {
            targets.push(AuxTarget::Icon {
                entity_id: ent.id.clone(),
                is_losing: true,
                src_path: losing.clone(),
            });
        }
    }

    if targets.is_empty() {
        return AuxTexturesCompileResult::default();
    }

    let total_targets = targets.len();
    let done_targets = AtomicUsize::new(0);
    let total_vram_saved = Arc::new(AtomicUsize::new(0));

    let outcomes: Vec<AuxOutcome> = targets
        .into_par_iter()
        .filter_map(|target| {
            let outcome = match target {
                AuxTarget::Countdown {
                    slot_index,
                    src_path,
                    has_frames,
                } => compile_countdown_slot(
                    slot_index,
                    &src_path,
                    has_frames,
                    tier,
                    out_dir,
                    prev_countdown.get(&slot_index),
                    &total_vram_saved,
                ),
                AuxTarget::Icon {
                    entity_id,
                    is_losing,
                    src_path,
                } => {
                    let key = if is_losing {
                        format!("{}::losing", entity_id)
                    } else {
                        format!("{}::normal", entity_id)
                    };
                    compile_character_icon(
                        &entity_id,
                        is_losing,
                        &key,
                        &src_path,
                        tier,
                        out_dir,
                        prev_icons.get(&key),
                        &total_vram_saved,
                    )
                }
            };
            let done = done_targets.fetch_add(1, Ordering::Relaxed) + 1;
            on_item_done(done, total_targets);
            outcome
        })
        .collect();

    let mut result = AuxTexturesCompileResult {
        vram_saved_bytes: total_vram_saved.load(Ordering::Relaxed),
        ..Default::default()
    };

    for item in outcomes {
        match item {
            AuxOutcome::Countdown {
                slot_index,
                out_path,
                entry,
            } => {
                result.countdown_paths.insert(slot_index, out_path);
                result.compiled_countdown_count += 1;
                if let Some(e) = entry {
                    result.next_cached_countdown.insert(slot_index, e);
                }
            }
            AuxOutcome::Icon {
                entity_id,
                is_losing,
                key,
                out_path,
                entry,
            } => {
                if is_losing {
                    result.icon_losing_paths.insert(entity_id, out_path);
                } else {
                    result.icon_normal_paths.insert(entity_id, out_path);
                }
                result.compiled_icon_count += 1;
                if let Some(e) = entry {
                    result.next_cached_icons.insert(key, e);
                }
            }
        }
    }

    result
}

fn compile_countdown_slot(
    slot_index: usize,
    src_path: &Path,
    has_frames: bool,
    tier: DeviceResourceTier,
    out_dir: Option<&Path>,
    prev_entry: Option<&CachedAuxTextureEntry>,
    total_vram_saved: &AtomicUsize,
) -> Option<AuxOutcome> {
    let max_side = countdown_max_side_for_tier(tier);
    let file_fp = AssetFileFingerprint::from_path(src_path);
    let fp_opt = file_fp.map(|file| AuxTextureFingerprint {
        file,
        tier_code: tier.as_code(),
        max_side,
        has_frames,
    });

    if out_dir.is_some() {
        if let (Some(fp), Some(cached)) = (fp_opt.as_ref(), prev_entry) {
            if &cached.fingerprint == fp
                && cached.compiled_webp_path.exists()
                && is_webp_path(&cached.compiled_webp_path)
            {
                total_vram_saved.fetch_add(cached.vram_saved_bytes, Ordering::Relaxed);
                return Some(AuxOutcome::Countdown {
                    slot_index,
                    out_path: cached.compiled_webp_path.clone(),
                    entry: Some(cached.clone()),
                });
            }
        }
    }

    let is_astc = is_astc_path(src_path);
    if !is_astc && is_webp_path(src_path) {
        if let Some((w, h)) = read_sprite_dimensions(src_path) {
            if w.max(h) <= max_side {
                let entry = if out_dir.is_some() {
                    fp_opt.map(|fingerprint| CachedAuxTextureEntry {
                        key: format!("countdown::{}", slot_index),
                        fingerprint,
                        compiled_webp_path: src_path.to_path_buf(),
                        downscaled: false,
                        vram_saved_bytes: 0,
                    })
                } else {
                    None
                };
                return Some(AuxOutcome::Countdown {
                    slot_index,
                    out_path: src_path.to_path_buf(),
                    entry,
                });
            }
        }
    }

    let img = load_sprite_image(src_path).ok()?;
    let orig_w = img.width().max(1);
    let orig_h = img.height().max(1);
    let orig_max = orig_w.max(orig_h);
    let (final_img, downscaled, saved) = if orig_max > max_side {
        let scale = (max_side as f32) / (orig_max as f32);
        let new_w = ((orig_w as f32) * scale).round().max(1.0) as u32;
        let new_h = ((orig_h as f32) * scale).round().max(1.0) as u32;
        let saved = (orig_w as usize * orig_h as usize * 4)
            .saturating_sub(new_w as usize * new_h as usize * 4);
        (
            img.resize_exact(new_w, new_h, FilterType::Lanczos3),
            true,
            saved,
        )
    } else {
        (img, false, 0)
    };

    let dir = out_dir?;
    let target_webp = dir.join(format!("countdown_slot_{}.webp", slot_index));
    save_compiled_webp(&final_img, &target_webp, WebpPixelPolicy::LosslessRgba).ok()?;
    total_vram_saved.fetch_add(saved, Ordering::Relaxed);
    let entry = fp_opt.map(|fingerprint| CachedAuxTextureEntry {
        key: format!("countdown::{}", slot_index),
        fingerprint,
        compiled_webp_path: target_webp.clone(),
        downscaled,
        vram_saved_bytes: saved,
    });
    Some(AuxOutcome::Countdown {
        slot_index,
        out_path: target_webp,
        entry,
    })
}

#[allow(clippy::too_many_arguments)]
fn compile_character_icon(
    entity_id: &str,
    is_losing: bool,
    key: &str,
    src_path: &Path,
    tier: DeviceResourceTier,
    out_dir: Option<&Path>,
    prev_entry: Option<&CachedAuxTextureEntry>,
    total_vram_saved: &AtomicUsize,
) -> Option<AuxOutcome> {
    let file_fp = AssetFileFingerprint::from_path(src_path);

    if out_dir.is_some() {
        if let (Some(fp), Some(cached)) = (file_fp.as_ref(), prev_entry) {
            if &cached.fingerprint.file == fp
                && cached.fingerprint.tier_code == tier.as_code()
                && cached.compiled_webp_path.exists()
                && is_webp_path(&cached.compiled_webp_path)
            {
                total_vram_saved.fetch_add(cached.vram_saved_bytes, Ordering::Relaxed);
                return Some(AuxOutcome::Icon {
                    entity_id: entity_id.to_string(),
                    is_losing,
                    key: key.to_string(),
                    out_path: cached.compiled_webp_path.clone(),
                    entry: Some(cached.clone()),
                });
            }
        }
    }

    let is_astc = is_astc_path(src_path);
    if !is_astc && is_webp_path(src_path) {
        if let Some((w, h)) = read_sprite_dimensions(src_path) {
            let is_dual_strip = (w as f32) >= (h as f32) * 1.85;
            let max_side = icon_max_side_for_tier(tier, is_dual_strip);
            if w.max(h) <= max_side {
                let entry = if out_dir.is_some() {
                    file_fp.map(|file| CachedAuxTextureEntry {
                        key: key.to_string(),
                        fingerprint: AuxTextureFingerprint {
                            file,
                            tier_code: tier.as_code(),
                            max_side,
                            has_frames: is_dual_strip,
                        },
                        compiled_webp_path: src_path.to_path_buf(),
                        downscaled: false,
                        vram_saved_bytes: 0,
                    })
                } else {
                    None
                };
                return Some(AuxOutcome::Icon {
                    entity_id: entity_id.to_string(),
                    is_losing,
                    key: key.to_string(),
                    out_path: src_path.to_path_buf(),
                    entry,
                });
            }
        }
    }

    let img = load_sprite_image(src_path).ok()?;
    let orig_w = img.width().max(1);
    let orig_h = img.height().max(1);
    let is_dual_strip = (orig_w as f32) >= (orig_h as f32) * 1.85;
    let max_side = icon_max_side_for_tier(tier, is_dual_strip);
    let orig_max = orig_w.max(orig_h);

    let (final_img, downscaled, saved) = if orig_max > max_side {
        let scale = (max_side as f32) / (orig_max as f32);
        let new_w = ((orig_w as f32) * scale).round().max(1.0) as u32;
        let new_h = ((orig_h as f32) * scale).round().max(1.0) as u32;
        let saved = (orig_w as usize * orig_h as usize * 4)
            .saturating_sub(new_w as usize * new_h as usize * 4);
        (
            img.resize_exact(new_w, new_h, FilterType::Lanczos3),
            true,
            saved,
        )
    } else {
        (img, false, 0)
    };

    let dir = out_dir?;
    let suffix = if is_losing { "losing" } else { "normal" };
    let target_webp = dir.join(format!("icon_{}_{}.webp", sanitize_cache_token(entity_id), suffix));
    save_compiled_webp(&final_img, &target_webp, WebpPixelPolicy::LosslessRgba).ok()?;
    total_vram_saved.fetch_add(saved, Ordering::Relaxed);
    let entry = file_fp.map(|file| CachedAuxTextureEntry {
        key: key.to_string(),
        fingerprint: AuxTextureFingerprint {
            file,
            tier_code: tier.as_code(),
            max_side,
            has_frames: is_dual_strip,
        },
        compiled_webp_path: target_webp.clone(),
        downscaled,
        vram_saved_bytes: saved,
    });
    Some(AuxOutcome::Icon {
        entity_id: entity_id.to_string(),
        is_losing,
        key: key.to_string(),
        out_path: target_webp,
        entry,
    })
}
