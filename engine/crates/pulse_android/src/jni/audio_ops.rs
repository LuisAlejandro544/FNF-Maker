//! Archivo: engine/crates/pulse_android/src/jni/audio_ops.rs
//! Propósito: Puntos de entrada JNI para importar, quitar o configurar hasta 3 pistas de audio
//! y controlar el transporte de preescucha en tiempo real (incluida lectura lock-free de estado).

use super::{read_str, to_jstr};
use crate::session;
use jni::JNIEnv;
use jni::objects::{JClass, JDoubleArray, JString};
use jni::sys::{jboolean, jdouble, jfloat, jint};
use std::path::PathBuf;

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeImportAudioTrack<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    slot: jint,
    path: JString<'local>,
) -> JString<'local> {
    let p = PathBuf::from(read_str(&mut env, &path));
    let json = session().lock().import_audio_track(slot.max(0) as usize, p);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeRemoveAudioTrack<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    slot: jint,
) -> JString<'local> {
    let json = session().lock().remove_audio_track(slot.max(0) as usize);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeUpdateAudioTrackConfig<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    slot: jint,
    gain: jfloat,
    muted: jboolean,
    duck_on_miss: jboolean,
) -> JString<'local> {
    let json = session().lock().update_audio_track_config(
        slot.max(0) as usize,
        gain,
        muted != 0,
        duck_on_miss != 0,
    );
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeToggleAudioPreview(
    _env: JNIEnv,
    _cls: JClass,
    play: jboolean,
    start_ms: jdouble,
) -> jboolean {
    let mut guard = session().lock();
    if play != 0 {
        guard.audio_engine.ensure_started();
        let any_audio = guard.project.active_audio_count() > 0;
        guard
            .audio_engine
            .mixer
            .use_demo_synth
            .store(!any_audio, std::sync::atomic::Ordering::Relaxed);
        guard.audio_engine.mixer.demo_bpm_bits.store(
            (guard.project.chart.bpm as f32).to_bits(),
            std::sync::atomic::Ordering::Relaxed,
        );
        guard.audio_engine.mixer.seek_ms(start_ms.max(0.0));
        guard.audio_engine.mixer.play();
        1
    } else {
        guard.audio_engine.mixer.pause();
        0
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSetAudioPreviewTransport(
    _env: JNIEnv,
    _cls: JClass,
    play: jboolean,
    seek_ms: jdouble,
) -> jboolean {
    let mut guard = session().lock();
    let any_audio = guard.project.active_audio_count() > 0;
    let demo_bpm = guard.project.chart.bpm as f32;
    if seek_ms >= 0.0 {
        guard.audio_engine.mixer.seek_ms(seek_ms);
    }
    if play != 0 {
        guard.audio_engine.ensure_started();
        guard
            .audio_engine
            .mixer
            .use_demo_synth
            .store(!any_audio, std::sync::atomic::Ordering::Relaxed);
        guard
            .audio_engine
            .mixer
            .demo_bpm_bits
            .store(demo_bpm.to_bits(), std::sync::atomic::Ordering::Relaxed);
        guard.audio_engine.mixer.play();
        1
    } else {
        guard.audio_engine.mixer.pause();
        0
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeReadAudioPreviewState<'local>(
    env: JNIEnv<'local>,
    _cls: JClass<'local>,
    out_state: JDoubleArray<'local>,
) -> jboolean {
    // Lectura 100% lock-free sobre atómicos de `PREVIEW_MIXER` para seguir el audio
    // a 60Hz desde el hilo UI sin competir jamás con el mutex global `SESSION`.
    let (pos_ms, is_playing, duration_ms) = crate::session::PREVIEW_MIXER
        .get()
        .map(|mixer| mixer.read_preview_state_lockfree())
        .unwrap_or((0.0, false, 0.0));

    let buf = [pos_ms, if is_playing { 1.0 } else { 0.0 }, duration_ms];
    if env.set_double_array_region(&out_state, 0, &buf).is_ok() {
        1
    } else {
        0
    }
}
