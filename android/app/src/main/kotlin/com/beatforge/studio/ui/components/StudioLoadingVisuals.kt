// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/components/StudioLoadingVisuals.kt
// Propósito: Componentes visuales animados y reutilizables para todas las pantallas de espera
// y preparación de BeatForge Studio (inicio del estudio, apertura de proyecto, preparación
// de partida y calibración de sonido), manteniendo la paleta cálida de consola hardware.

package com.beatforge.studio.ui.components

import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/**
 * Barras animadas de ecualizador rítmico estilo consola musical para acompañar pantallas de carga.
 */
@Composable
fun StudioEqualizerBars(
    modifier: Modifier = Modifier,
    barCount: Int = 5,
    primaryColor: Color = StudioColors.AccentCyan,
    secondaryColor: Color = StudioColors.AccentEmerald,
    maxHeight: Dp = 26.dp,
    barWidth: Dp = 5.dp,
    isActive: Boolean = true
) {
    val transition = rememberInfiniteTransition(label = "studioEqTransition")
    val phase by transition.animateFloat(
        initialValue = 0f,
        targetValue = (2.0 * PI).toFloat(),
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 1250, easing = LinearEasing),
            repeatMode = RepeatMode.Restart
        ),
        label = "studioEqPhase"
    )

    Row(
        modifier = modifier.height(maxHeight),
        horizontalArrangement = Arrangement.spacedBy(4.dp),
        verticalAlignment = Alignment.Bottom
    ) {
        for (i in 0 until barCount) {
            val wave = if (isActive) {
                0.30f + 0.70f * ((sin(phase + i * 0.85f) + 1f) * 0.5f)
            } else {
                0.35f
            }
            val barColor = if (i % 2 == 0) primaryColor else secondaryColor
            Box(
                modifier = Modifier
                    .width(barWidth)
                    .height(maxHeight * wave.coerceIn(0.22f, 1f))
                    .clip(RoundedCornerShape(50))
                    .background(
                        Brush.verticalGradient(
                            colors = listOf(barColor, barColor.copy(alpha = 0.55f))
                        )
                    )
            )
        }
    }
}

/**
 * Anillo orbital de progreso con halo sutil, arco de gradiente y punto luminoso animándose
 * tanto en modo determinado (`progress` 0..1) como indeterminado.
 */
@Composable
fun StudioOrbitProgressRing(
    progress: Float,
    modifier: Modifier = Modifier,
    indeterminate: Boolean = false,
    primaryColor: Color = StudioColors.AccentCyan,
    secondaryColor: Color = StudioColors.AccentEmerald,
    size: Dp = 72.dp,
    strokeWidth: Dp = 5.dp,
    centerContent: @Composable () -> Unit = {}
) {
    val transition = rememberInfiniteTransition(label = "studioOrbitRing")
    val spinDeg by transition.animateFloat(
        initialValue = 0f,
        targetValue = 360f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 1600, easing = LinearEasing),
            repeatMode = RepeatMode.Restart
        ),
        label = "orbitSpin"
    )
    val pulse by transition.animateFloat(
        initialValue = 0.75f,
        targetValue = 1.0f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 900, easing = FastOutSlowInEasing),
            repeatMode = RepeatMode.Reverse
        ),
        label = "orbitPulse"
    )

    val clamped = progress.coerceIn(0f, 1f)

    Box(
        modifier = modifier.size(size),
        contentAlignment = Alignment.Center
    ) {
        Canvas(modifier = Modifier.matchParentSize()) {
            val strokePx = strokeWidth.toPx()
            val inset = strokePx + 4.dp.toPx()
            val arcSize = Size(this.size.width - inset * 2f, this.size.height - inset * 2f)
            val topLeft = Offset(inset, inset)
            val center = Offset(this.size.width * 0.5f, this.size.height * 0.5f)
            val radius = arcSize.width * 0.5f

            // Halo ambiental suave
            drawCircle(
                color = primaryColor.copy(alpha = 0.08f * pulse),
                radius = radius + strokePx * 0.9f,
                center = center
            )

            // Pista base
            drawArc(
                color = StudioColors.BgDark,
                startAngle = 0f,
                sweepAngle = 360f,
                useCenter = false,
                topLeft = topLeft,
                size = arcSize,
                style = Stroke(width = strokePx, cap = StrokeCap.Round)
            )

            val startAngle = if (indeterminate) spinDeg - 90f else -90f
            val sweepAngle = if (indeterminate) 115f else (clamped * 360f).coerceAtLeast(if (clamped > 0f) 8f else 0f)

            if (sweepAngle > 0.5f) {
                drawArc(
                    brush = Brush.sweepGradient(
                        colors = listOf(primaryColor, secondaryColor, primaryColor)
                    ),
                    startAngle = startAngle,
                    sweepAngle = sweepAngle,
                    useCenter = false,
                    topLeft = topLeft,
                    size = arcSize,
                    style = Stroke(width = strokePx, cap = StrokeCap.Round)
                )

                // Destello orbital en la punta del arco
                val tipRad = Math.toRadians((startAngle + sweepAngle).toDouble())
                val tipX = center.x + (cos(tipRad) * radius).toFloat()
                val tipY = center.y + (sin(tipRad) * radius).toFloat()
                drawCircle(
                    color = secondaryColor.copy(alpha = 0.36f * pulse),
                    radius = strokePx * 1.35f,
                    center = Offset(tipX, tipY)
                )
                drawCircle(
                    color = Color.White.copy(alpha = 0.92f),
                    radius = strokePx * 0.55f,
                    center = Offset(tipX, tipY)
                )
            }
        }
        centerContent()
    }
}

/**
 * Barra de progreso horizontal con gradiente cálido y destello animado fluido.
 */
@Composable
fun StudioGlowingProgressBar(
    progress: Float,
    modifier: Modifier = Modifier,
    indeterminate: Boolean = false,
    primaryColor: Color = StudioColors.AccentCyan,
    secondaryColor: Color = StudioColors.AccentEmerald,
    height: Dp = 8.dp
) {
    val transition = rememberInfiniteTransition(label = "studioBarShimmer")
    val shimmer by transition.animateFloat(
        initialValue = -0.35f,
        targetValue = 1.35f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 1400, easing = LinearEasing),
            repeatMode = RepeatMode.Restart
        ),
        label = "barShimmerOffset"
    )
    val clamped = progress.coerceIn(0f, 1f)

    Canvas(
        modifier = modifier
            .fillMaxWidth()
            .height(height)
            .clip(RoundedCornerShape(50))
    ) {
        val corner = CornerRadius(size.height * 0.5f, size.height * 0.5f)
        drawRoundRect(
            color = StudioColors.BgDark,
            topLeft = Offset.Zero,
            size = size,
            cornerRadius = corner
        )

        if (indeterminate) {
            val segW = size.width * 0.34f
            val startX = (shimmer * size.width) - segW * 0.5f
            drawRoundRect(
                brush = Brush.horizontalGradient(
                    colors = listOf(
                        primaryColor.copy(alpha = 0.1f),
                        primaryColor,
                        secondaryColor,
                        secondaryColor.copy(alpha = 0.1f)
                    ),
                    startX = startX,
                    endX = startX + segW
                ),
                topLeft = Offset(startX.coerceIn(0f, size.width), 0f),
                size = Size(segW.coerceAtMost(size.width), size.height),
                cornerRadius = corner
            )
        } else if (clamped > 0.005f) {
            val fillW = (size.width * clamped).coerceAtLeast(size.height)
            drawRoundRect(
                brush = Brush.horizontalGradient(
                    colors = listOf(primaryColor, secondaryColor)
                ),
                topLeft = Offset.Zero,
                size = Size(fillW, size.height),
                cornerRadius = corner
            )
            // Reflejo luminoso deslizándose dentro del progreso llenado
            val glowCenter = fillW * shimmer
            val glowHalf = (fillW * 0.28f).coerceAtLeast(24f)
            drawRoundRect(
                brush = Brush.horizontalGradient(
                    colors = listOf(
                        Color.Transparent,
                        Color.White.copy(alpha = 0.34f),
                        Color.Transparent
                    ),
                    startX = glowCenter - glowHalf,
                    endX = glowCenter + glowHalf
                ),
                topLeft = Offset.Zero,
                size = Size(fillW, size.height),
                cornerRadius = corner
            )
        }
    }
}

/**
 * Indicador limpio de pasos para orientar al creador sin términos técnicos complejos.
 */
@Composable
fun StudioMilestoneStepper(
    steps: List<String>,
    activeIndex: Int,
    accentColor: Color = StudioColors.AccentEmerald,
    modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        steps.forEachIndexed { idx, label ->
            val completed = idx < activeIndex
            val active = idx == activeIndex
            val borderColor = when {
                completed -> accentColor.copy(alpha = 0.6f)
                active -> StudioColors.AccentCyan.copy(alpha = 0.8f)
                else -> StudioColors.BorderSubtle
            }
            val dotColor = when {
                completed -> accentColor
                active -> StudioColors.AccentCyan
                else -> StudioColors.TextMuted.copy(alpha = 0.4f)
            }
            val bgColor = when {
                active -> StudioColors.AccentCyan.copy(alpha = 0.12f)
                completed -> accentColor.copy(alpha = 0.08f)
                else -> StudioColors.BgCard
            }
            Surface(
                modifier = Modifier.weight(1f),
                shape = RoundedCornerShape(9.dp),
                color = bgColor,
                border = BorderStroke(1.dp, borderColor)
            ) {
                Row(
                    modifier = Modifier.padding(horizontal = 8.dp, vertical = 6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.Center
                ) {
                    Box(
                        modifier = Modifier
                            .size(7.dp)
                            .clip(CircleShape)
                            .background(dotColor)
                    )
                    Spacer(Modifier.width(6.dp))
                    Text(
                        text = label,
                        color = if (active || completed) StudioColors.TextPrimary else StudioColors.TextMuted,
                        fontSize = 10.5.sp,
                        fontWeight = if (active) FontWeight.Bold else FontWeight.Medium,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
            }
        }
    }
}
