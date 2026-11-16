//! Archivo: engine/crates/pulse_android/build.rs
//! Propósito: Script de compilación de Cargo para `pulse_android`.
//! El subsistema de audio ya es 100% Rust puro (AAudio C ABI vía `dlopen`, sin Oboe C++).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
}
