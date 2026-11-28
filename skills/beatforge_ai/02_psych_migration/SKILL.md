---
name: beatforge-psych-migration
description: Guía exacta para traducir y adaptar scripts Lua de Psych Engine / FNF clásicos al motor BeatForge Studio sin errores ni APIs inexistentes.
---

# Traducción de Mods Psych Engine -> BeatForge Studio

Cuando el usuario pegue o importe un script de **Psych Engine** (u otro motor FNF), **NO existe `doTweenX`, `setProperty`, `makeLuaSprite` de archivo externo ni `curBeat`/`curStep` globales mágicos** con la misma firma. Tradúcelos usando esta tabla de equivalencias exactas:

## 1. Callbacks y Variables Globales
| Psych Engine | BeatForge Studio | Notas |
|---|---|---|
| `curBeat` | Guardar parámetro de `onBeatHit(beat)` o calcular con `getSongPosMs()` y `getActiveBpm()` | No existe global `curBeat` |
| `curStep` | Guardar parámetro de `onStepHit(step)` | `onStepHit(step)` recibe el entero exacto |
| `mustHitSection` | `getMustHitSection()` | Devuelve booleano real |
| `gfSection` | `getGfSection()` | Devuelve booleano real |
| `songPosition` / `getSongPosition()` | `getSongPosMs()` | En milisegundos |
| `curBpm` | `getActiveBpm()` | BPM activo por sección |
| `downscroll` | `getDownscroll()` | Booleano |
| `scrollSpeed` | `getScrollSpeed()` | Número |
| `health` | `getHealth()` (`0.0..1.0`) | Ojo: en Psych va de `0..2`, **en BeatForge va de `0.0..1.0`!** Divide valores entre 2 |
| `goodNoteHit(id, dir, noteType, isSustain)` | `onNoteHit(lane, isPlayer, noteType)` | `isPlayer = true` |
| `opponentNoteHit(id, dir, noteType, isSustain)` | `onNoteHit(lane, isPlayer, noteType)` | `isPlayer = false` |
| `noteMiss(id, dir, noteType, isSustain)` | `onNoteMiss(lane, isPlayer, noteType)` | En BeatForge incluye `isPlayer` |
| `onCreatePost()` | `onCreate()` | BeatForge inicializa todo antes de `onCreate()` |
| `onCountdownTick(counter)` | `onCountdownTick(stage)` | **3 pasos:** `0`="2", `1`="1", `2`="¡YA!" |

## 2. Propiedades de Personajes y Sprites del Escenario
En Psych se usa `setProperty('boyfriend.alpha', 0)` o `setProperty('dad.x', 200)`.
En BeatForge **siempre consulta `list_stage_sprites` primero** para saber el `id` real (por ejemplo `"player_main"`, `"opponent_main"`, `"stage_bg"`, o IDs propios del mod).

| Psych Engine | BeatForge Studio |
|---|---|
| `setProperty('boyfriend.x', val)` | `setEntityProp('player_main', 'x', val)` |
| `setProperty('dad.alpha', 0.5)` | `setEntityProp('opponent_main', 'opacity', 0.5)` |
| `setProperty('mySprite.visible', false)` | `setEntityProp('mySprite', 'visible', 0)` (`1` visible, `0` oculto) |
| `setProperty('mySprite.flipX', true)` | `setEntityProp('mySprite', 'flipX', 1)` |
| `setProperty('mySprite.angle', 45)` | `setEntityProp('mySprite', 'angle', 45)` |
| `getProperty('boyfriend.x')` | `getEntityX('player_main')` o `getEntityProp('player_main', 'x')` |
| `characterPlayAnim('dad', 'hey', true)` / `playAnim(...)` | `playAnim('opponent_main', 'hey')` o `playEntityAnim(id, anim)` |

## 3. Tweens y Modcharts de Flechas (Strums)
| Psych Engine | BeatForge Studio |
|---|---|
| `doTweenX('t', 'boyfriend', 900, 0.5, 'quadOut')` | `tweenEntityProp('t', 'player_main', 'x', 900, 0.5, 'quadOut')` |
| `doTweenAlpha('t', 'camHUD', 0, 0.5, 'linear')` | `tweenHudOpacity('t', 0, 0.5, 'linear')` |
| `setProperty('camHUD.visible', false)` | `setHudVisible(false)` |
| `setProperty('camHUD.zoom', 1.1)` | `setHudZoom(1.1)` |
| `noteTweenX('t', 4, 800, 0.4, 'sineOut')` | `tweenStrumProp('t', 4, 'x', 800, 0.4, 'sineOut')` |
| `noteTweenY('t', 0, -40, 0.4, 'sineOut')` | `tweenStrumProp('t', 0, 'y', -40, 0.4, 'sineOut')` |
| `noteTweenAngle('t', 5, 360, 0.5, 'linear')` | `tweenStrumProp('t', 5, 'angle', 360, 0.5, 'linear')` |
| `noteTweenAlpha('t', 1, 0.2, 0.3, 'linear')` | `tweenStrumProp('t', 1, 'alpha', 0.2, 0.3, 'linear')` |

Nota sobre Strums: En Psych los strums del oponente son `0..3` y los del jugador `4..7`. En BeatForge es igual (`0..3` Rival, `4..7` Jugador), pero `x`/`y` en `setStrumProp`/`tweenStrumProp` son **offsets relativos** respecto a su posición base (`0, 0` es la posición normal).

## 4. Cámara, Flashes y Objetos HUD
| Psych Engine | BeatForge Studio |
|---|---|
| `setProperty('defaultCamZoom', 1.1)` | `setCameraBaseZoom(1.1)` |
| `setProperty('camGame.zoom', 1.2)` | `setCameraZoom(1.2)` |
| `cameraShake('game', 0.01, 0.2)` | `shakeCamera(6.0)` (en píxeles virtuales) |
| `cameraFlash('camGame', 'FFFFFF', 0.5)` | `flashCamera('#FFFFFF', 0.5)` |
| `cameraFlash('camHUD', 'FF0000', 0.4)` | `flashHud('#FF0000', 0.4)` |
| `makeLuaSprite('bar', '', 0, 0)` + `makeGraphic(...)` | `makeGraphic('bar', x, y, w, h, '#000000')` o `makeHudGraphic(...)` |
| `makeLuaText('txt', 'Hola', 0, 100, 50)` | `makeText('txt', 'Hola', 100, 50, 22, '#FFFFFF')` |
| `setTextString('txt', 'Nuevo')` | `setTextString('txt', 'Nuevo')` |

## 5. Buenas Prácticas al Portear
1. **No uses `makeLuaSprite` con rutas de PNG en disco desde Lua**: en BeatForge las texturas se precargan e importan en **Escenario / Sprite Lab** para compilarse a `.webp` lossless. Si el script intenta cargar sprites nuevos, pide/crea la entidad en Stage Editor o usa `preloadEntity(id)` / `swapEntity(oldId, newId, disposeOld)`.
2. **Respeta `camera_system.lua`**: si el proyecto ya tiene `camera_system.lua`, no dupliques lógica conflictiva de seguimiento de cámara; coordina con `CAM_SCRIPT_OVERRIDE` o escribe efectos puntuales en un script aparte (`stage_events.lua` o un script custom).
3. **Valida siempre**: tras editar con `patch_script`, el sistema puede validar la sintaxis con Rust.
