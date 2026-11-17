// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/ChartStepCueDialog.kt
// Propósito: Ventana modal interactiva activada al pulsar cualquier número de paso `#` en
// el Editor de Chart. Permite programar al vuelo swaps de visibilidad (mostrar/ocultar
// sprites + reciclaje de RAM), flashes con inicio/fin y capa (escenario o HUD), cambios
// de personaje activo (Jugador/Enemigo), precargas anticipadas y saltar a `stage_events.lua`.

package com.beatforge.studio.ui.popups

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Code
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.FlashOn
import androidx.compose.material.icons.rounded.Person
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.SwapHoriz
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.screens.chart.ChartTimelineRow
import com.beatforge.studio.ui.screens.chart.parseHexColorOrWhite
import com.beatforge.studio.ui.screens.chart.summarizeChartEvent
import com.beatforge.studio.ui.theme.StudioColors
import java.util.Locale
import kotlin.math.roundToInt

private enum class VisibilityAction {
    Keep,
    Show,
    Hide
}

private val FLASH_PRESET_COLORS = listOf(
    "#FFFFFF" to "Blanco",
    "#EF4444" to "Rojo",
    "#06B6D4" to "Cian",
    "#F59E0B" to "Ámbar",
    "#EC4899" to "Magenta",
    "#000000" to "Negro"
)

@Composable
fun ChartStepCueDialog(
    stepRow: ChartTimelineRow.StepRow,
    entities: List<StageEntitySnapshot>,
    onAddChartEvent: (timeMs: Double, name: String, value1: String, value2: String) -> Unit,
    onRemoveChartEvent: (globalIndex: Int) -> Unit,
    onMovePlayheadHere: (Double) -> Unit,
    onOpenStageEventsScript: () -> Unit,
    onDismiss: () -> Unit
) {
    var activeTab by remember { mutableIntStateOf(0) } // 0=Escena/Visibilidad, 1=Flash, 2=Personajes/Precarga

    // Tab 0: Visibilidad & Swap
    val visibilityChoices = remember(stepRow.globalStepIndex) {
        mutableStateMapOf<String, VisibilityAction>()
    }
    var disposeOldOnSwap by remember { mutableStateOf(true) }

    // Tab 1: Flash (inicio -> fin en pasos)
    var flashColorHex by remember { mutableStateOf("#FFFFFF") }
    var flashDurationSteps by remember { mutableIntStateOf(4) }
    var flashAlpha by remember { mutableFloatStateOf(0.85f) }
    var flashOverHud by remember { mutableStateOf(false) }

    // Tab 2: Personajes & Precarga
    var selectedPlayerId by remember { mutableStateOf("") }
    var selectedOpponentId by remember { mutableStateOf("") }
    var disposeOldCharacter by remember { mutableStateOf(true) }
    val manualPreloadIds = remember(stepRow.globalStepIndex) {
        mutableStateMapOf<String, Boolean>()
    }

    val secNum = stepRow.section.index + 1
    val secTimeText = String.format(Locale.US, "%.2fs", stepRow.stepTimeMs / 1000.0)

    Dialog(onDismissRequest = onDismiss) {
        Surface(
            shape = RoundedCornerShape(14.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.65f)),
            modifier = Modifier
                .fillMaxWidth()
                .height(460.dp)
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
                    Column {
                        Text(
                            text = "Eventos en Paso #${stepRow.stepInSec} (Sec. $secNum)",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 15.sp
                        )
                        Text(
                            text = "Marca temporal: $secTimeText (${stepRow.stepTimeMs.roundToInt()} ms)",
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp
                        )
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                        OutlinedButton(
                            onClick = {
                                onMovePlayheadHere(stepRow.stepTimeMs)
                                onDismiss()
                            },
                            contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                            modifier = Modifier.height(30.dp)
                        ) {
                            Icon(Icons.Rounded.PlayArrow, null, tint = StudioColors.AccentEmerald, modifier = Modifier.size(14.dp))
                            Spacer(Modifier.width(3.dp))
                            Text("Hilo aquí", fontSize = 10.sp)
                        }
                        OutlinedButton(
                            onClick = {
                                onDismiss()
                                onOpenStageEventsScript()
                            },
                            contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                            modifier = Modifier.height(30.dp)
                        ) {
                            Icon(Icons.Rounded.Code, null, tint = StudioColors.AccentPurple, modifier = Modifier.size(14.dp))
                            Spacer(Modifier.width(4.dp))
                            Text("stage_events.lua", fontSize = 10.sp)
                        }
                    }
                }

                // Selector de pestañas modular
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    CueTabButton(
                        label = "Visibilidad / Swap",
                        selected = activeTab == 0,
                        accent = StudioColors.AccentEmerald,
                        onClick = { activeTab = 0 },
                        modifier = Modifier.weight(1f)
                    )
                    CueTabButton(
                        label = "Flash",
                        selected = activeTab == 1,
                        accent = StudioColors.AccentAmber,
                        onClick = { activeTab = 1 },
                        modifier = Modifier.weight(0.7f)
                    )
                    CueTabButton(
                        label = "Personajes",
                        selected = activeTab == 2,
                        accent = StudioColors.AccentPurple,
                        onClick = { activeTab = 2 },
                        modifier = Modifier.weight(0.85f)
                    )
                }

                HorizontalDivider(color = StudioColors.BorderSubtle)

                when (activeTab) {
                    0 -> {
                        Text(
                            text = "Muestra sprites ocultos al instante u oculta los actuales (escenario, fondos o decoración). Los ocultos al inicio NO cargan en RAM hasta precargarse 4s antes.",
                            color = StudioColors.TextMuted,
                            fontSize = 10.sp,
                            lineHeight = 13.sp
                        )
                        entities.forEach { ent ->
                            val choice = visibilityChoices[ent.id] ?: VisibilityAction.Keep
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .background(StudioColors.BgCard, RoundedCornerShape(8.dp))
                                    .padding(horizontal = 8.dp, vertical = 6.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.SpaceBetween
                            ) {
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(
                                        text = ent.displayName,
                                        color = StudioColors.TextPrimary,
                                        fontSize = 12.sp,
                                        fontWeight = FontWeight.SemiBold,
                                        maxLines = 1
                                    )
                                    Text(
                                        text = "${ent.roleLabel} · inicio: ${if (ent.visible) "Visible" else "Oculto (en WebP)"}",
                                        color = StudioColors.TextMuted,
                                        fontSize = 10.sp
                                    )
                                }
                                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                                    ActionChip(
                                        label = "Mostrar",
                                        selected = choice == VisibilityAction.Show,
                                        color = StudioColors.AccentEmerald,
                                        onClick = {
                                            visibilityChoices[ent.id] = if (choice == VisibilityAction.Show) {
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
                                            visibilityChoices[ent.id] = if (choice == VisibilityAction.Hide) {
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
                            Switch(checked = disposeOldOnSwap, onCheckedChange = { disposeOldOnSwap = it })
                        }

                        val showList = visibilityChoices.filterValues { it == VisibilityAction.Show }.keys.toList()
                        val hideList = visibilityChoices.filterValues { it == VisibilityAction.Hide }.keys.toList()
                        Button(
                            enabled = showList.isNotEmpty() || hideList.isNotEmpty(),
                            onClick = {
                                val parts = mutableListOf<String>()
                                if (showList.isNotEmpty()) parts.add("show=${showList.joinToString(",")}")
                                if (hideList.isNotEmpty()) parts.add("hide=${hideList.joinToString(",")}")
                                parts.add("dispose=${if (disposeOldOnSwap) 1 else 0}")
                                onAddChartEvent(stepRow.stepTimeMs, "Stage Swap", parts.joinToString(";"), "")
                                visibilityChoices.clear()
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

                    1 -> {
                        Text("Color del Flash:", color = StudioColors.TextPrimary, fontSize = 11.sp, fontWeight = FontWeight.SemiBold)
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                            FLASH_PRESET_COLORS.forEach { (hex, label) ->
                                val swatchColor = parseHexColorOrWhite(hex)
                                val isSel = flashColorHex.equals(hex, ignoreCase = true)
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
                                        .clickable { flashColorHex = hex }
                                )
                            }
                            OutlinedTextField(
                                value = flashColorHex,
                                onValueChange = { flashColorHex = it },
                                label = { Text("Hex", fontSize = 10.sp) },
                                singleLine = true,
                                modifier = Modifier.weight(1f)
                            )
                        }

                        val endStepDisplay = stepRow.stepInSec + flashDurationSteps
                        val durSec = (flashDurationSteps * stepRow.stepMs) / 1000.0
                        val endMs = stepRow.stepTimeMs + (flashDurationSteps * stepRow.stepMs)
                        Text(
                            text = "Duración del Flash (Inicio Paso #${stepRow.stepInSec} → Fin Paso #$endStepDisplay · ${String.format(Locale.US, "%.2fs", durSec)})",
                            color = StudioColors.AccentAmber,
                            fontSize = 11.sp,
                            fontWeight = FontWeight.SemiBold
                        )
                        Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
                            listOf(2, 4, 8, 16, 32).forEach { steps ->
                                val isSel = flashDurationSteps == steps
                                ActionChip(
                                    label = "+${steps}p",
                                    selected = isSel,
                                    color = StudioColors.AccentAmber,
                                    onClick = { flashDurationSteps = steps },
                                    modifier = Modifier.weight(1f)
                                )
                            }
                        }

                        Text(
                            text = "Opacidad inicial: ${(flashAlpha * 100).roundToInt()}%",
                            color = StudioColors.TextPrimary,
                            fontSize = 11.sp
                        )
                        Slider(
                            value = flashAlpha,
                            onValueChange = { flashAlpha = it },
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
                            Switch(checked = flashOverHud, onCheckedChange = { flashOverHud = it })
                        }

                        Button(
                            onClick = {
                                val v2 = String.format(
                                    Locale.US,
                                    "dur=%.3f;endMs=%.2f;alpha=%.2f;hud=%d",
                                    durSec,
                                    endMs,
                                    flashAlpha,
                                    if (flashOverHud) 1 else 0
                                )
                                onAddChartEvent(
                                    stepRow.stepTimeMs,
                                    "Flash Effect",
                                    flashColorHex.ifBlank { "#FFFFFF" },
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

                    2 -> {
                        Text(
                            text = "Promueve cualquier personaje o sprite de escenario a Jugador u Oponente activo en este paso (cambia animaciones, cámara e íconos):",
                            color = StudioColors.TextMuted,
                            fontSize = 10.sp,
                            lineHeight = 13.sp
                        )

                        Text("Nuevo Jugador Activo (player):", color = StudioColors.AccentEmerald, fontSize = 11.sp, fontWeight = FontWeight.Bold)
                        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            entities.forEach { ent ->
                                val isSel = selectedPlayerId == ent.id
                                Row(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .clip(RoundedCornerShape(7.dp))
                                        .background(if (isSel) StudioColors.AccentEmerald.copy(alpha = 0.16f) else StudioColors.BgCard)
                                        .border(1.dp, if (isSel) StudioColors.AccentEmerald else StudioColors.BorderSubtle, RoundedCornerShape(7.dp))
                                        .clickable { selectedPlayerId = if (isSel) "" else ent.id }
                                        .padding(horizontal = 8.dp, vertical = 5.dp),
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                    verticalAlignment = Alignment.CenterVertically
                                ) {
                                    Text("${ent.displayName} (${ent.roleLabel})", color = StudioColors.TextPrimary, fontSize = 11.sp)
                                    Text(if (isSel) "Activo" else "Elegir", color = if (isSel) StudioColors.AccentEmerald else StudioColors.TextMuted, fontSize = 10.sp, fontWeight = FontWeight.Bold)
                                }
                            }
                        }

                        Text("Nuevo Enemigo Activo (opp):", color = StudioColors.AccentCoral, fontSize = 11.sp, fontWeight = FontWeight.Bold)
                        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            entities.forEach { ent ->
                                val isSel = selectedOpponentId == ent.id
                                Row(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .clip(RoundedCornerShape(7.dp))
                                        .background(if (isSel) StudioColors.AccentCoral.copy(alpha = 0.16f) else StudioColors.BgCard)
                                        .border(1.dp, if (isSel) StudioColors.AccentCoral else StudioColors.BorderSubtle, RoundedCornerShape(7.dp))
                                        .clickable { selectedOpponentId = if (isSel) "" else ent.id }
                                        .padding(horizontal = 8.dp, vertical = 5.dp),
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                    verticalAlignment = Alignment.CenterVertically
                                ) {
                                    Text("${ent.displayName} (${ent.roleLabel})", color = StudioColors.TextPrimary, fontSize = 11.sp)
                                    Text(if (isSel) "Activo" else "Elegir", color = if (isSel) StudioColors.AccentCoral else StudioColors.TextMuted, fontSize = 10.sp, fontWeight = FontWeight.Bold)
                                }
                            }
                        }

                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Text("Ocultar y reciclar personaje anterior (RAM)", color = StudioColors.TextPrimary, fontSize = 11.sp, modifier = Modifier.weight(1f))
                            Switch(checked = disposeOldCharacter, onCheckedChange = { disposeOldCharacter = it })
                        }

                        Button(
                            enabled = selectedPlayerId.isNotBlank() || selectedOpponentId.isNotBlank(),
                            onClick = {
                                val parts = mutableListOf<String>()
                                if (selectedPlayerId.isNotBlank()) parts.add("player=$selectedPlayerId")
                                if (selectedOpponentId.isNotBlank()) parts.add("opp=$selectedOpponentId")
                                parts.add("dispose=${if (disposeOldCharacter) 1 else 0}")
                                onAddChartEvent(stepRow.stepTimeMs, "Change Character", parts.joinToString(";"), "")
                                selectedPlayerId = ""
                                selectedOpponentId = ""
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
                            entities.filter { !it.visible }.take(4).forEach { ent ->
                                val checked = manualPreloadIds[ent.id] == true
                                ActionChip(
                                    label = ent.displayName.take(10),
                                    selected = checked,
                                    color = StudioColors.AccentCyan,
                                    onClick = { manualPreloadIds[ent.id] = !checked },
                                    modifier = Modifier.weight(1f)
                                )
                            }
                        }
                        val chosenPreloads = manualPreloadIds.filterValues { it }.keys.toList()
                        if (chosenPreloads.isNotEmpty()) {
                            OutlinedButton(
                                onClick = {
                                    onAddChartEvent(stepRow.stepTimeMs, "Preload Sprite", chosenPreloads.joinToString(","), "")
                                    manualPreloadIds.clear()
                                },
                                modifier = Modifier.fillMaxWidth()
                            ) {
                                Text("Añadir Precarga IO en Paso #${stepRow.stepInSec}", fontSize = 11.sp)
                            }
                        }
                    }
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

@Composable
private fun CueTabButton(
    label: String,
    selected: Boolean,
    accent: Color,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    Box(
        modifier = modifier
            .height(32.dp)
            .clip(RoundedCornerShape(8.dp))
            .background(if (selected) accent.copy(alpha = 0.2f) else StudioColors.BgCard)
            .border(1.dp, if (selected) accent else StudioColors.BorderSubtle, RoundedCornerShape(8.dp))
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = label,
            color = if (selected) accent else StudioColors.TextMuted,
            fontSize = 10.sp,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
            maxLines = 1
        )
    }
}

@Composable
private fun ActionChip(
    label: String,
    selected: Boolean,
    color: Color,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(6.dp))
            .background(if (selected) color.copy(alpha = 0.24f) else StudioColors.BgPanel)
            .border(1.dp, if (selected) color else StudioColors.BorderSubtle, RoundedCornerShape(6.dp))
            .clickable(onClick = onClick)
            .padding(horizontal = 8.dp, vertical = 4.dp),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = label,
            color = if (selected) color else StudioColors.TextMuted,
            fontSize = 10.sp,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
            maxLines = 1
        )
    }
}
