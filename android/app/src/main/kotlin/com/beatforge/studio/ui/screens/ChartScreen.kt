// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/ChartScreen.kt
// Propósito: Editor Manual de Chart de línea de tiempo larga (8 carriles: 4 Oponente + 4 Jugador)
// sincronizado directamente con Rust (`pulse_core/src/chart.rs`).
// Permite chartear manualmente paso a paso, editar metadatos completos de `chart.json`
// (`player1`, `player2`, `gfVersion`, `stage`, `needsVoices`, `events`, `changeBPM` por sección),
// exportar `.json` completo vía SAF y escuchar cada sección en tiempo real.

package com.beatforge.studio.ui.screens

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
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.RestartAlt
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material.icons.rounded.SwapHoriz
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material.icons.rounded.UploadFile
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
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import com.beatforge.studio.ChartSectionSnapshot
import com.beatforge.studio.ChartSnapshot
import com.beatforge.studio.ImportKind
import com.beatforge.studio.ProjectSnapshot
import com.beatforge.studio.VisibleNote
import com.beatforge.studio.ui.theme.StudioColors
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.roundToInt

private val LANE_ARROWS = listOf("←", "↓", "↑", "→")

private sealed class ChartTimelineRow {
    data class SectionHeader(val section: ChartSectionSnapshot) : ChartTimelineRow()
    data class StepRow(
        val section: ChartSectionSnapshot,
        val stepInSec: Int,
        val stepMs: Double,
        val rowNotesByCol: Map<Int, VisibleNote>
    ) : ChartTimelineRow()
}

@Composable
fun ChartScreen(
    snapshot: ProjectSnapshot,
    audioPreviewPlaying: Boolean,
    onRequestImport: (ImportKind) -> Unit,
    onRequestExportJson: () -> Unit,
    onRestoreDemoChart: () -> Unit,
    onSetChartBpm: (Double) -> Unit,
    onUpdateChartDetails: (
        title: String,
        speed: Double,
        player1: String,
        player2: String,
        gfVersion: String,
        stage: String,
        needsVoices: Boolean
    ) -> Unit,
    onSetSectionBpm: (secIndex: Int, bpm: Double, changeBpm: Boolean) -> Unit,
    onAddChartEvent: (timeMs: Double, name: String, value1: String, value2: String) -> Unit,
    onRemoveChartEvent: (index: Int) -> Unit,
    onAddSection: (mustHit: Boolean) -> Unit,
    onToggleSectionMustHit: (secIndex: Int) -> Unit,
    onToggleChartNote: (secIndex: Int, stepInSec: Int, col0To7: Int, sustainSteps: Int) -> Unit,
    onToggleAudioPreviewAtMs: (Double) -> Unit
) {
    val chart = snapshot.chart
    var selectedSustainSteps by remember { mutableIntStateOf(0) }
    var selectedSectionIdx by remember { mutableIntStateOf(0) }
    var showDetailsDialog by remember { mutableStateOf(false) }
    val listState = rememberLazyListState()
    val scope = rememberCoroutineScope()

    // Construye la línea temporal continua larga con todas las secciones y pasos
    val timelineData = remember(chart.sections, chart.notes, chart.bpm) {
        val rows = mutableListOf<ChartTimelineRow>()
        val sectionHeaderRowIndex = linkedMapOf<Int, Int>()

        chart.sections.forEach { sec ->
            sectionHeaderRowIndex[sec.index] = rows.size
            rows.add(ChartTimelineRow.SectionHeader(sec))

            val stepMs = (60_000.0 / sec.bpm.coerceAtLeast(40.0)) / 4.0
            val tolerance = stepMs * 0.45
            val secEndMs = sec.startMs + stepMs * sec.lengthInSteps
            val secNotes = chart.notes.filter {
                it.timeMs >= sec.startMs - tolerance && it.timeMs < secEndMs + tolerance
            }

            for (step in 0 until sec.lengthInSteps) {
                val targetMs = sec.startMs + step * stepMs
                val map = mutableMapOf<Int, VisibleNote>()
                for (n in secNotes) {
                    if (abs(n.timeMs - targetMs) <= tolerance) {
                        val col = (if (n.isPlayer) 4 else 0) + (n.direction.coerceIn(0, 3))
                        map[col] = n
                    }
                }
                rows.add(ChartTimelineRow.StepRow(sec, step, stepMs, map))
            }
        }
        rows to sectionHeaderRowIndex
    }

    val rows = timelineData.first
    val sectionPositions = timelineData.second
    val activeSection = chart.sections.getOrNull(selectedSectionIdx) ?: chart.sections.firstOrNull()

    Row(
        modifier = Modifier
            .fillMaxSize()
            .padding(10.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        // Columna izquierda: Herramientas del Chart + salto por secciones
        Surface(
            modifier = Modifier
                .weight(0.32f)
                .fillMaxHeight(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Column(
                modifier = Modifier
                    .padding(10.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Text(
                    text = chart.title,
                    color = StudioColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 14.sp,
                    maxLines = 1
                )

                Text(
                    text = "${chart.totalNotes} notas (${chart.opponentNotes} Rival · ${chart.playerNotes} Jugador)",
                    color = StudioColors.TextMuted,
                    fontSize = 11.sp
                )

                // Importar / Exportar / Demo
                Row(horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                    Button(
                        onClick = { onRequestImport(ImportKind.Chart) },
                        contentPadding = PaddingValues(horizontal = 6.dp, vertical = 4.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = StudioColors.AccentCyan,
                            contentColor = Color(0xFF1F0C00)
                        ),
                        modifier = Modifier
                            .weight(1f)
                            .height(33.dp)
                    ) {
                        Icon(Icons.Rounded.UploadFile, null, modifier = Modifier.size(14.dp))
                        Spacer(Modifier.width(3.dp))
                        Text("Importar", fontSize = 10.sp, fontWeight = FontWeight.Bold, maxLines = 1, softWrap = false)
                    }

                    Button(
                        onClick = onRequestExportJson,
                        contentPadding = PaddingValues(horizontal = 6.dp, vertical = 4.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = StudioColors.AccentIndigo,
                            contentColor = Color.White
                        ),
                        modifier = Modifier
                            .weight(1f)
                            .height(33.dp)
                    ) {
                        Icon(Icons.Rounded.Download, null, modifier = Modifier.size(14.dp))
                        Spacer(Modifier.width(3.dp))
                        Text("Exportar", fontSize = 10.sp, fontWeight = FontWeight.Bold, maxLines = 1, softWrap = false)
                    }
                }

                Row(horizontalArrangement = Arrangement.spacedBy(5.dp)) {
                    OutlinedButton(
                        onClick = { showDetailsDialog = true },
                        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                        modifier = Modifier
                            .weight(1.1f)
                            .height(32.dp)
                    ) {
                        Icon(Icons.Rounded.Tune, null, modifier = Modifier.size(14.dp))
                        Spacer(Modifier.width(4.dp))
                        Text("Config .json (${chart.events.size} ev.)", fontSize = 10.sp, maxLines = 1, softWrap = false)
                    }

                    OutlinedButton(
                        onClick = onRestoreDemoChart,
                        contentPadding = PaddingValues(horizontal = 6.dp, vertical = 4.dp),
                        modifier = Modifier
                            .weight(0.65f)
                            .height(32.dp)
                    ) {
                        Icon(Icons.Rounded.RestartAlt, null, modifier = Modifier.size(14.dp))
                        Spacer(Modifier.width(2.dp))
                        Text("Demo", fontSize = 10.sp, maxLines = 1, softWrap = false)
                    }
                }

                // Control de BPM
                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = StudioColors.BgCard,
                    border = BorderStroke(1.dp, StudioColors.BorderSubtle)
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 8.dp, vertical = 4.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text(
                            text = "BPM: ${chart.bpm.roundToInt()}",
                            color = StudioColors.AccentAmber,
                            fontWeight = FontWeight.Bold,
                            fontSize = 13.sp
                        )
                        Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            OutlinedButton(
                                onClick = { onSetChartBpm((chart.bpm - 5.0).coerceAtLeast(40.0)) },
                                contentPadding = PaddingValues(0.dp),
                                modifier = Modifier.size(30.dp)
                            ) {
                                Text("-5", fontSize = 11.sp)
                            }
                            OutlinedButton(
                                onClick = { onSetChartBpm((chart.bpm + 5.0).coerceAtMost(320.0)) },
                                contentPadding = PaddingValues(0.dp),
                                modifier = Modifier.size(30.dp)
                            ) {
                                Text("+5", fontSize = 11.sp)
                            }
                        }
                    }
                }

                // Duración de nota sostenida (Sustain)
                Text("Sustain al colocar (pasos):", color = StudioColors.TextMuted, fontSize = 11.sp)
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.fillMaxWidth()) {
                    listOf(0, 2, 4, 8).forEach { steps ->
                        val isSel = selectedSustainSteps == steps
                        Box(
                            modifier = Modifier
                                .weight(1f)
                                .height(30.dp)
                                .clip(RoundedCornerShape(8.dp))
                                .background(
                                    if (isSel) StudioColors.AccentAmber.copy(alpha = 0.22f)
                                    else StudioColors.BgCard
                                )
                                .border(
                                    width = 1.dp,
                                    color = if (isSel) StudioColors.AccentAmber else StudioColors.BorderSubtle,
                                    shape = RoundedCornerShape(8.dp)
                                )
                                .clickable { selectedSustainSteps = steps },
                            contentAlignment = Alignment.Center
                        ) {
                            Text(
                                text = if (steps == 0) "Normal" else "${steps}p",
                                color = if (isSel) StudioColors.AccentAmber else StudioColors.TextMuted,
                                fontSize = 11.sp,
                                fontWeight = if (isSel) FontWeight.Bold else FontWeight.Medium,
                                maxLines = 1,
                                softWrap = false
                            )
                        }
                    }
                }

                // Botón escuchar audio desde la sección seleccionada
                Button(
                    onClick = {
                        onToggleAudioPreviewAtMs(activeSection?.startMs ?: 0.0)
                    },
                    contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = if (audioPreviewPlaying) StudioColors.AccentCoral else StudioColors.AccentEmerald,
                        contentColor = if (audioPreviewPlaying) Color.White else Color(0xFF042117)
                    ),
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(36.dp)
                ) {
                    Icon(
                        imageVector = if (audioPreviewPlaying) Icons.Rounded.Stop else Icons.Rounded.PlayArrow,
                        contentDescription = null,
                        modifier = Modifier.size(16.dp)
                    )
                    Spacer(Modifier.width(6.dp))
                    Text(
                        text = if (audioPreviewPlaying) "Detener Audio" else "Escuchar Sec. ${(activeSection?.index ?: 0) + 1}",
                        fontSize = 12.sp,
                        fontWeight = FontWeight.Bold,
                        maxLines = 1,
                        softWrap = false
                    )
                }

                HorizontalDivider(color = StudioColors.BorderSubtle)

                // Secciones (salto rápido + añadir nueva sección larga)
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Secciones (${chart.sections.size})", color = StudioColors.TextPrimary, fontWeight = FontWeight.Bold, fontSize = 12.sp)
                    Button(
                        onClick = {
                            onAddSection(true)
                            selectedSectionIdx = chart.sections.size
                        },
                        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = StudioColors.BgElevated,
                            contentColor = StudioColors.AccentCyan
                        ),
                        modifier = Modifier.height(28.dp)
                    ) {
                        Icon(Icons.Rounded.Add, null, modifier = Modifier.size(14.dp))
                        Spacer(Modifier.width(2.dp))
                        Text("Sección", fontSize = 10.sp, maxLines = 1, softWrap = false)
                    }
                }

                Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    chart.sections.forEach { sec ->
                        val isSel = sec.index == selectedSectionIdx
                        Surface(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable {
                                    selectedSectionIdx = sec.index
                                    val rowIdx = sectionPositions[sec.index] ?: 0
                                    scope.launch { listState.animateScrollToItem(rowIdx) }
                                },
                            shape = RoundedCornerShape(7.dp),
                            color = if (isSel) StudioColors.AccentCyan.copy(alpha = 0.16f) else StudioColors.BgCard,
                            border = BorderStroke(1.dp, if (isSel) StudioColors.AccentCyan else StudioColors.BorderSubtle)
                        ) {
                            Row(
                                modifier = Modifier.padding(horizontal = 8.dp, vertical = 5.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.SpaceBetween
                            ) {
                                Text(
                                    text = "Sec ${sec.index + 1} · ${(sec.startMs / 1000.0).let { String.format("%.1fs", it) }}",
                                    color = StudioColors.TextPrimary,
                                    fontSize = 11.sp,
                                    fontWeight = FontWeight.SemiBold
                                )
                                Text(
                                    text = if (sec.mustHitSection) "Jugador" else "Oponente",
                                    color = if (sec.mustHitSection) StudioColors.AccentEmerald else StudioColors.AccentCoral,
                                    fontSize = 10.sp,
                                    fontWeight = FontWeight.Bold
                                )
                            }
                        }
                    }
                }
            }
        }

        // Columna derecha: Línea temporal larga desplazable (8 carriles táctiles)
        Surface(
            modifier = Modifier
                .weight(0.68f)
                .fillMaxHeight(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Column(modifier = Modifier.fillMaxSize()) {
                // Cabecera fija de carriles
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .background(StudioColors.BgCard)
                        .padding(horizontal = 8.dp, vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = "#",
                        color = StudioColors.TextMuted,
                        fontSize = 10.sp,
                        fontWeight = FontWeight.Bold,
                        modifier = Modifier.width(34.dp)
                    )
                    Column(modifier = Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                        Text("OPONENTE", color = StudioColors.AccentCoral, fontSize = 10.sp, fontWeight = FontWeight.Bold)
                        Row(modifier = Modifier.fillMaxWidth()) {
                            for (i in 0..3) {
                                Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                                    Text(LANE_ARROWS[i], color = StudioColors.LaneColors[i], fontSize = 12.sp, fontWeight = FontWeight.Bold)
                                }
                            }
                        }
                    }
                    Spacer(Modifier.width(6.dp))
                    Column(modifier = Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                        Text("JUGADOR", color = StudioColors.AccentEmerald, fontSize = 10.sp, fontWeight = FontWeight.Bold)
                        Row(modifier = Modifier.fillMaxWidth()) {
                            for (i in 0..3) {
                                Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                                    Text(LANE_ARROWS[i], color = StudioColors.LaneColors[i], fontSize = 12.sp, fontWeight = FontWeight.Bold)
                                }
                            }
                        }
                    }
                }
                HorizontalDivider(color = StudioColors.BorderSubtle)

                // Rejilla larga vertical continua de todas las secciones y pasos
                LazyColumn(
                    state = listState,
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        .background(StudioColors.BgDark)
                ) {
                    items(rows.size) { index ->
                        when (val row = rows[index]) {
                            is ChartTimelineRow.SectionHeader -> {
                                val sec = row.section
                                Row(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .background(StudioColors.BgElevated)
                                        .padding(horizontal = 10.dp, vertical = 5.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.SpaceBetween
                                ) {
                                    Text(
                                        text = "Sección ${sec.index + 1}  ·  ${String.format("%.2fs", sec.startMs / 1000.0)}  ·  ${sec.bpm.roundToInt()} BPM",
                                        color = StudioColors.TextPrimary,
                                        fontSize = 11.sp,
                                        fontWeight = FontWeight.Bold
                                    )
                                    Row(
                                        verticalAlignment = Alignment.CenterVertically,
                                        modifier = Modifier
                                            .clip(RoundedCornerShape(6.dp))
                                            .background(StudioColors.BgCard)
                                            .clickable { onToggleSectionMustHit(sec.index) }
                                            .padding(horizontal = 8.dp, vertical = 3.dp)
                                    ) {
                                        Icon(Icons.Rounded.SwapHoriz, null, tint = StudioColors.AccentAmber, modifier = Modifier.size(13.dp))
                                        Spacer(Modifier.width(4.dp))
                                        Text(
                                            text = if (sec.mustHitSection) "Cámara: Jugador" else "Cámara: Oponente",
                                            color = if (sec.mustHitSection) StudioColors.AccentEmerald else StudioColors.AccentCoral,
                                            fontSize = 10.sp,
                                            fontWeight = FontWeight.Bold
                                        )
                                    }
                                }
                            }

                            is ChartTimelineRow.StepRow -> {
                                val isBeat = row.stepInSec % 4 == 0
                                val rowBg = if (isBeat) StudioColors.BgPanel.copy(alpha = 0.72f) else StudioColors.BgDark
                                Row(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .height(34.dp)
                                        .background(rowBg)
                                        .padding(horizontal = 8.dp),
                                    verticalAlignment = Alignment.CenterVertically
                                ) {
                                    Text(
                                        text = "${row.stepInSec}",
                                        color = if (isBeat) StudioColors.AccentAmber else StudioColors.TextMuted.copy(alpha = 0.65f),
                                        fontSize = 10.sp,
                                        fontWeight = if (isBeat) FontWeight.Bold else FontWeight.Normal,
                                        modifier = Modifier.width(34.dp)
                                    )

                                    // 4 columnas Oponente (0..3)
                                    Row(
                                        modifier = Modifier.weight(1f),
                                        horizontalArrangement = Arrangement.spacedBy(3.dp)
                                    ) {
                                        for (c in 0..3) {
                                            ChartNoteCell(
                                                note = row.rowNotesByCol[c],
                                                laneIndex = c,
                                                stepMs = row.stepMs,
                                                onClick = {
                                                    selectedSectionIdx = row.section.index
                                                    onToggleChartNote(row.section.index, row.stepInSec, c, selectedSustainSteps)
                                                },
                                                modifier = Modifier.weight(1f)
                                            )
                                        }
                                    }

                                    Spacer(Modifier.width(6.dp))

                                    // 4 columnas Jugador (4..7)
                                    Row(
                                        modifier = Modifier.weight(1f),
                                        horizontalArrangement = Arrangement.spacedBy(3.dp)
                                    ) {
                                        for (c in 4..7) {
                                            ChartNoteCell(
                                                note = row.rowNotesByCol[c],
                                                laneIndex = c - 4,
                                                stepMs = row.stepMs,
                                                onClick = {
                                                    selectedSectionIdx = row.section.index
                                                    onToggleChartNote(row.section.index, row.stepInSec, c, selectedSustainSteps)
                                                },
                                                modifier = Modifier.weight(1f)
                                            )
                                        }
                                    }
                                }
                                HorizontalDivider(
                                    color = StudioColors.BorderSubtle.copy(alpha = if (isBeat) 0.55f else 0.22f),
                                    thickness = 0.5.dp
                                )
                            }
                        }
                    }
                }
            }
        }
    }
    if (showDetailsDialog) {
        ChartJsonDetailsDialog(
            chart = chart,
            activeSection = activeSection,
            onSaveDetails = onUpdateChartDetails,
            onSetSectionBpm = onSetSectionBpm,
            onAddEvent = onAddChartEvent,
            onRemoveEvent = onRemoveChartEvent,
            onDismiss = { showDetailsDialog = false }
        )
    }
}

@Composable
private fun ChartJsonDetailsDialog(
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

@Composable
private fun ChartNoteCell(
    note: VisibleNote?,
    laneIndex: Int,
    stepMs: Double,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val laneColor = StudioColors.LaneColors[laneIndex]
    val hasNote = note != null
    val sustainSteps = if (note != null && note.sustainMs > stepMs * 0.4) {
        (note.sustainMs / stepMs).roundToInt().coerceAtLeast(1)
    } else {
        0
    }

    Box(
        modifier = modifier
            .height(28.dp)
            .clip(RoundedCornerShape(5.dp))
            .background(if (hasNote) laneColor.copy(alpha = 0.88f) else StudioColors.BgCard.copy(alpha = 0.55f))
            .border(
                width = if (hasNote) 1.dp else 0.5.dp,
                color = if (hasNote) Color.White.copy(alpha = 0.65f) else StudioColors.BorderSubtle.copy(alpha = 0.45f),
                shape = RoundedCornerShape(5.dp)
            )
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        if (hasNote) {
            Text(
                text = if (sustainSteps > 0) "${LANE_ARROWS[laneIndex]}+$sustainSteps" else LANE_ARROWS[laneIndex],
                color = Color.White,
                fontSize = 11.sp,
                fontWeight = FontWeight.Bold,
                maxLines = 1,
                softWrap = false
            )
        }
    }
}
