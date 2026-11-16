//! Archivo: engine/crates/pulse_ui/src/screens/sprite_lab_screen.rs
//! Propósito: Laboratorio de Sprites XML Auto-Armables.
//! El usuario importa su hoja `.png` y su archivo `.xml` Sparrow; Rust extrae automáticamente
//! todos los sprites y arma las animaciones de Reposo (Idle), Arriba, Abajo, Izquierda y Derecha
//! con previsualización en tiempo real y ajuste fino opcional.

use crate::state::{AppState, ImportKind};
use crate::theme;
use eframe::egui;
use pulse_core::CharacterAnimKind;

pub fn draw_sprite_lab_screen(ui: &mut egui::Ui, state: &mut AppState) {
    let dt = ui.input(|i| i.stable_dt.min(0.1));
    state.preview_anim_timer += dt;

    ui.columns(2, |cols| {
        // Columna izquierda: Selección de entidad e importación de PNG + XML
        theme::card_frame().show(&mut cols[0], |ui| {
            ui.label(
                egui::RichText::new("3. Importador Automático de Spritesheets (.png + .xml)")
                    .strong()
                    .size(16.5)
                    .color(theme::ACCENT_INDIGO),
            );
            ui.label(
                egui::RichText::new(
                    "Rust lee tu hoja de assets y el archivo .xml para armar automáticamente Arriba, Abajo, Izquierda, Derecha e Idle sin herramientas externas.",
                )
                .size(11.5)
                .color(theme::TEXT_MUTED),
            );
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Objeto destino:");
                egui::ComboBox::from_id_salt("sprite_lab_entity_select")
                    .selected_text(
                        state
                            .project
                            .stage
                            .entities
                            .iter()
                            .find(|e| e.id == state.selected_entity_id)
                            .map(|e| e.display_name.as_str())
                            .unwrap_or("Seleccionar"),
                    )
                    .show_ui(ui, |ui| {
                        for ent in &state.project.stage.entities {
                            ui.selectable_value(
                                &mut state.selected_entity_id,
                                ent.id.clone(),
                                format!("{} [{}]", ent.display_name, ent.role.label_es()),
                            );
                        }
                    });
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("🖼 Importar Hoja .png").clicked() {
                    state.trigger_import(ImportKind::SpriteSheetPng);
                }
                if ui.button("📄 Importar Sparrow .xml").clicked() {
                    state.trigger_import(ImportKind::SpriteAtlasXml);
                }
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            let selected_id = state.selected_entity_id.clone();
            if let Some(entity) = state.project.stage.find_mut(&selected_id) {
                if let Some(atlas) = &mut entity.atlas {
                    ui.label(
                        egui::RichText::new(format!(
                            "Animaciones detectadas ({})",
                            atlas.animations.len()
                        ))
                        .strong()
                        .color(theme::ACCENT_EMERALD),
                    );
                    ui.add_space(6.0);

                    let available_prefixes: Vec<String> = atlas.animations.keys().cloned().collect();

                    for kind in CharacterAnimKind::CORE_FIVE {
                        let current = atlas
                            .bindings
                            .get(&kind)
                            .cloned()
                            .unwrap_or_else(|| "Sin asignar".to_string());

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(kind.label_es()).strong());
                            egui::ComboBox::from_id_salt(format!("bind_{:?}", kind))
                                .selected_text(&current)
                                .show_ui(ui, |ui| {
                                    for prefix in &available_prefixes {
                                        let mut val = current.clone();
                                        if ui.selectable_value(&mut val, prefix.clone(), prefix).clicked() {
                                            atlas.bindings.insert(kind, prefix.clone());
                                        }
                                    }
                                });
                        });
                    }
                } else {
                    ui.label(
                        egui::RichText::new(
                            "Aún no has cargado un archivo .xml en este objeto. Importa un .png y su .xml para ver el auto-armado.",
                        )
                        .color(theme::TEXT_MUTED),
                    );
                }
            }
        });

        // Columna derecha: Visor animado en vivo de Idle / Izquierda / Abajo / Arriba / Derecha
        theme::card_frame().show(&mut cols[1], |ui| {
            ui.label(
                egui::RichText::new("Vista Previa Animada del Personaje / Asset")
                    .strong()
                    .size(16.5)
                    .color(theme::TEXT_PRIMARY),
            );
            ui.add_space(6.0);

            ui.horizontal_wrapped(|ui| {
                for kind in CharacterAnimKind::CORE_FIVE {
                    let active = state.preview_anim_kind == kind;
                    let btn = egui::Button::new(kind.label_es()).fill(if active {
                        theme::ACCENT_TEAL
                    } else {
                        theme::BG_CARD
                    });
                    if ui.add(btn).clicked() {
                        state.preview_anim_kind = kind;
                        state.preview_anim_timer = 0.0;
                    }
                }
            });

            ui.add_space(10.0);
            let desired = egui::vec2(ui.available_width(), ui.available_height() - 10.0);
            let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
            let painter = ui.painter();
            painter.rect_filled(rect, 8.0, theme::BG_WELL);
            painter.rect_stroke(
                rect,
                8.0,
                egui::Stroke::new(1.0, theme::BORDER_SUBTLE),
                egui::StrokeKind::Inside,
            );

            let selected_id = state.selected_entity_id.clone();
            let tex_opt = state.entity_textures.get(&selected_id);
            let entity_opt = state.project.stage.entities.iter().find(|e| e.id == selected_id);

            if let (Some(tex), Some(entity)) = (tex_opt, entity_opt) {
                let tex_size = tex.size_vec2();
                if let Some(atlas) = &entity.atlas {
                    if let Some(frame) = atlas.frame_at(state.preview_anim_kind, state.preview_anim_timer) {
                        let uv = egui::Rect::from_min_max(
                            egui::pos2(
                                (frame.x as f32) / tex_size.x.max(1.0),
                                (frame.y as f32) / tex_size.y.max(1.0),
                            ),
                            egui::pos2(
                                ((frame.x + frame.width) as f32) / tex_size.x.max(1.0),
                                ((frame.y + frame.height) as f32) / tex_size.y.max(1.0),
                            ),
                        );
                        let aspect = (frame.width as f32) / (frame.height.max(1) as f32);
                        let max_h = (rect.height() - 40.0).max(64.0);
                        let draw_h = max_h.min((rect.width() - 40.0) / aspect.max(0.1));
                        let draw_w = draw_h * aspect;
                        let draw_rect = egui::Rect::from_center_size(
                            rect.center(),
                            egui::vec2(draw_w, draw_h),
                        );
                        painter.image(tex.id(), draw_rect, uv, egui::Color32::WHITE);
                        painter.text(
                            egui::pos2(rect.left() + 12.0, rect.bottom() - 14.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!("Frame: {} ({}x{})", frame.raw_name, frame.width, frame.height),
                            egui::FontId::proportional(11.5),
                            theme::TEXT_MUTED,
                        );
                        return;
                    }
                }

                // Fallback visual si solo subió PNG estático sin XML
                let aspect = tex_size.x / tex_size.y.max(1.0);
                let draw_h = (rect.height() - 40.0).max(64.0);
                let draw_w = (draw_h * aspect).min(rect.width() - 40.0);
                let draw_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(draw_w, draw_h));
                painter.image(
                    tex.id(),
                    draw_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            } else {
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Importa un .png + .xml para previsualizar el sprite armado",
                    egui::FontId::proportional(13.5),
                    theme::TEXT_MUTED,
                );
            }
        });
    });
}
