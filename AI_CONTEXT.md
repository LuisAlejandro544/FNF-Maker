# AI Context — Arquitectura Técnica de BeatForge Studio

Este archivo concentra el contexto técnico profundo necesario para entender cómo interactúan **Rust** (lógica, audio AAudio puro, scripting y rasterizado), **JNI**, **Kotlin Coroutines (`Dispatchers.IO`)** y **Jetpack Compose + Material 3** dentro de **BeatForge Studio**.

---

## 1. Dualidad del Entorno (Web Shell vs Motor Nativo Android)

1. **Entorno Web Vite (`/src/App.tsx`)**:
   - Debe mantenerse estrictamente como un `"Hello World"` mínimo.
   - Existe para que `npm run build` y el servidor de desarrollo en el puerto `3000` dentro de AI Studio compilen y arranquen sin errores.
   - **No** debe mezclarse aquí la lógica pesada del motor móvil.
2. **Motor Móvil Real (`/engine`, `/android`, `/sdk`)**:
   - Toda la lógica del juego, parseo de charts, decodificación de audio, mezcla en tiempo real, XML Sparrow, scripting Lua/TS/JS, SVG `resvg`, compilación en caliente (`rayon`) y simulación de gameplay vive en **Rust** (`/engine`).
   - Toda la interfaz gráfica horizontal, edición de texto con teclado nativo, navegación por pantallas y renderizado Compose Canvas vive en **Kotlin + Jetpack Compose + Material 3 + Material Icons Extended** (`/android`), conectado al motor mediante **JNI directo (`NativeEngineBridge.kt` ↔ `pulse_android`)**.
   - El contenedor del entorno web no tiene `rustc` ni Android NDK instalados localmente; la compilación nativa de las arquitecturas `arm64-v8a` (64 bits) y `armeabi-v7a` (32 bits) se ejecuta mediante GitHub Actions (`.github/workflows/build-apk.yml`).

---

## 2. Versiones Exactas y Quirks de APIs Verificadas

| Componente | Versión | Detalles Críticos de API |
| :--- | :--- | :--- |
| Jetpack Compose BOM | `2025.02.00` | Incluye `material3`, `material-icons-extended`, `ui`, `ui-graphics`, `activity-compose:1.10.1`, `lifecycle-viewmodel-compose:2.8.7`, `lifecycle-runtime-compose:2.8.7` y plugin `org.jetbrains.kotlin.plugin.compose:2.1.20`. |
| Android Gradle / Kotlin | AGP `8.9.1` / Kotlin `2.1.20` | `compileSdk = 35`, `targetSdk = 35`, `minSdk = 24`, `ndkVersion = "27.2.12479018"`. Sin `GameActivity` ni `prefab`. |
| Audio Android (`AAudio` Rust puro) | `libc` (`dlopen` C ABI) | En `pulse_audio/src/rust_audio_backend.rs`, se eliminó por completo C++ Oboe (`oboe-sys`) y `libc++_shared.so`. Carga `libaaudio.so` vía `dlopen`/`dlsym` en Rust puro (API 26+) con reloj de respaldo (`FallbackClockThread`) para Android 7.x (`minSdk = 24`). |
| Puente JNI (`jni` crate) | `0.21.1` | `pulse_android/src/lib.rs` exporta símbolos `Java_com_beatforge_studio_NativeEngineBridge_*` conectados a `EngineSession` (`parking_lot::Mutex`) e intercambia snapshots JSON (`serde_json` camelCase) + `ByteArray` PNG de `resvg`. |
| `oxc_*` | `0.158.0` | `ParserReturn`, `SemanticBuilderReturn` y `TransformerReturn` exponen `.diagnostics` (`oxc_diagnostics::Diagnostics`) en lugar de `.errors`. Usar `.diagnostics.has_errors()` y `.diagnostics.errors()`. |
| `mlua` | `0.12.2` | `Lua::new()` con `lua54` y `vendored`; `func.call::<()>(args)`. |
| `rquickjs` | `0.14.0` | `Function::new(ctx.clone(), closure)` + `func.call::<_, ()>(args)` con `features = ["rust-alloc", "macro", "bindgen"]`. |
| `resvg` | `0.50.0` | Rasteriza SVG sobre `resvg::tiny_skia::Pixmap` y `pulse_android/session.rs` codifica PNG en bytes vía `image` para consumirse como `ImageBitmap` en Compose. |
| `texture2ddecoder` | `0.1.2` | Licencia `MIT OR Apache-2.0`. Decodifica texturas `.astc` (bloques 4×4..12×12) por software en Rust puro en `pulse_core/src/astc_loader.rs` convirtiendo `u32::to_le_bytes()` a RGBA. |
| `cargo-ndk` | `v4.x` | El nivel de API se pasa con `--platform 24` (NO `-p 24`, porque `-p` en v4 es `--package`). |

---

## 3. Flujos Internos Clave del Motor

### A. Importación y Exportación desde Android (SAF + Coroutines + JNI)
1. En una pantalla Compose (`ChartScreen`, `AudioScreen`, `SpriteLabScreen`, `StageEditorScreen`, `ScriptStudioScreen` o `ArrowSkinDialog`), el usuario pulsa importar (o exportar `.json` en `ChartScreen`).
2. `StudioViewModel.requestImport(kind)` abre `ActivityResultContracts.OpenDocument` (o `CreateDocument("application/json")` para exportar el `chart.json` completo) registrado en `MainActivity.kt`.
3. Al recibir el `Uri`, `StudioViewModel.onUriPicked` / `exportChartToUri` ejecuta una corrutina en **`Dispatchers.IO`** para no bloquear el hilo principal.
4. `SafFileBridge.copyUriToInternalCache` resuelve el nombre real (`OpenableColumns.DISPLAY_NAME`), sanea caracteres especiales y copia el stream a `filesDir/mod_imports/`.
5. Luego invoca el método JNI correspondiente en `NativeEngineBridge` (`nativeImportChart`, `nativeExportChartJson`, `nativeUpdateChartDetails`, `nativeSetSectionBpm`, `nativeAddChartEvent`, `nativeRemoveChartEvent`, `nativeImportAudioTrack`, `nativeImportSpriteSheetPng`, `nativeImportSparrowXml`, `nativeImportScript`, `nativeImportArrowSvg`).
6. Rust procesa/decodifica el archivo y devuelve un `ActionResultJson<ProjectSnapshotJson>`, y `StudioViewModel` recarga en `Dispatchers.IO` los bitmaps de entidades (tanto en `SpritePng` como en `SparrowXml`, necesario cuando Adobe Animate genera una hoja horneada).

### B. Reloj Maestro de Audio, PCM `i16` Compacto, Fusión en Caliente y Streaming SPSC
- En el editor (`AudioScreen`), `DecodedTrack` almacena muestras en **`Vec<i16>` estéreo** (50 % menos RAM que `f32`) y `AudioMixer` mantiene hasta 3 pistas en `slots[0..3]` para previsualización interactiva de ganancia/silencio.
- Durante **`Hot Compile`**, `merge_tracks_to_wav()` (`pulse_audio/src/streaming.rs`) mezcla las 1–3 pistas activas no silenciadas (aplicando ganancia + limitador suave `tanh`) de forma atómica a `<cacheDir>/compiled_cache/compiled_mix.wav`.
- Al iniciar `start_gameplay()`:
  1. Abre `StreamingAudioSource` sobre `compiled_mix.wav` con un hilo lector en segundo plano (`pulse_audio_streamer`) que llena un **buffer circular SPSC lock-free de ~512 KB** (`STREAM_RING_FRAMES = 131_072` frames estéreo `i16`).
  2. Libera inmediatamente `decoded_tracks` y `mixer.clear_tracks()` en Rust para recuperar decenas/cientos de MB de RAM antes de jugar (al salir con `stop_gameplay()`, recarga las pistas del editor en `Dispatchers.IO`).
  3. `AudioMixer::start_countdown(bpm)` calcula `beat_frames = (sample_rate * 60.0 / bpm)` y fija `countdown_remaining_frames = beat_frames * 4` (`AtomicU64`).
- El callback en tiempo real (`render_audio_frames`, invocado por AAudio o `FallbackClockThread` en API 24–25) lee del `stream_source` usando `seek_lock.try_read()` (cero bloqueo en el hilo de audio) y sintetiza sin mutex la cuenta regresiva (`"3"`, `"2"`, `"1"`, `"¡YA!"`) hasta `0.0 ms`.

### C. Ensamblado de Spritesheets (`SpriteLabScreen`), Vista 1:1, Poses `Miss` e Íconos
- **Imágenes (`.png`, `.jpg`, `.webp`, `.astc`)**: `astc_loader.rs` valida la cabecera mágica `[0x13, 0xAB, 0xA1, 0x5C]`, decodifica bloques `.astc` vía `texture2ddecoder::decode_astc` a `RGBA8` y `sprite_ops.rs` guarda una copia PNG en `cacheDir/decoded_textures/<id>.png` para que Compose la cargue con `BitmapFactory` y `HotCompile` pueda reescalarla sin ruta extra.
- **Atlas (`.xml` y `.json`) y Poses `Miss`**:
  1. **Sparrow / Starling `.xml` y XML Spritemaps (`<TextureAtlas>` / `<ATLAS>`)**: `sparrow.rs` lee `<SubTexture>` y `<SPRITE>` con soporte para `width`/`w`, `height`/`h`, `frameX`, `frameY`, `frameWidth`, `frameHeight` y `rotated` (`true`/`1`) y auto-asigna `Idle`, `Left`, `Down`, `Up`, `Right` más las 4 poses de fallo **`Miss Left`**, **`Miss Down`**, **`Miss Up`** y **`Miss Right`**; `SpriteLabScreen.kt` y `GameplayScreen.kt` aplican los offsets de recorte y rotación `-90°` sin deformar la relación de aspecto.
  2. **TexturePacker / Aseprite JSON** (Hash o Array), convirtiéndolo a `SparrowAtlas`.
  3. **Adobe Animate `spritemap1.json`** (`ATLAS.SPRITES`): genera spritesheet de respaldo y registra sus piezas atómicas.
  4. **Adobe Animate `Animation.json`** (`AN`/`SD`/`TL`, tanto `M3D` clásico como `BTA 1.1.0` con `MX`, `SI` para símbolos `SN` y `ASI` para piezas `N`): `try_bake_animate_for_entity` rasteriza las poses mediante transformaciones afines en software, las empaqueta con *shelf packing* en `<cacheDir>/animate_baked/<id>_baked.png` y construye su `SparrowAtlas`.
- **Previsualización 1:1 Sin Borrosidad en `SpriteLabScreen`**: Para evitar que un spritesheet gigante (ej. `4096×4096`) escalado hacia abajo en `entityBitmaps` haga que el frame individual en `SpriteLabScreen` se vea pixelado o borroso, `StudioViewModel.decodeSparrowRegionBitmap()` decodifica **únicamente el rectángulo del frame activo** en resolución 1:1 mediante `BitmapRegionDecoder` sobre `Dispatchers.IO`, respaldado por un `LruCache<String, ImageBitmap>` acotado a **20 MB** y dibujado con `FilterQuality.High` (sin riesgo de OOM en 32 bits / 2–3 GB de RAM).
- **Íconos de Personajes y Barra de Vida `.svg` (`CharacterIconsDialog.kt`)**: Desde `SpriteLab` (`"Íconos / Vida"`), el usuario puede importar/limpiar íconos de estado **Normal** y **Perdiendo** para Rival y Jugador (si se importa una tira horizontal estilo FNF de 2 cuadros con proporción `w >= h * 1.65`, `StudioViewModel.loadEntityIconsInternal()` recorta automáticamente mitad izquierda = Normal y mitad derecha = Perdiendo) y reemplazar la barra de vida `.svg` rasterizada con `resvg`.

### D. Compilación en Caliente (`Hot Compile`) Sin Copias Duplicadas en RAM
- Disparada por `StudioViewModel.startCompileAndPlay()` en `Dispatchers.IO`, detectando automáticamente la RAM total, `memInfo.lowMemory` y el ABI (`armeabi-v7a` 32 bits vs `arm64-v8a` 64 bits) para elegir `tierCode` e invocando `NativeEngineBridge.nativeHotCompile(tierCode)` sobre `rayon::par_iter()`:
  1. Clasifica entidades según su rol (`Background`, `Player`, `Opponent`, `Decoration`) y orden Z.
  2. **Poda de invisibles**: Descarta cualquier entidad con `!visible` u `opacity <= 0.005` o fuera de escena que no sea referenciada por scripts, reportando la RAM/VRAM ahorrada.
  3. **Pipeline de copia única en RAM**: `hot_compile_project_to_dir` reescala texturas según gama (`LowRam32Bit`: `1536 px` estáticos / `2560 px` atlas / `24 MB` presupuesto auxiliar; `Balanced`: `2560 px` / `4096 px`; `HighQuality`: `4096 px`) y escribe los PNG optimizados directamente a `<cacheDir>/compiled_cache/*.png` liberando el buffer RGBA inmediatamente dentro del worker `rayon`. Rust no mantiene copias RGBA duplicadas (`texture_lru_cache` eliminado).
  4. **Exclusión mutua de Bitmaps en Kotlin**: `StudioViewModel` vacía `entityBitmaps` (liberando memoria nativa del editor, preescalada manteniendo `origWidth`/`origHeight`) **antes** de decodificar `compiledBitmaps` en gameplay, y viceversa al volver con `stopGameplayAndNavigate()`.
  5. **Fusión de audio en caliente**: Genera `compiled_mix.wav` y reporta `mergedAudioTracks`, `compiledAudioPath` y `estimatedAudioRamSavedKb` en `CompileReportJson`.
- `GameplayScreen.kt` sincroniza el loop de `withFrameNanos` llamando a `nativeStepGameplayBinary(dtSec, pressedMask, justPressedMask)` con zonas táctiles multi-touch de 4 carriles.

### E. Scripting Unificado Sin Ataduras (Lua 5.4 + TypeScript + JavaScript) y Sistema de Cámara
- Los scripts `.ts` pasan por `oxc_allocator` -> `oxc_parser` -> `oxc_semantic` -> `oxc_transformer` -> `oxc_codegen` en memoria, obteniendo JS ES2022 listo para `rquickjs`.
- Tanto Lua (`mlua`) como JS/TS (`rquickjs`) exponen la API completa sin restricciones (`pulse_scripting/src/bridge.rs`, `lua_vm.rs`, `js_vm.rs`):
  - **Cámara**: `setCameraZoom`, `setCameraBaseZoom`, `setCameraPos`, `getCameraX/Y/Zoom/BaseZoom`, `shakeCamera`.
  - **Entidades y Sprites/Textos Dinámicos**: `setEntityProp`, `getEntityProp`, `getEntityX`, `getEntityY`, `playEntityAnim` (soportando `angle`, `visible`, `flipX`, `zIndex`, `color`), `makeGraphic`, `makeHudGraphic`, `makeText`, `setTextString`, `cloneSprite`, `removeSprite`.
  - **Modcharts (`strumOverrides`)**: `setStrumProp`, `getStrumProp` (carriles `0..7`: `x`, `y`, `offsetX`, `offsetY`, `angle`, `scale`, `alpha`, `visible`, `scrollSpeed`).
  - **Tweens y Temporizadores Nativos**: `tweenEntityProp`, `tweenCameraPos`, `tweenCameraZoom`, `tweenStrumProp`, `cancelTween`, `runTimer`, `cancelTimer`.
  - **Gameplay, HUD y Audio**: `setHealth`, `addHealth`, `getHealth`, `setScore`, `addScore`, `getCombo`, `getMisses`, `getHits`, `getAccuracy`, `setScrollSpeed`, `setDownscroll`, `setHudVisible`, `setHudOpacity`, `setHudZoom`, `setTrackVolume`, `setTrackMute`, `getSongPosMs`, `getActiveBpm`, `getMustHitSection`.
  - **Hooks de Ciclo de Vida**: `onCreate`, `onUpdate`, `onStepHit`, `onBeatHit`, `onNoteHit`, `onNoteMiss`, `onCountdownTick`, `onEvent` (disparado por `chart.events`), `onTweenCompleted`, `onTimerCompleted`. Los errores de ejecución en tiempo real se reportan sin colgar el motor.
- **Sistema de Cámara Bidireccional (`StageCameraConfig` ↔ `camera_system.lua`)**: Cada proyecto incluye `camera_system.lua` (`id = "script_camera_system"`), sincronizado por bloques `-- [CAM_CFG_START]` .. `-- [CAM_CFG_END]` al editar visualmente en `StageEditorScreen` o manualmente en `ScriptStudioScreen`. Sigue a `opponent` y `player` cada N beats o por `mustHitSection` de `chart.json`, y `GameplayScreen` aplica `translate(pivot - cameraPos * s)` + `scale(cameraZoom, pivot)`.
- **Idle Sincronizado al BPM y Poses `Miss`**: En `gameplay_ops.rs`, las animaciones `Idle` de `Player` y `Opponent` reinician `anim_timer_sec` sobre el reloj de beats de `chart.json` (cada 1 o 2 beats según la duración del ciclo Sparrow) y añaden un pulso rítmico sutil para personajes PNG estáticos; al fallar una nota, el jugador ejecuta `missLeft/missDown/missUp/missRight` si están asignadas en Sparrow.
- Las mutaciones se encolan como `ScriptCommand` en `ScriptHostState` y `EngineSession::step_gameplay` las drena cada frame antes de enviar el frame binario **`BFGP` v2** (`140B` cabecera, `64B` por entidad con `rotationDeg`, `20B × 8` `strumOverrides`, `28B` por nota y `dynamicItems`) a Kotlin Compose.

### F. Optimización 60 FPS del Editor de Escenarios (`StageEditorScreen`)
- **Desacoplamiento 60 Hz (`StageLiveTransformState`)**: Los gestos táctiles en `StageCanvasPreview` (arrastre, pinch y redimensionado de marco de cámara/entidad) y los sliders de `StageEntityInspector` / `StageCameraInspector` mutan un titular `@Stable` local sin provocar recomposiciones de pantalla completa ni llamadas JNI por cada píxel; al soltar el dedo (`onDragEnd` / `onValueChangeFinished`) se realiza un único commit JNI.
- **Virtualización de capas (`StageLayersPanel`)**: Presenta la capa dedicada `Cámara (.lua)` y usa `LazyColumn` con `key = { it.id }` para renderizar únicamente las filas visibles incluso con decenas de elementos.
- **Canvas ligero (`StageCanvasPreview`)**: Pre-mide etiquetas `TextMeasurer` con `remember`, descarta entidades fuera del área visible (*viewport culling*), dibuja el marco de captura `(1280/zoom × 720/zoom)` en cian, usa `FilterQuality.Low` durante gestos activos o escenas con >20 elementos y busca toques en orden Z inverso sin copias de lista.
- **`update_stage_entity` y `update_stage_camera` livianos**: En `editor_ops.rs`, cuando `commit_save == false`, Rust responde `{"ok":true,...,"data":null}` evitando reserializar todo el `ProjectSnapshotJson`.

### G. HUD FNF en SVG (`resvg`), Letreros de Calificación y Note Splash (`GameplayScreen`)
- **Barra de Vida Vectorial + Íconos Reactivos**: Se retiró la barra superior ancha que cubría las flechas izquierdas del rival; en su lugar hay un botón de pausa compacto en la esquina superior derecha y una barra de vida `.svg` rasterizada vía `resvg` (con relleno dinámico rojo a la izquierda y verde a la derecha controlado por `1 - health`). Se posiciona arriba al centro en *downscroll* o abajo al centro en *upscroll*, respetando `hudVisible`, `hudOpacity` y `hudZoom`. Sobre el divisor rebotan al pulso `beatPhase` los íconos del Rival y del Jugador (con cambio automático a ícono perdiendo cuando la vida baja del 20% o supera el 80%, y volteo horizontal del jugador).
- **Contador debajo de la barra**: `Puntaje | Fallos | Precisión (%)` centrado directamente debajo de la barra de vida.
- **Ratings SVG sin dependencia de fuentes (`svg_skin.rs`)**: Los letreros `SICK!`, `GOOD!`, `BAD` y `MISS` están construidos exclusivamente con geometría `<path>` pura para que `resvg` los rasterice de forma nítida en Android sin requerir `fontdb` del sistema, apareciendo animados junto al personaje del jugador en espacio de mundo.
- **Note Splash en `Sick`**: Al acertar una nota en la ventana `Sick` (`|Δt| <= 45 ms`), `GameplayFrameState` dispara `splashEventId`/`splashLane` y `GameplayScreen` dibuja el estallido `.svg` (`resvg`) teñido con el color del carril sobre el receptor del jugador.

---

## 4. Política de Licencias y Propiedad Intelectual
- **Cero licencias copyleft fuertes/débiles**: Todas las dependencias Rust y Android son **MIT**, **Apache-2.0**, **Zlib** o equivalentes permisivas. Se descartaron librerías MPL-2.0 / GPL / LGPL (por ejemplo, se usa `lewton` + `hound` + `rmp3` en lugar de `symphonia`, y `texture2ddecoder` es `MIT OR Apache-2.0`).
- **Sin marcas registradas**: Prohibido incluir nombres de personajes, canciones o archivos con derechos de autor de terceros. La marca del proyecto es **BeatForge Studio** y las flechas por defecto son SVG geométricos originales (`sdk/assets/arrows/`).
