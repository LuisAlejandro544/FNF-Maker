//! Archivo: engine/crates/pulse_android/src/session/gameplay_ops/simulation/timeline_ram.rs
//! Propósito: Gestión anticipada de RAM durante el gameplay:
//! - Precarga lookahead (4 segundos antes) de sprites ocultos referenciados por eventos futuros.
//! - Ejecución de la línea temporal precomputada en caliente (`timeline_preload_queue` y
//!   `timeline_retire_queue`) para cargar texturas a tiempo y retirar sprites no reutilizados.

use crate::session::GameplayRuntimeState;
use pulse_core::StageEntity;
use pulse_core::chart::ChartEvent;

const PRELOAD_LOOKAHEAD_MS: f64 = 4000.0;

fn collect_preload_targets_from_event(
    ev: &ChartEvent,
    entities: &[StageEntity],
    out: &mut Vec<String>,
) {
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

/// Avanza la precarga anticipada por eventos y la cola temporal compilada de precarga/retiro en RAM.
pub(super) fn advance_timeline_preload_and_retire(
    session: &mut GameplayRuntimeState,
    events: &[ChartEvent],
    song_pos_ms: f64,
) {
    // Precarga anticipada (4 segundos antes) de sprites ocultos referenciados por eventos futuros
    let mut preload_ids_buf: Vec<String> = Vec::new();
    while session.next_preload_event_idx < events.len() {
        let ev = &events[session.next_preload_event_idx];
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
        let retire_after_ms = session.timeline_retire_queue[session.next_timeline_retire_idx].0;
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
}
