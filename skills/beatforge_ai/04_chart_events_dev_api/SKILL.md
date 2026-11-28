---
name: beatforge-chart-events-dev-api
description: Convenciones de tiempo (ms, beats, steps, secciones), eventos de chart.json, cuenta regresiva 2-1-¡YA! y sistema de versionado API de desarrollo de BeatForge Studio.
---

# Chart, Eventos Temporales y API de Desarrollo

## 1. Tiempo, Beats, Steps y Secciones
- `getSongPosMs()` devuelve la posición actual de reproducción en milisegundos (`>= 0` durante la canción, `< 0` durante la cuenta regresiva).
- La **cuenta regresiva** consta de **3 pasos**:
  - `0` -> `"2"` (Ready?)
  - `1` -> `"1"` (Set?)
  - `2` -> `"¡YA!"` (Go!)
- Para sincronizar efectos con secciones o beats concretos:
  - Usa `onBeatHit(beat)` o `onStepHit(step)` cuando la canción tenga BPM fijo o para cadencias rítmicas simples (`if beat == 32 then ... end`).
  - Usa `onEvent(name, value1, value2)` cuando el usuario coloque un cue exacto en el **Editor de Chart**.
  - Si hay cambios de BPM en `chart.json`, usa `get_chart_timing_info` para saber el `startMs` exacto de cada sección y poder comparar `getSongPosMs() >= startMs` o mapear el beat/step correcto.

## 2. Eventos Integrados en `stage_events.lua` y `ui_blackout.lua`
El motor sincroniza automáticamente marcadores protegidos dentro de los scripts del sistema:
- `camera_system.lua`: bloque `-- [CAM_CFG_START] .. -- [CAM_CFG_END]`.
  - Variables clave: `CAM_SCRIPT_OVERRIDE` (`false` por defecto; pon `true` solo si el script controla manualmente la cámara).
- `stage_events.lua`: bloque `-- [STAGE_CUES_START] .. -- [STAGE_CUES_END]`.
- `ui_blackout.lua`: bloque `-- [UI_BLACKOUT_START] .. -- [UI_BLACKOUT_END]`.
**Regla importante**: Al editar scripts del sistema con `patch_script`, **nunca borres ni corrompas los marcadores `-- [CAM_CFG_START]`, `-- [STAGE_CUES_START]` o `-- [UI_BLACKOUT_START]`**; agrega tu lógica personalizada fuera de esos bloques o en un script propio para que los editores visuales no sobrescriban tu código.

## 3. API de Desarrollo (`devApiVersion`)
BeatForge Studio versiona sus capacidades mediante `devApiVersion` en `project.json`:
- **v1 (Base Inicial)**: Scripts Lua 5.4 / TS / JS básicos, cámara estándar, cuenta regresiva inicial y chart básico.
- **v2 (Actual — Scripting Pro, Multicámara & Chart Cues)**:
  - Multicámara `Cam 0..3+` (`switchCamera`, `setCameraFollowPos`, `setCameraClamp`) y viñeta lateral animable (`setCameraVignette` / `tweenCameraVignette`).
  - Timeline cues avanzados (`stage_events.lua`, `ui_blackout.lua`), `preloadEntity` / `retireEntity` / `swapEntity` / `setActiveCharacter`.
  - Modcharts de strums (`setStrumProp` / `tweenStrumProp`), objetos dinámicos (`makeGraphic`, `makeHudGraphic`, `makeText`), tweens y timers nativos.
  - Cuenta regresiva sincronizada de 3 pasos (`2 -> 1 -> ¡YA!`) y notas especiales (`hurt`, `instakill`, `dodge`, `alt`, `no_anim`).
  - Editor de código con numeración de líneas y Asistente IA con Skills modulares.
- Todo proyecto nuevo nace en la última versión (`v2`); los proyectos antiguos conservan su versión (`v1`) hasta que el usuario decide migrarlos explícitamente desde el Hub.
