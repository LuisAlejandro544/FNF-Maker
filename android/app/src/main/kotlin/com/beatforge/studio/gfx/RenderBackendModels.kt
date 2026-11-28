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
        label = "Canvas 2D (Modo seguro)",
        shortBadge = "Canvas 2D",
        subtitle = "Modo seguro de respaldo por software/Canvas Compose"
    ),
    Gles2(
        key = "gles2",
        label = "OpenGL ES 2.0 (Respaldo GPU)",
        shortBadge = "GLES 2.0",
        subtitle = "Renderizador GPU por lotes compatible con Android 8.0+ (32/64-bit)"
    ),
    Gles3(
        key = "gles3",
        label = "OpenGL ES 3.0 (Por defecto)",
        shortBadge = "GLES 3.0",
        subtitle = "Renderizador GPU activo por defecto con fallback automático a GLES 2.0"
    ),
    Vulkan(
        key = "vulkan",
        label = "Vulkan (Preparado)",
        shortBadge = "Vulkan",
        subtitle = "Sondeo libvulkan.so activo; ejecuta gameplay sobre GLES 3.0 -> 2.0"
    );

    companion object {
        fun fromKey(raw: String): RenderBackendKind {
            return entries.firstOrNull { it.key.equals(raw.trim(), ignoreCase = true) } ?: Gles3
        }
    }
}

data class GfxCapabilities(
    val canvas2dSupported: Boolean = true,
    val eglAvailable: Boolean = false,
    val gles2Supported: Boolean = false,
    val gles3Supported: Boolean = false,
    val vulkanSupported: Boolean = false,
    val glesVersionLabel: String = "GLES 3.0 / 2.0",
    val vulkanHardwareLevel: Int = -1,
    val activeBackend: String = RenderBackendKind.Gles3.key,
    val recommendedBackend: String = RenderBackendKind.Gles3.key,
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
                    activeBackend = obj.optString("activeBackend", RenderBackendKind.Gles3.key),
                    recommendedBackend = obj.optString("recommendedBackend", RenderBackendKind.Gles3.key),
                    detail = obj.optString("detail", "")
                )
            } catch (_: Exception) {
                GfxCapabilities()
            }
        }
    }
}
