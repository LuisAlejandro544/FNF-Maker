// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/camera/CameraSlotFovTab.kt
// Propósito: Pestaña 0 ("Ranura & FOV") del inspector multi-cámara:
// configura el objetivo de la ranura activa (`Fija`, `Player`, `Enemigo`, `GF`, `Entidad`),
// zoom/FOV en tiempo real, paso en píxeles, cruceta direccional y atajos de encuadre.

package com.beatforge.studio.ui.screens.stage.camera

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.ArrowForward
import androidx.compose.material.icons.rounded.ArrowDownward
import androidx.compose.material.icons.rounded.ArrowUpward
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StageCameraSlotSnapshot
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.screens.stage.StageLiveTransformState
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

@Composable
internal fun CameraSlotFovTab(
    camera: StageCameraSnapshot,
    activeSlot: StageCameraSlotSnapshot,
    entities: List<StageEntitySnapshot>,
    stepPx: Int,
    shownSlotX: Float,
    shownSlotY: Float,
    shownZoom: Float,
    localZoom: Float,
    previewX: Float,
    previewY: Float,
    captureW: Int,
    captureH: Int,
    opponentEnt: StageEntitySnapshot?,
    playerEnt: StageEntitySnapshot?,
    liveTransform: StageLiveTransformState,
    onStepChange: (Int) -> Unit,
    onZoomDragChanged: (Boolean) -> Unit,
    onApplyImmediateSlotZoom: (nextZoom: Float, save: Boolean) -> Unit,
    onCommitActiveSlot: (
        name: String,
        targetKind: String,
        targetEntityId: String?,
        sx: Float,
        sy: Float,
        zoom: Float,
        smooth: Float,
        pan: Float,
        save: Boolean
    ) -> Unit
) {
    val roleHint = when (activeSlot.index) {
        0 -> "Cámara inicial del mod: permanece fija hasta que una sección, evento del chart o script mueva el enfoque."
        1 -> "Cámara del Player (Jugador): ajusta su FOV y encuadre relativo al personaje."
        2 -> "Cámara del Enemigo (Rival): ajusta su FOV y encuadre relativo al rival."
        else -> "Cámara extra personalizada: asígnala por sección o en cualquier paso # del chart."
    }
    Text(
        text = roleHint,
        color = StudioColors.TextMuted,
        fontSize = 9.5.sp,
        lineHeight = 12.5.sp
    )

    Text("Objetivo de Cam ${activeSlot.index}", color = StudioColors.TextMuted, fontSize = 10.5.sp)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        CAMERA_TARGET_OPTIONS.forEach { (kindKey, label) ->
            val isSel = activeSlot.targetKind == kindKey
            Box(
                modifier = Modifier
                    .height(26.dp)
                    .clip(RoundedCornerShape(6.dp))
                    .background(if (isSel) StudioColors.AccentCyan.copy(alpha = 0.22f) else StudioColors.BgCard)
                    .border(
                        1.dp,
                        if (isSel) StudioColors.AccentCyan else StudioColors.BorderSubtle,
                        RoundedCornerShape(6.dp)
                    )
                    .clickable {
                        val defaultId = when (kindKey) {
                            "player" -> entities.firstOrNull { it.role == "player" }?.id ?: "player_main"
                            "opponent" -> entities.firstOrNull { it.role == "opponent" }?.id ?: "opponent_main"
                            "gf" -> entities.firstOrNull { it.id == "stage_speakers" }?.id ?: "stage_speakers"
                            "entity" -> activeSlot.targetEntityId ?: entities.firstOrNull()?.id
                            else -> null
                        }
                        val defaultX = if (kindKey == "fixed") previewX else 0f
                        val defaultY = if (kindKey == "fixed") previewY else -24f
                        val defaultPan = if (kindKey == "fixed") 0f else activeSlot.panStrength.takeIf { it > 0.05f } ?: 0.85f
                        val updatedLabel = when (activeSlot.index) {
                            0 -> if (kindKey == "fixed") "Cam 0 · Inicial Fija" else "Cam 0 · $label"
                            1 -> if (kindKey == "player") "Cam 1 · Player" else "Cam 1 · $label"
                            2 -> if (kindKey == "opponent") "Cam 2 · Enemigo" else "Cam 2 · $label"
                            else -> "Cam ${activeSlot.index} · $label"
                        }
                        onCommitActiveSlot(
                            updatedLabel,
                            kindKey,
                            defaultId,
                            defaultX,
                            defaultY,
                            shownZoom,
                            activeSlot.smoothSpeed,
                            defaultPan,
                            true
                        )
                    }
                    .padding(horizontal = 8.dp),
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

    if (activeSlot.targetKind == "entity" && entities.isNotEmpty()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            entities.forEach { ent ->
                val isSel = activeSlot.targetEntityId == ent.id
                Box(
                    modifier = Modifier
                        .height(25.dp)
                        .clip(RoundedCornerShape(6.dp))
                        .background(if (isSel) StudioColors.AccentPurple.copy(alpha = 0.22f) else StudioColors.BgCard)
                        .border(
                            1.dp,
                            if (isSel) StudioColors.AccentPurple else StudioColors.BorderSubtle,
                            RoundedCornerShape(6.dp)
                        )
                        .clickable {
                            onCommitActiveSlot(
                                "Cam ${activeSlot.index} · ${ent.displayName}",
                                activeSlot.targetKind,
                                ent.id,
                                shownSlotX,
                                shownSlotY,
                                shownZoom,
                                activeSlot.smoothSpeed,
                                activeSlot.panStrength,
                                true
                            )
                        }
                        .padding(horizontal = 7.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Text(
                        text = ent.displayName,
                        color = if (isSel) StudioColors.AccentPurple else StudioColors.TextPrimary,
                        fontSize = 9.5.sp,
                        maxLines = 1,
                        softWrap = false
                    )
                }
            }
        }
    }

    Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("FOV / Captura ${captureW}x${captureH}", color = StudioColors.TextMuted, fontSize = 11.sp)
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
                onZoomDragChanged(true)
                liveTransform.beginCameraInteraction(camera, entities)
                onApplyImmediateSlotZoom(z, false)
            },
            onValueChangeFinished = {
                onZoomDragChanged(false)
                liveTransform.endCameraInteraction()
                onApplyImmediateSlotZoom(localZoom, true)
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
                onClick = { onApplyImmediateSlotZoom(localZoom - 0.1f, true) },
                modifier = Modifier.weight(1f)
            )
            CameraStepChip(
                label = "1.0x",
                onClick = { onApplyImmediateSlotZoom(1.0f, true) },
                modifier = Modifier.weight(1f)
            )
            CameraStepChip(
                label = "+ 0.1x",
                onClick = { onApplyImmediateSlotZoom(localZoom + 0.1f, true) },
                modifier = Modifier.weight(1f)
            )
        }
    }

    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        val coordLabel = if (activeSlot.targetKind == "fixed") {
            "Centro (${shownSlotX.roundToInt()}, ${shownSlotY.roundToInt()})"
        } else {
            "Offset (${shownSlotX.roundToInt()}, ${shownSlotY.roundToInt()}) · Vista (${previewX.roundToInt()}, ${previewY.roundToInt()})"
        }
        Text(
            text = coordLabel,
            color = StudioColors.TextPrimary,
            fontSize = 10.5.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            modifier = Modifier.weight(1f)
        )
        Spacer(Modifier.width(4.dp))
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
            onClick = {
                onCommitActiveSlot(
                    activeSlot.name,
                    activeSlot.targetKind,
                    activeSlot.targetEntityId,
                    shownSlotX,
                    shownSlotY - stepPx,
                    shownZoom,
                    activeSlot.smoothSpeed,
                    activeSlot.panStrength,
                    true
                )
            },
            modifier = Modifier.size(32.dp)
        ) {
            Icon(Icons.Rounded.ArrowUpward, "Arriba", tint = StudioColors.AccentCyan)
        }
        Row(
            horizontalArrangement = Arrangement.spacedBy(10.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            IconButton(
                onClick = {
                    onCommitActiveSlot(
                        activeSlot.name,
                        activeSlot.targetKind,
                        activeSlot.targetEntityId,
                        shownSlotX - stepPx,
                        shownSlotY,
                        shownZoom,
                        activeSlot.smoothSpeed,
                        activeSlot.panStrength,
                        true
                    )
                },
                modifier = Modifier.size(32.dp)
            ) {
                Icon(Icons.AutoMirrored.Rounded.ArrowBack, "Izquierda", tint = StudioColors.AccentCyan)
            }
            IconButton(
                onClick = {
                    val (rx, ry) = if (activeSlot.targetKind == "fixed") 640f to 360f else 0f to -24f
                    onCommitActiveSlot(
                        activeSlot.name,
                        activeSlot.targetKind,
                        activeSlot.targetEntityId,
                        rx,
                        ry,
                        shownZoom,
                        activeSlot.smoothSpeed,
                        activeSlot.panStrength,
                        true
                    )
                },
                modifier = Modifier
                    .size(30.dp)
                    .clip(RoundedCornerShape(8.dp))
                    .background(StudioColors.BgCard)
            ) {
                Icon(Icons.Rounded.MyLocation, "Restablecer centro", tint = StudioColors.TextMuted, modifier = Modifier.size(16.dp))
            }
            IconButton(
                onClick = {
                    onCommitActiveSlot(
                        activeSlot.name,
                        activeSlot.targetKind,
                        activeSlot.targetEntityId,
                        shownSlotX + stepPx,
                        shownSlotY,
                        shownZoom,
                        activeSlot.smoothSpeed,
                        activeSlot.panStrength,
                        true
                    )
                },
                modifier = Modifier.size(32.dp)
            ) {
                Icon(Icons.AutoMirrored.Rounded.ArrowForward, "Derecha", tint = StudioColors.AccentCyan)
            }
        }
        IconButton(
            onClick = {
                onCommitActiveSlot(
                    activeSlot.name,
                    activeSlot.targetKind,
                    activeSlot.targetEntityId,
                    shownSlotX,
                    shownSlotY + stepPx,
                    shownZoom,
                    activeSlot.smoothSpeed,
                    activeSlot.panStrength,
                    true
                )
            },
            modifier = Modifier.size(32.dp)
        ) {
            Icon(Icons.Rounded.ArrowDownward, "Abajo", tint = StudioColors.AccentCyan)
        }
    }

    if (activeSlot.targetKind == "fixed") {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(5.dp)
        ) {
            CameraStepChip(
                label = "Enemigo",
                onClick = {
                    val tx = opponentEnt?.x ?: 300f
                    val ty = (opponentEnt?.y ?: 415f) - 24f
                    onCommitActiveSlot(
                        activeSlot.name,
                        activeSlot.targetKind,
                        activeSlot.targetEntityId,
                        tx,
                        ty,
                        shownZoom,
                        activeSlot.smoothSpeed,
                        activeSlot.panStrength,
                        true
                    )
                },
                modifier = Modifier.weight(1f)
            )
            CameraStepChip(
                label = "Centro",
                onClick = {
                    onCommitActiveSlot(
                        activeSlot.name,
                        activeSlot.targetKind,
                        activeSlot.targetEntityId,
                        640f,
                        360f,
                        shownZoom,
                        activeSlot.smoothSpeed,
                        activeSlot.panStrength,
                        true
                    )
                },
                modifier = Modifier.weight(1f)
            )
            CameraStepChip(
                label = "Jugador",
                onClick = {
                    val tx = playerEnt?.x ?: 980f
                    val ty = (playerEnt?.y ?: 420f) - 24f
                    onCommitActiveSlot(
                        activeSlot.name,
                        activeSlot.targetKind,
                        activeSlot.targetEntityId,
                        tx,
                        ty,
                        shownZoom,
                        activeSlot.smoothSpeed,
                        activeSlot.panStrength,
                        true
                    )
                },
                modifier = Modifier.weight(1f)
            )
        }
    } else {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(5.dp)
        ) {
            CameraStepChip(
                label = "Encuadre (0, -24)",
                onClick = {
                    onCommitActiveSlot(
                        activeSlot.name,
                        activeSlot.targetKind,
                        activeSlot.targetEntityId,
                        0f,
                        -24f,
                        shownZoom,
                        activeSlot.smoothSpeed,
                        activeSlot.panStrength,
                        true
                    )
                },
                modifier = Modifier.weight(1f)
            )
            CameraStepChip(
                label = "Cabeza (0, -90)",
                onClick = {
                    onCommitActiveSlot(
                        activeSlot.name,
                        activeSlot.targetKind,
                        activeSlot.targetEntityId,
                        0f,
                        -90f,
                        shownZoom,
                        activeSlot.smoothSpeed,
                        activeSlot.panStrength,
                        true
                    )
                },
                modifier = Modifier.weight(1f)
            )
        }
    }
}
