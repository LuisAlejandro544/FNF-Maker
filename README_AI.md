# README_AI — Guía Operativa para Trabajar con IA en BeatForge Studio

Este documento resume cómo retomar, depurar, extender y validar **BeatForge Studio** usando flujos estructurados asistidos por IA directamente desde el repositorio.

---

## 1. Mapa Rápido de Contexto para Prompts

Cuando pidas cambios o mejoras a una IA sobre este proyecto, indica siempre qué capa quieres tocar:

| Capa | Ruta | Lenguaje | Qué cubre |
| :--- | :--- | :--- | :--- |
| Núcleo del motor | `engine/crates/pulse_core` | Rust | `chart.json` completo/export (`mustHitSection`, `gfSection`, `cameraSlot`, `changeBPM`, notas especiales `Hurt`/`Instakill`/`Dodge`/`Alt`/`NoAnim`, parser **`parse_chart_camera_event`**), Sparrow `.xml` (`page_index`) + `.json` Animate/TexturePacker (**centrado universal**, `Miss` + `Death`), `.astc` + **dithering `RGB_565` Floyd-Steinberg + cuantización `exoquant` + WebP VP8L** (`astc_loader.rs`), **`mod_descriptors.rs`**, escenario + **multicámara (`Cam 0..3+` + clamping)**, `svg_skin.rs`, `gfx_backend.rs`, `script_optimizer.rs`, **`atlas_repack.rs`** (`crunch`, alpha-trim, frames muertos, dedup exacta, poda segura y **paginación multi-página `_p1.._p5.webp`**), **`hot_compile` v6** (**`Lanczos3`** por pantalla/zoom + **Plan Temporal `compute_timeline_asset_lifecycle`**), `disk_cache.rs` y **`vanilla.rs`** |
| Audio baja latencia | `engine/crates/pulse_audio` | Rust puro (`dlopen` AAudio) | Decodificación OGG/WAV/MP3 a `i16` compacto, caché binario `.bfaud` (`audio_cache.rs`) de PCM/ondas con LRU, fusión en caliente a WAV único (`compiled_mix.wav`), streaming SPSC lock-free de 512 KB (`streaming.rs`), **Adaptador Acústico (`adaptive_master.rs` con `ebur128`, `realfft`, `rustfft`, `biquad`, `rubato` y `probe_aaudio_hardware()`)**, cuenta regresiva BPM (`3, 2, 1, ¡YA!`) y SFX Game Over |
| Scripting Mods | `engine/crates/pulse_scripting` | Rust + Lua + TS + JS | `mlua` (Lua 5.4), `oxc` (TypeScript + `validate_js_syntax`), `rquickjs` (QuickJS-NG) con pre-compilación (`.luac`/`.js`), watchdog real (`250ms/25ms` + cuarentena), **multicámara (`switchCamera`, `getActiveCamera`, `setCameraFollowPos`, `clearCameraFollowPos`, `setCameraClamp`, `getGfSection`)**, flashes, Game Over (`triggerGameOver`/`isGameOver`/`onGameOver`), `noteType` en hit/miss, swaps/RAM y modcharts |
| Puente JNI & Sesión | `engine/crates/pulse_android` | Rust (`jni` 0.21) | `EngineSession` modular (`editor_ops`, `gfx_probe.rs`, `sprite_ops` con `page_index`, `persistence_ops`, `gameplay_ops` con **`hot_compile_ops.rs` (`viewport`)**, **`audio_adapt_ops.rs`**, colas **`timeline_preload_queue` / `timeline_retire_queue`**, reporte **`compiled_texture_pages` / `texture_pixel_formats` / `timeline_windows` / `repacked_entity_ids`**, multicámara + clamping + `chart.json`, Game Over, Botplay/Práctica, notas X), `HOT_COMPILE_PROGRESS`, `AUDIO_ADAPT_PROGRESS` y `BFGP` v2 (`152B` + bits `8..15` `page_index` en `e_flags`, bits `9..13` y `noteKindCode`) |
| Interfaz Android | `android/app/src/main/kotlin/com/beatforge/studio/` | Kotlin (Jetpack Compose) | `MainActivity` (Lobby + **`ProjectLoadingScreen`** + **`AudioAdaptScreen`**), **`audio/`** (`AndroidAudioDeviceProbe`), **`gfx/`**, `StudioViewModel` (`Dispatchers.IO`, cuota `StorageManager`, `StudioBitmapLoader` con **`Bitmap.Config.RGB_565`**, decodificación multi-página **`extraPages`/`pageAt()`** y reciclaje diferido completo `recycleLoadedEntityBitmap`), pantallas (`Hub`, `Projects`, `Audio`, `Chart`, `StageEditor`, `SpriteLab`, `Gameplay`) y popups |
| SDK y Pruebas | `sdk/` | TypeScript / SVG / Lua / JS | Contratos `.d.ts` completos, utilidades testeadas con `node:test` (**60 casos**: `chart.json`, `parseChartCameraEvent`, `resolveCameraSlotTarget`, clamping, countdown, `.astc`, `.json`, `Miss`, `low_32bit`, `BFGP` v2 `152B` + `pageIndex`, notas X, `matchArrowSparrowSubTexture`, `RenderBackendKind`, FNF HUD/splash, audio, cámara Lua V2, Idle BPM, caché incremental, WebP lossless, **`Lanczos3` viewport scaling**, **Adaptador DSP `fuseDeviceAndAAudioProbe` + `buildAdaptiveDspPlan`**, **dedup/trim/poda `optimizeSparrowAtlasPlan`**, **Timeline Lifecycle `computeTimelineAssetLifecycle`**, **dithering `RGB_565` + `exoquant`**, **paginación multi-página `planMultiPageSparrowSplit`**, watchdog, sustain, ciclo RAM, LRU/`.bfaud`, `.mod`, descriptores, centrado universal, auto-tamaño y Flip X), flechas originales |
| Compilación APK | `.github/workflows/build-apk.yml` | YAML | Workflow manual `workflow_dispatch` con caché y 2 artefactos Debug (`arm64-v8a` + `armeabi-v7a`) |

---

## 2. Comandos de Verificación Local vs Nube

### En el entorno local (Web + SDK TypeScript)
```bash
# 1. Ejecutar pruebas unitarias del SDK TypeScript
npx --yes tsx --test sdk/ts/src/engine-tools.test.ts

# 2. Verificar compilación del cascarón web ("Hello World")
npm run build

# 3. Verificar tipos TypeScript / lint
npm run lint
```

### En GitHub Actions (Compilación Nativa Android 32 y 64 bits)
- El entorno contenedor no incluye `cargo` ni Android NDK.
- Para validar cambios en `engine/` o `android/`, dispara manualmente el workflow:
  `Actions → Compilar BeatForge Studio APK (Debug Manual) → Run workflow`.

---

## 3. Flujo Recomendado de Prompts por Rol

1. **El Arquitecto (Planificar antes de editar)**:
   - Pide siempre identificar qué módulo Rust (`pulse_core`, `pulse_audio`, `pulse_scripting`, `pulse_android`) o pantalla Compose (`ui/screens/*`, `ui/popups/*`, `gfx/*`) debe asumir la nueva responsabilidad sin mezclar lógica de motor en Kotlin.
2. **El Constructor (Implementar completo)**:
   - Exige manejo de errores explícito (`Result<T, E>` en Rust, `ActionResult` en Kotlin), comentarios explicativos en español y verificación previa de firmas JNI entre `NativeEngineBridge.kt` y `pulse_android/src/lib.rs`.
3. **El Detective (Depurar logs de GitHub Actions)**:
   - Al analizar un error de compilación de Actions, revisa si ocurrió en `cargo ndk` (Rust 32/64 bits) o en `compileDebugKotlin` (Compose/Gradle) antes de relanzar el workflow.
4. **El Crítico y El Optimizador**:
   - Audita que no haya mutex bloqueantes en el callback AAudio (`pulse_audio/src/mixer.rs` y `streaming.rs`), ni copias RGBA duplicadas en Rust/Kotlin, ni recomposiciones 60Hz globales en `StageEditorScreen` (usa `StageLiveTransformState`), ni copias SAF/JNI pesadas en el hilo principal (`Dispatchers.IO`).
5. **El Escudo (Testing)**:
   - Añade tests puros en `pulse_core` / `pulse_audio` / `pulse_android` (`#[cfg(test)]`) y en `sdk/ts/src/engine-tools.test.ts` cada vez que cambien reglas de charts, notas especiales, Sparrow XML, audio streaming, sesiones o validadores.

---

## 4. Checklist Antes de Cerrar Cualquier Cambio

- [ ] `/src/App.tsx` sigue mostrando únicamente `Hello World`.
- [ ] No hay nombres comerciales o assets protegidos por copyright.
- [ ] Todas las dependencias nuevas son MIT / Apache-2.0 (nunca GPL/LGPL/MPL).
- [ ] Kotlin Compose no implementa fallbacks de motor; todo se resuelve vía JNI en Rust.
- [ ] `entityBitmaps` (editor) y `compiledBitmaps` (gameplay) nunca coexisten en RAM al mismo tiempo, y todo `.recycle()` de sprites retirados en gameplay usa la cola diferida de 3 frames.
- [ ] `StageLiveTransformState` mantiene el editor de escenario sin commits JNI por píxel y `BFGP` v2 mantiene sincronizado el formato `152B` entre Rust, Kotlin y `sdk/ts`.
- [ ] Se mantienen activas ambas arquitecturas: `arm64-v8a` (64 bits) y `armeabi-v7a` (32 bits).
- [ ] `.github/workflows/build-apk.yml` permanece exclusivamente en `workflow_dispatch`.
- [ ] Los tests de `sdk/ts/src/engine-tools.test.ts` y `npm run build` / `lint` pasan después del último cambio.
