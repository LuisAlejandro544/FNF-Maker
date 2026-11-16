//! Archivo: engine/crates/pulse_ui/src/screens/chart_audio_screen.rs
//! Propósito: Pantalla dedicada a:
//! 1. Elegir/Importar Chart `.json` de algún mod y visualizar BPM, velocidad e histograma de notas.
//! 2. Importar hasta MÁXIMO 3 pistas de música (1: Mix completo o Instrumental, 2: Voces Player,
//!    3: Voces Enemigo) con control de ganancia, mute, forma de onda y prueba en Oboe.

use crate::state::{AppState, ImportKind};
use crate::theme;
use eframe::egui;
use pulse_core::ChartData;
use pulse_core::project::MAX_AUDIO_TRACKS;

pub fn draw_chart_audio_screen(ui: &mut egui::Ui, state: &mut AppState) {
    ui.columns(2, |cols| {
        // Panel izquierdo: Importador y analizador de Chart
        theme::card_frame().show(&mut cols[0], |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("1. Chart del Mod (.json)")
                        .strong()
                        .size(17.0)
                        .color(theme::ACCENT_TEAL),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(egui::RichText::new("📂 Importar Chart .json").strong())
                        .clicked()
                    {
                        state.trigger_import(ImportKind::ChartJson);
                    }
                    if ui.button("Usar Demo").clicked() {
                        state.project.chart = ChartData::bundled_demo_chart();
                        state.project.chart_source_path = None;
                        state.toast_message = Some("Chart demo restaurado".to_string());
                    }
                });
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            let chart = &mut state.project.chart;
            ui.label(
                egui::RichText::new(format!("Canción: {}", chart.title))
                    .strong()
                    .size(15.0),
            );
            ui.add_space(6.0);

            ui.horizontal(|ui| {
                ui.label(format!("BPM Base: {:.1}", chart.bpm));
                ui.add_space(16.0);
                ui.label(format!("Secciones: {}", chart.sections.len()));
                ui.add_space(16.0);
                ui.label(format!(
                    "Duración: {:.1}s",
                    chart.stats.duration_ms / 1000.0
                ));
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Velocidad de flechas (Scroll Speed):");
                ui.add(egui::Slider::new(&mut chart.speed, 0.5..=4.5).step_by(0.05));
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!("Total: {} notas", chart.stats.total_notes))
                        .color(theme::TEXT_PRIMARY),
                );
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(format!("Player: {}", chart.stats.player_notes))
                        .color(theme::ACCENT_EMERALD),
                );
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(format!("Enemigo: {}", chart.stats.opponent_notes))
                        .color(theme::ACCENT_CORAL),
                );
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(format!("Sost.: {}", chart.stats.sustain_notes))
                        .color(theme::ACCENT_INDIGO),
                );
            });

            ui.add_space(12.0);
            ui.label(
                egui::RichText::new("Mapa de Densidad de Notas")
                    .size(12.0)
                    .color(theme::TEXT_MUTED),
            );
            draw_density_histogram(ui, &chart.stats.density_buckets);
        });

        // Panel derecho: Importador de hasta máximo 3 audios en Oboe
        theme::card_frame().show(&mut cols[1], |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("2. Música y Voces (Máx. 3 Pistas Oboe)")
                        .strong()
                        .size(17.0)
                        .color(theme::ACCENT_INDIGO),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_playing = state
                        .audio_engine
                        .mixer
                        .is_playing
                        .load(std::sync::atomic::Ordering::Relaxed);
                    if ui
                        .button(if is_playing {
                            "⏹ Detener Preview"
                        } else {
                            "▶ Escuchar Mezcla"
                        })
                        .clicked()
                    {
                        if is_playing {
                            state.audio_engine.mixer.pause();
                        } else {
                            state.audio_engine.ensure_started();
                            state.audio_engine.mixer.seek_ms(0.0);
                            state.audio_engine.mixer.play();
                        }
                    }
                });
            });

            ui.label(
                egui::RichText::new(
                    "Soporta mods de 1 solo archivo (todo mezclado), 2 archivos (Inst + Voces) o 3 archivos (Inst + Player + Enemigo).",
                )
                .size(11.5)
                .color(theme::TEXT_MUTED),
            );
            ui.add_space(8.0);

            let mut import_requested_slot: Option<usize> = None;
            for slot in 0..MAX_AUDIO_TRACKS {
                let role_label = state.project.audio_tracks[slot].role_label.clone();
                let has_file = state.project.audio_tracks[slot].file_path.is_some();
                let file_lbl = state.project.audio_tracks[slot]
                    .file_name
                    .clone()
                    .unwrap_or_else(|| "Sin archivo asignado".to_string());

                egui::Frame::NONE
                    .fill(theme::BG_CARD)
                    .stroke(egui::Stroke::new(1.0, theme::BORDER_SUBTLE))
                    .corner_radius(8.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&role_label)
                                    .strong()
                                    .color(theme::TEXT_PRIMARY),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if has_file && ui.button("Quitar").clicked() {
                                    state.project.audio_tracks[slot].file_path = None;
                                    state.project.audio_tracks[slot].file_name = None;
                                    state.decoded_tracks[slot] = None;
                                    state.audio_engine.mixer.set_track(slot, None);
                                }
                                if ui.button("📂 Importar (.ogg/.wav/.mp3)").clicked() {
                                    import_requested_slot = Some(slot);
                                }
                            });
                        });

                        ui.label(
                            egui::RichText::new(&file_lbl)
                                .size(11.5)
                                .color(if has_file {
                                    theme::ACCENT_EMERALD
                                } else {
                                    theme::TEXT_MUTED
                                }),
                        );

                        ui.horizontal(|ui| {
                            ui.label("Vol:");
                            if ui
                                .add(egui::Slider::new(
                                    &mut state.project.audio_tracks[slot].gain,
                                    0.0..=1.5,
                                ))
                                .changed()
                            {
                                state.audio_engine.mixer.configs[slot]
                                    .set_gain(state.project.audio_tracks[slot].gain);
                            }
                            if ui
                                .checkbox(&mut state.project.audio_tracks[slot].muted, "Mute")
                                .changed()
                            {
                                state.audio_engine.mixer.configs[slot].muted.store(
                                    state.project.audio_tracks[slot].muted,
                                    std::sync::atomic::Ordering::Relaxed,
                                );
                            }
                            if slot == 1 {
                                ui.checkbox(
                                    &mut state.project.audio_tracks[slot].duck_on_player_miss,
                                    "Bajar al fallar",
                                );
                            }
                        });

                        if let Some(decoded) = state.decoded_tracks[slot].as_ref() {
                            draw_waveform_bars(ui, &decoded.waveform_peaks);
                        }
                    });
                ui.add_space(8.0);
            }
            if let Some(slot) = import_requested_slot {
                state.trigger_import(ImportKind::AudioTrack(slot));
            }
        });
    });
}

fn draw_density_histogram(ui: &mut egui::Ui, buckets: &[u16]) {
    let desired = egui::vec2(ui.available_width(), 90.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 6.0, theme::BG_WELL);

    if buckets.is_empty() {
        return;
    }
    let max_val = (*buckets.iter().max().unwrap_or(&1)).max(1) as f32;
    let bar_w = rect.width() / (buckets.len() as f32);

    for (i, &val) in buckets.iter().enumerate() {
        let ratio = (val as f32) / max_val;
        let h = (rect.height() - 12.0) * ratio;
        let x0 = rect.left() + (i as f32) * bar_w + 1.5;
        let x1 = (x0 + bar_w - 3.0).max(x0 + 1.0);
        let bar_rect = egui::Rect::from_min_max(
            egui::pos2(x0, rect.bottom() - 6.0 - h),
            egui::pos2(x1, rect.bottom() - 6.0),
        );
        painter.rect_filled(bar_rect, 2.0, theme::ACCENT_TEAL);
    }
}

fn draw_waveform_bars(ui: &mut egui::Ui, peaks: &[f32]) {
    let desired = egui::vec2(ui.available_width(), 28.0);
    let (rect, _) = ui.allocate_exact_size(desired, egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, theme::BG_WELL);

    if peaks.is_empty() {
        return;
    }
    let bar_w = rect.width() / (peaks.len() as f32);
    let cy = rect.center().y;
    for (i, &p) in peaks.iter().enumerate() {
        let half_h = ((rect.height() * 0.44) * p).max(1.5);
        let x = rect.left() + (i as f32) * bar_w + bar_w * 0.5;
        painter.line_segment(
            [egui::pos2(x, cy - half_h), egui::pos2(x, cy + half_h)],
            egui::Stroke::new((bar_w * 0.6).max(1.5), theme::ACCENT_INDIGO),
        );
    }
}
