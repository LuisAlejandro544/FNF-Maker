//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/simulation.rs
//! Propósito: Bucle interno de simulación del gameplay frame a frame (`step_gameplay_internal`):
//! cálculo de cuenta regresiva BPM, hooks de beats/steps/eventos, evaluación de notas
//! (`Sick`/`Good`/`Bad`/`Miss` y sostenidas `Hold`), retorno a `Idle` y recolección en arena.

use super::script_sync::{
    drain_and_apply_script_commands, sync_script_host_state, update_active_timers,
    update_active_tweens,
};
use crate::session::{
    EngineSession, GameplayRuntimeState, dir_to_anim, dir_to_miss_anim, trigger_role_anim,
};
use pulse_core::chart::ChartNote;
use pulse_core::frame_memory::{ArenaSpan, FrameVisibleNote, NotePlayState};
use pulse_core::sparrow::CharacterAnimKind;
use pulse_core::EntityRole;
use std::sync::atomic::Ordering;

const HIT_WINDOW_SICK_MS: f64 = 55.0;
const HIT_WINDOW_GOOD_MS: f64 = 105.0;
const HIT_WINDOW_BAD_MS: f64 = 155.0;

pub(super) struct StepSimOutput {
    pub song_pos_ms: f64,
    pub active_bpm: f64,
    pub beat_phase: f32,
    pub countdown_stage: i32,
    pub countdown_label: String,
    pub countdown_progress: f32,
    pub player_holding_mask: u8,
    pub opp_holding_mask: u8,
    pub visible_span: ArenaSpan<FrameVisibleNote>,
}

fn process_chart_notes(
    session: &mut GameplayRuntimeState,
    notes: &[ChartNote],
    song_pos_ms: f64,
    dt: f32,
    lane_pressed_mask: i32,
    just_pressed: &mut [bool; 4],
) -> (u8, u8, bool) {
    let mut player_holding_mask: u8 = 0;
    let mut opp_holding_mask: u8 = 0;
    let mut any_recent_miss = false;

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

        match state {
            NotePlayState::Holding => {
                let end_ms = note.time_ms + note.sustain_ms;
                let lane_idx = note.direction as usize;
                if !note.is_player {
                    if song_pos_ms >= end_ms {
                        session.note_window.mark_hit(idx);
                    } else {
                        opp_holding_mask |= 1u8 << (lane_idx as u8);
                        trigger_role_anim(
                            &mut session.bundle.active_entities,
                            EntityRole::Opponent,
                            dir_to_anim(note.direction),
                        );
                    }
                } else {
                    let is_held = (lane_pressed_mask & (1 << lane_idx)) != 0;
                    if song_pos_ms >= end_ms {
                        session.note_window.mark_hit(idx);
                        session.score += 100;
                        session.health = (session.health + 0.02).min(1.0);
                    } else if is_held {
                        player_holding_mask |= 1u8 << (lane_idx as u8);
                        session.score += ((dt * 220.0).round() as i64).max(1);
                        session.health = (session.health + 0.05 * dt).min(1.0);
                        trigger_role_anim(
                            &mut session.bundle.active_entities,
                            EntityRole::Player,
                            dir_to_anim(note.direction),
                        );
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
                            session.health = (session.health - 0.03).max(0.0);
                            session.last_judgement = "Miss".to_string();
                            session.last_rating_code = 4;
                            session.rating_event_id = session.rating_event_id.wrapping_add(1);
                            any_recent_miss = true;
                            trigger_role_anim(
                                &mut session.bundle.active_entities,
                                EntityRole::Player,
                                dir_to_miss_anim(note.direction),
                            );
                            session.runtime.call_on_note_miss(lane_idx as i32, true);
                        }
                    }
                }
            }
            NotePlayState::Pending => {
                if !note.is_player {
                    if song_pos_ms >= note.time_ms {
                        let lane_idx = note.direction as u8;
                        if note.sustain_ms > 45.0 {
                            session.note_window.mark_holding(idx);
                            opp_holding_mask |= 1u8 << lane_idx;
                        } else {
                            session.note_window.mark_hit(idx);
                        }
                        let dir_anim = dir_to_anim(note.direction);
                        trigger_role_anim(
                            &mut session.bundle.active_entities,
                            EntityRole::Opponent,
                            dir_anim,
                        );
                        session
                            .runtime
                            .call_on_note_hit(note.direction as i32, false);
                    }
                } else {
                    let diff = note.time_ms - song_pos_ms;
                    let abs_diff = diff.abs();
                    let lane_idx = note.direction as usize;
                    if just_pressed[lane_idx] && abs_diff <= HIT_WINDOW_BAD_MS {
                        if note.sustain_ms > 45.0 {
                            session.note_window.mark_holding(idx);
                            player_holding_mask |= 1u8 << (lane_idx as u8);
                        } else {
                            session.note_window.mark_hit(idx);
                        }
                        just_pressed[lane_idx] = false;

                        let (pts, rating_code, label, acc_weight) =
                            if abs_diff <= HIT_WINDOW_SICK_MS {
                                session.splash_event_id = session.splash_event_id.wrapping_add(1);
                                session.splash_lane = lane_idx as u8;
                                (350, 1u8, "¡Sick!", 100.0)
                            } else if abs_diff <= HIT_WINDOW_GOOD_MS {
                                (200, 2u8, "¡Good!", 75.0)
                            } else {
                                (100, 3u8, "Bad", 40.0)
                            };
                        session.last_judgement = label.to_string();
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
                        session.health = (session.health + 0.035).min(1.0);

                        trigger_role_anim(
                            &mut session.bundle.active_entities,
                            EntityRole::Player,
                            dir_to_anim(note.direction),
                        );
                        session.runtime.call_on_note_hit(lane_idx as i32, true);
                    } else if song_pos_ms > note.time_ms + HIT_WINDOW_BAD_MS {
                        session.note_window.mark_missed(idx);
                        session.combo = 0;
                        session.misses += 1;
                        session.judged_count += 1;
                        session.accuracy_pct = ((session.accuracy_sum
                            / session.judged_count as f64)
                            as f32)
                            .clamp(0.0, 100.0);
                        session.score -= 50;
                        session.health = (session.health - 0.06).max(0.0);
                        session.last_judgement = "Miss".to_string();
                        session.last_rating_code = 4;
                        session.rating_event_id = session.rating_event_id.wrapping_add(1);
                        any_recent_miss = true;
                        trigger_role_anim(
                            &mut session.bundle.active_entities,
                            EntityRole::Player,
                            dir_to_miss_anim(note.direction),
                        );
                        session.runtime.call_on_note_miss(lane_idx as i32, true);
                    }
                }
            }
            NotePlayState::Hit | NotePlayState::Missed => {}
        }
    }

    (player_holding_mask, opp_holding_mask, any_recent_miss)
}

impl EngineSession {
    /// Ejecuta un tick completo de simulación del gameplay sincronizado con el reloj de `pulse_audio`.
    pub(super) fn step_gameplay_internal(
        &mut self,
        dt_sec: f32,
        lane_pressed_mask: i32,
        lane_just_pressed_mask: i32,
    ) -> Option<StepSimOutput> {
        let session = self.gameplay.as_mut()?;

        session.frame_arena.reset();

        let dt = dt_sec.clamp(0.0, 0.05);
        let raw_mixer_ms = self.audio_engine.mixer.position_ms();
        let in_countdown = raw_mixer_ms < 0.0;
        let song_pos_ms = if in_countdown {
            raw_mixer_ms
        } else {
            raw_mixer_ms - self.project.audio_offset_ms
        };
        let active_bpm = self.project.chart.bpm_at_ms(song_pos_ms.max(0.0)).max(40.0);
        let beat_ms = 60_000.0 / active_bpm;
        let beat_sec = (beat_ms / 1000.0) as f32;
        let step_ms = beat_ms * 0.25;
        let beat_phase = (song_pos_ms.rem_euclid(beat_ms) / beat_ms) as f32;
        let current_beat = (song_pos_ms / beat_ms).floor() as i32;
        let current_step = (song_pos_ms / step_ms).floor() as i32;
        let must_hit_now = self.project.chart.must_hit_at_ms(song_pos_ms.max(0.0));

        let (countdown_stage, countdown_label, countdown_progress) = if in_countdown {
            let cd_beat_ms = 60_000.0 / self.project.chart.bpm.max(40.0);
            let total_cd_ms = cd_beat_ms * 4.0;
            let elapsed_cd = (total_cd_ms + raw_mixer_ms).clamp(0.0, total_cd_ms - 0.001);
            let stage = ((elapsed_cd / cd_beat_ms).floor() as i32).clamp(0, 3);
            let progress = ((elapsed_cd % cd_beat_ms) / cd_beat_ms) as f32;
            let label = match stage {
                0 => "3",
                1 => "2",
                2 => "1",
                _ => "¡YA!",
            };
            (stage, label.to_string(), progress.clamp(0.0, 1.0))
        } else {
            (-1, String::new(), 0.0)
        };

        let mut hud_changed = false;
        if countdown_stage != session.last_countdown_stage {
            session.last_countdown_stage = countdown_stage;
            hud_changed = true;
            if countdown_stage >= 0 && !session.paused && !session.finished {
                session.runtime.call_on_countdown_tick(countdown_stage);
            }
        }
        let prev_judgement = session.last_judgement.clone();
        let prev_paused = session.paused;
        let prev_finished = session.finished;
        let prev_log_count = session.last_logs.len();
        let prev_last_log = session.last_logs.last().cloned();
        let prev_err_count = session.script_errors.len();

        let mut just_pressed = [
            (lane_just_pressed_mask & 1) != 0,
            (lane_just_pressed_mask & 2) != 0,
            (lane_just_pressed_mask & 4) != 0,
            (lane_just_pressed_mask & 8) != 0,
        ];
        let mut player_holding_mask: u8 = 0;
        let mut opp_holding_mask: u8 = 0;

        if !session.paused && !session.finished {
            update_active_timers(session, dt);
            update_active_tweens(session, &mut self.project.stage, dt);

            sync_script_host_state(
                &session.runtime.state,
                &self.project.stage,
                &session.bundle.active_entities,
                &session.strum_overrides,
                song_pos_ms,
                active_bpm,
                must_hit_now,
                session.health,
                session.score,
                session.combo,
                session.misses,
                session.hits,
                session.accuracy_pct,
                session.active_scroll_speed,
                session.active_downscroll,
            );

            while session.next_event_idx < self.project.chart.events.len() {
                let ev = &self.project.chart.events[session.next_event_idx];
                if song_pos_ms < ev.time_ms {
                    break;
                }
                session
                    .runtime
                    .call_on_event(&ev.name, &ev.value1, &ev.value2);
                session.next_event_idx += 1;
            }

            if current_step > session.last_step {
                session.last_step = current_step;
                session.runtime.call_on_step_hit(current_step);
            }

            if current_beat > session.last_beat {
                session.last_beat = current_beat;
                session.runtime.call_on_beat_hit(current_beat);

                for ent in &mut session.bundle.active_entities {
                    if ent.active_anim == CharacterAnimKind::Idle
                        && matches!(ent.role, EntityRole::Player | EntityRole::Opponent)
                    {
                        let idle_dur = ent
                            .atlas
                            .as_ref()
                            .map(|a| a.anim_duration_sec(CharacterAnimKind::Idle))
                            .unwrap_or(0.0);
                        let beat_every = if idle_dur > beat_sec * 1.35 { 2 } else { 1 };
                        if current_beat.rem_euclid(beat_every) == 0 {
                            ent.anim_timer_sec = 0.0;
                        }
                    }
                }
            }
            session.runtime.call_on_update(dt);

            drain_and_apply_script_commands(
                session,
                &mut self.project.stage,
                &self.audio_engine,
            );

            let target_base_zoom = self.project.stage.camera_base_zoom.clamp(0.35, 2.5);
            self.project.stage.camera_zoom +=
                (target_base_zoom - self.project.stage.camera_zoom) * (dt * 6.0);
            self.project.stage.camera_shake *= (1.0 - dt * 10.0).max(0.0);

            let (p_hold, o_hold, any_recent_miss) = process_chart_notes(
                session,
                &self.project.chart.notes,
                song_pos_ms,
                dt,
                lane_pressed_mask,
                &mut just_pressed,
            );
            player_holding_mask = p_hold;
            opp_holding_mask = o_hold;

            session
                .note_window
                .advance_and_discard(&self.project.chart.notes, song_pos_ms, 240.0);

            self.audio_engine
                .mixer
                .player_missed_recently
                .store(any_recent_miss, Ordering::Relaxed);

            let sing_hold_sec = (beat_sec * 0.90).clamp(0.28, 0.58);
            for ent in &mut session.bundle.active_entities {
                ent.anim_timer_sec += dt;
                if ent.active_anim != CharacterAnimKind::Idle && ent.anim_timer_sec > sing_hold_sec {
                    ent.active_anim = CharacterAnimKind::Idle;
                    ent.anim_timer_sec = 0.0;
                }
            }

            if song_pos_ms > self.project.chart.stats.duration_ms + 1200.0 {
                session.finished = true;
                self.audio_engine.mixer.pause();
            }
        }

        if prev_judgement != session.last_judgement
            || prev_paused != session.paused
            || prev_finished != session.finished
            || prev_log_count != session.last_logs.len()
            || prev_last_log.as_deref() != session.last_logs.last().map(|s| s.as_str())
            || prev_err_count != session.script_errors.len()
        {
            hud_changed = true;
        }
        if hud_changed {
            session.hud_revision = session.hud_revision.wrapping_add(1);
        }

        let visible_span = session.note_window.collect_visible_into_arena(
            &self.project.chart.notes,
            song_pos_ms,
            240.0,
            1800.0,
            &mut session.frame_arena,
            &mut session.visible_note_scratch,
        );

        Some(StepSimOutput {
            song_pos_ms,
            active_bpm,
            beat_phase,
            countdown_stage,
            countdown_label,
            countdown_progress,
            player_holding_mask,
            opp_holding_mask,
            visible_span,
        })
    }
}
