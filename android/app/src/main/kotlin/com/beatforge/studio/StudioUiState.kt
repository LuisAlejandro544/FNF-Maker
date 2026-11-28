// Archivo: android/app/src/main/kotlin/com/beatforge/studio/StudioUiState.kt
// Propósito: Define los modelos de estado reactivo de la interfaz principal (`StudioUiState`)
// y el contenedor de bitmaps con dimensiones originales (`LoadedEntityBitmap`) consumidos por
// las pantallas de Jetpack Compose y los delegados modulares de `StudioViewModel`.

package com.beatforge.studio

import androidx.compose.ui.graphics.ImageBitmap
import com.beatforge.studio.gfx.GfxCapabilities

data class LoadedEntityBitmap(
    val bitmap: ImageBitmap,
    val origWidth: Int,
    val origHeight: Int,
    val extraPages: List<LoadedEntityBitmap> = emptyList()
) {
    fun pageAt(pageIndex: Int): LoadedEntityBitmap {
        if (pageIndex <= 0) return this
        return extraPages.getOrNull(pageIndex - 1) ?: this
    }
}

data class AudioPreviewTransportState(
    val posMs: Double = 0.0,
    val isPlaying: Boolean = false,
    val durationMs: Double = 0.0
)

data class StudioUiState(
    val initialized: Boolean = false,
    val hasSelectedProject: Boolean = false,
    val isLoadingProject: Boolean = false,
    val loadingProjectName: String = "",
    val loadingPhase: String = "Preparando estudio...",
    val loadingProgress: Float = 0f,
    val currentScreen: StudioScreen = StudioScreen.Projects,
    val snapshot: ProjectSnapshot = ProjectSnapshot(),
    val projects: List<ProjectSummary> = emptyList(),
    val selectedEntityId: String = "",
    val selectedScriptId: String = "",
    val arrowSkinDialogOpen: Boolean = false,
    val characterIconsDialogOpen: Boolean = false,
    val modFilesGuideDialogOpen: Boolean = false,
    val countdownAssetsDialogOpen: Boolean = false,
    val devApiDialogOpen: Boolean = false,
    val stagedArrowAtlasPngPath: String? = null,
    val stagedArrowAtlasXmlPath: String? = null,
    val audioPreviewPlaying: Boolean = false,
    val selectedTier: DeviceTier = DeviceTier.Balanced,
    val gfxCapabilities: GfxCapabilities = GfxCapabilities(),
    val hotCompileProgress: HotCompileProgress = HotCompileProgress(),
    val audioAdaptProgress: AudioAdaptProgress = AudioAdaptProgress(),
    val debugShowRam: Boolean = false,
    val debugShowTelemetry: Boolean = false,
    val debugBotplay: Boolean = false,
    val debugPracticeMode: Boolean = false,
    val debugHideHud: Boolean = false,
    val isBusy: Boolean = false,
    val statusBanner: String? = null,
    val statusIsError: Boolean = false,
    val scriptValidationMsg: String? = null,
    val scriptValidationOk: Boolean = true,
    val arrowBitmaps: Map<Int, ImageBitmap> = emptyMap(),
    val receptorBitmaps: Map<Int, ImageBitmap> = emptyMap(),
    val hurtArrowXBitmap: ImageBitmap? = null,
    val healthBarBitmap: ImageBitmap? = null,
    val ratingBitmaps: Map<Int, ImageBitmap> = emptyMap(),
    val noteSplashBitmap: ImageBitmap? = null,
    val countdownBitmaps: Map<Int, LoadedEntityBitmap> = emptyMap(),
    val defaultPlayerNormalIcon: ImageBitmap? = null,
    val defaultPlayerLosingIcon: ImageBitmap? = null,
    val defaultOpponentNormalIcon: ImageBitmap? = null,
    val defaultOpponentLosingIcon: ImageBitmap? = null,
    val entityNormalIcons: Map<String, ImageBitmap> = emptyMap(),
    val entityLosingIcons: Map<String, ImageBitmap> = emptyMap(),
    val entityBitmaps: Map<String, LoadedEntityBitmap> = emptyMap(),
    val compiledBitmaps: Map<String, LoadedEntityBitmap> = emptyMap(),
    val compiledBitmapsByIndex: List<LoadedEntityBitmap?> = emptyList()
)
