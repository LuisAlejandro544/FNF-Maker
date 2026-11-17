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
│   ├── Cargo.toml                       # Dependencias centralizadas (libc, jni, mlua, rquickjs, oxc, resvg, rayon, texture2ddecoder)
│   ├── .cargo/
│   │   └── config.toml                  # Flags de enlazador Android (alineación 16KB para Android 15+)
│   └── crates/
│       ├── pulse_core/                  # Lógica pura del motor (sin UI ni Android)
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs               # Exportaciones públicas y pruebas unitarias Rust
│       │       ├── chart.rs             # Parser y exportador universal `chart.json` (clásico, V-Slice, Codename, `strumLines`, doble envoltura, eventos)
│       │       ├── sparrow.rs           # Lector XML Sparrow y auto-armado de Idle/Left/Down/Up/Right
│       │       ├── astc_loader.rs       # Decodificador puro Rust `.astc` (`texture2ddecoder`) y cargador unificado de texturas
│       │       ├── atlas_json/          # Lectores `.json` y baker `Animation.json` divididos modularmente
│       │       │   ├── mod.rs           # Punto de entrada del módulo, detección de formato y reexportaciones públicas
│       │       │   ├── transform.rs     # Álgebra afín 2D (`Affine2D`) y matrices/color (`ColorTransform2D`)
│       │       │   ├── direct_formats.rs# Parsers directos `spritemap1.json` y TexturePacker Hash/Array
│       │       │   ├── animate_model.rs # Árbol de líneas de tiempo, capas, fotogramas y símbolos de `Animation.json`
│       │       │   └── animate_baker.rs # Rasterización por pose a atlas 4096×4096 empaquetado por estantes + tests
│       │       ├── stage.rs             # Escenario 1280×720, roles (Fondo/Player/Oponente/Decoración) y Z-order
│       │       ├── svg_skin.rs          # Flechas SVG originales + rasterizador resvg a RGBA8888
│       │       ├── project.rs           # Modelo ModProject, límites (máx. 3 pistas audio) y demo integrada
│       │       ├── frame_memory.rs      # Memoria por frame / ventana deslizante de notas
│       │       ├── script_optimizer.rs  # Poda y neutralización de bucles pesados en Lua/TS/JS
│       │       └── hot_compile.rs       # Compilación en caliente paralela (rayon) directa a disco, poda y límites 32-bit
│       │
│       ├── pulse_audio/                 # Subsistema de audio de baja latencia 100% Rust puro
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs               # AudioEngineHandle, merge_tracks_to_wav y control del stream
│       │       ├── decoder.rs           # Decodificación OGG/WAV/MP3 a PCM i16 compacto (50% menos RAM) y resampleo
│       │       ├── streaming.rs         # Fusión en caliente a WAV único (`compiled_mix.wav`) + streaming SPSC 512 KB desde disco
│       │       ├── mixer.rs             # Mezclador lock-free (stream único o pistas estudio) + reloj maestro + countdown BPM
│       │       └── rust_audio_backend.rs # Backend Android AAudio en Rust puro (dlopen C ABI, cero C++) + fallback
│       │
│       ├── pulse_scripting/             # Motores de scripting comunitario unificados
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs               # ModScriptRuntime y validador de sintaxis en caliente
│       │       ├── bridge.rs            # Cola thread-safe de ScriptCommand hacia el escenario
│       │       ├── lua_vm.rs            # Máquina virtual Lua 5.4 (mlua)
│       │       ├── ts_transpiler.rs     # Compilador TypeScript -> ES2022 JS en Rust (oxc 0.158)
│       │       └── js_vm.rs             # Máquina virtual JavaScript ES2022 (rquickjs / QuickJS-NG)
│       │
│       └── pulse_android/               # Puente nativo Android (`libpulse_android.so`)
│           ├── Cargo.toml               # Crate cdylib + rlib con jni, android_logger, serde_json e image
│           ├── build.rs                 # Script de compilación limpio (sin C++ ni GameActivity)
│           └── src/
│               ├── lib.rs               # Exportaciones JNI (`Java_com_beatforge_studio_NativeEngineBridge_*`)
│               └── session/             # `EngineSession` modularizado por responsabilidad (sin caché RGBA duplicado)
│                   ├── mod.rs           # Struct `EngineSession`, constructores y helpers compartidos
│                   ├── dto.rs           # DTOs serializables hacia Kotlin (`ProjectSnapshot` y reportes compilados)
│                   ├── editor_ops.rs    # Operaciones de editor de chart, audio, escenario, scripts y skins SVG
│                   ├── sprite_ops.rs    # Importación PNG/ASTC, Sparrow XML, baking Animate y previsualización
│                   ├── persistence_ops.rs # Guardado/carga de proyectos en disco y recarga de pistas PCM
│                   └── gameplay_ops.rs  # Hot-compile texturas + WAV único, streaming en gameplay y paquete binario BFGP
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
│           ├── AndroidManifest.xml      # `sensorLandscape`, teclado `adjustResize` y permisos Android 7..15
│           ├── res/values/strings.xml
│           └── kotlin/com/beatforge/studio/
│               ├── MainActivity.kt      # ComponentActivity + Compose root + OpenDocument + CreateDocument (.json)
│               ├── CrashReportActivity.kt # Pantalla aislada (:crash) para copiar diagnóstico técnico en teléfono
│               ├── GameplayFrameState.kt# Decoder binario BFGP sin GC por frame en Kotlin
│               ├── NativeEngineBridge.kt# Declaraciones `external fun` 1:1 hacia `libpulse_android.so`
│               ├── SafFileBridge.kt     # Copia segura de URIs SAF a almacenamiento interno
│               ├── StudioModels.kt      # Data classes Kotlin y parsers `org.json` de snapshots Rust
│               ├── StudioViewModel.kt   # Estado reactivo (`StateFlow`), detección RAM/ABI/lowMemory, exclusión mutua Bitmaps
│               └── ui/
│                   ├── theme/
│                   │   └── StudioTheme.kt         # Paleta estudio carbón mate, naranja estudio, ámbar y verde Material 3
│                   ├── components/
│                   │   └── StudioNavBar.kt        # Riel vertical izquierdo compacto (`StudioNavRail`) con Material Icons
│                   ├── popups/
│                   │   ├── ArrowSkinDialog.kt     # Modal para personalizar o restaurar flechas .svg (resvg)
│                   │   └── ChartJsonDetailsDialog.kt # Modal para editar metadatos `chart.json`, `changeBPM` y eventos
│                   └── screens/
│                       ├── HubScreen.kt           # Metadatos del mod, métricas y accesos directos modulares
│                       ├── ProjectsScreen.kt      # Gestión, creación, respaldo y carga de proyectos locales
│                       ├── ChartScreen.kt         # Coordinador del editor manual de 8 carriles
│                       ├── chart/                 # Subcomponentes modulares del editor de chart
│                       │   ├── ChartToolsPanel.kt # Panel izquierdo: importar/exportar, BPM, sustain y salto por secciones
│                       │   └── ChartTimelineGrid.kt # Rejilla vertical continua de 8 carriles y toggle de cámara
│                       ├── AudioScreen.kt         # Mezclador multicanal de hasta 3 pistas + forma de onda en vivo
│                       ├── SpriteLabScreen.kt     # Importador PNG/ASTC + XML/JSON (Animate) y visor 24 FPS con rotación
│                       ├── StageEditorScreen.kt   # Coordinador del editor visual de escenario en 3 columnas (60 FPS)
│                       ├── stage/                 # Subcomponentes modulares del editor de escenario
│                       │   ├── StageLiveTransformState.kt # Estado en vivo 60Hz aislado para entidades y marco de cámara
│                       │   ├── StageLayersPanel.kt    # Panel izquierdo con capa `Cámara (.lua)` y lista virtualizada `LazyColumn`
│                       │   ├── StageCanvasPreview.kt  # Lienzo 16:9 con marco de captura de cámara, culling y arrastre táctil
│                       │   ├── StageCameraInspector.kt# Inspector de Cámara (beats/secciones, captura/zoom, cruceta y enlace .lua)
│                       │   └── StageEntityInspector.kt# Inspector de entidad: rol 2x2, cruceta 1/10/35px, sliders y flipX
│                       ├── ScriptStudioScreen.kt  # Editor nativo Lua / TS / JS (incluyendo `camera_system.lua`) y validador Rust
│                       ├── HotCompileScreen.kt    # Pantalla de carga durante optimización Rayon + fusión de audio en caliente
│                       └── GameplayScreen.kt      # Renderizado Canvas 60 FPS, traslación/zoom de cámara, Idle BPM y 4 zonas touch
│
├── sdk/                                 # Recursos y SDK para creadores de mods y pruebas locales
│   ├── assets/arrows/                   # Flechas SVG originales libres de derechos (left, down, up, right)
│   ├── samples/                         # Scripts de muestra (.lua, .ts, .js)
│   └── ts/src/
│       ├── beatforge-sdk.d.ts           # Tipado TypeScript oficial del API de scripting
│       ├── engine-tools.ts              # Utilidades puras de validación de charts, audio y scripts
│       └── engine-tools.test.ts         # Pruebas unitarias ejecutables con `npx tsx --test`
│
└── Documentación raíz
    ├── README.md
    ├── ROADMAP.md
    ├── STRUCTURE.md
    ├── AI_CONTEXT.md
    ├── AGENTS.md
    └── README_AI.md
```
