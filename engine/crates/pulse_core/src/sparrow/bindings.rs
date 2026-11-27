//! Archivo: engine/crates/pulse_core/src/sparrow/bindings.rs
//! Propósito: Detección automática de enlaces (`CharacterAnimKind`) a partir de los prefijos
//! del atlas Sparrow (Idle, Left/Down/Up/Right, poses Miss y secuencia Death en inglés/español).

use super::models::{AnimSet, CharacterAnimKind, SparrowAtlas};
use std::collections::{BTreeMap, HashMap};

impl SparrowAtlas {
    /// Detecta automáticamente a qué dirección corresponde cada prefijo de animación del XML.
    /// Soporta convenciones en inglés y español (ej. `singLEFT`, `left`, `izquierda`, `idle`, `dance`).
    pub fn auto_detect_bindings(
        animations: &BTreeMap<String, AnimSet>,
    ) -> HashMap<CharacterAnimKind, String> {
        let mut bindings = HashMap::new();
        let prefixes: Vec<&String> = animations.keys().collect();

        for prefix in &prefixes {
            let low = prefix.to_ascii_lowercase();
            let is_miss = low.contains("miss") || low.contains("fallo");
            let is_death = low.contains("death") || low.contains("dies") || low.contains("gameover") || low.contains("muerte");

            if is_death {
                if low.contains("confirm") || low.contains("retry") || low.contains("reintent") {
                    bindings.entry(CharacterAnimKind::DeathConfirm).or_insert_with(|| (*prefix).clone());
                } else if low.contains("loop") || low.contains("bucle") || low.contains("dead") {
                    bindings.entry(CharacterAnimKind::DeathLoop).or_insert_with(|| (*prefix).clone());
                } else {
                    bindings.entry(CharacterAnimKind::DeathStart).or_insert_with(|| (*prefix).clone());
                }
            } else if !is_miss && (low.contains("idle") || low.contains("dance") || low.contains("reposo")) {
                bindings.entry(CharacterAnimKind::Idle).or_insert_with(|| (*prefix).clone());
            } else if low.contains("left") || low.contains("izq") {
                let kind = if is_miss { CharacterAnimKind::MissLeft } else { CharacterAnimKind::Left };
                bindings.entry(kind).or_insert_with(|| (*prefix).clone());
            } else if low.contains("down") || low.contains("abajo") {
                let kind = if is_miss { CharacterAnimKind::MissDown } else { CharacterAnimKind::Down };
                bindings.entry(kind).or_insert_with(|| (*prefix).clone());
            } else if low.contains("up") || low.contains("arriba") {
                let kind = if is_miss { CharacterAnimKind::MissUp } else { CharacterAnimKind::Up };
                bindings.entry(kind).or_insert_with(|| (*prefix).clone());
            } else if low.contains("right") || low.contains("der") {
                let kind = if is_miss { CharacterAnimKind::MissRight } else { CharacterAnimKind::Right };
                bindings.entry(kind).or_insert_with(|| (*prefix).clone());
            }
        }

        // Si es un objeto de fondo o decoración con una sola animación, enlazarla como Idle.
        if !bindings.contains_key(&CharacterAnimKind::Idle) {
            if let Some(first) = prefixes.first() {
                bindings.insert(CharacterAnimKind::Idle, (*first).clone());
            }
        }

        bindings
    }
}
