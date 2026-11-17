# BeatForge Studio — Motor y Juego Modular de Mods de Ritmo

BeatForge Studio (`com.beatforge.studio`) es un **motor de creación de mods de ritmo y juego horizontal jugable** para Android, construido con un núcleo nativo modular en **Rust** (`pulse_core`, `pulse_audio`, `pulse_scripting`, `pulse_android`), **audio AAudio en Rust puro** (sin C++), **Lua 5.4**, **TypeScript**, **JavaScript** y una interfaz moderna en **Kotlin + Jetpack Compose + Material 3 + Material Icons Extended**.

> **Nota sobre la vista web (`src/App.tsx`):** La aplicación web Vite muestra únicamente `Hello World` para compilar y previsualizar limpiamente en este entorno de navegador. Todo el motor nativo móvil, la interfaz Compose y la configuración Android residen de forma modular en `/engine`, `/android`, `/sdk` y `/.github/workflows`.

---

## Características Principales

1. **Editor Manual de Charts Rediseñado, Hilo Reproductor y Eventos `#` (`chart.json`)**:
   - Línea de tiempo continua de **8 carriles** (4 Oponente + 4 Jugador) con renderizado continuo de notas largas (`Head` -> haz vertical continuo + núcleo luminoso -> `Tail` redondeado, incluso cruzando entre secciones con distinto BPM) y selector de duración (`Normal`, `2p`, `4p`, `8p`, `16p`).
   - **Hilo Reproductor a 60 Hz + Seguimiento (`Seguir` / `Libre`)**: Reproduce la canción desde el mixer Rust viendo bajar un hilo luminoso sub-paso por la rejilla; permite **Pausar y Reanudar** conservando el cursor exacto, **Rebobinar a 0:00**, **arrastrar la barra de tiempo**, mantener pulsado cualquier paso `#` para situar el hilo o activar/pausar el **seguimiento automático de scroll** para localizar el paso exacto de cada Flash o Swap.
   - **Columna Interactiva `#` de Cues por Paso (`ChartStepCueDialog`)**: Pulsa cualquier número de paso `#` (o `"Poner Flash / Swap en #X"` sobre el hilo actual) para insertar o eliminar al instante cambios de visibilidad (`Visibilidad / Swap` con opción de liberar RAM), **Flashes** con paso de inicio y fin, color HEX, opacidad y capa (`Mundo` o `HUD`), cambios dinámicos de **Jugador / Oponente / Escenario** (pudiendo promover sprites de escenario a personaje activo) y precargas manuales, sincronizados bidireccionalmente con `stage_events.lua`.
   - Modal `Config .json` para editar metadatos completos (`player1`, `player2`, `gfVersion`, `stage`, `speed`, `needsVoices`) y eventos globales/inline `-1`, además de **importar y exportar `.json`** completo compatible vía SAF.
2. **Mezclador y Streaming de Audio en Rust Puro (AAudio C ABI) + Cuenta Regresiva BPM**:
   - Motor de audio 100% en Rust (`libaaudio.so` vía `libc::dlopen`, cero dependencias de C++ ni `libc++_shared.so`, nativo desde Android 8.0+ `minSdk = 26` con reloj de respaldo defensivo para emuladores).
   - Al entrar a jugar o reiniciar, ejecuta una **cuenta regresiva sincronizada al BPM real de la canción** (`"3"`, `"2"`, `"1"`, `"¡YA!"`) con voces/tonos sintetizados lock-free en `pulse_audio` y animación visual sincronizada en `GameplayScreen` antes de arrancar en `0.0 ms`.
   - Soporta mods con **1 archivo** (mix completo), **2 archivos** (Instrumental + Voces) o **3 archivos** (Instrumental + Voces Jugador + Voces Oponente) en **OGG** (`lewton`), **WAV** (`hound`) y **MP3** (`rmp3`), almacenados en **PCM `i16` compacto** en el editor (50 % menos RAM), respaldados por un **caché binario `.bfaud` (`cacheDir/audio_cache/`) de audio decodificado y picos de onda** para reabrir proyectos al instante sin re-decodificar, y fusionados en caliente en un único stream de disco (~512 KB de buffer circular) al jugar.
    - **Ensamblador Automático de Spritesheets (`.png`, `.astc`, `.webp`, `.xml`, `.json`), Posición/Offsets y Vista 1:1**:
    - Decodificación nativa de texturas comprimidas **`.astc`** en Rust puro (`texture2ddecoder`, sin GLES obligatorio en el editor) junto a `.png`/`.jpg`/`.webp`, almacenando las hojas decodificadas o horneadas en **WebP sin pérdida (lossless VP8L)** de máxima compresión.
    - Soporte de atlas **`.xml` Sparrow** y formatos **`.json`** de mods optimizados: **TexturePacker / Aseprite** (Hash/Array), **`spritemap1.json`** y **`Animation.json`** (Adobe Animate Texture Atlas), horneando automáticamente las poses a spritesheet RGBA WebP sin pérdida y detectando `Idle`, `Left`, `Down`, `Up`, `Right` y las 4 poses de fallo (**`Miss Left`**, **`Miss Down`**, **`Miss Up`**, **`Miss Right`**) con soporte de frames rotados 90°.
    - **Descriptores de Posición y Offsets de Personaje (`Posición (.json/.xml)`)**: Importa archivos de configuración de personaje (`.json` estilo Psych/V-Slice o `.xml` `<character>` estilo Codename) antes o después del atlas para aplicar automáticamente `position`, `scale`, `flip_x`, `camera_position`, FPS, bucle, prefijos de animación y los **`offsets [x, y]` exactos por cada pose**. Además, `SpriteLabScreen` incluye controles en vivo **`Offset Pose X / Y`** (`-5 / -1 / Centrar 0 / +1 / +5`) reflejados al instante tanto en la vista previa a 24 FPS como en Gameplay.
    - **Previsualización 1:1 sin borrosidad**: `SpriteLabScreen` decodifica el recorte exacto del frame activo mediante `BitmapRegionDecoder` con caché `LruCache` acotada a 20 MB en `Dispatchers.IO` (`FilterQuality.High`), evitando imágenes borrosas sin saturar la RAM en dispositivos de 32 bits.
    - **Popup Modal `Íconos / Vida` (`CharacterIconsDialog`) y Guía `¿Dónde van?` (`ModFilesGuideDialog`)**: Permite cargar o limpiar íconos de Rival y Jugador (estados **Normal** y **Perdiendo**, con recorte automático si subes una tira horizontal de 2 frames), personalizar la barra `.svg` y consultar desde Sprites o Escena dónde conseguir e importar cada archivo de un mod.
4. **Editor Visual de Escenario Horizontal (`1280×720`), Importación `Escena .json/.xml` y Sistema de Cámara en Lua**:
    - Mueve tus assets con **cruceta direccional** (pasos configurables de 1px, 10px, 35px) o arrastrando directamente sobre el lienzo 16:9 con etiquetas pre-medidas, *culling* fuera de vista y transformaciones en vivo (`StageLiveTransformState`) sin recomponer ni disparar JNI en cada píxel del arrastre.
    - **Importar `Escena .json/.xml`**: Carga descriptores de escenario (`stages/*.json` o `<stage>.xml`) para ubicar automáticamente Player, Enemigo y Decoración/Cabina, aplicar el zoom inicial de cámara (`camera_system.lua`), respetar `hide_girlfriend` y registrar props de fondo ordenados por capa Z.
    - Panel de capas virtualizado (`LazyColumn`) para escalar decenas de objetos sin lag de interfaz, con ajustes de escala, opacidad, volteo horizontal (`Flip X`), rol en cuadrícula 2×2 (`Fondo`, `Oponente`, `Jugador`, `Decoración`) y **orden de capa Z**.
    - **Sistema de Cámara (`Cámara (.lua)` ↔ `camera_system.lua`)**: Selecciona la capa de cámara en `Escena` para mover su centro, agrandar o achicar el marco de captura (zoom base `0.35x–2.50x`), configurar seguimiento automático entre **Enemigo y Jugador** (cada N beats o siguiendo `mustHitSection` de `chart.json`) y abrir/editar su script real `camera_system.lua` en Lua 5.4.
    - **Idle Sincronizado al BPM**: Las poses de reposo (`Idle`) del Jugador y Rival se disparan alineadas al reloj de beats/BPM activo de `chart.json` (respetando `changeBPM`), con cadencia automática cada 1 o 2 beats y pulso rítmico sutil para personajes PNG estáticos.
5. **Compilación en Caliente Incremental (`Hot Compile`) en WebP Lossless y Ciclo Inteligente de RAM**:
   - Detecta automáticamente la RAM disponible (`ActivityManager.MemoryInfo`, incluyendo `lowMemory`), la cuota real del sistema (`StorageManager.getCacheQuotaBytes`) y arquitectura (32/64 bits) de tu teléfono y muestra una pantalla de carga con **progreso real y porcentaje (`0%–100%`)** por fases mientras **Rust** (`rayon`):
     - **Caché multi-proyecto con cuota automática (LRU en disco, `compiled_cache/<projectId>/manifest.json`)**: Cada proyecto conserva su propia compilación incremental (hash FNV-1a/tamaño/`mtime`, gama `tier`, geometría, audio y scripts) sin sobrescribir la de otros mods, y un desalojo LRU elimina automáticamente las cachés más antiguas al superar la cuota (protegiendo el mod activo).
     - **Almacenamiento limpio (`Datos del usuario` mínimos)**: Las importaciones desde el selector SAF se copian de forma temporal en `cacheDir/import_staging/` y se borran inmediatamente tras importar; charts, XML, scripts y SVGs quedan embebidos en `project.json`, mientras solo los binarios permanentes (audio, spritesheets, íconos) se adoptan bajo `filesDir/projects/<id>/assets/` con recolección automática de archivos huérfanos.
     - **Sprites compilados a WebP sin pérdida (VP8L) + Carga perezosa**: Precompila en disco todas las texturas en `.webp` lossless (incluidas las entidades inicialmente ocultas pero referenciadas en `chart.events` o scripts, sin decodificarlas a RAM al arrancar Gameplay), respetando límites 32-bit (`1536 px` estáticos / `2560 px` atlas).
     - **Precarga anticipada 4 s + Reciclaje diferido + Reinicio seguro**: Durante la partida, Rust emite solicitudes de precarga `4000 ms` antes de un cambio y `StudioGameplayCoordinator` decodifica el `.webp` en `Dispatchers.IO` sin tirones de 60 FPS; al retirar sprites que no volverán a aparecer, los libera de forma segura con cola diferida de 3 frames (`.recycle()`), manteniendo reutilizables los que sí regresan más adelante y recargando automáticamente los iniciales al pulsar **Reiniciar** en Pausa.
     - **Fusiona las 1–3 pistas activas en `compiled_mix.wav`** dentro del caché del proyecto cuando cambia alguna pista o mezcla, liberando decenas/cientos de MB de PCM de la RAM al entrar a gameplay mediante un buffer circular SPSC lock-free de **512 KB**.
6. **HUD Estilo FNF en SVG (`resvg`), Flashes, Note Splash y Skins de Flechas Personalizables**:
   - **Barra de Vida SVG + Íconos Reactivos Dinámicos**: Barra de vida `.svg` rasterizada con `resvg` (roja para Rival, verde para Jugador, botón compacto de Pausa en esquina superior derecha), con íconos normal/perdiendo (`<20%`) que rebotan al ritmo del BPM y cambian automáticamente si ocurre un cambio dinámico de personaje (`activePlayerIdx` / `activeOpponentIdx`), más `Puntaje | Fallos | Precisión (%)` debajo de la barra.
   - **Flashes en Capa Mundo o HUD**: Efecto de flash con color, opacidad y desvanecimiento suave entre pasos exactos, renderizable detrás del HUD (capa mundo/cámara) o sobre toda la pantalla (capa HUD).
   - **Calificaciones SVG (`SICK!`, `GOOD!`, `BAD`, `MISS`) y Note Splash**: Letreros SVG vectoriales junto al jugador activo y destello animado (**Note Splash**) teñido al color de la flecha sobre el receptor al acertar `Sick`.
   - **Skins de Flechas `.svg` Personalizables**: Flechas vectoriales propias libres de derechos de autor (`sdk/assets/arrows/`) con popup modal (`ArrowSkinDialog`) para importar `.svg` por dirección.
7. **Scripting Comunitario Sin Ataduras en Lua 5.4, TypeScript y JavaScript (`stage_events.lua` + `camera_system.lua`)**:
   - **Lua 5.4** (`mlua`), **TypeScript** transpilado nativamente dentro del teléfono sin Node.js (`oxc_parser` + `oxc_transformer` + `oxc_codegen`) y **JavaScript ES2022** (`rquickjs` / QuickJS-NG).
   - **Script Dedicado de Escenario y Cambios (`stage_events.lua`)**: Provisionado en cada proyecto y sincronizado por bloques `-- [STAGE_CUES_START]` .. `-- [STAGE_CUES_END]` con el editor de pasos `#` del Chart para orquestar swaps de visibilidad/escenario, flashes, precargas/descargas de RAM y cambios de jugador/oponente.
   - **Pre-compilación en caché (`compiled_cache/scripts/`) y Watchdog Real**: Genera `.luac` y `.js` reutilizables; los hooks nativos (`HookTriggers::every_nth_instruction(4096)` y `set_interrupt_handler`) aplican presupuesto de **250 ms** en `onCreate` y **25 ms** por frame con cuarentena automática tras 3 excesos.
   - **API Completa de Modcharts, Escenario, Flashes y RAM**: Control de receptores (`setStrumProp`/`getStrumProp`), propiedades de entidades (`angle`, `visible`, `flipX`, `zIndex`, `color`), creación dinámica (`makeGraphic`, `makeHudGraphic`, `makeText`, `setTextString`, `cloneSprite`, `removeSprite`), flashes y ciclo de vida (`flashCamera`, `flashHud`, `flashEffect`, `preloadEntity`, `retireEntity`, `swapEntity`, `setActiveCharacter`), *tweens* y temporizadores nativos, HUD/audio y hooks completos (`onCreate`, `onUpdate`, `onStepHit`, `onBeatHit`, `onNoteHit`, `onNoteMiss`, `onCountdownTick`, `onEvent`, `onTweenCompleted`, `onTimerCompleted`).
8. **Interfaz Horizontal Nativa en Jetpack Compose + Material 3**:
   - Riel de navegación lateral izquierdo (`StudioNavRail`) con manejo de muesca de cámara (`displayCutoutPadding`), diseñado en paleta de estudio hardware (carbón mate, grafito cálido, naranja estudio, ámbar y verde limpio), controles multi-touch de 4 carriles y pantalla de diagnóstico aislada (`CrashReportActivity`).
9. **Contenedor Portátil Vanilla (`.mod`) con Compresión Zstandard (`zstd`)**:
   - Desde **Proyectos** (`Importar .mod` / `Exportar .mod`), empaqueta o instala mods completos usando el formato propio **Vanilla** (cabecera `VNLA` v1 de 24 bytes y extensión temporal `.mod` para pruebas) impulsado por **Zstandard (`zstd`)**.
   - **Compresión híbrida inteligente**: aplica **Zstd nivel 12** a `project.json`, charts, scripts `.lua/.ts/.js`, Sparrow `.xml` y `.svg`; **Zstd nivel 9** a `.wav`, `.astc` y `.bmp`; y **Raw** sin inflar ni gastar CPU en formatos ya comprimidos (`.ogg`, `.mp3`, `.webp`, `.jpg`), convirtiendo automáticamente sprites `.png`/`.bmp` a **WebP sin pérdida (VP8L)** cuando reduce el tamaño.
   - Deduplica assets idénticos, omite `decoded_textures` redundantes de `.astc` (reconstruyéndolos al importar), opera por **streaming en bloques de 64 KB** apto para teléfonos 32/64 bits y valida integridad **FNV-1a 64-bit** contra corrupción o *path traversal*.

---

## Bonus: Estudio CLI de Video, Edición de MP4 Largos y Animación Procedural (`tools/video-studio/`)

Además del motor nativo Android de BeatForge Studio, el repositorio incluye como **bonus** un conjunto modular de ejecutables CLI (`Unlicense`, `MIT`, `BSD-3-Clause`, `Apache-2.0`, `HPND`, cero dependencias copyleft) listo para reinstalar en cualquier contenedor Linux y producir, descargar, transcribir o editar videos `.mp4` directamente desde consola:

- **Instalador reproducible (`tools/video-studio/setup-env.sh`)**: Instala o verifica `yt-dlp` (última versión + alias `yt-dlp-latest`), `deno` + puente `node` (`--remote-components ejs:github` para desafíos JS), `uv` + entorno `/opt/video-studio-venv` (`faster-whisper` con Silero VAD, `scenedetect`, `opencv-python-headless`, `av`, `pillow`, `numpy`) y `ffmpeg`/`ffprobe`.
- **Descarga desde consola (`tools/video-studio/download-video`)**: Descarga videos públicos (con soporte opcional de `--cookies` Netscape cuando una plataforma restringe IPs de nube).
- **Ingesta en streaming para `.mp4` largos y pesados (`tools/video-studio/ingest-long-video`)**: Procesa archivos de varios GB sin cargarlos completos en RAM generando proxy ligero `480p`, audio `16 kHz` mono, detección de silencios (`ffmpeg silencedetect`), cortes de escena (`PySceneDetect`) y plan declarativo `edit.json`.
- **Transcripción + Subtítulos Karaoke (`tools/video-studio/transcribe-video`)**: Transcripción offline con `faster-whisper` + Silero VAD y exportación `.json`, `.srt` y `.ass`.
- **Renderizado modular (`tools/video-studio/render-edit` y `make-vertical-reel`)**: Soporta modo `copy` (recorte instantáneo sin recodificar), `draft` (borrador `480p` + hojas de contacto `--qc`) y `master` (`1080p` / `9:16`, normalización `-14 LUFS`, música con *ducking* y subtítulos quemados).
- **Generación procedural de animación + audio DSP (`tools/video-studio/render-stickman-fight`)**: Renderiza escenas animadas frame a frame en H.264/AAC (como la demo de combate **Stickman Azul vs Stickman Rojo** de `20.0 s` en `1280×720` @ `30 FPS`, con escenario dojo ciber-atardecer, cámara dinámica, partículas, BGM `138 BPM` y SFX sincronizados), visible directamente en `/ver-video.html` (MP4 directo en `/videos/stickman_fight_20s.mp4`) sin alterar el cascarón `Hello World` de `src/App.tsx`.

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

## Guía Rápida: ¿Dónde Va Cada Archivo y Dónde Se Consigue en un Mod?

También puedes abrir esta misma guía dentro de la app tocando **`¿Dónde van?`** en **Sprites** o **`?`** en **Escena** (`ModFilesGuideDialog`):

| Archivo del Mod | Dónde se importa en BeatForge Studio | Dónde suele estar en la carpeta de un mod |
| :--- | :--- | :--- |
| **Hoja de Sprites** (`.png` / `.webp` / `.astc`) | **Sprites** → `PNG / ASTC` (o **Escena** → `Subir PNG/.astc`) | `images/characters/` o `mods/images/characters/` (fondos/props en `images/backgrounds/` o `stages/<nombre>/`) |
| **Recorte de Animaciones** (`.xml` Sparrow / `.json` Atlas) | **Sprites** → `XML / JSON` | En la misma carpeta que la imagen: `images/characters/<personaje>.xml` o `spritemap1.json` + `Animation.json` |
| **Posición del Personaje y Offsets** (`.json` / `<character>.xml`) | **Sprites** → `Posición (.json/.xml)` | `characters/`, `mods/characters/` o `assets/data/characters/` |
| **Posición Global del Escenario** (`.json` / `<stage>.xml`) | **Escena** → `Escena .json/.xml` | `stages/`, `mods/stages/` o `assets/data/stages/` |
| **Íconos y Barra de Vida** (`.png` / `.svg`) | **Sprites** → `Íconos / Vida` | `images/icons/` (`icon-<personaje>.png`) |
| **Pistas de Audio** (`.ogg` / `.wav` / `.mp3`, máx. 3) | **Audio** → Pistas `1`, `2` y `3` | `songs/<cancion>/` o `mods/songs/<cancion>/` (`Inst.ogg`, `Voices.ogg`, `Voices-Player.ogg`, `Voices-Opponent.ogg`) |
| **Chart de Notas y Eventos** (`.json`) | **Chart** → `Importar` | `data/<cancion>/` o `mods/data/<cancion>/` (`<cancion>-hard.json`, `<cancion>.json`) |
| **Scripts Modchart** (`.lua` / `.ts` / `.js`) | **Scripts** → `Importar` | `scripts/`, `mods/scripts/`, `data/<cancion>/script.lua` o `custom_events/` |

---

## Documentación Relacionada

- `STRUCTURE.md` — Árbol completo de carpetas y propósito de cada módulo Rust y Kotlin Compose.
- `ROADMAP.md` — Fases completadas y próximos hitos del motor y comunidad.
- `AI_CONTEXT.md` — Contexto técnico profundo (crates Rust, AAudio puro, JNI, Jetpack Compose, `oxc`).
- `AGENTS.md` — Reglas estrictas para agentes de IA y colaboradores.
- `README_AI.md` — Guía rápida de prompts y comandos de verificación.
