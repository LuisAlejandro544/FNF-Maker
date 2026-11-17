// Archivo: android/app/src/main/kotlin/com/beatforge/studio/SafFileBridge.kt
// Propósito: Puente auxiliar Kotlin para copiar archivos seleccionados por el usuario
// mediante Storage Access Framework (SAF) hacia un área temporal en `cacheDir/import_staging/`
// en hilo IO (`Dispatchers.IO`). Tras el procesamiento en Rust, el archivo temporal se elimina
// para mantener limpio `Datos del usuario` (`filesDir`).

package com.beatforge.studio

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import java.io.File

object SafFileBridge {

    private const val STAGING_DIR_NAME = "import_staging"

    fun copyUriToStagingCache(context: Context, uri: Uri, kindTag: String): String? {
        return try {
            val rawName = resolveDisplayName(context, uri) ?: "import_$kindTag.bin"
            val safeName = rawName.replace(Regex("[^A-Za-z0-9._-]"), "_")
            val stagingDir = File(context.cacheDir, STAGING_DIR_NAME)
            if (!stagingDir.exists()) {
                stagingDir.mkdirs()
            }
            val destFile = File(stagingDir, "${System.currentTimeMillis()}_$safeName")
            context.contentResolver.openInputStream(uri)?.use { input ->
                destFile.outputStream().use { output ->
                    input.copyTo(output, bufferSize = 64 * 1024)
                }
            }
            if (destFile.isFile) destFile.absolutePath else null
        } catch (e: Exception) {
            e.printStackTrace()
            null
        }
    }

    /** Alias de compatibilidad que redirige al directorio temporal de `cacheDir`. */
    fun copyUriToInternalCache(context: Context, uri: Uri, kindTag: String): String? =
        copyUriToStagingCache(context, uri, kindTag)

    /** Crea una ruta temporal segura en `cacheDir/import_staging/` para escribir paquetes `.mod` por streaming. */
    fun createExportStagingFile(context: Context, fileName: String): File {
        val safeName = fileName.replace(Regex("[^A-Za-z0-9._-]"), "_").ifBlank { "export.mod" }
        val stagingDir = File(context.cacheDir, STAGING_DIR_NAME)
        if (!stagingDir.exists()) {
            stagingDir.mkdirs()
        }
        return File(stagingDir, "${System.currentTimeMillis()}_$safeName")
    }

    /** Elimina inmediatamente un archivo temporal tras ser importado, exportado o rechazado por Rust. */
    fun deleteStagedFile(context: Context, path: String?) {
        if (path.isNullOrBlank()) return
        try {
            val stagingRoot = File(context.cacheDir, STAGING_DIR_NAME).canonicalFile
            val target = File(path).canonicalFile
            if (target.path.startsWith(stagingRoot.path + File.separator) && target.isFile) {
                target.delete()
            }
        } catch (_: Exception) {
        }
    }

    /** Barre archivos temporales rezagados en `cacheDir/import_staging/` (p. ej. tras cierre inesperado). */
    fun sweepStaleStagingFiles(context: Context, maxAgeMs: Long = 3_600_000L) {
        try {
            val stagingDir = File(context.cacheDir, STAGING_DIR_NAME)
            if (!stagingDir.isDirectory) return
            val now = System.currentTimeMillis()
            stagingDir.listFiles()?.forEach { file ->
                if (file.isFile && (maxAgeMs <= 0L || (now - file.lastModified()) >= maxAgeMs)) {
                    file.delete()
                }
            }
        } catch (_: Exception) {
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
