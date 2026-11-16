// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/theme/StudioTheme.kt
// Propósito: Paleta gráfica de estudio hardware (carbón mate, grafito cálido, naranja estudio,
// ámbar y verde limpio) diseñada para evitar el aspecto genérico azul-neón y ofrecer máxima
// legibilidad en pantallas horizontales de teléfonos.

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
    val BgDark = Color(0xFF101012)
    val BgPanel = Color(0xFF18181B)
    val BgCard = Color(0xFF212126)
    val BgElevated = Color(0xFF2B2B32)
    val BorderSubtle = Color(0xFF3A3A44)

    val TextPrimary = Color(0xFFF4F4F5)
    val TextMuted = Color(0xFFA1A1AA)

    // Acentos cálidos y sobrios estilo estudio musical / consola DAW
    val AccentCyan = Color(0xFFF97316)     // Acento principal: Naranja Estudio
    val AccentEmerald = Color(0xFF10B981)  // Verde acción / Jugar / Válido
    val AccentIndigo = Color(0xFFF59E0B)   // Ámbar dorado / Capas / Selección
    val AccentAmber = Color(0xFFEAB308)    // Oro cálido / BPM / Sprites
    val AccentCoral = Color(0xFFEF4444)    // Rojo limpio / Error / Oponente
    val AccentPurple = Color(0xFFEC4899)   // Magenta estudio / Skins / Scripts

    // Colores clásicos de las 4 flechas rítmicas (Izq, Abajo, Arriba, Der)
    val LaneColors = listOf(
        Color(0xFFD946EF),
        Color(0xFF06B6D4),
        Color(0xFF22C55E),
        Color(0xFFF43F5E)
    )
}

private val BeatForgeDarkScheme = darkColorScheme(
    primary = StudioColors.AccentCyan,
    onPrimary = Color(0xFF1F0C00),
    primaryContainer = Color(0xFF3D1D08),
    onPrimaryContainer = Color(0xFFFDBA74),
    secondary = StudioColors.AccentEmerald,
    onSecondary = Color(0xFF032116),
    secondaryContainer = Color(0xFF0F3628),
    onSecondaryContainer = Color(0xFF6EE7B7),
    tertiary = StudioColors.AccentAmber,
    onTertiary = Color(0xFF231800),
    background = StudioColors.BgDark,
    onBackground = StudioColors.TextPrimary,
    surface = StudioColors.BgPanel,
    onSurface = StudioColors.TextPrimary,
    surfaceVariant = StudioColors.BgCard,
    onSurfaceVariant = StudioColors.TextMuted,
    outline = StudioColors.BorderSubtle,
    error = StudioColors.AccentCoral,
    onError = Color(0xFF290505)
)

private val BeatForgeTypography = Typography(
    titleLarge = TextStyle(
        fontSize = 19.sp,
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
