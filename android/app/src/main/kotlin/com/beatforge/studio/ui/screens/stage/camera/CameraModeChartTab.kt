// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/camera/CameraModeChartTab.kt
// Propósito: Pestaña 1 ("Modo") del inspector multi-cámara:
// configura el modo de seguimiento en Gameplay (`fixed`, `sections`, `beats`),
// suavizado de cámara, fuerza de paneo hacia el objetivo, límite al fondo y acceso a `camera_system.lua`.

package com.beatforge.studio.ui.screens.stage.camera

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Code
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
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
import com.beatforge.studio.StageCameraSlotSnapshot
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

@Composable
internal fun CameraModeChartTab(
    camera: StageCameraSnapshot,
    activeSlot: StageCameraSlotSnapshot,
    localSmooth: Float,
    localPan: Float,
    onSmoothChange: (Float, Boolean) -> Unit,
    onPanChange: (Float, Boolean) -> Unit,
    onCommitGlobalCamera: (
        enabled: Boolean,
        mode: String,
        beats: Int,
        clampToBackground: Boolean
    ) -> Unit,
    onCommitSlotSmoothPan: (smooth: Float, pan: Float, save: Boolean) -> Unit,
    onOpenCameraScript: () -> Unit
) {
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

    Text("Modo de seguimiento en Gameplay", color = StudioColors.TextMuted, fontSize = 11.sp)
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        listOf(
            "fixed" to "Cam 0 Fija",
            "sections" to "Chart + Ev",
            "beats" to "Cada Nb"
        ).forEach { (modeKey, label) ->
            val isSel = camera.followMode == modeKey
            Box(
                modifier = Modifier
                    .weight(1f)
                    .height(30.dp)
                    .clip(RoundedCornerShape(7.dp))
                    .background(if (isSel) StudioColors.AccentCyan.copy(alpha = 0.22f) else StudioColors.BgCard)
                    .border(
                        1.dp,
                        if (isSel) StudioColors.AccentCyan else StudioColors.BorderSubtle,
                        RoundedCornerShape(7.dp)
                    )
                    .clickable {
                        onCommitGlobalCamera(
                            camera.enabled,
                            modeKey,
                            camera.followIntervalBeats,
                            camera.clampToBackground
                        )
                    },
                contentAlignment = Alignment.Center
            ) {
                Text(
                    text = label,
                    color = if (isSel) StudioColors.AccentCyan else StudioColors.TextPrimary,
                    fontSize = 10.5.sp,
                    fontWeight = if (isSel) FontWeight.Bold else FontWeight.Medium,
                    maxLines = 1,
                    softWrap = false
                )
            }
        }
    }

    when (camera.followMode) {
        "fixed" -> {
            Text(
                text = "Arranca fija en Cam 0 hasta recibir una ranura explícita de sección, eventos de chart.json (FocusCamera, Switch Camera, Camera Follow Pos, ZoomCamera) o scripts.",
                color = StudioColors.TextMuted,
                fontSize = 10.sp,
                lineHeight = 13.sp
            )
        }
        "beats" -> {
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                Text(
                    text = "Arranca en Cam 0 y alterna Cam 1 / Cam 2 cada ${camera.followIntervalBeats} beats (los eventos del chart tienen prioridad).",
                    color = StudioColors.TextMuted,
                    fontSize = 10.sp,
                    lineHeight = 13.sp
                )
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    listOf(2, 4, 8, 16).forEach { beatsVal ->
                        val isSel = camera.followIntervalBeats == beatsVal
                        Box(
                            modifier = Modifier
                                .weight(1f)
                                .height(27.dp)
                                .clip(RoundedCornerShape(6.dp))
                                .background(if (isSel) StudioColors.AccentEmerald.copy(alpha = 0.22f) else StudioColors.BgCard)
                                .border(
                                    1.dp,
                                    if (isSel) StudioColors.AccentEmerald else StudioColors.BorderSubtle,
                                    RoundedCornerShape(6.dp)
                                )
                                .clickable {
                                    onCommitGlobalCamera(
                                        camera.enabled,
                                        camera.followMode,
                                        beatsVal,
                                        camera.clampToBackground
                                    )
                                },
                            contentAlignment = Alignment.Center
                        ) {
                            Text(
                                text = "${beatsVal}b",
                                color = if (isSel) StudioColors.AccentEmerald else StudioColors.TextPrimary,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.SemiBold
                            )
                        }
                    }
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    CameraStepChip(
                        label = "- 1 beat",
                        onClick = {
                            onCommitGlobalCamera(
                                camera.enabled,
                                camera.followMode,
                                (camera.followIntervalBeats - 1).coerceIn(1, 32),
                                camera.clampToBackground
                            )
                        },
                        modifier = Modifier.weight(1f)
                    )
                    CameraStepChip(
                        label = "+ 1 beat",
                        onClick = {
                            onCommitGlobalCamera(
                                camera.enabled,
                                camera.followMode,
                                (camera.followIntervalBeats + 1).coerceIn(1, 32),
                                camera.clampToBackground
                            )
                        },
                        modifier = Modifier.weight(1f)
                    )
                }
            }
        }
        else -> {
            Text(
                text = "Arranca en Cam 0 al empezar el conteo; usa mustHitSection, gfSection, ranuras Cam N de cada sección y eventos directos de chart.json (FocusCamera, Camera Follow Pos, ZoomCamera, Add Camera Zoom, Switch Camera).",
                color = StudioColors.TextMuted,
                fontSize = 10.sp,
                lineHeight = 13.sp
            )
        }
    }

    Column {
        Text(
            text = "Suavizado de Cam ${activeSlot.index}: ${((localSmooth * 10f).roundToInt() / 10f)}",
            color = StudioColors.TextMuted,
            fontSize = 11.sp
        )
        Slider(
            value = localSmooth.coerceIn(1f, 20f),
            onValueChange = { s -> onSmoothChange(s, true) },
            onValueChangeFinished = {
                onSmoothChange(localSmooth, false)
                onCommitSlotSmoothPan(localSmooth, localPan, true)
            },
            valueRange = 1f..20f,
            colors = SliderDefaults.colors(
                thumbColor = StudioColors.AccentIndigo,
                activeTrackColor = StudioColors.AccentIndigo
            )
        )
    }

    if (activeSlot.targetKind != "fixed") {
        Column {
            Text(
                text = "Pan hacia objetivo (Cam ${activeSlot.index}): ${(localPan * 100f).roundToInt()}%",
                color = StudioColors.TextMuted,
                fontSize = 11.sp
            )
            Slider(
                value = localPan.coerceIn(0f, 1f),
                onValueChange = { p -> onPanChange(p, true) },
                onValueChangeFinished = {
                    onPanChange(localPan, false)
                    onCommitSlotSmoothPan(localSmooth, localPan, true)
                },
                valueRange = 0f..1f,
                colors = SliderDefaults.colors(
                    thumbColor = StudioColors.AccentEmerald,
                    activeTrackColor = StudioColors.AccentEmerald
                )
            )
        }
    }

    Surface(
        shape = RoundedCornerShape(8.dp),
        color = StudioColors.BgCard,
        border = BorderStroke(
            1.dp,
            if (camera.clampToBackground) StudioColors.AccentCyan.copy(alpha = 0.45f) else StudioColors.BorderSubtle
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
                    text = "Limitar cámara al fondo",
                    color = StudioColors.TextPrimary,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
                Text(
                    text = "Evita bordes negros cuando el personaje está en un extremo",
                    color = StudioColors.TextMuted,
                    fontSize = 9.5.sp,
                    lineHeight = 12.sp
                )
            }
            Spacer(Modifier.width(6.dp))
            Switch(
                checked = camera.clampToBackground,
                onCheckedChange = { nextClamp ->
                    onCommitGlobalCamera(
                        camera.enabled,
                        camera.followMode,
                        camera.followIntervalBeats,
                        nextClamp
                    )
                }
            )
        }
    }
}
