// Archivo: android/app/build.gradle.kts
// Propósito: Script de compilación del módulo `:app`.
// Puntos clave:
// 1. Invoca obligatoriamente `cargo ndk` dentro de `preBuild` para compilar todo el motor Rust
//    (audio AAudio en Rust puro sin C++, Lua 5.4, QuickJS-NG, oxc TypeScript, resvg, Sparrow XML)
//    en 32-bit (`armeabi-v7a`) y 64-bit (`arm64-v8a`).
// 2. Configura Jetpack Compose + Material 3 + Material Icons Extended + ViewModel + Corrutinas Kotlin.
// 3. Prepara salida APK lista para distribuirse en Uptodown o tiendas APK de terceros.

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "com.beatforge.studio"
    compileSdk = 35
    ndkVersion = "27.2.12479018"

    defaultConfig {
        applicationId = "com.beatforge.studio"
        // minSdk 24 garantiza compatibilidad amplia con teléfonos de 32 y 64 bits
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
        compose = true
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
        }
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2025.02.00")
    implementation(composeBom)

    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.appcompat:appcompat:1.7.0")
    implementation("androidx.activity:activity-compose:1.10.1")
    implementation("androidx.lifecycle:lifecycle-runtime-ktx:2.8.7")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")

    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.ui:ui-tooling-preview")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.material:material-icons-extended")

    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.10.2")

    debugImplementation("androidx.compose.ui:ui-tooling")
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
        "build", "--keep-going", "-p", "pulse_android"
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
        "build", "--release", "--keep-going", "-p", "pulse_android"
    )
}

tasks.named("preBuild") {
    dependsOn(if (isReleaseRequested) buildRustRelease else buildRustDebug)
}
