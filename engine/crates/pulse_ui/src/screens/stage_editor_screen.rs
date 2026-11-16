//! Archivo: engine/crates/pulse_ui/src/screens/stage_editor_screen.rs
//! Propósito: Editor visual de escenario horizontal.
//! Permite al usuario:
//! - Importar nuevos objetos y asignarles su identificador (`Background`, `Player`, `Opponent`, `Decoration`).
//! - Mover el objeto seleccionado con botones de flechas (cruceta Arriba/Abajo/Izquierda/Derecha) o arrastrando.
//! - Cambiar tamaño (escala), opacidad (0% a 100%) y orden de capa Z (poner encima o debajo de otro objeto).

use crate::state::{AppState, ImportKind};
use crate::theme;
use eframe::egui;
use pulse_core::{EntityRole, StageEntity};

pub fn draw_stage_editor_screen(ui: &mut egui::Ui, state: &mut AppState) {
    let avail = ui.available_size();
    let left_w = 240.0_f32;
    let right_w = 290.0_f32;
    let center_w = (avail.x - left_w - right_w - 20.0).max(260.0);

    ui.horizontal_top(|ui| {
        // 1. Panel Izquierdo: Capas y orden Z
        ui.allocate_ui(egui::vec2(left_w, avail.y), |ui| {
            theme::card_frame().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Capas (Orden Z)")
                            .strong()
                            .color(theme::TEXT_PRIMARY),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("+ Asset").clicked() {
                            let idx = state.project.stage.entities.len() + 1;
                            let id = format!("asset_{}", idx);
                            let ent = StageEntity::new(
                                id.clone(),
                                format!("Objeto {}", idx),
                                EntityRole::Decoration,
                            );
                            state.project.stage.entities.push(ent);
                            state.project.stage.sort_by_z_order();
                            state.selected_entity_id = id;
                        }
                    });
                });

                ui.add_space(6.0);
                let selected_id = state.selected_entity_id.clone();

                // Controles para poner encima o debajo de otros objetos
                ui.horizontal(|ui| {
                    if ui.button("▲ Subir").clicked() {
                        state.project.stage.bring_forward(&selected_id);
                    }
                    if ui.button("▼ Bajar").clicked() {
                        state.project.stage.send_backward(&selected_id);
                    }
                    if ui.button("⤒ Frente").clicked() {
                        state.project.stage.bring_to_front(&selected_id);
                    }
                    if ui.button("⤓ Fondo").clicked() {
                        state.project.stage.send_to_back(&selected_id);
                    }
                });

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(6.0);

                egui::ScrollArea::vertical()
                    .max_height(avail.y - 120.0)
                    .show(ui, |ui| {
                        // Mostrar de frente hacia atrás (Z mayor arriba)
                        let ordered: Vec<(String, String, EntityRole, i32, bool)> = state
                            .project
                            .stage
                            .entities
                            .iter()
                            .rev()
                            .map(|e| (e.id.clone(), e.display_name.clone(), e.role, e.z_index, e.visible))
                            .collect();

                        for (id, name, role, z, visible) in ordered {
                            let is_sel = state.selected_entity_id == id;
                            let role_col = theme::role_color(role);
                            let frame = egui::Frame::NONE
                                .fill(if is_sel { theme::BG_CARD } else { theme::BG_WELL })
                                .stroke(egui::Stroke::new(
                                    if is_sel { 1.4 } else { 1.0 },
                                    if is_sel { theme::ACCENT_TEAL } else { theme::BORDER_SUBTLE },
                                ))
                                .corner_radius(6.0)
                                .inner_margin(8.0);

                            frame.show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    let eye = if visible { "👁" } else { "🚫" };
                                    ui.label(eye);
                                    if ui
                                        .selectable_label(is_sel, format!("{} (Z:{})", name, z))
                                        .clicked()
                                    {
                                        state.selected_entity_id = id.clone();
                                    }
                                });
                                ui.label(
                                    egui::RichText::new(role.label_es())
                                        .size(10.5)
                                        .color(role_col),
                                );
                            });
                            ui.add_space(4.0);
                        }
                    });
            });
        });

        // 2. Panel Central: Lienzo horizontal interactivo (1280x720 virtual)
        ui.allocate_ui(egui::vec2(center_w, avail.y), |ui| {
            theme::card_frame().show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Escenario Horizontal (1280x720)")
                        .strong()
                        .color(theme::TEXT_PRIMARY),
                );
                ui.add_space(4.0);

                let space = ui.available_size();
                let aspect = 16.0 / 9.0;
                let mut w = space.x;
                let mut h = w / aspect;
                if h > space.y - 8.0 {
                    h = (space.y - 8.0).max(160.0);
                    w = h * aspect;
                }

                let (rect, response) =
                    ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click_and_drag());
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, 6.0, theme::BG_WELL);

                // Rejilla guía sutil
                for gx in 1..4 {
                    let x = rect.left() + (w * (gx as f32) / 4.0);
                    painter.line_segment(
                        [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                        egui::Stroke::new(0.6, theme::BORDER_SUBTLE),
                    );
                }
                for gy in 1..3 {
                    let y = rect.top() + (h * (gy as f32) / 3.0);
                    painter.line_segment(
                        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                        egui::Stroke::new(0.6, theme::BORDER_SUBTLE),
                    );
                }

                let scale_x = w / state.project.stage.virtual_width;
                let scale_y = h / state.project.stage.virtual_height;

                // Arrastrar objeto seleccionado directamente en pantalla
                if response.dragged() {
                    let delta = response.drag_delta();
                    let sel = state.selected_entity_id.clone();
                    if let Some(ent) = state.project.stage.find_mut(&sel) {
                        ent.nudge(delta.x / scale_x, delta.y / scale_y);
                    }
                }

                for ent in &state.project.stage.entities {
                    if !ent.visible || ent.opacity <= 0.01 {
                        continue;
                    }
                    let cx = rect.left() + ent.x * scale_x;
                    let cy = rect.top() + ent.y * scale_y;
                    let ew = ent.width * ent.scale * scale_x;
                    let eh = ent.height * ent.scale * scale_y;
                    let box_rect =
                        egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(ew, eh));

                    let alpha = (ent.opacity * 255.0).round() as u8;
                    let tint = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);

                    if let Some(tex) = state.entity_textures.get(&ent.id) {
                        let uv = if ent.flip_x {
                            egui::Rect::from_min_max(egui::pos2(1.0, 0.0), egui::pos2(0.0, 1.0))
                        } else {
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0))
                        };
                        painter.image(tex.id(), box_rect, uv, tint);
                    } else {
                        let rcol = theme::role_color(ent.role);
                        let fill = egui::Color32::from_rgba_unmultiplied(
                            rcol.r(),
                            rcol.g(),
                            rcol.b(),
                            (alpha / 3).max(20),
                        );
                        painter.rect_filled(box_rect, 6.0, fill);
                    }

                    let is_sel = ent.id == state.selected_entity_id;
                    let stroke_col = if is_sel {
                        theme::ACCENT_TEAL
                    } else {
                        theme::role_color(ent.role)
                    };
                    painter.rect_stroke(
                        box_rect,
                        6.0,
                        egui::Stroke::new(if is_sel { 2.0 } else { 1.0 }, stroke_col),
                        egui::StrokeKind::Outside,
                    );
                    painter.text(
                        box_rect.left_top() + egui::vec2(4.0, 4.0),
                        egui::Align2::LEFT_TOP,
                        &ent.display_name,
                        egui::FontId::proportional(10.5),
                        theme::TEXT_PRIMARY,
                    );
                }
            });
        });

        // 3. Panel Derecho: Inspector (Identificador, Flechas de movimiento, Escala, Opacidad)
        ui.allocate_ui(egui::vec2(right_w, avail.y), |ui| {
            theme::card_frame().show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Inspector de Objeto")
                        .strong()
                        .color(theme::ACCENT_TEAL),
                );
                ui.add_space(6.0);

                let selected_id = state.selected_entity_id.clone();
                let mut import_img = false;

                if let Some(ent) = state.project.stage.find_mut(&selected_id) {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.label("Nombre:");
                        ui.text_edit_singleline(&mut ent.display_name);
                        ui.add_space(6.0);

                        ui.label(egui::RichText::new("Identificador (Rol):").strong());
                        egui::ComboBox::from_id_salt("entity_role_combo")
                            .selected_text(ent.role.label_es())
                            .show_ui(ui, |ui| {
                                for role in EntityRole::ALL {
                                    ui.selectable_value(&mut ent.role, role, role.label_es());
                                }
                            });

                        ui.add_space(8.0);
                        if ui.button("🖼 Cambiar Imagen / Spritesheet").clicked() {
                            import_img = true;
                        }

                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(6.0);

                        // Cruceta con flechas para mover el asset en el escenario
                        ui.label(egui::RichText::new("Mover con Flechas").strong());
                        ui.horizontal(|ui| {
                            ui.label("Paso:");
                            for step in [1.0_f32, 10.0, 35.0] {
                                ui.selectable_value(
                                    &mut state.move_step_px,
                                    step,
                                    format!("{}px", step as i32),
                                );
                            }
                        });

                        let step = state.move_step_px;
                        ui.vertical_centered(|ui| {
                            if ui.add_sized([54.0, 30.0], egui::Button::new("⬆")).clicked() {
                                ent.nudge(0.0, -step);
                            }
                            ui.horizontal(|ui| {
                                ui.add_space(44.0);
                                if ui.add_sized([54.0, 30.0], egui::Button::new("⬅")).clicked() {
                                    ent.nudge(-step, 0.0);
                                }
                                if ui.add_sized([54.0, 30.0], egui::Button::new("⬇")).clicked() {
                                    ent.nudge(0.0, step);
                                }
                                if ui.add_sized([54.0, 30.0], egui::Button::new("➡")).clicked() {
                                    ent.nudge(step, 0.0);
                                }
                            });
                        });

                        ui.label(
                            egui::RichText::new(format!("Posición: X {:.0}, Y {:.0}", ent.x, ent.y))
                                .size(11.5)
                                .color(theme::TEXT_MUTED),
                        );

                        ui.add_space(8.0);
                        ui.label("Tamaño (Escala):");
                        let mut sc = ent.scale;
                        if ui.add(egui::Slider::new(&mut sc, 0.1..=4.0)).changed() {
                            ent.set_scale(sc);
                        }

                        ui.label("Opacidad:");
                        let mut op = ent.opacity;
                        if ui.add(egui::Slider::new(&mut op, 0.0..=1.0)).changed() {
                            ent.set_opacity(op);
                        }

                        ui.checkbox(&mut ent.visible, "Visible en escenario");
                        ui.checkbox(&mut ent.flip_x, "Voltear horizontal (Flip X)");
                    });
                }

                if import_img {
                    state.trigger_import(ImportKind::SpriteSheetPng);
                }
            });
        });
    });
}
