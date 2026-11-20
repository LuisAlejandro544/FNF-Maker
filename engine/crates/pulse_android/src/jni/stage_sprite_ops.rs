//! Archivo: engine/crates/pulse_android/src/jni/stage_sprite_ops.rs
//! Propósito: Puntos de entrada JNI para hojas de sprites, atlas Sparrow XML, offsets de animación,
//! descriptores de personaje/escenario (`character.json` / `stage.json`), entidades y multicámara.

use super::{read_str, to_jstr};
use crate::session;
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jfloat, jint};
use std::path::PathBuf;

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportSpriteSheetPng<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    entity_id: JString<'local>,
    path: JString<'local>,
) -> JString<'local> {
    let eid = read_str(&mut env, &entity_id);
    let p = PathBuf::from(read_str(&mut env, &path));
    let json = session().lock().import_sprite_png(&eid, p);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportSparrowXml<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    entity_id: JString<'local>,
    path: JString<'local>,
) -> JString<'local> {
    let eid = read_str(&mut env, &entity_id);
    let p = PathBuf::from(read_str(&mut env, &path));
    let json = session().lock().import_sparrow_xml(&eid, p);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSetSparrowBinding<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    entity_id: JString<'local>,
    anim_kind: JString<'local>,
    prefix: JString<'local>,
) -> JString<'local> {
    let eid = read_str(&mut env, &entity_id);
    let kind = read_str(&mut env, &anim_kind);
    let pref = read_str(&mut env, &prefix);
    let json = session().lock().set_sparrow_binding(&eid, &kind, &pref);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeGetSparrowFrameInfo<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    entity_id: JString<'local>,
    anim_kind: JString<'local>,
    elapsed_sec: jfloat,
) -> JString<'local> {
    let eid = read_str(&mut env, &entity_id);
    let kind = read_str(&mut env, &anim_kind);
    let json = session()
        .lock()
        .get_sparrow_frame_info(&eid, &kind, elapsed_sec);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportCharacterConfig<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    entity_id: JString<'local>,
    path: JString<'local>,
) -> JString<'local> {
    let eid = read_str(&mut env, &entity_id);
    let p = PathBuf::from(read_str(&mut env, &path));
    let json = session().lock().import_character_config(&eid, p);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSetEntityAnimOffset<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    entity_id: JString<'local>,
    anim_kind: JString<'local>,
    offset_x: jfloat,
    offset_y: jfloat,
) -> JString<'local> {
    let eid = read_str(&mut env, &entity_id);
    let kind = read_str(&mut env, &anim_kind);
    let json = session()
        .lock()
        .set_entity_anim_offset(&eid, &kind, offset_x, offset_y);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportStageConfig<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    path: JString<'local>,
) -> JString<'local> {
    let p = PathBuf::from(read_str(&mut env, &path));
    let json = session().lock().import_stage_config(p);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeAddStageEntity<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    name: JString<'local>,
    role: JString<'local>,
) -> JString<'local> {
    let name_s = read_str(&mut env, &name);
    let role_s = read_str(&mut env, &role);
    let json = session().lock().add_stage_entity(&name_s, &role_s);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeUpdateStageEntity<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    entity_id: JString<'local>,
    name: JString<'local>,
    role: JString<'local>,
    dx: jfloat,
    dy: jfloat,
    abs_x: jfloat,
    abs_y: jfloat,
    use_abs_pos: jboolean,
    scale: jfloat,
    opacity: jfloat,
    visible: jboolean,
    flip_x: jboolean,
    commit_save: jboolean,
) -> JString<'local> {
    let eid = read_str(&mut env, &entity_id);
    let name_s = read_str(&mut env, &name);
    let role_s = read_str(&mut env, &role);
    let json = session().lock().update_stage_entity(
        &eid,
        &name_s,
        &role_s,
        dx,
        dy,
        abs_x,
        abs_y,
        use_abs_pos != 0,
        scale,
        opacity,
        visible != 0,
        flip_x != 0,
        commit_save != 0,
    );
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeReorderStageEntity<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    entity_id: JString<'local>,
    action: JString<'local>,
) -> JString<'local> {
    let eid = read_str(&mut env, &entity_id);
    let act = read_str(&mut env, &action);
    let json = session().lock().reorder_stage_entity(&eid, &act);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeUpdateStageCamera<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    enabled: jboolean,
    base_x: jfloat,
    base_y: jfloat,
    base_zoom: jfloat,
    follow_mode: JString<'local>,
    follow_interval_beats: jint,
    smooth_speed: jfloat,
    pan_strength: jfloat,
    clamp_to_background: jboolean,
    commit_save: jboolean,
) -> JString<'local> {
    let mode_s = read_str(&mut env, &follow_mode);
    let json = session().lock().update_stage_camera(
        enabled != 0,
        base_x,
        base_y,
        base_zoom,
        &mode_s,
        follow_interval_beats,
        smooth_speed,
        pan_strength,
        clamp_to_background != 0,
        commit_save != 0,
    );
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSelectStageCamera<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    slot_index: jint,
) -> JString<'local> {
    let json = session()
        .lock()
        .select_stage_camera(slot_index.max(0) as usize);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeUpdateCameraSlot<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    slot_index: jint,
    name: JString<'local>,
    target_kind: JString<'local>,
    target_entity_id: JString<'local>,
    x: jfloat,
    y: jfloat,
    zoom: jfloat,
    smooth_speed: jfloat,
    pan_strength: jfloat,
    commit_save: jboolean,
) -> JString<'local> {
    let name_s = read_str(&mut env, &name);
    let kind_s = read_str(&mut env, &target_kind);
    let entity_s = read_str(&mut env, &target_entity_id);
    let json = session().lock().update_camera_slot(
        slot_index.max(0) as usize,
        &name_s,
        &kind_s,
        &entity_s,
        x,
        y,
        zoom,
        smooth_speed,
        pan_strength,
        commit_save != 0,
    );
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeAddCustomCamera<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    name: JString<'local>,
    target_kind: JString<'local>,
    target_entity_id: JString<'local>,
    x: jfloat,
    y: jfloat,
    zoom: jfloat,
    smooth_speed: jfloat,
    pan_strength: jfloat,
) -> JString<'local> {
    let name_s = read_str(&mut env, &name);
    let kind_s = read_str(&mut env, &target_kind);
    let entity_s = read_str(&mut env, &target_entity_id);
    let json = session().lock().add_custom_camera(
        &name_s,
        &kind_s,
        &entity_s,
        x,
        y,
        zoom,
        smooth_speed,
        pan_strength,
    );
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeRemoveCustomCamera<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    slot_index: jint,
) -> JString<'local> {
    let json = session()
        .lock()
        .remove_custom_camera(slot_index.max(0) as usize);
    to_jstr(&mut env, &json)
}
