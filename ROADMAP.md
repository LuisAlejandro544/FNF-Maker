# Roadmap — BeatForge Studio

Hoja de ruta técnica y de producto para **BeatForge Studio** (`com.beatforge.studio`).

---

## Fase 1 — Núcleo Rust Modular, Audio AAudio Puro, UI Jetpack Compose y CI Manual 32/64 bits (Actual)

- [x] **Workspace Rust modular (`/engine`)**:
  - `pulse_core`: Parser y exportador completo de `chart.json` (`player1`, `player2`, `gfVersion`, `stage`, `events`, `changeBPM`, `mustHitSection`, `gfSection`, `cameraSlot`, `-1` inline, **`parse_chart_camera_event`** para `FocusCamera`/`ZoomCamera`/`Camera Follow Pos`/`Add Camera Zoom`/`Switch Camera`/`Camera Movement` y **`NoteSpecialKind`** para notas normales, `hurt` X, `instakill` X, `dodge`, `alt` y `no_anim`), decodificador **`.astc`** (`texture2ddecoder`) y codificador **WebP sin pérdida (VP8L)** puro Rust (`save_lossless_webp`), ensamblador Sparrow `.xml` + atlas **`.json`** (`TexturePacker`/`Aseprite`, `spritemap1.json`, `Animation.json` con poses `Idle`, direcciones, `Miss` y **`DeathStart` / `DeathLoop` / `DeathConfirm`** + **`normalize_and_center_animations()`** por defecto), **`mod_descriptors.rs`** (descriptores `.json`/`.xml` de personaje y escenario Psych/V-Slice/Codename), escena `1280×720` con **sistema multicámara (`Cam 0` Fija inicial, `Cam 1` Jugador, `Cam 2` Enemigo, `Cam 3+` personalizadas)**, auto-dimensionado inteligente (`fit_dimensions_to_atlas` / `fit_dimensions_to_image`), Z-order, cámara bidireccional (`camera_system.lua` V2), plantilla sincronizada `stage_events.lua`, **`svg_skin.rs`** (`resvg` + `hurt_x.svg` + `import_arrow_sparrow_atlas` para `NOTE_assets.png + .xml`), **`gfx_backend.rs`** (tipos `RenderBackendKind` y `GfxCapabilities` para Canvas2D / GLES2 / GLES3 / Vulkan), `script_optimizer.rs`, `hot_compile.rs`, `disk_cache.rs` (65%/35% LRU) y **`vanilla.rs`** (contenedor portátil **Vanilla** `.mod` + **Zstandard**).
  - `pulse_audio`: Mezclador lock-free de hasta 3 pistas sincronizado por muestras en **Rust puro** (`libaaudio.so` vía `dlopen`, cero C++), decodificadores permisivos (`lewton`, `hound`, `rmp3`) en **PCM `i16` compacto**, **caché binario `.bfaud` (`audio_cache.rs`)** en `cacheDir/audio_cache/` con escritura atómica y LRU, **fusión en caliente de 1–3 pistas a WAV único + streaming SPSC de 512 KB** (`streaming.rs`), **cuenta regresiva BPM ("3, 2, 1, ¡YA!")** y **síntesis Game Over (`DeathStart` / `DeathConfirm`)** lock-free.
  - `pulse_scripting`: VM **Lua 5.4** (`mlua`), transpilador **TypeScript** nativo en móvil (`oxc 0.158`) y VM **JavaScript ES2022** (`rquickjs`) **sin ataduras**, con **pre-compilación en caché (`.luac` / `.js`)** y **watchdog real en runtime** (`HookTriggers::every_nth_instruction(4096)` / `set_interrupt_handler` 250ms/25ms + cuarentena), exponiendo API de **multicámara (`switchCamera`, `getActiveCamera`, `setCameraFollowPos`, `clearCameraFollowPos`, `getGfSection`)**, modcharts, flashes, RAM, **`triggerGameOver()` / `isGameOver()` / `onGameOver()`** y `noteType` en `onNoteHit` / `onNoteMiss`.
  - `pulse_android`: Puente JNI (`EngineSession`) modularizado (`editor_ops` con gestión de ranuras `Cam 0..3+` y `section.cameraSlot`, `sprite_ops` con auto-escalado proporcional al importar imagen/atlas, `persistence_ops` con normalización automática de atlas al cargar proyectos, **`gfx_probe.rs`** por `dlopen`, `gameplay_ops` con multicámara y eventos directos de `chart.json`) + estado lock-free **`HOT_COMPILE_PROGRESS`**, staging temporal en `cacheDir`, adopción en `filesDir/projects/<id>/assets/`, caché multi-proyecto LRU `compiled_cache/<projectId>/`, precarga `4000 ms`, **`GameOver` completo (`DeathStart -> DeathLoop -> DeathConfirm`)**, **Botplay / Modo Práctica**, notas especiales `Hurt`/`Instakill`/`Dodge`/`Alt`/`NoAnim` y paquete binario **`BFGP` v2 (`152B`)** extendido con bits `9..13` (`gameOver`, `gameOverPhase`, `botplay`, `practiceMode`) y bits `4..7` de `noteFlags` (`noteKindCode`).
- [x] **Interfaz Android Nativa en Kotlin + Jetpack Compose (`/android`)**:
  - `ComponentActivity` + Material 3 + Material Icons Extended + `StudioViewModel` sobre `Dispatchers.IO`, `SafFileBridge`, cuota `StorageManager.getCacheQuotaBytes` (API 26+), sondeo `hotCompileProgress`, `BitmapRegionDecoder` 1:1 (`LruCache` 20 MB), carga perezosa + reciclaje diferido a 3 frames, y módulo **`gfx/`** (`RenderBackendModels.kt`, `VulkanProbeBridge.kt`, `GlesSurfaceHost.kt`) + `<uses-feature android:required="false" />` para OpenGL ES 2.0 y Vulkan en `AndroidManifest.xml`.
  - **Flujo de Entrada por Lobby y Carga**: Arranca en **`ProjectsScreen`** (lobby principal sin barra lateral), pasa por **`ProjectLoadingScreen`** mientras prepara el proyecto y permite regresar al menú principal cuando quieras; además incluye **`HubScreen`** (con selector **`Motor Gráfico (Terreno GPU)`**), **`ChartScreen`** (selector de notas especiales `Normal / Hurt X / Instakill X / Dodge / Alt / NoAnim`, selector `Cam N` por sección, hilo 60 Hz y columna `#`), **`AudioScreen`**, **`SpriteLabScreen`** (slots `Death Start / Loop / Confirm`, `Posición (.json/.xml)`, `Offset Pose X/Y`), **`StageEditorScreen`** (**sistema multicámara `Cam 0..3+`** con pestañas `Ranura & FOV` y `Modo & Chart` + marcos visuales de referencia), **`ScriptStudioScreen`**, **`HotCompileScreen`**, popups (**`ArrowSkinDialog`** con importador 2 pasos `NOTE_assets.png + .xml` + vista `"Nota X"`, `CharacterIconsDialog`, **`ChartStepCueDialog`** con pestaña `"Cámara"`, `ModFilesGuideDialog`) y **`GameplayScreen`** (Game Over interactivo con toque en cualquier parte, HUD limpio grabable, y **Menú de Pausa Debug** con **Monitor de RAM en vivo**, **FPS/Telemetría**, **Botplay**, **Práctica**, **Ocultar HUD** y **Probar Muerte**).
  - `minSdk 26` (Android 8.0+), `compileSdk 35`, `targetSdk 35`, `ndkVersion = "27.2.12479018"`.
  - Integración obligatoria de `cargo-ndk` en `preBuild` generando 2 APKs separados (`arm64-v8a` 64 bits y `armeabi-v7a` 32 bits).
- [x] **SDK & Pruebas (`/sdk`)**:
  - Definiciones `.d.ts` (`switchCamera`, `getActiveCamera`, `setCameraFollowPos`, `clearCameraFollowPos`, `getGfSection`, `triggerGameOver`, `isGameOver`, `onGameOver`, `noteType`), scripts de muestra (`.lua`, `.ts`, `.js`), flechas SVG originales (incluyendo `hurt_x.svg`) y suite de pruebas unitarias TypeScript (`tsx --test`, **50 casos**) cubriendo `chart.json` (`cameraSlot`), `parseChartCameraEvent`, `resolveCameraSlotTarget` multicámara, countdown BPM, `.astc`, `.json` sprites, `Miss`, `low_32bit`, `BFGP` v2 (`152B` + bits `gameOver`/`botplay`/`practiceMode`/`noteKindCode`), notas especiales X, `matchArrowSparrowSubTexture`, `RenderBackendKind`, FNF HUD / `Sick` splash, streaming audio, cámara Lua V2, Idle BPM, caché incremental, `.webp` lossless, watchdog, sustain continuo multi-sección, hilo reproductor, ciclo RAM, staging limpio, cuota LRU + `.bfaud`, Vanilla `.mod` + `zstd`, descriptores `.json`/`.xml`, centrado universal de atlas (`normalizeAndCenterAtlasFrames`), auto-dimensionado al importar en escenario (`computeStageImportDimensions`) y Flip X (`resolveCharacterAnimWithFlipX`).
- [x] **Workflow GitHub Actions Manual (`build-apk.yml`)**:
  - Disparo exclusivo `workflow_dispatch`, caché de Rust/Cargo/Gradle y subida de los 2 APKs Debug.

---

## Fase 2 — Empaquetado de Mods, Notas Especiales, Game Over y Terreno GPU (Completado)

- [x] **Exportador e Importador de Paquetes `.mod` (Contenedor Vanilla + Zstd)**:
  - Contenedor propio **Vanilla** (`VNLA` v1) con compresión **Zstandard (`zstd`)** en streaming 64 KB, conversión inteligente PNG→WebP lossless VP8L, deduplicación, FNV-1a 64-bit e integración SAF completa en `ProjectsScreen`.
- [x] **Offsets por Frame, Centrado Universal de Sprites, Auto-Tamaño de Escenario y Skins `NOTE_assets`**:
  - Descriptores `characters/*.json|xml` y `stages/*.json|xml`, centrado automático por defecto de todos los formatos de spritesheets (`Sparrow .xml`, TexturePacker/Aseprite `.json`, `spritemap1.json`, `Animation.json`), ajuste proporcional `1280×720` al importar sprites en escenario, `Offset Pose X / Y` en vivo, importación 2 pasos `NOTE_assets.png + .xml` y flecha especial SVG marcada con **"X"** para notas peligrosas.
- [x] **Lobby Inicial de Proyectos, Game Over Interactivo y Pausa Debug**:
  - Menú principal al arrancar + pantalla dedicada `ProjectLoadingScreen`, ciclo Game Over estilo FNF (toque en personaje o pantalla para reintentar) y panel Debug en Pausa (RAM, FPS, Botplay, Práctica, HUD oculto).

---

## Fase 3 — Renderizador Acelerado GPU (OpenGL ES 2.0/3.x y Vulkan) y Comunidad

- [x] **Sistema Multicámara Profundo y Seguimiento Directo desde `chart.json`**:
  - `Cam 0` (Fija inicial hasta recibir acciones), `Cam 1` (Jugador), `Cam 2` (Enemigo) y `Cam 3+` personalizadas (`Fijo`, `Jugador`, `Enemigo`, `GF`, `Entidad`), con FOV/zoom y suavizado por ranura, selector de cámara por sección en el Chart (`section.cameraSlot`), pestaña `"Cámara"` en `ChartStepCueDialog` y ejecución directa de `FocusCamera` (V-Slice), `ZoomCamera`, `Camera Follow Pos`, `Add Camera Zoom`, `Switch Camera` y `Camera Movement` (Codename).
- [x] **Editor Visual de Timeline `#` y `stage_events.lua` (Swaps, Flashes y Personajes)**:
  - Columna interactiva `#` en `ChartScreen` (`ChartStepCueDialog`) y script `stage_events.lua` para swaps, liberación/precarga RAM, flashes y cambios de personaje por paso.
- [x] **API Extendida Sin Ataduras y Terreno Base GLES 2.0/3.x + Vulkan**:
  - Gráficos/textos dinámicos (`makeText`, `makeHudGraphic`, `makeGraphic`), multicámara (`switchCamera`, `setCameraFollowPos`, `clearCameraFollowPos`, `getActiveCamera`, `getGfSection`), flashes, ciclo RAM, tweens, Game Over (`triggerGameOver` / `onGameOver`) y capa de sondeo + arquitectura `gfx/` lista para conectar renderizado GPU nativo.
- [ ] **Renderizador GPU Activo (`GLES 2.0 / GLES 3.x` + `Vulkan`)**:
  - Implementar pipelines de texturas y batching por GPU sobre `GlesSurfaceHost` / `Vulkan` aprovechando el terreno y sondeo ya integrados en `gfx_backend.rs` y `gfx_probe.rs`.
- [ ] **Reproducción de SFX desde Scripts y Calibrador de Latencia AAudio**:
  - API `playSound` para efectos cortos desde Lua/TS/JS y asistente rápido de golpeteo al ritmo para calcular `audio_offset_ms` óptimo.

---

## Fase 4 — Distribución en Uptodown y Tiendas de Terceros

- [ ] **Perfil Release Optimizado (LTO + Strip)**:
  - Opción en workflow manual para compilar APKs `Release` firmados con keystore configurado vía GitHub Secrets.
- [ ] **Iconos Adaptativos y Banner Horizontal**:
  - Recursos `mipmap-*` originales libres de derechos (`branding/`) para publicación limpia en Uptodown y tiendas APK alternativas.
- [ ] **Pruebas Automatizadas Rust en CI**:
  - Paso adicional de `cargo test --workspace` en host Linux dentro de GitHub Actions antes del cross-compile Android.

