// Archivo: android/app/build.gradle.kts
// Propósito: Script de compilación del módulo `:app`.
// Puntos clave:
// 1. Invoca obligatoriamente `cargo ndk` dentro de `preBuild` para compilar todo el motor Rust
//    (UI egui, Oboe, Lua 5.4, QuickJS-NG, oxc TypeScript, resvg, Sparrow XML) en 32-bit (`armeabi-v7a`)
//    y 64-bit (`arm64-v8a`, `x86_64`).
// 2. Copia `libc++_shared.so` del NDK a `jniLibs/<abi>/` requerido por Oboe.
// 3. Configura `androidx.games:games-activity:4.4.2` (`prefab = true`) y corrutinas Kotlin.
// 4. Prepara salida APK lista para distribuirse en Uptodown o tiendas APK de terceros.

import java.io.File

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "com.beatforge.studio"
    compileSdk = 35
    ndkVersion = "27.2.12479018"

    defaultConfig {
        applicationId = "com.beatforge.studio"
        // minSdk 24 garantiza compatibilidad amplia con teléfonos de 32 y 64 bits + Oboe (OpenSL/AAudio)
        minSdk = 24
        targetSdk = 35
        versionCode = 1
        versionName = "0.1.0"
    }

    // Genera 2 APKs separados por arquitectura: 64 bits (arm64-v8a) y 32 bits (armeabi-v7a)
    splits {
        abi {
            isEnable = true
            reset()
            include("armeabi-v7a", "arm64-v8a")
            isUniversalApk = false
        }
    }

    buildFeatures {
        prefab = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlinOptions {
        jvmTarget = "17"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            // Firma de debug temporal para permitir instalar el APK directo en Uptodown / prueba;
            // puede reemplazarse con keystore propio en releaseSigning.
            signingConfig = signingConfigs.getByName("debug")
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }

    packaging {
        jniLibs {
            useLegacyPackaging = false
            pickFirsts += listOf("**/libc++_shared.so")
        }
    }
}

dependencies {
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.activity:activity-ktx:1.10.1")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.games:games-activity:4.4.2")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")
}

val isReleaseRequested = gradle.startParameter.taskNames.any {
    it.contains("Release", ignoreCase = true)
}

// Tarea obligatoria: compila las librerías nativas Rust (.so) en modo Debug para 32-bit y 64-bit
val buildRustDebug by tasks.registering(Exec::class) {
    group = "rust"
    description = "Compila pulse_android en Debug con cargo-ndk para 32 bits (armeabi-v7a) y 64 bits (arm64-v8a)"
    workingDir = file("../../engine")
    val jniOut = file("src/main/jniLibs").absolutePath
    commandLine(
        "cargo", "ndk",
        "-t", "armeabi-v7a",
        "-t", "arm64-v8a",
        "--platform", "24",
        "-o", jniOut,
        "build", "-p", "pulse_android"
    )
}

// Tarea equivalente en Release cuando se solicite assembleRelease
val buildRustRelease by tasks.registering(Exec::class) {
    group = "rust"
    description = "Compila pulse_android en Release con cargo-ndk para 32 bits (armeabi-v7a) y 64 bits (arm64-v8a)"
    workingDir = file("../../engine")
    val jniOut = file("src/main/jniLibs").absolutePath
    commandLine(
        "cargo", "ndk",
        "-t", "armeabi-v7a",
        "-t", "arm64-v8a",
        "--platform", "24",
        "-o", jniOut,
        "build", "--release", "-p", "pulse_android"
    )
}

// Copia libc++_shared.so del NDK si cargo-ndk no lo incluyó, evitando fallos de carga en Oboe
val copyCppShared by tasks.registering {
    dependsOn(if (isReleaseRequested) buildRustRelease else buildRustDebug)
    doLast {
        val ndkHome = System.getenv("ANDROID_NDK_HOME")
            ?: android.ndkDirectory.absolutePath
        val abiMap = mapOf(
            "armeabi-v7a" to "arm-linux-androideabi",
            "arm64-v8a" to "aarch64-linux-android"
        )
        val prebuiltDir = File(ndkHome, "toolchains/llvm/prebuilt")
        val hostDir = prebuiltDir.listFiles()?.firstOrNull { it.isDirectory } ?: return@doLast
        for ((abi, triple) in abiMap) {
            val src = File(hostDir, "sysroot/usr/lib/$triple/libc++_shared.so")
            val dst = project.file("src/main/jniLibs/$abi/libc++_shared.so")
            if (src.exists() && !dst.exists()) {
                dst.parentFile.mkdirs()
                src.copyTo(dst, overwrite = true)
            }
        }
    }
}

tasks.named("preBuild") {
    dependsOn(copyCppShared)
}
