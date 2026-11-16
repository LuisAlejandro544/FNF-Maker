//! Archivo: engine/crates/pulse_ui/src/screens/compile_screen.rs
//! Propósito: Pantalla de carga mostrada mientras Rust compila en caliente en segundo plano:
//! optimiza spritesheets según recursos del dispositivo, descarta objetos permanentemente
//! invisibles durante toda la canción y rasteriza flechas `.svg`.

use crate::state::AppState;
use crate::theme;
use eframe::egui;

pub fn draw_compile_screen(ui: &mut egui::Ui, state: &mut AppState) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.28);

        egui::Frame::NONE
            .fill(theme::BG_ELEVATED)
            .stroke(egui::Stroke::new(1.2, theme::ACCENT_TEAL))
            .corner_radius(12.0)
            .inner_margin(28.0)
            .show(ui, |ui| {
                ui.set_width(520.0);
                ui.label(
                    egui::RichText::new("COMPILANDO EN CALIENTE")
                        .strong()
                        .size(20.0)
                        .color(theme::ACCENT_TEAL),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(&state.compile_status_text)
                        .size(13.5)
                        .color(theme::TEXT_PRIMARY),
                );
                ui.add_space(14.0);

                ui.add(
                    egui::ProgressBar::new(state.compile_progress.clamp(0.0, 1.0))
                        .show_percentage()
                        .desired_width(480.0),
                );

                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(format!(
                        "Perfil activo: {} • Descartando assets invisibles y optimizando texturas en paralelo...",
                        state.resource_tier.label_es()
                    ))
                    .size(11.5)
                    .color(theme::TEXT_MUTED),
                );
            });
    });
}
