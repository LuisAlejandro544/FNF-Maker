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
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
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
                .height(420.dp)
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
