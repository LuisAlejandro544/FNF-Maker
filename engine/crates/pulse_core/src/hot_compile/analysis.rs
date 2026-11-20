//! Archivo: engine/crates/pulse_core/src/hot_compile/analysis.rs
//! Propósito: Analiza referencias de entidades en scripts/eventos del chart, detecta objetos
//! permanentemente invisibles o reutilizados entre escenas y calcula el reescalado inteligente
//! de texturas según resolución de pantalla y gama del dispositivo.

use super::fingerprints::DeviceResourceTier;
use crate::project::ModProject;
use crate::stage::{EntityRole, StageEntity};
use std::collections::HashSet;

/// Recorre las fuentes de todos los scripts activos Y los eventos del chart (`chart.events`)
/// para saber qué IDs de entidad podrían revelarse, intercambiarse o animarse durante la canción.
pub fn collect_script_referenced_ids(project: &ModProject) -> HashSet<String> {
    let mut referenced = HashSet::new();
    for script in project.scripts.iter().filter(|s| s.enabled) {
        for entity in &project.stage.entities {
            if script.source_code.contains(&entity.id) {
                referenced.insert(entity.id.clone());
            }
        }
    }
    for ev in &project.chart.events {
        for entity in &project.stage.entities {
            if ev.value1.contains(&entity.id)
                || ev.value2.contains(&entity.id)
                || ev.name.contains(&entity.id)
            {
                referenced.insert(entity.id.clone());
            }
        }
    }
    referenced
}

/// Analiza los eventos del chart (`Stage Swap`, `Preload Sprite`, etc.) para detectar qué sprites
/// que se ocultan/desactivan en un punto de la canción vuelven a aparecer más adelante.
/// Estos sprites siguen pudiendo precargarse bajo demanda en el segundo uso aunque hayan salido de RAM.
pub fn analyze_reused_entity_ids(project: &ModProject) -> HashSet<String> {
    let mut sorted_events = project.chart.events.clone();
    sorted_events.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));

    let mut hidden_at_some_point: HashSet<String> = HashSet::new();
    let mut reused_later: HashSet<String> = HashSet::new();

    for ev in &sorted_events {
        let lower_name = ev.name.trim().to_ascii_lowercase();
        if lower_name != "stage swap"
            && lower_name != "swap stage"
            && lower_name != "change character"
            && lower_name != "preload sprite"
        {
            continue;
        }
        let mut shown: Vec<String> = Vec::new();
        let mut hidden: Vec<String> = Vec::new();

        for part in ev.value1.split(';').chain(ev.value2.split(';')) {
            let trimmed = part.trim();
            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim().to_ascii_lowercase();
                let ids: Vec<String> = v
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                match key.as_str() {
                    "show" | "mostrar" | "player" | "jugador" | "opp" | "opponent" | "enemigo"
                    | "rival" => shown.extend(ids),
                    "hide" | "ocultar" => hidden.extend(ids),
                    _ => {}
                }
            } else if lower_name == "preload sprite" && !trimmed.is_empty() {
                shown.extend(
                    trimmed
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty()),
                );
            }
        }

        for id in &shown {
            if hidden_at_some_point.contains(id) {
                reused_later.insert(id.clone());
            }
        }
        for id in hidden {
            hidden_at_some_point.insert(id);
        }
    }

    reused_later
}

/// Determina si una entidad está garantizada a ser invisible durante toda la canción.
pub fn is_permanently_invisible(
    entity: &StageEntity,
    script_refs: &HashSet<String>,
    virtual_w: f32,
    virtual_h: f32,
) -> bool {
    let touched_by_script = script_refs.contains(&entity.id);
    if touched_by_script {
        return false;
    }

    if !entity.visible || entity.opacity <= 0.005 || entity.scale <= 0.005 {
        return true;
    }

    let half_w = (entity.width * entity.scale) * 0.5;
    let half_h = (entity.height * entity.scale) * 0.5;
    let margin = 450.0_f32;
    let out_x = (entity.x + half_w) < -margin || (entity.x - half_w) > (virtual_w + margin);
    let out_y = (entity.y + half_h) < -margin || (entity.y - half_h) > (virtual_h + margin);
    out_x || out_y
}

/// Calcula inteligentemente si una textura requiere bajar resolución o si ya es óptima.
/// Devuelve `1.0` cuando NO conviene bajar resolución de píxeles.
pub fn compute_smart_texture_scale(
    entity: &StageEntity,
    orig_w: u32,
    orig_h: u32,
    tier: DeviceResourceTier,
) -> f32 {
    if orig_w == 0 || orig_h == 0 {
        return 1.0;
    }
    let largest = orig_w.max(orig_h);
    let has_atlas = entity.atlas.is_some();
    let is_background = entity.role == EntityRole::Background;
    let tier_cap = tier.max_dim_for_role(has_atlas, is_background);

    // 1. Límite duro de arquitectura/GPU del dispositivo
    let mut target_ratio = if largest > tier_cap {
        (tier_cap as f32) / (largest as f32)
    } else {
        1.0
    };

    // 2. Evaluación inteligente de nitidez respecto al tamaño visible en pantalla (1280x720 + zoom)
    let display_max = (entity.width.max(entity.height) * entity.scale).max(128.0);
    if let Some(atlas) = &entity.atlas {
        let mut max_frame_side = 0u32;
        for anim in atlas.animations.values() {
            for frame in &anim.frames {
                max_frame_side = max_frame_side.max(frame.width.max(frame.height));
            }
        }
        // Conservar al menos ~2.2x de resolución por frame frente a su tamaño en pantalla para cero borrosidad
        if max_frame_side > 0 {
            let ideal_frame_max = (display_max * 2.2).max(512.0);
            if (max_frame_side as f32) > ideal_frame_max * 1.15 && largest > 1536 {
                let frame_ratio = (ideal_frame_max / (max_frame_side as f32)).clamp(0.45, 1.0);
                target_ratio = target_ratio.min(frame_ratio);
            }
        }
    } else if is_background {
        // Escenarios/Fondos: conservar nitidez Full HD (mínimo 1600..1920px según tier) para que
        // texto, líneas finas y zooms de cámara nunca se vean pixelados, reduciendo solo excesos 4K+.
        let ideal_bg_max = (display_max * 1.6).clamp(1600.0, tier_cap as f32);
        if (largest as f32) > ideal_bg_max * 1.12 {
            let bg_ratio = (ideal_bg_max / (largest as f32)).clamp(0.45, 1.0);
            target_ratio = target_ratio.min(bg_ratio);
        }
    } else {
        // Props estáticos: conservar al menos 1.5x del tamaño en pantalla (mínimo 960px)
        let ideal_static_max = (display_max * 1.5).max(960.0);
        if (largest as f32) > ideal_static_max * 1.2 {
            let static_ratio = (ideal_static_max / (largest as f32)).clamp(0.40, 1.0);
            target_ratio = target_ratio.min(static_ratio);
        }
    }

    // Si la reducción sería menor al 8% (ratio >= 0.92) y cabe en tier_cap, conserva resolución 1:1.
    if target_ratio >= 0.92 && largest <= tier_cap {
        1.0
    } else {
        target_ratio.clamp(0.25, 1.0)
    }
}
