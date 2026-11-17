//! Archivo: engine/crates/pulse_core/src/lib.rs
//! Propósito: Punto de entrada del módulo núcleo `pulse_core`.
//! Expone toda la lógica independiente de la UI y del hardware:
//! - `astc_loader`: Decodificador puro en Rust de texturas comprimidas `.astc` y cargador de imágenes.
//! - `atlas_json`: Parsers y ensambladores para `spritemap1.json`, `Animation.json` y TexturePacker `.json`.
//! - `chart`: Lectura, edición y exportación completa de archivos `chart.json` (con eventos, personajes y BPM por sección).
//! - `sparrow`: Lectura de `.xml` + `.png` y armado automático de animaciones (Idle, Arriba, Abajo, Izquierda, Derecha).
//! - `stage`: Grafo de escenario horizontal, identificadores (Fondo, Player, Enemigo, Decoración), Z-order y transformaciones.
//! - `svg_skin`: Carga y rasterización de flechas en `.svg` personalizadas o por defecto libres de derechos.
//! - `project`: Estructura completa de un proyecto mod (incluyendo límite de 1 a 3 pistas de audio).
//! - `hot_compile`: Compilador en caliente paralelo con descarte de objetos invisibles, análisis inteligente de bucles en scripts y escalado adaptativo sin pérdida visible.
//! - `frame_memory`: Arena temporal por frame (`FrameBumpArena`), ventana deslizante de notas (`NoteSlidingWindow`) y caché LRU (`LruCache`).
//! - `script_optimizer`: Analizador inteligente de bucles pesados/infinitos en Lua, JS y TypeScript.

pub mod astc_loader;
pub mod atlas_json;
pub mod chart;
pub mod frame_memory;
pub mod hot_compile;
pub mod project;
pub mod script_optimizer;
pub mod sparrow;
pub mod stage;
pub mod svg_skin;

pub use astc_loader::{
    decode_astc_file_bytes, is_astc_path, is_webp_path, load_sprite_image, read_sprite_dimensions,
    save_lossless_webp,
};
pub use atlas_json::{AnimateDocument, AnimateSpritemap, DetectedSpriteJson, bake_animate_atlas, parse_sprite_json_str};
pub use chart::{ChartData, ChartEvent, ChartNote, ChartSection, ChartStats, NoteDirection};
pub use frame_memory::{ArenaSpan, FrameBumpArena, FrameVisibleNote, LruCache, NotePlayState, NoteSlidingWindow};
pub use hot_compile::{
    AssetFileFingerprint, AudioMergeFingerprint, AudioSlotFingerprint, CachedAudioEntry,
    CachedScriptEntry, CachedTextureEntry, CompiledBundle, DeviceResourceTier,
    HotCompileManifest, HotCompileReport, SCRIPT_OPTIMIZER_CACHE_VERSION, TextureFingerprint,
    fnv1a_64_bytes, fnv1a_64_str, hot_compile_project, hot_compile_project_to_dir,
    hot_compile_project_to_dir_with_manifest,
};
pub use project::{
    AudioTrackSlot, CAMERA_SCRIPT_ID, CAMERA_SCRIPT_NAME, MobileControlMode, ModProject,
    ScriptAsset, ScriptLanguage, parse_camera_config_from_lua, render_camera_lua_script,
};
pub use script_optimizer::{ScriptBatchReport, ScriptLoopOptResult, optimize_scripts_for_hot_compile, optimize_single_script};
pub use sparrow::{AnimSet, CharacterAnimKind, SparrowAtlas, SparrowFrame};
pub use stage::{CameraFollowMode, EntityRole, StageCameraConfig, StageEntity, StageScene};
pub use svg_skin::{ArrowSkinSet, RasterizedArrow, default_arrow_svg_bundle, default_receptor_svg_bundle, desaturate_to_neutral_receptor, rasterize_arrow_skin};
