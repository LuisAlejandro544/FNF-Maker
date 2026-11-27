// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/canvas/StageCanvasBars.kt
// Propósito: Barras superior e inferior del lienzo de escenario (estado activo de cámara/capa,
// toggles de rejilla, vista amplia overscan 0.68x, bloqueo y chips de acciones rápidas al pulgar).

package com.beatforge.studio.ui.screens.stage.canvas

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CropFree
import androidx.compose.material.icons.rounded.Flip
import androidx.compose.material.icons.rounded.GridOff
import androidx.compose.material.icons.rounded.GridOn
import androidx.compose.material.icons.rounded.Lock
import androidx.compose.material.icons.rounded.LockOpen
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material.icons.rounded.Videocam
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material.icons.rounded.VisibilityOff
import androidx.compose.material.icons.rounded.ZoomOutMap
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StageCameraSlotSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.screens.stage.StageLiveTransformState
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

@Composable
internal fun StageCanvasTopBar(
    isCameraSelected: Boolean,
    activeSlotIndex: Int,
    resolvedCamX: Float,
    resolvedCamY: Float,
    activeSlotZoom: Float,
    selected: StageEntitySnapshot?,
    liveTransform: StageLiveTransformState,
    showGrid: Boolean,
    wideView: Boolean,
    isSelectedLocked: Boolean,
    onToggleGrid: () -> Unit,
    onToggleWideView: () -> Unit,
    onToggleSelectedLock: () -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 5.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp)
    ) {
        if (isCameraSelected) {
            Icon(
                imageVector = Icons.Rounded.Videocam,
                contentDescription = null,
                tint = StudioColors.AccentCyan,
                modifier = Modifier.size(14.dp)
            )
            Text(
                text = "Cam $activeSlotIndex · (${resolvedCamX.roundToInt()}, ${resolvedCamY.roundToInt()}) · ${((activeSlotZoom * 100f).roundToInt() / 100f)}x",
                color = StudioColors.AccentCyan,
                fontSize = 10.5.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f)
            )
        } else if (selected != null) {
            val useLive = liveTransform.entityId == selected.id
            val effX = if (useLive) liveTransform.x else selected.x
            val effY = if (useLive) liveTransform.y else selected.y
            val effScale = if (useLive) liveTransform.scale else selected.scale
            val dotColor = when (selected.role) {
                "background" -> StudioColors.AccentIndigo
                "opponent" -> StudioColors.AccentCoral
                "player" -> StudioColors.AccentEmerald
                else -> StudioColors.AccentCyan
            }
            Box(
                modifier = Modifier
                    .size(7.dp)
                    .clip(CircleShape)
                    .background(dotColor)
            )
            Text(
                text = "${selected.displayName} · (${effX.roundToInt()}, ${effY.roundToInt()}) · ${((effScale * 100f).roundToInt() / 100f)}x",
                color = StudioColors.TextPrimary,
                fontSize = 10.5.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.weight(1f)
            )
        } else {
            Text(
                text = "Escenario 1280×720",
                color = StudioColors.TextMuted,
                fontSize = 10.5.sp,
                modifier = Modifier.weight(1f)
            )
        }

        CanvasToolbarToggle(
            icon = if (showGrid) Icons.Rounded.GridOn else Icons.Rounded.GridOff,
            label = null,
            active = showGrid,
            activeColor = StudioColors.AccentEmerald,
            onClick = onToggleGrid
        )
        CanvasToolbarToggle(
            icon = if (wideView) Icons.Rounded.ZoomOutMap else Icons.Rounded.CropFree,
            label = if (wideView) "Amplia" else "100%",
            active = wideView,
            activeColor = StudioColors.AccentCyan,
            onClick = onToggleWideView
        )
        if (!isCameraSelected && selected != null) {
            CanvasToolbarToggle(
                icon = if (isSelectedLocked) Icons.Rounded.Lock else Icons.Rounded.LockOpen,
                label = null,
                active = isSelectedLocked,
                activeColor = StudioColors.AccentIndigo,
                onClick = onToggleSelectedLock
            )
        }
    }
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
internal fun StageCanvasBottomBar(
    isCameraSelected: Boolean,
    cameraSlots: List<StageCameraSlotSnapshot>,
    activeSlot: StageCameraSlotSnapshot,
    activeSlotX: Float,
    activeSlotY: Float,
    activeSlotZoom: Float,
    selected: StageEntitySnapshot?,
    liveTransform: StageLiveTransformState,
    onSelectCameraSlot: (Int) -> Unit,
    onUpdateCameraSlot: (
        slotIndex: Int,
        name: String,
        targetKind: String,
        targetEntityId: String?,
        x: Float,
        y: Float,
        zoom: Float,
        smoothSpeed: Float,
        panStrength: Float,
        commitSave: Boolean
    ) -> Unit,
    onUpdateEntity: (
        entityId: String,
        name: String,
        role: String,
        dx: Float,
        dy: Float,
        absX: Float,
        absY: Float,
        useAbsPos: Boolean,
        scale: Float,
        opacity: Float,
        visible: Boolean,
        flipX: Boolean,
        commitSave: Boolean
    ) -> Unit
) {
    FlowRow(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 8.dp, vertical = 5.dp),
        horizontalArrangement = Arrangement.spacedBy(5.dp),
        verticalArrangement = Arrangement.spacedBy(5.dp)
    ) {
        if (isCameraSelected) {
            cameraSlots.forEach { slot ->
                val isSel = slot.index == activeSlot.index
                val accent = when (slot.targetKind) {
                    "player" -> StudioColors.AccentEmerald
                    "opponent" -> StudioColors.AccentCoral
                    "gf" -> StudioColors.AccentAmber
                    "entity" -> StudioColors.AccentPurple
                    else -> StudioColors.AccentCyan
                }
                QuickActionChip(
                    label = "Cam ${slot.index}",
                    active = isSel,
                    activeColor = accent,
                    onClick = { onSelectCameraSlot(slot.index) }
                )
            }
            QuickActionChip(
                label = "-0.1x",
                onClick = {
                    onUpdateCameraSlot(
                        activeSlot.index, activeSlot.name, activeSlot.targetKind, activeSlot.targetEntityId,
                        activeSlotX, activeSlotY, (activeSlotZoom - 0.1f).coerceIn(0.35f, 2.5f),
                        activeSlot.smoothSpeed, activeSlot.panStrength, true
                    )
                }
            )
            QuickActionChip(
                label = "1.0x",
                onClick = {
                    onUpdateCameraSlot(
                        activeSlot.index, activeSlot.name, activeSlot.targetKind, activeSlot.targetEntityId,
                        activeSlotX, activeSlotY, 1.0f,
                        activeSlot.smoothSpeed, activeSlot.panStrength, true
                    )
                }
            )
            QuickActionChip(
                label = "+0.1x",
                onClick = {
                    onUpdateCameraSlot(
                        activeSlot.index, activeSlot.name, activeSlot.targetKind, activeSlot.targetEntityId,
                        activeSlotX, activeSlotY, (activeSlotZoom + 0.1f).coerceIn(0.35f, 2.5f),
                        activeSlot.smoothSpeed, activeSlot.panStrength, true
                    )
                }
            )
            QuickActionChip(
                icon = Icons.Rounded.MyLocation,
                label = if (activeSlot.targetKind == "fixed") "Centro" else "Reset",
                onClick = {
                    val rx = if (activeSlot.targetKind == "fixed") 640f else 0f
                    val ry = if (activeSlot.targetKind == "fixed") 360f else -24f
                    onUpdateCameraSlot(
                        activeSlot.index, activeSlot.name, activeSlot.targetKind, activeSlot.targetEntityId,
                        rx, ry, activeSlotZoom, activeSlot.smoothSpeed, activeSlot.panStrength, true
                    )
                }
            )
        } else if (selected != null) {
            val curScale = if (liveTransform.entityId == selected.id) liveTransform.scale else selected.scale
            QuickActionChip(
                icon = if (selected.visible) Icons.Rounded.Visibility else Icons.Rounded.VisibilityOff,
                label = if (selected.visible) "Visible" else "Oculto",
                active = selected.visible,
                activeColor = StudioColors.AccentEmerald,
                onClick = {
                    onUpdateEntity(
                        selected.id, selected.displayName, selected.role,
                        0f, 0f, selected.x, selected.y, false,
                        curScale, selected.opacity, !selected.visible, selected.flipX, true
                    )
                }
            )
            QuickActionChip(
                icon = Icons.Rounded.Flip,
                label = "Espejo",
                active = selected.flipX,
                activeColor = StudioColors.AccentCyan,
                onClick = {
                    onUpdateEntity(
                        selected.id, selected.displayName, selected.role,
                        0f, 0f, selected.x, selected.y, false,
                        curScale, selected.opacity, selected.visible, !selected.flipX, true
                    )
                }
            )
            QuickActionChip(
                icon = Icons.Rounded.MyLocation,
                label = "Centrar",
                onClick = {
                    onUpdateEntity(
                        selected.id, selected.displayName, selected.role,
                        0f, 0f, 640f, 360f, true,
                        curScale, selected.opacity, selected.visible, selected.flipX, true
                    )
                }
            )
            QuickActionChip(
                label = "-0.1x",
                onClick = {
                    onUpdateEntity(
                        selected.id, selected.displayName, selected.role,
                        0f, 0f, selected.x, selected.y, false,
                        (curScale - 0.1f).coerceIn(0.1f, 5.0f), selected.opacity, selected.visible, selected.flipX, true
                    )
                }
            )
            QuickActionChip(
                label = "1.0x",
                onClick = {
                    onUpdateEntity(
                        selected.id, selected.displayName, selected.role,
                        0f, 0f, selected.x, selected.y, false,
                        1.0f, selected.opacity, selected.visible, selected.flipX, true
                    )
                }
            )
            QuickActionChip(
                label = "+0.1x",
                onClick = {
                    onUpdateEntity(
                        selected.id, selected.displayName, selected.role,
                        0f, 0f, selected.x, selected.y, false,
                        (curScale + 0.1f).coerceIn(0.1f, 5.0f), selected.opacity, selected.visible, selected.flipX, true
                    )
                }
            )
        }
    }
}

@Composable
private fun CanvasToolbarToggle(
    icon: ImageVector,
    label: String?,
    active: Boolean,
    activeColor: Color,
    onClick: () -> Unit
) {
    Row(
        modifier = Modifier
            .height(26.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(if (active) activeColor.copy(alpha = 0.20f) else StudioColors.BgCard)
            .border(
                1.dp,
                if (active) activeColor else StudioColors.BorderSubtle,
                RoundedCornerShape(6.dp)
            )
            .clickable(onClick = onClick)
            .padding(horizontal = if (label == null) 6.dp else 7.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(3.dp)
    ) {
        Icon(
            imageVector = icon,
            contentDescription = label,
            tint = if (active) activeColor else StudioColors.TextMuted,
            modifier = Modifier.size(14.dp)
        )
        if (label != null) {
            Text(
                text = label,
                color = if (active) activeColor else StudioColors.TextPrimary,
                fontSize = 10.sp,
                fontWeight = FontWeight.SemiBold,
                maxLines = 1,
                softWrap = false
            )
        }
    }
}

@Composable
private fun QuickActionChip(
    label: String,
    icon: ImageVector? = null,
    active: Boolean = false,
    activeColor: Color = StudioColors.AccentCyan,
    onClick: () -> Unit
) {
    Row(
        modifier = Modifier
            .height(26.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(if (active) activeColor.copy(alpha = 0.18f) else StudioColors.BgCard)
            .border(
                1.dp,
                if (active) activeColor else StudioColors.BorderSubtle,
                RoundedCornerShape(6.dp)
            )
            .clickable(onClick = onClick)
            .padding(horizontal = 6.5.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        if (icon != null) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = if (active) activeColor else StudioColors.TextMuted,
                modifier = Modifier.size(13.dp)
            )
        }
        Text(
            text = label,
            color = if (active) activeColor else StudioColors.TextPrimary,
            fontSize = 10.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            softWrap = false
        )
    }
}
