# BeatForge Studio — Motor y Juego Modular de Mods de Ritmo

BeatForge Studio (`com.beatforge.studio`) es un **motor de creación de mods de ritmo y juego horizontal jugable** para Android (inspirado en *Funky Maker: Mobile*), construido en **Rust**, **Lua 5.4**, **TypeScript**, **JavaScript** y **Kotlin** (`GameActivity` + **Google Oboe**).

> **Nota sobre la vista web (`src/App.tsx`):** La aplicación web Vite muestra únicamente `Hello World` para compilar y previsualizar limpiamente en este entorno de navegador. Todo el motor nativo móvil, la interfaz gráfica y la configuración Android residen de forma modular en `/engine`, `/android`, `/sdk` y `/.github/workflows`.

---

## Características Principales

1. **Importador de Charts (`.json`)**:
   - Carga charts de mods externos con detección automática de `bpm`, `speed`, secciones `mustHitSection`, cambios de BPM y notas sostenidas.
   - Incluye mapa visual de densidad de notas y estadísticas por bando.
2. **Mezclador de Audio de Baja Latencia con Google Oboe (hasta 3 pistas)**:
   - Soporta mods con **1 archivo** (mix completo), **2 archivos** (Instrumental + Voces) o **3 archivos** (Instrumental + Voces Player + Voces Enemigo).
   - Decodificadores con licencia permisiva: **OGG Vorbis** (`lewton`), **WAV** (`hound`) y **MP3** (`rmp3`), con control de volumen, mute, atenuación al fallar y forma de onda.
3. **Ensamblador Automático de Spritesheets (`.png` + `.xml`)**:
   - Olvídate de armar sprites manualmente en herramientas externas: importa tu hoja `.png` y tu atlas Sparrow `.xml` y **Rust** detecta y agrupa automáticamente `Idle`, `Arriba`, `Abajo`, `Izquierda` y `Derecha` con visor en vivo.
4. **Editor Visual de Escenario Horizontal (`1280×720`)**:
   - Mueve tus assets con **flechas direccionales** (pasos configurables de 1px, 10px, 35px) o arrastrando directamente.
   - Ajusta escala, opacidad, volteo horizontal (`Flip X`) y **orden de capa Z** (`Subir`, `Bajar`, `Frente`, `Fondo`).
5. **Identificadores de Entidad (Roles)**:
   - Etiqueta cada objeto del escenario como **Fondo** (incluyendo fondos animados), **Player**, **Enemigo** o **Decoración**.
6. **Compilación en Caliente (`Hot Compile`)**:
   - Antes de iniciar la partida, muestra una pantalla de carga mientras **Rust** (`rayon`) optimiza texturas según la gama del teléfono (`32 bits / RAM baja`, `Equilibrado`, `Alta calidad`) y **descarta automáticamente objetos invisibles** durante toda la canción para ahorrar RAM/VRAM.
7. **Skins de Flechas `.svg` Personalizables**:
   - Incluye flechas vectoriales propias en formato `.svg` libres de derechos de autor (`sdk/assets/arrows/`) rasterizadas con `resvg`, con popup modal para que el usuario importe sus propios `.svg` por dirección.
8. **Scripting Comunitario en Lua 5.4, TypeScript y JavaScript**:
   - **Lua 5.4** (`mlua`), **TypeScript** transpilado nativamente dentro del teléfono sin Node.js (`oxc_parser` + `oxc_transformer` + `oxc_codegen`) y **JavaScript ES2022** (`rquickjs` / QuickJS-NG).
9. **Interfaz Profesional Horizontal Hecha en Rust**:
   - Diseñada con `eframe` + `egui` en paleta azul pizarra profundo con acentos cian, índigo, esmeralda y coral, dividida en pantallas dedicadas y popups modales.

---

## Compilar los 2 APKs Debug sin PC (GitHub Actions Manual)

El proyecto está pensado para compilarse directamente en la nube desde tu teléfono:

1. Sube este repositorio a tu cuenta de GitHub.
2. Abre la pestaña **Actions** desde el navegador del teléfono.
3. Selecciona el workflow **Compilar BeatForge Studio APK (Debug Manual)** y pulsa **Run workflow** (solo se ejecuta manualmente mediante `workflow_dispatch`; nunca se dispara automáticamente en `push` ni `pull_request`).
4. El flujo utiliza caché (Rust, `cargo-ndk`, Gradle), descarga dependencias e instala targets de **64 bits** (`arm64-v8a`) y **32 bits** (`armeabi-v7a`).
5. Al terminar, descarga el artefacto correspondiente a tu dispositivo:
   - `BeatForgeStudio-64bit-arm64-v8a-Debug` (teléfonos modernos de 64 bits)
   - `BeatForgeStudio-32bit-armeabi-v7a-Debug` (teléfonos de 32 bits)

---

## Documentación Relacionada

- `STRUCTURE.md` — Árbol completo de carpetas y propósito de cada módulo.
- `ROADMAP.md` — Fases completadas y próximos hitos del motor y comunidad.
- `AI_CONTEXT.md` — Contexto técnico profundo (crates, Oboe, SAF/JNI, `oxc`, `egui`).
- `AGENTS.md` — Reglas estrictas para agentes de IA y colaboradores.
- `README_AI.md` — Guía rápida de prompts y comandos de verificación.
