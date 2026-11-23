//! Archivo: engine/crates/pulse_android/src/lib.rs
//! Propósito: Punto de entrada de la librería nativa `pulse_android` (`cdylib` / `rlib`)
//! entre Kotlin Jetpack Compose (`NativeEngineBridge.kt`) y el motor modular en Rust
//! (`pulse_core`, `pulse_audio`, `pulse_scripting`). Gestiona el estado global de sesión
//! (`SESSION`), progreso atómico de compilación en caliente y monta el submódulo `jni`.

#[cfg(target_os = "android")]
pub mod jni;
pub mod session;

use crate::session::EngineSession;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) static SESSION: OnceLock<Mutex<EngineSession>> = OnceLock::new();
pub(crate) static INITIAL_CACHE_QUOTA_BYTES: AtomicU64 = AtomicU64::new(0);
static HOT_COMPILE_PROGRESS: OnceLock<Mutex<HotCompileProgressState>> = OnceLock::new();
static AUDIO_ADAPT_PROGRESS: OnceLock<Mutex<AudioAdaptProgressState>> = OnceLock::new();

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HotCompileProgressState {
    pub active: bool,
    pub progress: f32,
    pub phase: String,
    pub detail: String,
}

impl Default for HotCompileProgressState {
    fn default() -> Self {
        Self {
            active: false,
            progress: 0.0,
            phase: "Preparando compilador...".to_string(),
            detail: String::new(),
        }
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioAdaptProgressState {
    pub active: bool,
    pub progress: f32,
    pub phase: String,
    pub detail: String,
}

impl Default for AudioAdaptProgressState {
    fn default() -> Self {
        Self {
            active: false,
            progress: 0.0,
            phase: "Preparando adaptador acústico...".to_string(),
            detail: String::new(),
        }
    }
}

fn progress_store() -> &'static Mutex<HotCompileProgressState> {
    HOT_COMPILE_PROGRESS.get_or_init(|| Mutex::new(HotCompileProgressState::default()))
}

fn audio_adapt_progress_store() -> &'static Mutex<AudioAdaptProgressState> {
    AUDIO_ADAPT_PROGRESS.get_or_init(|| Mutex::new(AudioAdaptProgressState::default()))
}

pub fn set_hot_compile_progress(active: bool, progress: f32, phase: &str, detail: &str) {
    let mut guard = progress_store().lock();
    guard.active = active;
    guard.progress = progress.clamp(0.0, 1.0);
    guard.phase = phase.to_string();
    guard.detail = detail.to_string();
}

pub fn get_hot_compile_progress_json() -> String {
    let guard = progress_store().lock();
    serde_json::to_string(&*guard).unwrap_or_else(|_| {
        "{\"active\":false,\"progress\":0.0,\"phase\":\"Preparando...\",\"detail\":\"\"}".to_string()
    })
}

pub fn set_audio_adapt_progress(active: bool, progress: f32, phase: &str, detail: &str) {
    let mut guard = audio_adapt_progress_store().lock();
    guard.active = active;
    guard.progress = progress.clamp(0.0, 1.0);
    guard.phase = phase.to_string();
    guard.detail = detail.to_string();
}

pub fn get_audio_adapt_progress_json() -> String {
    let guard = audio_adapt_progress_store().lock();
    serde_json::to_string(&*guard).unwrap_or_else(|_| {
        "{\"active\":false,\"progress\":0.0,\"phase\":\"Preparando adaptador acústico...\",\"detail\":\"\"}".to_string()
    })
}

pub fn init_session(cache_dir: PathBuf) -> &'static Mutex<EngineSession> {
    init_session_with_dirs(cache_dir.clone(), cache_dir)
}

pub fn init_session_with_dirs(cache_dir: PathBuf, data_dir: PathBuf) -> &'static Mutex<EngineSession> {
    let quota = INITIAL_CACHE_QUOTA_BYTES.load(Ordering::Relaxed);
    SESSION.get_or_init(|| {
        Mutex::new(EngineSession::new_with_dirs_and_quota(
            cache_dir, data_dir, quota,
        ))
    })
}

pub fn session() -> &'static Mutex<EngineSession> {
    SESSION.get_or_init(|| Mutex::new(EngineSession::new(std::env::temp_dir())))
}
