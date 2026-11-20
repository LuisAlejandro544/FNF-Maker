//! Archivo: engine/crates/pulse_android/src/jni/chart_ops.rs
//! Propósito: Puntos de entrada JNI para importación/exportación de `chart.json`,
//! restauración del chart demo, edición de metadatos, intro UI, secciones, BPM, eventos y notas.

use super::{read_str, to_jstr};
use crate::session;
use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jdouble, jfloat, jint};
use std::path::PathBuf;

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportChart<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    path: JString<'local>,
) -> JString<'local> {
    let p = PathBuf::from(read_str(&mut env, &path));
    let json = session().lock().import_chart(p);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeRestoreDemoChart<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JString<'local> {
    let json = {
        let mut guard = session().lock();
        guard.project.chart = pulse_core::ChartData::bundled_demo_chart();
        guard.project.chart_source_path = None;
        guard.project.sync_stage_events_script();
        guard.project.sync_ui_blackout_script();
        guard.invalidate_compiled_bundle();
        guard.sync_mixer_chart_duration();
        guard.save_active_project_now();
        guard.ok_response("Chart demo restaurado")
    };
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSetChartBpm<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    bpm: jdouble,
) -> JString<'local> {
    let json = session().lock().set_chart_bpm(bpm);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeExportChartJson<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JString<'local> {
    let json = session().lock().export_chart_json();
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeUpdateChartDetails<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    title: JString<'local>,
    speed: jdouble,
    player1: JString<'local>,
    player2: JString<'local>,
    gf_version: JString<'local>,
    stage: JString<'local>,
    needs_voices: jboolean,
) -> JString<'local> {
    let t = read_str(&mut env, &title);
    let p1 = read_str(&mut env, &player1);
    let p2 = read_str(&mut env, &player2);
    let gf = read_str(&mut env, &gf_version);
    let st = read_str(&mut env, &stage);
    let json = session().lock().update_chart_details(
        &t,
        speed,
        &p1,
        &p2,
        &gf,
        &st,
        needs_voices != 0,
    );
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeUpdateChartIntroUi<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    start_blackout: jboolean,
    blackout_alpha: jfloat,
    hide_hud: jboolean,
    hide_arrows: jboolean,
    reveal_on_song_start: jboolean,
    reveal_delay_ms: jdouble,
    fade_sec: jfloat,
) -> JString<'local> {
    let json = session().lock().update_chart_intro_ui(
        start_blackout != 0,
        blackout_alpha,
        hide_hud != 0,
        hide_arrows != 0,
        reveal_on_song_start != 0,
        reveal_delay_ms,
        fade_sec,
    );
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSetSectionBpm<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    sec_index: jint,
    bpm: jdouble,
    change_bpm: jboolean,
) -> JString<'local> {
    let json = session().lock().set_section_bpm(
        sec_index.max(0) as usize,
        bpm,
        change_bpm != 0,
    );
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeAddChartEvent<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    time_ms: jdouble,
    name: JString<'local>,
    value1: JString<'local>,
    value2: JString<'local>,
) -> JString<'local> {
    let n = read_str(&mut env, &name);
    let v1 = read_str(&mut env, &value1);
    let v2 = read_str(&mut env, &value2);
    let json = session().lock().add_chart_event(time_ms, &n, &v1, &v2);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeRemoveChartEvent<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    index: jint,
) -> JString<'local> {
    let json = session().lock().remove_chart_event(index.max(0) as usize);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeAddChartSection<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    must_hit: jboolean,
) -> JString<'local> {
    let json = session().lock().add_chart_section(must_hit != 0);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeToggleSectionMustHit<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    sec_index: jint,
) -> JString<'local> {
    let json = session()
        .lock()
        .toggle_section_must_hit(sec_index.max(0) as usize);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSetSectionCameraSlot<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    sec_index: jint,
    camera_slot: jint,
) -> JString<'local> {
    let json = session()
        .lock()
        .set_section_camera_slot(sec_index.max(0) as usize, camera_slot);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeToggleChartNote<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    sec_index: jint,
    step_in_sec: jint,
    col_0_to_7: jint,
    sustain_steps: jint,
) -> JString<'local> {
    let json = session().lock().toggle_chart_note(
        sec_index.max(0) as usize,
        step_in_sec.max(0) as u32,
        col_0_to_7.clamp(0, 7) as u8,
        sustain_steps.max(0) as u32,
    );
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeToggleChartNoteWithType<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    sec_index: jint,
    step_in_sec: jint,
    col_0_to_7: jint,
    sustain_steps: jint,
    note_type: JString<'local>,
) -> JString<'local> {
    let nt = read_str(&mut env, &note_type);
    let json = session().lock().toggle_chart_note_with_type(
        sec_index.max(0) as usize,
        step_in_sec.max(0) as u32,
        col_0_to_7.clamp(0, 7) as u8,
        sustain_steps.max(0) as u32,
        &nt,
    );
    to_jstr(&mut env, &json)
}
