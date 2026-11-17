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
pub mod gfx_probe;
pub mod persistence_ops;
pub mod sprite_ops;

pub use dto::{
    ActionResultJson, ActiveTimerState, ActiveTweenState, AudioTrackSnapshotJson,
    ChartEventJson, ChartSectionSnapshotJson, ChartSnapshotJson, CompileReportJson,
    DynamicDrawableState, DynamicScriptItemJson, GameplayEntityFrameJson, GameplayRuntimeState,
    GameplayStepJson, ProjectSnapshotJson, ProjectSummaryJson, ScriptSnapshotJson,
    SparrowFrameJson, StageEntitySnapshotJson, StrumOverrideJson, StrumOverrideState,
    TweenTarget, VisibleNoteJson,
};

use pulse_audio::{AudioEngineHandle, DecodedTrack};
use pulse_core::disk_cache::{
    CacheBudgetSplit, DEFAULT_TOTAL_CACHE_QUOTA_BYTES, compute_cache_budget_split,
    evict_lru_files, evict_lru_subdirs,
};
use pulse_core::hot_compile::{CompiledBundle, HotCompileReport};
use pulse_core::project::{MAX_AUDIO_TRACKS, MobileControlMode, ModProject};
use pulse_core::sparrow::CharacterAnimKind;
use pulse_core::{EntityRole, NoteDirection, ScriptLanguage, StageEntity};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

pub struct EngineSession {
    pub cache_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_quota_bytes: u64,
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
        Self::new_with_dirs_and_quota(cache_dir, data_dir, DEFAULT_TOTAL_CACHE_QUOTA_BYTES)
    }

    pub fn new_with_dirs(cache_dir: PathBuf, data_dir: PathBuf) -> Self {
        Self::new_with_dirs_and_quota(cache_dir, data_dir, DEFAULT_TOTAL_CACHE_QUOTA_BYTES)
    }

    pub fn new_with_dirs_and_quota(
        cache_dir: PathBuf,
        data_dir: PathBuf,
        cache_quota_bytes: u64,
    ) -> Self {
        let mut sess = Self {
            cache_dir,
            data_dir,
            cache_quota_bytes: if cache_quota_bytes == 0 {
                DEFAULT_TOTAL_CACHE_QUOTA_BYTES
            } else {
                cache_quota_bytes
            },
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

    pub fn cache_budget(&self) -> CacheBudgetSplit {
        compute_cache_budget_split(self.cache_quota_bytes)
    }

    pub fn compiled_cache_root_dir(&self) -> PathBuf {
        self.cache_dir.join("compiled_cache")
    }

    pub fn compiled_cache_dir_for(&self, project_id: &str) -> PathBuf {
        self.compiled_cache_root_dir()
            .join(persistence_ops::sanitize_project_id(project_id))
    }

    pub fn active_compiled_cache_dir(&self) -> PathBuf {
        self.compiled_cache_dir_for(&self.project.id)
    }

    pub fn audio_cache_dir(&self) -> PathBuf {
        self.cache_dir.join("audio_cache")
    }

    pub fn set_cache_quota_bytes(&mut self, quota_bytes: u64) {
        self.cache_quota_bytes = if quota_bytes == 0 {
            DEFAULT_TOTAL_CACHE_QUOTA_BYTES
        } else {
            quota_bytes
        };
        let budget = self.cache_budget();
        let active_safe = persistence_ops::sanitize_project_id(&self.project.id);
        let _ = evict_lru_subdirs(
            &self.compiled_cache_root_dir(),
            budget.compiled_cache_bytes,
            &[&active_safe],
        );
        let _ = evict_lru_files(&self.audio_cache_dir(), budget.audio_cache_bytes, &[]);
    }
}

/// Extrae el nombre legible del archivo importado quitando el prefijo temporal `<epochMs>_`
/// agregado por el staging SAF cuando existe.
pub(super) fn friendly_import_name(path: &Path) -> String {
    let raw = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("import")
        .to_string();
    if let Some((prefix, rest)) = raw.split_once('_') {
        if prefix.len() >= 10 && prefix.chars().all(|c| c.is_ascii_digit()) && !rest.is_empty() {
            return rest.to_string();
        }
    }
    raw
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
        CharacterAnimKind::DeathStart => "death_start",
        CharacterAnimKind::DeathLoop => "death_loop",
        CharacterAnimKind::DeathConfirm => "death_confirm",
    }
}

pub(super) fn parse_anim_kind(key: &str) -> Option<CharacterAnimKind> {
    match key.trim().to_ascii_lowercase().as_str() {
        "idle" | "reposo" => Some(CharacterAnimKind::Idle),
        "left" | "izquierda" | "singleft" => Some(CharacterAnimKind::Left),
        "down" | "abajo" | "singdown" => Some(CharacterAnimKind::Down),
        "up" | "arriba" | "singup" => Some(CharacterAnimKind::Up),
        "right" | "derecha" | "singright" => Some(CharacterAnimKind::Right),
        "miss_left" | "leftmiss" | "singleftmiss" | "fallo_izquierda" => Some(CharacterAnimKind::MissLeft),
        "miss_down" | "downmiss" | "singdownmiss" | "fallo_abajo" => Some(CharacterAnimKind::MissDown),
        "miss_up" | "upmiss" | "singupmiss" | "fallo_arriba" => Some(CharacterAnimKind::MissUp),
        "miss_right" | "rightmiss" | "singrightmiss" | "fallo_derecha" => Some(CharacterAnimKind::MissRight),
        "death_start" | "deathstart" | "firstdeath" | "death" | "dies" | "muerte_inicio" => Some(CharacterAnimKind::DeathStart),
        "death_loop" | "deathloop" | "dead" | "muerte_loop" => Some(CharacterAnimKind::DeathLoop),
        "death_confirm" | "deathconfirm" | "retry" | "muerte_confirm" => Some(CharacterAnimKind::DeathConfirm),
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

pub(super) fn dir_to_miss_anim(dir: NoteDirection) -> CharacterAnimKind {
    match dir {
        NoteDirection::Left => CharacterAnimKind::MissLeft,
        NoteDirection::Down => CharacterAnimKind::MissDown,
        NoteDirection::Up => CharacterAnimKind::MissUp,
        NoteDirection::Right => CharacterAnimKind::MissRight,
    }
}

pub(super) fn trigger_character_anim(
    entities: &mut [StageEntity],
    role: EntityRole,
    active_id: &str,
    anim: CharacterAnimKind,
) {
    let mut matched_active = false;
    if !active_id.is_empty() {
        for ent in entities.iter_mut() {
            if ent.id == active_id {
                ent.active_anim = anim;
                ent.anim_timer_sec = 0.0;
                matched_active = true;
            }
        }
    }
    if !matched_active {
        for ent in entities.iter_mut() {
            if ent.role == role && ent.visible {
                ent.active_anim = anim;
                ent.anim_timer_sec = 0.0;
            }
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
