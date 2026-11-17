# Roadmap — BeatForge Studio

Hoja de ruta técnica y de producto para **BeatForge Studio** (`com.beatforge.studio`).

---

## Fase 1 — Núcleo Rust Modular, Audio AAudio Puro, UI Jetpack Compose y CI Manual 32/64 bits (Actual)

- [x] **Workspace Rust modular (`/engine`)**:
  - `pulse_core`: Parser y exportador completo de `chart.json` (`player1`, `player2`, `gfVersion`, `stage`, `events`, `changeBPM`, `mustHitSection`, `-1` inline), decodificador **`.astc`** (`texture2ddecoder`) y codificador **WebP sin pérdida (VP8L)** puro Rust (`save_lossless_webp`), ensamblador Sparrow `.xml` + atlas **`.json`** (`TexturePacker`/`Aseprite`, `spritemap1.json`, `Animation.json` con baking por software + poses `Miss Left/Down/Up/Right`), escena `1280×720` con Z-order, cámara bidireccional (`camera_system.lua`), rasterizador SVG `resvg`, optimizador de scripts (`script_optimizer.rs`) y **compilador en caliente incremental (`HotCompileManifest` por huella FNV-1a/tamaño/`mtime` + tier + geometría + mezcla de audio) directo a `.webp` lossless** con límites `1536/2560px` para `LowRam32Bit`.
  - `pulse_audio`: Mezclador lock-free de hasta 3 pistas sincronizado por muestras en **Rust puro** (`libaaudio.so` vía `dlopen`, cero C++), decodificadores permisivos (`lewton`, `hound`, `rmp3`) en **PCM `i16` compacto**, **fusión en caliente de 1–3 pistas a WAV único + streaming desde disco por buffer circular SPSC de 512 KB** (`streaming.rs`) y **cuenta regresiva BPM ("3, 2, 1, ¡YA!")** antes de `0.0 ms`.
  - `pulse_scripting`: VM **Lua 5.4** (`mlua`), transpilador **TypeScript** nativo en móvil (`oxc 0.158`) y VM **JavaScript ES2022** (`rquickjs`) **sin ataduras**, con **pre-compilación en caché (`.luac` / `.js` validado)** y **watchdog real en runtime** (`HookTriggers::every_nth_instruction(4096)` en Lua + `set_interrupt_handler` en QuickJS, límites 250 ms/25 ms y cuarentena tras 3 disparos), compartiendo todos los hooks, modcharts (`setStrumProp`), tweens/timers nativos, sprites/textos dinámicos, HUD, audio y cámara.
  - `pulse_android`: Puente JNI (`EngineSession`) modularizado (`editor_ops`, `sprite_ops`, `persistence_ops`, `gameplay_ops`) + estado lock-free **`HOT_COMPILE_PROGRESS`** (`nativeGetHotCompileProgress`), salida `.webp` lossless para `.astc` y Adobe Animate, paquete binario `BFGP` v2 (`140B` cabecera, `64B` entidad con rotación, `strumOverrides`, `dynamicPool`, `accuracyPct`, `ratingEventId`, `splashEventId`), Idle al BPM, poses `Miss`, liberación de pistas PCM al jugar y caché incremental `manifest.json`.
- [x] **Interfaz Android Nativa en Kotlin + Jetpack Compose (`/android`)**:
  - `ComponentActivity` + Material 3 + Material Icons Extended + `StudioViewModel` con corrutinas `Dispatchers.IO`, sondeo en vivo del progreso real de compilación (`hotCompileProgress`), decodificación 1:1 de región Sparrow (`BitmapRegionDecoder` con `LruCache` 20 MB), recorte automático de tiras de íconos 2-frame, exclusión mutua en RAM entre `entityBitmaps` (editor) y `compiledBitmaps` (gameplay), y detección de `lowMemory` / 32-bit.
  - Pantallas dedicadas (`Hub`, `Proyectos`, `Chart`, `Audio`, `Sprites` con PNG/ASTC/WebP + XML/JSON, slots `Miss`, visor 1:1 nítido y popup `CharacterIconsDialog`, `Escenario` a 60 FPS con `StageLiveTransformState`, `StageCameraInspector` ligado a `camera_system.lua` y capas virtualizadas `LazyColumn`, `Scripts`, `Compilando` con indicador determinado `0%–100%` y detalle por fase, `Gameplay` multi-touch con barra de vida SVG, íconos normales/perdiendo al BPM, `Puntaje | Fallos | Precisión`, letreros SVG `SICK!/GOOD!/BAD/MISS`, `Note Splash` y pausa compacta) y popup modal de skins `.svg`.
  - `minSdk 24`, `compileSdk 35`, `targetSdk 35`, `ndkVersion = "27.2.12479018"`.
  - Integración obligatoria de `cargo-ndk` en `preBuild` generando 2 APKs separados (`arm64-v8a` 64 bits y `armeabi-v7a` 32 bits).
- [x] **SDK & Pruebas (`/sdk`)**:
  - Definiciones `.d.ts`, scripts de ejemplo (`.lua`, `.ts`, `.js`), flechas SVG originales y suite de pruebas unitarias TypeScript (`tsx --test`, **29 casos**) cubriendo `chart.json`, countdown BPM, `.astc`, `.json` sprites, poses `Miss`, escalado `low_32bit`, paquete binario `BFGP` v2, evaluación FNF HUD / `Sick` splash, streaming audio, cámara Lua, cadencia Idle por BPM, **caché incremental por huella**, **enrutamiento `.webp` lossless** y **watchdog con cuarentena**.
- [x] **Workflow GitHub Actions Manual (`build-apk.yml`)**:
  - Disparo exclusivo `workflow_dispatch`, caché de Rust/Cargo/Gradle y subida de los 2 APKs Debug.

---

## Fase 2 — Empaquetado de Mods y Mejoras de Juego en Tiempo Real

- [ ] **Exportador e Importador de Paquetes `.bfmod` (ZIP)**:
  - Empaquetar chart, 1–3 audios, hojas `.png`/`.xml`, escenario JSON, íconos, barra SVG, flechas `.svg` y scripts en un único archivo portátil compartible entre teléfonos.
- [ ] **Offsets por Frame y Editor de Colas Hold en Gameplay**:
  - Ajuste interactivo de `offset_x` / `offset_y` por animación (`Idle`, `Left`, `Down`, `Up`, `Right`, `Miss`) directamente en `SpriteLabScreen` y manejo preciso de notas sostenidas al mantener pulsado el carril.
- [ ] **Persistencia Automática de Proyectos**:
  - Guardado y carga de proyectos locales en `filesDir` con historial reciente en el Hub.

---

## Fase 3 — Herramientas Avanzadas de Escenario y Comunidad

- [ ] **Editor de Timeline / Eventos de Canción**:
  - Línea de tiempo visual para colocar triggers de cámara, tintes o llamadas a `onEvent` en beats exactos sin escribir código.
- [x] **API Extendida Sin Ataduras para Scripts Comunitarios**:
  - Control de textos/gráficos en HUD y mundo (`makeText`, `makeHudGraphic`, `makeGraphic`), modcharts (`setStrumProp`), *tweens* suaves nativos (`tweenEntityProp`, `tweenStrumProp`, `tweenCameraPos`, `tweenCameraZoom`), temporizadores (`runTimer`) y control de audio/HUD.
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
