// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/chart_cue/CueFlashTab.kt
// Propósito: Pestaña 1 de `ChartStepCueDialog` para configurar eventos `Flash Effect`
// (color RGB hexadecimal, duración en pasos, opacidad inicial y capa Escena vs HUD).

package com.beatforge.studio.ui.popups.chart_cue

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.FlashOn
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
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
import com.beatforge.studio.ui.screens.chart.ChartTimelineRow
import com.beatforge.studio.ui.screens.chart.parseHexColorOrWhite
import com.beatforge.studio.ui.theme.StudioColors
import java.util.Locale
import kotlin.math.roundToInt

@Composable
internal fun CueFlashTab(
    stepRow: ChartTimelineRow.StepRow,
    state: ChartCueDialogState,
    onAddChartEvent: (timeMs: Double, name: String, value1: String, value2: String) -> Unit
) {
    Text("Color del Flash:", color = StudioColors.TextPrimary, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
    Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
        FLASH_PRESET_COLORS.forEach { (hex, _) ->
            val swatchColor = parseHexColorOrWhite(hex)
            val isSel = state.flashColorHex.equals(hex, ignoreCase = true)
            Box(
                modifier = Modifier
                    .size(28.dp)
                    .clip(CircleShape)
                    .background(swatchColor)
                    .border(
                        width = if (isSel) 2.5.dp else 1.dp,
                        color = if (isSel) StudioColors.AccentCyan else StudioColors.BorderSubtle,
                        shape = CircleShape
                    )
                    .clickable { state.flashColorHex = hex }
            )
        }
        OutlinedTextField(
            value = state.flashColorHex,
            onValueChange = { state.flashColorHex = it },
            label = { Text("Hex", fontSize = 10.sp) },
            singleLine = true,
            modifier = Modifier.weight(1f)
        )
    }

    val endStepDisplay = stepRow.stepInSec + state.flashDurationSteps
    val durSec = (state.flashDurationSteps * stepRow.stepMs) / 1000.0
    val endMs = stepRow.stepTimeMs + (state.flashDurationSteps * stepRow.stepMs)
    Text(
        text = "Duración del Flash (Inicio Paso #${stepRow.stepInSec} → Fin Paso #$endStepDisplay · ${String.format(Locale.US, "%.2fs", durSec)})",
        color = StudioColors.AccentAmber,
        fontSize = 11.sp,
        fontWeight = FontWeight.SemiBold
    )
    Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
        listOf(2, 4, 8, 16, 32).forEach { steps ->
            val isSel = state.flashDurationSteps == steps
            ActionChip(
                label = "+${steps}p",
                selected = isSel,
                color = StudioColors.AccentAmber,
                onClick = { state.flashDurationSteps = steps },
                modifier = Modifier.weight(1f)
            )
        }
    }

    Text(
        text = "Opacidad inicial: ${(state.flashAlpha * 100).roundToInt()}%",
        color = StudioColors.TextPrimary,
        fontSize = 11.sp
    )
    Slider(
        value = state.flashAlpha,
        onValueChange = { state.flashAlpha = it },
        valueRange = 0.2f..1.0f
    )

    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Text("Flash sobre HUD y flechas", color = StudioColors.TextPrimary, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
            Text("Desactivado = flash detrás de las notas (solo escenario).", color = StudioColors.TextMuted, fontSize = 9.sp)
        }
        Switch(checked = state.flashOverHud, onCheckedChange = { state.flashOverHud = it })
    }

    Button(
        onClick = {
            val v2 = String.format(
                Locale.US,
                "dur=%.3f;endMs=%.2f;alpha=%.2f;hud=%d",
                durSec,
                endMs,
                state.flashAlpha,
                if (state.flashOverHud) 1 else 0
            )
            onAddChartEvent(
                stepRow.stepTimeMs,
                "Flash Effect",
                state.flashColorHex.ifBlank { "#FFFFFF" },
                v2
            )
        },
        colors = ButtonDefaults.buttonColors(
            containerColor = StudioColors.AccentAmber,
            contentColor = Color(0xFF1F1200)
        ),
        modifier = Modifier.fillMaxWidth()
    ) {
        Icon(Icons.Rounded.FlashOn, null, modifier = Modifier.size(16.dp))
        Spacer(Modifier.width(6.dp))
        Text("Añadir Flash en Paso #${stepRow.stepInSec}", fontWeight = FontWeight.Bold, fontSize = 12.sp)
    }
}
