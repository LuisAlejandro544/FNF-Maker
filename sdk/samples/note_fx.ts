// Archivo: sdk/samples/note_fx.ts
// Propósito: Script comunitario de ejemplo en TypeScript (transpilado por oxc en Rust).
/// <reference path="../ts/src/beatforge-sdk.d.ts" />

function onNoteHit(lane: number, isPlayer: boolean): void {
    const actor: string = isPlayer ? "player_main" : "opponent_main";
    if (lane === 2) {
        setEntityProp(actor, "scale", 1.08);
    } else {
        setEntityProp(actor, "scale", 1.0);
    }
}

function onNoteMiss(lane: number, isPlayer: boolean): void {
    if (isPlayer) {
        shakeCamera(6.0);
        log("Fallo en carril " + lane);
    }
}
