// Archivo: sdk/samples/stage_pulse.js
// Propósito: Script comunitario de ejemplo en JavaScript ES2022 (ejecutado en QuickJS-NG).

function onUpdate(dt) {
    const t = getSongPosMs() / 1000.0;
    const wave = 0.85 + 0.15 * Math.sin(t * 3.0);
    setEntityProp("stage_speakers", "opacity", wave);
}
