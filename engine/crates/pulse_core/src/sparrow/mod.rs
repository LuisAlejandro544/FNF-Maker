//! Archivo: engine/crates/pulse_core/src/sparrow/mod.rs
//! Propósito: Punto de entrada modular del analizador de hojas de sprites Sparrow `.xml`.
//! Agrupa modelos (`models`), parsing XML (`xml_parser`), detección automática de poses
//! (`bindings`), normalización y alineación de pies por píxeles (`normalize`) y pruebas (`tests`),
//! preservando las exportaciones públicas originales para `pulse_core` y `pulse_android`.

mod bindings;
mod models;
mod normalize;
mod xml_parser;

#[cfg(test)]
mod tests;

pub use models::{AnimSet, CharacterAnimKind, SparrowAtlas, SparrowError, SparrowFrame};
pub use xml_parser::{natural_frame_cmp, strip_trailing_frame_digits};
