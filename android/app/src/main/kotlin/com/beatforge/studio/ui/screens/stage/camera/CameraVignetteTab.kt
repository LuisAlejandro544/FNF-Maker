// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/camera/CameraVignetteTab.kt
// Propósito: Pestaña 2 ("Viñeta") del inspector multi-cámara:
// ajusta la viñeta / brillo lateral que sigue al encuadre de la cámara (color HEX,
// intensidad, ancho de borde, pulso al beat, capa Escenario/HUD y enlace a `camera_system.lua`).

package com.beatforge.studio.ui.screens.stage.camera

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Code
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun CameraVignetteTab(
    camera: StageCameraSnapshot,
    localVignetteColor: String,
    localVignetteAlpha: Float,
    localVignetteWidth: Float,
    localVignettePulse: Float,
    onVignetteColorChange: (String) -> Unit,
    onVignetteAlphaChange: (Float, Boolean) -> Unit,
    onVignetteWidthChange: (Float, Boolean) -> Unit,
    onVignettePulseChange: (Float, Boolean) -> Unit,
    onCommitVignette: (
        enabled: Boolean,
        colorHex: String,
        alpha: Float,
        width: Float,
        pulse: Float,
        overHud: Boolean,
        save: Boolean
    ) -> Unit,
    onOpenCameraScript: () -> Unit
) {
    Text(
        text = "Resplandor o viñeta lateral que acompaña siempre el encuadre de la cámara. Puedes activarla desde el inicio o hacer que aparezca/desaparezca en cualquier paso # del Chart.",
        color = StudioColors.TextMuted,
        fontSize = 9.5.sp,
        lineHeight = 12.5.sp
    )

    Surface(
        shape = RoundedCornerShape(8.dp),
        color = StudioColors.BgCard,
        border = BorderStroke(
            1.dp,
            if (camera.vignetteEnabled) StudioColors.AccentCoral.copy(alpha = 0.65f) else StudioColors.BorderSubtle
        )
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 8.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = "Activar viñeta lateral base",
                    color = StudioColors.TextPrimary,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
                Text(
                    text = "Visible desde el arranque (o desactívala y lánzala por pasos en el Chart)",
                    color = StudioColors.TextMuted,
                    fontSize = 9.5.sp,
                    lineHeight = 12.sp
                )
            }
            Spacer(Modifier.width(6.dp))
            Switch(
                checked = camera.vignetteEnabled,
                onCheckedChange = { nextEnabled ->
                    onCommitVignette(
                        nextEnabled,
                        localVignetteColor,
                        localVignetteAlpha,
                        localVignetteWidth,
                        localVignettePulse,
                        camera.vignetteOverHud,
                        true
                    )
                }
            )
        }
    }

    Text("Color de los bordes laterales", color = StudioColors.TextMuted, fontSize = 10.5.sp)
    FlowRow(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(5.dp),
        verticalArrangement = Arrangement.spacedBy(5.dp)
    ) {
        VIGNETTE_PRESET_COLORS.forEach { (hex, label) ->
            val parsed = parseInspectorHexColor(hex)
            val isSel = localVignetteColor.equals(hex, ignoreCase = true)
            Row(
                modifier = Modifier
                    .height(26.dp)
                    .clip(RoundedCornerShape(6.dp))
                    .background(if (isSel) parsed.copy(alpha = 0.24f) else StudioColors.BgCard)
                    .border(
                        1.dp,
                        if (isSel) parsed else StudioColors.BorderSubtle,
                        RoundedCornerShape(6.dp)
                    )
                    .clickable {
                        onVignetteColorChange(hex)
                        onCommitVignette(
                            true,
                            hex,
                            localVignetteAlpha,
                            localVignetteWidth,
                            localVignettePulse,
                            camera.vignetteOverHud,
                            true
                        )
                    }
                    .padding(horizontal = 7.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                Box(
                    modifier = Modifier
                        .size(10.dp)
                        .clip(CircleShape)
                        .background(parsed)
                        .border(0.8.dp, Color.White.copy(alpha = 0.45f), CircleShape)
                )
                Text(
                    text = label,
                    color = StudioColors.TextPrimary,
                    fontSize = 10.sp,
                    fontWeight = if (isSel) FontWeight.Bold else FontWeight.Medium,
                    maxLines = 1,
                    softWrap = false
                )
            }
        }
    }

    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(6.dp),
        verticalAlignment = Alignment.CenterVertically
    ) {
        OutlinedTextField(
            value = localVignetteColor,
            onValueChange = { raw ->
                onVignetteColorChange(raw)
                val clean = raw.trim().removePrefix("#")
                if (clean.length == 6 || clean.length == 8) {
                    val normalized = "#$clean"
                    onCommitVignette(
                        camera.vignetteEnabled,
                        normalized,
                        localVignetteAlpha,
                        localVignetteWidth,
                        localVignettePulse,
                        camera.vignetteOverHud,
                        false
                    )
                }
            },
            label = { Text("Color Hex (#RRGGBB)", fontSize = 10.sp) },
            singleLine = true,
            modifier = Modifier.weight(1f)
        )
        OutlinedButton(
            onClick = {
                onCommitVignette(
                    true,
                    localVignetteColor,
                    localVignetteAlpha,
                    localVignetteWidth,
                    localVignettePulse,
                    camera.vignetteOverHud,
                    true
                )
            },
            contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
            modifier = Modifier.height(34.dp)
        ) {
            Text("Fijar", fontSize = 10.5.sp, fontWeight = FontWeight.Bold)
        }
    }

    Column {
        Text(
            text = "Intensidad lateral: ${(localVignetteAlpha * 100f).roundToInt()}%",
            color = StudioColors.TextMuted,
            fontSize = 11.sp
        )
        Slider(
            value = localVignetteAlpha.coerceIn(0f, 1f),
            onValueChange = { nextA ->
                onVignetteAlphaChange(nextA, true)
                onCommitVignette(
                    camera.vignetteEnabled,
                    localVignetteColor,
                    nextA,
                    localVignetteWidth,
                    localVignettePulse,
                    camera.vignetteOverHud,
                    false
                )
            },
            onValueChangeFinished = {
                onVignetteAlphaChange(localVignetteAlpha, false)
                onCommitVignette(
                    camera.vignetteEnabled,
                    localVignetteColor,
                    localVignetteAlpha,
                    localVignetteWidth,
                    localVignettePulse,
                    camera.vignetteOverHud,
                    true
                )
            },
            valueRange = 0f..1f,
            colors = SliderDefaults.colors(
                thumbColor = StudioColors.AccentCoral,
                activeTrackColor = StudioColors.AccentCoral
            )
        )
    }

    Column {
        Text(
            text = "Ancho de borde lateral: ${(localVignetteWidth * 100f).roundToInt()}%",
            color = StudioColors.TextMuted,
            fontSize = 11.sp
        )
        Slider(
            value = localVignetteWidth.coerceIn(0.05f, 0.50f),
            onValueChange = { nextW ->
                onVignetteWidthChange(nextW, true)
                onCommitVignette(
                    camera.vignetteEnabled,
                    localVignetteColor,
                    localVignetteAlpha,
                    nextW,
                    localVignettePulse,
                    camera.vignetteOverHud,
                    false
                )
            },
            onValueChangeFinished = {
                onVignetteWidthChange(localVignetteWidth, false)
                onCommitVignette(
                    camera.vignetteEnabled,
                    localVignetteColor,
                    localVignetteAlpha,
                    localVignetteWidth,
                    localVignettePulse,
                    camera.vignetteOverHud,
                    true
                )
            },
            valueRange = 0.05f..0.50f,
            colors = SliderDefaults.colors(
                thumbColor = StudioColors.AccentCyan,
                activeTrackColor = StudioColors.AccentCyan
            )
        )
    }

    Column {
        Text(
            text = "Pulso al Beat / Shake: ${(localVignettePulse * 100f).roundToInt()}%",
            color = StudioColors.TextMuted,
            fontSize = 11.sp
        )
        Slider(
            value = localVignettePulse.coerceIn(0f, 1f),
            onValueChange = { nextP ->
                onVignettePulseChange(nextP, true)
                onCommitVignette(
                    camera.vignetteEnabled,
                    localVignetteColor,
                    localVignetteAlpha,
                    localVignetteWidth,
                    nextP,
                    camera.vignetteOverHud,
                    false
                )
            },
            onValueChangeFinished = {
                onVignettePulseChange(localVignettePulse, false)
                onCommitVignette(
                    camera.vignetteEnabled,
                    localVignetteColor,
                    localVignetteAlpha,
                    localVignetteWidth,
                    localVignettePulse,
                    camera.vignetteOverHud,
                    true
                )
            },
            valueRange = 0f..1f,
            colors = SliderDefaults.colors(
                thumbColor = StudioColors.AccentAmber,
                activeTrackColor = StudioColors.AccentAmber
            )
        )
    }

    Text("Capa visual de la viñeta", color = StudioColors.TextMuted, fontSize = 10.5.sp)
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        listOf(false to "Escenario (detrás HUD)", true to "Sobre HUD / Flechas").forEach { (over, label) ->
            val isSel = camera.vignetteOverHud == over
            Box(
                modifier = Modifier
                    .weight(1f)
                    .height(29.dp)
                    .clip(RoundedCornerShape(7.dp))
                    .background(if (isSel) StudioColors.AccentCyan.copy(alpha = 0.22f) else StudioColors.BgCard)
                    .border(
                        1.dp,
                        if (isSel) StudioColors.AccentCyan else StudioColors.BorderSubtle,
                        RoundedCornerShape(7.dp)
                    )
                    .clickable {
                        onCommitVignette(
                            camera.vignetteEnabled,
                            localVignetteColor,
                            localVignetteAlpha,
                            localVignetteWidth,
                            localVignettePulse,
                            over,
                            true
                        )
                    },
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = label,
                    color = if (isSel) StudioColors.AccentCyan else StudioColors.TextPrimary,
                    fontSize = 10.sp,
                    fontWeight = if (isSel) FontWeight.Bold else FontWeight.Medium,
                    maxLines = 1,
                    softWrap = false
                )
            }
        }
    }

    Button(
        onClick = onOpenCameraScript,
        contentPadding = PaddingValues(horizontal = 10.dp, vertical = 5.dp),
        colors = ButtonDefaults.buttonColors(
            containerColor = StudioColors.AccentPurple,
            contentColor = Color.White
        ),
        modifier = Modifier
            .fillMaxWidth()
            .height(34.dp)
    ) {
        Icon(Icons.Rounded.Code, null, modifier = Modifier.size(15.dp))
        Spacer(Modifier.width(6.dp))
        Text("Editar camera_system.lua", fontSize = 11.sp, fontWeight = FontWeight.Bold)
    }
}
