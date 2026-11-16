// Archivo: android/build.gradle.kts
// Propósito: Define las versiones raíz de Android Gradle Plugin, Kotlin Plugin y Compose Compiler
// compatibles con Kotlin 2.1.20 y la compilación nativa de Rust vía cargo-ndk.

plugins {
    id("com.android.application") version "8.9.1" apply false
    id("org.jetbrains.kotlin.android") version "2.1.20" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.1.20" apply false
}
