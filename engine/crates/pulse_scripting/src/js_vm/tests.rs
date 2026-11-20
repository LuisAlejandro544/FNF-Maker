//! Archivo: engine/crates/pulse_scripting/src/js_vm/tests.rs
//! Propósito: Pruebas unitarias del watchdog e interrupción de bucles infinitos en QuickJS.

use super::*;

#[test]
fn quickjs_watchdog_interrupts_infinite_loop_and_quarantines() {
    let js = r#"
        function onUpdate(dt) {
            while (true) {}
        }
    "#;
    let host = Arc::new(ScriptHostState::default());
    let mut vm = JsScriptVm::new_named("loop.js", js, Arc::clone(&host))
        .expect("carga inicial válida");

    for _ in 0..3 {
        vm.call_f32("onUpdate", 0.016);
    }
    assert!(vm.is_quarantined());
    let errs = host.drain_runtime_errors();
    assert!(errs.iter().any(|e| e.contains("cuarentena")));
}
