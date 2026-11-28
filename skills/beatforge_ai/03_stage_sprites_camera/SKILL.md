---
name: beatforge-stage-sprites-camera
description: Modelo de escena 1280x720, entidades/sprites (roles, propiedades válidas, animaciones), multicámara (Cam 0..3+) y viñeta lateral en BeatForge Studio.
---

# Escenario, Sprites y Sistema Multicámara

## 1. Coordenadas y Entidades del Escenario
- El mundo virtual mide **1280 × 720**.
- Cada entidad (`StageEntity`) tiene:
  - `id` único (ej. `"stage_bg"`, `"opponent_main"`, `"player_main"`, `"prop_lights"`).
  - `displayName`: nombre visible para el usuario.
  - `role`: `"background"`, `"decoration"`, `"opponent"`, `"player"`.
  - Propiedades modificables en runtime con `setEntityProp(id, prop, value)` y `tweenEntityProp(tag, id, prop, target, durSec, ease)`:
    - `"x"` (centro X en píxeles virtuales)
    - `"y"` (centro Y en píxeles virtuales)
    - `"width"`, `"height"` (`>= 1.0`)
    - `"scale"` (`0.01..10.0`)
    - `"opacity"` o `"alpha"` (`0.0..1.0`)
    - `"angle"` / `"rotation"` / `"rotationDeg"` (grados)
    - `"visible"` (`1` = visible, `0` = oculto)
    - `"flipX"` (`1` = invertido horizontalmente, `0` = normal)
    - `"zIndex"` / `"z"` (orden de dibujo)
    - `"scrollFactor"` (`0.0..4.0`, parallax: `0` fijo, `1` normal)
- Lectura segura:
  - `getEntityX(id)` -> `number | nil`
  - `getEntityY(id)` -> `number | nil`
  - `getEntityProp(id, prop)` -> `number | nil`

## 2. Animaciones y Gestión de RAM por Timeline
- `playAnim(entityId, animName)` o `playEntityAnim(entityId, animName)`: reproduce `"idle"`, `"left"`, `"down"`, `"up"`, `"right"`, `"miss_left"`, etc., o el prefijo exacto de Sparrow XML.
- Para optimizar RAM en móviles (32-bit y 64-bit) durante cambios de fase:
  - `preloadEntity(entityId)`: precarga el `.webp` compilado en disco hacia RAM ~4 s antes de mostrarse.
  - `retireEntity(entityId)`: libera la textura de RAM tras ocultarse.
  - `swapEntity(oldId, newId, disposeOld)`: oculta `oldId`, muestra `newId` y opcionalmente libera `oldId`.
  - `setActiveCharacter(role, entityId, disposeOld)`: cambia qué entidad canta como `"player"` u `"opponent"` en tiempo real (útil en canciones con dúos o relevos).

## 3. Sistema Multicámara (`Cam 0..3+`) y Viñeta Lateral
- **Ranuras base**:
  - `Cam 0`: Fija / Encuadre general (`fixed`).
  - `Cam 1`: Jugador (`player`).
  - `Cam 2`: Enemigo / Rival (`opponent`).
  - `Cam 3+`: Cámaras personalizadas creadas por el usuario en el Editor de Escena.
- **APIs de Cámara**:
  - `switchCamera(slot, zoom?, dur?, ease?)`: cambia a `Cam 0..N` con transición opcional.
  - `getActiveCamera() -> int`
  - `setCameraPos(x, y)`, `tweenCameraPos(tag, x, y, durSec, ease)`
  - `setCameraZoom(zoom)`, `setCameraBaseZoom(zoom)`, `tweenCameraZoom(tag, zoom, durSec, ease)`
  - `getCameraX()`, `getCameraY()`, `getCameraZoom()`, `getCameraBaseZoom()`
  - `setCameraFollowPos(x, y)` / `clearCameraFollowPos()`: fija o libera el objetivo manual de seguimiento.
  - `setCameraClamp(enabled)`: limita o libera los bordes respecto al fondo.
  - `shakeCamera(intensity)`
- **Viñeta / Brillo Lateral de Cámara**:
  - `setCameraVignette(colorHex, alpha?, width?, pulse?, overHud?)`
  - `tweenCameraVignette(tag, targetAlpha, dur?, ease?, colorHex?, width?, pulse?, overHud?)`
  - `clearCameraVignette(dur?, ease?)`
  - `getCameraVignetteAlpha() -> number`

## 4. Flujo Recomendado para el Asistente IA
Cuando el usuario diga *"haz que el sprite X haga Y en tal parte de la canción"*:
1. Llama a `list_stage_sprites` para obtener los `id` exactos, sus posiciones `(x, y)`, escalas, visibilidad y animaciones disponibles (sin imágenes).
2. Llama a `get_chart_timing_info` para conocer el BPM, duración, secciones (`index`, `startMs`, `bpm`, `mustHitSection`) y eventos existentes en el chart.
3. Lee el script relevante (`read_script`) —por ejemplo `stage_events.lua` o un script custom— y aplica un cambio quirúrgico con `patch_script` (o crea uno nuevo con `create_script` si procede).
