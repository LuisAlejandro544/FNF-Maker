// Archivo: android/app/src/main/kotlin/com/beatforge/studio/audio/AndroidAudioDeviceProbe.kt
// Propósito: Sondeo real del hardware y salida de audio activa del teléfono mediante
// `AudioManager`, `AudioDeviceInfo`, `PackageManager` y `Build` ejecutado en `Dispatchers.IO`.
// Envía el perfil físico serializado como JSON hacia Rust (`pulse_audio::adaptive_master`),
// donde se fusiona con los parámetros reales del stream `libaaudio.so`.

package com.beatforge.studio.audio

import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioDeviceInfo
import android.media.AudioFormat
import android.media.AudioManager
import android.os.Build
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.json.JSONArray
import org.json.JSONObject

object AndroidAudioDeviceProbe {

    private data class RouteSelection(
        val routeKey: String,
        val device: AudioDeviceInfo?
    )

    suspend fun probeDeviceJson(context: Context, aaudioDeviceId: Int = 0): String = withContext(Dispatchers.IO) {
        val appContext = context.applicationContext
        val audioManager = appContext.getSystemService(Context.AUDIO_SERVICE) as? AudioManager
        val pm = appContext.packageManager

        val osSampleRate = audioManager
            ?.getProperty(AudioManager.PROPERTY_OUTPUT_SAMPLE_RATE)
            ?.toIntOrNull()
            ?.takeIf { it >= 8_000 }
            ?: 48_000

        val osFramesPerBuffer = audioManager
            ?.getProperty(AudioManager.PROPERTY_OUTPUT_FRAMES_PER_BUFFER)
            ?.toIntOrNull()
            ?.takeIf { it > 0 }
            ?: 192

        val supportsLowLatency = pm.hasSystemFeature(PackageManager.FEATURE_AUDIO_LOW_LATENCY)
        val supportsProAudio = pm.hasSystemFeature(PackageManager.FEATURE_AUDIO_PRO)

        val outputs = try {
            audioManager?.getDevices(AudioManager.GET_DEVICES_OUTPUTS)?.toList().orEmpty()
        } catch (_: Throwable) {
            emptyList()
        }

        val matchedByAaudio = if (aaudioDeviceId > 0) {
            outputs.firstOrNull { it.id == aaudioDeviceId }
        } else {
            null
        }
        val selection = if (matchedByAaudio != null) {
            RouteSelection(mapDeviceTypeToRouteKey(matchedByAaudio), matchedByAaudio)
        } else {
            selectPrimaryOutputRoute(outputs)
        }
        val activeDevice = selection.device

        val deviceName = activeDevice?.productName?.toString()?.trim()?.ifBlank { null }
            ?: defaultRouteLabel(selection.routeKey)

        val channelInfo = resolveChannelCount(activeDevice, selection.routeKey)
        val supportedRates = activeDevice?.sampleRates
            ?.filter { it >= 8_000 }
            ?.distinct()
            ?.sorted()
            ?: listOf(osSampleRate)

        val json = JSONObject().apply {
            put("route", selection.routeKey)
            put("deviceName", deviceName)
            put("manufacturer", Build.MANUFACTURER.orEmpty().ifBlank { "Android" })
            put("model", Build.MODEL.orEmpty().ifBlank { "Device" })
            put("androidApiLevel", Build.VERSION.SDK_INT)
            put("osSampleRate", osSampleRate)
            put("osFramesPerBuffer", osFramesPerBuffer)
            put("reportedChannelCount", channelInfo.first)
            put("isMonoSpeaker", channelInfo.second)
            put("supportsLowLatencyFeature", supportsLowLatency)
            put("supportsProAudioFeature", supportsProAudio)
            put("aaudioMatched", matchedByAaudio != null)
            put(
                "supportedSampleRates",
                JSONArray().apply {
                    (supportedRates.ifEmpty { listOf(osSampleRate) }).forEach { put(it) }
                }
            )
        }
        json.toString()
    }

    private fun mapDeviceTypeToRouteKey(device: AudioDeviceInfo): String {
        return when (device.type) {
            AudioDeviceInfo.TYPE_USB_DEVICE,
            AudioDeviceInfo.TYPE_USB_HEADSET,
            AudioDeviceInfo.TYPE_USB_ACCESSORY -> "usb_audio"

            AudioDeviceInfo.TYPE_WIRED_HEADPHONES,
            AudioDeviceInfo.TYPE_WIRED_HEADSET,
            AudioDeviceInfo.TYPE_LINE_ANALOG,
            AudioDeviceInfo.TYPE_LINE_DIGITAL -> "wired_headphones"

            AudioDeviceInfo.TYPE_BLUETOOTH_A2DP -> "bluetooth_a2dp"

            AudioDeviceInfo.TYPE_HDMI,
            AudioDeviceInfo.TYPE_HDMI_ARC -> "hdmi_or_external"

            AudioDeviceInfo.TYPE_BUILTIN_SPEAKER,
            AudioDeviceInfo.TYPE_BUILTIN_EARPIECE -> "builtin_speaker"

            else -> {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && (
                        device.type == AudioDeviceInfo.TYPE_BLE_HEADSET ||
                            device.type == AudioDeviceInfo.TYPE_BLE_SPEAKER ||
                            device.type == AudioDeviceInfo.TYPE_HDMI_EARC ||
                            (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                                device.type == AudioDeviceInfo.TYPE_BLE_BROADCAST)
                        )
                ) {
                    if (device.type == AudioDeviceInfo.TYPE_HDMI_EARC) "hdmi_or_external" else "bluetooth_le"
                } else {
                    "unknown"
                }
            }
        }
    }

    private fun selectPrimaryOutputRoute(devices: List<AudioDeviceInfo>): RouteSelection {
        if (devices.isEmpty()) {
            return RouteSelection("builtin_speaker", null)
        }

        val usb = devices.firstOrNull {
            it.type == AudioDeviceInfo.TYPE_USB_DEVICE ||
                it.type == AudioDeviceInfo.TYPE_USB_HEADSET ||
                it.type == AudioDeviceInfo.TYPE_USB_ACCESSORY
        }
        if (usb != null) return RouteSelection("usb_audio", usb)

        val wired = devices.firstOrNull {
            it.type == AudioDeviceInfo.TYPE_WIRED_HEADPHONES ||
                it.type == AudioDeviceInfo.TYPE_WIRED_HEADSET ||
                it.type == AudioDeviceInfo.TYPE_LINE_ANALOG ||
                it.type == AudioDeviceInfo.TYPE_LINE_DIGITAL
        }
        if (wired != null) return RouteSelection("wired_headphones", wired)

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            val ble = devices.firstOrNull {
                it.type == AudioDeviceInfo.TYPE_BLE_HEADSET ||
                    it.type == AudioDeviceInfo.TYPE_BLE_SPEAKER ||
                    (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                        it.type == AudioDeviceInfo.TYPE_BLE_BROADCAST)
            }
            if (ble != null) return RouteSelection("bluetooth_le", ble)
        }

        val a2dp = devices.firstOrNull { it.type == AudioDeviceInfo.TYPE_BLUETOOTH_A2DP }
        if (a2dp != null) return RouteSelection("bluetooth_a2dp", a2dp)

        val hdmi = devices.firstOrNull {
            it.type == AudioDeviceInfo.TYPE_HDMI ||
                it.type == AudioDeviceInfo.TYPE_HDMI_ARC ||
                (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S &&
                    it.type == AudioDeviceInfo.TYPE_HDMI_EARC)
        }
        if (hdmi != null) return RouteSelection("hdmi_or_external", hdmi)

        val speaker = devices.firstOrNull { it.type == AudioDeviceInfo.TYPE_BUILTIN_SPEAKER }
        if (speaker != null) return RouteSelection("builtin_speaker", speaker)

        val earpiece = devices.firstOrNull { it.type == AudioDeviceInfo.TYPE_BUILTIN_EARPIECE }
        if (earpiece != null) return RouteSelection("builtin_speaker", earpiece)

        return RouteSelection("unknown", devices.firstOrNull())
    }

    private fun resolveChannelCount(device: AudioDeviceInfo?, routeKey: String): Pair<Int, Boolean> {
        if (device == null) {
            return 2 to false
        }
        val counts = try {
            device.channelCounts.filter { it >= 1 }
        } catch (_: Throwable) {
            emptyList()
        }
        val maxCount = counts.maxOrNull()
        val masks = try {
            device.channelMasks.toList()
        } catch (_: Throwable) {
            emptyList()
        }

        val inferredMax = when {
            maxCount != null -> maxCount
            masks.isNotEmpty() -> {
                val hasMulti = masks.any {
                    it != AudioFormat.CHANNEL_OUT_MONO && it != AudioFormat.CHANNEL_OUT_DEFAULT
                }
                if (hasMulti) 2 else 1
            }
            else -> 2
        }

        val isMonoSpeaker = (routeKey == "builtin_speaker" && inferredMax <= 1) ||
            device.type == AudioDeviceInfo.TYPE_BUILTIN_EARPIECE
        return inferredMax.coerceAtLeast(1) to isMonoSpeaker
    }

    private fun defaultRouteLabel(routeKey: String): String = when (routeKey) {
        "usb_audio" -> "DAC / Audífonos USB-C"
        "wired_headphones" -> "Audífonos por Cable"
        "bluetooth_a2dp" -> "Bluetooth A2DP"
        "bluetooth_le" -> "Bluetooth LE Audio"
        "hdmi_or_external" -> "Salida HDMI / Externa"
        "builtin_speaker" -> "Altavoz Integrado"
        else -> "Salida de Audio"
    }
}
