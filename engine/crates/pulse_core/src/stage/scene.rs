//! Archivo: engine/crates/pulse_core/src/stage/scene.rs
//! Propósito: Implementa `StageScene` en coordenadas lógicas 1280x720, incluyendo resolución
//! de objetivos de cámara, clamping contra fondos reales, orden Z y limpieza de entidades.

use super::camera::{
    CameraTargetKind, StageCameraConfig, StageCameraSlot, default_vignette_color_argb,
    default_vignette_pulse, default_vignette_width, parse_vignette_color_argb,
};
use super::models::{EntityRole, StageEntity};
use serde::{Deserialize, Serialize};

fn default_camera_base_zoom() -> f32 {
    1.0
}

/// Escena horizontal lógica (coordenadas virtuales 1280x720 independientes de la resolución).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StageScene {
    pub virtual_width: f32,
    pub virtual_height: f32,
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    #[serde(default = "default_camera_base_zoom")]
    pub camera_base_zoom: f32,
    pub camera_shake: f32,
    #[serde(default)]
    pub camera: StageCameraConfig,
    #[serde(default = "default_vignette_color_argb")]
    pub vignette_color_argb: u32,
    #[serde(default)]
    pub vignette_alpha: f32,
    #[serde(default = "default_vignette_width")]
    pub vignette_width: f32,
    #[serde(default = "default_vignette_pulse")]
    pub vignette_pulse: f32,
    #[serde(default)]
    pub vignette_over_hud: bool,
    pub entities: Vec<StageEntity>,
}

impl Default for StageScene {
    fn default() -> Self {
        let mut bg = StageEntity::new("stage_bg", "Fondo Escenario", EntityRole::Background);
        bg.x = 640.0;
        bg.y = 360.0;
        bg.width = 1280.0;
        bg.height = 720.0;

        let mut deco = StageEntity::new("stage_speakers", "Cabina de Sonido", EntityRole::Decoration);
        deco.x = 640.0;
        deco.y = 415.0;
        deco.width = 320.0;
        deco.height = 240.0;
        deco.z_index = 15;

        let mut opp = StageEntity::new("opponent_main", "Rival Principal", EntityRole::Opponent);
        opp.x = 300.0;
        opp.y = 415.0;
        opp.width = 280.0;
        opp.height = 380.0;
        opp.z_index = 20;

        let mut player = StageEntity::new("player_main", "Jugador Principal", EntityRole::Player);
        player.x = 980.0;
        player.y = 420.0;
        player.width = 280.0;
        player.height = 370.0;
        player.z_index = 30;

        let cam_cfg = StageCameraConfig::default();

        Self {
            virtual_width: 1280.0,
            virtual_height: 720.0,
            camera_x: cam_cfg.base_x,
            camera_y: cam_cfg.base_y,
            camera_zoom: cam_cfg.base_zoom,
            camera_base_zoom: cam_cfg.base_zoom,
            camera_shake: 0.0,
            vignette_color_argb: parse_vignette_color_argb(&cam_cfg.vignette_color),
            vignette_alpha: if cam_cfg.vignette_enabled { cam_cfg.vignette_alpha } else { 0.0 },
            vignette_width: cam_cfg.vignette_width,
            vignette_pulse: cam_cfg.vignette_pulse,
            vignette_over_hud: cam_cfg.vignette_over_hud,
            camera: cam_cfg,
            entities: vec![bg, deco, opp, player],
        }
    }
}

impl StageScene {
    /// Aplica una nueva configuración de cámara validando límites, asegurando `Cam 0/1/2` y sincronizando la vista base.
    pub fn apply_camera_config(&mut self, mut cfg: StageCameraConfig) {
        cfg.base_x = if cfg.base_x.is_finite() {
            cfg.base_x.clamp(-1000.0, 2600.0)
        } else {
            640.0
        };
        cfg.base_y = if cfg.base_y.is_finite() {
            cfg.base_y.clamp(-1000.0, 1800.0)
        } else {
            360.0
        };
        cfg.base_zoom = if cfg.base_zoom.is_finite() {
            cfg.base_zoom.clamp(0.35, 2.5)
        } else {
            1.0
        };
        cfg.follow_interval_beats = cfg.follow_interval_beats.clamp(1, 32);
        cfg.smooth_speed = if cfg.smooth_speed.is_finite() {
            cfg.smooth_speed.clamp(1.0, 20.0)
        } else {
            6.0
        };
        cfg.pan_strength = if cfg.pan_strength.is_finite() {
            cfg.pan_strength.clamp(0.0, 1.0)
        } else {
            0.85
        };

        cfg.ensure_default_cameras();
        cfg.sanitize_vignette();
        let (rx, ry, rzoom, _) =
            Self::resolve_slot_against_entities(&cfg, &self.entities, cfg.active_camera_index, "", "");

        self.camera_x = rx;
        self.camera_y = ry;
        self.camera_base_zoom = rzoom;
        self.camera_zoom = rzoom;
        self.vignette_color_argb = parse_vignette_color_argb(&cfg.vignette_color);
        self.vignette_alpha = if cfg.vignette_enabled { cfg.vignette_alpha } else { 0.0 };
        self.vignette_width = cfg.vignette_width;
        self.vignette_pulse = cfg.vignette_pulse;
        self.vignette_over_hud = cfg.vignette_over_hud;
        self.camera = cfg;
    }

    fn resolve_entity_anchor<'a>(
        entities: &'a [StageEntity],
        kind: CameraTargetKind,
        target_entity_id: Option<&str>,
        active_player_id: &str,
        active_opponent_id: &str,
    ) -> Option<&'a StageEntity> {
        match kind {
            CameraTargetKind::Fixed => None,
            CameraTargetKind::Player => {
                if !active_player_id.is_empty() {
                    if let Some(e) = entities.iter().find(|e| e.id == active_player_id) {
                        return Some(e);
                    }
                }
                entities
                    .iter()
                    .find(|e| e.role == EntityRole::Player && e.visible)
                    .or_else(|| entities.iter().find(|e| e.role == EntityRole::Player))
            }
            CameraTargetKind::Opponent => {
                if !active_opponent_id.is_empty() {
                    if let Some(e) = entities.iter().find(|e| e.id == active_opponent_id) {
                        return Some(e);
                    }
                }
                entities
                    .iter()
                    .find(|e| e.role == EntityRole::Opponent && e.visible)
                    .or_else(|| entities.iter().find(|e| e.role == EntityRole::Opponent))
            }
            CameraTargetKind::Gf => entities
                .iter()
                .find(|e| {
                    e.role == EntityRole::Decoration
                        && (e.id.eq_ignore_ascii_case("stage_speakers")
                            || e.id.to_ascii_lowercase().contains("gf")
                            || e.display_name.to_ascii_lowercase().contains("cabina"))
                })
                .or_else(|| {
                    entities
                        .iter()
                        .find(|e| e.role == EntityRole::Decoration && e.visible)
                }),
            CameraTargetKind::Entity => target_entity_id
                .and_then(|id| entities.iter().find(|e| e.id == id))
                .or_else(|| entities.first()),
        }
    }

    /// Calcula el rectángulo límite `[min_left, min_top, max_right, max_bottom]` formado por las
    /// capas de fondo (`EntityRole::Background`) para evitar que la cámara muestre bordes negros vacíos.
    fn compute_background_bounds(entities: &[StageEntity]) -> Option<[f32; 4]> {
        let mut candidates: Vec<&StageEntity> = entities
            .iter()
            .filter(|e| e.role == EntityRole::Background && e.visible && e.opacity > 0.01)
            .collect();
        if candidates.is_empty() {
            candidates = entities
                .iter()
                .filter(|e| e.role == EntityRole::Background)
                .collect();
        }
        if candidates.is_empty() {
            return None;
        }

        let large_only: Vec<&StageEntity> = candidates
            .iter()
            .copied()
            .filter(|e| (e.width * e.scale).abs() >= 320.0 && (e.height * e.scale).abs() >= 180.0)
            .collect();
        let sized = if large_only.is_empty() {
            &candidates
        } else {
            &large_only
        };

        // Si existen capas de fondo con imagen/atlas real, ignoramos placeholders vacíos
        // (como el `stage_bg` sin textura) para que no amplíen los límites con huecos oscuros.
        let with_media: Vec<&StageEntity> = sized
            .iter()
            .copied()
            .filter(|e| {
                e.image_path.is_some()
                    || e.raw_image_path.is_some()
                    || e.xml_path.is_some()
                    || e.animate_json_path.is_some()
                    || e.atlas.is_some()
                    || e.animate_spritemap.is_some()
            })
            .collect();
        let chosen = if with_media.is_empty() {
            sized
        } else {
            &with_media
        };

        let mut min_x = f32::INFINITY;
        let mut min_y = f32::INFINITY;
        let mut max_x = f32::NEG_INFINITY;
        let mut max_y = f32::NEG_INFINITY;
        for bg in chosen {
            let hw = (bg.width * bg.scale).abs() * 0.5;
            let hh = (bg.height * bg.scale).abs() * 0.5;
            min_x = min_x.min(bg.x - hw);
            max_x = max_x.max(bg.x + hw);
            min_y = min_y.min(bg.y - hh);
            max_y = max_y.max(bg.y + hh);
        }

        if min_x.is_finite() && min_y.is_finite() && max_x > min_x && max_y > min_y {
            Some([min_x, min_y, max_x, max_y])
        } else {
            None
        }
    }

    fn clamp_point_to_bounds(
        x: f32,
        y: f32,
        zoom: f32,
        virtual_w: f32,
        virtual_h: f32,
        bounds: Option<[f32; 4]>,
    ) -> (f32, f32) {
        let cx_world = if x.is_finite() {
            x.clamp(-1000.0, 2600.0)
        } else {
            640.0
        };
        let cy_world = if y.is_finite() {
            y.clamp(-1000.0, 1800.0)
        } else {
            360.0
        };
        let Some([min_x, min_y, max_x, max_y]) = bounds else {
            return (cx_world, cy_world);
        };
        let z = if zoom.is_finite() {
            zoom.clamp(0.35, 2.5)
        } else {
            1.0
        };
        let half_vw = (virtual_w.max(320.0) * 0.5) / z;
        let half_vh = (virtual_h.max(180.0) * 0.5) / z;

        let clamped_x = if min_x + half_vw <= max_x - half_vw {
            cx_world.clamp(min_x + half_vw, max_x - half_vw)
        } else {
            (min_x + max_x) * 0.5
        };
        let clamped_y = if min_y + half_vh <= max_y - half_vh {
            cy_world.clamp(min_y + half_vh, max_y - half_vh)
        } else {
            (min_y + max_y) * 0.5
        };
        (
            clamped_x.clamp(-1000.0, 2600.0),
            clamped_y.clamp(-1000.0, 1800.0),
        )
    }

    /// Devuelve el rectángulo delimitador del fondo actual (`[min_x, min_y, max_x, max_y]`).
    pub fn background_bounds(&self) -> Option<[f32; 4]> {
        Self::compute_background_bounds(&self.entities)
    }

    /// Devuelve el rectángulo delimitador del fondo a partir de una lista viva de entidades
    /// (por ejemplo `active_entities` durante `Stage Swap` en gameplay).
    pub fn background_bounds_for_entities(entities: &[StageEntity]) -> Option<[f32; 4]> {
        Self::compute_background_bounds(entities)
    }

    /// Restringe unas coordenadas `(x, y)` al área cubierta por el fondo del escenario según el `zoom`
    /// activo cuando `clamp_to_background` está habilitado.
    pub fn clamp_camera_to_background(&self, x: f32, y: f32, zoom: f32) -> (f32, f32) {
        self.clamp_camera_to_entities_background(x, y, zoom, &self.entities)
    }

    /// Restringe unas coordenadas `(x, y)` al área cubierta por el fondo vivo de `entities`
    /// (evitando franjas negras cuando scripts/eventos ocultan, reemplazan o mueven fondos).
    pub fn clamp_camera_to_entities_background(
        &self,
        x: f32,
        y: f32,
        zoom: f32,
        entities: &[StageEntity],
    ) -> (f32, f32) {
        if !self.camera.clamp_to_background {
            return (
                if x.is_finite() { x.clamp(-1000.0, 2600.0) } else { 640.0 },
                if y.is_finite() { y.clamp(-1000.0, 1800.0) } else { 360.0 },
            );
        }
        Self::clamp_point_to_bounds(
            x,
            y,
            zoom,
            self.virtual_width,
            self.virtual_height,
            Self::compute_background_bounds(entities),
        )
    }

    fn resolve_slot_against_entities(
        cfg: &StageCameraConfig,
        entities: &[StageEntity],
        slot_idx: usize,
        active_player_id: &str,
        active_opponent_id: &str,
    ) -> (f32, f32, f32, f32) {
        let cam0_x = cfg
            .cameras
            .first()
            .filter(|c| c.target_kind == CameraTargetKind::Fixed)
            .map(|c| c.x)
            .unwrap_or(cfg.base_x);
        let cam0_y = cfg
            .cameras
            .first()
            .filter(|c| c.target_kind == CameraTargetKind::Fixed)
            .map(|c| c.y)
            .unwrap_or(cfg.base_y);

        let bg_bounds = if cfg.clamp_to_background {
            Self::compute_background_bounds(entities)
        } else {
            None
        };

        let Some(slot) = cfg.cameras.get(slot_idx).or_else(|| cfg.cameras.first()) else {
            let z = cfg.base_zoom.clamp(0.35, 2.5);
            let (cx, cy) = Self::clamp_point_to_bounds(cfg.base_x, cfg.base_y, z, 1280.0, 720.0, bg_bounds);
            return (cx, cy, z, cfg.smooth_speed.clamp(1.0, 20.0));
        };

        let zoom = slot.zoom.clamp(0.35, 2.5);
        let smooth = slot.smooth_speed.clamp(1.0, 20.0);

        let (raw_x, raw_y) = if slot.target_kind == CameraTargetKind::Fixed {
            (slot.x.clamp(-1000.0, 2600.0), slot.y.clamp(-1000.0, 1800.0))
        } else if let Some(ent) = Self::resolve_entity_anchor(
            entities,
            slot.target_kind,
            slot.target_entity_id.as_deref(),
            active_player_id,
            active_opponent_id,
        ) {
            let dest_x = ent.x + ent.camera_offset[0] + slot.x;
            let dest_y = ent.y + ent.camera_offset[1] + slot.y;
            let pan = slot.pan_strength.clamp(0.0, 1.0);
            (
                (cam0_x + (dest_x - cam0_x) * pan).clamp(-1000.0, 2600.0),
                (cam0_y + (dest_y - cam0_y) * pan).clamp(-1000.0, 1800.0),
            )
        } else {
            (
                (cam0_x + slot.x).clamp(-1000.0, 2600.0),
                (cam0_y + slot.y).clamp(-1000.0, 1800.0),
            )
        };

        let (final_x, final_y) = if cfg.clamp_to_background {
            Self::clamp_point_to_bounds(raw_x, raw_y, zoom, 1280.0, 720.0, bg_bounds)
        } else {
            (raw_x, raw_y)
        };
        (final_x, final_y, zoom, smooth)
    }

    /// Resuelve coordenadas `(x, y, zoom, smooth_speed)` reales en escena para la cámara `slot_idx`.
    pub fn resolve_camera_slot_target(
        &self,
        slot_idx: usize,
        active_player_id: &str,
        active_opponent_id: &str,
    ) -> (f32, f32, f32, f32) {
        self.resolve_camera_slot_target_with_entities(
            slot_idx,
            &self.entities,
            active_player_id,
            active_opponent_id,
        )
    }

    /// Resuelve coordenadas `(x, y, zoom, smooth_speed)` usando el snapshot vivo `entities`
    /// (imprescindible durante gameplay tras eventos `Stage Swap` / `Change Character` / tweens).
    pub fn resolve_camera_slot_target_with_entities(
        &self,
        slot_idx: usize,
        entities: &[StageEntity],
        active_player_id: &str,
        active_opponent_id: &str,
    ) -> (f32, f32, f32, f32) {
        Self::resolve_slot_against_entities(
            &self.camera,
            entities,
            slot_idx,
            active_player_id,
            active_opponent_id,
        )
    }

    /// Actualiza los parámetros de una ranura de cámara (`Cam 0/1/2/3+`) y sincroniza los campos base si es `Cam 0`.
    #[allow(clippy::too_many_arguments)]
    pub fn update_camera_slot(
        &mut self,
        slot_index: usize,
        name: &str,
        target_kind: CameraTargetKind,
        target_entity_id: Option<String>,
        x: f32,
        y: f32,
        zoom: f32,
        smooth_speed: f32,
        pan_strength: f32,
    ) -> bool {
        self.camera.ensure_default_cameras();
        let Some(slot) = self.camera.cameras.get_mut(slot_index) else {
            return false;
        };
        if !name.trim().is_empty() {
            slot.name = name.trim().to_string();
        }
        slot.target_kind = target_kind;
        slot.target_entity_id = target_entity_id.filter(|s| !s.trim().is_empty());
        slot.x = x;
        slot.y = y;
        slot.zoom = zoom;
        slot.smooth_speed = smooth_speed;
        slot.pan_strength = pan_strength;
        slot.sanitize();

        if slot_index == 0 {
            self.camera.base_x = slot.x;
            self.camera.base_y = slot.y;
            self.camera.base_zoom = slot.zoom;
            self.camera.smooth_speed = slot.smooth_speed;
        }
        self.camera.active_camera_index = slot_index;
        let (rx, ry, rzoom, _) = self.resolve_camera_slot_target(slot_index, "", "");
        self.camera_x = rx;
        self.camera_y = ry;
        self.camera_base_zoom = rzoom;
        self.camera_zoom = rzoom;
        true
    }

    /// Añade una nueva cámara personalizada (`Cam 3`, `Cam 4`, ...) y devuelve su índice asignado.
    #[allow(clippy::too_many_arguments)]
    pub fn add_custom_camera(
        &mut self,
        name: &str,
        target_kind: CameraTargetKind,
        target_entity_id: Option<String>,
        x: f32,
        y: f32,
        zoom: f32,
        smooth_speed: f32,
        pan_strength: f32,
    ) -> usize {
        self.camera.ensure_default_cameras();
        let idx = self.camera.cameras.len();
        let clean_name = if name.trim().is_empty() {
            format!("Cam {} · Personalizada", idx)
        } else {
            name.trim().to_string()
        };
        let mut slot = StageCameraSlot {
            index: idx,
            name: clean_name,
            target_kind,
            target_entity_id: target_entity_id.filter(|s| !s.trim().is_empty()),
            x,
            y,
            zoom,
            smooth_speed,
            pan_strength,
        };
        slot.sanitize();
        self.camera.cameras.push(slot);
        self.camera.active_camera_index = idx;
        let (rx, ry, rzoom, _) = self.resolve_camera_slot_target(idx, "", "");
        self.camera_x = rx;
        self.camera_y = ry;
        self.camera_base_zoom = rzoom;
        self.camera_zoom = rzoom;
        idx
    }

    /// Elimina una cámara personalizada (`index >= 3`) preservando intactas `Cam 0`, `Cam 1` y `Cam 2`.
    pub fn remove_custom_camera(&mut self, slot_index: usize) -> bool {
        self.camera.ensure_default_cameras();
        if slot_index < 3 || slot_index >= self.camera.cameras.len() {
            return false;
        }
        self.camera.cameras.remove(slot_index);
        for (i, cam) in self.camera.cameras.iter_mut().enumerate() {
            cam.index = i;
        }
        if self.camera.active_camera_index >= self.camera.cameras.len() {
            self.camera.active_camera_index = self.camera.cameras.len().saturating_sub(1);
        }
        let active_idx = self.camera.active_camera_index;
        let (rx, ry, rzoom, _) = self.resolve_camera_slot_target(active_idx, "", "");
        self.camera_x = rx;
        self.camera_y = ry;
        self.camera_base_zoom = rzoom;
        self.camera_zoom = rzoom;
        true
    }

    /// Ordena las entidades por `z_index` ascendente para dibujar correctamente qué está encima y qué debajo.
    pub fn sort_by_z_order(&mut self) {
        self.entities.sort_by_key(|e| e.z_index);
    }

    pub fn bring_forward(&mut self, entity_id: &str) {
        if let Some(target) = self.entities.iter_mut().find(|e| e.id == entity_id) {
            target.z_index += 1;
        }
        self.sort_by_z_order();
    }

    pub fn send_backward(&mut self, entity_id: &str) {
        if let Some(target) = self.entities.iter_mut().find(|e| e.id == entity_id) {
            target.z_index -= 1;
        }
        self.sort_by_z_order();
    }

    pub fn bring_to_front(&mut self, entity_id: &str) {
        let max_z = self.entities.iter().map(|e| e.z_index).max().unwrap_or(0);
        if let Some(target) = self.entities.iter_mut().find(|e| e.id == entity_id) {
            target.z_index = max_z + 1;
        }
        self.sort_by_z_order();
    }

    pub fn send_to_back(&mut self, entity_id: &str) {
        let min_z = self.entities.iter().map(|e| e.z_index).min().unwrap_or(0);
        if let Some(target) = self.entities.iter_mut().find(|e| e.id == entity_id) {
            target.z_index = min_z - 1;
        }
        self.sort_by_z_order();
    }

    pub fn find_mut(&mut self, entity_id: &str) -> Option<&mut StageEntity> {
        self.entities.iter_mut().find(|e| e.id == entity_id)
    }

    /// Elimina una capa/entidad individual de la escena y desvincula cualquier ranura de cámara
    /// (`CameraTargetKind::Entity`) que apuntara a ella convirtiéndola a modo `Fixed`.
    pub fn remove_entity(&mut self, entity_id: &str) -> Option<StageEntity> {
        let idx = self.entities.iter().position(|e| e.id == entity_id)?;
        let removed = self.entities.remove(idx);

        for slot in &mut self.camera.cameras {
            if slot.target_entity_id.as_deref() == Some(entity_id) {
                slot.target_entity_id = None;
                if slot.target_kind == CameraTargetKind::Entity {
                    slot.target_kind = CameraTargetKind::Fixed;
                    slot.pan_strength = 0.0;
                }
            }
        }

        let active_idx = self.camera.active_camera_index;
        let (rx, ry, rzoom, _) = self.resolve_camera_slot_target(active_idx, "", "");
        self.camera_x = rx;
        self.camera_y = ry;
        self.camera_base_zoom = rzoom;
        self.camera_zoom = rzoom;

        Some(removed)
    }

    /// Vacía todas las capas de la escena, desvincula objetivos de entidad en las cámaras
    /// pasándolos a posición `Fixed`, y devuelve las entidades retiradas para limpiar sus assets.
    pub fn clear_entities(&mut self) -> Vec<StageEntity> {
        let removed = std::mem::take(&mut self.entities);

        for slot in &mut self.camera.cameras {
            slot.target_entity_id = None;
            if slot.target_kind == CameraTargetKind::Entity {
                slot.target_kind = CameraTargetKind::Fixed;
                slot.pan_strength = 0.0;
            }
        }

        let active_idx = self.camera.active_camera_index;
        let (rx, ry, rzoom, _) = self.resolve_camera_slot_target(active_idx, "", "");
        self.camera_x = rx;
        self.camera_y = ry;
        self.camera_base_zoom = rzoom;
        self.camera_zoom = rzoom;

        removed
    }
}
