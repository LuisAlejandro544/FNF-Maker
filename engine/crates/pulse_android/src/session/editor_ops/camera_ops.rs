//! Archivo: engine/crates/pulse_android/src/session/editor_ops/camera_ops.rs
//! Propósito: Gestión multicámara del escenario (`Cam 0..3+`), configuración global de
//! seguimiento/clamping, viñeta lateral (`vignette_*`) y regeneración automática del script
//! `camera_system.lua` en `EngineSession`.

use crate::session::EngineSession;
use pulse_core::project::{CAMERA_SCRIPT_ID, CAMERA_SCRIPT_NAME, render_camera_lua_script};
use pulse_core::{CameraFollowMode, CameraTargetKind};

impl EngineSession {
    pub(super) fn sync_camera_lua_script(&mut self) {
        self.project.ensure_camera_script();
        let applied_cfg = self.project.stage.camera.clone();
        if let Some(cam_script) = self
            .project
            .scripts
            .iter_mut()
            .find(|s| s.id == CAMERA_SCRIPT_ID || s.name == CAMERA_SCRIPT_NAME)
        {
            cam_script.enabled = applied_cfg.enabled;
            cam_script.source_code =
                render_camera_lua_script(&applied_cfg, Some(&cam_script.source_code));
            cam_script.precompiled_bytecode = None;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_stage_camera(
        &mut self,
        enabled: bool,
        base_x: f32,
        base_y: f32,
        base_zoom: f32,
        follow_mode_key: &str,
        follow_interval_beats: i32,
        smooth_speed: f32,
        pan_strength: f32,
        clamp_to_background: bool,
        commit_save: bool,
    ) -> String {
        let mut cfg = self.project.stage.camera.clone();
        cfg.enabled = enabled;
        cfg.base_x = base_x;
        cfg.base_y = base_y;
        cfg.base_zoom = base_zoom;
        cfg.follow_mode = CameraFollowMode::from_str_loose(follow_mode_key);
        cfg.follow_interval_beats = follow_interval_beats.clamp(1, 32) as u32;
        cfg.smooth_speed = smooth_speed;
        cfg.pan_strength = pan_strength;
        cfg.clamp_to_background = clamp_to_background;
        cfg.sync_slot0_from_base();
        self.project.stage.apply_camera_config(cfg);
        if commit_save {
            self.sync_camera_lua_script();
            self.invalidate_compiled_bundle();
        }

        self.mark_dirty_and_maybe_save(commit_save);
        if commit_save {
            self.ok_response("Cámara y camera_system.lua actualizados")
        } else {
            "{\"ok\":true,\"message\":\"Cámara actualizada\",\"data\":null}".to_string()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_stage_camera_vignette(
        &mut self,
        enabled: bool,
        color_hex: &str,
        alpha: f32,
        width: f32,
        pulse: f32,
        over_hud: bool,
        commit_save: bool,
    ) -> String {
        let mut cfg = self.project.stage.camera.clone();
        cfg.vignette_enabled = enabled;
        cfg.vignette_color = color_hex.to_string();
        cfg.vignette_alpha = alpha;
        cfg.vignette_width = width;
        cfg.vignette_pulse = pulse;
        cfg.vignette_over_hud = over_hud;
        self.project.stage.apply_camera_config(cfg);
        if commit_save {
            self.sync_camera_lua_script();
            self.invalidate_compiled_bundle();
        }
        self.mark_dirty_and_maybe_save(commit_save);
        if commit_save {
            self.ok_response("Viñeta lateral de cámara guardada")
        } else {
            "{\"ok\":true,\"message\":\"Viñeta lateral actualizada\",\"data\":null}".to_string()
        }
    }

    pub fn select_stage_camera(&mut self, slot_index: usize) -> String {
        self.project.stage.camera.ensure_default_cameras();
        let max_idx = self.project.stage.camera.cameras.len().saturating_sub(1);
        let idx = slot_index.min(max_idx);
        self.project.stage.camera.active_camera_index = idx;
        let (rx, ry, rzoom, _) = self.project.stage.resolve_camera_slot_target(idx, "", "");
        self.project.stage.camera_x = rx;
        self.project.stage.camera_y = ry;
        self.project.stage.camera_base_zoom = rzoom;
        self.project.stage.camera_zoom = rzoom;
        self.sync_camera_lua_script();
        self.invalidate_compiled_bundle();
        self.mark_dirty_and_maybe_save(true);
        self.ok_response(format!("Previsualizando Cam {}", idx))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_camera_slot(
        &mut self,
        slot_index: usize,
        name: &str,
        target_kind_key: &str,
        target_entity_id: &str,
        x: f32,
        y: f32,
        zoom: f32,
        smooth_speed: f32,
        pan_strength: f32,
        commit_save: bool,
    ) -> String {
        let kind = CameraTargetKind::from_str_loose(target_kind_key);
        let entity_opt = if target_entity_id.trim().is_empty() {
            None
        } else {
            Some(target_entity_id.trim().to_string())
        };
        if !self.project.stage.update_camera_slot(
            slot_index,
            name,
            kind,
            entity_opt,
            x,
            y,
            zoom,
            smooth_speed,
            pan_strength,
        ) {
            return self.err_response("Ranura de cámara no encontrada");
        }
        if commit_save {
            self.sync_camera_lua_script();
            self.invalidate_compiled_bundle();
        }
        self.mark_dirty_and_maybe_save(commit_save);
        if commit_save {
            self.ok_response(format!("Cam {} actualizada", slot_index))
        } else {
            "{\"ok\":true,\"message\":\"Camara actualizada\",\"data\":null}".to_string()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_custom_camera(
        &mut self,
        name: &str,
        target_kind_key: &str,
        target_entity_id: &str,
        x: f32,
        y: f32,
        zoom: f32,
        smooth_speed: f32,
        pan_strength: f32,
    ) -> String {
        let kind = CameraTargetKind::from_str_loose(target_kind_key);
        let entity_opt = if target_entity_id.trim().is_empty() {
            None
        } else {
            Some(target_entity_id.trim().to_string())
        };
        let new_idx = self.project.stage.add_custom_camera(
            name,
            kind,
            entity_opt,
            x,
            y,
            zoom,
            smooth_speed,
            pan_strength,
        );
        self.sync_camera_lua_script();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response(format!("Nueva cámara Cam {} añadida", new_idx))
    }

    pub fn remove_custom_camera(&mut self, slot_index: usize) -> String {
        if !self.project.stage.remove_custom_camera(slot_index) {
            return self.err_response(
                "Cam 0, Cam 1 y Cam 2 son cámaras base; solo puedes eliminar Cam 3 o superiores",
            );
        }
        for sec in &mut self.project.chart.sections {
            if let Some(s) = sec.camera_slot {
                if s == slot_index {
                    sec.camera_slot = None;
                } else if s > slot_index {
                    sec.camera_slot = Some(s - 1);
                }
            }
        }
        self.sync_camera_lua_script();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response(format!("Cámara Cam {} eliminada", slot_index))
    }
}
