//! Archivo: engine/crates/pulse_core/src/chart/mod.rs
//! Propósito: Punto de entrada modular del subsistema de charts (`chart.json`).
//! Reexporta tipos de dominio, parser multi-formato, exportador JSON,
//! operaciones de edición e intérprete de eventos de cámara.

mod camera_events;
mod editor;
mod models;
mod parser;
mod serializer;

#[cfg(test)]
mod tests;

pub use camera_events::{ParsedChartCameraAction, parse_chart_camera_event};
pub use models::{
    ChartData, ChartError, ChartEvent, ChartIntroUiConfig, ChartNote, ChartSection, ChartStats,
    NoteDirection, NoteSpecialKind,
};
