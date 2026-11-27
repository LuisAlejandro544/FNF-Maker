//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/simulation/note_judge.rs
//! Propósito: Evaluación de notas y colas sostenidas (`Hold`) sobre la ventana deslizante
//! del chart: ventanas de juicio `Sick` / `Good` / `Bad` / `Miss`, notas especiales (`Hurt` X,
//! `Instakill`, `Dodge`, `NoAnim`), modo `Botplay` / `Práctica`, puntaje, salud, precisión
//! y disparo de animaciones y hooks de scripts (`onNoteHit` / `onNoteMiss`).

use crate::session::{GameplayRuntimeState, dir_to_anim, dir_to_miss_anim, trigger_character_anim};
use pulse_core::EntityRole;
use pulse_core::chart::{ChartNote, NoteSpecialKind};
use pulse_core::frame_memory::NotePlayState;

const HIT_WINDOW_SICK_MS: f64 = 55.0;
const HIT_WINDOW_GOOD_MS: f64 = 105.0;
const HIT_WINDOW_BAD_MS: f64 = 155.0;

pub(super) fn process_chart_notes(
    session: &mut GameplayRuntimeState,
    notes: &[ChartNote],
    song_pos_ms: f64,
    dt: f32,
    lane_pressed_mask: i32,
    just_pressed: &mut [bool; 4],
) -> (u8, u8, bool, bool) {
    let mut player_holding_mask: u8 = 0;
    let mut opp_holding_mask: u8 = 0;
    let mut any_recent_miss = false;
    let mut triggered_instakill = false;

    let min_hp = if session.practice_mode { 0.02 } else { 0.0 };
    let note_len = notes.len();
    let start_idx = session.note_window.active_start_idx.min(note_len);

    for idx in start_idx..note_len {
        let state = session.note_window.state(idx);
        if matches!(state, NotePlayState::Hit | NotePlayState::Missed) {
            continue;
        }
        let note = &notes[idx];
        if note.time_ms > song_pos_ms + HIT_WINDOW_BAD_MS + 10.0 {
            break;
        }

        let special_kind = NoteSpecialKind::from_str_loose(&note.note_type);

        match state {
            NotePlayState::Holding => {
                let end_ms = note.time_ms + note.sustain_ms;
                let lane_idx = note.direction as usize;
                if !note.is_player {
                    if song_pos_ms >= end_ms {
                        session.note_window.mark_hit(idx);
                    } else {
                        opp_holding_mask |= 1u8 << (lane_idx as u8);
                        if special_kind != NoteSpecialKind::NoAnim {
                            trigger_character_anim(
                                &mut session.bundle.active_entities,
                                EntityRole::Opponent,
                                &session.active_opponent_id,
                                dir_to_anim(note.direction),
                            );
                        }
                    }
                } else {
                    let is_held = session.botplay || (lane_pressed_mask & (1 << lane_idx)) != 0;
                    if song_pos_ms >= end_ms {
                        session.note_window.mark_hit(idx);
                        session.score += 100;
                        session.health = (session.health + 0.02).min(1.0);
                    } else if is_held {
                        player_holding_mask |= 1u8 << (lane_idx as u8);
                        session.score += ((dt * 220.0).round() as i64).max(1);
                        session.health = (session.health + 0.05 * dt).min(1.0);
                        if special_kind != NoteSpecialKind::NoAnim {
                            trigger_character_anim(
                                &mut session.bundle.active_entities,
                                EntityRole::Player,
                                &session.active_player_id,
                                dir_to_anim(note.direction),
                            );
                        }
                    } else if song_pos_ms > note.time_ms + 130.0 {
                        if song_pos_ms >= end_ms - 140.0 {
                            session.note_window.mark_hit(idx);
                        } else {
                            session.note_window.mark_missed(idx);
                            session.combo = 0;
                            session.misses += 1;
                            session.judged_count += 1;
                            session.accuracy_pct = ((session.accuracy_sum
                                / session.judged_count as f64)
                                as f32)
                                .clamp(0.0, 100.0);
                            session.score -= 25;
                            session.health = (session.health - 0.03).max(min_hp);
                            session.last_judgement = "Miss".to_string();
                            session.last_rating_code = 4;
                            session.rating_event_id = session.rating_event_id.wrapping_add(1);
                            any_recent_miss = true;
                            if special_kind != NoteSpecialKind::NoAnim {
                                trigger_character_anim(
                                    &mut session.bundle.active_entities,
                                    EntityRole::Player,
                                    &session.active_player_id,
                                    dir_to_miss_anim(note.direction),
                                );
                            }
                            session
                                .runtime
                                .call_on_note_miss(lane_idx as i32, true, &note.note_type);
                        }
                    }
                }
            }
            NotePlayState::Pending => {
                if !note.is_player {
                    if special_kind.is_hazard_x() {
                        if song_pos_ms > note.time_ms + 80.0 {
                            session.note_window.mark_hit(idx);
                        }
                    } else if song_pos_ms >= note.time_ms {
                        let lane_idx = note.direction as u8;
                        if note.sustain_ms > 45.0 {
                            session.note_window.mark_holding(idx);
                            opp_holding_mask |= 1u8 << lane_idx;
                        } else {
                            session.note_window.mark_hit(idx);
                        }
                        if special_kind != NoteSpecialKind::NoAnim {
                            trigger_character_anim(
                                &mut session.bundle.active_entities,
                                EntityRole::Opponent,
                                &session.active_opponent_id,
                                dir_to_anim(note.direction),
                            );
                        }
                        session
                            .runtime
                            .call_on_note_hit(note.direction as i32, false, &note.note_type);
                    }
                } else {
                    let diff = note.time_ms - song_pos_ms;
                    let abs_diff = diff.abs();
                    let lane_idx = note.direction as usize;
                    let bot_should_hit =
                        session.botplay && !special_kind.is_hazard_x() && song_pos_ms >= note.time_ms;
                    let manual_hit = !special_kind.is_hazard_x()
                        && just_pressed[lane_idx]
                        && abs_diff <= HIT_WINDOW_BAD_MS;
                    let hazard_hit = !session.botplay
                        && special_kind.is_hazard_x()
                        && just_pressed[lane_idx]
                        && abs_diff <= HIT_WINDOW_GOOD_MS;

                    if hazard_hit {
                        session.note_window.mark_hit(idx);
                        just_pressed[lane_idx] = false;
                        session.combo = 0;
                        session.misses += 1;
                        session.judged_count += 1;
                        session.accuracy_pct = ((session.accuracy_sum
                            / session.judged_count as f64)
                            as f32)
                            .clamp(0.0, 100.0);
                        session.last_rating_code = 4;
                        session.rating_event_id = session.rating_event_id.wrapping_add(1);
                        any_recent_miss = true;

                        if special_kind == NoteSpecialKind::Instakill {
                            if session.practice_mode {
                                session.score -= 150;
                                session.health = (session.health - 0.30).max(min_hp);
                                session.last_judgement = "¡Nota Mortal X! (Práctica)".to_string();
                            } else {
                                session.health = 0.0;
                                session.score -= 250;
                                session.last_judgement = "¡Nota Mortal X!".to_string();
                                triggered_instakill = true;
                            }
                        } else {
                            session.score -= 100;
                            session.health = (session.health - 0.22).max(min_hp);
                            session.last_judgement = "¡Nota X! -Vida".to_string();
                        }

                        trigger_character_anim(
                            &mut session.bundle.active_entities,
                            EntityRole::Player,
                            &session.active_player_id,
                            dir_to_miss_anim(note.direction),
                        );
                        session
                            .runtime
                            .call_on_note_miss(lane_idx as i32, true, &note.note_type);
                    } else if bot_should_hit || manual_hit {
                        if note.sustain_ms > 45.0 {
                            session.note_window.mark_holding(idx);
                            player_holding_mask |= 1u8 << (lane_idx as u8);
                        } else {
                            session.note_window.mark_hit(idx);
                        }
                        if !bot_should_hit {
                            just_pressed[lane_idx] = false;
                        }

                        let effective_diff = if bot_should_hit { 0.0 } else { abs_diff };
                        let (mut pts, rating_code, label, acc_weight) =
                            if effective_diff <= HIT_WINDOW_SICK_MS {
                                session.splash_event_id = session.splash_event_id.wrapping_add(1);
                                session.splash_lane = lane_idx as u8;
                                (350, 1u8, "¡Sick!", 100.0)
                            } else if effective_diff <= HIT_WINDOW_GOOD_MS {
                                (200, 2u8, "¡Good!", 75.0)
                            } else {
                                (100, 3u8, "Bad", 40.0)
                            };
                        let mut hp_gain = 0.035_f32;
                        if special_kind == NoteSpecialKind::Dodge {
                            pts += 150;
                            hp_gain += 0.06;
                            session.last_judgement = format!("¡Esquive! {}", label);
                        } else {
                            session.last_judgement = label.to_string();
                        }
                        session.last_rating_code = rating_code;
                        session.rating_event_id = session.rating_event_id.wrapping_add(1);
                        session.judged_count += 1;
                        session.accuracy_sum += acc_weight;
                        session.accuracy_pct = ((session.accuracy_sum
                            / session.judged_count as f64)
                            as f32)
                            .clamp(0.0, 100.0);

                        session.score += pts;
                        session.combo += 1;
                        session.max_combo = session.max_combo.max(session.combo);
                        session.hits += 1;
                        session.health = (session.health + hp_gain).min(1.0);

                        if special_kind != NoteSpecialKind::NoAnim {
                            trigger_character_anim(
                                &mut session.bundle.active_entities,
                                EntityRole::Player,
                                &session.active_player_id,
                                dir_to_anim(note.direction),
                            );
                        }
                        session
                            .runtime
                            .call_on_note_hit(lane_idx as i32, true, &note.note_type);
                    } else if song_pos_ms > note.time_ms + HIT_WINDOW_BAD_MS {
                        if special_kind.is_hazard_x() {
                            // Las notas con X se esquivan dejándolas pasar sin presionarlas.
                            session.note_window.mark_hit(idx);
                        } else {
                            session.note_window.mark_missed(idx);
                            session.combo = 0;
                            session.misses += 1;
                            session.judged_count += 1;
                            session.accuracy_pct = ((session.accuracy_sum
                                / session.judged_count as f64)
                                as f32)
                                .clamp(0.0, 100.0);
                            session.score -= 50;
                            let penalty = if special_kind == NoteSpecialKind::Dodge {
                                0.14
                            } else {
                                0.06
                            };
                            session.health = (session.health - penalty).max(min_hp);
                            session.last_judgement = if special_kind == NoteSpecialKind::Dodge {
                                "¡Sin esquivar!".to_string()
                            } else {
                                "Miss".to_string()
                            };
                            session.last_rating_code = 4;
                            session.rating_event_id = session.rating_event_id.wrapping_add(1);
                            any_recent_miss = true;
                            if special_kind != NoteSpecialKind::NoAnim {
                                trigger_character_anim(
                                    &mut session.bundle.active_entities,
                                    EntityRole::Player,
                                    &session.active_player_id,
                                    dir_to_miss_anim(note.direction),
                                );
                            }
                            session.runtime.call_on_note_miss(
                                lane_idx as i32,
                                true,
                                &note.note_type,
                            );
                        }
                    }
                }
            }
            NotePlayState::Hit | NotePlayState::Missed => {}
        }
    }

    (
        player_holding_mask,
        opp_holding_mask,
        any_recent_miss,
        triggered_instakill,
    )
}
