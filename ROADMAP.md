# Roadmap — BeatForge Studio

Hoja de ruta técnica y de producto para **BeatForge Studio** (`com.beatforge.studio`).

---

## Fase 1 — Núcleo del Motor, UI Horizontal y CI Manual 32/64 bits (Actual)

- [x] **Workspace Rust modular (`/engine`)**:
  - `pulse_core`: Parser de charts `.json`, ensamblador automático Sparrow `.png` + `.xml`, escena `1280×720` con roles y orden Z, rasterizador SVG `resvg`, compilador en caliente con `rayon` y descarte de assets invisibles.
  - `pulse_audio`: Mezclador lock-free de hasta 3 pistas sincronizado por muestras con **Google Oboe** y decodificadores permisivos (`lewton`, `hound`, `rmp3`).
  - `pulse_scripting`: VM **Lua 5.4** (`mlua`), transpilador **TypeScript** nativo en móvil (`oxc 0.158`) y VM **JavaScript ES2022** (`rquickjs`), compartiendo hooks (`onCreate`, `onUpdate`, `onBeatHit`, `onNoteHit`, `onNoteMiss`).
  - `pulse_ui`: Interfaz multi-pantalla horizontal (`Hub`, `Chart y Música`, `Sprites XML`, `Escenario`, `Scripts`, `Compilación en Caliente`, `Gameplay`) y popup modal de skins `.svg`.
  - `pulse_android`: Entrada nativa `GameActivity` + puente JNI con corrutinas Kotlin (`Dispatchers.IO`).
- [x] **Proyecto Android (`/android`)**:
  - `minSdk 24`, `compileSdk 35`, `targetSdk 35`, `ndkVersion 27.2.12479018`.
  - Integración obligatoria de `cargo-ndk` en `preBuild` + copia automática de `libc++_shared.so`.
  - Generación de 2 APKs separados (`arm64-v8a` 64 bits y `armeabi-v7a` 32 bits).
- [x] **SDK & Pruebas (`/sdk`)**:
  - Definiciones `.d.ts`, scripts de ejemplo (`.lua`, `.ts`, `.js`), flechas SVG originales y suite de pruebas unitarias TypeScript (`tsx --test`).
- [x] **Workflow GitHub Actions Manual (`build-apk.yml`)**:
  - Disparo exclusivo `workflow_dispatch`, caché de Rust/Cargo/Gradle y subida de los 2 APKs Debug.

---

## Fase 2 — Empaquetado de Mods y Mejoras de Juego en Tiempo Real

- [ ] **Exportador e Importador de Paquetes `.bfmod` (ZIP)**:
  - Empaquetar chart, 1–3 audios, hojas `.png`/`.xml`, escenario JSON, flechas `.svg` y scripts en un único archivo portátil compartible entre teléfonos.
- [ ] **Notas Sostenidas (Hold Notes) y Eventos de Cámara Avanzados**:
  - Renderizado de colas de duración (`sustain_ms`) sincronizadas con el scroll speed y soporte para eventos de cambio de cámara por sección (`mustHitSection`).
- [ ] **Previsualización de Animación XML con Offsets por Frame**:
  - Ajuste interactivo de `offset_x` / `offset_y` por animación (`Idle`, `Left`, `Down`, `Up`, `Right`) directamente en `SpriteLab`.
- [ ] **Persistencia Automática de Proyectos**:
  - Guardado y carga de proyectos locales en `filesDir` con historial reciente en el Hub.

---

## Fase 3 — Herramientas Avanzadas de Escenario y Comunidad

- [ ] **Editor de Timeline / Eventos de Canción**:
  - Línea de tiempo visual para colocar triggers de cámara, shaders/tintes o llamadas a scripts en beats exactos sin escribir código.
- [ ] **API Extendida para Scripts Comunitarios**:
  - Control de texto en HUD desde Lua/TS/JS, tweening suave de propiedades (`tweenEntityProp`), reproducción de SFX y shaders básicos 2D.
- [ ] **Calibrador Interactivo de Latencia Oboe**:
  - Asistente rápido de golpeteo al ritmo para calcular `audio_offset_ms` óptimo en auriculares Bluetooth o altavoz.

---

## Fase 4 — Distribución en Uptodown y Tiendas de Terceros

- [ ] **Perfil Release Optimizado (LTO + Strip)**:
  - Opción en workflow manual para compilar APKs `Release` firmados con keystore configurado vía GitHub Secrets.
- [ ] **Iconos Adaptativos y Banner Horizontal**:
  - Recursos `mipmap-*` originales libres de derechos para publicación limpia en Uptodown y tiendas APK alternativas.
- [ ] **Pruebas Automatizadas Rust en CI**:
  - Paso adicional de `cargo test --workspace` en host Linux dentro de GitHub Actions antes del cross-compile Android.
