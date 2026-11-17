# Roadmap — BeatForge Studio

Hoja de ruta técnica y de producto para **BeatForge Studio** (`com.beatforge.studio`).

---

## Fase 1 — Núcleo Rust Modular, Audio AAudio Puro, UI Jetpack Compose y CI Manual 32/64 bits (Actual)

- [x] **Workspace Rust modular (`/engine`)**:
  - `pulse_core`: Parser y exportador completo de `chart.json` (`player1`, `player2`, `gfVersion`, `stage`, `events`, `changeBPM`, `mustHitSection`, `-1` inline), decodificador **`.astc`** (`texture2ddecoder`) y codificador **WebP sin pérdida (VP8L)** puro Rust (`save_lossless_webp`), ensamblador Sparrow `.xml` + atlas **`.json`** (`TexturePacker`/`Aseprite`, `spritemap1.json`, `Animation.json` con baking por software + poses `Miss Left/Down/Up/Right`), escena `1280×720` con Z-order, cámara bidireccional (`camera_system.lua`), plantilla sincronizada `stage_events.lua` (`-- [STAGE_CUES_START/END]`), rasterizador SVG `resvg`, optimizador de scripts (`script_optimizer.rs`), **compilador en caliente incremental (`HotCompileManifest` por huella FNV-1a/tamaño/`mtime` + preservación de sprites referenciados en `chart.events` + análisis de reutilización `analyze_reused_entity_ids`) directo a `.webp` lossless**, **`disk_cache.rs`** (presupuestos 65%/35% por cuota del sistema y desalojo LRU en disco) y **`vanilla.rs`** (contenedor portátil **Vanilla** `.mod` con streaming **Zstandard**).
  - `pulse_audio`: Mezclador lock-free de hasta 3 pistas sincronizado por muestras en **Rust puro** (`libaaudio.so` vía `dlopen`, cero C++), decodificadores permisivos (`lewton`, `hound`, `rmp3`) en **PCM `i16` compacto**, **caché binario `.bfaud` (`audio_cache.rs`) de audio decodificado + picos de onda en `cacheDir/audio_cache/` con escritura atómica y LRU**, **fusión en caliente de 1–3 pistas a WAV único + streaming desde disco por buffer circular SPSC de 512 KB** (`streaming.rs`) y **cuenta regresiva BPM ("3, 2, 1, ¡YA!")** antes de `0.0 ms`.
  - `pulse_scripting`: VM **Lua 5.4** (`mlua`), transpilador **TypeScript** nativo en móvil (`oxc 0.158`) y VM **JavaScript ES2022** (`rquickjs`) **sin ataduras**, con **pre-compilación en caché (`.luac` / `.js` validado)** y **watchdog real en runtime** (`HookTriggers::every_nth_instruction(4096)` en Lua + `set_interrupt_handler` en QuickJS, límites 250 ms/25 ms y cuarentena tras 3 disparos), compartiendo hooks, modcharts (`setStrumProp`), flashes (`flashCamera`/`flashHud`/`flashEffect`), gestión RAM (`preloadEntity`/`retireEntity`/`swapEntity`/`setActiveCharacter`), tweens/timers nativos, sprites/textos dinámicos, HUD, audio y cámara.
  - `pulse_android`: Puente JNI (`EngineSession`) modularizado (`editor_ops`, `sprite_ops`, `persistence_ops`, `gameplay_ops`) + estado lock-free **`HOT_COMPILE_PROGRESS`**, **staging temporal en `cacheDir` sin ensuciar `Datos del usuario` + adopción permanente en `filesDir/projects/<id>/assets/` + recolección de huérfanos en `mod_imports/`**, **caché multi-proyecto `compiled_cache/<projectId>/` con cuota automática y desalojo LRU**, precarga anticipada `4000 ms` antes de swaps (`Dispatchers.IO`), salida `.webp` lossless para `.astc` y Adobe Animate, paquete binario `BFGP` v2 (`152B` cabecera con `flashColorArgb`, `flashAlpha`, `flashOverHud`, `activePlayerIdx`, `activeOpponentIdx`; `64B` entidad con `e_flags` bits `6–7` para precarga/retiro, `strumOverrides`, `dynamicPool`, `accuracyPct`, `ratingEventId`, `splashEventId`), Idle al BPM, poses `Miss` y liberación de pistas PCM al jugar.
- [x] **Interfaz Android Nativa en Kotlin + Jetpack Compose (`/android`)**:
  - `ComponentActivity` + Material 3 + Material Icons Extended + `StudioViewModel` con corrutinas `Dispatchers.IO`, **`SafFileBridge` con staging efímero en `cacheDir/import_staging/` y limpieza inmediata `try/finally`**, reporte automático de cuota vía **`StorageManager.getCacheQuotaBytes` (API 26+) → `nativeSetCacheQuota`**, sondeo en vivo del progreso real de compilación (`hotCompileProgress`), decodificación 1:1 de región Sparrow (`BitmapRegionDecoder` con `LruCache` 20 MB), recorte automático de tiras de íconos 2-frame, carga perezosa en Gameplay (omite sprites inicialmente ocultos, precarga `4 s` antes en `Dispatchers.IO`, reciclaje diferido a 3 frames con `.recycle()` y recarga segura en Reiniciar), y detección de `lowMemory` / 32-bit.
  - Pantallas dedicadas (`Hub`, `Proyectos`, `Chart` con hilo reproductor 60 Hz + botón `Seguir/Libre` + scrubber + colas *sustain* continuas entre secciones + columna `#` interactiva `ChartStepCueDialog` + acceso a `stage_events.lua`, `Audio`, `Sprites` con PNG/ASTC/WebP + XML/JSON, slots `Miss`, visor 1:1 nítido y popup `CharacterIconsDialog`, `Escenario` a 60 FPS con `StageLiveTransformState`, `StageCameraInspector` ligado a `camera_system.lua` y capas virtualizadas `LazyColumn`, `Scripts`, `Compilando` con indicador determinado `0%–100%` y detalle por fase, `Gameplay` multi-touch con barra de vida SVG, íconos dinámicos al BPM, flashes Mundo/HUD, `Puntaje | Fallos | Precisión`, letreros SVG, `Note Splash` y pausa compacta) y popup modal de skins `.svg`.
  - `minSdk 26` (Android 8.0+), `compileSdk 35`, `targetSdk 35`, `ndkVersion = "27.2.12479018"`.
  - Integración obligatoria de `cargo-ndk` en `preBuild` generando 2 APKs separados (`arm64-v8a` 64 bits y `armeabi-v7a` 32 bits).
- [x] **SDK & Pruebas (`/sdk`)**:
  - Definiciones `.d.ts`, scripts de ejemplo (`.lua`, `.ts`, `.js`), flechas SVG originales y suite de pruebas unitarias TypeScript (`tsx --test`, **35 casos**) cubriendo `chart.json`, countdown BPM, `.astc`, `.json` sprites, poses `Miss`, escalado `low_32bit`, paquete binario `BFGP` v2 (`152B`), evaluación FNF HUD / `Sick` splash, streaming audio, cámara Lua, cadencia Idle por BPM, caché incremental, `.webp` lossless, watchdog con cuarentena, **colas sustain continuas multi-sección**, **hilo reproductor + seguimiento**, **ciclo RAM / precarga 4 s / flash-swap cues**, **staging limpio en `cacheDir`**, **cuota LRU multi-proyecto + caché binario `.bfaud`** y **contenedor Vanilla (`.mod` + `zstd`)**.
- [x] **Workflow GitHub Actions Manual (`build-apk.yml`)**:
  - Disparo exclusivo `workflow_dispatch`, caché de Rust/Cargo/Gradle y subida de los 2 APKs Debug.

---

## Fase 2 — Empaquetado de Mods y Mejoras de Juego en Tiempo Real

- [x] **Exportador e Importador de Paquetes `.mod` (Contenedor Vanilla + Zstd)**:
  - Contenedor propio **Vanilla** (`VNLA` v1, extensión temporal `.mod` para pruebas) con compresión **Zstandard (`zstd`)** en streaming 64 KB (nivel 12 para JSON/XML/scripts/SVG, nivel 9 para WAV/ASTC y Raw para OGG/MP3/WebP/JPG), conversión inteligente PNG→WebP lossless VP8L, deduplicación de assets, verificación FNV-1a 64-bit e integración SAF completa en `ProjectsScreen` (`Exportar .mod` / `Importar .mod`).
- [ ] **Offsets por Frame y Calibración Avanzada en SpriteLab**:
  - Ajuste interactivo de `offset_x` / `offset_y` por animación (`Idle`, `Left`, `Down`, `Up`, `Right`, `Miss`) directamente en `SpriteLabScreen`.
- [x] **Persistencia Automática de Proyectos**:
  - Guardado atómico y carga de proyectos locales en `filesDir/projects/<id>/` con historial reciente, duplicación, borrado y adopción limpia de assets en `ProjectsScreen`.

---

## Fase 3 — Herramientas Avanzadas de Escenario y Comunidad

- [x] **Editor Visual de Timeline `#` y `stage_events.lua` (Swaps, Flashes y Personajes)**:
  - Columna interactiva `#` en `ChartScreen` (`ChartStepCueDialog`) y script dedicado `stage_events.lua` para programar visibilidad instantánea, liberación/precarga RAM de sprites, flashes inicio->fin (Mundo/HUD) y cambios de jugador/oponente/escenario por paso sin escribir código manual.
- [x] **API Extendida Sin Ataduras para Scripts Comunitarios**:
  - Control de textos/gráficos en HUD y mundo (`makeText`, `makeHudGraphic`, `makeGraphic`), flashes y ciclo RAM (`flashCamera`, `flashHud`, `preloadEntity`, `retireEntity`, `swapEntity`, `setActiveCharacter`), modcharts (`setStrumProp`), *tweens* suaves nativos (`tweenEntityProp`, `tweenStrumProp`, `tweenCameraPos`, `tweenCameraZoom`), temporizadores (`runTimer`) y control de audio/HUD.
- [ ] **Reproducción de SFX desde Scripts y Calibrador de Latencia AAudio**:
  - API `playSound` para efectos cortos desde Lua/TS/JS y asistente rápido de golpeteo al ritmo para calcular `audio_offset_ms` óptimo en auriculares Bluetooth o altavoz.

---

## Fase 4 — Distribución en Uptodown y Tiendas de Terceros

- [ ] **Perfil Release Optimizado (LTO + Strip)**:
  - Opción en workflow manual para compilar APKs `Release` firmados con keystore configurado vía GitHub Secrets.
- [ ] **Iconos Adaptativos y Banner Horizontal**:
  - Recursos `mipmap-*` originales libres de derechos para publicación limpia en Uptodown y tiendas APK alternativas.
- [ ] **Pruebas Automatizadas Rust en CI**:
  - Paso adicional de `cargo test --workspace` en host Linux dentro de GitHub Actions antes del cross-compile Android.
