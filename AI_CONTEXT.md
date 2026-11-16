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

### B. Reloj Maestro de Audio, Cuenta Regresiva BPM y 1–3 Pistas
- `AudioMixer` mantiene hasta 3 `TrackSlotConfig` + `Arc<DecodedTrack>` (`slots[0..3]`).
- Al iniciar `start_gameplay()`, `AudioMixer::start_countdown(bpm)` calcula `beat_frames = (sample_rate * 60.0 / bpm)` y fija `countdown_remaining_frames = beat_frames * 4` (`AtomicU64`).
- El callback en tiempo real (`render_audio_frames`, invocado tanto por AAudio como por `FallbackClockThread` en Android 7.x API 24–25) sintetiza sin mutex bloqueantes tonos vocales/formantes para los 4 tiempos (`"3"`, `"2"`, `"1"`, `"¡YA!"`) y transiciona con precisión de muestra a `0.0 ms` sobre las pistas de audio.
- `position_ms()` retorna tiempo negativo (`-4 * beat_ms` hasta `0.0`) durante la cuenta regresiva; `GameplayStepJson` expone `activeBpm` (resuelto con `chart.bpm_at_ms(song_pos_ms)`), `countdownStage`, `countdownLabel` y `countdownProgress` para el overlay animado de `GameplayScreen.kt`.

### C. Ensamblado de Spritesheets y Formatos Exóticos (`.png`, `.astc`, `.xml`, `.json`)
- **Imágenes (`.png`, `.jpg`, `.webp`, `.astc`)**: `astc_loader.rs` valida la cabecera mágica `[0x13, 0xAB, 0xA1, 0x5C]`, decodifica bloques `.astc` vía `texture2ddecoder::decode_astc` a `RGBA8` y `session.rs` guarda una copia PNG en `cacheDir/decoded_textures/<id>.png` para que Compose la cargue con `BitmapFactory` y `HotCompile` pueda reescalarla sin ruta extra.
- **Atlas (`.xml` y `.json`)**: `atlas_json.rs` autodetecta:
  1. **TexturePacker / Aseprite JSON** (Hash o Array), convirtiéndolo a `SparrowAtlas` (incluyendo soporte de frames `rotated = true`, rotados `-90°` al renderizar en `SpriteLabScreen.kt` y `GameplayScreen.kt`).
  2. **Adobe Animate `spritemap1.json`** (`ATLAS.SPRITES`): genera spritesheet de respaldo y registra sus piezas atómicas.
  3. **Adobe Animate `Animation.json`** (`AN`/`SD`/`TL`/`M3D`/`M`): cuando una entidad posee tanto `spritemap1` como `Animation.json` (en cualquier orden de importación), `try_bake_animate_for_entity` rasteriza las poses mediante transformaciones afines en software, las empaqueta con *shelf packing* en `<cacheDir>/animate_baked/<id>_baked.png` y construye su `SparrowAtlas` listo para `Idle`, `Left`, `Down`, `Up`, `Right`.

### D. Compilación en Caliente (`Hot Compile`)
- Disparada por `StudioViewModel.startCompileAndPlay()` en `Dispatchers.IO`, detectando automáticamente la RAM total del teléfono y el ABI (32 vs 64 bits) para elegir `tierCode` e invocando `NativeEngineBridge.nativeHotCompile(tierCode)` sobre `rayon::par_iter()`:
  1. Clasifica entidades según su rol (`Background`, `Player`, `Opponent`, `Decoration`) y orden Z.
  2. **Poda de invisibles**: Descarta cualquier entidad con `!visible` u `opacity <= 0.01` fuera de escena que no sea referenciada por scripts, reportando la RAM/VRAM ahorrada.
  3. **Reescalado automático por gama de teléfono** (`LowRam32Bit`: máx. 1024px, `Balanced`: 2048px, `HighQuality`: 4096px), cargando tanto PNGs normales como texturas `.astc` decodificadas o hojas Animate horneadas en `<cacheDir>/compiled_cache/*.png`, y rasteriza las flechas `.svg` con `resvg`.
- `GameplayScreen.kt` sincroniza el loop de `withFrameNanos` llamando a `nativeStepGameplay(dtSec, pressedMask, justPressedMask)` con zonas táctiles multi-touch de 4 carriles.

### E. Scripting Unificado (Lua 5.4 + TypeScript + JavaScript)
- Los scripts `.ts` pasan por `oxc_allocator` -> `oxc_parser` -> `oxc_semantic` -> `oxc_transformer` -> `oxc_codegen` en memoria, obteniendo JS ES2022 listo para `rquickjs`.
- Tanto Lua (`mlua`) como JS/TS (`rquickjs`) registran exactamente las mismas funciones globales: `log`, `setCameraZoom`, `shakeCamera`, `setEntityProp`, `playEntityAnim`, `getSongPosMs`.
- Las mutaciones se encolan como `ScriptCommand` en `ScriptHostState` y `EngineSession::step_gameplay` las drena cada frame antes de enviar el `GameplayStepJson` a Kotlin Compose.

---

## 4. Política de Licencias y Propiedad Intelectual
- **Cero licencias copyleft fuertes/débiles**: Todas las dependencias Rust y Android son **MIT**, **Apache-2.0**, **Zlib** o equivalentes permisivas. Se descartaron librerías MPL-2.0 / GPL / LGPL (por ejemplo, se usa `lewton` + `hound` + `rmp3` en lugar de `symphonia`, y `texture2ddecoder` es `MIT OR Apache-2.0`).
- **Sin marcas registradas**: Prohibido incluir nombres de personajes, canciones o archivos con derechos de autor de terceros. La marca del proyecto es **BeatForge Studio** y las flechas por defecto son SVG geométricos originales (`sdk/assets/arrows/`).
