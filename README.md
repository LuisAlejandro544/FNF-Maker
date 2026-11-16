# BeatForge Studio — Motor y Juego Modular de Mods de Ritmo

BeatForge Studio (`com.beatforge.studio`) es un **motor de creación de mods de ritmo y juego horizontal jugable** para Android, construido con un núcleo nativo modular en **Rust** (`pulse_core`, `pulse_audio`, `pulse_scripting`, `pulse_android`), **audio AAudio en Rust puro** (sin C++), **Lua 5.4**, **TypeScript**, **JavaScript** y una interfaz moderna en **Kotlin + Jetpack Compose + Material 3 + Material Icons Extended**.

> **Nota sobre la vista web (`src/App.tsx`):** La aplicación web Vite muestra únicamente `Hello World` para compilar y previsualizar limpiamente en este entorno de navegador. Todo el motor nativo móvil, la interfaz Compose y la configuración Android residen de forma modular en `/engine`, `/android`, `/sdk` y `/.github/workflows`.

---

## Características Principales

1. **Importador de Charts (`.json`)**:
   - Carga charts de mods externos con detección automática de `bpm`, `speed`, secciones `mustHitSection`, cambios de BPM y notas sostenidas.
   - Incluye histograma visual de densidad de notas en Compose `Canvas` y estadísticas por bando.
2. **Mezclador de Audio de Baja Latencia en Rust Puro (AAudio C ABI, hasta 3 pistas)**:
   - Motor de audio 100% en Rust (`libaaudio.so` vía `libc::dlopen`, cero dependencias de C++ ni `libc++_shared.so`, con reloj de respaldo para Android 7.x `minSdk = 24`).
   - Soporta mods con **1 archivo** (mix completo), **2 archivos** (Instrumental + Voces) o **3 archivos** (Instrumental + Voces Jugador + Voces Oponente).
   - Decodificadores con licencia permisiva: **OGG Vorbis** (`lewton`), **WAV** (`hound`) y **MP3** (`rmp3`), con control de volumen, mute, atenuación al fallar y forma de onda.
3. **Ensamblador Automático de Spritesheets (`.png` + `.xml`)**:
   - Importa tu hoja `.png` y tu atlas Sparrow `.xml` y **Rust** detecta y agrupa automáticamente `Idle`, `Arriba`, `Abajo`, `Izquierda` y `Derecha`, previsualizando los frames recortados en vivo a 24 FPS desde Kotlin Compose.
4. **Editor Visual de Escenario Horizontal (`1280×720`)**:
   - Mueve tus assets con **cruceta direccional** (pasos configurables de 1px, 10px, 35px) o arrastrando directamente sobre el lienzo 16:9.
   - Ajusta escala, opacidad, volteo horizontal (`Flip X`), rol (`Fondo`, `Jugador`, `Oponente`, `Decoración`) y **orden de capa Z** (`Subir`, `Bajar`, `Frente`, `Fondo`).
5. **Compilación en Caliente (`Hot Compile`)**:
   - Antes de iniciar la partida, muestra una pantalla de carga mientras **Rust** (`rayon`) optimiza y cachea texturas según el perfil del dispositivo (`32-bit / RAM baja`, `Equilibrado`, `Alta calidad`) y **descarta automáticamente objetos invisibles** durante toda la canción para ahorrar RAM/VRAM.
6. **Skins de Flechas `.svg` Personalizables**:
   - Incluye flechas vectoriales propias en formato `.svg` libres de derechos de autor (`sdk/assets/arrows/`) rasterizadas por `resvg` en Rust, con popup modal (`ArrowSkinDialog`) para importar `.svg` por dirección.
7. **Scripting Comunitario en Lua 5.4, TypeScript y JavaScript**:
   - **Lua 5.4** (`mlua`), **TypeScript** transpilado nativamente dentro del teléfono sin Node.js (`oxc_parser` + `oxc_transformer` + `oxc_codegen`) y **JavaScript ES2022** (`rquickjs` / QuickJS-NG).
   - Editor nativo en Compose (`OutlinedTextField` monoespaciado) con soporte completo del teclado virtual de Android y validación real de sintaxis en Rust.
8. **Interfaz Horizontal Nativa en Jetpack Compose + Material 3**:
   - Diseñada con **Kotlin Jetpack Compose**, **Material 3** y **Material Icons Extended** en paleta oscura de estudio (pizarra profundo, cian, esmeralda, índigo y coral), dividida en pantallas modulares dedicadas, controles multi-touch de 4 carriles y pantalla de diagnóstico aislada (`CrashReportActivity`).

---

## Compilar los 2 APKs Debug sin PC (GitHub Actions Manual)

El proyecto está pensado para compilarse directamente en la nube desde tu teléfono y distribuirse en Uptodown o tiendas APK de terceros:

1. Sube este repositorio a tu cuenta de GitHub.
2. Abre la pestaña **Actions** desde el navegador del teléfono.
3. Selecciona el workflow **Compilar BeatForge Studio APK (Debug Manual)** y pulsa **Run workflow** (solo se ejecuta manualmente mediante `workflow_dispatch`; nunca se dispara automáticamente en `push` ni `pull_request`).
4. El flujo utiliza caché (Rust, `cargo-ndk`, Gradle), descarga dependencias e instala targets de **64 bits** (`arm64-v8a`) y **32 bits** (`armeabi-v7a`).
5. Al terminar, descarga el artefacto correspondiente a tu dispositivo:
   - `BeatForgeStudio-64bit-arm64-v8a-Debug` (teléfonos modernos de 64 bits)
   - `BeatForgeStudio-32bit-armeabi-v7a-Debug` (teléfonos de 32 bits)

---

## Documentación Relacionada

- `STRUCTURE.md` — Árbol completo de carpetas y propósito de cada módulo Rust y Kotlin Compose.
- `ROADMAP.md` — Fases completadas y próximos hitos del motor y comunidad.
- `AI_CONTEXT.md` — Contexto técnico profundo (crates Rust, AAudio puro, JNI, Jetpack Compose, `oxc`).
- `AGENTS.md` — Reglas estrictas para agentes de IA y colaboradores.
- `README_AI.md` — Guía rápida de prompts y comandos de verificación.
