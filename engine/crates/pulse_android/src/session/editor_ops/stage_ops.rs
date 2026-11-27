//! Archivo: engine/crates/pulse_android/src/session/editor_ops/stage_ops.rs
//! Propósito: Gestión de entidades/capas del escenario (`StageEntity`) en `EngineSession`:
//! alta, eliminación individual, vaciado completo de escena con limpieza de assets no
//! referenciados, actualización de transformaciones/Z-order e importación `stages/*.json|.xml`.

use crate::session::{EngineSession, parse_role_key};
use pulse_core::mod_descriptors::{normalize_stage_world_pos, parse_stage_descriptor_str};
use pulse_core::project::{CAMERA_SCRIPT_ID, CAMERA_SCRIPT_NAME, render_camera_lua_script};
use pulse_core::{EntityRole, StageEntity};
use std::path::PathBuf;

impl EngineSession {
    pub fn add_stage_entity(&mut self, name: &str, role_key: &str) -> String {
        let mut idx = self.project.stage.entities.len() + 1;
        while self
            .project
            .stage
            .entities
            .iter()
            .any(|e| e.id == format!("asset_{}", idx))
        {
            idx += 1;
        }
        let id = format!("asset_{}", idx);
        let role = parse_role_key(role_key);
        let clean_name = if name.trim().is_empty() {
            format!("Objeto {}", idx)
        } else {
            name.trim().to_string()
        };
        let ent = StageEntity::new(id, clean_name, role);
        self.project.stage.entities.push(ent);
        self.project.stage.sort_by_z_order();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Nuevo objeto añadido al escenario")
    }

    /// Elimina una capa/entidad de la escena actual, limpia sus assets si no se reutilizan,
    /// desvincula cualquier cámara que la estuviera siguiendo y guarda el proyecto.
    pub fn remove_stage_entity(&mut self, entity_id: &str) -> String {
        let Some(removed) = self.project.stage.remove_entity(entity_id) else {
            return self.err_response("Capa u objeto de escena no encontrado");
        };

        let candidate_paths = [
            removed.raw_image_path,
            removed.image_path,
            removed.icon_normal_path,
            removed.icon_losing_path,
        ];
        for path_opt in &candidate_paths {
            self.remove_managed_asset_if_unreferenced(path_opt.as_deref());
        }

        self.sync_camera_lua_script();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response(format!("Capa '{}' eliminada de la escena", removed.display_name))
    }

    /// Elimina todas las capas actuales de la escena y limpia sus texturas/íconos no referenciados.
    pub fn clear_stage_scene(&mut self) -> String {
        let removed_all = self.project.stage.clear_entities();
        if removed_all.is_empty() {
            return self.ok_response("La escena ya estaba vacía");
        }
        let removed_count = removed_all.len();
        for removed in &removed_all {
            self.remove_managed_asset_if_unreferenced(removed.raw_image_path.as_deref());
            self.remove_managed_asset_if_unreferenced(removed.image_path.as_deref());
            self.remove_managed_asset_if_unreferenced(removed.icon_normal_path.as_deref());
            self.remove_managed_asset_if_unreferenced(removed.icon_losing_path.as_deref());
        }

        self.sync_camera_lua_script();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response(format!(
            "Escena vaciada ({} capas eliminadas)",
            removed_count
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_stage_entity(
        &mut self,
        entity_id: &str,
        name: &str,
        role_key: &str,
        dx: f32,
        dy: f32,
        abs_x: f32,
        abs_y: f32,
        use_abs_pos: bool,
        scale: f32,
        opacity: f32,
        visible: bool,
        flip_x: bool,
        commit_save: bool,
    ) -> String {
        if let Some(ent) = self.project.stage.find_mut(entity_id) {
            if !name.trim().is_empty() {
                ent.display_name = name.trim().to_string();
            }
            ent.role = parse_role_key(role_key);
            if use_abs_pos {
                ent.x = abs_x.clamp(-1000.0, 2600.0);
                ent.y = abs_y.clamp(-1000.0, 1800.0);
            } else if dx != 0.0 || dy != 0.0 {
                ent.nudge(dx, dy);
            }
            ent.set_scale(scale);
            ent.set_opacity(opacity);
            ent.visible = visible;
            ent.flip_x = flip_x;
            if commit_save {
                self.invalidate_compiled_bundle();
            }
            self.mark_dirty_and_maybe_save(commit_save);
            if commit_save {
                self.ok_response("Objeto actualizado")
            } else {
                "{\"ok\":true,\"message\":\"Objeto actualizado\",\"data\":null}".to_string()
            }
        } else {
            self.err_response("Objeto no encontrado")
        }
    }

    pub fn reorder_stage_entity(&mut self, entity_id: &str, action: &str) -> String {
        match action {
            "forward" => self.project.stage.bring_forward(entity_id),
            "backward" => self.project.stage.send_backward(entity_id),
            "front" => self.project.stage.bring_to_front(entity_id),
            "back" => self.project.stage.send_to_back(entity_id),
            _ => {}
        }
        self.invalidate_compiled_bundle();
        self.save_active_project_now();
        self.ok_response("Orden Z actualizado")
    }

    /// Importa un archivo de escenario (`stages/*.json` o `stages/*.xml`) para ubicar
    /// automáticamente al Player, Enemigo, Cabina/Decoración, zoom inicial y props declarados.
    pub fn import_stage_config(&mut self, path: PathBuf) -> String {
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                return self.err_response(format!("No se pudo leer el archivo de escenario: {}", e));
            }
        };

        let desc = match parse_stage_descriptor_str(&raw) {
            Ok(d) => d,
            Err(e) => return self.err_response(e),
        };

        let mut cam_cfg = self.project.stage.camera.clone();
        if let Some(zoom) = desc.default_zoom {
            cam_cfg.base_zoom = zoom.clamp(0.35, 2.5);
        }
        if let Some([cx, cy]) = desc.camera_start {
            if cx.is_finite() && cy.is_finite() {
                cam_cfg.base_x = cx.clamp(-1000.0, 2600.0);
                cam_cfg.base_y = cy.clamp(-1000.0, 1800.0);
            }
        }
        cam_cfg.sync_slot0_from_base();
        self.project.stage.apply_camera_config(cam_cfg);
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

        let mut updated_slots = 0usize;
        let player_world = desc
            .player_pos
            .map(|p| normalize_stage_world_pos(p, EntityRole::Player, desc.is_bottom_center_coords));
        let opp_world = desc.opponent_pos.map(|p| {
            normalize_stage_world_pos(p, EntityRole::Opponent, desc.is_bottom_center_coords)
        });
        let gf_world = desc.gf_pos.map(|p| {
            normalize_stage_world_pos(p, EntityRole::Decoration, desc.is_bottom_center_coords)
        });

        let mut applied_gf = false;
        for ent in &mut self.project.stage.entities {
            match ent.role {
                EntityRole::Player => {
                    if let Some([wx, wy]) = player_world {
                        ent.x = wx;
                        ent.y = wy;
                        if let Some(pending) = &ent.pending_character_desc {
                            if let Some(cpos) = pending.position {
                                if pending.is_relative_position {
                                    ent.x = (ent.x + cpos[0] * 0.55).clamp(-1000.0, 2600.0);
                                    ent.y = (ent.y + cpos[1] * 0.25).clamp(-1000.0, 1800.0);
                                }
                            }
                        }
                        updated_slots += 1;
                    }
                    if let Some(z) = desc.player_z {
                        ent.z_index = z;
                    }
                    if let Some(cam_off) = desc.player_cam_offset {
                        ent.camera_offset = cam_off;
                    }
                }
                EntityRole::Opponent => {
                    if let Some([wx, wy]) = opp_world {
                        ent.x = wx;
                        ent.y = wy;
                        if let Some(pending) = &ent.pending_character_desc {
                            if let Some(cpos) = pending.position {
                                if pending.is_relative_position {
                                    ent.x = (ent.x + cpos[0] * 0.55).clamp(-1000.0, 2600.0);
                                    ent.y = (ent.y + cpos[1] * 0.25).clamp(-1000.0, 1800.0);
                                }
                            }
                        }
                        updated_slots += 1;
                    }
                    if let Some(z) = desc.opponent_z {
                        ent.z_index = z;
                    }
                    if let Some(cam_off) = desc.opponent_cam_offset {
                        ent.camera_offset = cam_off;
                    }
                }
                EntityRole::Decoration if !applied_gf => {
                    let is_gf_slot = ent.id == "stage_speakers"
                        || ent.display_name.to_ascii_lowercase().contains("cabina")
                        || ent.display_name.to_ascii_lowercase().contains("gf")
                        || ent.display_name.to_ascii_lowercase().contains("girlfriend");
                    if is_gf_slot {
                        applied_gf = true;
                        if desc.hide_gf {
                            ent.visible = false;
                        } else if let Some([wx, wy]) = gf_world {
                            ent.x = wx;
                            ent.y = wy;
                            ent.visible = true;
                            updated_slots += 1;
                        }
                        if let Some(z) = desc.gf_z {
                            ent.z_index = z;
                        }
                        if let Some(cam_off) = desc.gf_cam_offset {
                            ent.camera_offset = cam_off;
                        }
                    }
                }
                _ => {}
            }
        }

        let mut added_props = 0usize;
        for (i, prop) in desc.props.iter().enumerate() {
            let role = if i == 0 && prop.scroll_factor <= 0.95 {
                EntityRole::Background
            } else {
                EntityRole::Decoration
            };
            let [wx, wy] = normalize_stage_world_pos(
                prop.position,
                role,
                desc.is_bottom_center_coords,
            );
            let slug: String = prop
                .name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() {
                        c.to_ascii_lowercase()
                    } else {
                        '_'
                    }
                })
                .collect();
            let prop_id = if slug.trim_matches('_').is_empty() {
                format!("stage_prop_{}", i + 1)
            } else {
                format!("stage_prop_{}", slug.trim_matches('_'))
            };

            if let Some(existing) = self
                .project
                .stage
                .entities
                .iter_mut()
                .find(|e| e.id == prop_id || e.display_name.eq_ignore_ascii_case(&prop.name))
            {
                existing.x = wx;
                existing.y = wy;
                existing.set_scale(prop.scale);
                existing.scroll_factor = prop.scroll_factor;
                existing.z_index = prop.z_index;
                existing.flip_x = prop.flip_x;
                existing.set_opacity(prop.alpha);
            } else {
                let mut ent = StageEntity::new(prop_id, prop.name.clone(), role);
                ent.x = wx;
                ent.y = wy;
                ent.set_scale(prop.scale);
                ent.scroll_factor = prop.scroll_factor;
                ent.z_index = prop.z_index;
                ent.flip_x = prop.flip_x;
                ent.set_opacity(prop.alpha);
                self.project.stage.entities.push(ent);
                added_props += 1;
            }
        }

        self.project.stage.sort_by_z_order();
        self.invalidate_compiled_bundle();
        self.save_active_project_now();

        self.ok_response(format!(
            "{} aplicado: zoom {:.2}x, {} posiciones actualizadas, {} props creados",
            desc.format_label, applied_cfg.base_zoom, updated_slots, added_props
        ))
    }
}
