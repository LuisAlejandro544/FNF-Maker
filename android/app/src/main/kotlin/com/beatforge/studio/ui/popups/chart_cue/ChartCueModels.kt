// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/chart_cue/ChartCueModels.kt
// Propósito: Enums, paletas preestablecidas y contenedor de estado reactivo
// (`ChartCueDialogState`) compartido entre las pestañas del modal `ChartStepCueDialog`.

package com.beatforge.studio.ui.popups.chart_cue

import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.ui.screens.chart.ChartTimelineRow
import kotlin.math.roundToInt

internal enum class VisibilityAction {
    Keep,
    Show,
    Hide
}

internal val FLASH_PRESET_COLORS = listOf(
    "#FFFFFF" to "Blanco",
    "#EF4444" to "Rojo",
    "#06B6D4" to "Cian",
    "#F59E0B" to "Ámbar",
    "#EC4899" to "Magenta",
    "#000000" to "Negro"
)

internal val VIGNETTE_PRESET_COLORS = listOf(
    "#EF4444" to "Rojo",
    "#DC2626" to "Carmesí",
    "#3B82F6" to "Azul",
    "#06B6D4" to "Cian",
    "#A855F7" to "Morado",
    "#F59E0B" to "Ámbar",
    "#10B981" to "Verde",
    "#09090B" to "Negro"
)

internal class ChartCueDialogState(
    stepRow: ChartTimelineRow.StepRow,
    stageCamera: StageCameraSnapshot,
    defaultEndStepGlobalIdx: Int
) {
    var activeTab by mutableIntStateOf(0)

    // Tab 0: Visibilidad & Swap
    val visibilityChoices = mutableStateMapOf<String, VisibilityAction>()
    var disposeOldOnSwap by mutableStateOf(true)

    // Tab 1: Flash
    var flashColorHex by mutableStateOf("#FFFFFF")
    var flashDurationSteps by mutableIntStateOf(4)
    var flashAlpha by mutableFloatStateOf(0.85f)
    var flashOverHud by mutableStateOf(false)

    // Tab 2: Personajes & Precarga
    var selectedPlayerId by mutableStateOf("")
    var selectedOpponentId by mutableStateOf("")
    var disposeOldCharacter by mutableStateOf(true)
    val manualPreloadIds = mutableStateMapOf<String, Boolean>()

    // Tab 3: UI / Pantalla en Negro
    var uiPresetAction by mutableStateOf(if (stepRow.stepTimeMs <= 120.0) "intro" else "hide")
    var uiStartAtZero by mutableStateOf(stepRow.stepTimeMs <= 120.0)
    var uiEndMode by mutableStateOf(if (stepRow.stepTimeMs <= 120.0) "on_ya" else "step")
    var uiEndStepGlobalIdx by mutableIntStateOf(defaultEndStepGlobalIdx)
    var uiEndMenuExpanded by mutableStateOf(false)
    var uiBlackAlpha by mutableFloatStateOf(1.0f)
    var uiHideHud by mutableStateOf(true)
    var uiHideArrows by mutableStateOf(true)
    var uiFadeSec by mutableFloatStateOf(0.50f)
    var uiRestoreFadeSec by mutableFloatStateOf(0.65f)
    var uiIntroDelayMs by mutableFloatStateOf(0.0f)

    // Tab 4: Cámara & Viñeta Lateral
    private val cameraSlots = stageCamera.resolvedCameras()
    private val initialSlot = stageCamera.activeCameraIndex.coerceIn(0, (cameraSlots.size - 1).coerceAtLeast(0))
    var camTargetSlot by mutableIntStateOf(initialSlot)
    var camOverrideZoom by mutableStateOf(false)
    var camCustomZoom by mutableFloatStateOf(cameraSlots.getOrNull(initialSlot)?.zoom ?: stageCamera.baseZoom)
    var camTransDurSec by mutableFloatStateOf(0.0f)
    var camEase by mutableStateOf("sineOut")
    var camFollowPosX by mutableStateOf(stageCamera.baseX.roundToInt().toString())
    var camFollowPosY by mutableStateOf(stageCamera.baseY.roundToInt().toString())
    var camZoomOnly by mutableFloatStateOf(1.20f)
    var camZoomSteps by mutableIntStateOf(4)

    var vigMode by mutableStateOf("on")
    var vigColorHex by mutableStateOf(stageCamera.vignetteColor.ifBlank { "#EF4444" })
    var vigAlpha by mutableFloatStateOf(stageCamera.vignetteAlpha.coerceIn(0.15f, 1.0f))
    var vigWidth by mutableFloatStateOf(stageCamera.vignetteWidth.coerceIn(0.05f, 0.50f))
    var vigPulse by mutableFloatStateOf(stageCamera.vignettePulse.coerceIn(0.0f, 1.0f))
    var vigOverHud by mutableStateOf(stageCamera.vignetteOverHud)
    var vigFadeSec by mutableFloatStateOf(0.35f)
    var vigEndStepGlobalIdx by mutableIntStateOf(defaultEndStepGlobalIdx)
    var vigEndMenuExpanded by mutableStateOf(false)
}

@Composable
internal fun rememberChartCueDialogState(
    stepRow: ChartTimelineRow.StepRow,
    stageCamera: StageCameraSnapshot,
    defaultEndStepGlobalIdx: Int
): ChartCueDialogState {
    return remember(stepRow.globalStepIndex) {
        ChartCueDialogState(
            stepRow = stepRow,
            stageCamera = stageCamera,
            defaultEndStepGlobalIdx = defaultEndStepGlobalIdx
        )
    }
}
