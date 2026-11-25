// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/chart_cue/CueCameraTab.kt
// Propósito: Pestaña 4 de `ChartStepCueDialog` para programar eventos de cámara
// (`Switch Camera`, `Camera Follow Pos`, `ZoomCamera`, `Add Camera Zoom`) y viñeta lateral.

package com.beatforge.studio.ui.popups.chart_cue

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ArrowDropDown
import androidx.compose.material.icons.rounded.Videocam
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
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
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.screens.chart.ChartStepStageState
import com.beatforge.studio.ui.screens.chart.ChartTimelineRow
import com.beatforge.studio.ui.screens.chart.parseHexColorOrWhite
import com.beatforge.studio.ui.theme.StudioColors
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToInt

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun CueCameraTab(
    stepRow: ChartTimelineRow.StepRow,
    entities: List<StageEntitySnapshot>,
    stageCamera: StageCameraSnapshot,
    stageStateAtStep: ChartStepStageState,
    futureSteps: List<ChartTimelineRow.StepRow>,
    state: ChartCueDialogState,
    onAddChartEvent: (timeMs: Double, name: String, value1: String, value2: String) -> Unit
) {
    val cameraSlots = stageCamera.resolvedCameras()

    Text(
        text = "Controla cámaras directamente desde chart.json: cambia entre Cam 0/1/2/3+, bloquea coordenadas (Camera Follow Pos), ajusta FOV (ZoomCamera) o lanza golpes de zoom.",
        color = StudioColors.TextMuted,
        fontSize = 10.sp,
        lineHeight = 13.sp
    )

    Text("1. Cambiar Ranura de Cámara (Switch Camera):", color = StudioColors.AccentCyan, fontSize = 11.sp, fontWeight = FontWeight.Bold)
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(5.dp)
    ) {
        cameraSlots.take(5).forEach { slot ->
            val isSel = state.camTargetSlot == slot.index
            ActionChip(
                label = "Cam ${slot.index}",
                selected = isSel,
                color = StudioColors.AccentCyan,
                onClick = {
                    state.camTargetSlot = slot.index
                    state.camCustomZoom = slot.zoom
                },
                modifier = Modifier.weight(1f)
            )
        }
    }
    if (cameraSlots.size > 5) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(5.dp)
        ) {
            cameraSlots.drop(5).take(5).forEach { slot ->
                val isSel = state.camTargetSlot == slot.index
                ActionChip(
                    label = "Cam ${slot.index}",
                    selected = isSel,
                    color = StudioColors.AccentCyan,
                    onClick = {
                        state.camTargetSlot = slot.index
                        state.camCustomZoom = slot.zoom
                    },
                    modifier = Modifier.weight(1f)
                )
            }
        }
    }
    val chosenSlotInfo = cameraSlots.firstOrNull { it.index == state.camTargetSlot }
    if (chosenSlotInfo != null) {
        Text(
            text = "Seleccionada: ${chosenSlotInfo.name} · FOV base ${((chosenSlotInfo.zoom * 100f).roundToInt() / 100f)}x",
            color = StudioColors.TextMuted,
            fontSize = 10.sp
        )
    }

    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Text("Forzar FOV/Zoom específico en este evento", color = StudioColors.TextPrimary, fontSize = 11.sp, modifier = Modifier.weight(1f))
        Switch(checked = state.camOverrideZoom, onCheckedChange = { state.camOverrideZoom = it })
    }
    if (state.camOverrideZoom) {
        Text(
            text = "Zoom FOV: ${((state.camCustomZoom * 100f).roundToInt() / 100f)}x",
            color = StudioColors.AccentCyan,
            fontSize = 11.sp,
            fontWeight = FontWeight.SemiBold
        )
        Slider(
            value = state.camCustomZoom.coerceIn(0.35f, 2.5f),
            onValueChange = { state.camCustomZoom = it },
            valueRange = 0.35f..2.5f
        )
    }

    Text("Duración e interpolación (paneo):", color = StudioColors.TextPrimary, fontSize = 11.sp)
    Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
        listOf(0.0f to "Corte/Suave", 0.25f to "0.25s", 0.5f to "0.50s", 1.0f to "1.0s").forEach { (sec, label) ->
            ActionChip(
                label = label,
                selected = abs(state.camTransDurSec - sec) < 0.05f,
                color = StudioColors.AccentCyan,
                onClick = { state.camTransDurSec = sec },
                modifier = Modifier.weight(1f)
            )
        }
    }
    Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
        listOf("sineOut" to "SineOut", "quadInOut" to "QuadInOut", "linear" to "Lineal").forEach { (key, label) ->
            ActionChip(
                label = label,
                selected = state.camEase == key,
                color = StudioColors.AccentIndigo,
                onClick = { state.camEase = key },
                modifier = Modifier.weight(1f)
            )
        }
    }

    Button(
        onClick = {
            val parts = mutableListOf("cam=${state.camTargetSlot}")
            if (state.camOverrideZoom) {
                parts.add(String.format(Locale.US, "zoom=%.2f", state.camCustomZoom))
            }
            if (state.camTransDurSec > 0.01f) {
                parts.add(String.format(Locale.US, "dur=%.2f", state.camTransDurSec))
                parts.add("ease=${state.camEase}")
            }
            onAddChartEvent(stepRow.stepTimeMs, "Switch Camera", parts.joinToString(";"), "")
        },
        colors = ButtonDefaults.buttonColors(
            containerColor = StudioColors.AccentCyan,
            contentColor = Color(0xFF041C24)
        ),
        modifier = Modifier.fillMaxWidth()
    ) {
        Icon(Icons.Rounded.Videocam, null, modifier = Modifier.size(16.dp))
        Spacer(Modifier.width(6.dp))
        Text("Cambiar a Cam ${state.camTargetSlot} en Paso #${stepRow.stepInSec}", fontWeight = FontWeight.Bold, fontSize = 12.sp)
    }

    HorizontalDivider(color = StudioColors.BorderSubtle)
    Text("2. Posición Fija en Escenario (Camera Follow Pos):", color = StudioColors.AccentEmerald, fontSize = 11.sp, fontWeight = FontWeight.Bold)
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.fillMaxWidth()) {
        OutlinedTextField(
            value = state.camFollowPosX,
            onValueChange = { state.camFollowPosX = it },
            label = { Text("X (ej. 640)", fontSize = 10.sp) },
            singleLine = true,
            modifier = Modifier.weight(1f)
        )
        OutlinedTextField(
            value = state.camFollowPosY,
            onValueChange = { state.camFollowPosY = it },
            label = { Text("Y (ej. 360)", fontSize = 10.sp) },
            singleLine = true,
            modifier = Modifier.weight(1f)
        )
    }
    Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
        ActionChip(
            label = "Cam 0",
            selected = false,
            color = StudioColors.AccentCyan,
            onClick = {
                state.camFollowPosX = stageCamera.baseX.roundToInt().toString()
                state.camFollowPosY = stageCamera.baseY.roundToInt().toString()
            },
            modifier = Modifier.weight(1f)
        )
        ActionChip(
            label = "Enemigo",
            selected = false,
            color = StudioColors.AccentCoral,
            onClick = {
                val opp = entities.firstOrNull { it.id == stageStateAtStep.activeOpponentId }
                    ?: entities.firstOrNull { it.role == "opponent" }
                state.camFollowPosX = (opp?.x ?: 300f).roundToInt().toString()
                state.camFollowPosY = ((opp?.y ?: 415f) - 24f).roundToInt().toString()
            },
            modifier = Modifier.weight(1f)
        )
        ActionChip(
            label = "Jugador",
            selected = false,
            color = StudioColors.AccentEmerald,
            onClick = {
                val pl = entities.firstOrNull { it.id == stageStateAtStep.activePlayerId }
                    ?: entities.firstOrNull { it.role == "player" }
                state.camFollowPosX = (pl?.x ?: 980f).roundToInt().toString()
                state.camFollowPosY = ((pl?.y ?: 420f) - 24f).roundToInt().toString()
            },
            modifier = Modifier.weight(1f)
        )
    }
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.fillMaxWidth()) {
        Button(
            onClick = {
                val xVal = state.camFollowPosX.trim().toFloatOrNull() ?: stageCamera.baseX
                val yVal = state.camFollowPosY.trim().toFloatOrNull() ?: stageCamera.baseY
                onAddChartEvent(
                    stepRow.stepTimeMs,
                    "Camera Follow Pos",
                    xVal.roundToInt().toString(),
                    yVal.roundToInt().toString()
                )
            },
            colors = ButtonDefaults.buttonColors(
                containerColor = StudioColors.AccentEmerald,
                contentColor = Color(0xFF042117)
            ),
            modifier = Modifier.weight(1f)
        ) {
            Text("Fijar Pos", fontSize = 11.sp, fontWeight = FontWeight.Bold)
        }
        OutlinedButton(
            onClick = {
                onAddChartEvent(stepRow.stepTimeMs, "Camera Follow Pos", "", "")
            },
            modifier = Modifier.weight(1f)
        ) {
            Text("Liberar Pos", fontSize = 11.sp)
        }
    }

    HorizontalDivider(color = StudioColors.BorderSubtle)
    Text("3. Campo de Visión / Zoom (ZoomCamera):", color = StudioColors.AccentAmber, fontSize = 11.sp, fontWeight = FontWeight.Bold)
    Text(
        text = "Zoom destino: ${((state.camZoomOnly * 100f).roundToInt() / 100f)}x · Duración ${state.camZoomSteps}p",
        color = StudioColors.TextPrimary,
        fontSize = 11.sp
    )
    Slider(
        value = state.camZoomOnly.coerceIn(0.4f, 2.2f),
        onValueChange = { state.camZoomOnly = it },
        valueRange = 0.4f..2.2f
    )
    Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
        listOf(0, 4, 8, 16).forEach { st ->
            ActionChip(
                label = if (st == 0) "Corte" else "${st}p",
                selected = state.camZoomSteps == st,
                color = StudioColors.AccentAmber,
                onClick = { state.camZoomSteps = st },
                modifier = Modifier.weight(1f)
            )
        }
    }
    OutlinedButton(
        onClick = {
            val payload = String.format(
                Locale.US,
                "{\"zoom\":%.2f,\"duration\":%d,\"mode\":\"direct\",\"ease\":\"%s\"}",
                state.camZoomOnly,
                state.camZoomSteps,
                state.camEase
            )
            onAddChartEvent(stepRow.stepTimeMs, "ZoomCamera", payload, "")
        },
        modifier = Modifier.fillMaxWidth()
    ) {
        Text("Aplicar ZoomCamera (${((state.camZoomOnly * 100f).roundToInt() / 100f)}x)", fontSize = 11.sp)
    }

    HorizontalDivider(color = StudioColors.BorderSubtle)
    Text("4. Golpe / Pulso de Zoom (Add Camera Zoom):", color = StudioColors.TextPrimary, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
    Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
        ActionChip(
            label = "Suave +0.015",
            selected = false,
            color = StudioColors.AccentCyan,
            onClick = { onAddChartEvent(stepRow.stepTimeMs, "Add Camera Zoom", "0.015", "0.03") },
            modifier = Modifier.weight(1f)
        )
        ActionChip(
            label = "Medio +0.04",
            selected = false,
            color = StudioColors.AccentAmber,
            onClick = { onAddChartEvent(stepRow.stepTimeMs, "Add Camera Zoom", "0.04", "0.02") },
            modifier = Modifier.weight(1f)
        )
        ActionChip(
            label = "Fuerte +0.08",
            selected = false,
            color = StudioColors.AccentCoral,
            onClick = { onAddChartEvent(stepRow.stepTimeMs, "Add Camera Zoom", "0.08", "0.04") },
            modifier = Modifier.weight(1f)
        )
    }

    HorizontalDivider(color = StudioColors.BorderSubtle)
    Text("5. Brillo / Viñeta Lateral de Cámara (Camera Vignette):", color = StudioColors.AccentCoral, fontSize = 11.sp, fontWeight = FontWeight.Bold)
    Text(
        text = "Degradado atmosférico en los laterales que sigue a la cámara. Puedes hacer que aparezca en este paso, desaparezca, o dure hasta un paso final.",
        color = StudioColors.TextMuted,
        fontSize = 10.sp,
        lineHeight = 13.sp
    )

    Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
        ActionChip(
            label = "Aparecer",
            selected = state.vigMode == "on",
            color = StudioColors.AccentCoral,
            onClick = { state.vigMode = "on" },
            modifier = Modifier.weight(1f)
        )
        if (futureSteps.isNotEmpty()) {
            ActionChip(
                label = "Inicio → Fin",
                selected = state.vigMode == "range",
                color = StudioColors.AccentAmber,
                onClick = { state.vigMode = "range" },
                modifier = Modifier.weight(1f)
            )
        }
        ActionChip(
            label = "Desaparecer",
            selected = state.vigMode == "off",
            color = StudioColors.AccentCyan,
            onClick = { state.vigMode = "off" },
            modifier = Modifier.weight(1f)
        )
    }

    if (state.vigMode != "off") {
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(6.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
            modifier = Modifier.fillMaxWidth()
        ) {
            VIGNETTE_PRESET_COLORS.forEach { (hex, _) ->
                val swatch = parseHexColorOrWhite(hex)
                val isSel = state.vigColorHex.equals(hex, ignoreCase = true)
                Box(
                    modifier = Modifier
                        .size(24.dp)
                        .clip(CircleShape)
                        .background(swatch)
                        .border(
                            width = if (isSel) 2.5.dp else 1.dp,
                            color = if (isSel) StudioColors.AccentCyan else StudioColors.BorderSubtle,
                            shape = CircleShape
                        )
                        .clickable { state.vigColorHex = hex }
                )
            }
        }
        OutlinedTextField(
            value = state.vigColorHex,
            onValueChange = { state.vigColorHex = it },
            label = { Text("Color Hex (ej. #EF4444)", fontSize = 10.sp) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )

        Text(
            text = "Intensidad lateral: ${(state.vigAlpha * 100).roundToInt()}% · Ancho: ${(state.vigWidth * 100).roundToInt()}%",
            color = StudioColors.TextPrimary,
            fontSize = 11.sp
        )
        Slider(
            value = state.vigAlpha,
            onValueChange = { state.vigAlpha = it },
            valueRange = 0.10f..1.0f
        )
        Slider(
            value = state.vigWidth,
            onValueChange = { state.vigWidth = it },
            valueRange = 0.05f..0.50f
        )

        Text(
            text = "Pulso con Beat/Shake: ${(state.vigPulse * 100).roundToInt()}%",
            color = StudioColors.TextPrimary,
            fontSize = 11.sp
        )
        Slider(
            value = state.vigPulse,
            onValueChange = { state.vigPulse = it },
            valueRange = 0.0f..1.0f
        )

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("Dibujar sobre HUD y flechas", color = StudioColors.TextPrimary, fontSize = 11.sp, modifier = Modifier.weight(1f))
            Switch(checked = state.vigOverHud, onCheckedChange = { state.vigOverHud = it })
        }
    }

    Text(
        text = "Transición (fade): ${String.format(Locale.US, "%.2fs", state.vigFadeSec)}",
        color = StudioColors.TextPrimary,
        fontSize = 11.sp
    )
    Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
        listOf(0.0f to "Corte", 0.25f to "0.25s", 0.45f to "0.45s", 1.0f to "1.0s").forEach { (sec, label) ->
            ActionChip(
                label = label,
                selected = abs(state.vigFadeSec - sec) < 0.05f,
                color = StudioColors.AccentCoral,
                onClick = { state.vigFadeSec = sec },
                modifier = Modifier.weight(1f)
            )
        }
    }

    if (state.vigMode == "range" && futureSteps.isNotEmpty()) {
        val selectedVigEndStep = futureSteps.firstOrNull { it.globalStepIndex == state.vigEndStepGlobalIdx }
            ?: futureSteps.last()
        Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.fillMaxWidth()) {
            listOf(4, 8, 16, 32).forEach { offset ->
                val targetIdx = stepRow.globalStepIndex + offset
                val candidate = futureSteps.firstOrNull { it.globalStepIndex == targetIdx }
                    ?: futureSteps.last()
                ActionChip(
                    label = "+${offset}p",
                    selected = selectedVigEndStep.globalStepIndex == candidate.globalStepIndex,
                    color = StudioColors.AccentAmber,
                    onClick = { state.vigEndStepGlobalIdx = candidate.globalStepIndex },
                    modifier = Modifier.weight(1f)
                )
            }
        }
        Box(modifier = Modifier.fillMaxWidth()) {
            val vigEndTxt = String.format(Locale.US, "%.2fs", selectedVigEndStep.stepTimeMs / 1000.0)
            OutlinedButton(
                onClick = { state.vigEndMenuExpanded = true },
                modifier = Modifier.fillMaxWidth(),
                contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp)
            ) {
                Text(
                    text = "Desaparece en: Sec ${selectedVigEndStep.section.index + 1} · Paso #${selectedVigEndStep.stepInSec} ($vigEndTxt)",
                    fontSize = 11.sp,
                    color = StudioColors.AccentAmber,
                    modifier = Modifier.weight(1f)
                )
                Icon(Icons.Rounded.ArrowDropDown, contentDescription = null, tint = StudioColors.AccentAmber)
            }
            DropdownMenu(
                expanded = state.vigEndMenuExpanded,
                onDismissRequest = { state.vigEndMenuExpanded = false },
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
                            state.vigEndStepGlobalIdx = target.globalStepIndex
                            state.vigEndMenuExpanded = false
                        }
                    )
                }
            }
        }
    }

    Button(
        onClick = {
            if (state.vigMode == "off") {
                val v2 = String.format(Locale.US, "action=off;alpha=0.00;dur=%.2f", state.vigFadeSec)
                onAddChartEvent(stepRow.stepTimeMs, "Camera Vignette", "off", v2)
            } else {
                val cleanHex = state.vigColorHex.trim().ifBlank { "#EF4444" }
                val endStepTarget = if (state.vigMode == "range" && futureSteps.isNotEmpty()) {
                    futureSteps.firstOrNull { it.globalStepIndex == state.vigEndStepGlobalIdx }
                        ?: futureSteps.last()
                } else {
                    null
                }
                val kvParts = mutableListOf(
                    "action=on",
                    "color=$cleanHex",
                    String.format(Locale.US, "alpha=%.2f", state.vigAlpha),
                    String.format(Locale.US, "width=%.2f", state.vigWidth),
                    String.format(Locale.US, "pulse=%.2f", state.vigPulse),
                    "hud=${if (state.vigOverHud) 1 else 0}",
                    String.format(Locale.US, "dur=%.2f", state.vigFadeSec)
                )
                if (endStepTarget != null) {
                    kvParts.add(String.format(Locale.US, "endMs=%.2f", endStepTarget.stepTimeMs))
                }
                onAddChartEvent(stepRow.stepTimeMs, "Camera Vignette", cleanHex, kvParts.joinToString(";"))
                if (endStepTarget != null) {
                    val offV2 = String.format(Locale.US, "action=off;alpha=0.00;dur=%.2f", state.vigFadeSec)
                    onAddChartEvent(endStepTarget.stepTimeMs, "Camera Vignette", "off", offV2)
                }
            }
        },
        colors = ButtonDefaults.buttonColors(
            containerColor = if (state.vigMode == "off") StudioColors.AccentCyan else StudioColors.AccentCoral,
            contentColor = if (state.vigMode == "off") Color(0xFF041C24) else Color.White
        ),
        modifier = Modifier.fillMaxWidth()
    ) {
        Icon(Icons.Rounded.Videocam, null, modifier = Modifier.size(16.dp))
        Spacer(Modifier.width(6.dp))
        Text(
            text = when (state.vigMode) {
                "off" -> "Desaparecer Viñeta en Paso #${stepRow.stepInSec}"
                "range" -> "Aplicar Viñeta (Inicio → Fin)"
                else -> "Aparecer Viñeta en Paso #${stepRow.stepInSec}"
            },
            fontWeight = FontWeight.Bold,
            fontSize = 12.sp
        )
    }
}
