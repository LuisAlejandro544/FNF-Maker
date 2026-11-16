//! Archivo: engine/crates/pulse_android/build.rs
//! Propósito: Script de compilación de Cargo. En Android enlaza explícitamente `libc++_shared`
//! requerido por Google Oboe (`liboboe` C++) en arquitecturas de 32 bits y 64 bits.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-link-lib=c++_shared");
    }
}
