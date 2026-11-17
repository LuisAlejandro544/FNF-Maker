# Estructura del Proyecto — BeatForge Studio

Este documento describe el árbol modular del repositorio y la responsabilidad de cada carpeta y archivo.

```text
.
├── .github/
│   └── workflows/
│       └── build-apk.yml                # CI manual (workflow_dispatch): compila 2 APKs Debug (64b + 32b)
│
├── src/                                 # Cascarón Web Vite/React ("Hello World" para entorno AI Studio)
│   ├── App.tsx                          # Vista mínima Hello World (mantiene compilación web verde)
│   ├── index.css                        # Import de Tailwind CSS
│   └── main.tsx                         # Punto de entrada React
├── index.html                           # Entrada HTML web
├── metadata.json                        # Metadatos del applet
├── package.json                         # Dependencias web + tsx para tests del SDK
│
├── engine/                              # Motor nativo modular en Rust (Workspace Cargo, sin UI)
│   ├── Cargo.toml                       # Dependencias centralizadas (libc, jni, mlua, rquickjs, oxc, resvg, rayon, texture2ddecoder, zstd)
│   ├── .cargo/
│   │   └── config.toml                  # Flags de enlazador Android (alineación 16KB para Android 15+)
│   └── crates/
│       ├── pulse_core/                  # Lógica pura del motor (sin UI ni Android)
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs               # Exportaciones públicas y pruebas unitarias Rust
│       │       ├── chart.rs             # Parser y exportador universal `chart.json` (clásico, V-Slice, Codename, `strumLines`, doble envoltura, eventos)
│       │       ├── sparrow.rs           # Lector XML Sparrow y auto-armado de Idle/Left/Down/Up/Right + Miss Left/Down/Up/Right
│       │       ├── astc_loader.rs       # Decodificador puro Rust `.astc` (`texture2ddecoder`) y codificador WebP lossless (`save_lossless_webp`)
│       │       ├── atlas_json/          # Lectores `.json` y baker `Animation.json` divididos modularmente
│       │       │   ├── mod.rs           # Punto de entrada del módulo, detección de formato y reexportaciones públicas
│       │       │   ├── transform.rs     # Álgebra afín 2D (`Affine2D`) y matrices/color (`ColorTransform2D`)
│       │       │   ├── direct_formats.rs# Parsers directos `spritemap1.json` y TexturePacker Hash/Array
│       │       │   ├── animate_model.rs # Árbol de líneas de tiempo, capas, fotogramas y símbolos de `Animation.json`
│       │       │   └── animate_baker.rs # Rasterización por pose a atlas 4096×4096 empaquetado por estantes + tests
│       │       ├── stage.rs             # Escenario 1280×720, roles (Fondo/Player/Oponente/Decoración), anim_offsets y Z-order
│       │       ├── mod_descriptors.rs   # Parsers `.json`/`.xml` de personaje (`characters/*`) y escenario (`stages/*`: Psych/V-Slice/Codename)
│       │       ├── svg_skin.rs          # Flechas, barra de vida, íconos, ratings (`SICK!`/`GOOD!`/`BAD`/`MISS`) y Note Splash SVG vía resvg
│       │       ├── project.rs           # Modelo ModProject, `camera_system.lua` y `stage_events.lua` (`-- [STAGE_CUES_START/END]`)
│       │       ├── frame_memory.rs      # Memoria por frame / ventana deslizante de notas
│       │       ├── script_optimizer.rs  # Poda segura preservando API completa (flashes, swap, preload/retire, personaje)
│       │       ├── hot_compile.rs       # Compilación incremental (`HotCompileManifest`, FNV-1a, WebP lossless, preservación de eventos y reuso)
│       │       ├── disk_cache.rs        # Presupuestos 65%/35% por cuota del sistema, limpieza plana heredada y desalojo LRU en disco
│       │       └── vanilla.rs           # Contenedor portátil Vanilla (`.mod`) con Zstandard, PNG->WebP lossless, dedup y FNV-1a
│       │
│       ├── pulse_audio/                 # Subsistema de audio de baja latencia 100% Rust puro
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs               # AudioEngineHandle, merge_tracks_to_wav y control del stream
│       │       ├── decoder.rs           # Decodificación OGG/WAV/MP3 a PCM i16 compacto (50% menos RAM) y resampleo
│       │       ├── audio_cache.rs       # Caché binario `.bfaud` (picos de onda + PCM i16) en `cacheDir/audio_cache/` con escritura atómica y LRU
│       │       ├── streaming.rs         # Fusión en caliente a WAV único (`compiled_mix.wav`) + streaming SPSC 512 KB desde disco
│       │       ├── mixer.rs             # Mezclador lock-free (stream único o pistas estudio) + reloj maestro + countdown BPM
│       │       └── rust_audio_backend.rs # Backend Android AAudio en Rust puro (dlopen C ABI, cero C++) + fallback
│       │
│       ├── pulse_scripting/             # Motores de scripting comunitario unificados sin ataduras
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs               # ModScriptRuntime, `precompile_scripts_with_cache` (`.luac`/`.js`), hooks y validador en caliente
│       │       ├── bridge.rs            # Cola thread-safe de ScriptCommand (modcharts, flashes, swap/RAM, personaje, tweens, HUD/audio)
│       │       ├── lua_vm.rs            # VM Lua 5.4 (mlua) con `precompile_bytecode`, API completa y hook watchdog (250ms/25ms)
│       │       ├── ts_transpiler.rs     # Compilador TS -> ES2022 JS y validador de sintaxis (`validate_js_syntax`) vía oxc 0.158
│       │       └── js_vm.rs             # VM ES2022 (rquickjs) con API completa y `set_interrupt_handler` watchdog (250ms/25ms)
│       │
│       └── pulse_android/               # Puente nativo Android (`libpulse_android.so`)
│           ├── Cargo.toml               # Crate cdylib + rlib con jni, android_logger, serde_json e image
│           ├── build.rs                 # Script de compilación limpio (sin C++ ni GameActivity)
│           └── src/
│               ├── lib.rs               # Exportaciones JNI (`nativeSetCacheQuota`, `nativeImportCharacterConfig`, `nativeSetEntityAnimOffset`, `nativeImportStageConfig`, `.mod`) y `HOT_COMPILE_PROGRESS`
│               └── session/             # `EngineSession` modularizado por responsabilidad (sin caché RGBA duplicado)
│                   ├── mod.rs           # Struct `EngineSession`, rutas `compiled_cache/<id>/` y `audio_cache/`, cuota de caché y helpers
│                   ├── dto.rs           # DTOs hacia Kotlin (incl. `character_config_path`, `anim_offsets` y métricas de caché)
│                   ├── editor_ops.rs    # Operaciones de editor (`import_stage_config`, `stage_events.lua`), audio, escenario, scripts e íconos
│                   ├── sprite_ops.rs    # Importación PNG/ASTC/WebP, Sparrow XML, Animate, `import_character_config` y `set_entity_anim_offset`
│                   ├── persistence_ops.rs # Guardado/carga, adopción `assets/`, GC de huérfanos, audio `.bfaud` y export/import Vanilla `.mod`
│                   └── gameplay_ops/      # Compilación incremental y simulación de gameplay modularizada
│                       ├── mod.rs         # Coordinador `start/pause/stop_gameplay`, toque/LRU de caché y BFGP v2 (`152B`)
│                       ├── hot_compile_ops.rs # `hot_compile_with_progress` en `compiled_cache/<projectId>/` con cuota y desalojo LRU multi-proyecto
│                       ├── script_sync.rs # `tweens`, timers, `flash`, `active_player/opponent`, precarga/retiro y drenaje `ScriptCommand`
│                       ├── simulation.rs  # Simulación por frame (`step_gameplay_internal`), lookahead 4s, flashes y notas/hold
│                       └── packet_encoder.rs # Serialización JSON (`step_gameplay`) y binaria `BFGP` v2 (`152B` cabecera) con `anim_offsets` por pose
│
├── android/                             # Proyecto Gradle Android (Kotlin + Jetpack Compose + Material 3)
│   ├── settings.gradle.kts
│   ├── build.gradle.kts                 # AGP 8.9.1 + Kotlin 2.1.20 + Compose Compiler Plugin
│   ├── gradle.properties
│   ├── gradle/wrapper/gradle-wrapper.properties
│   └── app/
│       ├── build.gradle.kts             # Splits 32b/64b, Compose BOM, Material Icons Extended y cargo-ndk en preBuild
│       ├── proguard-rules.pro           # Reglas JNI para NativeEngineBridge y SafFileBridge
│       └── src/main/
│           ├── AndroidManifest.xml      # `sensorLandscape`, teclado `adjustResize`, iconos `@mipmap/ic_launcher` y permisos Android 8..15
│           ├── res/
│           │   ├── drawable/
│           │   │   └── ic_launcher_monochrome.xml # Silueta monocroma VectorDrawable (Android 13+)
│           │   ├── mipmap-anydpi-v26/   # Adaptive Icons (`ic_launcher.xml`, `ic_launcher_round.xml`)
│           │   ├── mipmap-mdpi..xxxhdpi/# Iconos PNG `ic_launcher`, `ic_launcher_round`, `_foreground` y `_background`
│           │   └── values/strings.xml
│           └── kotlin/com/beatforge/studio/
│               ├── MainActivity.kt      # ComponentActivity + Compose root + OpenDocument + CreateDocument (.json y .mod)
│               ├── CrashReportActivity.kt # Pantalla aislada (:crash) para copiar diagnóstico técnico en teléfono
│               ├── GameplayFrameState.kt# Decoder binario BFGP v2 (`152B` cabecera: flash, activePlayer/Opponent, flags preload/retired)
│               ├── NativeEngineBridge.kt# Declaraciones `external fun` 1:1 hacia `libpulse_android.so` (incl. `nativeExport/ImportModPackage`)
│               ├── SafFileBridge.kt     # Staging temporal de URIs SAF en `cacheDir/import_staging/` (incl. exportación `.mod`) y barrido al arranque
│               ├── StudioModels.kt      # Data classes Kotlin (`HotCompileProgress`, `CompileReportSnapshot`, `ImportKind.ModPackage`) y parsers JSON
│               ├── StudioUiState.kt     # Estado reactivo `StudioUiState` y contenedor `LoadedEntityBitmap`
│               ├── StudioViewModel.kt   # Coordinador principal (`StateFlow`) que delega en `viewmodel/*` sobre `Dispatchers.IO`
│               ├── viewmodel/           # Submódulos especializados del ViewModel
│               │   ├── StudioBitmapLoader.kt      # Detección 32/64-bit, `LruCache` 1:1 Sparrow y decodificación de bitmaps/SVG
│               │   ├── StudioEditorDelegate.kt    # Proyectos, cuota `StorageManager` (API 26+), staging SAF, exportación/importación `.mod` y edición
│               │   └── StudioGameplayCoordinator.kt # Ciclo RAM (skip ocultos, precarga 4s IO, recycle diferido 3 frames, restart) y BFGP v2
│               └── ui/
│                   ├── theme/
│                   │   └── StudioTheme.kt         # Paleta estudio carbón mate, naranja estudio, ámbar y verde Material 3
│                   ├── components/
│                   │   └── StudioNavBar.kt        # Riel vertical izquierdo compacto (`StudioNavRail`) con Material Icons
│                   ├── popups/
│                   │   ├── ArrowSkinDialog.kt     # Modal para personalizar o restaurar flechas .svg (resvg)
│                   │   ├── CharacterIconsDialog.kt# Modal para íconos normal/perdiendo (auto-split 2-frame) y barra de vida .svg
│                   │   ├── ChartJsonDetailsDialog.kt # Modal para editar metadatos `chart.json`, `changeBPM` y eventos
│                   │   ├── ChartStepCueDialog.kt  # Modal en columna `#` para Swaps/RAM, Flashes (inicio->fin) y cambio de Personajes
│                   │   └── ModFilesGuideDialog.kt # Modal guía: qué archivo va en cada pestaña y en qué carpeta de mod suele estar
│                   └── screens/
│                       ├── HubScreen.kt           # Metadatos del mod, métricas y accesos directos modulares
│                       ├── ProjectsScreen.kt      # Gestión, creación, copia, borrado e importación/exportación Vanilla (`.mod`) de proyectos
│                       ├── ChartScreen.kt         # Coordinador del editor manual de 8 carriles + eventos `#` y `stage_events.lua`
│                       ├── chart/                 # Subcomponentes modulares del editor de chart
│                       │   ├── ChartTimelineModels.kt # Colas sustain continuas, mapa `#`, lookup O(log N) del hilo y duración total
│                       │   ├── ChartPlayheadState.kt  # Estado `@Stable` a 60 Hz del hilo reproductor + toggle Seguir/Libre sin recomponer filas
│                       │   ├── ChartToolsPanel.kt     # Panel izquierdo: importar/exportar, BPM, sustain, transporte hilo/Seguir y `stage_events.lua`
│                       │   └── ChartTimelineGrid.kt   # Rejilla DAW continua + barra de transporte/scrubber y overlay 60 FPS del hilo reproductor
│                       ├── AudioScreen.kt         # Mezclador multicanal de hasta 3 pistas + forma de onda en vivo
│                       ├── SpriteLabScreen.kt     # PNG/ASTC + XML/JSON, `Posición (.json/.xml)`, `Offset Pose X/Y`, `¿Dónde van?`, visor 1:1 y `Íconos / Vida`
│                       ├── StageEditorScreen.kt   # Coordinador del editor visual de escenario en 3 columnas (60 FPS) + `Escena .json/.xml`
│                       ├── stage/                 # Subcomponentes modulares del editor de escenario
│                       │   ├── StageLiveTransformState.kt # Estado en vivo 60Hz aislado para entidades y marco de cámara
│                       │   ├── StageLayersPanel.kt    # Panel izquierdo con capa `Cámara (.lua)`, `Escena .json/.xml`, ayuda `?` y `LazyColumn`
│                       │   ├── StageCanvasPreview.kt  # Lienzo 16:9 con marco de captura de cámara, culling y arrastre táctil
│                       │   ├── StageCameraInspector.kt# Inspector de Cámara (beats/secciones, captura/zoom, cruceta y enlace .lua)
│                       │   └── StageEntityInspector.kt# Inspector de entidad: rol 2x2, cruceta 1/10/35px, sliders y flipX
│                       ├── ScriptStudioScreen.kt  # Editor nativo Lua / TS / JS (`camera_system.lua` y `stage_events.lua`) con referencia API
│                       ├── HotCompileScreen.kt    # Pantalla de carga con barra y círculo determinados (`0%–100%`) + fase en vivo
│                       ├── GameplayScreen.kt      # Coordinador jugable 60 FPS (`withFrameNanos`) + flash HUD y barra/íconos dinámicos
│                       └── gameplay/              # Submódulos especializados de `GameplayScreen`
│                           ├── GameplayRenderPrimitives.kt # Pools sin GC, `GameplayTouchMaskState` y geometría de carriles
│                           ├── GameplayWorldRenderer.kt    # Dibujo 60 FPS de escena (omite retirados), flash mundo, cámara, notas y Splash
│                           ├── GameplayTouchOverlay.kt     # Entrada multi-touch de 4 carriles (Hitbox Clásico / Tocar Flechas)
│                           └── GameplayHudOverlay.kt       # Barra de vida SVG + íconos dinámicos BPM, cuenta regresiva y Pausa/Fin
│
├── sdk/                                 # Recursos y SDK para creadores de mods y pruebas locales
│   ├── assets/arrows/                   # Flechas SVG originales libres de derechos (left, down, up, right)
│   ├── samples/                         # Scripts de muestra (.lua, .ts, .js)
│   └── ts/src/
│       ├── beatforge-sdk.d.ts           # Tipado TypeScript oficial del API completo (incl. flashes, swaps, preload/retire, personaje)
│       ├── engine-tools.ts              # Validación, sustain, hilo, ciclo RAM 4s, staging/cuota LRU/`.bfaud`, Vanilla `.mod`, descriptores `.json/.xml` y BFGP v2 (`152B`)
│       └── engine-tools.test.ts         # 39 pruebas unitarias ejecutables con `npx tsx --test`
│
├── branding/                            # Identidad visual "Beat Hammer" (SVGs maestros, generador y PNGs de tienda/miniatura)
│   ├── generate_pngs.mjs                # Conversor automático SVG -> PNG (resvg-js) para Android y tiendas
│   ├── svg/                             # Fuentes vectoriales (.svg): icono, redondo, foreground/background/monochrome, banner y miniatura
│   └── png/                             # PNGs listos para Uptodown/tiendas (512, 1024, banner 1024x500) y miniaturas (1280x720, 640x360)
│
├── public/                              # Activos estáticos servidos directamente por Vite sin tocar `src/App.tsx`
│   ├── ver-video.html                   # Reproductor HTML5 móvil para ver/descargar el combate stickman de 20s
│   └── videos/
│       └── stickman_fight_20s.mp4       # Video MP4 H.264/AAC (1280×720, 30 FPS, 20.0s) exportado por Video Studio CLI
│
├── tools/
│   └── video-studio/                    # Bonus: suite CLI modular de descarga, edición de MP4 largos y animación procedural
│       ├── setup-env.sh                 # Instalador reproducible (yt-dlp, deno, uv, faster-whisper, scenedetect, OpenCV, PyAV, Pillow)
│       ├── download-video               # Descargador CLI con soporte JS runtime (deno/node) y --cookies opcional
│       ├── ingest-long-video            # Ingesta en streaming para MP4 largos (proxy 480p, audio 16k, silencios, escenas y edit.json)
│       ├── transcribe-video             # Transcripción offline con Silero VAD + subtítulos .json/.srt/.ass
│       ├── render-edit                  # Renderizador modular (modos copy / draft / master + QC frames)
│       ├── make-vertical-reel           # Atajo para extraer clips verticales 9:16 con subtítulos desde MP4 largos
│       ├── render-stickman-fight        # CLI generador de combate stickman (Azul vs Rojo) con audio DSP BGM + SFX
│       ├── video_pipeline_lib.py        # Lógica pura de intervalos, cortes, ffmpeg filters y planes edit.json
│       ├── stickman_fight_lib.py        # Motor procedural de animación, cinemática stickman, partículas y síntesis BGM/SFX
│       ├── test_video_pipeline.py       # Pruebas unitarias `unittest` del pipeline de video
│       ├── README.md                    # Guía rápida de uso del Bonus Video Studio CLI
│       └── workspace/                   # Salidas de trabajo locales (ignorado en git vía .gitignore)
│
└── Documentación raíz
    ├── README.md
    ├── ROADMAP.md
    ├── STRUCTURE.md
    ├── AI_CONTEXT.md
    ├── AGENTS.md
    └── README_AI.md
```
