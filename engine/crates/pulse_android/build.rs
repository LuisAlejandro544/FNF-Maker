//! Archivo: engine/crates/pulse_android/build.rs
//! Propósito: Script de compilación de Cargo para `pulse_android`.
//! La librería resultante (`libpulse_android.so`) se enlaza vía JNI puro con Kotlin Compose
//! y utiliza AAudio en Rust puro (`dlopen` C ABI), sin C++ Oboe ni `GameActivity`.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
}
