//! Archivo: engine/crates/pulse_android/build.rs
//! Propósito: Script de compilación de Cargo para `pulse_android`.
//! Mantiene la alineación con `c++_static` usado por `android-activity` y `oboe`
//! evitando mezclar `libc++_static` y `libc++_shared` en `libpulse_android.so`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
}
