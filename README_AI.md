# README_AI — Guía Operativa para Trabajar con IA en BeatForge Studio

Este documento resume cómo retomar, depurar, extender y validar **BeatForge Studio** usando flujos estructurados asistidos por IA directamente desde el repositorio.

---

## 1. Mapa Rápido de Contexto para Prompts

Cuando pidas cambios o mejoras a una IA sobre este proyecto, indica siempre qué capa quieres tocar:

| Capa | Ruta | Lenguaje | Qué cubre |
| :--- | :--- | :--- | :--- |
| Núcleo del motor | `engine/crates/pulse_core` | Rust | `chart.json` completo/export, Sparrow `.xml`, `.astc` (`astc_loader.rs`), `.json` Animate/TexturePacker (`atlas_json.rs`), escenario Z-order, `resvg`, `rayon` |
| Audio baja latencia | `engine/crates/pulse_audio` | Rust puro (`dlopen` AAudio) | Mezclador 1–3 pistas, decodificación OGG/WAV/MP3, reloj maestro por muestras y cuenta regresiva BPM (`3, 2, 1, ¡YA!`) |
| Scripting Mods | `engine/crates/pulse_scripting` | Rust + Lua + TS + JS | `mlua` (Lua 5.4), `oxc` (TypeScript), `rquickjs` (QuickJS-NG), puente de comandos |
| Puente JNI & Sesión | `engine/crates/pulse_android` | Rust (`jni` 0.21) | `EngineSession` + JNI `NativeEngineBridge_*` + caché PNG para `.astc` y rigs `Animation.json` horneados |
| Interfaz Android | `android/app/src/main/kotlin/com/beatforge/studio/` | Kotlin (Jetpack Compose) | `MainActivity` (SAF Open/CreateDocument), `StudioViewModel` (`Dispatchers.IO`), Material 3 + Icons Extended, overlay countdown y pantallas modulares |
| SDK y Pruebas | `sdk/` | TypeScript / SVG / Lua / JS | Contratos `.d.ts`, utilidades testeadas con `node:test` (`chart.json`, countdown, `.astc`, `.json`), flechas originales |
| Compilación APK | `.github/workflows/build-apk.yml` | YAML | Workflow manual `workflow_dispatch` con caché y 2 artefactos Debug |

---

## 2. Comandos de Verificación Local vs Nube

### En el entorno local (Web + SDK TypeScript)
```bash
# 1. Ejecutar pruebas unitarias del SDK TypeScript
npx --yes tsx --test sdk/ts/src/engine-tools.test.ts

# 2. Verificar compilación del cascarón web ("Hello World")
npm run build

# 3. Verificar tipos TypeScript / lint
npm run lint
```

### En GitHub Actions (Compilación Nativa Android 32 y 64 bits)
- El entorno contenedor no incluye `cargo` ni Android NDK.
- Para validar cambios en `engine/` o `android/`, dispara manualmente el workflow:
  `Actions → Compilar BeatForge Studio APK (Debug Manual) → Run workflow`.

---

## 3. Flujo Recomendado de Prompts por Rol

1. **El Arquitecto (Planificar antes de editar)**:
   - Pide siempre identificar qué módulo Rust (`pulse_core`, `pulse_audio`, `pulse_scripting`, `pulse_android`) o pantalla Compose (`ui/screens/*`, `ui/popups/*`) debe asumir la nueva responsabilidad sin mezclar lógica de motor en Kotlin.
2. **El Constructor (Implementar completo)**:
   - Exige manejo de errores explícito (`Result<T, E>` en Rust, `ActionResult` en Kotlin), comentarios explicativos en español y verificación previa de firmas JNI entre `NativeEngineBridge.kt` y `pulse_android/src/lib.rs`.
3. **El Detective (Depurar logs de GitHub Actions)**:
   - Al analizar un error de compilación de Actions, revisa si ocurrió en `cargo ndk` (Rust 32/64 bits) o en `compileDebugKotlin` (Compose/Gradle) antes de relanzar el workflow.
4. **El Crítico y El Optimizador**:
   - Audita que no haya mutex bloqueantes en el callback AAudio (`pulse_audio/src/mixer.rs`), ni copias SAF/JNI pesadas en el hilo principal de Kotlin (`Dispatchers.IO`), ni dependencias con licencia copyleft/MPL.
5. **El Escudo (Testing)**:
   - Añade tests puros en `pulse_core` / `pulse_android` (`#[cfg(test)]`) y en `sdk/ts/src/engine-tools.test.ts` cada vez que cambien reglas de charts, Sparrow XML, sesiones o validadores.

---

## 4. Checklist Antes de Cerrar Cualquier Cambio

- [ ] `/src/App.tsx` sigue mostrando únicamente `Hello World`.
- [ ] No hay nombres comerciales o assets protegidos por copyright.
- [ ] Todas las dependencias nuevas son MIT / Apache-2.0 (nunca GPL/LGPL/MPL).
- [ ] Kotlin Compose no implementa fallbacks de motor; todo se resuelve vía JNI en Rust.
- [ ] Se mantienen activas ambas arquitecturas: `arm64-v8a` (64 bits) y `armeabi-v7a` (32 bits).
- [ ] `.github/workflows/build-apk.yml` permanece exclusivamente en `workflow_dispatch`.
- [ ] Los tests de `sdk/ts/src/engine-tools.test.ts` y `npm run build` / `lint` pasan después del último cambio.
