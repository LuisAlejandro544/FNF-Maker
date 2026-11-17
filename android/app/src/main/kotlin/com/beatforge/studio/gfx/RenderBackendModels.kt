// Archivo: android/app/src/main/kotlin/com/beatforge/studio/gfx/RenderBackendModels.kt
// Propósito: Modelos y enumeración modular para los backends gráficos preparados en BeatForge Studio
// (Canvas 2D estable, OpenGL ES 2.0, OpenGL ES 3.x y Vulkan) y su reporte de capacidades.

package com.beatforge.studio.gfx

import org.json.JSONObject

enum class RenderBackendKind(
    val key: String,
    val label: String,
    val shortBadge: String,
    val subtitle: String
) {
    Canvas2d(
        key = "canvas2d",
        label = "Canvas 2D (Estable)",
        shortBadge = "Canvas 2D",
        subtitle = "Renderizador activo por defecto con Jetpack Compose Canvas"
    ),
    Gles2(
        key = "gles2",
        label = "OpenGL ES 2.0 (Base)",
        shortBadge = "GLES 2.0",
        subtitle = "Terreno preparado con GLSurfaceView y shaders básicos 32/64-bit"
    ),
    Gles3(
        key = "gles3",
        label = "OpenGL ES 3.x (Base)",
        shortBadge = "GLES 3.x",
        subtitle = "Terreno preparado con contexto OpenGL ES 3.0+ para futuras instancias GPU"
    ),
    Vulkan(
        key = "vulkan",
        label = "Vulkan (Preparado)",
        shortBadge = "Vulkan",
        subtitle = "Terreno preparado con sondeo nativo libvulkan.so para pipeline futuro"
    );

    companion object {
        fun fromKey(raw: String): RenderBackendKind {
            return entries.firstOrNull { it.key.equals(raw.trim(), ignoreCase = true) } ?: Canvas2d
        }
    }
}

data class GfxCapabilities(
    val canvas2dSupported: Boolean = true,
    val eglAvailable: Boolean = false,
    val gles2Supported: Boolean = false,
    val gles3Supported: Boolean = false,
    val vulkanSupported: Boolean = false,
    val glesVersionLabel: String = "GLES 2.0+",
    val vulkanHardwareLevel: Int = -1,
    val activeBackend: String = RenderBackendKind.Canvas2d.key,
    val recommendedBackend: String = RenderBackendKind.Canvas2d.key,
    val detail: String = "Sondeando capacidades gráficas..."
) {
    fun supports(kind: RenderBackendKind): Boolean {
        return when (kind) {
            RenderBackendKind.Canvas2d -> canvas2dSupported
            RenderBackendKind.Gles2 -> gles2Supported
            RenderBackendKind.Gles3 -> gles3Supported
            RenderBackendKind.Vulkan -> vulkanSupported
        }
    }

    companion object {
        fun parseFromRustJson(rawJson: String): GfxCapabilities {
            return try {
                val obj = JSONObject(rawJson)
                GfxCapabilities(
                    canvas2dSupported = obj.optBoolean("canvas2dSupported", true),
                    eglAvailable = obj.optBoolean("eglAvailable", false),
                    gles2Supported = obj.optBoolean("gles2Supported", false),
                    gles3Supported = obj.optBoolean("gles3Supported", false),
                    vulkanSupported = obj.optBoolean("vulkanSupported", false),
                    activeBackend = obj.optString("activeBackend", RenderBackendKind.Canvas2d.key),
                    recommendedBackend = obj.optString("recommendedBackend", RenderBackendKind.Canvas2d.key),
                    detail = obj.optString("detail", "")
                )
            } catch (_: Exception) {
                GfxCapabilities()
            }
        }
    }
}
