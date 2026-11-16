# BeatForge Studio — Motor y Juego Modular de Mods de Ritmo

> **Nota sobre la vista web (`src/App.tsx`):** La aplicación web Vite muestra únicamente `Hello World` para compilar limpiamente en este entorno de navegador, mientras que todo el motor nativo móvil y su interfaz en Rust residen de forma modular en `/engine`, `/android` y `/sdk`.

## Arquitectura Modular

- `/engine/crates/pulse_core`: Núcleo en Rust puro (MIT/Apache-2.0):
  - `chart.rs`: Importador de charts `.json` con normalización de carriles, `mustHitSection`, cambios de BPM y notas sostenidas.
  - `sparrow.rs`: Ensamblador automático de spritesheets `.png` + `.xml` (detecta y arma `Idle`, `Left`, `Down`, `Up`, `Right` sin depender de páginas externas).
  - `stage.rs`: Escenario horizontal `1280x720`, identificadores (`Fondo`, `Player`, `Enemigo`, `Decoración`), movimiento con flechas, escala, opacidad y orden Z.
  - `svg_skin.rs`: Rasterizador de flechas `.svg` usando `resvg` + skin vectorial propia libre de derechos.
  - `hot_compile.rs`: Compilador en caliente paralelo (`rayon`) que optimiza texturas según la gama del teléfono y descarta automáticamente objetos invisibles durante toda la canción.
- `/engine/crates/pulse_audio`: Mezclador lock-free de hasta **3 pistas simultáneas** (Mix único/Instrumental, Voces Player, Voces Enemigo) con Google **Oboe** (`oboe` crate) y decodificadores permisivos (`lewton` OGG, `hound` WAV, `rmp3` MP3).
- `/engine/crates/pulse_scripting`: Scripting comunitario unificado en **Lua 5.4** (`mlua`), **TypeScript** (`oxc_parser` + `oxc_transformer` + `oxc_codegen`) y **JavaScript ES2022** (`rquickjs` / QuickJS-NG).
- `/engine/crates/pulse_ui`: Interfaz gráfica horizontal completa hecha en Rust (`eframe` + `egui`) dividida en pantallas dedicadas y popups.
- `/engine/crates/pulse_android` + `/android`: Proyecto Gradle Android (`GameActivity`, corrutinas Kotlin `Dispatchers.IO` para SAF, `cargo-ndk` para 32 bits `armeabi-v7a` y 64 bits `arm64-v8a` / `x86_64`) listo para compilar APK para Uptodown o distribución directa.

## Compilar el APK sin PC (desde tu teléfono)

1. Sube este repositorio a GitHub.
2. Entra a la pestaña **Actions** desde el navegador de tu teléfono y ejecuta **Compilar BeatForge Studio APK**.
3. Descarga el artefacto `BeatForgeStudio-APK` generado con soporte nativo para 32 y 64 bits.
