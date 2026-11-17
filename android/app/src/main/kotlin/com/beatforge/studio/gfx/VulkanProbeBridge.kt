// Archivo: android/app/src/main/kotlin/com/beatforge/studio/gfx/VulkanProbeBridge.kt
// Propósito: Combina la inspección del sistema Android (`ActivityManager` para OpenGL ES 2/3 y
// `PackageManager` para características Vulkan en API 26+) con el sondeo `dlopen` del motor Rust
// (`nativeProbeGraphicsBackends`) sin bloquear el hilo principal.

package com.beatforge.studio.gfx

import android.app.ActivityManager
import android.content.Context
import android.content.pm.PackageManager
import com.beatforge.studio.NativeEngineBridge

object VulkanProbeBridge {

    fun probeCombinedCapabilities(context: Context): GfxCapabilities {
        val rustCaps = try {
            GfxCapabilities.parseFromRustJson(NativeEngineBridge.nativeProbeGraphicsBackends())
        } catch (_: Throwable) {
            GfxCapabilities()
        }

        val pm = context.packageManager
        val am = context.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
        val glEsHex = try {
            am?.deviceConfigurationInfo?.reqGlEsVersion ?: 0x00020000
        } catch (_: Throwable) {
            0x00020000
        }
        val majorGl = (glEsHex ushr 16) and 0xFFFF
        val minorGl = glEsHex and 0xFFFF

        val androidGles2 = majorGl >= 2
        val androidGles3 = majorGl >= 3
        val hasVulkanFeature = try {
            pm.hasSystemFeature(PackageManager.FEATURE_VULKAN_HARDWARE_VERSION)
        } catch (_: Throwable) {
            false
        }
        val vulkanLevel = try {
            if (pm.hasSystemFeature(PackageManager.FEATURE_VULKAN_HARDWARE_LEVEL, 1)) 1
            else if (pm.hasSystemFeature(PackageManager.FEATURE_VULKAN_HARDWARE_LEVEL, 0)) 0
            else -1
        } catch (_: Throwable) {
            -1
        }

        val finalGles2 = rustCaps.gles2Supported || androidGles2
        val finalGles3 = rustCaps.gles3Supported || androidGles3
        val finalVulkan = rustCaps.vulkanSupported || hasVulkanFeature
        val glesLabel = "GLES $majorGl.$minorGl"

        val combinedDetail = buildString {
            append(glesLabel)
            append(" | GLES2: ").append(if (finalGles2) "OK" else "No")
            append(" | GLES3: ").append(if (finalGles3) "OK" else "No")
            append(" | Vulkan: ").append(if (finalVulkan) "OK (Nvl ${maxOf(vulkanLevel, 0)})" else "No")
            if (rustCaps.eglAvailable) append(" | EGL nativo")
        }

        return rustCaps.copy(
            gles2Supported = finalGles2,
            gles3Supported = finalGles3,
            vulkanSupported = finalVulkan,
            glesVersionLabel = glesLabel,
            vulkanHardwareLevel = vulkanLevel,
            detail = combinedDetail
        )
    }
}
