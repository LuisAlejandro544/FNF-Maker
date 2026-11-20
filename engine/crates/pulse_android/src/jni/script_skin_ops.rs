//! Archivo: engine/crates/pulse_android/src/jni/script_skin_ops.rs
//! Propósito: Puntos de entrada JNI para gestión/validación de scripts (Lua/TS/JS)
//! y skins SVG/atlas/PNG de flechas, barra de vida, iconos, ratings y note splashes.

use super::{read_str, to_jstr};
use crate::session;
use jni::JNIEnv;
use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{jboolean, jint};
use std::path::PathBuf;

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeAddScript<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    language: JString<'local>,
) -> JString<'local> {
    let lang = read_str(&mut env, &language);
    let json = session().lock().add_script(&lang);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportScript<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    path: JString<'local>,
) -> JString<'local> {
    let p = PathBuf::from(read_str(&mut env, &path));
    let json = session().lock().import_script(p);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeUpdateScript<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    script_id: JString<'local>,
    enabled: jboolean,
    source_code: JString<'local>,
) -> JString<'local> {
    let sid = read_str(&mut env, &script_id);
    let src = read_str(&mut env, &source_code);
    let json = session().lock().update_script(&sid, enabled != 0, src);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeValidateScript<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    script_id: JString<'local>,
) -> JString<'local> {
    let sid = read_str(&mut env, &script_id);
    let json = session().lock().validate_script(&sid);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportArrowSvg<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    direction: jint,
    path: JString<'local>,
) -> JString<'local> {
    let p = PathBuf::from(read_str(&mut env, &path));
    let json = session()
        .lock()
        .import_arrow_svg((direction.clamp(0, 3)) as u8, p);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeRestoreDefaultArrowSvg<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JString<'local> {
    let json = session().lock().restore_default_arrow_svg();
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeGetArrowPngBytes<'local>(
    env: JNIEnv<'local>,
    _cls: JClass<'local>,
    direction: jint,
) -> JByteArray<'local> {
    let bytes = session()
        .lock()
        .rasterize_arrow_png_bytes((direction.clamp(0, 3)) as u8);
    env.byte_array_from_slice(&bytes).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeGetReceptorPngBytes<'local>(
    env: JNIEnv<'local>,
    _cls: JClass<'local>,
    direction: jint,
) -> JByteArray<'local> {
    let bytes = session()
        .lock()
        .rasterize_receptor_png_bytes((direction.clamp(0, 3)) as u8);
    env.byte_array_from_slice(&bytes).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportCharacterIcon<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    entity_id: JString<'local>,
    is_losing: jboolean,
    path: JString<'local>,
) -> JString<'local> {
    let eid = read_str(&mut env, &entity_id);
    let p = PathBuf::from(read_str(&mut env, &path));
    let json = session()
        .lock()
        .import_character_icon(&eid, is_losing != 0, p);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeClearCharacterIcons<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    entity_id: JString<'local>,
) -> JString<'local> {
    let eid = read_str(&mut env, &entity_id);
    let json = session().lock().clear_character_icons(&eid);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportHealthBarSvg<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    path: JString<'local>,
) -> JString<'local> {
    let p = PathBuf::from(read_str(&mut env, &path));
    let json = session().lock().import_health_bar_svg(p);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeRestoreDefaultHealthBarSvg<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JString<'local> {
    let json = session().lock().restore_default_health_bar_svg();
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeGetHealthBarPngBytes<'local>(
    env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JByteArray<'local> {
    let bytes = session().lock().rasterize_health_bar_png_bytes();
    env.byte_array_from_slice(&bytes).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeGetRatingPngBytes<'local>(
    env: JNIEnv<'local>,
    _cls: JClass<'local>,
    rating_code: jint,
) -> JByteArray<'local> {
    let bytes = session()
        .lock()
        .rasterize_rating_png_bytes((rating_code.clamp(1, 4)) as u8);
    env.byte_array_from_slice(&bytes).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeGetDefaultIconPngBytes<'local>(
    env: JNIEnv<'local>,
    _cls: JClass<'local>,
    is_player: jboolean,
    is_losing: jboolean,
) -> JByteArray<'local> {
    let bytes = session()
        .lock()
        .rasterize_default_icon_png_bytes(is_player != 0, is_losing != 0);
    env.byte_array_from_slice(&bytes).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeGetNoteSplashPngBytes<'local>(
    env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JByteArray<'local> {
    let bytes = session().lock().rasterize_note_splash_png_bytes();
    env.byte_array_from_slice(&bytes).unwrap_or_default()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportArrowAtlas<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    png_path: JString<'local>,
    xml_path: JString<'local>,
) -> JString<'local> {
    let png = PathBuf::from(read_str(&mut env, &png_path));
    let xml = PathBuf::from(read_str(&mut env, &xml_path));
    let json = session().lock().import_arrow_atlas(png, xml);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeGetHurtArrowPngBytes<'local>(
    env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JByteArray<'local> {
    let bytes = session().lock().rasterize_hurt_arrow_png_bytes();
    env.byte_array_from_slice(&bytes).unwrap_or_default()
}
