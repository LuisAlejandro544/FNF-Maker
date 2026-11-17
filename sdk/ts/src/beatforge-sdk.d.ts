/**
 * Archivo: sdk/ts/src/beatforge-sdk.d.ts
 * Propósito: Declaraciones de tipos TypeScript para scripts de mods creados por
 * la comunidad o por el creador dentro de BeatForge Studio.
 * Todas estas funciones globales son registradas nativamente por `pulse_scripting`
 * tanto en Lua 5.4 como en QuickJS-NG (`rquickjs` + `oxc_transformer`).
 */

declare function log(message: string): void;
declare function setCameraZoom(zoom: number): void;
declare function setCameraBaseZoom(zoom: number): void;
declare function setCameraPos(x: number, y: number): void;
declare function getCameraX(): number;
declare function getCameraY(): number;
declare function getCameraZoom(): number;
declare function getCameraBaseZoom(): number;
declare function shakeCamera(intensity: number): void;
declare function setEntityProp(entityId: string, prop: 'x' | 'y' | 'scale' | 'opacity', value: number): void;
declare function getEntityX(entityIdOrRole: string): number | null;
declare function getEntityY(entityIdOrRole: string): number | null;
declare function playAnim(entityId: string, animName: string): void;
declare function playEntityAnim(entityId: string, animName: string): void;
declare function getSongPosMs(): number;
declare function getActiveBpm(): number;
declare function getMustHitSection(): boolean;

declare function onCreate(): void;
declare function onUpdate(dt: number): void;
declare function onBeatHit(beat: number): void;
declare function onNoteHit(lane: number, isPlayer: boolean): void;
declare function onNoteMiss(lane: number, isPlayer: boolean): void;
