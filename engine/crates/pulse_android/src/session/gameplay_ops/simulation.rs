//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/simulation.rs
//! Propósito: Bucle interno de simulación del gameplay frame a frame (`step_gameplay_internal`):
//! cálculo de cuenta regresiva BPM, hooks de beats/steps/eventos, evaluación de notas
//! (`Sick`/`Good`/`Bad`/`Miss` y sostenidas `Hold`), retorno a `Idle` y recolección en arena.

use super::enter_game_over;
use super::script_sync::{
    drain_and_apply_script_commands, sync_script_host_state, update_active_timers,
    update_active_tweens,
};
use crate::session::{
    ActiveTweenState, EngineSession, GameplayRuntimeState, TweenTarget, dir_to_anim,
    dir_to_miss_anim, trigger_character_anim,
};
use pulse_core::chart::{ChartEvent, ChartNote, NoteSpecialKind, ParsedChartCameraAction};
use pulse_core::frame_memory::{ArenaSpan, FrameVisibleNote, NotePlayState};
use pulse_core::sparrow::CharacterAnimKind;
use pulse_core::{
    CameraFollowMode, EntityRole, StageEntity, StageScene, parse_chart_camera_event,
};
use std::sync::atomic::Ordering;

const PRELOAD_LOOKAHEAD_MS: f64 = 4000.0;

fn collect_preload_targets_from_event(ev: &ChartEvent, entities: &[StageEntity], out: &mut Vec<String>) {
    let lower_name = ev.name.trim().to_ascii_lowercase();
    if lower_name == "preload sprite" {
        for id in ev.value1.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
            out.push(id.to_string());
        }
        return;
    }
    if lower_name == "stage swap" || lower_name == "swap stage" || lower_name == "change character" {
        for part in ev.value1.split(';').chain(ev.value2.split(';')) {
            if let Some((k, v)) = part.trim().split_once('=') {
                let key = k.trim().to_ascii_lowercase();
                if matches!(
                    key.as_str(),
                    "show" | "mostrar" | "player" | "jugador" | "opp" | "opponent" | "enemigo" | "rival"
                ) {
                    for id in v.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                        out.push(id.to_string());
                    }
                }
            }
        }
        return;
    }
    // Eventos genéricos: precarga entidades ocultas mencionadas en los parámetros
    for ent in entities {
        if (!ent.visible || ent.opacity <= 0.005)
            && (ev.value1.contains(&ent.id) || ev.value2.contains(&ent.id))
        {
            out.push(ent.id.clone());
        }
    }
}

const HIT_WINDOW_SICK_MS: f64 = 55.0;
const HIT_WINDOW_GOOD_MS: f64 = 105.0;
const HIT_WINDOW_BAD_MS: f64 = 155.0;

fn push_camera_pos_tween(
    session: &mut GameplayRuntimeState,
    stage: &StageScene,
    tx: f32,
    ty: f32,
    duration_sec: f32,
    ease: &str,
) {
    let (clamped_x, clamped_y) =
        stage.clamp_camera_to_background(tx, ty, stage.camera_base_zoom);
    session
        .active_tweens
        .retain(|t| !matches!(t.target, TweenTarget::CameraPosX | TweenTarget::CameraPosY));
    session.active_tweens.push(ActiveTweenState {
        tag: "__chart_cam_x".to_string(),
        target: TweenTarget::CameraPosX,
        start_val: stage.camera_x,
        end_val: clamped_x,
        elapsed_sec: 0.0,
        duration_sec: duration_sec.max(0.001),
        ease: ease.to_string(),
    });
    session.active_tweens.push(ActiveTweenState {
        tag: "__chart_cam_y".to_string(),
        target: TweenTarget::CameraPosY,
        start_val: stage.camera_y,
        end_val: clamped_y,
        elapsed_sec: 0.0,
        duration_sec: duration_sec.max(0.001),
        ease: ease.to_string(),
    });
}

fn push_camera_zoom_tween(
    session: &mut GameplayRuntimeState,
    stage: &StageScene,
    target_zoom: f32,
    duration_sec: f32,
    ease: &str,
) {
    session
        .active_tweens
        .retain(|t| !matches!(t.target, TweenTarget::CameraZoom));
    session.active_tweens.push(ActiveTweenState {
        tag: "__chart_cam_z".to_string(),
        target: TweenTarget::CameraZoom,
        start_val: stage.camera_zoom,
        end_val: target_zoom.clamp(0.35, 2.5),
        elapsed_sec: 0.0,
        duration_sec: duration_sec.max(0.001),
        ease: ease.to_string(),
    });
}

fn resolve_gf_target(stage: &StageScene) -> (f32, f32) {
    let cam0 = stage.resolve_camera_slot_target(0, "", "");
    if let Some(gf) = stage.entities.iter().find(|e| {
        e.role == EntityRole::Decoration
            && (e.id.eq_ignore_ascii_case("stage_speakers")
                || e.id.to_ascii_lowercase().contains("gf")
                || e.display_name.to_ascii_lowercase().contains("cabina"))
    }) {
        let dest_x = gf.x + gf.camera_offset[0];
        let dest_y = gf.y + gf.camera_offset[1] - 20.0;
        let pan = stage.camera.pan_strength.clamp(0.0, 1.0);
        let tx = (cam0.0 + (dest_x - cam0.0) * pan).clamp(-1000.0, 2600.0);
        let ty = (cam0.1 + (dest_y - cam0.1) * pan).clamp(-1000.0, 1800.0);
        (tx, ty)
    } else {
        (cam0.0, cam0.1)
    }
}

fn apply_chart_camera_action(
    session: &mut GameplayRuntimeState,
    stage: &mut StageScene,
    action: ParsedChartCameraAction,
) {
    match action {
        ParsedChartCameraAction::SelectCamera {
            slot,
            focus_gf,
            offset_x,
            offset_y,
            zoom,
            duration_sec,
            ease,
        } => {
            session.camera_follow_lock = None;
            session.camera_gf_override = focus_gf;
            session.camera_offset_override = [offset_x, offset_y];

            let max_slot = stage.camera.cameras.len().saturating_sub(1);
            let target_slot = slot.unwrap_or(session.active_camera_slot).min(max_slot);
            if slot.is_some() {
                session.active_camera_slot = target_slot;
            }

            let (base_tx, base_ty, slot_zoom, _) = if focus_gf {
                let (gx, gy) = resolve_gf_target(stage);
                let (_, _, z, s) = stage.resolve_camera_slot_target(
                    session.active_camera_slot,
                    &session.active_player_id,
                    &session.active_opponent_id,
                );
                (gx, gy, z, s)
            } else {
                stage.resolve_camera_slot_target(
                    target_slot,
                    &session.active_player_id,
                    &session.active_opponent_id,
                )
            };

            let z_override = zoom.map(|z| z.clamp(0.35, 2.5));
            session.camera_slot_zoom_override = z_override;
            let final_zoom = z_override.unwrap_or(slot_zoom);
            stage.camera_base_zoom = final_zoom;
            let (tx, ty) = stage.clamp_camera_to_background(
                base_tx + offset_x,
                base_ty + offset_y,
                final_zoom,
            );

            if duration_sec > 0.01 {
                push_camera_pos_tween(session, stage, tx, ty, duration_sec, &ease);
                if z_override.is_some() {
                    push_camera_zoom_tween(session, stage, final_zoom, duration_sec, &ease);
                }
            } else {
                session
                    .active_tweens
                    .retain(|t| !matches!(t.target, TweenTarget::CameraPosX | TweenTarget::CameraPosY));
            }
        }
        ParsedChartCameraAction::FollowPos {
            x,
            y,
            zoom,
            duration_sec,
            ease,
        } => {
            let target_zoom = zoom.map(|z| z.clamp(0.35, 2.5)).unwrap_or(stage.camera_base_zoom);
            let (cx, cy) = stage.clamp_camera_to_background(x, y, target_zoom);
            session.camera_follow_lock = Some([cx, cy]);
            session.camera_gf_override = false;
            session.camera_offset_override = [0.0, 0.0];
            if let Some(z) = zoom {
                let cz = z.clamp(0.35, 2.5);
                session.camera_slot_zoom_override = Some(cz);
                stage.camera_base_zoom = cz;
                if duration_sec > 0.01 {
                    push_camera_zoom_tween(session, stage, cz, duration_sec, &ease);
                }
            }
            if duration_sec > 0.01 {
                push_camera_pos_tween(session, stage, cx, cy, duration_sec, &ease);
            }
        }
        ParsedChartCameraAction::ClearFollowPos => {
            session.camera_follow_lock = None;
            session.camera_offset_override = [0.0, 0.0];
        }
        ParsedChartCameraAction::SetZoom {
            zoom,
            is_direct_stage,
            duration_sec,
            ease,
        } => {
            let final_zoom = if is_direct_stage {
                (stage.camera.base_zoom * zoom).clamp(0.35, 2.5)
            } else {
                zoom.clamp(0.35, 2.5)
            };
            session.camera_slot_zoom_override = Some(final_zoom);
            stage.camera_base_zoom = final_zoom;
            if duration_sec > 0.01 {
                push_camera_zoom_tween(session, stage, final_zoom, duration_sec, &ease);
            } else {
                session
                    .active_tweens
                    .retain(|t| !matches!(t.target, TweenTarget::CameraZoom));
                stage.camera_zoom = final_zoom;
            }
        }
        ParsedChartCameraAction::AddZoomBump {
            cam_zoom_add,
            hud_zoom_add,
        } => {
            stage.camera_zoom = (stage.camera_zoom + cam_zoom_add).clamp(0.35, 2.5);
            session.hud_zoom = (session.hud_zoom + hud_zoom_add).clamp(0.35, 2.5);
        }
    }
}

fn update_native_camera_follow(
    session: &mut GameplayRuntimeState,
    stage: &mut StageScene,
    dt: f32,
) {
    let (_, _, slot_zoom, slot_smooth) = stage.resolve_camera_slot_target(
        session.active_camera_slot,
        &session.active_player_id,
        &session.active_opponent_id,
    );
    let desired_base_zoom = session
        .camera_slot_zoom_override
        .unwrap_or(slot_zoom)
        .clamp(0.35, 2.5);
    stage.camera_base_zoom = desired_base_zoom;

    if stage.camera.enabled {
        let pos_tween_active = session
            .active_tweens
            .iter()
            .any(|t| matches!(t.target, TweenTarget::CameraPosX | TweenTarget::CameraPosY));
        if !pos_tween_active {
            let (raw_tx, raw_ty) = if let Some([lx, ly]) = session.camera_follow_lock {
                (lx, ly)
            } else if session.camera_gf_override {
                let (gx, gy) = resolve_gf_target(stage);
                (
                    gx + session.camera_offset_override[0],
                    gy + session.camera_offset_override[1],
                )
            } else {
                let (tx, ty, _, _) = stage.resolve_camera_slot_target(
                    session.active_camera_slot,
                    &session.active_player_id,
                    &session.active_opponent_id,
                );
                (
                    tx + session.camera_offset_override[0],
                    ty + session.camera_offset_override[1],
                )
            };
            let (target_x, target_y) =
                stage.clamp_camera_to_background(raw_tx, raw_ty, desired_base_zoom);
            let lerp_t = (dt * slot_smooth.clamp(1.0, 20.0)).min(1.0);
            stage.camera_x += (target_x - stage.camera_x) * lerp_t;
            stage.camera_y += (target_y - stage.camera_y) * lerp_t;
        }
    }

    let zoom_tween_active = session
        .active_tweens
        .iter()
        .any(|t| matches!(t.target, TweenTarget::CameraZoom));
    if !zoom_tween_active {
        stage.camera_zoom += (desired_base_zoom - stage.camera_zoom) * (dt * 6.0).min(1.0);
    }

    // Garantizar en cada frame que la cámara (incluyendo tweens o cambios de zoom) no descubra bordes negros fuera del fondo.
    if stage.camera.enabled {
        let (clamped_cam_x, clamped_cam_y) =
            stage.clamp_camera_to_background(stage.camera_x, stage.camera_y, stage.camera_zoom);
        stage.camera_x = clamped_cam_x;
        stage.camera_y = clamped_cam_y;
    }

    let hud_zoom_tween_active = session
        .active_tweens
        .iter()
        .any(|t| matches!(t.target, TweenTarget::HudProp { ref prop } if prop == "zoom"));
    if !hud_zoom_tween_active {
        session.hud_zoom += (1.0 - session.hud_zoom) * (dt * 6.0).min(1.0);
    }

    stage.camera_shake *= (1.0 - dt * 10.0).max(0.0);
}

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

            // Precarga anticipada (4 segundos antes) de sprites ocultos referenciados por eventos futuros
            let mut preload_ids_buf: Vec<String> = Vec::new();
            while session.next_preload_event_idx < self.project.chart.events.len() {
                let ev = &self.project.chart.events[session.next_preload_event_idx];
                if ev.time_ms > song_pos_ms + PRELOAD_LOOKAHEAD_MS {
                    break;
                }
                preload_ids_buf.clear();
                collect_preload_targets_from_event(
                    ev,
                    &session.bundle.active_entities,
                    &mut preload_ids_buf,
                );
                for id in preload_ids_buf.drain(..) {
                    session.retired_entities.remove(&id);
                    session.preload_requested_entities.insert(id);
                }
                session.next_preload_event_idx += 1;
            }

            // Plan temporal precomputado por el compilador en caliente: precarga temprana y retiro seguro
            let timeline_clock_ms = song_pos_ms.max(0.0);
            while session.next_timeline_preload_idx < session.timeline_preload_queue.len() {
                let preload_at_ms = session.timeline_preload_queue[session.next_timeline_preload_idx].0;
                if preload_at_ms > timeline_clock_ms {
                    break;
                }
                let id = session.timeline_preload_queue[session.next_timeline_preload_idx]
                    .1
                    .clone();
                session.retired_entities.remove(&id);
                session.preload_requested_entities.insert(id);
                session.next_timeline_preload_idx += 1;
            }
            while session.next_timeline_retire_idx < session.timeline_retire_queue.len() {
                let retire_after_ms =
                    session.timeline_retire_queue[session.next_timeline_retire_idx].0;
                if retire_after_ms > song_pos_ms {
                    break;
                }
                let id = session.timeline_retire_queue[session.next_timeline_retire_idx]
                    .1
                    .clone();
                if !session.reused_later_entities.contains(&id) {
                    let is_still_hidden = session
                        .bundle
                        .active_entities
                        .iter()
                        .find(|e| e.id == id)
                        .map(|e| !e.visible || e.opacity <= 0.005)
                        .unwrap_or(true);
                    if is_still_hidden {
                        if let Some(ent) = session
                            .bundle
                            .active_entities
                            .iter_mut()
                            .find(|e| e.id == id)
                        {
                            ent.visible = false;
                        }
                        session.preload_requested_entities.remove(&id);
                        session.retired_entities.insert(id);
                    }
                }
                session.next_timeline_retire_idx += 1;
            }

            if !in_countdown {
                if sec_idx as isize != session.last_section_idx {
                    session.last_section_idx = sec_idx as isize;
                    if let Some(explicit_slot) = section_cam_slot {
                        let max_slot = self.project.stage.camera.cameras.len().saturating_sub(1);
                        session.active_camera_slot = explicit_slot.min(max_slot);
                        session.camera_follow_lock = None;
                        session.camera_gf_override = false;
                        session.camera_offset_override = [0.0, 0.0];
                        session.camera_slot_zoom_override = None;
                    } else if self.project.stage.camera.follow_mode == CameraFollowMode::ChartSections
                    {
                        session.camera_gf_override = gf_section_now;
                        session.camera_offset_override = [0.0, 0.0];
                        session.camera_slot_zoom_override = None;
                        if !gf_section_now {
                            session.active_camera_slot = if must_hit_now { 1 } else { 2 };
                        }
                    }
                }
                if self.project.stage.camera.follow_mode == CameraFollowMode::BeatsInterval
                    && section_cam_slot.is_none()
                    && session.camera_follow_lock.is_none()
                {
                    let interval = self.project.stage.camera.follow_interval_beats.max(1) as i32;
                    let bucket = current_beat.div_euclid(interval);
                    if bucket != session.last_cam_beat_bucket {
                        session.last_cam_beat_bucket = bucket;
                        session.active_camera_slot = if bucket.rem_euclid(2) == 0 { 2 } else { 1 };
                        session.camera_gf_override = false;
                        session.camera_offset_override = [0.0, 0.0];
                        session.camera_slot_zoom_override = None;
                    }
                }
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
