// Archivo: android/settings.gradle.kts
// Propósito: Configura los repositorios Maven (Google y MavenCentral) y declara el módulo `:app`
// de BeatForge Studio para compilar el APK de Android.

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "BeatForgeStudio"
include(":app")
