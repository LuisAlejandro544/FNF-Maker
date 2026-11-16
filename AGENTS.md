# AGENTS.md — Reglas para Agentes de IA y Colaboradores

Cualquier agente de programación o colaborador que modifique este repositorio **DEBE** respetar las siguientes reglas sin excepción:

---

## 1. Cascarón Web Intocable (`Hello World`)
- Mantén `/src/App.tsx` mostrando únicamente `Hello World` (con su `<title>` en `/index.html` y `metadata.json` intactos).
- **No** traslades pantallas del motor móvil ni simuladores complejos a React/Web a menos que el usuario lo solicite explícitamente.

## 2. Arquitectura Modular y Comentarios Explicativos
- Respeta la separación de responsabilidades entre **Rust** (motor puro) y **Kotlin Jetpack Compose** (interfaz móvil):
  - `pulse_core`: lógica pura, estructuras de datos, parsers, optimización (cero dependencias de UI o Android).
  - `pulse_audio`: decodificación y mezclador de audio en **Rust puro** (`rust_audio_backend.rs` vía `libaaudio.so`, sin C++ Oboe).
  - `pulse_scripting`: Lua 5.4, transpilador `oxc` TypeScript y QuickJS-NG.
  - `pulse_android`: `EngineSession` y puente JNI hacia `NativeEngineBridge.kt`.
  - `android/.../ui/`: pantallas separadas (`screens/`), barra (`components/`), popups (`popups/`) y tema (`theme/`) en **Jetpack Compose + Material 3 + Material Icons Extended**.
- Todo archivo de código nuevo o modificado debe incluir comentarios de cabecera claros (en español) explicando qué hace su lógica y propósito.
- No amontones controles en una sola pantalla: utiliza pantallas dedicadas (`screens/`) o ventanas modales (`popups/`).
- No cambies el diseño visual, la paleta (`StudioTheme.kt`), los iconos ni la lógica existente que ya funciona salvo que el usuario lo pida o sea necesario para corregir un error.

## 3. Reglas Críticas de Android y Multi-Arquitectura (32 y 64 bits)
- **Soporte obligatorio de 32 y 64 bits**: Mantén `armeabi-v7a` y `arm64-v8a` en `android/app/build.gradle.kts` (`splits.abi`), `.cargo/config.toml` y `.github/workflows/build-apk.yml`.
- **Nunca usar `persist.sys.*`** ni propiedades privilegiadas del sistema Android.
- **No bloquear el hilo principal en Kotlin**: Toda lectura/copia de archivos desde SAF (`Uri`), decodificación de Bitmaps o llamada JNI pesada debe ejecutarse con corrutinas en `Dispatchers.IO`.
- **No crear fallbacks en Kotlin** que reemplacen el motor Rust; Kotlin únicamente presenta la interfaz Compose, selector SAF y pasarela JNI hacia Rust.
- **No omitir `cargo ndk` en Gradle**: `preBuild` debe compilar obligatoriamente `pulse_android` con `cargo ndk --platform 24`.
- **`minSdk = 24`**: Verifica compatibilidad con Android 7.0+ antes de añadir APIs nuevas de Android o NDK (en audio AAudio, mantener respaldo en Rust para API 24–25).

## 4. Política de Dependencias y Propiedad Intelectual
- **Solo licencias permisivas** (MIT, Apache-2.0, BSD, Zlib, ISC, Unlicense). **Prohibido** añadir dependencias GPL, LGPL, AGPL, MPL-2.0 (como `symphonia`) o licencias que obliguen a abrir código / atribución restrictiva.
- **Cero nombres o assets protegidos por derechos de autor** en archivos, clases, paquetes o recursos.
- **Sin soluciones improvisadas sin dependencias** cuando existe una librería estándar confiable y permisiva que resuelva el problema al 100%.

## 5. Flujo de GitHub Actions
- `.github/workflows/build-apk.yml` debe activarse **únicamente de forma manual (`workflow_dispatch`)**. No añadas triggers `push` ni `pull_request`.
- Debe mantener caché de Rust, `cargo-ndk` y Gradle, compilar `assembleDebug` y subir **2 artefactos separados**:
  - `BeatForgeStudio-64bit-arm64-v8a-Debug`
  - `BeatForgeStudio-32bit-armeabi-v7a-Debug`

## 6. Verificación y Honestidad Técnica
- En este entorno web sin `rustc` ni Android SDK, verifica siempre:
  1. Pruebas del SDK TypeScript: `npx --yes tsx --test sdk/ts/src/engine-tools.test.ts`
  2. Compilación y lint web (`compile_applet` + `lint_applet`).
- Al tocar APIs de Rust (`oxc_*`, `rquickjs`, `mlua`, `jni`, `resvg`), verifica las firmas exactas contra `docs.rs` o el tarball de `crates.io` correspondiente a la versión fijada en `engine/Cargo.toml`.
- Reporta honestamente qué se verificó localmente y qué requiere ejecutar manualmente el workflow de GitHub Actions para validación nativa ARM.
