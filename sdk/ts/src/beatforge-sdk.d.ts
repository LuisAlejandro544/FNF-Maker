/**
 * Archivo: sdk/ts/src/beatforge-sdk.d.ts
 * Propósito: Declaraciones de tipos TypeScript para scripts de mods creados por
 * la comunidad o por el creador dentro de BeatForge Studio.
 * Todas estas funciones globales y callbacks de ciclo de vida son registrados
 * nativamente por `pulse_scripting` en Lua 5.4, QuickJS-NG (`rquickjs`) y TypeScript (`oxc`).
 */

type EntityPropName =
  | 'x'
  | 'y'
  | 'scale'
  | 'opacity'
  | 'alpha'
  | 'angle'
  | 'rotation'
  | 'visible'
  | 'flipX'
  | 'zIndex'
  | 'width'
  | 'height'
  | 'color';

type StrumPropName = 'x' | 'offsetX' | 'y' | 'offsetY' | 'angle' | 'rotation' | 'scale' | 'alpha' | 'opacity';

type EaseName =
  | 'linear'
  | 'sineIn'
  | 'sineOut'
  | 'sineInOut'
  | 'quadIn'
  | 'quadOut'
  | 'quadInOut'
  | 'cubicIn'
  | 'cubicOut'
  | 'cubicInOut'
  | 'backOut'
  | 'elasticOut'
  | 'bounceOut';

declare function log(message: string): void;

// Cámara
declare function setCameraZoom(zoom: number): void;
declare function setCameraBaseZoom(zoom: number): void;
declare function setCameraPos(x: number, y: number): void;
declare function getCameraX(): number;
declare function getCameraY(): number;
declare function getCameraZoom(): number;
declare function getCameraBaseZoom(): number;
declare function shakeCamera(intensity: number): void;
declare function switchCamera(slot: number, zoom?: number, durationSec?: number, ease?: EaseName): void;
declare function getActiveCamera(): number;
declare function setCameraFollowPos(x: number, y: number): void;
declare function clearCameraFollowPos(): void;
declare function tweenCameraPos(tag: string, targetX: number, targetY: number, durationSec: number, ease?: EaseName): void;
declare function tweenCameraZoom(tag: string, targetZoom: number, durationSec: number, ease?: EaseName): void;
declare function flashCamera(colorHex?: string, durationSec?: number, alpha?: number): void;
declare function flashHud(colorHex?: string, durationSec?: number, alpha?: number): void;
declare function flashEffect(colorHex?: string, durationSec?: number, alpha?: number, overHud?: boolean): void;

// Entidades, Swaps de Escenario, Memoria RAM y Animaciones
declare function preloadEntity(entityId: string): void;
declare function retireEntity(entityId: string): void;
declare function swapEntity(hideId: string, showId: string, disposeOld?: boolean): void;
declare function setActiveCharacter(role: 'player' | 'opponent', entityId: string, disposeOld?: boolean): void;
declare function setEntityProp(entityId: string, prop: EntityPropName, value: number | boolean | string): void;
declare function getEntityProp(entityId: string, prop: EntityPropName): number;
declare function getEntityX(entityIdOrRole: string): number | null;
declare function getEntityY(entityIdOrRole: string): number | null;
declare function playAnim(entityId: string, animName: string): void;
declare function playEntityAnim(entityId: string, animName: string): void;
declare function tweenEntityProp(
  tag: string,
  entityId: string,
  prop: 'x' | 'y' | 'scale' | 'opacity' | 'alpha' | 'angle' | 'rotation',
  targetValue: number,
  durationSec: number,
  ease?: EaseName
): void;

// Modchart de Flechas / Receptores (strums 0..3 Rival, 4..7 Jugador)
declare function setStrumProp(strumIndex: number, prop: StrumPropName, value: number): void;
declare function getStrumProp(strumIndex: number, prop: StrumPropName): number;
declare function tweenStrumProp(
  tag: string,
  strumIndex: number,
  prop: StrumPropName,
  targetValue: number,
  durationSec: number,
  ease?: EaseName
): void;

// Objetos Dinámicos (Mundo y HUD)
declare function makeGraphic(id: string, x: number, y: number, width: number, height: number, colorHex: string): void;
declare function makeHudGraphic(id: string, x: number, y: number, width: number, height: number, colorHex: string): void;
declare function makeText(id: string, text: string, x: number, y: number, size?: number, colorHex?: string): void;
declare function setTextString(id: string, text: string): void;
declare function cloneSprite(sourceId: string, newId: string, x?: number, y?: number): void;
declare function removeSprite(id: string): void;

// Temporizadores y Tweens
declare function cancelTween(tag: string): void;
declare function runTimer(tag: string, durationSec: number): void;
declare function cancelTimer(tag: string): void;

// Estado de Partida, Salud, Puntuación, HUD y Audio
declare function getSongPosMs(): number;
declare function getActiveBpm(): number;
declare function getMustHitSection(): boolean;
declare function getGfSection(): boolean;
declare function setHealth(health: number): void;
declare function addHealth(delta: number): void;
declare function getHealth(): number;
declare function setScore(score: number): void;
declare function addScore(delta: number): void;
declare function getCombo(): number;
declare function getMisses(): number;
declare function getHits(): number;
declare function getAccuracy(): number;
declare function setScrollSpeed(speed: number): void;
declare function getScrollSpeed(): number;
declare function setDownscroll(downscroll: boolean): void;
declare function setHudVisible(visible: boolean): void;
declare function getHudVisible(): boolean;
declare function setHudOpacity(opacity: number): void;
declare function getHudOpacity(): number;
declare function setHudZoom(zoom: number): void;
declare function getHudZoom(): number;
declare function tweenHudProp(
  tag: string,
  prop: 'opacity' | 'alpha' | 'zoom' | 'scale',
  targetValue: number,
  durationSec: number,
  ease?: EaseName
): void;
declare function tweenHudOpacity(tag: string, targetOpacity: number, durationSec: number, ease?: EaseName): void;
declare function setArrowsVisible(visible: boolean): void;
declare function getArrowsVisible(): boolean;
declare function setArrowsOpacity(opacity: number): void;
declare function getArrowsOpacity(): number;
declare function tweenArrowsOpacity(tag: string, targetOpacity: number, durationSec: number, ease?: EaseName): void;
declare function setScreenBlackout(alpha: number): void;
declare function getScreenBlackout(): number;
declare function tweenScreenBlackout(tag: string, targetAlpha: number, durationSec: number, ease?: EaseName): void;
declare function setTrackVolume(slotIndex: number, volume: number): void;
declare function setTrackMute(slotIndex: number, muted: boolean): void;

// Estado Game Over / Muerte estilo FNF
declare function triggerGameOver(): void;
declare function isGameOver(): boolean;

// Callbacks de ciclo de vida del Mod
declare function onCreate(): void;
declare function onSongStart(): void;
declare function onUpdate(dt: number): void;
declare function onBeatHit(beat: number): void;
declare function onStepHit(step: number): void;
declare function onCountdownTick(stage: number): void;
declare function onNoteHit(lane: number, isPlayer: boolean, noteType?: string): void;
declare function onNoteMiss(lane: number, isPlayer: boolean, noteType?: string): void;
declare function onGameOver(): void;
declare function onEvent(name: string, value1: string, value2: string): void;
declare function onTweenCompleted(tag: string): void;
declare function onTimerCompleted(tag: string): void;
