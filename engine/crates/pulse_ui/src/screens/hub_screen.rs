//! Archivo: engine/crates/pulse_ui/src/screens/hub_screen.rs
//! Propósito: Pantalla de Inicio / Hub del Proyecto en formato horizontal.
//! Muestra el resumen del mod activo, botones directos para ir a cada apartado separado
//! (Chart y Música, Sprites XML, Escenario, Scripts, Skin SVG), selector de recursos
//! del teléfono para el compilador en caliente y configuración de gameplay.

use crate::state::{ActiveScreen, AppState};
use crate::theme;
use eframe::egui;
use pulse_core::DeviceResourceTier;

pub fn draw_hub_screen(ui: &mut egui::Ui, state: &mut AppState) {
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.columns(2, |cols| {
        // Columna izquierda: Resumen del proyecto y accesos directos modulares
        theme::card_frame().show(&mut cols[0], |ui| {
            ui.label(
                egui::RichText::new("Proyecto Activo")
                    .strong()
                    .size(18.0)
                    .color(theme::TEXT_PRIMARY),
            );
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Nombre:").color(theme::TEXT_MUTED));
                ui.text_edit_singleline(&mut state.project.name);
            });
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Autor:").color(theme::TEXT_MUTED));
                ui.text_edit_singleline(&mut state.project.author);
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            // Tarjetas informativas rápidas
            ui.horizontal_wrapped(|ui| {
                stat_chip(
                    ui,
                    "Chart",
                    &format!("{} notas", state.project.chart.stats.total_notes),
                    theme::ACCENT_TEAL,
                );
                stat_chip(
                    ui,
                    "Pistas Audio",
                    &format!("{}/3 cargadas", state.project.active_audio_count()),
                    theme::ACCENT_INDIGO,
                );
                stat_chip(
                    ui,
                    "Escenario",
                    &format!("{} objetos", state.project.stage.entities.len()),
                    theme::ACCENT_EMERALD,
                );
                stat_chip(
                    ui,
                    "Scripts",
                    &format!("{} activos", state.project.scripts.iter().filter(|s| s.enabled).count()),
                    theme::ACCENT_CORAL,
                );
            });

            ui.add_space(14.0);
            ui.label(
                egui::RichText::new("Módulos del Motor (Pantallas Separadas)")
                    .strong()
                    .color(theme::TEXT_PRIMARY),
            );
            ui.add_space(6.0);

            let modules = [
                (
                    ActiveScreen::ChartAndAudio,
                    "1. Elegir Chart y Música (1-3 pistas)",
                    "Importar .json y pistas .ogg/.wav/.mp3 en Oboe",
                    theme::ACCENT_TEAL,
                ),
                (
                    ActiveScreen::SpriteLab,
                    "2. Sprites XML Auto-Armables",
                    "Sube .png + .xml y Rust arma Idle/Arriba/Abajo/Izq/Der",
                    theme::ACCENT_INDIGO,
                ),
                (
                    ActiveScreen::StageEditor,
                    "3. Editor de Escenario e Identificadores",
                    "Mueve con flechas, escala, opacidad, orden Z y rol",
                    theme::ACCENT_EMERALD,
                ),
                (
                    ActiveScreen::ScriptStudio,
                    "4. Scripts Comunitarios (Lua / TS / JS)",
                    "Edita o importa lógica personalizada en tiempo real",
                    theme::ACCENT_CORAL,
                ),
            ];

            for (screen, title, desc, accent) in modules {
                let frame = egui::Frame::NONE
                    .fill(theme::BG_CARD)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER_SUBTLE))
                    .corner_radius(8.0)
                    .inner_margin(10.0);
                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(title).strong().color(accent));
                            ui.label(egui::RichText::new(desc).size(11.5).color(theme::TEXT_MUTED));
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Abrir →").clicked() {
                                state.screen = screen;
                            }
                        });
                    });
                });
                ui.add_space(6.0);
            }
        });

        // Columna derecha: Optimización del teléfono, flechas SVG y lanzamiento
        theme::card_frame().show(&mut cols[1], |ui| {
            ui.label(
                egui::RichText::new("Optimización y Compilación en Caliente")
                    .strong()
                    .size(18.0)
                    .color(theme::TEXT_PRIMARY),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(
                    "Antes de iniciar la canción, Rust optimiza texturas en paralelo y descarta objetos invisibles.",
                )
                .size(12.0)
                .color(theme::TEXT_MUTED),
            );
            ui.add_space(8.0);

            ui.label(egui::RichText::new("Perfil de recursos del teléfono:").strong());
            for tier in [
                DeviceResourceTier::LowRam32Bit,
                DeviceResourceTier::Balanced,
                DeviceResourceTier::HighQuality,
            ] {
                ui.radio_value(&mut state.resource_tier, tier, tier.label_es());
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(8.0);

            ui.label(egui::RichText::new("Preferencias de Juego").strong());
            ui.checkbox(
                &mut state.project.downscroll,
                "Modo Downscroll (las flechas bajan en lugar de subir)",
            );
            ui.horizontal(|ui| {
                ui.label("Offset Audio Oboe (ms):");
                ui.add(egui::Slider::new(&mut state.project.audio_offset_ms, -250.0..=250.0));
            });

            if let Some(report) = &state.last_compile_report {
                ui.add_space(8.0);
                egui::Frame::NONE
                    .fill(theme::BG_WELL)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER_SUBTLE))
                    .corner_radius(8.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new("Última Compilación en Caliente")
                                .strong()
                                .color(theme::ACCENT_EMERALD),
                        );
                        ui.label(format!("• Tiempo: {} ms", report.elapsed_ms));
                        ui.label(format!("• Objetos activos: {}", report.kept_entities));
                        ui.label(format!(
                            "• Objetos invisibles descartados: {}",
                            report.discarded_invisible_entities.len()
                        ));
                        ui.label(format!(
                            "• RAM/VRAM ahorrada: {} KB",
                            report.estimated_vram_saved_kb
                        ));
                    });
            }

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                let skin_btn = egui::Button::new("Personalizar Flechas .SVG")
                    .fill(theme::BG_CARD)
                    .stroke(egui::Stroke::new(1.0, theme::ACCENT_INDIGO));
                if ui.add_sized([190.0, 42.0], skin_btn).clicked() {
                    state.show_arrow_skin_popup = true;
                }

                let launch_btn = egui::Button::new(
                    egui::RichText::new("▶ COMPILAR Y JUGAR")
                        .strong()
                        .size(15.0)
                        .color(theme::BG_CANVAS),
                )
                .fill(theme::ACCENT_EMERALD)
                .corner_radius(8.0);
                if ui
                    .add_sized([ui.available_width().max(140.0), 42.0], launch_btn)
                    .clicked()
                {
                    state.start_hot_compile();
                }
            });
        });
            });
        });
}

fn stat_chip(ui: &mut egui::Ui, label: &str, value: &str, color: egui::Color32) {
    egui::Frame::NONE
        .fill(theme::BG_WELL)
        .stroke(egui::Stroke::new(1.0, color))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(label).size(10.5).color(theme::TEXT_MUTED));
                ui.label(egui::RichText::new(value).strong().size(13.0).color(color));
            });
        });
}
