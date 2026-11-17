// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/chart/ChartPlayheadState.kt
// Propósito: Estado `@Stable` desacoplado a 60 Hz para el hilo reproductor del Editor de Chart.
// Mantiene la posición temporal en ms, la duración total y el estado de seguimiento automático
// sin recomponer todas las filas de `ChartTimelineGrid` mientras baja el hilo.

package com.beatforge.studio.ui.screens.chart

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import java.util.Locale

@Stable
class ChartPlayheadState(
    initialDurationMs: Double = 0.0
) {
    var posMs by mutableDoubleStateOf(0.0)
    var durationMs by mutableDoubleStateOf(initialDurationMs.coerceAtLeast(0.0))
    var followThread by mutableStateOf(true)
    var activeTarget by mutableStateOf<ChartPlayheadTarget?>(null)

    fun updatePosition(
        nextPosMs: Double,
        nextDurationMs: Double,
        stepLookup: List<ChartStepLookupEntry>
    ) {
        val safeDur = nextDurationMs.coerceAtLeast(0.0)
        durationMs = safeDur
        val clamped = if (safeDur > 0.0) nextPosMs.coerceIn(0.0, safeDur) else nextPosMs.coerceAtLeast(0.0)
        posMs = clamped
        activeTarget = resolveChartPlayheadTarget(stepLookup, clamped)
    }
}

fun formatChartTimeMs(ms: Double): String {
    val safeMs = ms.coerceAtLeast(0.0)
    val totalSec = (safeMs / 1000.0).toInt()
    val min = totalSec / 60
    val sec = totalSec % 60
    val centi = ((safeMs % 1000.0) / 10.0).toInt().coerceIn(0, 99)
    return String.format(Locale.US, "%02d:%02d.%02d", min, sec, centi)
}
