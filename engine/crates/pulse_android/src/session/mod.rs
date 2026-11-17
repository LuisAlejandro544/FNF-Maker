//! Archivo: engine/crates/pulse_android/src/session/mod.rs
//! Propósito: Núcleo de sesión del motor Rust (`EngineSession`) compartido por el puente JNI.
//! Divide sus responsabilidades en módulos especializados:
//! - `dto`: Estructuras JSON serializables (`ProjectSnapshotJson`, `GameplayStepJson`, etc.) y builders.
//! - `editor_ops`: Edición de charts, audio, escenario, scripts y skins SVG.
//! - `sprite_ops`: Carga de texturas `.png`/`.astc`, XML Sparrow y horneado Adobe Animate.
//! - `gameplay_ops`: Compilación en caliente (`rayon`) y simulación frame a frame con cuenta regresiva BPM.

//! - `persistence_ops`: Persistencia automática instantánea en `filesDir` y menú de proyectos.

pub mod dto;
pub mod editor_ops;
pub mod gameplay_ops;
pub mod persistence_ops;
pub mod sprite_ops;

pub use dto::{
    ActionResultJson, AudioTrackSnapshotJson, ChartEventJson, ChartSectionSnapshotJson,
    ChartSnapshotJson, CompileReportJson, GameplayEntityFrameJson, GameplayRuntimeState,
    GameplayStepJson, ProjectSnapshotJson, ProjectSummaryJson, ScriptSnapshotJson,
    SparrowFrameJson, StageEntitySnapshotJson, VisibleNoteJson,
};

use pulse_audio::{AudioEngineHandle, DecodedTrack};
use pulse_core::hot_compile::{CompiledBundle, HotCompileReport};
use pulse_core::project::{MAX_AUDIO_TRACKS, MobileControlMode, ModProject};
use pulse_core::sparrow::CharacterAnimKind;
use pulse_core::{EntityRole, NoteDirection, ScriptLanguage, StageEntity};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

pub struct EngineSession {
    pub cache_dir: PathBuf,
    pub data_dir: PathBuf,
    pub project: ModProject,
    pub audio_engine: AudioEngineHandle,
    pub decoded_tracks: [Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS],
    pub compiled_bundle: Option<CompiledBundle>,
    pub compiled_texture_paths: HashMap<String, String>,
    pub compiled_audio_path: Option<PathBuf>,
    pub last_compile_report: Option<HotCompileReport>,
    pub gameplay: Option<GameplayRuntimeState>,
    pub dirty: bool,
    pub last_save_at: Instant,
}

impl EngineSession {
    pub fn new(cache_dir: PathBuf) -> Self {
        let data_dir = cache_dir.clone();
        Self::new_with_dirs(cache_dir, data_dir)
    }

    pub fn new_with_dirs(cache_dir: PathBuf, data_dir: PathBuf) -> Self {
        let mut sess = Self {
            cache_dir,
            data_dir,
            project: ModProject::new_demo(),
            audio_engine: AudioEngineHandle::new_deferred(),
            decoded_tracks: [None, None, None],
            compiled_bundle: None,
            compiled_texture_paths: HashMap::new(),
            compiled_audio_path: None,
            last_compile_report: None,
            gameplay: None,
            dirty: false,
            last_save_at: Instant::now(),
        };
        sess.load_initial_project();
        sess
    }
}

pub(super) fn role_to_key(role: EntityRole) -> &'static str {
    match role {
        EntityRole::Background => "background",
        EntityRole::Player => "player",
        EntityRole::Opponent => "opponent",
        EntityRole::Decoration => "decoration",
    }
}

pub(super) fn parse_role_key(key: &str) -> EntityRole {
    match key {
        "background" => EntityRole::Background,
        "player" => EntityRole::Player,
        "opponent" => EntityRole::Opponent,
        _ => EntityRole::Decoration,
    }
}

pub(super) fn lang_to_key(lang: ScriptLanguage) -> &'static str {
    match lang {
        ScriptLanguage::Lua => "lua",
        ScriptLanguage::TypeScript => "ts",
        ScriptLanguage::JavaScript => "js",
    }
}

pub(super) fn parse_lang_key(key: &str) -> ScriptLanguage {
    match key {
        "ts" | "typescript" => ScriptLanguage::TypeScript,
        "js" | "javascript" => ScriptLanguage::JavaScript,
        _ => ScriptLanguage::Lua,
    }
}

pub(super) fn control_mode_to_key(mode: MobileControlMode) -> &'static str {
    match mode {
        MobileControlMode::TapArrows => "tap_arrows",
        MobileControlMode::ClassicHitbox => "classic_hitbox",
    }
}

pub(super) fn parse_control_mode_key(key: &str) -> MobileControlMode {
    match key {
        "classic_hitbox" | "hitbox" | "classic" => MobileControlMode::ClassicHitbox,
        _ => MobileControlMode::TapArrows,
    }
}

pub(super) fn anim_kind_key(kind: CharacterAnimKind) -> &'static str {
    match kind {
        CharacterAnimKind::Idle => "idle",
        CharacterAnimKind::Left => "left",
        CharacterAnimKind::Down => "down",
        CharacterAnimKind::Up => "up",
        CharacterAnimKind::Right => "right",
        CharacterAnimKind::MissLeft => "miss_left",
        CharacterAnimKind::MissDown => "miss_down",
        CharacterAnimKind::MissUp => "miss_up",
        CharacterAnimKind::MissRight => "miss_right",
    }
}

pub(super) fn parse_anim_kind(key: &str) -> Option<CharacterAnimKind> {
    match key {
        "idle" => Some(CharacterAnimKind::Idle),
        "left" => Some(CharacterAnimKind::Left),
        "down" => Some(CharacterAnimKind::Down),
        "up" => Some(CharacterAnimKind::Up),
        "right" => Some(CharacterAnimKind::Right),
        _ => None,
    }
}

pub(super) fn dir_to_anim(dir: NoteDirection) -> CharacterAnimKind {
    match dir {
        NoteDirection::Left => CharacterAnimKind::Left,
        NoteDirection::Down => CharacterAnimKind::Down,
        NoteDirection::Up => CharacterAnimKind::Up,
        NoteDirection::Right => CharacterAnimKind::Right,
    }
}

pub(super) fn trigger_role_anim(
    entities: &mut [StageEntity],
    role: EntityRole,
    anim: CharacterAnimKind,
) {
    for ent in entities.iter_mut() {
        if ent.role == role {
            ent.active_anim = anim;
            ent.anim_timer_sec = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_session_hot_compile_and_gameplay_step_work() {
        let tmp = std::env::temp_dir().join("beatforge_session_test");
        let mut session = EngineSession::new(tmp);
        let compile_json = session.hot_compile(1);
        assert!(compile_json.contains("\"ok\":true"));

        let gp_json = session.start_gameplay();
        assert!(gp_json.contains("\"active\":true"));

        let arrow_png = session.rasterize_arrow_png_bytes(0);
        assert!(arrow_png.starts_with(&[0x89, b'P', b'N', b'G']));

        let receptor_png = session.rasterize_receptor_png_bytes(0);
        assert!(receptor_png.starts_with(&[0x89, b'P', b'N', b'G']));
    }
}
