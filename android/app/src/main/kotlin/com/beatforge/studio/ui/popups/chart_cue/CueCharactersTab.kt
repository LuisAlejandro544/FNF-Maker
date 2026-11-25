// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/chart_cue/CueCharactersTab.kt
// Propósito: Pestaña 2 de `ChartStepCueDialog` para programar eventos `Change Character`
// (cambio de Jugador o Enemigo activo con reciclaje opcional) y `Preload Sprite` manual.

package com.beatforge.studio.ui.popups.chart_cue

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
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
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
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
internal fun CueCharactersTab(
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
        text = "Promueve cualquier personaje o sprite de escenario a Jugador u Oponente activo en este paso (cambia animaciones, cámara e íconos):",
        color = StudioColors.TextMuted,
        fontSize = 10.sp,
        lineHeight = 13.sp
    )

    Text("Nuevo Jugador Activo (player):", color = StudioColors.AccentEmerald, fontSize = 11.sp, fontWeight = FontWeight.Bold)
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        entities.forEach { ent ->
            val isSel = state.selectedPlayerId == ent.id
            val isCurrentPlayer = ent.id == stageStateAtStep.activePlayerId
            val isCurrentOpponent = ent.id == stageStateAtStep.activeOpponentId
            val isVisibleNow = stageStateAtStep.visibleIds.contains(ent.id)
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(7.dp))
                    .background(
                        when {
                            isSel -> StudioColors.AccentEmerald.copy(alpha = 0.22f)
                            isCurrentPlayer -> StudioColors.AccentEmerald.copy(alpha = 0.10f)
                            else -> StudioColors.BgCard
                        }
                    )
                    .border(
                        1.dp,
                        when {
                            isSel -> StudioColors.AccentEmerald
                            isCurrentPlayer -> StudioColors.AccentEmerald.copy(alpha = 0.65f)
                            isCurrentOpponent -> StudioColors.AccentCoral.copy(alpha = 0.45f)
                            else -> StudioColors.BorderSubtle
                        },
                        RoundedCornerShape(7.dp)
                    )
                    .clickable { state.selectedPlayerId = if (isSel) "" else ent.id }
                    .padding(horizontal = 8.dp, vertical = 5.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(
                    modifier = Modifier
                        .weight(1f)
                        .padding(end = 6.dp),
                    verticalArrangement = Arrangement.spacedBy(2.dp)
                ) {
                    Text("${ent.displayName} (${ent.roleLabel})", color = StudioColors.TextPrimary, fontSize = 11.sp, fontWeight = FontWeight.Medium)
                    FlowRow(
                        horizontalArrangement = Arrangement.spacedBy(4.dp),
                        verticalArrangement = Arrangement.spacedBy(2.dp)
                    ) {
                        if (isCurrentPlayer) {
                            StageStatusBadge("JUGADOR EN USO", StudioColors.AccentEmerald)
                        }
                        if (isCurrentOpponent) {
                            StageStatusBadge("ENEMIGO EN USO", StudioColors.AccentCoral)
                        }
                        StageStatusBadge(
                            if (isVisibleNow) "VISIBLE" else "OCULTO",
                            if (isVisibleNow) StudioColors.AccentCyan else StudioColors.TextMuted
                        )
                    }
                }
                Text(
                    text = when {
                        isSel -> "Elegido"
                        isCurrentPlayer -> "En uso"
                        else -> "Elegir"
                    },
                    color = when {
                        isSel || isCurrentPlayer -> StudioColors.AccentEmerald
                        else -> StudioColors.TextMuted
                    },
                    fontSize = 10.sp,
                    fontWeight = FontWeight.Bold
                )
            }
        }
    }

    Text("Nuevo Enemigo Activo (opp):", color = StudioColors.AccentCoral, fontSize = 11.sp, fontWeight = FontWeight.Bold)
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        entities.forEach { ent ->
            val isSel = state.selectedOpponentId == ent.id
            val isCurrentOpponent = ent.id == stageStateAtStep.activeOpponentId
            val isCurrentPlayer = ent.id == stageStateAtStep.activePlayerId
            val isVisibleNow = stageStateAtStep.visibleIds.contains(ent.id)
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(7.dp))
                    .background(
                        when {
                            isSel -> StudioColors.AccentCoral.copy(alpha = 0.22f)
                            isCurrentOpponent -> StudioColors.AccentCoral.copy(alpha = 0.10f)
                            else -> StudioColors.BgCard
                        }
                    )
                    .border(
                        1.dp,
                        when {
                            isSel -> StudioColors.AccentCoral
                            isCurrentOpponent -> StudioColors.AccentCoral.copy(alpha = 0.65f)
                            isCurrentPlayer -> StudioColors.AccentEmerald.copy(alpha = 0.45f)
                            else -> StudioColors.BorderSubtle
                        },
                        RoundedCornerShape(7.dp)
                    )
                    .clickable { state.selectedOpponentId = if (isSel) "" else ent.id }
                    .padding(horizontal = 8.dp, vertical = 5.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(
                    modifier = Modifier
                        .weight(1f)
                        .padding(end = 6.dp),
                    verticalArrangement = Arrangement.spacedBy(2.dp)
                ) {
                    Text("${ent.displayName} (${ent.roleLabel})", color = StudioColors.TextPrimary, fontSize = 11.sp, fontWeight = FontWeight.Medium)
                    FlowRow(
                        horizontalArrangement = Arrangement.spacedBy(4.dp),
                        verticalArrangement = Arrangement.spacedBy(2.dp)
                    ) {
                        if (isCurrentOpponent) {
                            StageStatusBadge("ENEMIGO EN USO", StudioColors.AccentCoral)
                        }
                        if (isCurrentPlayer) {
                            StageStatusBadge("JUGADOR EN USO", StudioColors.AccentEmerald)
                        }
                        StageStatusBadge(
                            if (isVisibleNow) "VISIBLE" else "OCULTO",
                            if (isVisibleNow) StudioColors.AccentCyan else StudioColors.TextMuted
                        )
                    }
                }
                Text(
                    text = when {
                        isSel -> "Elegido"
                        isCurrentOpponent -> "En uso"
                        else -> "Elegir"
                    },
                    color = when {
                        isSel || isCurrentOpponent -> StudioColors.AccentCoral
                        else -> StudioColors.TextMuted
                    },
                    fontSize = 10.sp,
                    fontWeight = FontWeight.Bold
                )
            }
        }
    }

    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text("Ocultar y reciclar personaje anterior (RAM)", color = StudioColors.TextPrimary, fontSize = 11.sp, modifier = Modifier.weight(1f))
        Switch(checked = state.disposeOldCharacter, onCheckedChange = { state.disposeOldCharacter = it })
    }

    Button(
        enabled = state.selectedPlayerId.isNotBlank() || state.selectedOpponentId.isNotBlank(),
        onClick = {
            val parts = mutableListOf<String>()
            if (state.selectedPlayerId.isNotBlank()) parts.add("player=${state.selectedPlayerId}")
            if (state.selectedOpponentId.isNotBlank()) parts.add("opp=${state.selectedOpponentId}")
            parts.add("dispose=${if (state.disposeOldCharacter) 1 else 0}")
            onAddChartEvent(stepRow.stepTimeMs, "Change Character", parts.joinToString(";"), "")
            state.selectedPlayerId = ""
            state.selectedOpponentId = ""
        },
        colors = ButtonDefaults.buttonColors(
            containerColor = StudioColors.AccentPurple,
            contentColor = Color.White
        ),
        modifier = Modifier.fillMaxWidth()
    ) {
        Icon(Icons.Rounded.Person, null, modifier = Modifier.size(16.dp))
        Spacer(Modifier.width(6.dp))
        Text("Guardar Cambio de Personaje", fontWeight = FontWeight.Bold, fontSize = 12.sp)
    }

    HorizontalDivider(color = StudioColors.BorderSubtle)
    Text("Precarga Manual Extra (opcional, el motor ya precarga 4s antes):", color = StudioColors.TextMuted, fontSize = 10.sp)
    Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
        entities.filter { !stageStateAtStep.visibleIds.contains(it.id) }.take(4).forEach { ent ->
            val checked = state.manualPreloadIds[ent.id] == true
            ActionChip(
                label = ent.displayName.take(10),
                selected = checked,
                color = StudioColors.AccentCyan,
                onClick = { state.manualPreloadIds[ent.id] = !checked },
                modifier = Modifier.weight(1f)
            )
        }
    }
    val chosenPreloads = state.manualPreloadIds.filterValues { it }.keys.toList()
    if (chosenPreloads.isNotEmpty()) {
        OutlinedButton(
            onClick = {
                onAddChartEvent(stepRow.stepTimeMs, "Preload Sprite", chosenPreloads.joinToString(","), "")
                state.manualPreloadIds.clear()
            },
            modifier = Modifier.fillMaxWidth()
        ) {
            Text("Añadir Precarga IO en Paso #${stepRow.stepInSec}", fontSize = 11.sp)
        }
    }
}
