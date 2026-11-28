//! Archivo: engine/crates/pulse_android/src/jni/project_ops.rs
//! Propósito: Puntos de entrada JNI para inicialización de sesión nativa, cuota de caché,
//! guardado, listado/creación/duplicado/borrado de proyectos, metadatos e import/export `.bfsmod`/Vanilla.

use super::{read_str, to_jstr};
use crate::{
    INITIAL_CACHE_QUOTA_BYTES, SESSION, init_session_with_dirs, session,
};
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jdouble, jlong};
use std::path::PathBuf;
use std::sync::atomic::Ordering;

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSetCacheQuota(
    _env: JNIEnv,
    _cls: JClass,
    quota_bytes: jlong,
) {
    let quota = quota_bytes.max(0) as u64;
    INITIAL_CACHE_QUOTA_BYTES.store(quota, Ordering::Relaxed);
    if let Some(sess) = SESSION.get() {
        sess.lock().set_cache_quota_bytes(quota);
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeInit<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    cache_dir: JString<'local>,
    data_dir: JString<'local>,
) -> JString<'local> {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("BeatForgeEngine"),
    );
    let cdir = PathBuf::from(read_str(&mut env, &cache_dir));
    let ddir_raw = read_str(&mut env, &data_dir);
    let ddir = if ddir_raw.trim().is_empty() {
        cdir.clone()
    } else {
        PathBuf::from(ddir_raw)
    };
    let sess = init_session_with_dirs(cdir, ddir);
    let json = sess.lock().snapshot_json_string();
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeFlushSave(
    _env: JNIEnv,
    _cls: JClass,
) {
    session().lock().flush_pending_save();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeListProjects<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JString<'local> {
    let json = session().lock().list_projects_json();
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeCreateProject<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    name: JString<'local>,
    author: JString<'local>,
) -> JString<'local> {
    let n = read_str(&mut env, &name);
    let a = read_str(&mut env, &author);
    let json = session().lock().create_project(&n, &a);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeOpenProject<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    project_id: JString<'local>,
) -> JString<'local> {
    let pid = read_str(&mut env, &project_id);
    let json = session().lock().open_project(&pid);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeDuplicateProject<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    project_id: JString<'local>,
) -> JString<'local> {
    let pid = read_str(&mut env, &project_id);
    let json = session().lock().duplicate_project(&pid);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeDeleteProject<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    project_id: JString<'local>,
) -> JString<'local> {
    let pid = read_str(&mut env, &project_id);
    let json = session().lock().delete_project(&pid);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeExportModPackage<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    project_id: JString<'local>,
    out_path: JString<'local>,
    optimize_images: jboolean,
) -> JString<'local> {
    let pid = read_str(&mut env, &project_id);
    let out = PathBuf::from(read_str(&mut env, &out_path));
    let json = session()
        .lock()
        .export_project_vanilla_mod(&pid, &out, optimize_images != 0);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportModPackage<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    path: JString<'local>,
) -> JString<'local> {
    let p = PathBuf::from(read_str(&mut env, &path));
    let json = session().lock().import_project_vanilla_mod(p);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeGetSnapshot<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JString<'local> {
    let json = session().lock().snapshot_json_string();
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeUpdateProjectMeta<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    name: JString<'local>,
    author: JString<'local>,
    downscroll: jboolean,
    control_mode: JString<'local>,
    audio_offset_ms: jdouble,
    scroll_speed: jdouble,
) -> JString<'local> {
    let name_s = read_str(&mut env, &name);
    let author_s = read_str(&mut env, &author);
    let mode_s = read_str(&mut env, &control_mode);
    let json = session().lock().update_project_meta(
        name_s,
        author_s,
        downscroll != 0,
        &mode_s,
        audio_offset_ms,
        scroll_speed,
    );
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeUpgradeProjectDevApi<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JString<'local> {
    let json = session().lock().upgrade_dev_api_version();
    to_jstr(&mut env, &json)
}
