//! Archivo: engine/crates/pulse_android/src/jni/mod.rs
//! Propósito: Punto de entrada del módulo JNI para Android (`libpulse_android.so`).
//! Agrupa por dominio las exportaciones `Java_com_beatforge_studio_NativeEngineBridge_*`
//! y proporciona utilidades compartidas para leer y construir cadenas `JString`.

pub mod audio_ops;
pub mod chart_ops;
pub mod gameplay_gfx_ops;
pub mod project_ops;
pub mod script_skin_ops;
pub mod stage_sprite_ops;

use jni::JNIEnv;
use jni::objects::JString;

pub(super) fn read_str(env: &mut JNIEnv, input: &JString) -> String {
    env.get_string(input)
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub(super) fn to_jstr<'local>(env: &mut JNIEnv<'local>, s: &str) -> JString<'local> {
    env.new_string(s).unwrap_or_default()
}
