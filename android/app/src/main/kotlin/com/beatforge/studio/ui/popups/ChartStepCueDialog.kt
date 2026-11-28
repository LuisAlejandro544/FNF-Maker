// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/ChartStepCueDialog.kt
// Propósito: Ventana modal interactiva activada al pulsar cualquier número de paso `#` en
// el Editor de Chart. Coordina las 5 pestañas modulares en `ui/popups/chart_cue/`
// (Visibilidad, Flash, Personajes, UI/Negro y Cámara) más la lista de eventos activos.

package com.beatforge.studio.ui.popups

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Code
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.beatforge.studio.ChartEventSnapshot
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.popups.chart_cue.CueBlackoutTab
import com.beatforge.studio.ui.popups.chart_cue.CueCameraTab
import com.beatforge.studio.ui.popups.chart_cue.CueCharactersTab
import com.beatforge.studio.ui.popups.chart_cue.CueFlashTab
import com.beatforge.studio.ui.popups.chart_cue.CueTabButton
import com.beatforge.studio.ui.popups.chart_cue.CueVisibilityTab
import com.beatforge.studio.ui.popups.chart_cue.rememberChartCueDialogState
import com.beatforge.studio.ui.screens.chart.ChartTimelineRow
import com.beatforge.studio.ui.screens.chart.resolveChartStageStateAtStep
import com.beatforge.studio.ui.screens.chart.summarizeChartEvent
import com.beatforge.studio.ui.theme.StudioColors
import java.util.Locale
import kotlin.math.roundToInt

@Composable
fun ChartStepCueDialog(
    stepRow: ChartTimelineRow.StepRow,
    entities: List<StageEntitySnapshot>,
    chartEvents: List<ChartEventSnapshot>,
    stageCamera: StageCameraSnapshot,
    allStepRows: List<ChartTimelineRow.StepRow> = emptyList(),
    onAddChartEvent: (timeMs: Double, name: String, value1: String, value2: String) -> Unit,
    onRemoveChartEvent: (globalIndex: Int) -> Unit,
    onMovePlayheadHere: (Double) -> Unit,
    onOpenStageEventsScript: () -> Unit,
    onOpenUiBlackoutScript: () -> Unit,
    onDismiss: () -> Unit
) {
    val futureSteps = remember(allStepRows, stepRow.globalStepIndex) {
        allStepRows.filter { it.globalStepIndex > stepRow.globalStepIndex }
    }
    val defaultEndStepGlobalIdx = remember(futureSteps, stepRow.globalStepIndex) {
        futureSteps.firstOrNull { it.globalStepIndex >= stepRow.globalStepIndex + 8 }?.globalStepIndex
            ?: futureSteps.lastOrNull()?.globalStepIndex
            ?: stepRow.globalStepIndex
    }

    val stageStateAtStep = remember(
        entities,
        chartEvents,
        stepRow.globalStepIndex,
        stepRow.stepTimeMs,
        stepRow.stepMs
    ) {
        resolveChartStageStateAtStep(
            entities = entities,
            events = chartEvents,
            stepTimeMs = stepRow.stepTimeMs,
            stepMs = stepRow.stepMs
        )
    }
    val activePlayerName = entities.firstOrNull { it.id == stageStateAtStep.activePlayerId }?.displayName
        ?: stageStateAtStep.activePlayerId.ifBlank { "Sin asignar" }
    val activeOpponentName = entities.firstOrNull { it.id == stageStateAtStep.activeOpponentId }?.displayName
        ?: stageStateAtStep.activeOpponentId.ifBlank { "Sin asignar" }
    val visibleCountAtStep = entities.count { stageStateAtStep.visibleIds.contains(it.id) }

    val state = rememberChartCueDialogState(
        stepRow = stepRow,
        stageCamera = stageCamera,
        defaultEndStepGlobalIdx = defaultEndStepGlobalIdx
    )

    val secNum = stepRow.section.index + 1
    val secTimeText = String.format(Locale.US, "%.2fs", stepRow.stepTimeMs / 1000.0)

    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false)
    ) {
        Surface(
            shape = RoundedCornerShape(14.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.65f)),
            modifier = Modifier
                .fillMaxWidth(0.88f)
                .heightIn(max = 490.dp)
                .padding(10.dp)
        ) {
            Column(
                modifier = Modifier
                    .padding(14.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = "Eventos en Paso #${stepRow.stepInSec} (Sec. $secNum)",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 15.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis
                        )
                        Text(
                            text = "Marca temporal: $secTimeText (${stepRow.stepTimeMs.roundToInt()} ms)",
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis
                        )
                    }
                    Spacer(Modifier.width(10.dp))
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                        OutlinedButton(
                            onClick = {
                                onMovePlayheadHere(stepRow.stepTimeMs)
                                onDismiss()
                            },
                            contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                            modifier = Modifier.height(30.dp)
                        ) {
                            Icon(Icons.Rounded.PlayArrow, null, tint = StudioColors.AccentEmerald, modifier = Modifier.size(14.dp))
                            Spacer(Modifier.width(4.dp))
                            Text("Hilo aquí", fontSize = 10.sp, maxLines = 1, softWrap = false)
                        }
                        OutlinedButton(
                            onClick = {
                                onDismiss()
                                onOpenStageEventsScript()
                            },
                            contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                            modifier = Modifier.height(30.dp)
                        ) {
                            Icon(Icons.Rounded.Code, null, tint = StudioColors.AccentPurple, modifier = Modifier.size(14.dp))
                            Spacer(Modifier.width(4.dp))
                            Text("stage_events.lua", fontSize = 10.sp, maxLines = 1, softWrap = false)
                        }
                    }
                }

                // Selector de pestañas modular
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(5.dp)
                ) {
                    CueTabButton(
                        label = "Visibilidad",
                        selected = state.activeTab == 0,
                        accent = StudioColors.AccentEmerald,
                        onClick = { state.activeTab = 0 },
                        modifier = Modifier.weight(0.95f)
                    )
                    CueTabButton(
                        label = "Flash",
                        selected = state.activeTab == 1,
                        accent = StudioColors.AccentAmber,
                        onClick = { state.activeTab = 1 },
                        modifier = Modifier.weight(0.65f)
                    )
                    CueTabButton(
                        label = "Personajes",
                        selected = state.activeTab == 2,
                        accent = StudioColors.AccentPurple,
                        onClick = { state.activeTab = 2 },
                        modifier = Modifier.weight(0.9f)
                    )
                    CueTabButton(
                        label = "UI/Negro",
                        selected = state.activeTab == 3,
                        accent = StudioColors.AccentCoral,
                        onClick = { state.activeTab = 3 },
                        modifier = Modifier.weight(0.8f)
                    )
                    CueTabButton(
                        label = "Cámara",
                        selected = state.activeTab == 4,
                        accent = StudioColors.AccentCyan,
                        onClick = { state.activeTab = 4 },
                        modifier = Modifier.weight(0.75f)
                    )
                }

                HorizontalDivider(color = StudioColors.BorderSubtle)

                when (state.activeTab) {
                    0 -> CueVisibilityTab(
                        stepRow = stepRow,
                        entities = entities,
                        stageStateAtStep = stageStateAtStep,
                        activePlayerName = activePlayerName,
                        activeOpponentName = activeOpponentName,
                        visibleCountAtStep = visibleCountAtStep,
                        state = state,
                        onAddChartEvent = onAddChartEvent
                    )

                    1 -> CueFlashTab(
                        stepRow = stepRow,
                        state = state,
                        onAddChartEvent = onAddChartEvent
                    )

                    2 -> CueCharactersTab(
                        stepRow = stepRow,
                        entities = entities,
                        stageStateAtStep = stageStateAtStep,
                        activePlayerName = activePlayerName,
                        activeOpponentName = activeOpponentName,
                        visibleCountAtStep = visibleCountAtStep,
                        state = state,
                        onAddChartEvent = onAddChartEvent
                    )

                    3 -> CueBlackoutTab(
                        stepRow = stepRow,
                        secTimeText = secTimeText,
                        futureSteps = futureSteps,
                        state = state,
                        onAddChartEvent = onAddChartEvent,
                        onOpenUiBlackoutScript = onOpenUiBlackoutScript,
                        onDismiss = onDismiss
                    )

                    4 -> CueCameraTab(
                        stepRow = stepRow,
                        entities = entities,
                        stageCamera = stageCamera,
                        stageStateAtStep = stageStateAtStep,
                        futureSteps = futureSteps,
                        state = state,
                        onAddChartEvent = onAddChartEvent
                    )
                }

                if (stepRow.rowEvents.isNotEmpty()) {
                    HorizontalDivider(color = StudioColors.BorderSubtle)
                    Text(
                        text = "Eventos activos en Paso #${stepRow.stepInSec} (${stepRow.rowEvents.size}):",
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 12.sp
                    )
                    stepRow.rowEvents.forEach { indexed ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .background(StudioColors.BgCard, RoundedCornerShape(7.dp))
                                .padding(horizontal = 8.dp, vertical = 5.dp),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Text(
                                text = summarizeChartEvent(indexed.event),
                                color = StudioColors.TextPrimary,
                                fontSize = 11.sp,
                                modifier = Modifier.weight(1f)
                            )
                            IconButton(
                                onClick = { onRemoveChartEvent(indexed.globalIndex) },
                                modifier = Modifier.size(26.dp)
                            ) {
                                Icon(Icons.Rounded.Delete, "Eliminar", tint = StudioColors.AccentCoral, modifier = Modifier.size(15.dp))
                            }
                        }
                    }
                }

                OutlinedButton(onClick = onDismiss, modifier = Modifier.fillMaxWidth()) {
                    Text("Cerrar")
                }
            }
        }
    }
}
