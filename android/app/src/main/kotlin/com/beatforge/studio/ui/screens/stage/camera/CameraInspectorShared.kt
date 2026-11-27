// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/camera/CameraInspectorShared.kt
// Propósito: Constantes, utilidades de color/etiquetas y chips reutilizables para
// las pestañas del inspector multi-cámara (`StageCameraInspector`).

package com.beatforge.studio.ui.screens.stage.camera

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StageCameraSlotSnapshot
import com.beatforge.studio.ui.theme.StudioColors

internal val CAMERA_TARGET_OPTIONS = listOf(
    "fixed" to "Fija (X,Y)",
    "player" to "Player",
    "opponent" to "Enemigo",
    "gf" to "Cabina/GF",
    "entity" to "Entidad"
)

internal val VIGNETTE_PRESET_COLORS = listOf(
    "#EF4444" to "Rojo",
    "#DC2626" to "Carmesí",
    "#3B82F6" to "Azul",
    "#06B6D4" to "Cian",
    "#A855F7" to "Morado",
    "#F59E0B" to "Ámbar",
    "#10B981" to "Verde",
    "#09090B" to "Negro"
)

internal fun parseInspectorHexColor(raw: String): Color {
    val clean = raw.trim().removePrefix("#")
    return try {
        when (clean.length) {
            6 -> Color((0xFF000000L or clean.toLong(16)).toInt())
            8 -> Color(clean.toLong(16).toInt())
            else -> StudioColors.AccentCoral
        }
    } catch (_: Throwable) {
        StudioColors.AccentCoral
    }
}

internal fun cameraSlotAccentColor(slot: StageCameraSlotSnapshot): Color = when (slot.targetKind) {
    "player" -> StudioColors.AccentEmerald
    "opponent" -> StudioColors.AccentCoral
    "gf" -> StudioColors.AccentAmber
    "entity" -> StudioColors.AccentPurple
    else -> StudioColors.AccentCyan
}

internal fun cameraSlotRoleShortTag(slot: StageCameraSlotSnapshot): String = when (slot.targetKind) {
    "player" -> "Jug"
    "opponent" -> "Riv"
    "gf" -> "GF"
    "entity" -> "Ent"
    else -> "Fija"
}

@Composable
internal fun CameraTabChip(
    label: String,
    icon: ImageVector,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier
            .height(29.dp)
            .clip(RoundedCornerShape(7.dp))
            .background(if (selected) StudioColors.AccentCyan.copy(alpha = 0.22f) else StudioColors.BgCard)
            .border(
                1.dp,
                if (selected) StudioColors.AccentCyan else StudioColors.BorderSubtle,
                RoundedCornerShape(7.dp)
            )
            .clickable(onClick = onClick),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = if (selected) StudioColors.AccentCyan else StudioColors.TextMuted,
            modifier = Modifier.size(13.dp)
        )
        Spacer(Modifier.width(4.dp))
        Text(
            text = label,
            color = if (selected) StudioColors.AccentCyan else StudioColors.TextPrimary,
            fontSize = 11.sp,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
            maxLines = 1,
            softWrap = false
        )
    }
}

@Composable
internal fun CameraStepChip(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    Box(
        modifier = modifier
            .height(27.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(StudioColors.BgCard)
            .border(1.dp, StudioColors.BorderSubtle, RoundedCornerShape(6.dp))
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = label,
            color = StudioColors.TextPrimary,
            fontSize = 10.5.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            softWrap = false
        )
    }
}
