// Archivo: android/app/src/main/kotlin/com/beatforge/studio/SafFileBridge.kt
// Propósito: Puente auxiliar Kotlin <-> Rust para copiar archivos seleccionados por el usuario
// mediante Storage Access Framework (SAF) a almacenamiento interno en hilo IO (`Dispatchers.IO`)
// conservando su nombre/extensión real para que Rust los decodifique sin bloquear la UI.

package com.beatforge.studio

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import java.io.File

object SafFileBridge {

    external fun nativeOnFilePicked(localPath: String)

    fun copyUriToInternalCache(context: Context, uri: Uri, kindCode: Int): String? {
        return try {
            val displayName = resolveDisplayName(context, uri) ?: "import_$kindCode.bin"
            val importsDir = File(context.filesDir, "mod_imports")
            if (!importsDir.exists()) {
                importsDir.mkdirs()
            }
            val destFile = File(importsDir, "${System.currentTimeMillis()}_$displayName")
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
