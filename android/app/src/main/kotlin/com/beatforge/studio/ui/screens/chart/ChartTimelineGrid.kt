// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/chart/ChartTimelineGrid.kt
// Propósito: Rejilla vertical continua de la línea de tiempo del chart (8 carriles:
// 4 Oponente + 4 Jugador). Dibuja colas de notas sostenidas continuas desde la flecha
// origen hasta su paso final (incluso cruzando secciones) y convierte la columna `#`
// en disparadores interactivos de Flashes, Swaps de visibilidad y Cambio de Personajes.

package com.beatforge.studio.ui.screens.chart

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
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
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.GpsFixed
import androidx.compose.material.icons.rounded.GpsOff
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material.icons.rounded.SwapHoriz
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.VisibleNote
import com.beatforge.studio.ui.theme.StudioColors
import java.util.Locale
import kotlin.math.roundToInt

private val LANE_ARROWS = listOf("←", "↓", "↑", "→")

@Composable
fun ChartTimelineGrid(
    rows: List<ChartTimelineRow>,
    listState: LazyListState,
    selectedSustainSteps: Int,
    audioPreviewPlaying: Boolean,
    playheadState: ChartPlayheadState,
    onPlayOrPausePlayhead: () -> Unit,
    onRewindPlayheadToStart: () -> Unit,
    onToggleFollowPlayhead: () -> Unit,
    onScrubPlayheadMs: (Double) -> Unit,
    onSeekPlayheadAndPauseOrKeep: (Double) -> Unit,
    onToggleSectionMustHit: (secIndex: Int) -> Unit,
    onSelectSection: (secIndex: Int) -> Unit,
    onToggleChartNote: (secIndex: Int, stepInSec: Int, col0To7: Int, sustainSteps: Int) -> Unit,
    onOpenStepCueDialog: (ChartTimelineRow.StepRow) -> Unit,
    modifier: Modifier = Modifier
) {
    Surface(
        modifier = modifier.fillMaxHeight(),
        shape = RoundedCornerShape(12.dp),
        color = StudioColors.BgPanel,
        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Barra superior compacta de transporte del hilo reproductor y seguimiento
            ChartPlayheadTransportBar(
                audioPreviewPlaying = audioPreviewPlaying,
                playheadState = playheadState,
                onPlayOrPause = onPlayOrPausePlayhead,
                onRewindToStart = onRewindPlayheadToStart,
                onToggleFollow = onToggleFollowPlayhead,
                onScrubMs = onScrubPlayheadMs
            )
            HorizontalDivider(color = StudioColors.BorderSubtle)

            // Cabecera fija de carriles y columna interactiva # / Eventos
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(StudioColors.BgCard)
                    .padding(horizontal = 8.dp, vertical = 5.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(
                    modifier = Modifier.width(52.dp),
                    horizontalAlignment = Alignment.CenterHorizontally
                ) {
                    Text(
                        text = "# Ev.",
                        color = StudioColors.AccentCyan,
                        fontSize = 10.sp,
                        fontWeight = FontWeight.Bold
                    )
                    Text(
                        text = "mantener=hilo",
                        color = StudioColors.TextMuted,
                        fontSize = 7.sp,
                        maxLines = 1,
                        softWrap = false
                    )
                }

                Column(modifier = Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("OPONENTE", color = StudioColors.AccentCoral, fontSize = 10.sp, fontWeight = FontWeight.Bold)
                    Row(modifier = Modifier.fillMaxWidth()) {
                        for (i in 0..3) {
                            Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                                Text(
                                    text = LANE_ARROWS[i],
                                    color = StudioColors.LaneColors[i],
                                    fontSize = 12.sp,
                                    fontWeight = FontWeight.Bold
                                )
                            }
                        }
                    }
                }

                Spacer(Modifier.width(8.dp))

                Column(modifier = Modifier.weight(1f), horizontalAlignment = Alignment.CenterHorizontally) {
                    Text("JUGADOR", color = StudioColors.AccentEmerald, fontSize = 10.sp, fontWeight = FontWeight.Bold)
                    Row(modifier = Modifier.fillMaxWidth()) {
                        for (i in 0..3) {
                            Box(Modifier.weight(1f), contentAlignment = Alignment.Center) {
                                Text(
                                    text = LANE_ARROWS[i],
                                    color = StudioColors.LaneColors[i],
                                    fontSize = 12.sp,
                                    fontWeight = FontWeight.Bold
                                )
                            }
                        }
                    }
                }
            }
            HorizontalDivider(color = StudioColors.BorderSubtle)

            // Contenedor con LazyColumn + hilo reproductor flotante a 60 FPS sin recomponer filas
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
                    .background(StudioColors.BgDark)
            ) {
                LazyColumn(
                    state = listState,
                    modifier = Modifier.fillMaxSize()
                ) {
                    items(rows.size) { index ->
                        when (val row = rows[index]) {
                            is ChartTimelineRow.SectionHeader -> {
                                ChartSectionHeaderRow(
                                    row = row,
                                    onToggleSectionMustHit = { onToggleSectionMustHit(row.section.index) }
                                )
                            }

                            is ChartTimelineRow.StepRow -> {
                                ChartStepTimelineRow(
                                    row = row,
                                    selectedSustainSteps = selectedSustainSteps,
                                    onSelectSection = onSelectSection,
                                    onToggleChartNote = onToggleChartNote,
                                    onOpenStepCueDialog = onOpenStepCueDialog,
                                    onSeekToStep = { onSeekPlayheadAndPauseOrKeep(row.stepTimeMs) }
                                )
                            }
                        }
                    }
                }

                ChartPlayheadThreadOverlay(
                    listState = listState,
                    playheadState = playheadState,
                    audioPreviewPlaying = audioPreviewPlaying,
                    modifier = Modifier.fillMaxSize()
                )
            }
        }
    }
}

@Composable
private fun ChartPlayheadTransportBar(
    audioPreviewPlaying: Boolean,
    playheadState: ChartPlayheadState,
    onPlayOrPause: () -> Unit,
    onRewindToStart: () -> Unit,
    onToggleFollow: () -> Unit,
    onScrubMs: (Double) -> Unit
) {
    val dur = playheadState.durationMs.coerceAtLeast(1.0)
    val sliderPos = (playheadState.posMs / dur).toFloat().coerceIn(0f, 1f)
    val currentTarget = playheadState.activeTarget

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(StudioColors.BgElevated.copy(alpha = 0.72f))
            .padding(horizontal = 8.dp, vertical = 3.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(5.dp)
    ) {
        IconButton(
            onClick = onPlayOrPause,
            modifier = Modifier
                .size(28.dp)
                .clip(RoundedCornerShape(7.dp))
                .background(if (audioPreviewPlaying) StudioColors.AccentAmber else StudioColors.AccentEmerald)
        ) {
            Icon(
                imageVector = if (audioPreviewPlaying) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
                contentDescription = if (audioPreviewPlaying) "Pausar" else "Reproducir",
                tint = Color(0xFF0A1410),
                modifier = Modifier.size(16.dp)
            )
        }

        IconButton(
            onClick = onRewindToStart,
            modifier = Modifier.size(26.dp)
        ) {
            Icon(
                imageVector = Icons.Rounded.SkipPrevious,
                contentDescription = "Reiniciar hilo",
                tint = StudioColors.TextMuted,
                modifier = Modifier.size(16.dp)
            )
        }

        Row(
            verticalAlignment = Alignment.CenterVertically,
            modifier = Modifier
                .clip(RoundedCornerShape(7.dp))
                .background(
                    if (playheadState.followThread) StudioColors.AccentCyan.copy(alpha = 0.16f)
                    else StudioColors.BgCard
                )
                .border(
                    1.dp,
                    if (playheadState.followThread) StudioColors.AccentCyan else StudioColors.BorderSubtle,
                    RoundedCornerShape(7.dp)
                )
                .clickable(onClick = onToggleFollow)
                .padding(horizontal = 7.dp, vertical = 4.dp)
        ) {
            Icon(
                imageVector = if (playheadState.followThread) Icons.Rounded.GpsFixed else Icons.Rounded.GpsOff,
                contentDescription = null,
                tint = if (playheadState.followThread) StudioColors.AccentCyan else StudioColors.TextMuted,
                modifier = Modifier.size(12.dp)
            )
            Spacer(Modifier.width(3.dp))
            Text(
                text = if (playheadState.followThread) "Seguir" else "Libre",
                color = if (playheadState.followThread) StudioColors.AccentCyan else StudioColors.TextMuted,
                fontSize = 10.sp,
                fontWeight = FontWeight.Bold,
                maxLines = 1,
                softWrap = false
            )
        }

        Slider(
            value = sliderPos,
            onValueChange = { frac ->
                onScrubMs(frac.toDouble() * dur)
            },
            colors = SliderDefaults.colors(
                thumbColor = if (audioPreviewPlaying) StudioColors.AccentEmerald else StudioColors.AccentCyan,
                activeTrackColor = if (audioPreviewPlaying) StudioColors.AccentEmerald else StudioColors.AccentCyan,
                inactiveTrackColor = StudioColors.BorderSubtle
            ),
            modifier = Modifier
                .weight(1f)
                .height(24.dp)
        )

        Text(
            text = "${formatChartTimeMs(playheadState.posMs)} / #${currentTarget?.stepRow?.stepInSec ?: 0}",
            color = StudioColors.TextPrimary,
            fontSize = 10.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            softWrap = false
        )
    }
}

@Composable
private fun ChartPlayheadThreadOverlay(
    listState: LazyListState,
    playheadState: ChartPlayheadState,
    audioPreviewPlaying: Boolean,
    modifier: Modifier = Modifier
) {
    Canvas(modifier = modifier) {
        val target = playheadState.activeTarget ?: return@Canvas
        val visItem = listState.layoutInfo.visibleItemsInfo.firstOrNull { it.index == target.rowIndex }
            ?: return@Canvas

        val lineY = visItem.offset.toFloat() + (target.stepFraction * visItem.size.toFloat())
        if (lineY < -8f || lineY > size.height + 8f) return@Canvas

        val threadColor = if (audioPreviewPlaying) StudioColors.AccentEmerald else StudioColors.AccentCyan
        val padPx = 8.dp.toPx()

        // Resalte sutil de la fila donde descansa el hilo
        drawRect(
            color = threadColor.copy(alpha = 0.08f),
            topLeft = Offset(0f, visItem.offset.toFloat()),
            size = Size(size.width, visItem.size.toFloat())
        )

        // Halo luminoso y línea del hilo reproductor cruzando # y los 8 carriles
        drawRect(
            color = threadColor.copy(alpha = 0.28f),
            topLeft = Offset(padPx, lineY - 3.dp.toPx()),
            size = Size((size.width - padPx * 2f).coerceAtLeast(10f), 6.dp.toPx())
        )
        drawLine(
            color = threadColor,
            start = Offset(padPx, lineY),
            end = Offset(size.width - padPx, lineY),
            strokeWidth = 2.2.dp.toPx()
        )

        // Puntero lateral izquierdo junto a la columna '#'
        val tipX = padPx + 8.dp.toPx()
        val halfH = 5.dp.toPx()
        val arrowPath = Path().apply {
            moveTo(padPx, lineY - halfH)
            lineTo(tipX, lineY)
            lineTo(padPx, lineY + halfH)
            close()
        }
        drawPath(arrowPath, color = threadColor)
    }
}

@Composable
private fun ChartSectionHeaderRow(
    row: ChartTimelineRow.SectionHeader,
    onToggleSectionMustHit: () -> Unit
) {
    val sec = row.section
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(30.dp)
            .background(StudioColors.BgElevated.copy(alpha = 0.92f))
    ) {
        // Si alguna nota sostenida cruza desde la sección anterior, dibuja la estela continua detrás de la cabecera
        if (row.crossingCols.isNotEmpty()) {
            Row(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(horizontal = 8.dp)
            ) {
                Spacer(Modifier.width(52.dp))
                Row(modifier = Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                    for (c in 0..3) {
                        Box(modifier = Modifier.weight(1f).fillMaxHeight()) {
                            if (c in row.crossingCols) {
                                SustainConnectorCanvas(laneIndex = c)
                            }
                        }
                    }
                }
                Spacer(Modifier.width(8.dp))
                Row(modifier = Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(3.dp)) {
                    for (c in 4..7) {
                        Box(modifier = Modifier.weight(1f).fillMaxHeight()) {
                            if (c in row.crossingCols) {
                                SustainConnectorCanvas(laneIndex = c - 4)
                            }
                        }
                    }
                }
            }
        }

        Row(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                Text(
                    text = "Sección ${sec.index + 1} · ${String.format(Locale.US, "%.2fs", sec.startMs / 1000.0)} · ${sec.bpm.roundToInt()} BPM",
                    color = StudioColors.TextPrimary,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Bold
                )
                if (row.sectionCueCount > 0) {
                    Box(
                        modifier = Modifier
                            .clip(RoundedCornerShape(4.dp))
                            .background(StudioColors.AccentCyan.copy(alpha = 0.2f))
                            .padding(horizontal = 5.dp, vertical = 1.dp)
                    ) {
                        Text(
                            text = "${row.sectionCueCount} ev.",
                            color = StudioColors.AccentCyan,
                            fontSize = 9.sp,
                            fontWeight = FontWeight.Bold
                        )
                    }
                }
            }

            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .clip(RoundedCornerShape(6.dp))
                    .background(StudioColors.BgCard.copy(alpha = 0.92f))
                    .clickable(onClick = onToggleSectionMustHit)
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
}

@Composable
private fun ChartStepTimelineRow(
    row: ChartTimelineRow.StepRow,
    selectedSustainSteps: Int,
    onSelectSection: (Int) -> Unit,
    onToggleChartNote: (Int, Int, Int, Int) -> Unit,
    onOpenStepCueDialog: (ChartTimelineRow.StepRow) -> Unit,
    onSeekToStep: () -> Unit
) {
    val isBeat = row.stepInSec % 4 == 0
    val isMeasureStart = row.stepInSec == 0
    val rowBg = when {
        isMeasureStart -> StudioColors.BgElevated.copy(alpha = 0.45f)
        isBeat -> StudioColors.BgPanel.copy(alpha = 0.80f)
        else -> StudioColors.BgDark
    }
    val dividerColor = StudioColors.BorderSubtle.copy(alpha = if (isBeat) 0.50f else 0.18f)

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .height(34.dp)
            .background(rowBg)
            .drawBehind {
                drawLine(
                    color = dividerColor,
                    start = Offset(0f, 0f),
                    end = Offset(size.width, 0f),
                    strokeWidth = if (isBeat) 1.5f else 0.8f
                )
            }
            .padding(horizontal = 8.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        // Columna # interactiva para eventos (tap) o saltar hilo aquí (pulsación larga)
        StepNumberCueCell(
            row = row,
            isBeat = isBeat,
            onClick = {
                onSelectSection(row.section.index)
                onOpenStepCueDialog(row)
            },
            onLongClick = {
                onSelectSection(row.section.index)
                onSeekToStep()
            }
        )

        // 4 columnas Oponente (0..3)
        Row(
            modifier = Modifier
                .weight(1f)
                .fillMaxHeight(),
            horizontalArrangement = Arrangement.spacedBy(3.dp)
        ) {
            for (c in 0..3) {
                ChartNoteCell(
                    note = row.rowNotesByCol[c],
                    sustainPass = row.sustainPassesByCol[c],
                    laneIndex = c,
                    stepMs = row.stepMs,
                    isBeat = isBeat,
                    onClick = {
                        onSelectSection(row.section.index)
                        onToggleChartNote(row.section.index, row.stepInSec, c, selectedSustainSteps)
                    },
                    modifier = Modifier.weight(1f)
                )
            }
        }

        Spacer(Modifier.width(8.dp))

        // 4 columnas Jugador (4..7)
        Row(
            modifier = Modifier
                .weight(1f)
                .fillMaxHeight(),
            horizontalArrangement = Arrangement.spacedBy(3.dp)
        ) {
            for (c in 4..7) {
                ChartNoteCell(
                    note = row.rowNotesByCol[c],
                    sustainPass = row.sustainPassesByCol[c],
                    laneIndex = c - 4,
                    stepMs = row.stepMs,
                    isBeat = isBeat,
                    onClick = {
                        onSelectSection(row.section.index)
                        onToggleChartNote(row.section.index, row.stepInSec, c, selectedSustainSteps)
                    },
                    modifier = Modifier.weight(1f)
                )
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun StepNumberCueCell(
    row: ChartTimelineRow.StepRow,
    isBeat: Boolean,
    onClick: () -> Unit,
    onLongClick: () -> Unit
) {
    val hasEvents = row.rowEvents.isNotEmpty()
    val flashColor = row.activeFlashColor
    val badgeKinds = row.rowEvents.map { classifyChartEvent(it.event) }.distinct()

    Box(
        modifier = Modifier
            .width(52.dp)
            .fillMaxHeight()
            .padding(vertical = 2.dp, horizontal = 1.dp)
            .clip(RoundedCornerShape(5.dp))
            .background(
                when {
                    hasEvents -> StudioColors.AccentCyan.copy(alpha = 0.16f)
                    flashColor != null -> flashColor.copy(alpha = 0.10f)
                    else -> Color.Transparent
                }
            )
            .border(
                width = if (hasEvents) 1.dp else 0.dp,
                color = if (hasEvents) StudioColors.AccentCyan.copy(alpha = 0.7f) else Color.Transparent,
                shape = RoundedCornerShape(5.dp)
            )
            .combinedClickable(
                onClick = onClick,
                onLongClick = onLongClick
            )
    ) {
        if (flashColor != null) {
            Box(
                modifier = Modifier
                    .align(Alignment.CenterStart)
                    .width(3.dp)
                    .fillMaxHeight()
                    .background(flashColor.copy(alpha = 0.85f))
            )
        }

        Row(
            modifier = Modifier
                .fillMaxSize()
                .padding(horizontal = 5.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween
        ) {
            Text(
                text = "${row.stepInSec}",
                color = when {
                    hasEvents -> StudioColors.AccentCyan
                    isBeat -> StudioColors.AccentAmber
                    else -> StudioColors.TextMuted.copy(alpha = 0.7f)
                },
                fontSize = 10.sp,
                fontWeight = if (isBeat || hasEvents) FontWeight.Bold else FontWeight.Medium
            )

            if (hasEvents) {
                Row(horizontalArrangement = Arrangement.spacedBy(2.dp), verticalAlignment = Alignment.CenterVertically) {
                    badgeKinds.take(3).forEach { kind ->
                        val dotColor = when (kind) {
                            StepCueKind.Flash -> StudioColors.AccentAmber
                            StepCueKind.Swap -> StudioColors.AccentEmerald
                            StepCueKind.Character -> StudioColors.AccentPurple
                            StepCueKind.Preload -> Color(0xFF06B6D4)
                            StepCueKind.UiBlackout -> StudioColors.AccentCoral
                            StepCueKind.Other -> StudioColors.TextMuted
                        }
                        Box(
                            modifier = Modifier
                                .size(6.dp)
                                .clip(CircleShape)
                                .background(dotColor)
                        )
                    }
                }
            } else if (isBeat) {
                Icon(
                    imageVector = Icons.Rounded.AutoAwesome,
                    contentDescription = null,
                    tint = StudioColors.TextMuted.copy(alpha = 0.25f),
                    modifier = Modifier.size(9.dp)
                )
            }
        }
    }
}

@Composable
private fun SustainConnectorCanvas(laneIndex: Int) {
    val laneColor = StudioColors.LaneColors[laneIndex]
    Canvas(modifier = Modifier.fillMaxSize()) {
        val beamW = (size.width * 0.34f).coerceAtLeast(8f)
        val coreW = (size.width * 0.13f).coerceAtLeast(3f)
        drawRect(
            color = laneColor.copy(alpha = 0.38f),
            topLeft = Offset((size.width - beamW) / 2f, 0f),
            size = Size(beamW, size.height)
        )
        drawRect(
            color = Color.White.copy(alpha = 0.24f),
            topLeft = Offset((size.width - coreW) / 2f, 0f),
            size = Size(coreW, size.height)
        )
    }
}

@Composable
private fun ChartNoteCell(
    note: VisibleNote?,
    sustainPass: ChartSustainPass?,
    laneIndex: Int,
    stepMs: Double,
    isBeat: Boolean,
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
    val segKind = sustainPass?.kind ?: SustainSegmentKind.None

    Box(
        modifier = modifier
            .fillMaxHeight()
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        // Capa 1: Estela vertical continua del sustain (Head -> Body -> Tail redondeado)
        Canvas(modifier = Modifier.fillMaxSize()) {
            val padX = 1.dp.toPx()
            val padY = 3.dp.toPx()
            val slotW = (size.width - padX * 2f).coerceAtLeast(4f)
            val slotH = (size.height - padY * 2f).coerceAtLeast(4f)

            if (!hasNote) {
                drawRoundRect(
                    color = StudioColors.BgCard.copy(alpha = if (isBeat) 0.72f else 0.42f),
                    topLeft = Offset(padX, padY),
                    size = Size(slotW, slotH),
                    cornerRadius = CornerRadius(5.dp.toPx(), 5.dp.toPx())
                )
            }

            if (segKind != SustainSegmentKind.None) {
                val beamW = (size.width * 0.36f).coerceAtLeast(9f)
                val coreW = (size.width * 0.14f).coerceAtLeast(3.5f)
                val beamLeft = (size.width - beamW) / 2f
                val coreLeft = (size.width - coreW) / 2f

                when (segKind) {
                    SustainSegmentKind.Head -> {
                        val startY = size.height * 0.5f
                        drawRect(
                            color = laneColor.copy(alpha = 0.76f),
                            topLeft = Offset(beamLeft, startY),
                            size = Size(beamW, size.height - startY)
                        )
                        drawRect(
                            color = Color.White.copy(alpha = 0.45f),
                            topLeft = Offset(coreLeft, startY),
                            size = Size(coreW, size.height - startY)
                        )
                    }
                    SustainSegmentKind.Body -> {
                        drawRect(
                            color = laneColor.copy(alpha = 0.74f),
                            topLeft = Offset(beamLeft, 0f),
                            size = Size(beamW, size.height)
                        )
                        drawRect(
                            color = Color.White.copy(alpha = 0.42f),
                            topLeft = Offset(coreLeft, 0f),
                            size = Size(coreW, size.height)
                        )
                    }
                    SustainSegmentKind.Tail -> {
                        val endY = size.height * 0.74f
                        val capR = beamW * 0.5f
                        drawRoundRect(
                            color = laneColor.copy(alpha = 0.80f),
                            topLeft = Offset(beamLeft, -capR),
                            size = Size(beamW, endY + capR),
                            cornerRadius = CornerRadius(capR, capR)
                        )
                        val coreCapR = coreW * 0.5f
                        drawRoundRect(
                            color = Color.White.copy(alpha = 0.48f),
                            topLeft = Offset(coreLeft, -coreCapR),
                            size = Size(coreW, (endY - 2f).coerceAtLeast(4f) + coreCapR),
                            cornerRadius = CornerRadius(coreCapR, coreCapR)
                        )
                    }
                    SustainSegmentKind.None -> {}
                }
            }
        }

        // Capa 2: Cabeza de nota (flecha o nota especial X / Esquive / Alt / Sin Anim)
        if (hasNote) {
            val isHurt = note!!.noteKindCode == 1 || note.noteType.equals("hurt", ignoreCase = true)
            val isInstakill = note.noteKindCode == 2 || note.noteType.equals("instakill", ignoreCase = true)
            val isDodge = note.noteKindCode == 3 || note.noteType.equals("dodge", ignoreCase = true)
            val isAlt = note.noteKindCode == 4 || note.noteType.equals("alt", ignoreCase = true)
            val isNoAnim = note.noteKindCode == 5 || note.noteType.equals("no_anim", ignoreCase = true)

            val noteBgColor = when {
                isInstakill -> Color(0xFFB71C1C)
                isHurt -> StudioColors.AccentCoral
                isDodge -> StudioColors.AccentAmber
                isAlt -> StudioColors.AccentPurple
                isNoAnim -> StudioColors.BgElevated
                else -> laneColor.copy(alpha = 0.94f)
            }
            val noteBorderColor = when {
                isInstakill -> Color(0xFFFF8A80)
                isHurt -> Color.White
                isDodge -> Color(0xFFFFF59D)
                else -> Color.White.copy(alpha = 0.75f)
            }
            val symbolText = when {
                isInstakill -> "X!"
                isHurt -> "X"
                isDodge -> "⚡"
                isAlt -> "${LANE_ARROWS[laneIndex]}A"
                isNoAnim -> "${LANE_ARROWS[laneIndex]}∅"
                else -> LANE_ARROWS[laneIndex]
            }

            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(28.dp)
                    .padding(horizontal = 1.dp)
                    .clip(RoundedCornerShape(6.dp))
                    .background(noteBgColor)
                    .border(if (isHurt || isInstakill) 1.5.dp else 1.dp, noteBorderColor, RoundedCornerShape(6.dp)),
                contentAlignment = Alignment.Center
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(2.dp)
                ) {
                    Text(
                        text = symbolText,
                        color = if (isDodge) Color(0xFF1F1200) else Color.White,
                        fontSize = 11.5.sp,
                        fontWeight = FontWeight.ExtraBold,
                        maxLines = 1,
                        softWrap = false
                    )
                    if (sustainSteps > 0) {
                        Text(
                            text = "${sustainSteps}p",
                            color = (if (isDodge) Color(0xFF1F1200) else Color.White).copy(alpha = 0.90f),
                            fontSize = 9.sp,
                            fontWeight = FontWeight.Bold,
                            maxLines = 1,
                            softWrap = false
                        )
                    }
                }
            }
        }
    }
}
