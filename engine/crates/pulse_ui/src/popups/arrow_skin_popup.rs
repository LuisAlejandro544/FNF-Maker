//! Archivo: engine/crates/pulse_ui/src/popups/arrow_skin_popup.rs
//! Propósito: Popup modal para personalizar la skin de flechas en formato `.svg`.
//! Por defecto usa nuestras flechas vectoriales originales libres de derechos, y permite
//! al usuario reemplazar cualquiera de las 4 direcciones con su propio archivo `.svg`.

use crate::state::{AppState, ImportKind};
use crate::theme;
use eframe::egui;
use pulse_core::NoteDirection;
use pulse_core::svg_skin::ArrowSkinSet;

pub fn draw_arrow_skin_popup(ctx: &egui::Context, state: &mut AppState) {
    let mut open = state.show_arrow_skin_popup;
    let mut refresh_needed = false;

    egui::Window::new("Personalizar Skin de Flechas (.svg)")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([620.0, 340.0])
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(
                    "Usa nuestras flechas SVG originales libres de derechos o importa tus propios archivos .svg para cada dirección.",
                )
                .size(12.0)
                .color(theme::TEXT_MUTED),
            );
            ui.add_space(10.0);

            ui.columns(4, |cols| {
                for (idx, dir) in NoteDirection::ALL.iter().enumerate() {
                    let col = &mut cols[idx];
                    egui::Frame::NONE
                        .fill(theme::BG_CARD)
                        .stroke(egui::Stroke::new(1.0, theme::BORDER_SUBTLE))
                        .corner_radius(8.0)
                        .inner_margin(10.0)
                        .show(col, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.label(egui::RichText::new(dir.label_es()).strong());
                                ui.add_space(6.0);
                                if let Some(tex) = state.arrow_textures.get(dir) {
                                    ui.image((tex.id(), egui::vec2(72.0, 72.0)));
                                }
                                ui.add_space(8.0);
                                if ui.button("Cambiar .svg").clicked() {
                                    state.trigger_import(ImportKind::ArrowSvg(*dir));
                                }
                            });
                        });
                }
            });

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui.button("Restaurar SVG Original").clicked() {
                    state.project.arrow_skin = ArrowSkinSet::bundled_default();
                    refresh_needed = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(egui::RichText::new("Listo").strong())
                        .clicked()
                    {
                        state.show_arrow_skin_popup = false;
                    }
                });
            });
        });

    if refresh_needed {
        state.refresh_arrow_textures(ctx);
    }
    if !open {
        state.show_arrow_skin_popup = false;
    }
}
