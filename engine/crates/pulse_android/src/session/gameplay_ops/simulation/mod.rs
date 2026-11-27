//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/simulation/mod.rs
//! Propósito: Coordinador modular del bucle interno de simulación frame a frame (`step_gameplay_internal`):
//! - `timeline_ram`: Precarga anticipada (4 s) y retiro seguro en RAM según línea temporal.
//! - `camera_runtime`: Multicámara por sección/beat, eventos de cámara del chart y seguimiento suave.
//! - `note_judge`: Evaluación de notas (`Sick`/`Good`/`Bad`/`Miss`), sostenidas y notas especiales X.

mod camera_runtime;
mod note_judge;
mod timeline_ram;

use super::enter_game_over;
use super::script_sync::{
    drain_and_apply_script_commands, sync_script_host_state, update_active_timers,
    update_active_tweens,
};
use crate::session::EngineSession;
use camera_runtime::{
    apply_chart_camera_action, update_native_camera_follow, update_section_and_beat_camera,
};
use note_judge::process_chart_notes;
use pulse_core::frame_memory::{ArenaSpan, FrameVisibleNote};
use pulse_core::sparrow::CharacterAnimKind;
use pulse_core::{EntityRole, parse_chart_camera_event};
use std::sync::atomic::Ordering;
use timeline_ram::advance_timeline_preload_and_retire;

pub(in crate::session::gameplay_ops) struct StepSimOutput {
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

impl EngineSession {
    /// Ejecuta un tick completo de simulación del gameplay sincronizado con el reloj de `pulse_audio`.
    pub(in crate::session::gameplay_ops) fn step_gameplay_internal(
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
        let (sec_idx, must_hit_now, gf_section_now, section_cam_slot) =
            self.project.chart.camera_state_at_ms(song_pos_ms.max(0.0));

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
        let prev_hud_visible = session.hud_visible;
        let prev_hud_opacity = session.hud_opacity;
        let prev_arrows_visible = session.arrows_visible;
        let prev_arrows_opacity = session.arrows_opacity;
        let prev_blackout_alpha = session.screen_blackout_alpha;

        let mut just_pressed = [
            (lane_just_pressed_mask & 1) != 0,
            (lane_just_pressed_mask & 2) != 0,
            (lane_just_pressed_mask & 4) != 0,
            (lane_just_pressed_mask & 8) != 0,
        ];
        let mut player_holding_mask: u8 = 0;
        let mut opp_holding_mask: u8 = 0;
        let mut should_restart_after_confirm = false;

        if session.game_over {
            session.game_over_timer_sec += dt;
            for ent in &mut session.bundle.active_entities {
                let is_player_ent = if !session.active_player_id.is_empty() {
                    ent.id == session.active_player_id
                } else {
                    ent.role == EntityRole::Player && ent.visible
                };
                if is_player_ent {
                    ent.anim_timer_sec += dt;
                    if session.game_over_phase == 1 {
                        let start_dur = ent
                            .atlas
                            .as_ref()
                            .map(|a| a.anim_duration_sec(CharacterAnimKind::DeathStart))
                            .filter(|d| *d > 0.05)
                            .unwrap_or(0.85);
                        if ent.anim_timer_sec >= start_dur {
                            session.game_over_phase = 2;
                            ent.active_anim = CharacterAnimKind::DeathLoop;
                            ent.anim_timer_sec = 0.0;
                            hud_changed = true;
                        }
                    }
                }
            }
            if session.game_over_phase == 3 && session.game_over_timer_sec >= 0.72 {
                should_restart_after_confirm = true;
            }
        } else if !session.paused && !session.finished {
            if session.flash_alpha > 0.0 {
                session.flash_alpha =
                    (session.flash_alpha - session.flash_decay_per_sec * dt).max(0.0);
            }

            update_active_timers(session, dt);
            update_active_tweens(session, &mut self.project.stage, dt);

            advance_timeline_preload_and_retire(session, &self.project.chart.events, song_pos_ms);

            if !in_countdown {
                update_section_and_beat_camera(
                    session,
                    &self.project.stage,
                    sec_idx,
                    must_hit_now,
                    gf_section_now,
                    section_cam_slot,
                    current_beat,
                );
            }

            sync_script_host_state(
                &session.runtime.state,
                &self.project.stage,
                &session.bundle.active_entities,
                &session.dynamic_items,
                &session.active_player_id,
                &session.active_opponent_id,
                &session.strum_overrides,
                song_pos_ms,
                active_bpm,
                must_hit_now,
                gf_section_now,
                session.active_camera_slot,
                session.health,
                session.score,
                session.combo,
                session.misses,
                session.hits,
                session.accuracy_pct,
                session.active_scroll_speed,
                session.active_downscroll,
                session.hud_visible,
                session.hud_opacity,
                session.hud_zoom,
                session.arrows_visible,
                session.arrows_opacity,
                session.screen_blackout_alpha,
                session.game_over,
            );

            if !in_countdown && !session.song_started {
                session.song_started = true;
                session.runtime.call_on_song_start();
            }

            while session.next_event_idx < self.project.chart.events.len() {
                let ev = &self.project.chart.events[session.next_event_idx];
                if song_pos_ms < ev.time_ms {
                    break;
                }
                if let Some(cam_action) =
                    parse_chart_camera_event(&ev.name, &ev.value1, &ev.value2, active_bpm)
                {
                    apply_chart_camera_action(session, &mut self.project.stage, cam_action);
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

            update_native_camera_follow(session, &mut self.project.stage, dt);

            let (p_hold, o_hold, any_recent_miss, triggered_instakill) = process_chart_notes(
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

            if (triggered_instakill || session.health <= 0.0001) && !session.practice_mode {
                enter_game_over(session, &self.audio_engine);
            } else {
                let sing_hold_sec = (beat_sec * 0.90).clamp(0.28, 0.58);
                for ent in &mut session.bundle.active_entities {
                    ent.anim_timer_sec += dt;
                    if ent.active_anim != CharacterAnimKind::Idle
                        && ent.anim_timer_sec > sing_hold_sec
                    {
                        ent.active_anim = CharacterAnimKind::Idle;
                        ent.anim_timer_sec = 0.0;
                    }
                }

                if song_pos_ms > self.project.chart.stats.duration_ms + 1200.0 {
                    session.finished = true;
                    self.audio_engine.mixer.pause();
                }
            }
        }

        if should_restart_after_confirm {
            let _ = self.restart_gameplay();
            return self.step_gameplay_internal(0.0, 0, 0);
        }

        if prev_judgement != session.last_judgement
            || prev_paused != session.paused
            || prev_finished != session.finished
            || prev_log_count != session.last_logs.len()
            || prev_last_log.as_deref() != session.last_logs.last().map(|s| s.as_str())
            || prev_err_count != session.script_errors.len()
            || prev_hud_visible != session.hud_visible
            || (prev_hud_opacity - session.hud_opacity).abs() > 0.002
            || prev_arrows_visible != session.arrows_visible
            || (prev_arrows_opacity - session.arrows_opacity).abs() > 0.002
            || (prev_blackout_alpha - session.screen_blackout_alpha).abs() > 0.002
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
