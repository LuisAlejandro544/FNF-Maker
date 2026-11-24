// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/ChartJsonDetailsDialog.kt
// Propósito: Ventana modal emergente para editar los metadatos completos de `chart.json`
// (`song`, `speed`, `player1`, `player2`, `gfVersion`, `stage`, `needsVoices`), ajustar
// `changeBPM` por sección y añadir o eliminar eventos cronológicos del chart.

package com.beatforge.studio.ui.popups

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Delete
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.beatforge.studio.ChartSectionSnapshot
import com.beatforge.studio.ChartSnapshot
import com.beatforge.studio.ui.theme.StudioColors
import java.util.Locale
import kotlin.math.abs
import kotlin.math.roundToInt

@Composable
fun ChartJsonDetailsDialog(
    chart: ChartSnapshot,
    activeSection: ChartSectionSnapshot?,
    onSaveDetails: (
        title: String,
        speed: Double,
        player1: String,
        player2: String,
        gfVersion: String,
        stage: String,
        needsVoices: Boolean
    ) -> Unit,
    onSaveIntroUi: (
        startBlackout: Boolean,
        blackoutAlpha: Float,
        hideHud: Boolean,
        hideArrows: Boolean,
        revealOnSongStart: Boolean,
        revealDelayMs: Double,
        fadeSec: Float
    ) -> Unit,
    onSetSectionBpm: (secIndex: Int, bpm: Double, changeBpm: Boolean) -> Unit,
    onAddEvent: (timeMs: Double, name: String, value1: String, value2: String) -> Unit,
    onRemoveEvent: (index: Int) -> Unit,
    onDismiss: () -> Unit
) {
    var titleText by remember(chart.title) { mutableStateOf(chart.title) }
    var speedText by remember(chart.speed) { mutableStateOf(chart.speed.toString()) }
    var player1Text by remember(chart.player1) { mutableStateOf(chart.player1) }
    var player2Text by remember(chart.player2) { mutableStateOf(chart.player2) }
    var gfText by remember(chart.gfVersion) { mutableStateOf(chart.gfVersion) }
    var stageText by remember(chart.stage) { mutableStateOf(chart.stage) }
    var needsVoices by remember(chart.needsVoices) { mutableStateOf(chart.needsVoices) }

    val introCfg = chart.introUi
    var introStartBlackout by remember(introCfg) { mutableStateOf(introCfg.startBlackout) }
    var introBlackoutAlpha by remember(introCfg) { mutableFloatStateOf(introCfg.blackoutAlpha.coerceIn(0f, 1f)) }
    var introHideHud by remember(introCfg) { mutableStateOf(introCfg.hideHud) }
    var introHideArrows by remember(introCfg) { mutableStateOf(introCfg.hideArrows) }
    var introRevealOnSongStart by remember(introCfg) { mutableStateOf(introCfg.revealOnSongStart) }
    var introRevealDelayMs by remember(introCfg) { mutableFloatStateOf(introCfg.revealDelayMs.toFloat().coerceIn(0f, 600_000f)) }
    var introFadeSec by remember(introCfg) { mutableFloatStateOf(introCfg.fadeSec.coerceIn(0f, 10f)) }

    var evName by remember { mutableStateOf("Add Camera Zoom") }
    var evVal1 by remember { mutableStateOf("0.04") }
    var evVal2 by remember { mutableStateOf("0.02") }

    Dialog(onDismissRequest = onDismiss) {
        Surface(
            shape = RoundedCornerShape(14.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.6f)),
            modifier = Modifier
                .fillMaxWidth()
                .heightIn(max = 520.dp)
        ) {
            Column(
                modifier = Modifier
                    .padding(14.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Text(
                    text = "Propiedades Completas de chart.json",
                    color = StudioColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 15.sp
                )

                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(
                        value = titleText,
                        onValueChange = { titleText = it },
                        label = { Text("Canción (song)", fontSize = 10.sp) },
                        singleLine = true,
                        modifier = Modifier.weight(1.3f)
                    )
                    OutlinedTextField(
                        value = speedText,
                        onValueChange = { speedText = it },
                        label = { Text("Velocidad", fontSize = 10.sp) },
                        singleLine = true,
                        modifier = Modifier.weight(0.7f)
                    )
                }

                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(
                        value = player1Text,
                        onValueChange = { player1Text = it },
                        label = { Text("player1", fontSize = 10.sp) },
                        singleLine = true,
                        modifier = Modifier.weight(1f)
                    )
                    OutlinedTextField(
                        value = player2Text,
                        onValueChange = { player2Text = it },
                        label = { Text("player2", fontSize = 10.sp) },
                        singleLine = true,
                        modifier = Modifier.weight(1f)
                    )
                }

                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(
                        value = gfText,
                        onValueChange = { gfText = it },
                        label = { Text("gfVersion", fontSize = 10.sp) },
                        singleLine = true,
                        modifier = Modifier.weight(1f)
                    )
                    OutlinedTextField(
                        value = stageText,
                        onValueChange = { stageText = it },
                        label = { Text("stage", fontSize = 10.sp) },
                        singleLine = true,
                        modifier = Modifier.weight(1f)
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("needsVoices (separar pistas vocales)", color = StudioColors.TextPrimary, fontSize = 12.sp)
                    Switch(checked = needsVoices, onCheckedChange = { needsVoices = it })
                }

                Button(
                    onClick = {
                        val spd = speedText.toDoubleOrNull() ?: chart.speed
                        onSaveDetails(titleText, spd, player1Text, player2Text, gfText, stageText, needsVoices)
                    },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = StudioColors.AccentEmerald,
                        contentColor = Color(0xFF042117)
                    ),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text("Guardar Metadatos del Chart", fontWeight = FontWeight.Bold, fontSize = 12.sp)
                }

                HorizontalDivider(color = StudioColors.BorderSubtle)
                Text(
                    text = "Inicio del Mod (Pantalla Negra / UI / Flechas)",
                    color = StudioColors.AccentCoral,
                    fontWeight = FontWeight.Bold,
                    fontSize = 12.sp
                )
                Text(
                    text = "Se guarda dentro de chart.json (uiBlackout) y se aplica al Frame 0 sin ocultar '3, 2, 1, ¡YA!' ni Pausa.",
                    color = StudioColors.TextMuted,
                    fontSize = 10.sp,
                    lineHeight = 13.sp
                )

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Iniciar en pantalla negra", color = StudioColors.TextPrimary, fontSize = 12.sp, modifier = Modifier.weight(1f))
                    Switch(checked = introStartBlackout, onCheckedChange = { introStartBlackout = it })
                }

                if (introStartBlackout) {
                    Text(
                        text = "Opacidad cortina negra: ${(introBlackoutAlpha * 100).roundToInt()}%",
                        color = StudioColors.TextMuted,
                        fontSize = 11.sp
                    )
                    Slider(
                        value = introBlackoutAlpha,
                        onValueChange = { introBlackoutAlpha = it },
                        valueRange = 0.0f..1.0f
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Ocultar HUD al entrar al mod", color = StudioColors.TextPrimary, fontSize = 12.sp, modifier = Modifier.weight(1f))
                    Switch(checked = introHideHud, onCheckedChange = { introHideHud = it })
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Ocultar flechas al entrar al mod", color = StudioColors.TextPrimary, fontSize = 12.sp, modifier = Modifier.weight(1f))
                    Switch(checked = introHideArrows, onCheckedChange = { introHideArrows = it })
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Revelar al terminar '3, 2, 1, ¡YA!'", color = StudioColors.TextPrimary, fontSize = 12.sp, modifier = Modifier.weight(1f))
                    Switch(checked = introRevealOnSongStart, onCheckedChange = { introRevealOnStart -> introRevealOnSongStart = introRevealOnStart })
                }

                if (introRevealOnSongStart) {
                    val delaySecTotal = introRevealDelayMs / 1000.0
                    val delayLabel = if (delaySecTotal >= 60.0) {
                        val mins = (delaySecTotal / 60.0).toInt()
                        val rem = delaySecTotal - mins * 60.0
                        String.format(Locale.US, "%dm %04.1fs", mins, rem)
                    } else {
                        String.format(Locale.US, "%.1fs", delaySecTotal)
                    }
                    Text(
                        text = "Espera al ¡YA!: $delayLabel · Fade vuelta a color: ${String.format(Locale.US, "%.2fs", introFadeSec)}",
                        color = StudioColors.TextMuted,
                        fontSize = 10.sp
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.fillMaxWidth()) {
                        listOf(0.0f to "0s", 1000.0f to "+1s", 4000.0f to "+4s", 10000.0f to "+10s", 30000.0f to "+30s").forEach { (ms, label) ->
                            OutlinedButton(
                                onClick = { introRevealDelayMs = ms },
                                contentPadding = PaddingValues(horizontal = 4.dp, vertical = 2.dp),
                                border = if (abs(introRevealDelayMs - ms) < 15f) BorderStroke(1.5.dp, StudioColors.AccentAmber) else null,
                                modifier = Modifier.weight(1f).height(28.dp)
                            ) {
                                Text(label, fontSize = 9.sp, maxLines = 1, softWrap = false)
                            }
                        }
                    }
                    Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.fillMaxWidth()) {
                        listOf(0.0f to "Corte", 0.35f to "0.35s", 0.65f to "0.65s", 1.2f to "1.2s", 2.5f to "2.5s").forEach { (sec, label) ->
                            OutlinedButton(
                                onClick = { introFadeSec = sec },
                                contentPadding = PaddingValues(horizontal = 4.dp, vertical = 2.dp),
                                border = if (abs(introFadeSec - sec) < 0.05f) BorderStroke(1.5.dp, StudioColors.AccentCoral) else null,
                                modifier = Modifier.weight(1f).height(28.dp)
                            ) {
                                Text(label, fontSize = 9.sp, maxLines = 1, softWrap = false)
                            }
                        }
                    }
                }

                Button(
                    onClick = {
                        onSaveIntroUi(
                            introStartBlackout,
                            introBlackoutAlpha.coerceIn(0f, 1f),
                            introHideHud,
                            introHideArrows,
                            introRevealOnSongStart,
                            introRevealDelayMs.toDouble(),
                            introFadeSec.coerceIn(0f, 10f)
                        )
                    },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = StudioColors.AccentCoral,
                        contentColor = Color.White
                    ),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text("Guardar Inicio Oscuro / UI / Flechas", fontWeight = FontWeight.Bold, fontSize = 12.sp)
                }

                if (activeSection != null) {
                    HorizontalDivider(color = StudioColors.BorderSubtle)
                    Text(
                        text = "BPM en Sección ${activeSection.index + 1} (${activeSection.bpm.roundToInt()} BPM)",
                        color = StudioColors.AccentAmber,
                        fontWeight = FontWeight.SemiBold,
                        fontSize = 12.sp
                    )
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            Switch(
                                checked = activeSection.changeBpm,
                                onCheckedChange = { checked ->
                                    onSetSectionBpm(activeSection.index, activeSection.bpm, checked)
                                }
                            )
                            Text("changeBPM", color = StudioColors.TextMuted, fontSize = 11.sp)
                        }
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            OutlinedButton(
                                onClick = { onSetSectionBpm(activeSection.index, (activeSection.bpm - 5.0).coerceAtLeast(40.0), true) },
                                contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp),
                                modifier = Modifier.height(30.dp)
                            ) {
                                Text("-5", fontSize = 11.sp)
                            }
                            OutlinedButton(
                                onClick = { onSetSectionBpm(activeSection.index, (activeSection.bpm + 5.0).coerceAtMost(320.0), true) },
                                contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp),
                                modifier = Modifier.height(30.dp)
                            ) {
                                Text("+5", fontSize = 11.sp)
                            }
                        }
                    }
                }

                HorizontalDivider(color = StudioColors.BorderSubtle)
                Text(
                    text = "Eventos de chart.json (${chart.events.size})",
                    color = StudioColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 12.sp
                )
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp), verticalAlignment = Alignment.CenterVertically) {
                    OutlinedTextField(
                        value = evName,
                        onValueChange = { evName = it },
                        label = { Text("Evento", fontSize = 10.sp) },
                        singleLine = true,
                        modifier = Modifier.weight(1.2f)
                    )
                    OutlinedTextField(
                        value = evVal1,
                        onValueChange = { evVal1 = it },
                        label = { Text("v1", fontSize = 10.sp) },
                        singleLine = true,
                        modifier = Modifier.weight(0.6f)
                    )
                    OutlinedTextField(
                        value = evVal2,
                        onValueChange = { evVal2 = it },
                        label = { Text("v2", fontSize = 10.sp) },
                        singleLine = true,
                        modifier = Modifier.weight(0.6f)
                    )
                    Button(
                        onClick = {
                            onAddEvent(activeSection?.startMs ?: 0.0, evName, evVal1, evVal2)
                        },
                        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                        modifier = Modifier.height(42.dp)
                    ) {
                        Icon(Icons.Rounded.Add, "Añadir", modifier = Modifier.size(16.dp))
                    }
                }

                chart.events.forEachIndexed { idx, ev ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .background(StudioColors.BgCard, RoundedCornerShape(6.dp))
                            .padding(horizontal = 8.dp, vertical = 4.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = "${(ev.timeMs / 1000.0).let { String.format("%.2fs", it) }} · ${ev.name} [${ev.value1}, ${ev.value2}]",
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp,
                            modifier = Modifier.weight(1f)
                        )
                        IconButton(onClick = { onRemoveEvent(idx) }, modifier = Modifier.size(24.dp)) {
                            Icon(Icons.Rounded.Delete, "Quitar", tint = StudioColors.AccentCoral, modifier = Modifier.size(14.dp))
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
