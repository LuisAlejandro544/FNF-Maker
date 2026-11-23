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
│   ├── Cargo.toml                       # Dependencias centralizadas (libc, jni, mlua, rquickjs, oxc, resvg, rayon, crunch, exoquant, texture2ddecoder, zstd, ebur128, realfft, rustfft, biquad, rubato)
│   ├── .cargo/
│   │   └── config.toml                  # Flags de enlazador Android (alineación 16KB para Android 15+)
│   └── crates/
│       ├── pulse_core/                  # Lógica pura del motor (sin UI ni Android)
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs               # Exportaciones públicas y pruebas unitarias Rust
│       │       ├── chart/               # Parser, editor y serializador universal `chart.json` modularizado
│       │       │   ├── mod.rs           # Reexportaciones públicas del módulo `chart`
│       │       │   ├── models.rs        # Tipos de chart, notas especiales (`Hurt`/`Instakill`/`Dodge`), secciones (`cameraSlot`) y eventos
│       │       │   ├── editor.rs        # Helpers y mutadores del editor de chart (`toggle_note`, BPM, secciones)
│       │       │   ├── parser.rs        # Parsers de formatos `chart.json` (Psych, V-Slice, Codename y eventos)
│       │       │   ├── serializer.rs    # Exportación universal limpia a `chart.json`
│       │       │   ├── camera_events.rs # Parser de eventos de cámara (`parse_chart_camera_event` V-Slice/Psych/Codename)
│       │       │   └── tests.rs         # Pruebas unitarias de parsing, serialización y eventos de cámara
│       │       ├── sparrow.rs           # Lector XML Sparrow (`page_index`), centrado universal (`normalize_and_center_animations`) y auto-armado de Idle/Direcciones + Miss + Death
│       │       ├── astc_loader.rs       # Decodificador puro Rust `.astc` (`texture2ddecoder`), dithering `RGB_565` Floyd-Steinberg, cuantización `exoquant` y codificador WebP (`save_compiled_webp`)
│       │       ├── atlas_json/          # Lectores `.json` y baker `Animation.json` divididos modularmente (con centrado unificado por defecto)
│       │       │   ├── mod.rs           # Punto de entrada del módulo, detección de formato y reexportaciones públicas
│       │       │   ├── transform.rs     # Álgebra afín 2D (`Affine2D`) y matrices/color (`ColorTransform2D`)
│       │       │   ├── direct_formats.rs# Parsers directos `spritemap1.json` y TexturePacker Hash/Array centrados por defecto
│       │       │   ├── animate_model.rs # Árbol de líneas de tiempo, capas, fotogramas y símbolos de `Animation.json`
│       │       │   └── animate_baker.rs # Rasterización por pose a atlas 4096×4096 empaquetado por estantes + centrado + tests
│       │       ├── stage.rs             # Escenario 1280×720, sistema multicámara (`Cam 0..3+` con target/FOV/pan), auto-dimensionado (`fit_dimensions_to_*`), anim_offsets y Z-order
│       │       ├── mod_descriptors.rs   # Parsers `.json`/`.xml` de personaje (`characters/*`) y escenario (`stages/*`: Psych/V-Slice/Codename)
│       │       ├── svg_skin.rs          # Flechas (incl. nota X `default_hurt_arrow_x_svg`), importador `NOTE_assets.png+.xml`, barra, íconos, ratings y Note Splash SVG vía resvg
│       │       ├── gfx_backend.rs       # Tipos base `RenderBackendKind` (`Canvas2d`/`Gles2`/`Gles3`/`Vulkan`) y `GfxCapabilities`
│       │       ├── project/             # Modelo `ModProject` y generadores de scripts Lua predeterminados
│       │       │   ├── mod.rs           # Reexportaciones públicas del módulo `project`
│       │       │   ├── models.rs        # Estructuras `ModProject`, metadatos, pistas de audio y `render_backend`
│       │       │   ├── lua_helpers.rs   # Utilidades compartidas de formato e interpolación para plantillas Lua
│       │       │   ├── camera_lua.rs    # Generador `camera_system.lua` V2 (`-- [CAM_RUNTIME_V2]`)
│       │       │   ├── stage_events_lua.rs # Generador `stage_events.lua` para eventos de escenario y cámara
│       │       │   └── ui_blackout_lua.rs  # Generador `ui_blackout.lua` para apagones e intro HUD (`3-2-1-GO`)
│       │       ├── frame_memory.rs      # Memoria por frame / ventana deslizante de notas
│       │       ├── script_optimizer.rs  # Poda segura preservando API completa (flashes, swap, preload/retire, personaje)
│       │       ├── atlas_repack.rs      # Optimizador y reempaquetador (`crunch`): alpha-trim, frames muertos `(0,0) 1x1`, deduplicación exacta, poda segura y paginación multi-página (`_p1.._p5`)
│       │       ├── hot_compile/         # Compilación incremental modularizada (`HotCompileManifest` v6)
│       │       │   ├── mod.rs           # Orquestación pública y reexportaciones de compilación en caliente
│       │       │   ├── fingerprints.rs  # Huellas FNV-1a de texturas, pantalla física, zoom máx., audio y scripts
│       │       │   ├── manifest.rs      # Lectura/escritura atómica de `HotCompileManifest` v6 en disco (`TexturePixelFormat`, `timeline_windows`)
│       │       │   ├── analysis.rs      # Planificación incremental, `compute_timeline_asset_lifecycle` (4s/1.5s), zoom máx. y escala `Lanczos3` por pantalla
│       │       │   ├── pipeline.rs      # Pre-escalado `Lanczos3`, `RGB_565` + `exoquant`, mega-atlas multi-página WebP VP8L, caché `.bfaud`/`.luac` y reporte
│       │       │   └── tests.rs         # Pruebas unitarias de huellas, invalidación, timeline lifecycle, dithering/cuantización y pre-escalado por viewport
│       │       ├── disk_cache.rs        # Presupuestos 65%/35% por cuota del sistema, limpieza plana heredada y desalojo LRU en disco
│       │       └── vanilla.rs           # Contenedor portátil Vanilla (`.mod`) con Zstandard, PNG->WebP lossless, dedup y FNV-1a
│       │
│       ├── pulse_audio/                 # Subsistema de audio de baja latencia y DSP 100% Rust puro
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs               # AudioEngineHandle, AudioHardwareProbe, adaptive_master y streaming
│       │       ├── decoder.rs           # Decodificación OGG/WAV/MP3 a PCM i16 compacto (50% menos RAM) y resampleo
│       │       ├── audio_cache.rs       # Caché binario `.bfaud` (picos de onda + PCM i16) en `cacheDir/audio_cache/` con escritura atómica y LRU
│       │       ├── streaming.rs         # Fusión en caliente a WAV único (`compiled_mix.wav`) + streaming SPSC 512 KB desde disco
│       │       ├── adaptive_master.rs   # Adaptador Acústico (`ebur128` LUFS + `realfft` 5 bandas + `biquad` IIR + `rubato` FftFixedIn + caché)
│       │       ├── mixer.rs             # Mezclador lock-free + reloj maestro + countdown BPM + síntesis SFX Game Over
│       │       └── rust_audio_backend.rs # Backend Android AAudio en Rust puro (dlopen C ABI + probe hardware, cero C++) + fallback
│       │
│       ├── pulse_scripting/             # Motores de scripting comunitario unificados sin ataduras
│       │   ├── Cargo.toml
│       │   └── src/
│       │       ├── lib.rs               # ModScriptRuntime, `precompile_scripts_with_cache` (`.luac`/`.js`), hooks (`onGameOver`, `noteType`) y validador
│       │       ├── bridge.rs            # Cola thread-safe de ScriptCommand (multicámara, modcharts, flashes, Game Over, swap/RAM, personaje, tweens, HUD/audio)
│       │       ├── lua_vm/              # VM Lua 5.4 (`mlua`) modularizada por categoría de API
│       │       │   ├── mod.rs           # Inicialización Lua 5.4, `precompile_bytecode`, ejecución de hooks y watchdog
│       │       │   ├── bindings_world.rs  # Bindings Lua de cámara (`switchCamera`, `setCameraFollowPos`, `getGfSection`), escenario y sprites
│       │       │   ├── bindings_hud_fx.rs # Bindings Lua de HUD, flashes, UI blackout, tweens, notas especiales, Game Over y audio
│       │       │   └── tests.rs         # Pruebas unitarias de ejecución Lua y watchdog
│       │       ├── ts_transpiler.rs     # Compilador TS -> ES2022 JS y validador de sintaxis (`validate_js_syntax`) vía oxc 0.158
│       │       └── js_vm/               # VM ES2022 (`rquickjs`) modularizada con worker dedicado
│       │           ├── mod.rs           # Interfaz pública `JsVm` y despacho hacia el worker QuickJS-NG
│       │           ├── bindings_world.rs  # Bindings JS de cámara multicámara, escenario, personaje y sprites
│       │           ├── bindings_hud_fx.rs # Bindings JS de HUD, flashes, UI blackout, tweens, Game Over y audio
│       │           ├── worker.rs        # Hilo worker `rquickjs` con `set_interrupt_handler` watchdog (250ms/25ms)
│       │           └── tests.rs         # Pruebas unitarias de VM JS/TS y límites del watchdog
│       │
│       └── pulse_android/               # Puente nativo Android (`libpulse_android.so`)
│           ├── Cargo.toml               # Crate cdylib + rlib con jni, android_logger, serde_json e image
│           ├── build.rs                 # Script de compilación limpio (sin C++ ni GameActivity)
│           └── src/
│               ├── lib.rs               # Punto de entrada crate `pulse_android`: estado global `SESSION`, `JNI_OnLoad` y registro modular
│               ├── jni/                 # Puentes JNI (`Java_com_beatforge_studio_NativeEngineBridge_*`) separados por dominio
│               │   ├── mod.rs           # Registro de submódulos JNI (activado bajo `target_os = "android"`)
│               │   ├── project_ops.rs   # JNI de ciclo de vida del motor, proyectos, caché y exportación Vanilla `.mod`
│               │   ├── chart_ops.rs     # JNI de chart, secciones (`cameraSlot`), notas especiales, eventos y metadata JSON
│               │   ├── audio_ops.rs     # JNI de pistas de audio, formas de onda, volumen/mute y reproducción del editor
│               │   ├── stage_sprite_ops.rs # JNI de escenario, multicámara (`Cam 0..3+`), sprites Sparrow/Animate y offsets
│               │   ├── script_skin_ops.rs  # JNI de scripts Lua/TS/JS, flechas SVG/`NOTE_assets`, íconos y barra de vida
│               │   └── gameplay_gfx_ops.rs # JNI de compilación incremental, simulación gameplay `BFGP` v2, debug y probe GPU (`gfx`)
│               └── session/             # `EngineSession` modularizado por responsabilidad (sin caché RGBA duplicado)
│                   ├── mod.rs           # Struct `EngineSession`, rutas `compiled_cache/<id>/` y `audio_cache/`, cuota de caché y helpers
│                   ├── dto.rs           # DTOs hacia Kotlin (incl. `StageCameraSlotSnapshotJson`, `cameraSlot`, `repacked_entity_ids`, `compiled_texture_pages`, `texture_pixel_formats`, `timeline_windows`, `page_index`)
│                   ├── editor_ops.rs    # Operaciones de editor (`select_stage_camera`, `update_camera_slot`, `add/remove_custom_camera`, `set_section_camera_slot`, `import_stage_config`, `stage_events.lua`)
│                   ├── gfx_probe.rs     # Sondeo dinámico `dlopen` en Android 32/64b de `libEGL.so`, `libGLESv2.so`, `libGLESv3.so` y `libvulkan.so`
│                   ├── sprite_ops.rs    # Importación PNG/ASTC/WebP, Sparrow XML, Animate, auto-dimensionado en escenario, `import_character_config` y `set_entity_anim_offset`
│                   ├── persistence_ops.rs # Guardado/carga (normalización de atlas, ranuras `Cam 0..2` y limpieza de cachés v6), adopción `assets/`, GC, `.bfaud` y Vanilla `.mod`
│                   └── gameplay_ops/      # Compilación incremental y simulación de gameplay modularizada
│                       ├── mod.rs         # Coordinador `start/pause/stop/retry_game_over/trigger_test_game_over`, colas ordenadas de timeline y BFGP v2 (`152B`)
│                       ├── hot_compile_ops.rs # `hot_compile_with_progress_and_viewport` en `compiled_cache/<projectId>/` con cuota y desalojo LRU multi-proyecto
│                       ├── audio_adapt_ops.rs # Fase independiente post-compilación: sondeo hardware + EBU R128/FFT + plan DSP Biquad/Rubato
│                       ├── script_sync.rs # `tweens`, timers, `SwitchCameraSlot`/`SetCameraFollowLock`, `flash`, `GameOver`, swaps y drenaje `ScriptCommand`
│                       ├── simulation.rs  # Simulación por frame (drenaje de colas timeline preload/retire, multicámara + `chart.json`, notas X, Game Over, Botplay/Práctica)
│                       └── packet_encoder.rs # Serialización JSON y binaria `BFGP` v2 (`152B` + bits 8..15 `page_index` en `e_flags`, bits 9..13 GameOver/Debug + `noteKindCode`)
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
│           ├── AndroidManifest.xml      # `sensorLandscape`, `<uses-feature>` opcional para OpenGL ES 2.0 y Vulkan, iconos y permisos Android 8..15
│           ├── res/
│           │   ├── drawable/
│           │   │   └── ic_launcher_monochrome.xml # Silueta monocroma VectorDrawable (Android 13+)
│           │   ├── mipmap-anydpi-v26/   # Adaptive Icons (`ic_launcher.xml`, `ic_launcher_round.xml`)
│           │   ├── mipmap-mdpi..xxxhdpi/# Iconos PNG `ic_launcher`, `ic_launcher_round`, `_foreground` y `_background`
│           │   └── values/strings.xml
│           └── kotlin/com/beatforge/studio/
│               ├── MainActivity.kt      # ComponentActivity + Lobby inicial/Pantalla de carga + OpenDocument/CreateDocument (.json, .mod, NOTE_assets)
│               ├── CrashReportActivity.kt # Pantalla aislada (:crash) para copiar diagnóstico técnico en teléfono
│               ├── GameplayFrameState.kt# Decoder binario BFGP v2 (`152B` cabecera: GameOver, Botplay, Práctica, `noteKindCode`, `pageIndex` bits 8..15, flash, preload/retired)
│               ├── NativeEngineBridge.kt# Declaraciones `external fun` 1:1 hacia `libpulse_android.so` (incl. Game Over, Debug, Arrow Atlas y Backend GPU)
│               ├── SafFileBridge.kt     # Staging temporal de URIs SAF en `cacheDir/import_staging/` y barrido al arranque
│               ├── StudioModels.kt      # Data classes Kotlin (`NoteSpecialKind`, `EntityLifecycleWindowSnapshot`, `CompileReportSnapshot` con `compiledTexturePages`/`texturePixelFormats`, `ArrowSkinCatalog`)
│               ├── StudioUiState.kt     # Estado reactivo (`hasSelectedProject`, `isLoadingProject`, `debug*`, `stagedArrowAtlas*`) y `LoadedEntityBitmap` (`extraPages` + `pageAt()`)
│               ├── StudioViewModel.kt   # Coordinador principal (`StateFlow`) que delega en `viewmodel/*` sobre `Dispatchers.IO`
│               ├── audio/               # Sondeo físico del audio de Android (sin bloqueos en UI)
│               │   └── AndroidAudioDeviceProbe.kt # Consulta `AudioManager` + `AudioDeviceInfo` en `Dispatchers.IO` (ruta, canales, mono, burst)
│               ├── gfx/                 # Capa modular de preparación GPU (OpenGL ES 2.0/3.x y Vulkan)
│               │   ├── RenderBackendModels.kt # Modelos Kotlin de backend seleccionado y capacidades de drivers detectados
│               │   ├── VulkanProbeBridge.kt   # Sondeo ligero Android PackageManager (GLES / Vulkan) enlazado con el reporte Rust
│               │   └── GlesSurfaceHost.kt     # Contenedor base GLSurfaceView (ES 2.0 / 3.x) listo para futuro renderizado GPU
│               ├── viewmodel/           # Submódulos especializados del ViewModel
│               │   ├── StudioBitmapLoader.kt      # Detección 32/64-bit, `LruCache` 1:1 Sparrow, `Bitmap.Config.RGB_565`, decodificación multi-página y alineación `repackedEntityIds`
│               │   ├── StudioEditorDelegate.kt    # Lobby/Carga de proyectos, cuota `StorageManager`, staging SAF, `NOTE_assets.png+.xml`, `.mod` y backend GPU
│               │   └── StudioGameplayCoordinator.kt # Ciclo RAM (`nativeHotCompileWithViewport`), fase `AudioAdaptScreen`, colas timeline/precarga 4s y reciclaje `extraPages` a 3 frames
│               └── ui/
│                   ├── theme/
│                   │   └── StudioTheme.kt         # Paleta estudio carbón mate, naranja estudio, ámbar y verde Material 3
│                   ├── components/
│                   │   └── StudioNavBar.kt        # Riel vertical izquierdo compacto (`StudioNavRail`) con Material Icons
│                   ├── popups/
│                   │   ├── ArrowSkinDialog.kt     # Modal para personalizar flechas `.svg` o importar 2 pasos `NOTE_assets.png + .xml` (+ vista Nota X)
│                   │   ├── CharacterIconsDialog.kt# Modal para íconos normal/perdiendo (auto-split 2-frame) y barra de vida .svg
│                   │   ├── ChartJsonDetailsDialog.kt # Modal para editar metadatos `chart.json`, `changeBPM` y eventos
│                   │   ├── ChartStepCueDialog.kt  # Modal en columna `#` (5 pestañas: Swaps/RAM, Flash, Personajes, UI Blackout y Cámara directa del chart)
│                   │   └── ModFilesGuideDialog.kt # Modal guía: qué archivo va en cada pestaña y en qué carpeta de mod suele estar (incl. `NOTE_assets`)
│                   └── screens/
│                       ├── HubScreen.kt           # Metadatos del mod, selector `Motor Gráfico (Terreno GPU)`, métricas y accesos directos
│                       ├── ProjectsScreen.kt      # Lobby principal al arranque + gestión, creación, copia, borrado, `.mod` e insignia GPU
│                       ├── ProjectLoadingScreen.kt# Pantalla dedicada de carga al abrir/crear/importar un mod desde el lobby
│                       ├── ChartScreen.kt         # Coordinador del editor manual de 8 carriles + notas especiales + cámara por sección (`Cam N`) + eventos `#`
│                       ├── chart/                 # Subcomponentes modulares del editor de chart
│                       │   ├── ChartTimelineModels.kt # Colas sustain continuas, mapa `#` (incl. cues `Camera`), lookup O(log N) del hilo y duración total
│                       │   ├── ChartPlayheadState.kt  # Estado `@Stable` a 60 Hz del hilo reproductor + toggle Seguir/Libre sin recomponer filas
│                       │   ├── ChartToolsPanel.kt     # Panel izquierdo: selector tipo de nota, transporte, salto de sección con insignia `Cam N` y `stage_events.lua`
│                       │   └── ChartTimelineGrid.kt   # Rejilla DAW continua, dropdown `Auto / Cam N` por sección, cues `#`, scrubber y hilo 60 FPS
│                       ├── AudioScreen.kt         # Mezclador multicanal de hasta 3 pistas + forma de onda + toggle del Adaptador Acústico
│                       ├── SpriteLabScreen.kt     # PNG/ASTC + XML/JSON, slots `Miss` + `Death Start/Loop/Confirm`, `Posición (.json/.xml)`, `Offset Pose X/Y` y visor 1:1
│                       ├── StageEditorScreen.kt   # Coordinador del editor visual de escenario en 3 columnas (60 FPS) + `Escena .json/.xml` + multicámara
│                       ├── stage/                 # Subcomponentes modulares del editor de escenario
│                       │   ├── StageLiveTransformState.kt # Estado en vivo 60Hz aislado para entidades y ranura de cámara activa (`resolveCameraSlotPreviewCenter`)
│                       │   ├── StageLayersPanel.kt    # Panel izquierdo con capa `Cámara (.lua)` (ranuras + modo), `Escena .json/.xml`, ayuda `?` y `LazyColumn`
│                       │   ├── StageCanvasPreview.kt  # Lienzo 16:9 con marco activo de cámara, marcos secundarios (`C0..CN`), culling y arrastre táctil
│                       │   ├── StageCameraInspector.kt# Inspector Multicámara (`Cam 0..3+`, `+ Cam`, borrar `Cam 3+`, pestañas `Ranura & FOV` y `Modo & Chart`, cruceta)
│                       │   └── StageEntityInspector.kt# Inspector de entidad: rol 2x2, cruceta 1/10/35px, sliders y flipX
│                       ├── ScriptStudioScreen.kt  # Editor nativo Lua / TS / JS (`camera_system.lua` y `stage_events.lua`) con referencia API
│                       ├── HotCompileScreen.kt    # Pantalla de carga con barra y círculo determinados (`0%–100%`) + fase en vivo
│                       ├── AudioAdaptScreen.kt    # Pantalla independiente pre-gameplay: hardware/AAudio, EBU R128 + FFT 5 bandas y plan DSP
│                       ├── GameplayScreen.kt      # Coordinador jugable 60 FPS (`withFrameNanos`), medición FPS, flash HUD y ocultamiento HUD limpio
│                       └── gameplay/              # Submódulos especializados de `GameplayScreen`
│                           ├── GameplayRenderPrimitives.kt # Pools sin GC, `GameplayTouchMaskState` y geometría de carriles
│                           ├── GameplayWorldRenderer.kt    # Dibujo 60 FPS de escena, foco Game Over FNF, flechas X (`hurt_x`), notas y Splash
│                           ├── GameplayTouchOverlay.kt     # Entrada multi-touch de 4 carriles (desactivada durante pausa/fin/Game Over)
│                           └── GameplayHudOverlay.kt       # HUD SVG, overlay Game Over (toque para reintentar), overlay Debug RAM/FPS y Pausa Debug
│
├── sdk/                                 # Recursos y SDK para creadores de mods y pruebas locales
│   ├── assets/arrows/                   # Flechas SVG originales libres de derechos (`left`, `down`, `up`, `right`, `hurt_x`)
│   ├── samples/                         # Scripts de muestra (.lua, .ts, .js)
│   └── ts/src/
│       ├── beatforge-sdk.d.ts           # Tipado TypeScript oficial del API completo (`switchCamera`, `setCameraFollowPos`, `getGfSection`, `triggerGameOver`, `noteType`)
│       ├── engine-tools.ts              # Validación, multicámara, `Lanczos3` viewport scaling, `TimelineAssetLifecycle`, `RGB_565`+`exoquant`, multi-page splitting, adaptador DSP, `optimizeSparrowAtlasPlan`, `.mod`, `BFGP` v2
│       └── engine-tools.test.ts         # 60 pruebas unitarias ejecutables con `npx tsx --test`
│
├── branding/                            # Identidad visual "Beat Hammer" (SVGs maestros, generador y PNGs de tienda/miniatura)
│   ├── generate_pngs.mjs                # Conversor automático SVG -> PNG (resvg-js) para Android y tiendas
│   ├── svg/                             # Fuentes vectoriales (.svg): icono, redondo, foreground/background/monochrome, banner y miniatura
│   └── png/                             # PNGs listos para Uptodown/tiendas (512, 1024, banner 1024x500) y miniaturas (1280x720, 640x360)
│
└── Documentación raíz
    ├── README.md
    ├── ROADMAP.md
    ├── STRUCTURE.md
    ├── AI_CONTEXT.md
    ├── AGENTS.md
    └── README_AI.md
```
