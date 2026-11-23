//! Archivo: engine/crates/pulse_android/src/jni/gameplay_gfx_ops.rs
//! Propósito: Puntos de entrada JNI para compilación en caliente (con lectura atómica de progreso),
//! bucle de gameplay (JSON y paquete binario zero-copy), pausa/reinicio/game over, debug y GPU backends.

use super::{read_str, to_jstr};
use crate::{get_hot_compile_progress_json, session, set_hot_compile_progress};
use jni::JNIEnv;
use jni::objects::{JByteBuffer, JClass, JString};
use jni::sys::{jboolean, jfloat, jint};

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeGetHotCompileProgress<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JString<'local> {
    // Lectura atómica sobre `HOT_COMPILE_PROGRESS` sin bloquear jamás el mutex `SESSION`.
    let json = get_hot_compile_progress_json();
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeHotCompile<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    tier_code: jint,
) -> JString<'local> {
    set_hot_compile_progress(
        true,
        0.02,
        "Iniciando compilador en caliente...",
        "Preparando caché incremental y pool paralelo",
    );
    let json = session().lock().hot_compile_with_progress(
        tier_code,
        |pct, phase, detail| {
            set_hot_compile_progress(true, pct, phase, detail);
        },
    );
    set_hot_compile_progress(
        false,
        0.90,
        "Motor Rust completado",
        "Cargando texturas WebP y audio hacia gameplay",
    );
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeHotCompileWithViewport<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    tier_code: jint,
    viewport_w: jint,
    viewport_h: jint,
) -> JString<'local> {
    set_hot_compile_progress(
        true,
        0.02,
        "Iniciando compilador en caliente...",
        "Calculando zoom máximo de cámara y escala física en pantalla",
    );
    let json = session().lock().hot_compile_with_progress_and_viewport(
        tier_code,
        viewport_w,
        viewport_h,
        |pct, phase, detail| {
            set_hot_compile_progress(true, pct, phase, detail);
        },
    );
    set_hot_compile_progress(
        false,
        0.90,
        "Motor Rust completado",
        "Texturas pre-escaladas con Lanczos3 y scripts listos",
    );
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeStartGameplay<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JString<'local> {
    let json = session().lock().start_gameplay();
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeStepGameplay<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    dt_sec: jfloat,
    lane_pressed_mask: jint,
    lane_just_pressed_mask: jint,
) -> JString<'local> {
    let json = session()
        .lock()
        .step_gameplay(dt_sec, lane_pressed_mask, lane_just_pressed_mask);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeStepGameplayBinary<'local>(
    env: JNIEnv<'local>,
    _cls: JClass<'local>,
    dt_sec: jfloat,
    lane_pressed_mask: jint,
    lane_just_pressed_mask: jint,
    out_buffer: JByteBuffer<'local>,
) -> jint {
    let Ok(ptr) = env.get_direct_buffer_address(&out_buffer) else {
        return -1;
    };
    let Ok(cap) = env.get_direct_buffer_capacity(&out_buffer) else {
        return -1;
    };
    if ptr.is_null() || cap == 0 {
        return -1;
    }
    // SAFETY: `out_buffer` es un DirectByteBuffer válido proporcionado por Kotlin;
    // su rango `[ptr, ptr + cap)` permanece vivo y exclusivo durante esta llamada JNI síncrona.
    let slice = unsafe { std::slice::from_raw_parts_mut(ptr, cap) };
    session()
        .lock()
        .step_gameplay_binary(dt_sec, lane_pressed_mask, lane_just_pressed_mask, slice)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSetGameplayPaused<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    paused: jboolean,
) -> JString<'local> {
    let json = session().lock().set_gameplay_paused(paused != 0);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeStopGameplay<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JString<'local> {
    let json = session().lock().stop_gameplay();
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeTriggerGameOver<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JString<'local> {
    let json = session().lock().trigger_game_over();
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeConfirmGameOverRetry<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JString<'local> {
    let json = session().lock().confirm_game_over_retry();
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeRestartGameplay<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JString<'local> {
    let json = session().lock().restart_gameplay();
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSetGameplayDebug<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    botplay: jboolean,
    practice_mode: jboolean,
) -> JString<'local> {
    let json = session()
        .lock()
        .set_gameplay_debug(botplay != 0, practice_mode != 0);
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeProbeGraphicsBackends<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
) -> JString<'local> {
    let json = session().lock().probe_graphics_backends_json();
    to_jstr(&mut env, &json)
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_beatforge_studio_NativeEngineBridge_nativeSetPreferredRenderBackend<'local>(
    mut env: JNIEnv<'local>,
    _cls: JClass<'local>,
    backend_key: JString<'local>,
) -> JString<'local> {
    let key = read_str(&mut env, &backend_key);
    let json = session().lock().set_preferred_render_backend(&key);
    to_jstr(&mut env, &json)
}
