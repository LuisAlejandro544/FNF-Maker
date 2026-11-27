// Archivo: android/app/src/main/kotlin/com/beatforge/studio/viewmodel/editor/SafImportExportDelegate.kt
// Propósito: Subdelegado encargado de importaciones y exportaciones vía Android SAF
// sobre `cacheDir/import_staging/`: exportación de `chart.json` y `.mod` Vanilla,
// importación multi-formato (`ImportKind`) y flujo en dos pasos para `NOTE_assets.png + .xml`.

package com.beatforge.studio.viewmodel.editor

import android.net.Uri
import com.beatforge.studio.ImportKind
import com.beatforge.studio.NativeEngineBridge
import com.beatforge.studio.SafFileBridge
import com.beatforge.studio.StudioJsonParser
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import java.io.File

internal class SafImportExportDelegate(
    private val helper: EditorStateHelper
) {
    fun exportChartToUri(uri: Uri) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.setBusy(true)
            try {
                val jsonText = NativeEngineBridge.nativeExportChartJson()
                val os = helper.app.contentResolver.openOutputStream(uri)
                if (os == null) {
                    helper.postBanner("No se pudo abrir destino para exportar chart.json", isError = true)
                } else {
                    os.use { stream ->
                        stream.write(jsonText.toByteArray(Charsets.UTF_8))
                        stream.flush()
                    }
                    helper.postBanner("chart.json completo exportado correctamente", isError = false)
                }
            } catch (e: Exception) {
                helper.postBanner("Error exportando chart.json: ${e.message}", isError = true)
            }
            helper.setBusy(false)
        }
    }

    fun exportModPackageToUri(projectId: String, uri: Uri) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.setBusy(true)
            val stagedOut = SafFileBridge.createExportStagingFile(helper.app, "vanilla_export.mod")
            val stagedBase = stagedOut.nameWithoutExtension
            try {
                val rawJson = NativeEngineBridge.nativeExportModPackage(
                    projectId = projectId,
                    outPath = stagedOut.absolutePath,
                    optimizeImages = true
                )
                val res = StudioJsonParser.parseActionResult(rawJson)
                if (!res.ok || !stagedOut.isFile) {
                    helper.postBanner(
                        res.message.ifBlank { "No se pudo empaquetar el mod en Vanilla .mod" },
                        isError = true
                    )
                } else {
                    val os = helper.app.contentResolver.openOutputStream(uri)
                    if (os == null) {
                        helper.postBanner("No se pudo abrir el destino para guardar el archivo .mod", isError = true)
                    } else {
                        os.use { output ->
                            stagedOut.inputStream().use { input ->
                                input.copyTo(output, bufferSize = 64 * 1024)
                            }
                            output.flush()
                        }
                        helper.postBanner(res.message, isError = false)
                    }
                }
            } catch (e: Exception) {
                helper.postBanner("Error exportando Vanilla .mod: ${e.message}", isError = true)
            } finally {
                SafFileBridge.deleteStagedFile(helper.app, stagedOut.absolutePath)
                stagedOut.parent?.let { dir ->
                    SafFileBridge.deleteStagedFile(helper.app, "$dir/$stagedBase.mod.blobs.tmp")
                    SafFileBridge.deleteStagedFile(helper.app, "$dir/$stagedBase.mod.tmp")
                }
                helper.setBusy(false)
            }
        }
    }

    fun importUri(kind: ImportKind, uri: Uri) {
        helper.scope.launch(helper.engineIoDispatcher) {
            helper.setBusy(true)
            val localPath = SafFileBridge.copyUriToStagingCache(helper.app, uri, kind.tag)
            if (localPath == null) {
                helper.postBanner("No se pudo copiar el archivo seleccionado", isError = true)
                helper.setBusy(false)
                return@launch
            }

            var deleteLocalInFinally = true
            try {
                when (kind) {
                    is ImportKind.ModPackage -> {
                        if (helper.uiState.value.audioPreviewPlaying) {
                            NativeEngineBridge.nativeToggleAudioPreview(false, 0.0)
                        }
                        helper.beginLoadingOverlay("Importando paquete .mod...", 0.25f, "Extrayendo archivos del mod...")
                        val rawJson = NativeEngineBridge.nativeImportModPackage(localPath)
                        helper.applyProjectSwitchResult(rawJson, navigateToHub = true)
                    }

                    is ImportKind.ArrowAtlasPng -> {
                        deleteLocalInFinally = false
                        helper.uiState.value.stagedArrowAtlasPngPath?.let { old ->
                            SafFileBridge.deleteStagedFile(helper.app, old)
                        }
                        helper.uiState.update { it.copy(stagedArrowAtlasPngPath = localPath) }
                        maybeApplyStagedArrowAtlas()
                    }

                    is ImportKind.ArrowAtlasXml -> {
                        deleteLocalInFinally = false
                        helper.uiState.value.stagedArrowAtlasXmlPath?.let { old ->
                            SafFileBridge.deleteStagedFile(helper.app, old)
                        }
                        helper.uiState.update { it.copy(stagedArrowAtlasXmlPath = localPath) }
                        maybeApplyStagedArrowAtlas()
                    }

                    else -> {
                        val rawJson = when (kind) {
                            is ImportKind.Chart -> NativeEngineBridge.nativeImportChart(localPath)
                            is ImportKind.AudioTrack -> NativeEngineBridge.nativeImportAudioTrack(kind.slot, localPath)
                            is ImportKind.SpritePng -> NativeEngineBridge.nativeImportSpriteSheetPng(kind.entityId, localPath)
                            is ImportKind.SparrowXml -> NativeEngineBridge.nativeImportSparrowXml(kind.entityId, localPath)
                            is ImportKind.CharacterConfig -> NativeEngineBridge.nativeImportCharacterConfig(kind.entityId, localPath)
                            is ImportKind.StageConfig -> NativeEngineBridge.nativeImportStageConfig(localPath)
                            is ImportKind.Script -> NativeEngineBridge.nativeImportScript(localPath)
                            is ImportKind.ArrowSvg -> NativeEngineBridge.nativeImportArrowSvg(kind.direction, localPath)
                            is ImportKind.CharacterIcon -> NativeEngineBridge.nativeImportCharacterIcon(kind.entityId, kind.isLosing, localPath)
                            is ImportKind.HealthBarSvg -> NativeEngineBridge.nativeImportHealthBarSvg(localPath)
                            is ImportKind.CountdownImage -> NativeEngineBridge.nativeImportCountdownSpriteImage(kind.slot, localPath)
                            is ImportKind.CountdownXml -> NativeEngineBridge.nativeImportCountdownSpriteXml(kind.slot, localPath)
                            is ImportKind.CountdownAudio -> NativeEngineBridge.nativeImportCountdownAudio(kind.slot, localPath)
                            is ImportKind.ModPackage,
                            is ImportKind.ArrowAtlasPng,
                            is ImportKind.ArrowAtlasXml -> error("handled above")
                        }

                        helper.applyActionJson(rawJson)
                        when (kind) {
                            is ImportKind.ArrowSvg -> helper.reloadArrowBitmaps()
                            is ImportKind.HealthBarSvg -> helper.reloadHealthBarBitmap()
                            is ImportKind.CharacterIcon -> helper.reloadCustomIcons(helper.uiState.value.snapshot.stageEntities)
                            is ImportKind.CountdownImage, is ImportKind.CountdownXml -> {
                                helper.reloadCountdownBitmaps(helper.uiState.value.snapshot.countdownSlots)
                            }
                            is ImportKind.SpritePng, is ImportKind.SparrowXml, is ImportKind.CharacterConfig, is ImportKind.StageConfig -> {
                                helper.bitmapLoader.evictRegionCache()
                                helper.reloadEntityBitmaps(helper.uiState.value.snapshot.stageEntities)
                            }
                            else -> Unit
                        }
                        helper.refreshProjectsInternal()
                    }
                }
            } finally {
                if (deleteLocalInFinally) {
                    SafFileBridge.deleteStagedFile(helper.app, localPath)
                }
                helper.endLoadingOverlay()
                helper.setBusy(false)
            }
        }
    }

    private fun maybeApplyStagedArrowAtlas() {
        val st = helper.uiState.value
        val pngPath = st.stagedArrowAtlasPngPath
        val xmlPath = st.stagedArrowAtlasXmlPath
        if (pngPath != null && xmlPath != null && File(pngPath).isFile && File(xmlPath).isFile) {
            val rawJson = NativeEngineBridge.nativeImportArrowAtlas(pngPath, xmlPath)
            helper.applyActionJson(rawJson)
            helper.reloadArrowBitmaps()
            SafFileBridge.deleteStagedFile(helper.app, pngPath)
            SafFileBridge.deleteStagedFile(helper.app, xmlPath)
            helper.uiState.update {
                it.copy(stagedArrowAtlasPngPath = null, stagedArrowAtlasXmlPath = null)
            }
            helper.refreshProjectsInternal()
        } else if (pngPath != null && xmlPath == null) {
            helper.postBanner("PNG de flechas listo. Ahora selecciona el archivo .xml (NOTE_assets.xml).", isError = false)
        } else if (xmlPath != null && pngPath == null) {
            helper.postBanner("XML de flechas listo. Ahora selecciona la hoja .png (NOTE_assets.png).", isError = false)
        }
    }

    fun clearStagedArrowAtlas() {
        val st = helper.uiState.value
        st.stagedArrowAtlasPngPath?.let { SafFileBridge.deleteStagedFile(helper.app, it) }
        st.stagedArrowAtlasXmlPath?.let { SafFileBridge.deleteStagedFile(helper.app, it) }
        helper.uiState.update { it.copy(stagedArrowAtlasPngPath = null, stagedArrowAtlasXmlPath = null) }
    }
}
