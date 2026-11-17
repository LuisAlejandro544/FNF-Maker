// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/StageCameraInspector.kt
// Propósito: Inspector derecho modular para configurar el Sistema de Cámara del escenario,
// organizado en dos pestañas (`Encuadre` y `Seguimiento`) para ajustar zoom/posición sin scroll
// incómodo, elegir modo por beats o secciones de `chart.json` y abrir `camera_system.lua`.

package com.beatforge.studio.ui.screens.stage

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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ArrowBack
import androidx.compose.material.icons.rounded.ArrowDownward
import androidx.compose.material.icons.rounded.ArrowForward
import androidx.compose.material.icons.rounded.ArrowUpward
import androidx.compose.material.icons.rounded.Code
import androidx.compose.material.icons.rounded.CropFree
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material.icons.rounded.Videocam
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

@Composable
fun StageCameraInspector(
    camera: StageCameraSnapshot,
    entities: List<StageEntitySnapshot>,
    stepPx: Int,
    liveTransform: StageLiveTransformState,
    onStepChange: (Int) -> Unit,
    onUpdateCamera: (
        enabled: Boolean,
        baseX: Float,
        baseY: Float,
        baseZoom: Float,
        followMode: String,
        followIntervalBeats: Int,
        smoothSpeed: Float,
        panStrength: Float,
        commitSave: Boolean
    ) -> Unit,
    onOpenCameraScript: () -> Unit
) {
    var localZoom by remember { mutableFloatStateOf(camera.baseZoom) }
    var isDraggingZoom by remember { mutableStateOf(false) }
    var localSmooth by remember { mutableFloatStateOf(camera.smoothSpeed) }
    var isDraggingSmooth by remember { mutableStateOf(false) }
    var localPan by remember { mutableFloatStateOf(camera.panStrength) }
    var isDraggingPan by remember { mutableStateOf(false) }
    var activeTab by remember { mutableIntStateOf(0) } // 0 = Encuadre, 1 = Seguimiento

    LaunchedEffect(camera.baseZoom) {
        if (!isDraggingZoom) localZoom = camera.baseZoom
    }
    LaunchedEffect(camera.smoothSpeed) {
        if (!isDraggingSmooth) localSmooth = camera.smoothSpeed
    }
    LaunchedEffect(camera.panStrength) {
        if (!isDraggingPan) localPan = camera.panStrength
    }

    val shownX = if (liveTransform.isInteractingCamera) liveTransform.camX else camera.baseX
    val shownY = if (liveTransform.isInteractingCamera) liveTransform.camY else camera.baseY
    val shownZoom = if (!isDraggingZoom && liveTransform.isInteractingCamera) {
        liveTransform.camZoom
    } else {
        localZoom
    }.coerceIn(0.35f, 2.5f)

    val captureW = (1280f / shownZoom).roundToInt()
    val captureH = (720f / shownZoom).roundToInt()

    fun commitCamera(
        enabled: Boolean = camera.enabled,
        bx: Float = shownX,
        by: Float = shownY,
        bz: Float = shownZoom,
        mode: String = camera.followMode,
        beats: Int = camera.followIntervalBeats,
        smooth: Float = localSmooth,
        pan: Float = localPan,
        save: Boolean = true
    ) {
        onUpdateCamera(enabled, bx, by, bz, mode, beats, smooth, pan, save)
    }

    fun applyImmediateZoom(nextZoom: Float, save: Boolean) {
        val clamped = nextZoom.coerceIn(0.35f, 2.5f)
        localZoom = clamped
        liveTransform.updateCamera(shownX, shownY, clamped)
        if (save) {
            commitCamera(bz = clamped, save = true)
        }
    }

    val opponentEnt = remember(entities) { entities.firstOrNull { it.role == "opponent" } }
    val playerEnt = remember(entities) { entities.firstOrNull { it.role == "player" } }

    Column(
        modifier = Modifier.padding(9.dp),
        verticalArrangement = Arrangement.spacedBy(7.dp)
    ) {
        Surface(
            shape = RoundedCornerShape(9.dp),
            color = StudioColors.BgElevated,
            border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.45f))
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Icon(
                    imageVector = Icons.Rounded.Videocam,
                    contentDescription = null,
                    tint = StudioColors.AccentCyan,
                    modifier = Modifier.size(17.dp)
                )
                Spacer(Modifier.width(6.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = "Cámara del Escenario",
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 12.sp
                    )
                    Text(
                        text = "camera_system.lua",
                        color = StudioColors.TextMuted,
                        fontSize = 10.sp
                    )
                }
                Switch(
                    checked = camera.enabled,
                    onCheckedChange = { nextEnabled -> commitCamera(enabled = nextEnabled) }
                )
            }
        }

        // Pestañas ergonómicas: Encuadre / Seguimiento
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            CameraTabChip(
                label = "Encuadre",
                icon = Icons.Rounded.CropFree,
                selected = activeTab == 0,
                onClick = { activeTab = 0 },
                modifier = Modifier.weight(1f)
            )
            CameraTabChip(
                label = "Seguimiento",
                icon = Icons.Rounded.Tune,
                selected = activeTab == 1,
                onClick = { activeTab = 1 },
                modifier = Modifier.weight(1f)
            )
        }

        Column(
            modifier = Modifier
                .weight(1f)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            if (activeTab == 0) {
                // Tamaño de captura / Zoom base
                Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text("Captura ${captureW}x${captureH}", color = StudioColors.TextMuted, fontSize = 11.sp)
                        Text(
                            text = "Zoom ${((shownZoom * 100f).roundToInt() / 100f)}x",
                            color = StudioColors.AccentCyan,
                            fontSize = 11.sp,
                            fontWeight = FontWeight.Bold
                        )
                    }
                    Slider(
                        value = shownZoom.coerceIn(0.35f, 2.5f),
                        onValueChange = { z ->
                            isDraggingZoom = true
                            liveTransform.beginCameraInteraction(camera)
                            applyImmediateZoom(z, save = false)
                        },
                        onValueChangeFinished = {
                            isDraggingZoom = false
                            liveTransform.endCameraInteraction()
                            applyImmediateZoom(localZoom, save = true)
                        },
                        valueRange = 0.35f..2.5f,
                        colors = SliderDefaults.colors(
                            thumbColor = StudioColors.AccentCyan,
                            activeTrackColor = StudioColors.AccentCyan
                        )
                    )
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(5.dp)
                    ) {
                        CameraStepChip(
                            label = "- 0.1x",
                            onClick = { applyImmediateZoom(localZoom - 0.1f, save = true) },
                            modifier = Modifier.weight(1f)
                        )
                        CameraStepChip(
                            label = "1.0x",
                            onClick = { applyImmediateZoom(1.0f, save = true) },
                            modifier = Modifier.weight(1f)
                        )
                        CameraStepChip(
                            label = "+ 0.1x",
                            onClick = { applyImmediateZoom(localZoom + 0.1f, save = true) },
                            modifier = Modifier.weight(1f)
                        )
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Text(
                        text = "Centro (${shownX.roundToInt()}, ${shownY.roundToInt()})",
                        color = StudioColors.TextPrimary,
                        fontSize = 11.sp,
                        fontWeight = FontWeight.SemiBold
                    )
                    Text(
                        text = "Paso ${stepPx}px",
                        color = StudioColors.AccentEmerald,
                        fontSize = 10.sp,
                        fontWeight = FontWeight.Bold
                    )
                }

                Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
                    listOf(1, 10, 35).forEach { px ->
                        val isSel = stepPx == px
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
                                .clickable { onStepChange(px) },
                            contentAlignment = Alignment.Center
                        ) {
                            Text(
                                text = "${px}px",
                                color = if (isSel) StudioColors.AccentEmerald else StudioColors.TextPrimary,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.SemiBold,
                                maxLines = 1,
                                softWrap = false
                            )
                        }
                    }
                }

                Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
                    IconButton(
                        onClick = { commitCamera(bx = shownX, by = shownY - stepPx) },
                        modifier = Modifier.size(32.dp)
                    ) {
                        Icon(Icons.Rounded.ArrowUpward, "Arriba", tint = StudioColors.AccentCyan)
                    }
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        IconButton(
                            onClick = { commitCamera(bx = shownX - stepPx, by = shownY) },
                            modifier = Modifier.size(32.dp)
                        ) {
                            Icon(Icons.Rounded.ArrowBack, "Izquierda", tint = StudioColors.AccentCyan)
                        }
                        IconButton(
                            onClick = { commitCamera(bx = 640f, by = 360f) },
                            modifier = Modifier
                                .size(30.dp)
                                .clip(RoundedCornerShape(8.dp))
                                .background(StudioColors.BgCard)
                        ) {
                            Icon(Icons.Rounded.MyLocation, "Centrar cámara", tint = StudioColors.TextMuted, modifier = Modifier.size(16.dp))
                        }
                        IconButton(
                            onClick = { commitCamera(bx = shownX + stepPx, by = shownY) },
                            modifier = Modifier.size(32.dp)
                        ) {
                            Icon(Icons.Rounded.ArrowForward, "Derecha", tint = StudioColors.AccentCyan)
                        }
                    }
                    IconButton(
                        onClick = { commitCamera(bx = shownX, by = shownY + stepPx) },
                        modifier = Modifier.size(32.dp)
                    ) {
                        Icon(Icons.Rounded.ArrowDownward, "Abajo", tint = StudioColors.AccentCyan)
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(5.dp)
                ) {
                    CameraStepChip(
                        label = "Enemigo",
                        onClick = {
                            val tx = opponentEnt?.x ?: 300f
                            val ty = (opponentEnt?.y ?: 420f) - 24f
                            commitCamera(bx = tx, by = ty)
                        },
                        modifier = Modifier.weight(1f)
                    )
                    CameraStepChip(
                        label = "Centro",
                        onClick = { commitCamera(bx = 640f, by = 360f) },
                        modifier = Modifier.weight(1f)
                    )
                    CameraStepChip(
                        label = "Jugador",
                        onClick = {
                            val tx = playerEnt?.x ?: 980f
                            val ty = (playerEnt?.y ?: 430f) - 24f
                            commitCamera(bx = tx, by = ty)
                        },
                        modifier = Modifier.weight(1f)
                    )
                }
            } else {
                // Pestaña 2: Modo, Intervalo, Suavizado y Script Lua
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

                Text("Modo de seguimiento", color = StudioColors.TextMuted, fontSize = 11.sp)
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(5.dp)
                ) {
                    listOf(
                        "beats" to "Cada N beats",
                        "sections" to "Secciones chart"
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
                                .clickable { commitCamera(mode = modeKey) },
                            contentAlignment = Alignment.Center
                        ) {
                            Text(
                                text = label,
                                color = if (isSel) StudioColors.AccentCyan else StudioColors.TextPrimary,
                                fontSize = 11.sp,
                                fontWeight = if (isSel) FontWeight.Bold else FontWeight.Medium,
                                maxLines = 1,
                                softWrap = false
                            )
                        }
                    }
                }

                if (camera.followMode == "beats") {
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text(
                            text = "Alternar enfoque cada ${camera.followIntervalBeats} beats",
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp
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
                                        .clickable { commitCamera(beats = beatsVal) },
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
                                onClick = { commitCamera(beats = (camera.followIntervalBeats - 1).coerceIn(1, 32)) },
                                modifier = Modifier.weight(1f)
                            )
                            CameraStepChip(
                                label = "+ 1 beat",
                                onClick = { commitCamera(beats = (camera.followIntervalBeats + 1).coerceIn(1, 32)) },
                                modifier = Modifier.weight(1f)
                            )
                        }
                    }
                } else {
                    Text(
                        text = "Usa mustHitSection del chart.json para enfocar automáticamente al Jugador o al Enemigo.",
                        color = StudioColors.TextMuted,
                        fontSize = 10.sp,
                        lineHeight = 13.sp
                    )
                }

                Column {
                    Text(
                        text = "Suavizado de movimiento: ${((localSmooth * 10f).roundToInt() / 10f)}",
                        color = StudioColors.TextMuted,
                        fontSize = 11.sp
                    )
                    Slider(
                        value = localSmooth.coerceIn(1f, 20f),
                        onValueChange = { s ->
                            isDraggingSmooth = true
                            localSmooth = s
                        },
                        onValueChangeFinished = {
                            isDraggingSmooth = false
                            commitCamera(smooth = localSmooth, save = true)
                        },
                        valueRange = 1f..20f,
                        colors = SliderDefaults.colors(
                            thumbColor = StudioColors.AccentIndigo,
                            activeTrackColor = StudioColors.AccentIndigo
                        )
                    )
                }

                Column {
                    Text(
                        text = "Fuerza de seguimiento: ${(localPan * 100f).roundToInt()}%",
                        color = StudioColors.TextMuted,
                        fontSize = 11.sp
                    )
                    Slider(
                        value = localPan.coerceIn(0f, 1f),
                        onValueChange = { p ->
                            isDraggingPan = true
                            localPan = p
                        },
                        onValueChangeFinished = {
                            isDraggingPan = false
                            commitCamera(pan = localPan, save = true)
                        },
                        valueRange = 0f..1f,
                        colors = SliderDefaults.colors(
                            thumbColor = StudioColors.AccentEmerald,
                            activeTrackColor = StudioColors.AccentEmerald
                        )
                    )
                }
            }
        }
    }
}

@Composable
private fun CameraTabChip(
    label: String,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
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
private fun CameraStepChip(
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
