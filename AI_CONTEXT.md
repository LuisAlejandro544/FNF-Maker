# AI Context — Arquitectura Técnica de BeatForge Studio

Este archivo concentra el contexto técnico profundo necesario para entender cómo interactúan Rust, Lua, TypeScript/JavaScript, Google Oboe y Android (`GameActivity` + Kotlin Coroutines) dentro de **BeatForge Studio**.

---

## 1. Dualidad del Entorno (Web Shell vs Motor Nativo Android)

1. **Entorno Web Vite (`/src/App.tsx`)**:
   - Debe mantenerse estrictamente como un `"Hello World"` mínimo.
   - Existe para que `npm run build` y el servidor de desarrollo en el puerto `3000` dentro de AI Studio compilen y arranquen sin errores.
   - **No** debe mezclarse aquí la lógica pesada del motor móvil.
2. **Motor Móvil Real (`/engine`, `/android`, `/sdk`)**:
   - Toda la lógica de procesamiento, renderizado, audio y UI táctil horizontal vive en Rust dentro del workspace `/engine`.
   - El contenedor del entorno web no tiene `rustc` ni Android NDK instalados localmente; la compilación nativa de las arquitecturas `arm64-v8a` (64 bits) y `armeabi-v7a` (32 bits) se ejecuta mediante GitHub Actions (`.github/workflows/build-apk.yml`).

---

## 2. Versiones Exactas y Quirks de APIs Verificadas

| Componente | Versión | Detalles Críticos de API |
| :--- | :--- | :--- |
| `eframe` / `egui` | `0.37.0` | `eframe::App` usa `fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame)` (ya no `update`). `TopBottomPanel` se reemplazó por `egui::Panel::top(...).exact_size(...)` + `.show_inside(ui, ...)`, y `CentralPanel::default().show_inside(ui, ...)`. |
| `winit` | `0.30.13` | `eframe` 0.37 usa `winit 0.30` con feature `android-game-activity` y `winit::platform::android::activity::AndroidApp`. |
| Audio Android (`AAudio` Rust puro) | `libc` (`dlopen` C ABI) | En `rust_audio_backend.rs`, se eliminó `oboe` C++ (`oboe-sys`) para evitar conflictos de STL (`c++_static`/`libc++_shared`). Carga `libaaudio.so` vía `dlopen`/`dlsym` en Rust puro (API 26+) con reloj de respaldo en Rust para Android 7.x (`minSdk = 24`). |
| `oxc_*` | `0.158.0` | `ParserReturn`, `SemanticBuilderReturn` y `TransformerReturn` exponen `.diagnostics` (`oxc_diagnostics::Diagnostics`) en lugar de `.errors`. Usar `.diagnostics.has_errors()` y `.diagnostics.errors()`. |
| `mlua` | `0.12.2` | `Lua::new()` con `lua54` y `vendored`; `func.call::<()>(args)`. |
| `rquickjs` | `0.14.0` | `Function::new(ctx.clone(), closure)` + `func.call::<_, ()>(args)` con `features = ["rust-alloc", "macro", "bindgen"]`. |
| `resvg` | `0.50.0` | Rasteriza SVG sobre `resvg::tiny_skia::Pixmap` (RGBA premultiplicado que `svg_skin.rs` des-premultiplica a RGBA recto para `egui::ColorImage`). |
| `cargo-ndk` | `v4.x` | El nivel de API se pasa con `--platform 24` (NO `-p 24`, porque `-p` en v4 es `--package`). |

---

## 3. Flujos Internos Clave del Motor

### A. Importación de Archivos desde Android (SAF + Coroutines + JNI)
1. En Rust (`pulse_ui`), el usuario pulsa un botón de importar (Chart, Pista 1–3, PNG, XML, SVG o Script).
2. `AndroidSafPicker::request_import` guarda `(ImportKind, UnboundedSender<ImportedFileEvent>)` en `PENDING_PICK_TX` y llama vía JNI a `MainActivity.launchSafPicker(kindCode)`.
3. `MainActivity` abre el selector nativo de Android (`ActivityResultContracts.OpenDocument`) en el hilo UI.
4. Al recibir el `Uri`, lanza una corrutina en **`Dispatchers.IO`** (`lifecycleScope.launch(Dispatchers.IO)`) para no bloquear el hilo principal ni congelar el renderizado.
5. `SafFileBridge.copyUriToInternalCache` resuelve el nombre real (`OpenableColumns.DISPLAY_NAME`) y copia el stream a `filesDir/mod_imports/`.
6. Luego invoca `@JvmStatic external fun nativeOnFilePicked(localPath: String)` -> `Java_com_beatforge_studio_SafFileBridge_nativeOnFilePicked` en Rust, enviando el `ImportedFileEvent` al canal `tokio::sync::mpsc`.
7. `AppState::poll_background_events` procesa el archivo en el siguiente frame y dispara decodificación/carga asíncrona en `tokio`.

### B. Reloj Maestro de Audio (Google Oboe) y 1–3 Pistas
- `AudioMixer` mantiene hasta 3 `TrackSlotConfig` + `Arc<DecodedTrack>` (`slots[0..3]`).
- El callback en tiempo real de Oboe (`on_audio_ready`) avanza `played_frames: AtomicU64` **sin mutex bloqueantes** (`try_read` / atomics), garantizando cero cortes en 32 y 64 bits.
- `position_ms()` deriva directamente de `played_frames / sample_rate`, manteniendo notas, scripts y animaciones sincronizados con precisión de muestra.
- Si no hay pistas importadas, activa un metrónomo/base sintetizada demo para que el mod siempre pueda probarse al instante.

### C. Ensamblado Automático de Spritesheets (`.png` + `.xml`)
- `SparrowAtlas::from_xml_str` parsea los `<SubTexture name="..." x="..." y="..." width="..." height="..." .../>` usando `roxmltree`.
- Agrupa frames por prefijo normalizado (quitando dígitos finales como `0000`, `0001`) y los asigna heurísticamente a los 5 estados base: `Idle`, `Left`, `Down`, `Up`, `Right`, permitiendo ajuste manual en `SpriteLab` si el mod usa nombres personalizados.

### D. Compilación en Caliente (`Hot Compile`)
- Ejecutada en `tokio_rt.spawn_blocking` usando `rayon::par_iter()`:
  1. Clasifica entidades según su rol (`Background`, `Player`, `Opponent`, `Decoration`) y orden Z.
  2. **Poda de invisibles**: Descarta cualquier entidad con `!visible` u `opacity <= 0.01` que no sea referenciada dinámicamente, reportando la RAM/VRAM ahorrada.
  3. **Reescalado por gama de dispositivo** (`LowRam32Bit`: máx. 1024px, `Balanced`: 2048px, `HighQuality`: 4096px) y rasterización de flechas `.svg` a RGBA8888.

### E. Scripting Unificado (Lua 5.4 + TypeScript + JavaScript)
- Los scripts `.ts` pasan por `oxc_allocator` -> `oxc_parser` -> `oxc_semantic` -> `oxc_transformer` -> `oxc_codegen` en memoria, obteniendo JS ES2022 listo para `rquickjs`.
- Tanto Lua (`mlua`) como JS/TS (`rquickjs`) registran exactamente las mismas funciones globales: `log`, `setCameraZoom`, `shakeCamera`, `setEntityProp`, `playAnim`, `getSongPosMs`.
- Las mutaciones no tocan la UI desde dentro del VM: se encolan como `ScriptCommand` en `ScriptHostState` y `gameplay_screen.rs` las drena cada frame.

---

## 4. Política de Licencias y Propiedad Intelectual
- **Cero licencias copyleft fuertes/débiles**: Todas las dependencias Rust y Android son **MIT**, **Apache-2.0**, **Zlib** o equivalentes permisivas. Se descartaron librerías MPL-2.0 / GPL / LGPL (por ejemplo, se usa `lewton` + `hound` + `rmp3` en lugar de `symphonia`).
- **Sin marcas registradas**: Prohibido incluir nombres de personajes, canciones o archivos con derechos de autor de terceros. La marca del proyecto es **BeatForge Studio** y las flechas por defecto son SVG geométricos originales (`sdk/assets/arrows/`).
