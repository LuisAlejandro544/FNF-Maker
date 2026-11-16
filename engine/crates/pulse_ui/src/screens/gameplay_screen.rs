//! Archivo: engine/crates/pulse_ui/src/screens/gameplay_screen.rs
//! Propósito: Motor de juego horizontal jugable en tiempo real.
//! - Sincroniza las notas y el ritmo al reloj maestro por muestras de Google Oboe.
//! - Dibuja el escenario ordenado por capas Z (Fondo -> Decoración -> Enemigo -> Player)
//!   aplicando el recorte de frame Sparrow XML según el canto de cada personaje.
//! - Ejecuta callbacks Lua / TypeScript / JavaScript (`onUpdate`, `onBeatHit`, `onNoteHit`, `onNoteMiss`).
//! - Provee controles táctiles en 4 carriles horizontales para celular + soporte teclado (`D/F/J/K` y flechas).

use crate::state::{ActiveScreen, AppState};
use crate::theme;
use eframe::egui;
use pulse_core::{CharacterAnimKind, EntityRole, NoteDirection};
use pulse_scripting::ScriptCommand;

const HIT_WINDOW_GOOD_MS: f64 = 155.0;
const HIT_WINDOW_PERFECT_MS: f64 = 55.0;

pub fn draw_gameplay_screen(ui: &mut egui::Ui, state: &mut AppState) {
    let rect = ui.available_rect_before_wrap();
    let dt = ui.input(|i| i.stable_dt.min(0.05));

    let Some(mut session) = state.gameplay.take() else {
        state.screen = ActiveScreen::Hub;
        return;
    };

    let song_pos_ms = state.audio_engine.mixer.position_ms() - state.project.audio_offset_ms;
    let bpm = state.project.chart.bpm.max(40.0);
    let beat_ms = 60_000.0 / bpm;
    let current_beat = (song_pos_ms / beat_ms).floor() as i32;

    if !session.paused && !session.finished {
        // Actualizar reloj para scripts y disparar onBeatHit / onUpdate
        *session.runtime.state.song_pos_ms.lock() = song_pos_ms;
        if current_beat > session.last_beat {
            session.last_beat = current_beat;
            session.runtime.call_on_beat_hit(current_beat);
        }
        session.runtime.call_on_update(dt);

        // Aplicar comandos emitidos por scripts Lua/TS/JS
        for cmd in session.runtime.drain_commands() {
            match cmd {
                ScriptCommand::SetCameraZoom(z) => {
                    state.project.stage.camera_zoom = z.clamp(0.5, 2.5);
                }
                ScriptCommand::ShakeCamera { intensity } => {
                    state.project.stage.camera_shake = intensity.clamp(0.0, 25.0);
                }
                ScriptCommand::SetEntityProp {
                    entity_id,
                    prop,
                    value,
                } => {
                    if let Some(ent) = session
                        .bundle
                        .active_entities
                        .iter_mut()
                        .find(|e| e.id == entity_id)
                    {
                        match prop.as_str() {
                            "opacity" => ent.set_opacity(value),
                            "scale" => ent.set_scale(value),
                            "x" => ent.x = value,
                            "y" => ent.y = value,
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        // Suavizar zoom y sacudida de cámara
        state.project.stage.camera_zoom += (1.0 - state.project.stage.camera_zoom) * (dt * 6.0);
        state.project.stage.camera_shake *= (1.0 - dt * 10.0).max(0.0);

        // Leer teclado (D/F/J/K y flechas)
        let mut just_pressed = [false; 4];
        ui.input(|i| {
            let keys = [
                [egui::Key::D, egui::Key::ArrowLeft],
                [egui::Key::F, egui::Key::ArrowDown],
                [egui::Key::J, egui::Key::ArrowUp],
                [egui::Key::K, egui::Key::ArrowRight],
            ];
            for lane in 0..4 {
                let down = i.key_down(keys[lane][0]) || i.key_down(keys[lane][1]);
                if i.key_pressed(keys[lane][0]) || i.key_pressed(keys[lane][1]) {
                    just_pressed[lane] = true;
                }
                session.lane_pressed[lane] = down;
            }
        });

        // Procesar carriles táctiles inferiores si el usuario toca en celular
        let lane_w = rect.width() / 4.0;
        let touch_zone_top = rect.bottom() - 88.0;
        let pointer_down = ui.input(|i| i.pointer.any_down());
        let pointer_just_pressed = ui.input(|i| i.pointer.any_pressed());
        if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
            if pos.y >= touch_zone_top {
                let lane = ((pos.x - rect.left()) / lane_w).floor().clamp(0.0, 3.0) as usize;
                if pointer_down {
                    session.lane_pressed[lane] = true;
                }
                if pointer_just_pressed {
                    just_pressed[lane] = true;
                }
            }
        }

        // Evaluar notas del Rival (auto-canto) y aciertos del Player
        let mut any_recent_miss = false;
        for (idx, note) in state.project.chart.notes.iter().enumerate() {
            if session.notes_hit_state[idx] || session.notes_miss_state[idx] {
                continue;
            }
            if !note.is_player {
                if song_pos_ms >= note.time_ms {
                    session.notes_hit_state[idx] = true;
                    let dir_anim = dir_to_anim(note.direction);
                    trigger_role_anim(&mut session.bundle.active_entities, EntityRole::Opponent, dir_anim);
                    session.runtime.call_on_note_hit(note.direction as i32, false);
                }
            } else {
                let diff = note.time_ms - song_pos_ms;
                let lane_idx = note.direction as usize;
                if just_pressed[lane_idx] && diff.abs() <= HIT_WINDOW_GOOD_MS {
                    session.notes_hit_state[idx] = true;
                    just_pressed[lane_idx] = false;
                    session.lane_flash_timer[lane_idx] = 0.16;

                    let pts = if diff.abs() <= HIT_WINDOW_PERFECT_MS {
                        session.last_judgement = "¡Perfecto!".to_string();
                        350
                    } else {
                        session.last_judgement = "¡Bien!".to_string();
                        200
                    };
                    session.score += pts;
                    session.combo += 1;
                    session.max_combo = session.max_combo.max(session.combo);
                    session.hits += 1;
                    session.health = (session.health + 0.035).min(1.0);

                    trigger_role_anim(
                        &mut session.bundle.active_entities,
                        EntityRole::Player,
                        dir_to_anim(note.direction),
                    );
                    session.runtime.call_on_note_hit(lane_idx as i32, true);
                } else if song_pos_ms > note.time_ms + HIT_WINDOW_GOOD_MS {
                    session.notes_miss_state[idx] = true;
                    session.combo = 0;
                    session.misses += 1;
                    session.score -= 50;
                    session.health = (session.health - 0.06).max(0.0);
                    session.last_judgement = "Fallo".to_string();
                    any_recent_miss = true;
                    session.runtime.call_on_note_miss(lane_idx as i32, true);
                }
            }
        }

        state
            .audio_engine
            .mixer
            .player_missed_recently
            .store(any_recent_miss, std::sync::atomic::Ordering::Relaxed);

        // Avanzar timers de animación de entidades
        for ent in &mut session.bundle.active_entities {
            ent.anim_timer_sec += dt;
            if ent.active_anim != CharacterAnimKind::Idle && ent.anim_timer_sec > 0.48 {
                ent.active_anim = CharacterAnimKind::Idle;
                ent.anim_timer_sec = 0.0;
            }
        }
        for t in &mut session.lane_flash_timer {
            *t = (*t - dt).max(0.0);
        }

        if song_pos_ms > state.project.chart.stats.duration_ms + 1200.0 {
            session.finished = true;
            state.audio_engine.mixer.pause();
        }
    }

    // --- RENDERIZADO DEL ESCENARIO Y NOTAS ---
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme::BG_WELL);

    let cam_zoom = state.project.stage.camera_zoom;
    let sx = (rect.width() / state.project.stage.virtual_width) * cam_zoom;
    let sy = (rect.height() / state.project.stage.virtual_height) * cam_zoom;

    for ent in &session.bundle.active_entities {
        if !ent.visible || ent.opacity <= 0.01 {
            continue;
        }
        let cx = rect.center().x + (ent.x - state.project.stage.camera_x) * sx;
        let cy = rect.center().y + (ent.y - state.project.stage.camera_y) * sy;
        let ew = ent.width * ent.scale * sx;
        let eh = ent.height * ent.scale * sy;
        let box_rect = egui::Rect::from_center_size(egui::pos2(cx, cy), egui::vec2(ew, eh));
        let alpha = (ent.opacity * 255.0).round() as u8;
        let tint = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);

        if let Some(tex) = state.entity_textures.get(&ent.id) {
            let tex_sz = tex.size_vec2();
            let mut uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
            if let Some(atlas) = &ent.atlas {
                if let Some(frame) = atlas.frame_at(ent.active_anim, ent.anim_timer_sec) {
                    uv = egui::Rect::from_min_max(
                        egui::pos2((frame.x as f32) / tex_sz.x, (frame.y as f32) / tex_sz.y),
                        egui::pos2(
                            ((frame.x + frame.width) as f32) / tex_sz.x,
                            ((frame.y + frame.height) as f32) / tex_sz.y,
                        ),
                    );
                }
            }
            if ent.flip_x {
                std::mem::swap(&mut uv.min.x, &mut uv.max.x);
            }
            painter.image(tex.id(), box_rect, uv, tint);
        } else {
            let base = theme::role_color(ent.role);
            let col = egui::Color32::from_rgba_unmultiplied(base.r(), base.g(), base.b(), alpha / 3);
            painter.rect_filled(box_rect, 10.0, col);
        }
    }

    // Receptores y notas SVG (Izquierda = Enemigo, Derecha = Player)
    let downscroll = state.project.downscroll;
    let receptor_y = if downscroll {
        rect.bottom() - 125.0
    } else {
        rect.top() + 86.0
    };
    let note_size = 54.0_f32;
    let lane_gap = 64.0_f32;
    let opp_start_x = rect.left() + rect.width() * 0.14;
    let plr_start_x = rect.left() + rect.width() * 0.58;

    for lane in 0..4 {
        let dir = NoteDirection::from_lane(lane as u8);
        for (base_x, is_plr) in [(opp_start_x, false), (plr_start_x, true)] {
            let x = base_x + (lane as f32) * lane_gap;
            let r_rect = egui::Rect::from_center_size(
                egui::pos2(x, receptor_y),
                egui::vec2(note_size, note_size),
            );
            let flash = if is_plr && session.lane_pressed[lane] { 230 } else { 95 };
            let r_tint = egui::Color32::from_rgba_unmultiplied(220, 232, 248, flash);
            if let Some(tex) = state.arrow_textures.get(&dir) {
                painter.image(
                    tex.id(),
                    r_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    r_tint,
                );
            }
        }
    }

    // Dibujar notas activas dentro de la ventana visual
    let scroll_px_per_ms = 0.45 * (state.project.chart.speed as f32);
    for (idx, note) in state.project.chart.notes.iter().enumerate() {
        if session.notes_hit_state[idx] {
            continue;
        }
        let delta_ms = (note.time_ms - song_pos_ms) as f32;
        if delta_ms < -240.0 || delta_ms > 1800.0 {
            continue;
        }
        let lane = note.direction as usize;
        let base_x = if note.is_player { plr_start_x } else { opp_start_x };
        let x = base_x + (lane as f32) * lane_gap;
        let offset_y = delta_ms * scroll_px_per_ms;
        let y = if downscroll {
            receptor_y - offset_y
        } else {
            receptor_y + offset_y
        };

        let n_rect =
            egui::Rect::from_center_size(egui::pos2(x, y), egui::vec2(note_size, note_size));
        if let Some(tex) = state.arrow_textures.get(&note.direction) {
            painter.image(
                tex.id(),
                n_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
    }

    // Botones táctiles horizontales en la parte inferior para jugar en teléfono
    let touch_top = rect.bottom() - 78.0;
    let lane_w = rect.width() / 4.0;
    for (idx, dir) in NoteDirection::ALL.iter().enumerate() {
        let p_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + (idx as f32) * lane_w + 4.0, touch_top),
            egui::pos2(rect.left() + ((idx + 1) as f32) * lane_w - 4.0, rect.bottom() - 6.0),
        );
        let active = session.lane_pressed[idx];
        painter.rect_filled(
            p_rect,
            8.0,
            if active {
                egui::Color32::from_rgba_unmultiplied(34, 211, 238, 70)
            } else {
                egui::Color32::from_rgba_unmultiplied(23, 29, 41, 165)
            },
        );
        painter.rect_stroke(
            p_rect,
            8.0,
            egui::Stroke::new(1.0, theme::BORDER_SUBTLE),
            egui::StrokeKind::Inside,
        );
        painter.text(
            p_rect.center(),
            egui::Align2::CENTER_CENTER,
            dir.label_es(),
            egui::FontId::proportional(13.0),
            theme::TEXT_PRIMARY,
        );
    }

    // HUD Superior: Barra de vida, Puntuación, Combo y botón Pausa
    egui::Area::new(egui::Id::new("gameplay_hud"))
        .anchor(egui::Align2::CENTER_TOP, [0.0, 8.0])
        .show(ui.ctx(), |ui| {
            egui::Frame::NONE
                .fill(egui::Color32::from_rgba_unmultiplied(16, 20, 29, 210))
                .corner_radius(8.0)
                .inner_margin(egui::Margin::symmetric(14, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("Score: {}", session.score))
                                .strong()
                                .color(theme::TEXT_PRIMARY),
                        );
                        ui.add_space(14.0);
                        ui.label(
                            egui::RichText::new(format!("Combo: {}x", session.combo))
                                .strong()
                                .color(theme::ACCENT_TEAL),
                        );
                        ui.add_space(14.0);
                        ui.label(
                            egui::RichText::new(format!("Fallos: {}", session.misses))
                                .color(theme::ACCENT_CORAL),
                        );
                        ui.add_space(14.0);
                        ui.label(
                            egui::RichText::new(&session.last_judgement)
                                .strong()
                                .color(theme::ACCENT_EMERALD),
                        );
                        ui.add_space(18.0);
                        if ui.button("⏸ Pausa").clicked() {
                            session.paused = true;
                            state.audio_engine.mixer.pause();
                        }
                    });
                });
        });

    // Popup de Pausa o Resultados al terminar canción
    let mut exit_to_hub = false;
    if session.paused || session.finished {
        let title = if session.finished {
            "¡Canción Completada!"
        } else {
            "Juego en Pausa"
        };
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label(format!("Puntuación final: {}", session.score));
                ui.label(format!("Combo máximo: {}x", session.max_combo));
                ui.label(format!("Aciertos: {} | Fallos: {}", session.hits, session.misses));
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if !session.finished && ui.button("▶ Reanudar").clicked() {
                        session.paused = false;
                        state.audio_engine.mixer.play();
                    }
                    if ui.button("Volver al Editor").clicked() {
                        state.audio_engine.mixer.pause();
                        exit_to_hub = true;
                    }
                });
            });
    }

    if exit_to_hub {
        state.screen = ActiveScreen::Hub;
    } else {
        state.gameplay = Some(session);
    }
}

fn dir_to_anim(dir: NoteDirection) -> CharacterAnimKind {
    match dir {
        NoteDirection::Left => CharacterAnimKind::Left,
        NoteDirection::Down => CharacterAnimKind::Down,
        NoteDirection::Up => CharacterAnimKind::Up,
        NoteDirection::Right => CharacterAnimKind::Right,
    }
}

fn trigger_role_anim(
    entities: &mut [pulse_core::StageEntity],
    role: EntityRole,
    anim: CharacterAnimKind,
) {
    for ent in entities.iter_mut() {
        if ent.role == role {
            ent.active_anim = anim;
            ent.anim_timer_sec = 0.0;
        }
    }
}
