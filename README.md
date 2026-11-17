# BeatForge Studio — Motor y Juego Modular de Mods de Ritmo

BeatForge Studio (`com.beatforge.studio`) es un **motor de creación de mods de ritmo y juego horizontal jugable** para Android, construido con un núcleo nativo modular en **Rust** (`pulse_core`, `pulse_audio`, `pulse_scripting`, `pulse_android`), **audio AAudio en Rust puro** (sin C++), **Lua 5.4**, **TypeScript**, **JavaScript** y una interfaz moderna en **Kotlin + Jetpack Compose + Material 3 + Material Icons Extended**.

> **Nota sobre la vista web (`src/App.tsx`):** La aplicación web Vite muestra únicamente `Hello World` para compilar y previsualizar limpiamente en este entorno de navegador. Todo el motor nativo móvil, la interfaz Compose y la configuración Android residen de forma modular en `/engine`, `/android`, `/sdk` y `/.github/workflows`.

---

## Características Principales

1. **Editor Manual de Charts, Configuración Completa y Exportación (`chart.json`)**:
   - Línea de tiempo larga continua de **8 carriles** (4 Oponente + 4 Jugador) para colocar/quitar notas manualmente por paso, elegir duración de *sustain*, ajustar BPM global o por sección (`changeBPM`), agregar secciones y cambiar enfoque de cámara o escuchar cada sección al instante.
   - Modal `Config .json` para editar metadatos completos (`player1`, `player2`, `gfVersion`, `stage`, `speed`, `needsVoices`) y eventos de canción (`events` globales e inline `-1`), además de **importar y exportar `.json`** completo compatible vía SAF.
2. **Mezclador y Streaming de Audio en Rust Puro (AAudio C ABI) + Cuenta Regresiva BPM**:
   - Motor de audio 100% en Rust (`libaaudio.so` vía `libc::dlopen`, cero dependencias de C++ ni `libc++_shared.so`, con reloj de respaldo para Android 7.x `minSdk = 24`).
   - Al entrar a jugar o reiniciar, ejecuta una **cuenta regresiva sincronizada al BPM real de la canción** (`"3"`, `"2"`, `"1"`, `"¡YA!"`) con voces/tonos sintetizados lock-free en `pulse_audio` y animación visual sincronizada en `GameplayScreen` antes de arrancar en `0.0 ms`.
   - Soporta mods con **1 archivo** (mix completo), **2 archivos** (Instrumental + Voces) o **3 archivos** (Instrumental + Voces Jugador + Voces Oponente) en **OGG** (`lewton`), **WAV** (`hound`) y **MP3** (`rmp3`), almacenados en **PCM `i16` compacto** en el editor (50 % menos RAM) y fusionados en caliente en un único stream de disco (~512 KB de buffer circular) al jugar.
3. **Ensamblador Automático de Spritesheets (`.png`, `.astc`, `.xml`, `.json`), Vista 1:1 y Animaciones Miss**:
   - Decodificación nativa de texturas comprimidas **`.astc`** en Rust puro (`texture2ddecoder`, sin GLES obligatorio en el editor) junto a `.png`/`.jpg`/`.webp`.
   - Soporte de atlas **`.xml` Sparrow** y formatos **`.json`** de mods optimizados: **TexturePacker / Aseprite** (Hash/Array), **`spritemap1.json`** y **`Animation.json`** (Adobe Animate Texture Atlas), horneando automáticamente las poses a spritesheet RGBA y detectando `Idle`, `Left`, `Down`, `Up`, `Right` y las 4 poses de fallo (**`Miss Left`**, **`Miss Down`**, **`Miss Up`**, **`Miss Right`**) con soporte de frames rotados 90°.
   - **Previsualización 1:1 sin borrosidad**: `SpriteLabScreen` decodifica el recorte exacto del frame activo mediante `BitmapRegionDecoder` con caché `LruCache` acotada a 20 MB en `Dispatchers.IO` (`FilterQuality.High`), evitando imágenes borrosas sin saturar la RAM en dispositivos de 32 bits.
   - **Popup Modal `Íconos / Vida` (`CharacterIconsDialog`)**: Permite cargar o limpiar íconos de Rival y Jugador (estados **Normal** y **Perdiendo**, con recorte automático si subes una tira horizontal de 2 frames) y personalizar el archivo **`.svg`** de la barra de vida.
4. **Editor Visual de Escenario Horizontal (`1280×720`) y Sistema de Cámara en Lua**:
   - Mueve tus assets con **cruceta direccional** (pasos configurables de 1px, 10px, 35px) o arrastrando directamente sobre el lienzo 16:9 con etiquetas pre-medidas, *culling* fuera de vista y transformaciones en vivo (`StageLiveTransformState`) sin recomponer ni disparar JNI en cada píxel del arrastre.
   - Panel de capas virtualizado (`LazyColumn`) para escalar decenas de objetos sin lag de interfaz, con ajustes de escala, opacidad, volteo horizontal (`Flip X`), rol en cuadrícula 2×2 (`Fondo`, `Oponente`, `Jugador`, `Decoración`) y **orden de capa Z**.
   - **Sistema de Cámara (`Cámara (.lua)` ↔ `camera_system.lua`)**: Selecciona la capa de cámara en `Escena` para mover su centro, agrandar o achicar el marco de captura (zoom base `0.35x–2.50x`), configurar seguimiento automático entre **Enemigo y Jugador** (cada N beats o siguiendo `mustHitSection` de `chart.json`) y abrir/editar su script real `camera_system.lua` en Lua 5.4.
   - **Idle Sincronizado al BPM**: Las poses de reposo (`Idle`) del Jugador y Rival se disparan alineadas al reloj de beats/BPM activo de `chart.json` (respetando `changeBPM`), con cadencia automática cada 1 o 2 beats y pulso rítmico sutil para personajes PNG estáticos.
5. **Compilación en Caliente Automática (`Hot Compile`) con Audio Fusionado y Cero Copias Duplicadas**:
   - Detecta automáticamente la RAM disponible (`ActivityManager.MemoryInfo`, incluyendo `lowMemory`) y arquitectura (32/64 bits) de tu teléfono y muestra una pantalla de carga mientras **Rust** (`rayon`):
     - **Fusiona las 1–3 pistas activas en un único archivo WAV (`compiled_mix.wav`)** en disco (aplicando ganancia, omitiendo silenciadas y protegiendo con limitador suave `tanh`), liberando decenas/cientos de MB de PCM de la RAM al entrar a gameplay mediante un buffer circular SPSC lock-free de **512 KB**.
     - Optimiza y escribe texturas reescaladas **directamente a caché en disco** sin retener buffers RGBA dobles o triples en RAM, descartando objetos permanentemente invisibles y aplicando límites seguros para teléfonos de 32 bits (`armeabi-v7a`) o 2–3 GB de RAM (`1536 px` estáticos / `2560 px` atlas).
6. **HUD Estilo FNF en SVG (`resvg`), Note Splash y Skins de Flechas Personalizables**:
   - **Barra de Vida SVG + Íconos Reactivos**: Reemplaza la barra superior invasiva por una barra de vida `.svg` rasterizada con `resvg` (roja para Rival, verde para Jugador, botón compacto de Pausa en la esquina superior derecha), con íconos normal/perdiendo (`<20%`) que rebotan al ritmo del BPM y se ubican arriba al centro (*downscroll*) o abajo al centro (*upscroll*).
   - **Estadísticas debajo de la barra**: Muestra `Puntaje | Fallos | Precisión (%)` directamente bajo la barra de vida.
   - **Calificaciones SVG (`SICK!`, `GOOD!`, `BAD`, `MISS`) y Note Splash**: Al acertar o fallar notas aparecen letreros SVG vectoriales junto al jugador, y al acertar **Perfecto (`Sick`)** se dispara un destello animado (**Note Splash**) teñido al color de la flecha sobre el receptor.
   - **Skins de Flechas `.svg` Personalizables**: Incluye flechas vectoriales propias libres de derechos de autor (`sdk/assets/arrows/`) con popup modal (`ArrowSkinDialog`) para importar `.svg` por dirección.
7. **Scripting Comunitario Sin Ataduras en Lua 5.4, TypeScript y JavaScript**:
   - **Lua 5.4** (`mlua`), **TypeScript** transpilado nativamente dentro del teléfono sin Node.js (`oxc_parser` + `oxc_transformer` + `oxc_codegen`) y **JavaScript ES2022** (`rquickjs` / QuickJS-NG).
   - **API Completa de Modcharts y HUD**: Control de receptores (`setStrumProp`/`getStrumProp` para mover, rotar, escalar u ocultar cualquiera de las 8 flechas), propiedades completas de entidades (`angle`, `visible`, `flipX`, `zIndex`, `color`), creación dinámica de objetos/textos en mundo o HUD (`makeGraphic`, `makeHudGraphic`, `makeText`, `setTextString`, `cloneSprite`, `removeSprite`), *tweens* y temporizadores nativos (`tweenEntityProp`, `tweenCameraPos`, `tweenCameraZoom`, `tweenStrumProp`, `runTimer`), control de gameplay/HUD/audio (`setHealth`, `setScore`, `setScrollSpeed`, `setDownscroll`, `setHudVisible`, `setHudOpacity`, `setHudZoom`, `setTrackVolume`, `setTrackMute`) y hooks completos (`onCreate`, `onUpdate`, `onStepHit`, `onBeatHit`, `onNoteHit`, `onNoteMiss`, `onCountdownTick`, `onEvent`, `onTweenCompleted`, `onTimerCompleted`).
8. **Interfaz Horizontal Nativa en Jetpack Compose + Material 3**:
   - Riel de navegación lateral izquierdo (`StudioNavRail`) con manejo de muesca de cámara (`displayCutoutPadding`), diseñado en paleta de estudio hardware (carbón mate, grafito cálido, naranja estudio, ámbar y verde limpio), controles multi-touch de 4 carriles y pantalla de diagnóstico aislada (`CrashReportActivity`).

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
