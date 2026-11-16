// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/theme/StudioTheme.kt
// Propósito: Paleta oscura de estudio de producción (Material 3 Dark) para BeatForge Studio.
// Define colores de alto contraste pensados para pantallas horizontales de teléfono,
// preservando legibilidad en editores de sprites, líneas de tiempo, scripts y gameplay.

package com.beatforge.studio.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

object StudioColors {
    val BgDark = Color(0xFF0B0F17)
    val BgPanel = Color(0xFF131A26)
    val BgCard = Color(0xFF1A2334)
    val BgElevated = Color(0xFF222D42)
    val BorderSubtle = Color(0xFF2D3A52)

    val TextPrimary = Color(0xFFF1F5F9)
    val TextMuted = Color(0xFF94A3B8)

    val AccentCyan = Color(0xFF22D3EE)
    val AccentEmerald = Color(0xFF34D399)
    val AccentIndigo = Color(0xFF818CF8)
    val AccentAmber = Color(0xFFFBBF24)
    val AccentCoral = Color(0xFFFB7185)
    val AccentPurple = Color(0xFFC084FC)

    // Colores clásicos de los 4 carriles rítmicos (Izq, Abajo, Arriba, Der)
    val LaneColors = listOf(
        Color(0xFFC084FC),
        Color(0xFF22D3EE),
        Color(0xFF34D399),
        Color(0xFFFB7185)
    )
}

private val BeatForgeDarkScheme = darkColorScheme(
    primary = StudioColors.AccentCyan,
    onPrimary = Color(0xFF041C24),
    primaryContainer = Color(0xFF123847),
    onPrimaryContainer = StudioColors.AccentCyan,
    secondary = StudioColors.AccentEmerald,
    onSecondary = Color(0xFF042419),
    secondaryContainer = Color(0xFF133A2E),
    onSecondaryContainer = StudioColors.AccentEmerald,
    tertiary = StudioColors.AccentIndigo,
    onTertiary = Color(0xFF14163D),
    background = StudioColors.BgDark,
    onBackground = StudioColors.TextPrimary,
    surface = StudioColors.BgPanel,
    onSurface = StudioColors.TextPrimary,
    surfaceVariant = StudioColors.BgCard,
    onSurfaceVariant = StudioColors.TextMuted,
    outline = StudioColors.BorderSubtle,
    error = StudioColors.AccentCoral,
    onError = Color(0xFF2B0710)
)

private val BeatForgeTypography = Typography(
    titleLarge = TextStyle(
        fontSize = 20.sp,
        fontWeight = FontWeight.Bold,
        color = StudioColors.TextPrimary
    ),
    titleMedium = TextStyle(
        fontSize = 15.sp,
        fontWeight = FontWeight.SemiBold,
        color = StudioColors.TextPrimary
    ),
    bodyMedium = TextStyle(
        fontSize = 13.sp,
        fontWeight = FontWeight.Normal,
        color = StudioColors.TextPrimary
    ),
    labelMedium = TextStyle(
        fontSize = 12.sp,
        fontWeight = FontWeight.Medium,
        color = StudioColors.TextMuted
    )
)

@Composable
fun BeatForgeTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = BeatForgeDarkScheme,
        typography = BeatForgeTypography,
        content = content
    )
}
