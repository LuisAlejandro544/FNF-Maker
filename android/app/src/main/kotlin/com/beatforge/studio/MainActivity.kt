// Archivo: android/app/src/main/kotlin/com/beatforge/studio/MainActivity.kt
// Propósito: Actividad principal basada en Google `GameActivity`.
// No contiene lógica del motor ni fallbacks (todo el motor y UI viven en Rust).
// Solo gestiona el ciclo de vida de Android y lanza el selector nativo SAF (`ACTION_OPEN_DOCUMENT`)
// delegando la copia de archivos a corrutinas en `Dispatchers.IO` para no bloquear el hilo principal.

package com.beatforge.studio

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.os.Process
import android.view.WindowManager
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.lifecycleScope
import com.google.androidgamesdk.GameActivity
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import java.io.File

class MainActivity : GameActivity() {

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
        // Captura excepciones Kotlin/Java no controladas (incluyendo UnsatisfiedLinkError)
        // antes de inicializar GameActivity para evitar cierres silenciosos en el teléfono.
        Thread.setDefaultUncaughtExceptionHandler { _, throwable ->
            launchCrashScreen(
                "Error inesperado al iniciar BeatForge Studio",
                throwable.stackTraceToString()
            )
            try {
                Thread.sleep(150)
            } catch (_: InterruptedException) {
            }
            Process.killProcess(Process.myPid())
        }

        try {
            // Audio ahora corre 100% en Rust puro (AAudio C ABI vía dlopen, sin Oboe C++ ni libc++_shared.so).
            System.loadLibrary("pulse_android")
        } catch (t: Throwable) {
            launchCrashScreen(
                "No se pudo cargar el motor nativo (libpulse_android.so)",
                t.stackTraceToString()
            )
            finish()
            try {
                Thread.sleep(150)
            } catch (_: InterruptedException) {
            }
            Process.killProcess(Process.myPid())
            return
        }

        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
    }

    private fun launchCrashScreen(title: String, details: String) {
        try {
            File(filesDir, "last_java_crash.txt").writeText("$title\n\n$details")
        } catch (_: Throwable) {
        }

        val intent = Intent(this, CrashReportActivity::class.java).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
            putExtra(CrashReportActivity.EXTRA_TITLE, title)
            putExtra(CrashReportActivity.EXTRA_DETAILS, details)
        }
        startActivity(intent)
    }

    /**
     * Método invocado desde Rust vía JNI (`pulse_android`) si ocurre un panic o fallo de OpenGL ES.
     */
    @Suppress("unused")
    fun reportNativeCrash(details: String) {
        runOnUiThread {
            launchCrashScreen("Error en el motor nativo Rust", details)
            finish()
        }
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
            try {
                openDocLauncher.launch(mimeTypes)
            } catch (e: Exception) {
                e.printStackTrace()
            }
        }
    }
}
