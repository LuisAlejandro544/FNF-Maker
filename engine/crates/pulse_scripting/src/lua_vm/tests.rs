//! Archivo: engine/crates/pulse_scripting/src/lua_vm/tests.rs
//! Propósito: Pruebas unitarias de precompilación de bytecode Lua 5.4 y del watchdog
//! de interrupción ante bucles infinitos.

use super::*;

#[test]
fn lua_bytecode_precompile_loads_and_runs_cleanly() {
    let src = r#"
        function onBeatHit(beat)
            setCameraZoom(1.0 + beat * 0.01)
        end
    "#;
    let bytecode = LuaScriptVm::precompile_bytecode(src).expect("precompilación Lua");
    assert!(!bytecode.is_empty());

    let host = Arc::new(ScriptHostState::default());
    let mut vm = LuaScriptVm::new_with_bytecode("test.lua", src, Some(&bytecode), Arc::clone(&host))
        .expect("carga de bytecode");
    vm.call_i32("onBeatHit", 4);
    assert!((*host.camera_zoom.lock() - 1.04).abs() < 0.001);
}

#[test]
fn lua_watchdog_interrupts_infinite_loop_and_quarantines_after_three_trips() {
    let src = r#"
        function onUpdate(dt)
            while true do
            end
        end
    "#;
    let host = Arc::new(ScriptHostState::default());
    let mut vm = LuaScriptVm::new_with_bytecode("hang.lua", src, None, Arc::clone(&host))
        .expect("carga inicial válida");

    for _ in 0..3 {
        vm.call_f32("onUpdate", 0.016);
    }
    assert!(vm.is_quarantined());
    let errs = host.drain_runtime_errors();
    assert!(errs.iter().any(|e| e.contains("cuarentena")));
}
