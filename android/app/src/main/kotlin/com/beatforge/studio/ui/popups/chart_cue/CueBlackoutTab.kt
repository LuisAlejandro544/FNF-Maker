// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/chart_cue/CueBlackoutTab.kt
// Propósito: Pestaña 3 de `ChartStepCueDialog` para configurar eventos `UI Blackout`,
// `Hide Arrows`/`Show Arrows` con punto de inicio (0:00 o paso actual), punto final de
// regreso a color y acceso directo al script `ui_blackout.lua`.

package com.beatforge.studio.ui.popups.chart_cue

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ArrowDropDown
import androidx.compose.material.icons.rounded.Code
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material.icons.rounded.VisibilityOff
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.ui.screens.chart.ChartTimelineRow
import com.beatforge.studio.ui.theme.StudioColors
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToInt

@Composable
internal fun CueBlackoutTab(
    stepRow: ChartTimelineRow.StepRow,
    secTimeText: String,
    futureSteps: List<ChartTimelineRow.StepRow>,
    state: ChartCueDialogState,
    onAddChartEvent: (timeMs: Double, name: String, value1: String, value2: String) -> Unit,
    onOpenUiBlackoutScript: () -> Unit,
    onDismiss: () -> Unit
) {
    Text(
        text = "Configura dónde empieza a ponerse en negro / ocultar UI y en qué punto exacto termina y vuelve a su color normal.",
        color = StudioColors.TextMuted,
        fontSize = 10.sp,
        lineHeight = 13.sp
    )

    Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
        ActionChip(
            label = "Poner en Negro",
            selected = state.uiPresetAction == "hide" || state.uiPresetAction == "intro",
            color = StudioColors.AccentCoral,
            onClick = {
                state.uiPresetAction = if (state.uiStartAtZero) "intro" else "hide"
                state.uiBlackAlpha = 1.0f
                state.uiHideHud = true
                state.uiHideArrows = true
            },
            modifier = Modifier.weight(1.1f)
        )
        ActionChip(
            label = "Volver a Color",
            selected = state.uiPresetAction == "show",
            color = StudioColors.AccentEmerald,
            onClick = {
                state.uiPresetAction = "show"
                state.uiBlackAlpha = 0.0f
                state.uiHideHud = false
                state.uiHideArrows = false
            },
            modifier = Modifier.weight(1.0f)
        )
        ActionChip(
            label = "Solo Flechas",
            selected = state.uiPresetAction == "arrows_hide",
            color = StudioColors.AccentPurple,
            onClick = {
                state.uiPresetAction = "arrows_hide"
                state.uiBlackAlpha = 0.0f
                state.uiHideHud = false
                state.uiHideArrows = true
                if (state.uiEndMode == "on_ya") state.uiEndMode = "step"
            },
            modifier = Modifier.weight(0.95f)
        )
        ActionChip(
            label = "Alternar",
            selected = state.uiPresetAction == "toggle",
            color = StudioColors.AccentCyan,
            onClick = { state.uiPresetAction = "toggle" },
            modifier = Modifier.weight(0.8f)
        )
    }

    if (state.uiPresetAction != "show" && state.uiPresetAction != "toggle") {
        if (state.uiPresetAction != "arrows_hide") {
            Text(
                text = "1. Punto de Inicio:",
                color = StudioColors.AccentCoral,
                fontSize = 11.sp,
                fontWeight = FontWeight.Bold
            )
            Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
                ActionChip(
                    label = "En Paso #${stepRow.stepInSec} ($secTimeText)",
                    selected = !state.uiStartAtZero,
                    color = StudioColors.AccentCoral,
                    onClick = {
                        state.uiStartAtZero = false
                        state.uiPresetAction = "hide"
                        if (state.uiEndMode == "on_ya") {
                            state.uiEndMode = if (futureSteps.isNotEmpty()) "step" else "none"
                        }
                    },
                    modifier = Modifier.weight(1f)
                )
                ActionChip(
                    label = "Inicio 0:00 (2-1-¡YA!)",
                    selected = state.uiStartAtZero,
                    color = StudioColors.AccentAmber,
                    onClick = {
                        state.uiStartAtZero = true
                        state.uiPresetAction = "intro"
                    },
                    modifier = Modifier.weight(1f)
                )
            }
        }

        if (state.uiPresetAction != "arrows_hide") {
            Text(
                text = "Oscuridad de pantalla negra: ${(state.uiBlackAlpha * 100).roundToInt()}%",
                color = StudioColors.TextPrimary,
                fontSize = 11.sp
            )
            Slider(
                value = state.uiBlackAlpha,
                onValueChange = { state.uiBlackAlpha = it },
                valueRange = 0.0f..1.0f
            )
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Text("Ocultar barra de vida y puntaje (HUD)", color = StudioColors.TextPrimary, fontSize = 11.sp, modifier = Modifier.weight(1f))
                Switch(checked = state.uiHideHud, onCheckedChange = { state.uiHideHud = it })
            }
        }
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("Ocultar flechas y notas", color = StudioColors.TextPrimary, fontSize = 11.sp, modifier = Modifier.weight(1f))
            Switch(checked = state.uiHideArrows, onCheckedChange = { state.uiHideArrows = it })
        }

        HorizontalDivider(color = StudioColors.BorderSubtle)
        Text(
            text = "2. Punto Final (vuelve a su color y muestra UI):",
            color = StudioColors.AccentEmerald,
            fontSize = 11.sp,
            fontWeight = FontWeight.Bold
        )
        Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
            ActionChip(
                label = "Sin fin (Manual)",
                selected = state.uiEndMode == "none",
                color = StudioColors.TextMuted,
                onClick = { state.uiEndMode = "none" },
                modifier = Modifier.weight(1f)
            )
            if (state.uiStartAtZero && state.uiPresetAction != "arrows_hide") {
                ActionChip(
                    label = "Tras '¡YA!'",
                    selected = state.uiEndMode == "on_ya",
                    color = StudioColors.AccentAmber,
                    onClick = { state.uiEndMode = "on_ya" },
                    modifier = Modifier.weight(0.95f)
                )
            }
            if (futureSteps.isNotEmpty()) {
                ActionChip(
                    label = "Elegir Paso Fin",
                    selected = state.uiEndMode == "step",
                    color = StudioColors.AccentEmerald,
                    onClick = { state.uiEndMode = "step" },
                    modifier = Modifier.weight(1f)
                )
            }
        }

        if (state.uiEndMode == "on_ya" && state.uiStartAtZero) {
            Text(
                text = "Esperar tras '¡YA!' antes de volver al color:",
                color = StudioColors.AccentAmber,
                fontSize = 10.sp,
                fontWeight = FontWeight.SemiBold
            )
            Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
                listOf(0.0f to "Al ¡YA!", 500.0f to "+0.5s", 1000.0f to "+1s", 3000.0f to "+3s").forEach { (ms, label) ->
                    ActionChip(
                        label = label,
                        selected = abs(state.uiIntroDelayMs - ms) < 10f,
                        color = StudioColors.AccentAmber,
                        onClick = { state.uiIntroDelayMs = ms },
                        modifier = Modifier.weight(1f)
                    )
                }
            }
        } else if (state.uiEndMode == "step" && futureSteps.isNotEmpty()) {
            val selectedUiEndStep = futureSteps.firstOrNull { it.globalStepIndex == state.uiEndStepGlobalIdx }
                ?: futureSteps.last()
            Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.fillMaxWidth()) {
                listOf(4, 8, 16, 32).forEach { offset ->
                    val targetIdx = stepRow.globalStepIndex + offset
                    val candidate = futureSteps.firstOrNull { it.globalStepIndex == targetIdx }
                        ?: futureSteps.last()
                    ActionChip(
                        label = "+${offset}p",
                        selected = selectedUiEndStep.globalStepIndex == candidate.globalStepIndex,
                        color = StudioColors.AccentEmerald,
                        onClick = { state.uiEndStepGlobalIdx = candidate.globalStepIndex },
                        modifier = Modifier.weight(1f)
                    )
                }
            }
            Box(modifier = Modifier.fillMaxWidth()) {
                val endTimeTxt = String.format(Locale.US, "%.2fs", selectedUiEndStep.stepTimeMs / 1000.0)
                OutlinedButton(
                    onClick = { state.uiEndMenuExpanded = true },
                    modifier = Modifier.fillMaxWidth(),
                    contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp)
                ) {
                    Text(
                        text = "Vuelve a color en: Sec ${selectedUiEndStep.section.index + 1} · Paso #${selectedUiEndStep.stepInSec} ($endTimeTxt)",
                        fontSize = 11.sp,
                        color = StudioColors.AccentEmerald,
                        modifier = Modifier.weight(1f)
                    )
                    Icon(Icons.Rounded.ArrowDropDown, contentDescription = null, tint = StudioColors.AccentEmerald)
                }
                DropdownMenu(
                    expanded = state.uiEndMenuExpanded,
                    onDismissRequest = { state.uiEndMenuExpanded = false },
                    modifier = Modifier.heightIn(max = 240.dp)
                ) {
                    futureSteps.forEach { target ->
                        val tStr = String.format(Locale.US, "%.2fs", target.stepTimeMs / 1000.0)
                        DropdownMenuItem(
                            text = {
                                Text(
                                    text = "Sec ${target.section.index + 1} · Paso #${target.stepInSec} ($tStr)",
                                    fontSize = 11.sp
                                )
                            },
                            onClick = {
                                state.uiEndStepGlobalIdx = target.globalStepIndex
                                state.uiEndMenuExpanded = false
                            }
                        )
                    }
                }
            }
        }
    }

    Text(
        text = "Fade al iniciar: ${String.format(Locale.US, "%.2fs", state.uiFadeSec)}",
        color = StudioColors.TextPrimary,
        fontSize = 11.sp
    )
    Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
        listOf(0.0f to "Corte", 0.35f to "0.35s", 0.65f to "0.65s", 1.2f to "1.2s", 2.5f to "2.5s").forEach { (sec, label) ->
            ActionChip(
                label = label,
                selected = abs(state.uiFadeSec - sec) < 0.05f,
                color = StudioColors.AccentCoral,
                onClick = { state.uiFadeSec = sec },
                modifier = Modifier.weight(1f)
            )
        }
    }

    if (state.uiPresetAction != "show" && state.uiPresetAction != "toggle" && state.uiEndMode != "none") {
        Text(
            text = "Fade al volver al color: ${String.format(Locale.US, "%.2fs", state.uiRestoreFadeSec)}",
            color = StudioColors.AccentEmerald,
            fontSize = 11.sp
        )
        Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
            listOf(0.0f to "Corte", 0.35f to "0.35s", 0.65f to "0.65s", 1.2f to "1.2s", 2.5f to "2.5s").forEach { (sec, label) ->
                ActionChip(
                    label = label,
                    selected = abs(state.uiRestoreFadeSec - sec) < 0.05f,
                    color = StudioColors.AccentEmerald,
                    onClick = { state.uiRestoreFadeSec = sec },
                    modifier = Modifier.weight(1f)
                )
            }
        }
    }

    Button(
        onClick = {
            val targetBlack = if (state.uiPresetAction == "show") 0.0f else state.uiBlackAlpha
            val targetHud = when {
                state.uiPresetAction == "show" -> 1.0f
                state.uiHideHud -> 0.0f
                else -> 1.0f
            }
            val targetArrows = when {
                state.uiPresetAction == "show" -> 1.0f
                state.uiHideArrows -> 0.0f
                else -> 1.0f
            }

            val endStepTarget = if (state.uiPresetAction != "show" && state.uiPresetAction != "toggle" && state.uiEndMode == "step") {
                futureSteps.firstOrNull { it.globalStepIndex == state.uiEndStepGlobalIdx }
                    ?: futureSteps.lastOrNull()
            } else {
                null
            }

            val startTimeMs = if (state.uiStartAtZero && state.uiPresetAction == "intro") 0.0 else stepRow.stepTimeMs
            val effectiveAction = when {
                state.uiPresetAction == "arrows_hide" -> "hide"
                state.uiStartAtZero -> "intro"
                else -> state.uiPresetAction
            }
            val effectiveEventName = if (state.uiPresetAction == "arrows_hide") "Hide Arrows" else "UI Blackout"
            val revealOnSongStartFlag = state.uiStartAtZero && state.uiEndMode == "on_ya"
            val revealDelayVal = if (revealOnSongStartFlag) state.uiIntroDelayMs else 0.0f

            val kvParts = mutableListOf(
                String.format(Locale.US, "black=%.2f", targetBlack),
                String.format(Locale.US, "hud=%.2f", targetHud),
                String.format(Locale.US, "arrows=%.2f", targetArrows),
                String.format(Locale.US, "dur=%.2f", state.uiFadeSec),
                String.format(Locale.US, "revealMs=%.0f", revealDelayVal),
                "onStart=${if (revealOnSongStartFlag) 1 else 0}"
            )
            if (endStepTarget != null) {
                kvParts.add(String.format(Locale.US, "endMs=%.2f", endStepTarget.stepTimeMs))
                kvParts.add(String.format(Locale.US, "fadeOut=%.2f", state.uiRestoreFadeSec))
            }

            onAddChartEvent(startTimeMs, effectiveEventName, effectiveAction, kvParts.joinToString(";"))

            if (endStepTarget != null) {
                if (state.uiPresetAction == "arrows_hide") {
                    onAddChartEvent(
                        endStepTarget.stepTimeMs,
                        "Show Arrows",
                        "show",
                        String.format(Locale.US, "arrows=1.00;dur=%.2f", state.uiRestoreFadeSec)
                    )
                } else {
                    onAddChartEvent(
                        endStepTarget.stepTimeMs,
                        "UI Blackout",
                        "show",
                        String.format(Locale.US, "black=0.00;hud=1.00;arrows=1.00;dur=%.2f", state.uiRestoreFadeSec)
                    )
                }
            }
        },
        colors = ButtonDefaults.buttonColors(
            containerColor = if (state.uiPresetAction == "show") StudioColors.AccentEmerald else StudioColors.AccentCoral,
            contentColor = if (state.uiPresetAction == "show") Color(0xFF042117) else Color.White
        ),
        modifier = Modifier.fillMaxWidth()
    ) {
        Icon(
            if (state.uiPresetAction == "show") Icons.Rounded.Visibility else Icons.Rounded.VisibilityOff,
            null,
            modifier = Modifier.size(16.dp)
        )
        Spacer(Modifier.width(6.dp))
        Text(
            text = when {
                state.uiPresetAction == "show" -> "Volver a Color en Paso #${stepRow.stepInSec}"
                state.uiPresetAction == "arrows_hide" -> "Guardar Ocultar Flechas"
                state.uiEndMode == "step" && futureSteps.isNotEmpty() -> "Guardar Inicio → Fin (Vuelve a Color)"
                state.uiStartAtZero -> "Guardar Pantalla Negra desde 0:00"
                else -> "Aplicar Pantalla Negra / UI en Paso #${stepRow.stepInSec}"
            },
            fontWeight = FontWeight.Bold,
            fontSize = 12.sp
        )
    }

    OutlinedButton(
        onClick = {
            onDismiss()
            onOpenUiBlackoutScript()
        },
        modifier = Modifier.fillMaxWidth()
    ) {
        Icon(Icons.Rounded.Code, null, tint = StudioColors.AccentCoral, modifier = Modifier.size(15.dp))
        Spacer(Modifier.width(6.dp))
        Text("Editar script ui_blackout.lua", fontSize = 11.sp)
    }
}
