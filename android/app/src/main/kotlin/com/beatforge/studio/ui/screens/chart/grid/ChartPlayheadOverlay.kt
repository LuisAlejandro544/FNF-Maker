// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/chart/grid/ChartPlayheadOverlay.kt
// Propósito: Capa Canvas a 60 Hz que dibuja el hilo horizontal luminoso del reproductor,
// el resalte de fila activa y el puntero lateral junto a la columna `#`.

package com.beatforge.studio.ui.screens.chart.grid

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.unit.dp
import com.beatforge.studio.ui.screens.chart.ChartPlayheadState
import com.beatforge.studio.ui.theme.StudioColors

@Composable
internal fun ChartPlayheadThreadOverlay(
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
