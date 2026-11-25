// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/chart_cue/CueVisibilityTab.kt
// Propósito: Pestaña 0 de `ChartStepCueDialog` para programar eventos `Stage Swap`
// (mostrar/ocultar entidades de escenario al instante y reciclar RAM opcionalmente).

package com.beatforge.studio.ui.popups.chart_cue

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.SwapHoriz
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.screens.chart.ChartStepStageState
import com.beatforge.studio.ui.screens.chart.ChartTimelineRow
import com.beatforge.studio.ui.theme.StudioColors

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun CueVisibilityTab(
    stepRow: ChartTimelineRow.StepRow,
    entities: List<StageEntitySnapshot>,
    stageStateAtStep: ChartStepStageState,
    activePlayerName: String,
    activeOpponentName: String,
    visibleCountAtStep: Int,
    state: ChartCueDialogState,
    onAddChartEvent: (timeMs: Double, name: String, value1: String, value2: String) -> Unit
) {
    StageStateSummaryBanner(
        stepInSec = stepRow.stepInSec,
        activePlayerName = activePlayerName,
        activeOpponentName = activeOpponentName,
        visibleCount = visibleCountAtStep,
        totalCount = entities.size
    )
    Text(
        text = "Muestra sprites ocultos al instante u oculta los actuales (escenario, fondos o decoración). Los indicadores muestran qué elementos están en uso en este paso.",
        color = StudioColors.TextMuted,
        fontSize = 10.sp,
        lineHeight = 13.sp
    )
    entities.forEach { ent ->
        val choice = state.visibilityChoices[ent.id] ?: VisibilityAction.Keep
        val isVisibleNow = stageStateAtStep.visibleIds.contains(ent.id)
        val isActivePlayer = ent.id == stageStateAtStep.activePlayerId
        val isActiveOpponent = ent.id == stageStateAtStep.activeOpponentId
        val changedHere = stageStateAtStep.changedAtThisStepIds.contains(ent.id)
        val changedEarlier = stageStateAtStep.changedEarlierIds.contains(ent.id)
        val cardBorderColor = when {
            isActivePlayer -> StudioColors.AccentEmerald.copy(alpha = 0.72f)
            isActiveOpponent -> StudioColors.AccentCoral.copy(alpha = 0.72f)
            isVisibleNow -> StudioColors.AccentCyan.copy(alpha = 0.48f)
            else -> StudioColors.BorderSubtle.copy(alpha = 0.55f)
        }
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(8.dp))
                .background(
                    if (isVisibleNow || isActivePlayer || isActiveOpponent) {
                        StudioColors.BgCard
                    } else {
                        StudioColors.BgCard.copy(alpha = 0.62f)
                    }
                )
                .border(1.dp, cardBorderColor, RoundedCornerShape(8.dp))
                .padding(horizontal = 8.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Column(
                modifier = Modifier
                    .weight(1f)
                    .padding(end = 6.dp),
                verticalArrangement = Arrangement.spacedBy(3.dp)
            ) {
                Text(
                    text = ent.displayName,
                    color = if (isVisibleNow || isActivePlayer || isActiveOpponent) {
                        StudioColors.TextPrimary
                    } else {
                        StudioColors.TextMuted
                    },
                    fontSize = 12.sp,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1
                )
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                    verticalArrangement = Arrangement.spacedBy(3.dp)
                ) {
                    if (isActivePlayer) {
                        StageStatusBadge(
                            label = "EN USO · JUGADOR",
                            color = StudioColors.AccentEmerald
                        )
                    }
                    if (isActiveOpponent) {
                        StageStatusBadge(
                            label = "EN USO · ENEMIGO",
                            color = StudioColors.AccentCoral
                        )
                    }
                    StageStatusBadge(
                        label = if (isVisibleNow) "VISIBLE" else "OCULTO",
                        color = if (isVisibleNow) StudioColors.AccentCyan else StudioColors.TextMuted
                    )
                    if (changedHere) {
                        StageStatusBadge(
                            label = "en este paso",
                            color = StudioColors.AccentAmber
                        )
                    } else if (changedEarlier) {
                        StageStatusBadge(
                            label = "evento previo",
                            color = StudioColors.AccentPurple
                        )
                    }
                }
                Text(
                    text = "${ent.roleLabel} · ${if (isVisibleNow) "En escena ahora" else "Oculto (en WebP / RAM libre)"}",
                    color = StudioColors.TextMuted,
                    fontSize = 9.sp
                )
            }
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                ActionChip(
                    label = "Mostrar",
                    selected = choice == VisibilityAction.Show,
                    color = StudioColors.AccentEmerald,
                    onClick = {
                        state.visibilityChoices[ent.id] = if (choice == VisibilityAction.Show) {
                            VisibilityAction.Keep
                        } else {
                            VisibilityAction.Show
                        }
                    }
                )
                ActionChip(
                    label = "Ocultar",
                    selected = choice == VisibilityAction.Hide,
                    color = StudioColors.AccentCoral,
                    onClick = {
                        state.visibilityChoices[ent.id] = if (choice == VisibilityAction.Hide) {
                            VisibilityAction.Keep
                        } else {
                            VisibilityAction.Hide
                        }
                    }
                )
            }
        }
    }

    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text("Liberar RAM de sprites ocultados", color = StudioColors.TextPrimary, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
            Text("Si vuelven más adelante, se recargarán en IO 4s antes.", color = StudioColors.TextMuted, fontSize = 9.sp)
        }
        Switch(checked = state.disposeOldOnSwap, onCheckedChange = { state.disposeOldOnSwap = it })
    }

    val showList = state.visibilityChoices.filterValues { it == VisibilityAction.Show }.keys.toList()
    val hideList = state.visibilityChoices.filterValues { it == VisibilityAction.Hide }.keys.toList()
    Button(
        enabled = showList.isNotEmpty() || hideList.isNotEmpty(),
        onClick = {
            val parts = mutableListOf<String>()
            if (showList.isNotEmpty()) parts.add("show=${showList.joinToString(",")}")
            if (hideList.isNotEmpty()) parts.add("hide=${hideList.joinToString(",")}")
            parts.add("dispose=${if (state.disposeOldOnSwap) 1 else 0}")
            onAddChartEvent(stepRow.stepTimeMs, "Stage Swap", parts.joinToString(";"), "")
            state.visibilityChoices.clear()
        },
        colors = ButtonDefaults.buttonColors(
            containerColor = StudioColors.AccentEmerald,
            contentColor = Color(0xFF042117)
        ),
        modifier = Modifier.fillMaxWidth()
    ) {
        Icon(Icons.Rounded.SwapHoriz, null, modifier = Modifier.size(16.dp))
        Spacer(Modifier.width(6.dp))
        Text("Aplicar Cambio de Escenario / Visibilidad", fontWeight = FontWeight.Bold, fontSize = 12.sp)
    }
}
