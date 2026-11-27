// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/chart/grid/ChartStepRow.kt
// Propósito: Fila de paso de 34.dp en la rejilla del chart, con la celda interactiva `#`
// (cues de Flash/Swap/Blackout/Cámara/Viñeta o salto de hilo en pulsación larga) y un único
// `Canvas` de 8 carriles con caché de glifos para scroll fluido sin recomposiciones.

package com.beatforge.studio.ui.screens.chart.grid

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.ui.screens.chart.ChartTimelineRow
import com.beatforge.studio.ui.screens.chart.StepCueKind
import com.beatforge.studio.ui.screens.chart.SustainSegmentKind
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

internal val LANE_ARROWS = listOf("←", "↓", "↑", "→")

@Composable
internal fun ChartStepTimelineRow(
    row: ChartTimelineRow.StepRow,
    selectedSustainSteps: Int,
    textMeasurer: TextMeasurer,
    textCache: MutableMap<String, TextLayoutResult>,
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

        // Los 8 carriles (Oponente 0..3 + separador 8dp + Jugador 4..7) se dibujan en un solo Canvas
        // para reducir de ~36 nodos de layout por fila a 1 Canvas sin recomposiciones al hacer scroll.
        ChartStepLanesCanvas(
            row = row,
            isBeat = isBeat,
            selectedSustainSteps = selectedSustainSteps,
            textMeasurer = textMeasurer,
            textCache = textCache,
            onTapCol = { col ->
                onSelectSection(row.section.index)
                onToggleChartNote(row.section.index, row.stepInSec, col, selectedSustainSteps)
            },
            modifier = Modifier
                .weight(1f)
                .fillMaxHeight()
        )
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
    val blackoutActive = row.activeBlackoutAlpha > 0.04f
    val vignetteColor = row.activeVignetteColor
    val badgeKinds = row.eventKinds

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
                    blackoutActive -> Color.Black.copy(alpha = 0.42f * row.activeBlackoutAlpha.coerceIn(0.25f, 1f))
                    vignetteColor != null -> vignetteColor.copy(alpha = 0.08f)
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
        } else if (blackoutActive) {
            Box(
                modifier = Modifier
                    .align(Alignment.CenterStart)
                    .width(3.dp)
                    .fillMaxHeight()
                    .background(StudioColors.AccentCoral.copy(alpha = 0.72f))
            )
        }
        if (vignetteColor != null) {
            Box(
                modifier = Modifier
                    .align(Alignment.CenterEnd)
                    .width(2.5.dp)
                    .fillMaxHeight()
                    .background(vignetteColor.copy(alpha = 0.85f))
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
                            StepCueKind.Camera -> StudioColors.AccentCyan
                            StepCueKind.Vignette -> Color(0xFFF43F5E)
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

private fun getOrMeasureCachedLaneText(
    textMeasurer: TextMeasurer,
    textCache: MutableMap<String, TextLayoutResult>,
    text: String,
    color: Color,
    fontSizeSp: Float,
    fontWeight: FontWeight
): TextLayoutResult {
    val key = "$text|${color.toArgb()}|$fontSizeSp|${fontWeight.weight}"
    textCache[key]?.let { return it }
    val measured = textMeasurer.measure(
        text = AnnotatedString(text),
        style = TextStyle(
            color = color,
            fontSize = fontSizeSp.sp,
            fontWeight = fontWeight
        ),
        maxLines = 1,
        softWrap = false
    )
    if (textCache.size < 128) {
        textCache[key] = measured
    }
    return measured
}

/**
 * Dibuja los 8 carriles de una fila de pasos dentro de un solo `Canvas` con cache de glifos
 * y detecta taps por coordenada X sin crear 8 `Box` + 8 `Canvas` + 16 `Text` por fila.
 */
@Composable
private fun ChartStepLanesCanvas(
    row: ChartTimelineRow.StepRow,
    isBeat: Boolean,
    selectedSustainSteps: Int,
    textMeasurer: TextMeasurer,
    textCache: MutableMap<String, TextLayoutResult>,
    onTapCol: (Int) -> Unit,
    modifier: Modifier = Modifier
) {
    val currentOnTapCol by rememberUpdatedState(onTapCol)

    Canvas(
        modifier = modifier.pointerInput(row.globalStepIndex, row.section.index, selectedSustainSteps) {
            detectTapGestures { offset ->
                val w = size.width.toFloat()
                if (w <= 0f) return@detectTapGestures
                val gapPx = 8.dp.toPx()
                val laneGapPx = 3.dp.toPx()
                val sideW = (w - gapPx).coerceAtLeast(16f) / 2f
                val laneStride = ((sideW - laneGapPx * 3f).coerceAtLeast(4f) / 4f) + laneGapPx
                val x = offset.x
                val col = when {
                    x < sideW -> (x / laneStride).toInt().coerceIn(0, 3)
                    x >= sideW + gapPx -> 4 + ((x - sideW - gapPx) / laneStride).toInt().coerceIn(0, 3)
                    else -> return@detectTapGestures
                }
                currentOnTapCol(col)
            }
        }
    ) {
        val w = size.width
        val h = size.height
        val gapPx = 8.dp.toPx()
        val laneGapPx = 3.dp.toPx()
        val sideW = (w - gapPx).coerceAtLeast(16f) / 2f
        val laneW = (sideW - laneGapPx * 3f).coerceAtLeast(4f) / 4f
        val padX = 1.dp.toPx()
        val padY = 3.dp.toPx()
        val slotW = (laneW - padX * 2f).coerceAtLeast(4f)
        val slotH = (h - padY * 2f).coerceAtLeast(4f)
        val slotCorner = CornerRadius(5.dp.toPx(), 5.dp.toPx())
        val slotColor = StudioColors.BgCard.copy(alpha = if (isBeat) 0.72f else 0.42f)

        val beamW = (laneW * 0.36f).coerceAtLeast(9f)
        val coreW = (laneW * 0.14f).coerceAtLeast(3.5f)
        val noteH = 28.dp.toPx().coerceAtMost((h - 2f).coerceAtLeast(8f))
        val noteTop = (h - noteH) * 0.5f
        val noteCorner = CornerRadius(6.dp.toPx(), 6.dp.toPx())

        for (c in 0..7) {
            val isPlayer = c >= 4
            val laneIndex = if (isPlayer) c - 4 else c
            val sideOriginX = if (isPlayer) sideW + gapPx else 0f
            val laneX = sideOriginX + laneIndex * (laneW + laneGapPx)
            val laneColor = StudioColors.LaneColors[laneIndex]
            val note = row.rowNotesByCol[c]
            val segKind = row.sustainPassesByCol[c]?.kind ?: SustainSegmentKind.None

            if (note == null) {
                drawRoundRect(
                    color = slotColor,
                    topLeft = Offset(laneX + padX, padY),
                    size = Size(slotW, slotH),
                    cornerRadius = slotCorner
                )
            }

            if (segKind != SustainSegmentKind.None) {
                val beamLeft = laneX + (laneW - beamW) / 2f
                val coreLeft = laneX + (laneW - coreW) / 2f
                when (segKind) {
                    SustainSegmentKind.Head -> {
                        val startY = h * 0.5f
                        drawRect(
                            color = laneColor.copy(alpha = 0.76f),
                            topLeft = Offset(beamLeft, startY),
                            size = Size(beamW, h - startY)
                        )
                        drawRect(
                            color = Color.White.copy(alpha = 0.45f),
                            topLeft = Offset(coreLeft, startY),
                            size = Size(coreW, h - startY)
                        )
                    }
                    SustainSegmentKind.Body -> {
                        drawRect(
                            color = laneColor.copy(alpha = 0.74f),
                            topLeft = Offset(beamLeft, 0f),
                            size = Size(beamW, h)
                        )
                        drawRect(
                            color = Color.White.copy(alpha = 0.42f),
                            topLeft = Offset(coreLeft, 0f),
                            size = Size(coreW, h)
                        )
                    }
                    SustainSegmentKind.Tail -> {
                        val endY = h * 0.74f
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

            if (note != null) {
                val isHurt = note.noteKindCode == 1 || note.noteType.equals("hurt", ignoreCase = true)
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
                val sustainSteps = if (note.sustainMs > row.stepMs * 0.4) {
                    (note.sustainMs / row.stepMs).roundToInt().coerceAtLeast(1)
                } else {
                    0
                }

                val noteLeft = laneX + padX
                drawRoundRect(
                    color = noteBgColor,
                    topLeft = Offset(noteLeft, noteTop),
                    size = Size(slotW, noteH),
                    cornerRadius = noteCorner
                )
                drawRoundRect(
                    color = noteBorderColor,
                    topLeft = Offset(noteLeft, noteTop),
                    size = Size(slotW, noteH),
                    cornerRadius = noteCorner,
                    style = Stroke(width = (if (isHurt || isInstakill) 1.5.dp else 1.dp).toPx())
                )

                val contentColor = if (isDodge) Color(0xFF1F1200) else Color.White
                val symLayout = getOrMeasureCachedLaneText(
                    textMeasurer = textMeasurer,
                    textCache = textCache,
                    text = symbolText,
                    color = contentColor,
                    fontSizeSp = 11.5f,
                    fontWeight = FontWeight.ExtraBold
                )
                if (sustainSteps > 0) {
                    val susLayout = getOrMeasureCachedLaneText(
                        textMeasurer = textMeasurer,
                        textCache = textCache,
                        text = "${sustainSteps}p",
                        color = contentColor.copy(alpha = 0.90f),
                        fontSizeSp = 9f,
                        fontWeight = FontWeight.Bold
                    )
                    val innerGap = 2.dp.toPx()
                    val totalTextW = symLayout.size.width + innerGap + susLayout.size.width
                    val startX = noteLeft + ((slotW - totalTextW) * 0.5f).coerceAtLeast(2f)
                    drawText(
                        textLayoutResult = symLayout,
                        topLeft = Offset(startX, noteTop + (noteH - symLayout.size.height) * 0.5f)
                    )
                    drawText(
                        textLayoutResult = susLayout,
                        topLeft = Offset(
                            startX + symLayout.size.width + innerGap,
                            noteTop + (noteH - susLayout.size.height) * 0.5f
                        )
                    )
                } else {
                    drawText(
                        textLayoutResult = symLayout,
                        topLeft = Offset(
                            noteLeft + (slotW - symLayout.size.width) * 0.5f,
                            noteTop + (noteH - symLayout.size.height) * 0.5f
                        )
                    )
                }
            }
        }
    }
}
