//! Archivo: engine/crates/pulse_android/src/session/sprite_ops.rs
//! Propósito: Gestión de texturas `.png`/`.astc`, atlas Sparrow `.xml`, atlas JSON
//! (TexturePacker, `spritemap1.json`, `Animation.json`), descriptores de personaje (`.json`/`.xml`),
//! offsets por animación y horneado por software de rigs Adobe Animate.

use super::{EngineSession, SparrowFrameJson, friendly_import_name, parse_anim_kind};
use pulse_core::astc_loader::{
    is_astc_path, load_sprite_image, read_sprite_dimensions, save_lossless_webp,
};
use pulse_core::atlas_json::{DetectedSpriteJson, bake_animate_atlas, parse_sprite_json_str};
use pulse_core::mod_descriptors::{
    DetectedModDescriptor, parse_character_descriptor_str, parse_mod_descriptor_str,
};
use pulse_core::sparrow::CharacterAnimKind;
use pulse_core::{EntityRole, SparrowAtlas};
use std::path::PathBuf;

impl EngineSession {
    /// Si la entidad ya tiene `Animation.json` + `spritemap1.json` + textura (`.astc` o `.png`),
    /// hornea automáticamente los frames compuestos a un spritesheet RGBA en caché y actualiza su atlas.
    fn try_bake_animate_for_entity(&mut self, entity_id: &str) -> Result<Option<String>, String> {
        let (doc, spritemap, raw_img_path) = {
            let Some(ent) = self.project.stage.entities.iter().find(|e| e.id == entity_id) else {
                return Ok(None);
            };
            let (Some(doc), Some(sm)) = (ent.animate_doc.clone(), ent.animate_spritemap.clone()) else {
                return Ok(None);
            };
            let Some(img_path) = ent.raw_image_path.clone().or_else(|| ent.image_path.clone()) else {
                return Ok(None);
            };
            (doc, sm, img_path)
        };

        let src_img = load_sprite_image(&raw_img_path)?;
        let (baked_sheet, baked_atlas) = bake_animate_atlas(&doc, &spritemap, &src_img)?;
        let anim_count = baked_atlas.animations.len();

        let baked_dir = self.project_textures_dir().join("animate_baked");
        let _ = std::fs::create_dir_all(&baked_dir);
        let baked_path = baked_dir.join(format!("{}_baked.webp", entity_id));
        save_lossless_webp(&baked_sheet, &baked_path)
            .map_err(|e| format!("Error guardando spritesheet Animate WebP: {}", e))?;

        if let Some(ent) = self.project.stage.find_mut(entity_id) {
            ent.image_path = Some(baked_path);
            ent.atlas = Some(baked_atlas);
            if let Some(pending) = ent.pending_character_desc.clone() {
                ent.apply_character_descriptor(&pending, false);
            }
        }

        Ok(Some(format!(
            "Rig Adobe Animate ensamblado en Rust: {} animaciones listas",
            anim_count
        )))
    }

    pub fn import_sprite_png(&mut self, entity_id: &str, path: PathBuf) -> String {
        if !path.is_file() {
            return self.err_response("Archivo de textura no encontrado");
        }
        if self.project.stage.find_mut(entity_id).is_none() {
            return self.err_response("Objeto de escenario no encontrado");
        }

        let is_astc = is_astc_path(&path);
        let (adopted_raw_path, display_img_path, dims) = if is_astc {
            let decoded = match load_sprite_image(&path) {
                Ok(img) => img,
                Err(e) => return self.err_response(format!("Error decodificando .astc: {}", e)),
            };
            let dims = Some((decoded.width(), decoded.height()));
            let adopted_raw = match self.adopt_permanent_asset(&path, "sprites", entity_id) {
                Ok(p) => p,
                Err(e) => return self.err_response(e),
            };
            let dec_dir = self.project_textures_dir().join("decoded_textures");
            let _ = std::fs::create_dir_all(&dec_dir);
            let out_webp = dec_dir.join(format!("{}.webp", entity_id));
            if let Err(e) = save_lossless_webp(&decoded, &out_webp) {
                let _ = std::fs::remove_file(&adopted_raw);
                return self.err_response(format!("Error guardando caché ASTC->WebP: {}", e));
            }
            (adopted_raw, out_webp, dims)
        } else {
            let dims = read_sprite_dimensions(&path);
            if dims.is_none() {
                return self.err_response("Archivo de imagen inválido o corrupto");
            }
            let adopted_img = match self.adopt_permanent_asset(&path, "sprites", entity_id) {
                Ok(p) => p,
                Err(e) => return self.err_response(e),
            };
            (adopted_img.clone(), adopted_img, dims)
        };

        let mut old_raw: Option<PathBuf> = None;
        let mut old_img: Option<PathBuf> = None;
        if let Some(ent) = self.project.stage.find_mut(entity_id) {
            old_raw = ent.raw_image_path.take();
            old_img = ent.image_path.take();
            ent.raw_image_path = Some(adopted_raw_path);
            ent.image_path = Some(display_img_path);
            if let Some((w, h)) = dims {
                if ent.atlas.is_none() && ent.role != EntityRole::Background && w > 0 && h > 0 {
                    // Ajustar tamaño lógico inicial manteniendo proporción razonable dentro de 1280x720.
                    let max_side = w.max(h) as f32;
                    let norm = (320.0 / max_side).min(1.0);
                    ent.width = (w as f32 * norm).max(64.0);
                    ent.height = (h as f32 * norm).max(64.0);
                }
            }
        }
        self.remove_managed_asset_if_unreferenced(old_raw.as_deref());
        if old_img.as_deref() != old_raw.as_deref() {
            self.remove_managed_asset_if_unreferenced(old_img.as_deref());
        }

        match self.try_bake_animate_for_entity(entity_id) {
            Ok(Some(msg)) => {
                self.save_active_project_now();
                self.ok_response(msg)
            }
            Ok(None) => {
                self.save_active_project_now();
                self.ok_response(if is_astc {
                    "Textura .astc decodificada y vinculada en Rust"
                } else {
                    "Hoja de textura vinculada en Rust"
                })
            }
            Err(e) => self.err_response(e),
        }
    }

    pub fn import_sparrow_xml(&mut self, entity_id: &str, path: PathBuf) -> String {
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => return self.err_response(format!("No se pudo leer el descriptor: {}", e)),
        };

        let display_descriptor_path = PathBuf::from(friendly_import_name(&path));
        let is_json = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("json"))
            .unwrap_or(false)
            || raw.trim_start().starts_with('{')
            || raw.trim_start().starts_with('[');

        if is_json {
            match parse_sprite_json_str(&raw) {
                Ok(DetectedSpriteJson::DirectAtlas(atlas)) => {
                    let count = atlas.animations.len();
                    if let Some(ent) = self.project.stage.find_mut(entity_id) {
                        ent.xml_path = Some(display_descriptor_path);
                        ent.animate_spritemap = None;
                        ent.animate_doc = None;
                        ent.animate_json_path = None;
                        ent.atlas = Some(atlas);
                        if let Some(pending) = ent.pending_character_desc.clone() {
                            ent.apply_character_descriptor(&pending, false);
                        }
                        self.save_active_project_now();
                        return self.ok_response(format!(
                            "Atlas JSON cargado automáticamente: {} animaciones",
                            count
                        ));
                    }
                    return self.err_response("Objeto de escenario no encontrado");
                }
                Ok(DetectedSpriteJson::AnimateSpritemap(sm)) => {
                    let piece_count = sm.sprites.len();
                    let fallback = sm.to_fallback_sparrow();
                    if let Some(ent) = self.project.stage.find_mut(entity_id) {
                        ent.xml_path = Some(display_descriptor_path);
                        ent.animate_spritemap = Some(sm);
                        ent.atlas = Some(fallback);
                    } else {
                        return self.err_response("Objeto de escenario no encontrado");
                    }
                    return match self.try_bake_animate_for_entity(entity_id) {
                        Ok(Some(msg)) => {
                            self.save_active_project_now();
                            self.ok_response(msg)
                        }
                        Ok(None) => {
                            self.save_active_project_now();
                            self.ok_response(format!(
                                "spritemap1.json cargado ({} piezas). Falta Animation.json o textura para hornear el rig.",
                                piece_count
                            ))
                        }
                        Err(e) => self.err_response(e),
                    };
                }
                Ok(DetectedSpriteJson::AnimateDocument(doc)) => {
                    let anim_count = doc.detected_animations.len();
                    if let Some(ent) = self.project.stage.find_mut(entity_id) {
                        ent.animate_json_path = Some(display_descriptor_path);
                        ent.animate_doc = Some(doc);
                    } else {
                        return self.err_response("Objeto de escenario no encontrado");
                    }
                    return match self.try_bake_animate_for_entity(entity_id) {
                        Ok(Some(msg)) => {
                            self.save_active_project_now();
                            self.ok_response(msg)
                        }
                        Ok(None) => {
                            self.save_active_project_now();
                            self.ok_response(format!(
                                "Animation.json cargado ({} animaciones). Importa spritemap1.json y la textura (.astc/.png).",
                                anim_count
                            ))
                        }
                        Err(e) => self.err_response(e),
                    };
                }
                Err(atlas_err) => match parse_mod_descriptor_str(&raw) {
                    Ok(DetectedModDescriptor::Character(_)) => {
                        return self.import_character_config(entity_id, path);
                    }
                    Ok(DetectedModDescriptor::Stage(_)) => {
                        return self.err_response(
                            "Este JSON describe un escenario completo. Impórtalo en la pestaña Escena -> Escena .json/.xml.",
                        );
                    }
                    Err(_) => return self.err_response(atlas_err),
                },
            }
        }

        match SparrowAtlas::from_xml_str(&raw) {
            Ok(atlas) => {
                let count = atlas.animations.len();
                if let Some(ent) = self.project.stage.find_mut(entity_id) {
                    ent.xml_path = Some(display_descriptor_path);
                    ent.animate_spritemap = None;
                    ent.animate_doc = None;
                    ent.animate_json_path = None;
                    ent.atlas = Some(atlas);
                    if let Some(pending) = ent.pending_character_desc.clone() {
                        ent.apply_character_descriptor(&pending, false);
                    }
                    self.save_active_project_now();
                    self.ok_response(format!(
                        "XML Sparrow ensamblado automáticamente: {} animaciones",
                        count
                    ))
                } else {
                    self.err_response("Objeto de escenario no encontrado")
                }
            }
            Err(sparrow_err) => match parse_mod_descriptor_str(&raw) {
                Ok(DetectedModDescriptor::Character(_)) => {
                    self.import_character_config(entity_id, path)
                }
                Ok(DetectedModDescriptor::Stage(_)) => self.err_response(
                    "Este XML describe un escenario (<stage>). Impórtalo en la pestaña Escena -> Escena .json/.xml.",
                ),
                Err(_) => self.err_response(format!("Error XML Sparrow: {}", sparrow_err)),
            },
        }
    }

    pub fn import_character_config(&mut self, entity_id: &str, path: PathBuf) -> String {
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                return self.err_response(format!(
                    "No se pudo leer el descriptor de personaje: {}",
                    e
                ));
            }
        };

        let desc = match parse_character_descriptor_str(&raw) {
            Ok(d) => d,
            Err(e) => return self.err_response(e),
        };

        let display_path = PathBuf::from(friendly_import_name(&path));
        let Some(ent) = self.project.stage.find_mut(entity_id) else {
            return self.err_response("Objeto de escenario no encontrado");
        };

        ent.character_config_path = Some(display_path);
        let (mapped_anims, applied_offsets) = ent.apply_character_descriptor(&desc, true);
        let final_x = ent.x;
        let final_y = ent.y;
        let format_label = desc.format_label;

        self.save_active_project_now();
        self.ok_response(format!(
            "{} aplicado: pos ({:.0}, {:.0}), {} anims enlazadas, {} offsets",
            format_label, final_x, final_y, mapped_anims, applied_offsets
        ))
    }

    pub fn set_entity_anim_offset(
        &mut self,
        entity_id: &str,
        kind_key: &str,
        offset_x: f32,
        offset_y: f32,
    ) -> String {
        let Some(kind) = parse_anim_kind(kind_key) else {
            return self.err_response("Tipo de animación desconocido");
        };
        let Some(ent) = self.project.stage.find_mut(entity_id) else {
            return self.err_response("Objeto de escenario no encontrado");
        };
        ent.set_anim_offset(kind, offset_x, offset_y);
        self.save_active_project_now();
        self.ok_response(format!(
            "Offset {} ajustado a ({:.0}, {:.0})",
            kind.label_es(),
            offset_x,
            offset_y
        ))
    }

    pub fn set_sparrow_binding(&mut self, entity_id: &str, kind_key: &str, prefix: &str) -> String {
        let Some(kind) = parse_anim_kind(kind_key) else {
            return self.err_response("Tipo de animación desconocido");
        };
        let mut updated = false;
        if let Some(ent) = self.project.stage.find_mut(entity_id) {
            if let Some(atlas) = &mut ent.atlas {
                atlas.bindings.insert(kind, prefix.to_string());
                updated = true;
            }
        }
        if updated {
            self.save_active_project_now();
            self.ok_response("Animación asignada")
        } else {
            self.err_response("El objeto no tiene un atlas cargado")
        }
    }

    pub fn get_sparrow_frame_info(
        &self,
        entity_id: &str,
        kind_key: &str,
        elapsed_sec: f32,
    ) -> String {
        let kind = parse_anim_kind(kind_key).unwrap_or(CharacterAnimKind::Idle);
        let frame = self
            .project
            .stage
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .and_then(|e| e.atlas.as_ref())
            .and_then(|a| a.frame_at(kind, elapsed_sec));

        let resp = match frame {
            Some(f) => SparrowFrameJson {
                found: true,
                raw_name: f.raw_name.clone(),
                x: f.x,
                y: f.y,
                width: f.width,
                height: f.height,
                frame_x: f.frame_x,
                frame_y: f.frame_y,
                frame_width: f.frame_width,
                frame_height: f.frame_height,
                rotated: f.rotated,
            },
            None => SparrowFrameJson {
                found: false,
                raw_name: String::new(),
                x: 0,
                y: 0,
                width: 0,
                height: 0,
                frame_x: 0,
                frame_y: 0,
                frame_width: 0,
                frame_height: 0,
                rotated: false,
            },
        };
        serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn import_character_icon(
        &mut self,
        entity_id: &str,
        is_losing: bool,
        path: PathBuf,
    ) -> String {
        if !path.is_file() {
            return self.err_response("Archivo de ícono no encontrado");
        }
        if self.project.stage.find_mut(entity_id).is_none() {
            return self.err_response("Personaje no encontrado en el escenario");
        }
        if read_sprite_dimensions(&path).is_none() {
            return self.err_response("Archivo de ícono inválido o corrupto");
        }
        let stem_hint = format!("{}_{}", entity_id, if is_losing { "losing" } else { "normal" });
        let adopted_path = match self.adopt_permanent_asset(&path, "icons", &stem_hint) {
            Ok(p) => p,
            Err(e) => return self.err_response(e),
        };

        let mut old_icon: Option<PathBuf> = None;
        if let Some(ent) = self.project.stage.find_mut(entity_id) {
            if is_losing {
                old_icon = ent.icon_losing_path.take();
                ent.icon_losing_path = Some(adopted_path);
            } else {
                old_icon = ent.icon_normal_path.take();
                ent.icon_normal_path = Some(adopted_path);
            }
        }
        self.remove_managed_asset_if_unreferenced(old_icon.as_deref());
        self.save_active_project_now();
        self.ok_response(if is_losing {
            "Ícono en estado perdiendo asignado"
        } else {
            "Ícono normal de personaje asignado"
        })
    }

    pub fn clear_character_icons(&mut self, entity_id: &str) -> String {
        let (old_normal, old_losing) = if let Some(ent) = self.project.stage.find_mut(entity_id) {
            (ent.icon_normal_path.take(), ent.icon_losing_path.take())
        } else {
            return self.err_response("Personaje no encontrado en el escenario");
        };
        self.remove_managed_asset_if_unreferenced(old_normal.as_deref());
        self.remove_managed_asset_if_unreferenced(old_losing.as_deref());
        self.save_active_project_now();
        self.ok_response("Íconos SVG predeterminados restaurados")
    }
}
