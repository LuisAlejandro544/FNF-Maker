// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/gameplay/ArrowPaletteExtractor.kt
// Propósito: Extrae automáticamente la paleta dominante de cada flecha cargada (`arrowBitmaps`)
// —incluyendo skins personalizadas `.svg` o atlas `NOTE_assets.png + .xml`— para que las notas
// largas (colas sustain), los receptores activos y los destellos `Note Splash` coincidan con
// el color real del diseño (incluyendo tonos grises/monocromáticos en estilos blanco y negro).

package com.beatforge.studio.ui.screens.gameplay

import android.graphics.Bitmap
import android.graphics.Color as AndroidColor
import android.os.Build
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asAndroidBitmap
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

data class ArrowLanePalette(
    val bodyColor: Color,
    val coreColor: Color,
    val splashColor: Color
)

/**
 * Extrae una paleta coherente (`bodyColor`, `coreColor`, `splashColor`) analizando los píxeles
 * visibles del bitmap de flecha. Si la flecha es monocromática / escala de grises (por ejemplo,
 * estilo boceto o blanco y negro), devuelve tonos grises limpios en lugar de los colores neón
 * por defecto.
 */
fun extractArrowLanePalette(
    bitmap: ImageBitmap?,
    fallbackColor: Color
): ArrowLanePalette {
    val defaultPalette = ArrowLanePalette(
        bodyColor = fallbackColor,
        coreColor = blendTowardWhite(fallbackColor, 0.65f),
        splashColor = fallbackColor
    )
    if (bitmap == null) return defaultPalette

    val ab = try {
        bitmap.asAndroidBitmap()
    } catch (_: Throwable) {
        return defaultPalette
    }
    if (ab.isRecycled || ab.width <= 0 || ab.height <= 0) {
        return defaultPalette
    }
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && ab.config == Bitmap.Config.HARDWARE) {
        return defaultPalette
    }

    val w = ab.width
    val h = ab.height
    val stepX = max(1, w / 36)
    val stepY = max(1, h / 36)

    var opaqueCount = 0
    var chromaticCount = 0
    var chromaticWeightSum = 0.0
    var chromR = 0.0
    var chromG = 0.0
    var chromB = 0.0

    var midNeutralCount = 0
    var midNeutralLumaSum = 0.0
    var allNeutralLumaSum = 0.0

    var y = 0
    while (y < h) {
        var x = 0
        while (x < w) {
            val px = ab.getPixel(x, y)
            val alpha = AndroidColor.alpha(px)
            if (alpha >= 96) {
                opaqueCount += 1
                val r = AndroidColor.red(px)
                val g = AndroidColor.green(px)
                val b = AndroidColor.blue(px)
                val maxC = max(r, max(g, b))
                val minC = min(r, min(g, b))
                val chroma = maxC - minC
                val luma = 0.299 * r + 0.587 * g + 0.114 * b

                allNeutralLumaSum += luma
                if (luma in 42.0..226.0) {
                    midNeutralCount += 1
                    midNeutralLumaSum += luma
                }

                if (chroma >= 34 && maxC >= 42) {
                    chromaticCount += 1
                    val weight = 1.0 + (chroma / 128.0)
                    chromaticWeightSum += weight
                    chromR += r * weight
                    chromG += g * weight
                    chromB += b * weight
                }
            }
            x += stepX
        }
        y += stepY
    }

    if (opaqueCount == 0) return defaultPalette

    val minChromaticSamples = max(3, (opaqueCount * 0.08f).roundToInt())
    if (chromaticCount >= minChromaticSamples && chromaticWeightSum > 0.0) {
        val r = (chromR / chromaticWeightSum).roundToInt().coerceIn(0, 255)
        val g = (chromG / chromaticWeightSum).roundToInt().coerceIn(0, 255)
        val b = (chromB / chromaticWeightSum).roundToInt().coerceIn(0, 255)
        val body = Color(r, g, b, 255)
        return ArrowLanePalette(
            bodyColor = body,
            coreColor = blendTowardWhite(body, 0.62f),
            splashColor = body
        )
    }

    // Skin monocromática / boceto en escala de grises: nunca filtrar colores neón por defecto.
    val rawLuma = if (midNeutralCount > 0) {
        midNeutralLumaSum / midNeutralCount
    } else {
        allNeutralLumaSum / opaqueCount
    }
    val bodyGray = rawLuma.roundToInt().coerceIn(108, 172)
    val coreGray = (bodyGray + 84).coerceIn(220, 245)
    val splashGray = (bodyGray + 54).coerceIn(180, 228)

    return ArrowLanePalette(
        bodyColor = Color(bodyGray, bodyGray, bodyGray, 255),
        coreColor = Color(coreGray, coreGray, coreGray, 255),
        splashColor = Color(splashGray, splashGray, splashGray, 255)
    )
}

/**
 * Calcula y agrupa las paletas de los 4 carriles (0=Izquierda, 1=Abajo, 2=Arriba, 3=Derecha).
 */
fun extractAllArrowLanePalettes(arrowBitmaps: Map<Int, ImageBitmap>): List<ArrowLanePalette> {
    return (0..3).map { dir ->
        extractArrowLanePalette(
            bitmap = arrowBitmaps[dir],
            fallbackColor = StudioColors.LaneColors[dir]
        )
    }
}

private fun blendTowardWhite(color: Color, amount: Float): Color {
    val t = amount.coerceIn(0f, 1f)
    return Color(
        red = color.red + (1f - color.red) * t,
        green = color.green + (1f - color.green) * t,
        blue = color.blue + (1f - color.blue) * t,
        alpha = 1f
    )
}
