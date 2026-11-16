//! Archivo: engine/crates/pulse_core/src/lib.rs
//! Propósito: Punto de entrada del módulo núcleo `pulse_core`.
//! Expone toda la lógica independiente de la UI y del hardware:
//! - `chart`: Lectura y validación de archivos `.json` de ritmo (estilo estándar comunitario).
//! - `sparrow`: Lectura de `.xml` + `.png` y armado automático de animaciones (Idle, Arriba, Abajo, Izquierda, Derecha).
//! - `stage`: Grafo de escenario horizontal, identificadores (Fondo, Player, Enemigo, Decoración), Z-order y transformaciones.
//! - `svg_skin`: Carga y rasterización de flechas en `.svg` personalizadas o por defecto libres de derechos.
//! - `project`: Estructura completa de un proyecto mod (incluyendo límite de 1 a 3 pistas de audio).
//! - `hot_compile`: Compilador en caliente paralelo con descarte de objetos invisibles y escalado por recursos.

pub mod chart;
pub mod hot_compile;
pub mod project;
pub mod sparrow;
pub mod stage;
pub mod svg_skin;

pub use chart::{ChartData, ChartNote, ChartSection, ChartStats, NoteDirection};
pub use hot_compile::{CompiledBundle, DeviceResourceTier, HotCompileReport, hot_compile_project};
pub use project::{AudioTrackSlot, ModProject, ScriptAsset, ScriptLanguage};
pub use sparrow::{AnimSet, CharacterAnimKind, SparrowAtlas, SparrowFrame};
pub use stage::{EntityRole, StageEntity, StageScene};
pub use svg_skin::{ArrowSkinSet, RasterizedArrow, default_arrow_svg_bundle, rasterize_arrow_skin};
