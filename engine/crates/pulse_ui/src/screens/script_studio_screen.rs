//! Archivo: engine/crates/pulse_ui/src/screens/script_studio_screen.rs
//! Propósito: Estudio de Scripts Comunitarios para Lua 5.4, TypeScript y JavaScript.
//! Permite crear, importar, activar/desactivar, editar código y verificar la sintaxis
//! en tiempo real usando el compilador TypeScript `oxc`, QuickJS-NG y `mlua`.

use crate::state::{AppState, ImportKind};
use crate::theme;
use eframe::egui;
use pulse_core::{ScriptAsset, ScriptLanguage};
use pulse_scripting::validate_script_syntax;

pub fn draw_script_studio_screen(ui: &mut egui::Ui, state: &mut AppState) {
    let avail = ui.available_size();
    let left_w = 270.0_f32;
    let right_w = (avail.x - left_w - 14.0).max(300.0);

    ui.horizontal_top(|ui| {
        // Lista de scripts y creación por lenguaje
        ui.allocate_ui(egui::vec2(left_w, avail.y), |ui| {
            theme::card_frame().show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Scripts del Mod")
                        .strong()
                        .size(16.0)
                        .color(theme::ACCENT_CORAL),
                );
                ui.add_space(6.0);

                ui.horizontal_wrapped(|ui| {
                    if ui.button("+ Lua").clicked() {
                        add_new_script(state, ScriptLanguage::Lua);
                    }
                    if ui.button("+ TypeScript").clicked() {
                        add_new_script(state, ScriptLanguage::TypeScript);
                    }
                    if ui.button("+ JS").clicked() {
                        add_new_script(state, ScriptLanguage::JavaScript);
                    }
                    if ui.button("📂 Importar").clicked() {
                        state.trigger_import(ImportKind::ScriptFile);
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(6.0);

                for script in &mut state.project.scripts {
                    let is_sel = state.selected_script_id == script.id;
                    egui::Frame::NONE
                        .fill(if is_sel { theme::BG_CARD } else { theme::BG_WELL })
                        .stroke(egui::Stroke::new(
                            1.0,
                            if is_sel { theme::ACCENT_TEAL } else { theme::BORDER_SUBTLE },
                        ))
                        .corner_radius(6.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut script.enabled, "");
                                if ui.selectable_label(is_sel, &script.name).clicked() {
                                    state.selected_script_id = script.id.clone();
                                    state.script_check_status = None;
                                }
                            });
                            ui.label(
                                egui::RichText::new(script.language.label())
                                    .size(10.5)
                                    .color(theme::TEXT_MUTED),
                            );
                        });
                    ui.add_space(4.0);
                }
            });
        });

        // Editor de código y botón de verificación
        ui.allocate_ui(egui::vec2(right_w, avail.y), |ui| {
            theme::card_frame().show(ui, |ui| {
                let sel_id = state.selected_script_id.clone();
                if let Some(script) = state.project.scripts.iter_mut().find(|s| s.id == sel_id) {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Editando: {}", script.name))
                                .strong()
                                .size(15.0),
                        );
                        ui.label(
                            egui::RichText::new(format!("[{}]", script.language.label()))
                                .color(theme::ACCENT_INDIGO),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("✓ Verificar Sintaxis").clicked() {
                                state.script_check_status = Some(validate_script_syntax(script));
                            }
                        });
                    });

                    if let Some(res) = &state.script_check_status {
                        let (txt, col) = match res {
                            Ok(ok) => (ok.as_str(), theme::ACCENT_EMERALD),
                            Err(err) => (err.as_str(), theme::ACCENT_DANGER),
                        };
                        ui.label(egui::RichText::new(txt).size(12.0).color(col));
                    }

                    ui.add_space(6.0);
                    egui::ScrollArea::vertical()
                        .max_height(avail.y - 125.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut script.source_code)
                                    .code_editor()
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(16),
                            );
                        });

                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(
                            "API disponible: onCreate(), onUpdate(dt), onBeatHit(beat), onNoteHit(lane, isPlayer), setCameraZoom(z), shakeCamera(i), setEntityProp(id, prop, val), playAnim(id, anim)",
                        )
                        .size(11.0)
                        .color(theme::TEXT_MUTED),
                    );
                } else {
                    ui.label("Selecciona o crea un script en la lista izquierda.");
                }
            });
        });
    });
}

fn add_new_script(state: &mut AppState, lang: ScriptLanguage) {
    let idx = state.project.scripts.len() + 1;
    let id = format!("script_{}", idx);
    let name = format!("custom_{}.{}", idx, lang.extension());
    let template = match lang {
        ScriptLanguage::Lua => {
            "-- Nuevo script Lua\nfunction onBeatHit(beat)\n    setCameraZoom(1.03)\nend\n"
        }
        ScriptLanguage::TypeScript => {
            "// Nuevo script TypeScript\nfunction onBeatHit(beat: number): void {\n    if (beat % 4 === 0) shakeCamera(4.0);\n}\n"
        }
        ScriptLanguage::JavaScript => {
            "// Nuevo script JavaScript\nfunction onNoteHit(lane, isPlayer) {\n    if (isPlayer) log('Hit lane ' + lane);\n}\n"
        }
    };

    state.project.scripts.push(ScriptAsset {
        id: id.clone(),
        name,
        language: lang,
        source_code: template.to_string(),
        enabled: true,
    });
    state.selected_script_id = id;
    state.script_check_status = None;
}
