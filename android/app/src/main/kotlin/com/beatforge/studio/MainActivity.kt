// Archivo: android/app/src/main/kotlin/com/beatforge/studio/MainActivity.kt
// Propósito: Actividad principal basada en Google `GameActivity`.
// No contiene lógica del motor ni fallbacks (todo el motor y UI viven en Rust).
// Solo gestiona el ciclo de vida de Android y lanza el selector nativo SAF (`ACTION_OPEN_DOCUMENT`)
// delegando la copia de archivos a corrutinas en `Dispatchers.IO` para no bloquear el hilo principal.

package com.beatforge.studio

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.lifecycleScope
import com.google.androidgamesdk.GameActivity
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch

class MainActivity : GameActivity() {

    companion object {
        init {
            System.loadLibrary("c++_shared")
            System.loadLibrary("pulse_android")
        }
    }

    private var pendingKindCode: Int = 1

    private val openDocLauncher =
        registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
            if (uri != null) {
                // Copia asíncrona fuera del hilo principal usando Kotlin Coroutines (Dispatchers.IO)
                lifecycleScope.launch(Dispatchers.IO) {
                    val localPath = SafFileBridge.copyUriToInternalCache(applicationContext, uri, pendingKindCode)
                    if (localPath != null) {
                        SafFileBridge.nativeOnFilePicked(localPath)
                    }
                }
            }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
    }

    /**
     * Método invocado desde Rust vía JNI (`pulse_android`) cuando el usuario pulsa un botón de importar.
     */
    @Suppress("unused")
    fun launchSafPicker(kindCode: Int) {
        pendingKindCode = kindCode
        runOnUiThread {
            val mimeTypes = when (kindCode) {
                1 -> arrayOf("application/json", "text/plain", "*/*")
                10, 11, 12 -> arrayOf("audio/*", "application/ogg", "*/*")
                20 -> arrayOf("image/png", "image/*")
                21 -> arrayOf("text/xml", "application/xml", "*/*")
                30, 31, 32, 33 -> arrayOf("image/svg+xml", "*/*")
                else -> arrayOf("*/*")
            }
            openDocLauncher.launch(mimeTypes)
        }
    }
}
