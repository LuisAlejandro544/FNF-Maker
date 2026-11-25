// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/chart_cue/CueSharedComponents.kt
// Propósito: Componentes visuales reutilizables dentro del modal de cues del chart:
// botones de pestaña (`CueTabButton`), chips de acción (`ActionChip`) y banners de estado.

package com.beatforge.studio.ui.popups.chart_cue

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.ui.theme.StudioColors

@Composable
internal fun CueTabButton(
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
internal fun ActionChip(
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
internal fun StageStateSummaryBanner(
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
internal fun StageStatusBadge(
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
