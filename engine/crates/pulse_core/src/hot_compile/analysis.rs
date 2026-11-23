//! Archivo: engine/crates/pulse_core/src/hot_compile/analysis.rs
//! Propósito: Analiza referencias de entidades en scripts/eventos del chart, detecta objetos
//! permanentemente invisibles o reutilizados entre escenas y calcula el reescalado inteligente
//! de texturas según resolución de pantalla y gama del dispositivo.

use super::fingerprints::{DeviceResourceTier, ScreenViewportMetrics};
use crate::chart::camera_events::parse_chart_camera_event;
use crate::chart::ParsedChartCameraAction;
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

/// Calcula el zoom máximo alcanzado por la cámara en toda la partida (ranuras `Cam 0..N` + eventos `ZoomCamera` / `Add Camera Zoom`).
pub fn compute_project_max_camera_zoom(project: &ModProject) -> f32 {
    let mut max_zoom = if project.stage.camera.base_zoom.is_finite() {
        project.stage.camera.base_zoom.clamp(0.35, 2.5)
    } else {
        1.0
    };
    for slot in &project.stage.camera.cameras {
        if slot.zoom.is_finite() {
            max_zoom = max_zoom.max(slot.zoom.clamp(0.35, 2.5));
        }
    }

    let mut max_bump = 0.0_f32;
    for ev in &project.chart.events {
        if let Some(action) =
            parse_chart_camera_event(&ev.name, &ev.value1, &ev.value2, project.chart.bpm)
        {
            match action {
                ParsedChartCameraAction::SelectCamera { zoom: Some(z), .. }
                | ParsedChartCameraAction::FollowPos { zoom: Some(z), .. }
                | ParsedChartCameraAction::SetZoom { zoom: z, .. } => {
                    if z.is_finite() {
                        max_zoom = max_zoom.max(z.clamp(0.35, 2.5));
                    }
                }
                ParsedChartCameraAction::AddZoomBump { cam_zoom_add, .. } => {
                    if cam_zoom_add.is_finite() && cam_zoom_add > 0.0 {
                        max_bump = max_bump.max(cam_zoom_add.min(0.25));
                    }
                }
                _ => {}
            }
        }
    }

    (max_zoom + max_bump).clamp(0.55, 2.5)
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

/// Calcula el pre-escalado inteligente al tamaño real en pantalla del teléfono (`viewport`)
/// multiplicado por el zoom máximo de cámara para aplicar mip-downscale limpio con `Lanczos3`.
pub fn compute_smart_texture_scale_for_viewport(
    entity: &StageEntity,
    orig_w: u32,
    orig_h: u32,
    tier: DeviceResourceTier,
    viewport: ScreenViewportMetrics,
    max_camera_zoom: f32,
    virtual_w: f32,
    virtual_h: f32,
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

    // 2. Tamaño físico real que ocupará la entidad en la pantalla del teléfono bajo zoom máximo
    let norm_vp = viewport.normalized();
    let px_scale = norm_vp.virtual_to_pixel_scale(virtual_w, virtual_h);
    let zoom_factor = if max_camera_zoom.is_finite() {
        max_camera_zoom.clamp(0.55, 2.5)
    } else {
        1.0
    };
    let screen_factor = (px_scale * zoom_factor).clamp(0.45, 3.2);
    let rendered_max_px = (entity.width.max(entity.height) * entity.scale * screen_factor).max(128.0);

    if let Some(atlas) = &entity.atlas {
        let mut max_frame_side = 0u32;
        for anim in atlas.animations.values() {
            for frame in &anim.frames {
                max_frame_side = max_frame_side.max(frame.width.max(frame.height));
            }
        }
        // Conservar ~1.45x sobre el tamaño real en píxeles (suficiente para rotación/zoom sin aliasing)
        if max_frame_side > 0 {
            let ideal_frame_max = (rendered_max_px * 1.45).max(384.0);
            if (max_frame_side as f32) > ideal_frame_max * 1.15 && max_frame_side > 384 {
                let frame_ratio = (ideal_frame_max / (max_frame_side as f32)).clamp(0.35, 1.0);
                target_ratio = target_ratio.min(frame_ratio);
            }
        }
    } else if is_background {
        let bg_floor = ((norm_vp.pixel_width as f32) * zoom_factor * 1.25).clamp(1152.0, tier_cap as f32);
        let ideal_bg_max = (rendered_max_px * 1.6).clamp(bg_floor, tier_cap as f32);
        if (largest as f32) > ideal_bg_max * 1.12 {
            let bg_ratio = (ideal_bg_max / (largest as f32)).clamp(0.35, 1.0);
            target_ratio = target_ratio.min(bg_ratio);
        }
    } else {
        let prop_floor = ((norm_vp.pixel_height as f32) * 0.9).clamp(480.0, 960.0);
        let ideal_static_max = (rendered_max_px * 1.5).max(prop_floor);
        if (largest as f32) > ideal_static_max * 1.15 {
            let static_ratio = (ideal_static_max / (largest as f32)).clamp(0.25, 1.0);
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

/// Wrapper compatible usando viewport lógico base `1280x720` y zoom `1.0`.
pub fn compute_smart_texture_scale(
    entity: &StageEntity,
    orig_w: u32,
    orig_h: u32,
    tier: DeviceResourceTier,
) -> f32 {
    compute_smart_texture_scale_for_viewport(
        entity,
        orig_w,
        orig_h,
        tier,
        ScreenViewportMetrics::default(),
        1.0,
        1280.0,
        720.0,
    )
}

/// Margen de anticipación (en milisegundos) antes del primer uso para precargar texturas ocultas en segundo plano.
pub const TIMELINE_PRELOAD_LEAD_MS: f64 = 4000.0;
/// Margen de cortesía después de ocultar una entidad antes de liberar su bitmap de RAM/VRAM.
pub const TIMELINE_RETIRE_GRACE_MS: f64 = 1500.0;

/// Plan temporal precomputado por el compilador en caliente para cargar antes de entrar a escena
/// y retirar de RAM después del último uso real de cada entidad visual.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityLifecycleWindow {
    pub entity_id: String,
    pub initially_visible: bool,
    pub preload_at_ms: Option<f64>,
    pub first_visible_ms: Option<f64>,
    pub last_used_ms: Option<f64>,
    pub retire_after_ms: Option<f64>,
    pub reused_later: bool,
    pub trigger_reason: String,
}

fn parse_stage_swap_targets(raw_name: &str, v1: &str, v2: &str) -> (Vec<String>, Vec<String>) {
    let lower_name = raw_name.trim().to_ascii_lowercase();
    let mut shown: Vec<String> = Vec::new();
    let mut hidden: Vec<String> = Vec::new();

    if lower_name != "stage swap"
        && lower_name != "swap stage"
        && lower_name != "change character"
        && lower_name != "preload sprite"
        && lower_name != "preload"
    {
        return (shown, hidden);
    }

    for part in v1.split(';').chain(v2.split(';')) {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim().to_ascii_lowercase();
            let ids: Vec<String> = v
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            match key.as_str() {
                "show" | "mostrar" | "player" | "jugador" | "opp" | "opponent" | "enemigo"
                | "rival" | "preload" => shown.extend(ids),
                "hide" | "ocultar" => hidden.extend(ids),
                _ => {}
            }
        } else if lower_name == "preload sprite" || lower_name == "preload" {
            shown.extend(
                trimmed
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
            );
        }
    }

    (shown, hidden)
}

/// Escanea visibilidad inicial, eventos ordenados del chart (`Stage Swap`, `Change Character`,
/// `Preload Sprite`, eventos de cámara/entidad) y scripts activos para generar la línea de vida
/// completa de cada entidad antes de iniciar el gameplay.
pub fn compute_timeline_asset_lifecycle(project: &ModProject) -> Vec<EntityLifecycleWindow> {
    let mut sorted_events = project.chart.events.clone();
    sorted_events.sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));

    let script_sources: Vec<&str> = project
        .scripts
        .iter()
        .filter(|s| s.enabled)
        .map(|s| s.source_code.as_str())
        .collect();

    let mut windows = Vec::with_capacity(project.stage.entities.len());

    for entity in &project.stage.entities {
        let id = entity.id.as_str();
        let initially_visible =
            entity.visible && entity.opacity > 0.005 && entity.scale > 0.005;
        let mut first_visible_ms = if initially_visible { Some(0.0) } else { None };
        let mut last_used_ms = if initially_visible { Some(0.0) } else { None };
        let mut hidden_at_ms: Option<f64> = None;
        let mut reused_later = false;
        let mut reasons: Vec<&'static str> = Vec::new();

        if initially_visible {
            reasons.push("visible_inicial");
        }

        for ev in &sorted_events {
            let t = ev.time_ms.max(0.0);
            let (shown, hidden) = parse_stage_swap_targets(&ev.name, &ev.value1, &ev.value2);
            let explicitly_shown = shown.iter().any(|s| s == id);
            let explicitly_hidden = hidden.iter().any(|s| s == id);

            if explicitly_shown {
                if hidden_at_ms.is_some() {
                    reused_later = true;
                }
                hidden_at_ms = None;
                if first_visible_ms.is_none() {
                    first_visible_ms = Some(t);
                }
                last_used_ms = Some(last_used_ms.map_or(t, |prev| prev.max(t)));
                if !reasons.contains(&"evento_chart") {
                    reasons.push("evento_chart");
                }
            } else if explicitly_hidden {
                hidden_at_ms = Some(t);
                last_used_ms = Some(last_used_ms.map_or(t, |prev| prev.max(t)));
                if !reasons.contains(&"evento_swap") {
                    reasons.push("evento_swap");
                }
            } else if ev.value1.contains(id) || ev.value2.contains(id) || ev.name.contains(id) {
                if hidden_at_ms.is_some() {
                    reused_later = true;
                    hidden_at_ms = None;
                }
                if first_visible_ms.is_none() {
                    first_visible_ms = Some(t);
                }
                last_used_ms = Some(last_used_ms.map_or(t, |prev| prev.max(t)));
                if !reasons.contains(&"ref_evento") {
                    reasons.push("ref_evento");
                }
            }
        }

        // Si un script activo referencia explícitamente esta entidad sin horario fijo en eventos,
        // evitamos retirarla prematuramente porque Lua/TS puede mostrarla en cualquier compás.
        let touched_by_script = script_sources.iter().any(|src| src.contains(id));
        if touched_by_script {
            if !reasons.contains(&"script_activo") {
                reasons.push("script_activo");
            }
            if hidden_at_ms.is_some() {
                reused_later = true;
            }
        }

        let preload_at_ms = if !initially_visible {
            first_visible_ms.map(|t_first| (t_first - TIMELINE_PRELOAD_LEAD_MS).max(0.0))
        } else {
            None
        };

        let retire_after_ms = if !touched_by_script {
            hidden_at_ms.map(|t_hide| t_hide + TIMELINE_RETIRE_GRACE_MS)
        } else {
            None
        };

        let trigger_reason = if reasons.is_empty() {
            "inactivo".to_string()
        } else {
            reasons.join("+")
        };

        windows.push(EntityLifecycleWindow {
            entity_id: entity.id.clone(),
            initially_visible,
            preload_at_ms,
            first_visible_ms,
            last_used_ms,
            retire_after_ms,
            reused_later,
            trigger_reason,
        });
    }

    windows
}
