//! Archivo: engine/crates/pulse_core/src/script_optimizer/tests.rs
//! Propósito: Pruebas unitarias del optimizador de bucles para TypeScript, JavaScript y Lua 5.4.

use super::*;

#[test]
fn discards_useless_heavy_js_loop_and_keeps_small_loop() {
    let ts = r#"
function onUpdate(dt: number) {
    let acc = 0;
    for (let i = 0; i < 1000000; i++) {
        acc += i;
    }
    for (let lane = 0; lane < 4; lane++) {
        setEntityProp("player_main", "opacity", 1.0);
    }
}
"#;
    let res = optimize_single_script(ScriptLanguage::TypeScript, ts);
    assert_eq!(res.discarded_loops, 1);
    assert!(!res.optimized_source.contains("1000000"));
    assert!(res.optimized_source.contains("lane < 4"));
    assert!(res.optimized_source.contains("setEntityProp"));
}

#[test]
fn neutralizes_infinite_loop_with_necessary_engine_logic() {
    let js = r#"
function onUpdate(dt) {
    while (true) {
        setCameraZoom(1.08);
    }
}
"#;
    let res = optimize_single_script(ScriptLanguage::JavaScript, js);
    assert_eq!(res.neutralized_loops, 1);
    assert!(!res.optimized_source.contains("while (true)"));
    assert!(res.optimized_source.contains("setCameraZoom(1.08)"));
}

#[test]
fn discards_useless_lua_loop_and_caps_heavy_necessary_lua_loop() {
    let lua = r#"
function onUpdate(dt)
    local x = 0
    while true do
        x = x + 1
    end
    for i = 1, 50000 do
        setEntityProp("stage_bg", "opacity", 0.9)
    end
end
"#;
    let res = optimize_single_script(ScriptLanguage::Lua, lua);
    assert_eq!(res.discarded_loops, 1);
    assert_eq!(res.neutralized_loops, 1);
    assert!(!res.optimized_source.contains("while true do"));
    assert!(res.optimized_source.contains("1, 64"));
    assert!(res.optimized_source.contains("setEntityProp"));
}
