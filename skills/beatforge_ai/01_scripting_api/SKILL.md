---
name: beatforge-scripting-api
description: Referencia completa de callbacks, VM (Lua 5.4, TypeScript oxc, JavaScript QuickJS-NG), Watchdog, Tweens, HUD, Audio y FX nativos de BeatForge Studio.
---

# BeatForge Studio — API Oficial de Scripting (Lua 5.4 / TS / JS)

Todos los scripts viven dentro del mod activo y se ejecutan en Rust nativo (`pulse_scripting`) sin JNI por frame:
- **Lua 5.4** (`mlua`, compilado a bytecode en Hot Compile).
- **TypeScript** (transpilado con `oxc` + ejecutado en QuickJS-NG).
- **JavaScript ES2022** (`rquickjs` / QuickJS-NG).

## 1. Callbacks del Ciclo de Vida
Define únicamente los callbacks que necesites:
- `onCreate()`: Se ejecuta una vez al cargar el gameplay. Presupuesto Watchdog: **250 ms**.
- `onSongStart()`: Al terminar la cuenta regresiva (`2 -> 1 -> ¡YA!`) e iniciar la música.
- `onUpdate(dt)`: En cada frame; `dt` está en segundos (`number`). Presupuesto Watchdog: **25 ms/frame** (tras 3 excesos entra en cuarentena).
- `onBeatHit(beat)`: Cada beat (`0, 1, 2...`).
- `onStepHit(step)`: Cada semicorchea / step (`0, 1, 2...`).
- `onCountdownTick(stage)`: `0 = "2 / Ready?"`, `1 = "1 / Set?"`, `2 = "¡YA! / Go"`.
- `onNoteHit(lane, isPlayer, noteType)`: `lane` es `0..3` (0=Izq, 1=Abajo, 2=Arriba, 3=Der), `isPlayer` (`boolean`), `noteType` (`"default"`, `"hurt"`, `"instakill"`, `"dodge"`, `"alt"`, `"no_anim"`).
- `onNoteMiss(lane, isPlayer, noteType)`: Al fallar nota o golpear una nota dañina.
- `onEvent(name, value1, value2)`: Eventos de `chart.json` disparados en tiempo real.
- `onTweenCompleted(tag)` y `onTimerCompleted(tag)`: Al finalizar un tween o temporizador.
- `onGameOver()`: Al llegar la vida a `0` o activar `triggerGameOver()`.

## 2. Lienzo Virtual y Reglas Críticas
- Resolución virtual fija: **1280 × 720** (`(0,0)` arriba-izquierda, `(640,360)` centro).
- **Nunca uses bucles `while true` ni bucles pesados** en `onUpdate(dt)`: el optimizador estático neutraliza bucles grandes (>64 iteraciones) y el Watchdog detiene scripts lentos.
- Para consultar qué sprites existen en la escena del usuario usa primero la herramienta `list_stage_sprites` (nunca inventes IDs de entidades).
- Prefiere `patch_script` sobre reescritura completa cuando modifiques un archivo existente.

## 3. Estado del Chart, Gameplay y Audio
- `getMustHitSection() -> boolean`, `getGfSection() -> boolean`
- `getActiveBpm() -> number`, `getSongPosMs() -> number`
- `getHealth() -> number` (`0.0..1.0`), `setHealth(value)`, `addHealth(delta)`
- `getScore() -> number`, `setScore(val)`, `addScore(delta)`
- `getCombo() -> int`, `getMisses() -> int`, `getHits() -> int`, `getAccuracy() -> number`
- `getScrollSpeed() -> number`, `setScrollSpeed(speed)` (`0.3..6.0`)
- `getDownscroll() -> boolean`, `setDownscroll(enabled)`
- `setTrackVolume(slot, gain)` (`slot`: `0`=Inst/Mix, `1`=Voces Player, `2`=Voces Rival; `gain`: `0.0..2.0`)
- `setTrackMute(slot, muted)`
- `triggerGameOver()`, `isGameOver() -> boolean`, `log(msg)`

## 4. HUD, Flechas y Pantalla en Negro (UI Blackout)
- `setHudVisible(visible)`, `getHudVisible() -> boolean`
- `setHudOpacity(alpha)`, `getHudOpacity() -> number` (`0..1`)
- `setHudZoom(zoom)`, `getHudZoom() -> number` (`0.4..2.5`)
- `setArrowsVisible(visible)`, `getArrowsVisible() -> boolean`
- `setArrowsOpacity(alpha)`, `getArrowsOpacity() -> number` (`0..1`)
- `setScreenBlackout(alpha)`, `getScreenBlackout() -> number` (`0..1`, cubre escenario detrás del countdown)
- Flashes:
  - `flashCamera(colorHex, durationSec)`
  - `flashHud(colorHex, durationSec)`
  - `flashEffect(colorHex, durationSec, startAlpha, overHud)` (colores hex `#RRGGBB`, `#AARRGGBB` o nombres `red`, `white`, `black`, `cyan`, etc.)

## 5. Tweens Nativos, Timers y Objetos Dinámicos
Easings válidos: `"linear"`, `"sineIn"`, `"sineOut"`, `"sineInOut"`, `"quadIn"`, `"quadOut"`, `"quadInOut"`, `"cubicIn"`, `"cubicOut"`, `"cubicInOut"`, `"backOut"`, `"elasticOut"`.
- `tweenEntityProp(tag, entityId, prop, target, durSec, ease)`
- `tweenCameraPos(tag, x, y, durSec, ease)`
- `tweenCameraZoom(tag, zoom, durSec, ease)`
- `tweenStrumProp(tag, strumIdx, prop, target, durSec, ease)` (`strumIdx`: `0..3` Rival, `4..7` Jugador; `prop`: `"x"`/`"offsetX"`, `"y"`/`"offsetY"`, `"angle"`/`"rotation"`, `"scale"`, `"alpha"`/`"opacity"`)
- `tweenHudProp(tag, prop, target, durSec, ease)` (`"opacity"`, `"zoom"`)
- `tweenHudOpacity(tag, target, durSec, ease)`
- `tweenArrowsOpacity(tag, target, durSec, ease)`
- `tweenScreenBlackout(tag, target, durSec, ease)`
- `cancelTween(tag)`, `runTimer(tag, durSec)`, `cancelTimer(tag)`
- Gráficos/textos dinámicos:
  - `makeGraphic(id, x, y, width, height, colorHex)` (en mundo)
  - `makeHudGraphic(id, x, y, width, height, colorHex)` (sobre HUD)
  - `makeText(id, text, x, y, fontSize, colorHex)`
  - `setTextString(id, text)`
  - `cloneSprite(id, sourceId, x, y)`
  - `removeSprite(id)`
