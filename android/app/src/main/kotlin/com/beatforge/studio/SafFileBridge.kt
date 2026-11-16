// Archivo: android/app/src/main/kotlin/com/beatforge/studio/SafFileBridge.kt
// Propósito: Puente auxiliar Kotlin para copiar archivos seleccionados por el usuario
// mediante Storage Access Framework (SAF) a almacenamiento interno en hilo IO (`Dispatchers.IO`)
// conservando su nombre y extensión reales para que el motor Rust los lea y decodifique
// sin bloquear el hilo principal.

package com.beatforge.studio

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import java.io.File

object SafFileBridge {

    fun copyUriToInternalCache(context: Context, uri: Uri, kindTag: String): String? {
        return try {
            val rawName = resolveDisplayName(context, uri) ?: "import_$kindTag.bin"
            val safeName = rawName.replace(Regex("[^A-Za-z0-9._-]"), "_")
            val importsDir = File(context.filesDir, "mod_imports")
            if (!importsDir.exists()) {
                importsDir.mkdirs()
            }
            val destFile = File(importsDir, "${System.currentTimeMillis()}_$safeName")
            context.contentResolver.openInputStream(uri)?.use { input ->
                destFile.outputStream().use { output ->
                    input.copyTo(output, bufferSize = 64 * 1024)
                }
            }
            destFile.absolutePath
        } catch (e: Exception) {
            e.printStackTrace()
            null
        }
    }

    private fun resolveDisplayName(context: Context, uri: Uri): String? {
        context.contentResolver.query(uri, null, null, null, null)?.use { cursor ->
            val nameIdx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
            if (nameIdx >= 0 && cursor.moveToFirst()) {
                return cursor.getString(nameIdx)
            }
        }
        return uri.lastPathSegment?.substringAfterLast('/')
    }
}
