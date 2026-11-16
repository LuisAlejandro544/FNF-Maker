//! Archivo: engine/crates/pulse_ui/src/lib.rs
//! Propósito: Aplicación principal `BeatForgeApp` implementando `eframe::App`.
//! Conecta la barra superior de navegación horizontal, el enrutador multi-pantalla,
//! los popups modales y el puente de archivos nativos (`PlatformFilePicker`) hacia Android SAF.

pub mod popups;
pub mod screens;
pub mod state;
pub mod theme;

use crate::popups::arrow_skin_popup::draw_arrow_skin_popup;
use crate::screens::{
    chart_audio_screen::draw_chart_audio_screen, compile_screen::draw_compile_screen,
    gameplay_screen::draw_gameplay_screen, hub_screen::draw_hub_screen,
    script_studio_screen::draw_script_studio_screen, stage_editor_screen::draw_stage_editor_screen,
    sprite_lab_screen::draw_sprite_lab_screen,
};
use crate::state::{ActiveScreen, AppState, ImportKind, PlatformFilePicker};
use eframe::egui;
use std::sync::Arc;

pub struct BeatForgeApp {
    pub state: AppState,
}

impl BeatForgeApp {
    pub fn new(cc: &eframe::CreationContext<'_>, file_picker: Arc<dyn PlatformFilePicker>) -> Self {
        theme::apply_studio_theme(&cc.egui_ctx);
        let mut state = AppState::new(file_picker);
        state.refresh_arrow_textures(&cc.egui_ctx);
        Self { state }
    }
}

impl eframe::App for BeatForgeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Procesar archivos recién importados desde Android SAF (hilo IO coroutine)
        // y progreso de compilación en caliente o decodificación de audio.
        self.state.poll_background_events(ctx);

        // Si estamos en gameplay, ocultamos la barra de editor para vista horizontal inmersiva.
        if self.state.screen != ActiveScreen::Gameplay && self.state.screen != ActiveScreen::HotCompile {
            draw_top_studio_nav(ctx, &mut self.state);
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(theme::BG_CANVAS).inner_margin(12.0))
            .show(ctx, |ui| match self.state.screen {
                ActiveScreen::Hub => draw_hub_screen(ui, &mut self.state),
                ActiveScreen::ChartAndAudio => draw_chart_audio_screen(ui, &mut self.state),
                ActiveScreen::SpriteLab => draw_sprite_lab_screen(ui, &mut self.state),
                ActiveScreen::StageEditor => draw_stage_editor_screen(ui, &mut self.state),
                ActiveScreen::ScriptStudio => draw_script_studio_screen(ui, &mut self.state),
                ActiveScreen::HotCompile => draw_compile_screen(ui, &mut self.state),
                ActiveScreen::Gameplay => draw_gameplay_screen(ui, &mut self.state),
            });

        // Popups modales dedicados (sin amontonar en las pantallas principales).
        if self.state.show_arrow_skin_popup {
            draw_arrow_skin_popup(ctx, &mut self.state);
        }

        if let Some(msg) = self.state.toast_message.clone() {
            draw_toast_banner(ctx, &msg, &mut self.state);
        }

        // En Gameplay o Compilación en caliente solicitamos repintado continuo a 60+ FPS.
        if matches!(
            self.state.screen,
            ActiveScreen::Gameplay | ActiveScreen::HotCompile | ActiveScreen::SpriteLab
        ) {
            ctx.request_repaint();
        }
    }
}

fn draw_top_studio_nav(ctx: &egui::Context, state: &mut AppState) {
    egui::TopBottomPanel::top("studio_nav_bar")
        .exact_height(52.0)
        .frame(
            egui::Frame::NONE
                .fill(theme::BG_ELEVATED)
                .inner_margin(egui::Margin::symmetric(14, 8))
                .stroke(egui::Stroke::new(1.0, theme::BORDER_SUBTLE)),
        )
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(
                    egui::RichText::new("BEATFORGE STUDIO")
                        .strong()
                        .size(15.0)
                        .color(theme::ACCENT_TEAL),
                );
                ui.add_space(12.0);

                let nav_items = [
                    (ActiveScreen::Hub, "Inicio"),
                    (ActiveScreen::ChartAndAudio, "Chart y Música"),
                    (ActiveScreen::SpriteLab, "Sprites XML"),
                    (ActiveScreen::StageEditor, "Escenario"),
                    (ActiveScreen::ScriptStudio, "Scripts (Lua/TS/JS)"),
                ];

                for (target, label) in nav_items {
                    let active = state.screen == target;
                    let btn = egui::Button::new(
                        egui::RichText::new(label)
                            .size(13.0)
                            .color(if active { theme::BG_CANVAS } else { theme::TEXT_PRIMARY }),
                    )
                    .fill(if active { theme::ACCENT_TEAL } else { theme::BG_CARD })
                    .corner_radius(6.0);

                    if ui.add_sized([0.0, 32.0], btn).clicked() {
                        state.screen = target;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let play_btn = egui::Button::new(
                        egui::RichText::new("▶ Compilar y Jugar")
                            .strong()
                            .size(13.0)
                            .color(theme::BG_CANVAS),
                    )
                    .fill(theme::ACCENT_EMERALD)
                    .corner_radius(6.0);
                    if ui.add_sized([155.0, 34.0], play_btn).clicked() {
                        state.start_hot_compile();
                    }

                    ui.add_space(8.0);
                    let skin_btn = egui::Button::new(
                        egui::RichText::new("Skin Flechas SVG")
                            .size(12.5)
                            .color(theme::TEXT_PRIMARY),
                    )
                    .fill(theme::BG_CARD)
                    .stroke(egui::Stroke::new(1.0, theme::ACCENT_INDIGO))
                    .corner_radius(6.0);
                    if ui.add_sized([125.0, 32.0], skin_btn).clicked() {
                        state.show_arrow_skin_popup = true;
                    }

                    // Acceso directo rápido si necesita importar cualquier archivo.
                    let _ = ImportKind::ChartJson;
                });
            });
        });
}

fn draw_toast_banner(ctx: &egui::Context, msg: &str, state: &mut AppState) {
    egui::Area::new(egui::Id::new("studio_toast"))
        .anchor(egui::Align2::CENTER_BOTTOM, [0.0, -14.0])
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(theme::BG_CARD)
                .stroke(egui::Stroke::new(1.2, theme::ACCENT_TEAL))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::symmetric(14, 8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(msg).color(theme::TEXT_PRIMARY).size(12.5));
                        if ui.small_button("OK").clicked() {
                            state.toast_message = None;
                        }
                    });
                });
        });
}
