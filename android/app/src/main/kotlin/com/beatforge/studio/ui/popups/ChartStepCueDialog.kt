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
import androidx.compose.material.icons.rounded.Videocam
import androidx.compose.material.icons.rounded.VisibilityOff
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
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.beatforge.studio.ChartEventSnapshot
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.screens.chart.ChartTimelineRow
import com.beatforge.studio.ui.screens.chart.parseHexColorOrWhite
import com.beatforge.studio.ui.screens.chart.resolveChartStageStateAtStep
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

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun ChartStepCueDialog(
    stepRow: ChartTimelineRow.StepRow,
    entities: List<StageEntitySnapshot>,
    chartEvents: List<ChartEventSnapshot>,
    stageCamera: StageCameraSnapshot,
    onAddChartEvent: (timeMs: Double, name: String, value1: String, value2: String) -> Unit,
    onRemoveChartEvent: (globalIndex: Int) -> Unit,
    onMovePlayheadHere: (Double) -> Unit,
    onOpenStageEventsScript: () -> Unit,
    onOpenUiBlackoutScript: () -> Unit,
    onDismiss: () -> Unit
) {
    var activeTab by remember { mutableIntStateOf(0) } // 0=Escena/Visibilidad, 1=Flash, 2=Personajes/Precarga, 3=UI/Negro, 4=Cámara

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

    // Tab 3: UI / Pantalla en Negro
    var uiPresetAction by remember { mutableStateOf("intro") } // "intro", "hide", "arrows_hide", "show", "toggle"
    var uiBlackAlpha by remember { mutableFloatStateOf(1.0f) }
    var uiHideHud by remember { mutableStateOf(true) }
    var uiHideArrows by remember { mutableStateOf(true) }
    var uiRevealOnStart by remember { mutableStateOf(true) }
    var uiFadeSec by remember { mutableFloatStateOf(0.65f) }
    var uiIntroDelayMs by remember { mutableFloatStateOf(0.0f) }

    // Tab 4: Eventos de Cámara en chart.json
    val cameraSlots = stageCamera.resolvedCameras()
    var camTargetSlot by remember { mutableIntStateOf(stageCamera.activeCameraIndex.coerceIn(0, (cameraSlots.size - 1).coerceAtLeast(0))) }
    var camOverrideZoom by remember { mutableStateOf(false) }
    var camCustomZoom by remember { mutableFloatStateOf(cameraSlots.getOrNull(camTargetSlot)?.zoom ?: stageCamera.baseZoom) }
    var camTransDurSec by remember { mutableFloatStateOf(0.0f) }
    var camEase by remember { mutableStateOf("sineOut") }
    var camFollowPosX by remember { mutableStateOf(stageCamera.baseX.roundToInt().toString()) }
    var camFollowPosY by remember { mutableStateOf(stageCamera.baseY.roundToInt().toString()) }
    var camZoomOnly by remember { mutableFloatStateOf(1.20f) }
    var camZoomSteps by remember { mutableIntStateOf(4) }

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
                    horizontalArrangement = Arrangement.spacedBy(5.dp)
                ) {
                    CueTabButton(
                        label = "Visibilidad",
                        selected = activeTab == 0,
                        accent = StudioColors.AccentEmerald,
                        onClick = { activeTab = 0 },
                        modifier = Modifier.weight(0.95f)
                    )
                    CueTabButton(
                        label = "Flash",
                        selected = activeTab == 1,
                        accent = StudioColors.AccentAmber,
                        onClick = { activeTab = 1 },
                        modifier = Modifier.weight(0.65f)
                    )
                    CueTabButton(
                        label = "Personajes",
                        selected = activeTab == 2,
                        accent = StudioColors.AccentPurple,
                        onClick = { activeTab = 2 },
                        modifier = Modifier.weight(0.9f)
                    )
                    CueTabButton(
                        label = "UI/Negro",
                        selected = activeTab == 3,
                        accent = StudioColors.AccentCoral,
                        onClick = { activeTab = 3 },
                        modifier = Modifier.weight(0.8f)
                    )
                    CueTabButton(
                        label = "Cámara",
                        selected = activeTab == 4,
                        accent = StudioColors.AccentCyan,
                        onClick = { activeTab = 4 },
                        modifier = Modifier.weight(0.75f)
                    )
                }

                HorizontalDivider(color = StudioColors.BorderSubtle)

                when (activeTab) {
                    0 -> {
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
                            val choice = visibilityChoices[ent.id] ?: VisibilityAction.Keep
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
                                val isSel = selectedPlayerId == ent.id
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
                                        .clickable { selectedPlayerId = if (isSel) "" else ent.id }
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
                                val isSel = selectedOpponentId == ent.id
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
                                        .clickable { selectedOpponentId = if (isSel) "" else ent.id }
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
                            entities.filter { !stageStateAtStep.visibleIds.contains(it.id) }.take(4).forEach { ent ->
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

                    3 -> {
                        Text(
                            text = "Controla pantalla negra y visibilidad del HUD y flechas sin depender de programar desde cero. La intro negra oculta todo al arrancar dejando visible '3, 2, 1, ¡YA!' y aparece suave después.",
                            color = StudioColors.TextMuted,
                            fontSize = 10.sp,
                            lineHeight = 13.sp
                        )

                        Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
                            ActionChip(
                                label = "Intro Negra",
                                selected = uiPresetAction == "intro",
                                color = StudioColors.AccentCoral,
                                onClick = {
                                    uiPresetAction = "intro"
                                    uiBlackAlpha = 1.0f
                                    uiHideHud = true
                                    uiHideArrows = true
                                    uiRevealOnStart = true
                                },
                                modifier = Modifier.weight(1f)
                            )
                            ActionChip(
                                label = "Ocultar Todo",
                                selected = uiPresetAction == "hide",
                                color = StudioColors.AccentAmber,
                                onClick = {
                                    uiPresetAction = "hide"
                                    uiBlackAlpha = 1.0f
                                    uiHideHud = true
                                    uiHideArrows = true
                                },
                                modifier = Modifier.weight(1f)
                            )
                            ActionChip(
                                label = "Mostrar Todo",
                                selected = uiPresetAction == "show",
                                color = StudioColors.AccentEmerald,
                                onClick = {
                                    uiPresetAction = "show"
                                    uiBlackAlpha = 0.0f
                                    uiHideHud = false
                                    uiHideArrows = false
                                },
                                modifier = Modifier.weight(1f)
                            )
                        }

                        Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
                            ActionChip(
                                label = "Solo Flechas Ocultas",
                                selected = uiPresetAction == "arrows_hide",
                                color = StudioColors.AccentPurple,
                                onClick = {
                                    uiPresetAction = "arrows_hide"
                                    uiBlackAlpha = 0.0f
                                    uiHideHud = false
                                    uiHideArrows = true
                                },
                                modifier = Modifier.weight(1.2f)
                            )
                            ActionChip(
                                label = "Alternar UI",
                                selected = uiPresetAction == "toggle",
                                color = StudioColors.AccentCyan,
                                onClick = { uiPresetAction = "toggle" },
                                modifier = Modifier.weight(0.9f)
                            )
                        }

                        if (uiPresetAction != "show") {
                            Text(
                                text = "Oscuridad de pantalla negra: ${(uiBlackAlpha * 100).roundToInt()}%",
                                color = StudioColors.TextPrimary,
                                fontSize = 11.sp
                            )
                            Slider(
                                value = uiBlackAlpha,
                                onValueChange = { uiBlackAlpha = it },
                                valueRange = 0.0f..1.0f
                            )

                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text("Ocultar barra de vida y puntaje (HUD)", color = StudioColors.TextPrimary, fontSize = 11.sp, modifier = Modifier.weight(1f))
                                Switch(checked = uiHideHud, onCheckedChange = { uiHideHud = it })
                            }
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text("Ocultar flechas y notas", color = StudioColors.TextPrimary, fontSize = 11.sp, modifier = Modifier.weight(1f))
                                Switch(checked = uiHideArrows, onCheckedChange = { uiHideArrows = it })
                            }
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text("Revelar tras '3, 2, 1, ¡YA!' (onStart)", color = StudioColors.TextPrimary, fontSize = 11.sp, modifier = Modifier.weight(1f))
                                Switch(checked = uiRevealOnStart, onCheckedChange = { uiRevealOnStart = it })
                            }
                        }

                        Text(
                            text = "Duración del desvanecimiento (fade): ${String.format(Locale.US, "%.2fs", uiFadeSec)}",
                            color = StudioColors.TextPrimary,
                            fontSize = 11.sp
                        )
                        Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
                            listOf(0.0f to "Corte", 0.35f to "0.35s", 0.65f to "0.65s", 1.2f to "1.2s", 2.5f to "2.5s").forEach { (sec, label) ->
                                ActionChip(
                                    label = label,
                                    selected = kotlin.math.abs(uiFadeSec - sec) < 0.05f,
                                    color = StudioColors.AccentCoral,
                                    onClick = { uiFadeSec = sec },
                                    modifier = Modifier.weight(1f)
                                )
                            }
                        }

                        if (uiPresetAction == "intro" || (uiPresetAction != "show" && uiRevealOnStart)) {
                            Text(
                                text = "Esperar tras '¡YA!' antes de aparecer:",
                                color = StudioColors.AccentAmber,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.SemiBold
                            )
                            Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
                                listOf(0.0f to "Al ¡YA!", 500.0f to "+0.5s", 1000.0f to "+1.0s", 2000.0f to "+2.0s").forEach { (ms, label) ->
                                    ActionChip(
                                        label = label,
                                        selected = kotlin.math.abs(uiIntroDelayMs - ms) < 10f,
                                        color = StudioColors.AccentAmber,
                                        onClick = { uiIntroDelayMs = ms },
                                        modifier = Modifier.weight(1f)
                                    )
                                }
                            }
                        }

                        Button(
                            onClick = {
                                val targetBlack = if (uiPresetAction == "show") 0.0f else uiBlackAlpha
                                val targetHud = when {
                                    uiPresetAction == "show" -> 1.0f
                                    uiHideHud -> 0.0f
                                    else -> 1.0f
                                }
                                val targetArrows = when {
                                    uiPresetAction == "show" -> 1.0f
                                    uiHideArrows -> 0.0f
                                    else -> 1.0f
                                }
                                val effectiveAction = if (uiPresetAction == "arrows_hide") "hide" else uiPresetAction
                                val effectiveEventName = if (uiPresetAction == "arrows_hide") "Hide Arrows" else "UI Blackout"
                                val revealFlag = if (uiPresetAction == "intro") true else uiRevealOnStart
                                val v2 = String.format(
                                    Locale.US,
                                    "black=%.2f;hud=%.2f;arrows=%.2f;dur=%.2f;revealMs=%.0f;onStart=%d",
                                    targetBlack,
                                    targetHud,
                                    targetArrows,
                                    uiFadeSec,
                                    if (revealFlag) uiIntroDelayMs else 0.0f,
                                    if (revealFlag) 1 else 0
                                )
                                onAddChartEvent(stepRow.stepTimeMs, effectiveEventName, effectiveAction, v2)
                            },
                            colors = ButtonDefaults.buttonColors(
                                containerColor = StudioColors.AccentCoral,
                                contentColor = Color.White
                            ),
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            Icon(Icons.Rounded.VisibilityOff, null, modifier = Modifier.size(16.dp))
                            Spacer(Modifier.width(6.dp))
                            Text(
                                text = when (uiPresetAction) {
                                    "intro" -> "Guardar Intro Pantalla en Negro"
                                    "arrows_hide" -> "Ocultar Flechas en Paso #${stepRow.stepInSec}"
                                    else -> "Aplicar Evento UI / Negro en Paso #${stepRow.stepInSec}"
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

                    4 -> {
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
                                val isSel = camTargetSlot == slot.index
                                ActionChip(
                                    label = "Cam ${slot.index}",
                                    selected = isSel,
                                    color = StudioColors.AccentCyan,
                                    onClick = {
                                        camTargetSlot = slot.index
                                        camCustomZoom = slot.zoom
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
                                    val isSel = camTargetSlot == slot.index
                                    ActionChip(
                                        label = "Cam ${slot.index}",
                                        selected = isSel,
                                        color = StudioColors.AccentCyan,
                                        onClick = {
                                            camTargetSlot = slot.index
                                            camCustomZoom = slot.zoom
                                        },
                                        modifier = Modifier.weight(1f)
                                    )
                                }
                            }
                        }
                        val chosenSlotInfo = cameraSlots.firstOrNull { it.index == camTargetSlot }
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
                            Switch(checked = camOverrideZoom, onCheckedChange = { camOverrideZoom = it })
                        }
                        if (camOverrideZoom) {
                            Text(
                                text = "Zoom FOV: ${((camCustomZoom * 100f).roundToInt() / 100f)}x",
                                color = StudioColors.AccentCyan,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.SemiBold
                            )
                            Slider(
                                value = camCustomZoom.coerceIn(0.35f, 2.5f),
                                onValueChange = { camCustomZoom = it },
                                valueRange = 0.35f..2.5f
                            )
                        }

                        Text("Duración e interpolación (paneo):", color = StudioColors.TextPrimary, fontSize = 11.sp)
                        Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
                            listOf(0.0f to "Corte/Suave", 0.25f to "0.25s", 0.5f to "0.50s", 1.0f to "1.0s").forEach { (sec, label) ->
                                ActionChip(
                                    label = label,
                                    selected = kotlin.math.abs(camTransDurSec - sec) < 0.05f,
                                    color = StudioColors.AccentCyan,
                                    onClick = { camTransDurSec = sec },
                                    modifier = Modifier.weight(1f)
                                )
                            }
                        }
                        Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
                            listOf("sineOut" to "SineOut", "quadInOut" to "QuadInOut", "linear" to "Lineal").forEach { (key, label) ->
                                ActionChip(
                                    label = label,
                                    selected = camEase == key,
                                    color = StudioColors.AccentIndigo,
                                    onClick = { camEase = key },
                                    modifier = Modifier.weight(1f)
                                )
                            }
                        }

                        Button(
                            onClick = {
                                val parts = mutableListOf("cam=$camTargetSlot")
                                if (camOverrideZoom) {
                                    parts.add(String.format(Locale.US, "zoom=%.2f", camCustomZoom))
                                }
                                if (camTransDurSec > 0.01f) {
                                    parts.add(String.format(Locale.US, "dur=%.2f", camTransDurSec))
                                    parts.add("ease=$camEase")
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
                            Text("Cambiar a Cam $camTargetSlot en Paso #${stepRow.stepInSec}", fontWeight = FontWeight.Bold, fontSize = 12.sp)
                        }

                        HorizontalDivider(color = StudioColors.BorderSubtle)
                        Text("2. Posición Fija en Escenario (Camera Follow Pos):", color = StudioColors.AccentEmerald, fontSize = 11.sp, fontWeight = FontWeight.Bold)
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.fillMaxWidth()) {
                            OutlinedTextField(
                                value = camFollowPosX,
                                onValueChange = { camFollowPosX = it },
                                label = { Text("X (ej. 640)", fontSize = 10.sp) },
                                singleLine = true,
                                modifier = Modifier.weight(1f)
                            )
                            OutlinedTextField(
                                value = camFollowPosY,
                                onValueChange = { camFollowPosY = it },
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
                                    camFollowPosX = stageCamera.baseX.roundToInt().toString()
                                    camFollowPosY = stageCamera.baseY.roundToInt().toString()
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
                                    camFollowPosX = (opp?.x ?: 300f).roundToInt().toString()
                                    camFollowPosY = ((opp?.y ?: 415f) - 24f).roundToInt().toString()
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
                                    camFollowPosX = (pl?.x ?: 980f).roundToInt().toString()
                                    camFollowPosY = ((pl?.y ?: 420f) - 24f).roundToInt().toString()
                                },
                                modifier = Modifier.weight(1f)
                            )
                        }
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.fillMaxWidth()) {
                            Button(
                                onClick = {
                                    val xVal = camFollowPosX.trim().toFloatOrNull() ?: stageCamera.baseX
                                    val yVal = camFollowPosY.trim().toFloatOrNull() ?: stageCamera.baseY
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
                            text = "Zoom destino: ${((camZoomOnly * 100f).roundToInt() / 100f)}x · Duración ${camZoomSteps}p",
                            color = StudioColors.TextPrimary,
                            fontSize = 11.sp
                        )
                        Slider(
                            value = camZoomOnly.coerceIn(0.4f, 2.2f),
                            onValueChange = { camZoomOnly = it },
                            valueRange = 0.4f..2.2f
                        )
                        Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
                            listOf(0, 4, 8, 16).forEach { st ->
                                ActionChip(
                                    label = if (st == 0) "Corte" else "${st}p",
                                    selected = camZoomSteps == st,
                                    color = StudioColors.AccentAmber,
                                    onClick = { camZoomSteps = st },
                                    modifier = Modifier.weight(1f)
                                )
                            }
                        }
                        OutlinedButton(
                            onClick = {
                                val payload = String.format(
                                    Locale.US,
                                    "{\"zoom\":%.2f,\"duration\":%d,\"mode\":\"direct\",\"ease\":\"%s\"}",
                                    camZoomOnly,
                                    camZoomSteps,
                                    camEase
                                )
                                onAddChartEvent(stepRow.stepTimeMs, "ZoomCamera", payload, "")
                            },
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            Text("Aplicar ZoomCamera (${((camZoomOnly * 100f).roundToInt() / 100f)}x)", fontSize = 11.sp)
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

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun StageStateSummaryBanner(
    stepInSec: Int,
    activePlayerName: String,
    activeOpponentName: String,
    visibleCount: Int,
    totalCount: Int
) {
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .background(StudioColors.BgDark.copy(alpha = 0.72f))
            .border(1.dp, StudioColors.BorderSubtle, RoundedCornerShape(8.dp))
            .padding(horizontal = 8.dp, vertical = 6.dp),
        verticalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        Text(
            text = "Estado de escena en Paso #$stepInSec:",
            color = StudioColors.TextPrimary,
            fontSize = 10.sp,
            fontWeight = FontWeight.Bold
        )
        FlowRow(
            horizontalArrangement = Arrangement.spacedBy(5.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            StageStatusBadge(label = "Jugador: $activePlayerName", color = StudioColors.AccentEmerald)
            StageStatusBadge(label = "Enemigo: $activeOpponentName", color = StudioColors.AccentCoral)
            StageStatusBadge(label = "Visibles: $visibleCount/$totalCount", color = StudioColors.AccentCyan)
        }
    }
}

@Composable
private fun StageStatusBadge(
    label: String,
    color: Color
) {
    Box(
        modifier = Modifier
            .clip(RoundedCornerShape(5.dp))
            .background(color.copy(alpha = 0.16f))
            .border(1.dp, color.copy(alpha = 0.55f), RoundedCornerShape(5.dp))
            .padding(horizontal = 5.dp, vertical = 2.dp)
    ) {
        Text(
            text = label,
            color = color,
            fontSize = 9.sp,
            fontWeight = FontWeight.Bold,
            maxLines = 1
        )
    }
}

