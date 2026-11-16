//! Archivo: engine/crates/pulse_android/src/lib.rs
//! Propósito: Punto de entrada nativo `android_main` para Android (`GameActivity`) y puente JNI
//! bidireccional con `SafFileBridge.kt` para importar charts, pistas de audio (1-3),
//! hojas PNG + XML, flechas SVG y scripts comunitarios desde el almacenamiento del teléfono.

use parking_lot::Mutex;
use pulse_core::NoteDirection;
use pulse_ui::state::{ImportKind, ImportedFileEvent, PlatformFilePicker};
use pulse_ui::BeatForgeApp;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use tokio::sync::mpsc;

/// Canal global protegido para entregar archivos copiados por las corrutinas Kotlin al hilo Rust.
static PENDING_PICK_TX: OnceLock<Mutex<Option<(ImportKind, mpsc::UnboundedSender<ImportedFileEvent>)>>> =
    OnceLock::new();

fn pick_slot() -> &'static Mutex<Option<(ImportKind, mpsc::UnboundedSender<ImportedFileEvent>)>> {
    PENDING_PICK_TX.get_or_init(|| Mutex::new(None))
}

/// Implementación real del selector de archivos llamando a Kotlin `MainActivity.launchSafPicker` vía JNI.
pub struct AndroidSafPicker;

impl PlatformFilePicker for AndroidSafPicker {
    fn request_import(&self, kind: ImportKind, tx: mpsc::UnboundedSender<ImportedFileEvent>) {
        *pick_slot().lock() = Some((kind, tx));

        #[cfg(target_os = "android")]
        {
            let kind_code = encode_import_kind(kind);
            if let Err(e) = call_kotlin_saf_picker(kind_code) {
                log::error!("Error invocando SAF picker vía JNI: {}", e);
            }
        }
    }
}

fn encode_import_kind(kind: ImportKind) -> i32 {
    match kind {
        ImportKind::ChartJson => 1,
        ImportKind::AudioTrack(0) => 10,
        ImportKind::AudioTrack(1) => 11,
        ImportKind::AudioTrack(_) => 12,
        ImportKind::SpriteSheetPng => 20,
        ImportKind::SpriteAtlasXml => 21,
        ImportKind::ArrowSvg(NoteDirection::Left) => 30,
        ImportKind::ArrowSvg(NoteDirection::Down) => 31,
        ImportKind::ArrowSvg(NoteDirection::Up) => 32,
        ImportKind::ArrowSvg(NoteDirection::Right) => 33,
        ImportKind::ScriptFile => 40,
    }
}

#[cfg(target_os = "android")]
fn call_kotlin_saf_picker(kind_code: i32) -> Result<(), String> {
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let activity = unsafe { jni::objects::JObject::from_raw(ctx.context().cast()) };

    env.call_method(
        &activity,
        "launchSafPicker",
        "(I)V",
        &[jni::objects::JValue::Int(kind_code)],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// Callback JNI llamado desde `SafFileBridge.kt` cuando la corrutina `Dispatchers.IO`
/// termina de copiar el archivo seleccionado a almacenamiento interno.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn Java_com_beatforge_studio_SafFileBridge_nativeOnFilePicked(
    mut env: jni::JNIEnv,
    _class: jni::objects::JClass,
    local_path: jni::objects::JString,
) {
    if let Ok(path_str) = env.get_string(&local_path) {
        let path_buf = PathBuf::from(path_str.to_string_lossy().into_owned());
        if let Some((kind, tx)) = pick_slot().lock().take() {
            let _ = tx.send(ImportedFileEvent {
                kind,
                path: path_buf,
            });
        }
    }
}

/// Punto de entrada principal ejecutado por Android GameActivity.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn android_main(app: winit::platform::android::activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_min_level(log::LevelFilter::Info)
            .with_tag("BeatForgeEngine"),
    );

    let options = eframe::NativeOptions {
        android_app: Some(app),
        ..Default::default()
    };

    let picker: Arc<dyn PlatformFilePicker> = Arc::new(AndroidSafPicker);
    let _ = eframe::run_native(
        "BeatForge Studio",
        options,
        Box::new(move |cc| Ok(Box::new(BeatForgeApp::new(cc, picker)))),
    );
}
