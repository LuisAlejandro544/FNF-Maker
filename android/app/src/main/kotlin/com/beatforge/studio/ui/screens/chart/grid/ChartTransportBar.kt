// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/chart/grid/ChartTransportBar.kt
// Propósito: Barra superior de transporte del hilo reproductor en la rejilla del chart
// (Reproducir/Pausar, reiniciar al inicio, toggle Seguir/Libre, scrubber temporal y contador).

package com.beatforge.studio.ui.screens.chart.grid

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.GpsFixed
import androidx.compose.material.icons.rounded.GpsOff
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.SkipPrevious
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.ui.screens.chart.ChartPlayheadState
import com.beatforge.studio.ui.screens.chart.formatChartTimeMs
import com.beatforge.studio.ui.theme.StudioColors

@Composable
internal fun ChartPlayheadTransportBar(
    audioPreviewPlaying: Boolean,
    playheadState: ChartPlayheadState,
    onPlayOrPause: () -> Unit,
    onRewindToStart: () -> Unit,
    onToggleFollow: () -> Unit,
    onScrubMs: (Double) -> Unit
) {
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

        ChartPlayheadFollowButton(
            playheadState = playheadState,
            onToggleFollow = onToggleFollow
        )

        ChartPlayheadScrubberSlider(
            playheadState = playheadState,
            audioPreviewPlaying = audioPreviewPlaying,
            onScrubMs = onScrubMs,
            modifier = Modifier
                .weight(1f)
                .height(24.dp)
        )

        ChartPlayheadTimeCounter(playheadState = playheadState)
    }
}

@Composable
private fun ChartPlayheadFollowButton(
    playheadState: ChartPlayheadState,
    onToggleFollow: () -> Unit
) {
    val following = playheadState.followThread
    Row(
        verticalAlignment = Alignment.CenterVertically,
        modifier = Modifier
            .clip(RoundedCornerShape(7.dp))
            .background(
                if (following) StudioColors.AccentCyan.copy(alpha = 0.16f)
                else StudioColors.BgCard
            )
            .border(
                1.dp,
                if (following) StudioColors.AccentCyan else StudioColors.BorderSubtle,
                RoundedCornerShape(7.dp)
            )
            .clickable(onClick = onToggleFollow)
            .padding(horizontal = 7.dp, vertical = 4.dp)
    ) {
        Icon(
            imageVector = if (following) Icons.Rounded.GpsFixed else Icons.Rounded.GpsOff,
            contentDescription = null,
            tint = if (following) StudioColors.AccentCyan else StudioColors.TextMuted,
            modifier = Modifier.size(12.dp)
        )
        Spacer(Modifier.width(3.dp))
        Text(
            text = if (following) "Seguir" else "Libre",
            color = if (following) StudioColors.AccentCyan else StudioColors.TextMuted,
            fontSize = 10.sp,
            fontWeight = FontWeight.Bold,
            maxLines = 1,
            softWrap = false
        )
    }
}

@Composable
private fun ChartPlayheadScrubberSlider(
    playheadState: ChartPlayheadState,
    audioPreviewPlaying: Boolean,
    onScrubMs: (Double) -> Unit,
    modifier: Modifier = Modifier
) {
    val dur = playheadState.durationMs.coerceAtLeast(1.0)
    val sliderPos = (playheadState.posMs / dur).toFloat().coerceIn(0f, 1f)
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
        modifier = modifier
    )
}

@Composable
private fun ChartPlayheadTimeCounter(
    playheadState: ChartPlayheadState
) {
    val stepInSec = playheadState.activeTarget?.stepRow?.stepInSec ?: 0
    Text(
        text = "${formatChartTimeMs(playheadState.posMs)} / #$stepInSec",
        color = StudioColors.TextPrimary,
        fontSize = 10.sp,
        fontWeight = FontWeight.SemiBold,
        maxLines = 1,
        softWrap = false
    )
}
