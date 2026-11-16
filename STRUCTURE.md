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
├── engine/                              # Motor nativo modular en Rust (Workspace Cargo)
│   ├── Cargo.toml                       # Dependencias centralizadas (eframe, oboe, mlua, rquickjs, oxc, resvg)
│   ├── .cargo/
│   │   └── config.toml                  # Flags de enlazador Android (alineación 16KB para Android 15+)
│   └── crates/
│       ├── pulse_core/                  # Lógica pura del motor (sin UI ni Android)
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs               # Exportaciones públicas y pruebas unitarias Rust
│       │       ├── chart.rs             # Parser de charts .json, BPM, secciones y densidad de notas
│       │       ├── sparrow.rs           # Lector XML Sparrow y auto-armado de Idle/Left/Down/Up/Right
│       │       ├── stage.rs             # Escenario 1280×720, roles (Fondo/Player/Enemigo/Decoración) y Z-order
│       │       ├── svg_skin.rs          # Flechas SVG originales + rasterizador resvg a RGBA8888
│       │       ├── project.rs           # Modelo ModProject, límites (máx. 3 pistas audio) y demo integrada
│       │       └── hot_compile.rs       # Compilación en caliente paralela (rayon) y poda de invisibles
│       │
│       ├── pulse_audio/                 # Subsistema de audio de baja latencia
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs               # AudioEngineHandle y control del stream Oboe
│       │       ├── decoder.rs           # Decodificación OGG (lewton), WAV (hound), MP3 (rmp3) y resampleo
│       │       ├── mixer.rs             # Mezclador lock-free de hasta 3 pistas + reloj maestro por muestras
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
│       ├── pulse_ui/                    # Interfaz gráfica horizontal completa en Rust (eframe + egui)
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs               # BeatForgeApp (eframe::App::ui) y barra de navegación superior
│       │       ├── theme.rs             # Paleta pizarra profesional (cian, índigo, esmeralda, coral)
│       │       ├── state.rs             # AppState, runtime tokio, caché de texturas GPU y eventos SAF
│       │       ├── popups/
│       │       │   ├── mod.rs
│       │       │   └── arrow_skin_popup.rs # Modal para personalizar o restaurar flechas .svg
│       │       └── screens/
│       │           ├── mod.rs
│       │           ├── hub_screen.rs          # Inicio, resumen del proyecto y perfil de recursos del móvil
│       │           ├── chart_audio_screen.rs  # Importador de chart .json y 1–3 pistas de audio
│       │           ├── sprite_lab_screen.rs   # Importador .png + .xml con visor animado en vivo
│       │           ├── stage_editor_screen.rs # Editor de escenario, cruceta de flechas, escala, opacidad y Z
│       │           ├── script_studio_screen.rs# Editor y validador en vivo Lua / TypeScript / JS
│       │           ├── compile_screen.rs      # Pantalla de carga durante la compilación en caliente
│       │           └── gameplay_screen.rs     # Motor jugable horizontal (Oboe sync + controles táctiles/teclado)
│       │
│       └── pulse_android/               # Puente nativo Android (`libpulse_android.so`)
│           ├── Cargo.toml               # Crate cdylib con android-game-activity, jni y winit 0.30
│           ├── build.rs                 # Enlaza `c++_shared` requerido por Google Oboe
│           └── src/
│               └── lib.rs               # `android_main` + callbacks JNI hacia SafFileBridge.kt
│
├── android/                             # Proyecto Gradle Android (Kotlin + GameActivity)
│   ├── settings.gradle.kts
│   ├── build.gradle.kts
│   ├── gradle.properties
│   ├── gradle/wrapper/gradle-wrapper.properties
│   └── app/
│       ├── build.gradle.kts             # Splits 32b/64b, prefab GameActivity, tasks cargo-ndk y copyCppShared
│       ├── proguard-rules.pro           # Reglas JNI para MainActivity y SafFileBridge
│       └── src/main/
│           ├── AndroidManifest.xml      # `sensorLandscape`, OpenGL ES 3.0 y permisos Android 7..15
│           ├── res/values/strings.xml
│           └── kotlin/com/beatforge/studio/
│               ├── MainActivity.kt      # GameActivity + ActivityResultContracts + corrutina Dispatchers.IO
│               └── SafFileBridge.kt     # Copia segura de URIs SAF a almacenamiento interno y llamada JNI
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
