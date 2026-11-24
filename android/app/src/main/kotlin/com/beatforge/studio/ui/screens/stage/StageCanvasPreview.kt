// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/StageCanvasPreview.kt
// Propósito: Lienzo interactivo central 16:9 (espacio virtual 1280x720) del editor de escenario,
// con visualización y edición directa de múltiples ranuras de cámara (`Cam 0/1/2/3+`),
// barra superior rápida (coordenadas, rejilla, vista amplia overscan 0.68x y bloqueo de capa)
// y barra inferior de acciones rápidas al alcance del pulgar.

package com.beatforge.studio.ui.screens.stage

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
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
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.abs
import kotlin.math.hypot
import kotlin.math.min
import kotlin.math.roundToInt

@OptIn(ExperimentalLayoutApi::class)
@Composable
fun StageCanvasPreview(
    entities: List<StageEntitySnapshot>,
    selected: StageEntitySnapshot?,
    stageCamera: StageCameraSnapshot,
    isCameraSelected: Boolean,
    showGrid: Boolean,
    wideView: Boolean,
    lockedEntityIds: Set<String>,
    entityBitmaps: Map<String, LoadedEntityBitmap>,
    liveTransform: StageLiveTransformState,
    onToggleGrid: () -> Unit,
    onToggleWideView: () -> Unit,
    onToggleSelectedLock: () -> Unit,
    onSelectEntity: (String) -> Unit,
    onSelectCamera: () -> Unit,
    onSelectCameraSlot: (slotIndex: Int) -> Unit,
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
    ) -> Unit,
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
    modifier: Modifier = Modifier
) {
    val latestEntities by rememberUpdatedState(entities)
    val latestSelected by rememberUpdatedState(selected)
    val latestCamera by rememberUpdatedState(stageCamera)
    val latestIsCameraSelected by rememberUpdatedState(isCameraSelected)
    val latestWideView by rememberUpdatedState(wideView)
    val latestLockedIds by rememberUpdatedState(lockedEntityIds)

    val textMeasurer = rememberTextMeasurer()
    // Pre-medir etiquetas de entidades y ranuras de cámara fuera de Canvas DrawScope
    val preMeasuredLabels = remember(entities, textMeasurer) {
        entities.associate { ent ->
            ent.id to textMeasurer.measure(
                text = ent.displayName,
                style = TextStyle(
                    color = StudioColors.TextPrimary,
                    fontSize = 9.sp,
                    fontWeight = FontWeight.SemiBold
                )
            )
        }
    }
    val cameraSlots = stageCamera.resolvedCameras()
    val preMeasuredCamLabels = remember(cameraSlots, textMeasurer) {
        cameraSlots.associate { slot ->
            slot.index to textMeasurer.measure(
                text = "C${slot.index}",
                style = TextStyle(
                    color = StudioColors.AccentCyan,
                    fontSize = 9.sp,
                    fontWeight = FontWeight.Bold
                )
            )
        }
    }

    val activeSlot = stageCamera.activeSlot()
    val useLiveCam = liveTransform.isInteractingCamera && liveTransform.camSlotIndex == activeSlot.index
    val activeSlotZoom = (if (useLiveCam) liveTransform.camZoom else activeSlot.zoom).coerceIn(0.35f, 2.5f)
    val activeSlotX = if (useLiveCam) liveTransform.camSlotX else activeSlot.x
    val activeSlotY = if (useLiveCam) liveTransform.camSlotY else activeSlot.y
    val (resolvedCamX, resolvedCamY) = if (useLiveCam) {
        liveTransform.camX to liveTransform.camY
    } else {
        resolveCameraSlotPreviewCenter(stageCamera, activeSlot, entities)
    }

    val isSelectedLocked = !isCameraSelected && selected != null && lockedEntityIds.contains(selected.id)

    Surface(
        modifier = modifier.fillMaxHeight(),
        shape = RoundedCornerShape(12.dp),
        color = StudioColors.BgPanel,
        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Barra superior compacta del lienzo
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
                        text = "Cam ${activeSlot.index} · (${resolvedCamX.roundToInt()}, ${resolvedCamY.roundToInt()}) · ${((activeSlotZoom * 100f).roundToInt() / 100f)}x",
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

            HorizontalDivider(color = StudioColors.BorderSubtle)

            // Lienzo 16:9
            BoxWithConstraints(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .padding(horizontal = 6.dp, vertical = 4.dp),
                contentAlignment = Alignment.Center
            ) {
                val targetW: androidx.compose.ui.unit.Dp
                val targetH: androidx.compose.ui.unit.Dp
                if (maxWidth / 16f <= maxHeight / 9f) {
                    targetW = maxWidth
                    targetH = maxWidth * (9f / 16f)
                } else {
                    targetH = maxHeight
                    targetW = maxHeight * (16f / 9f)
                }

                Canvas(
                    modifier = Modifier
                        .width(targetW)
                        .height(targetH)
                        .clip(RoundedCornerShape(8.dp))
                        .background(StudioColors.BgDark)
                        .border(1.dp, StudioColors.BorderSubtle, RoundedCornerShape(8.dp))
                        .clipToBounds()
                        .pointerInput(selected?.id, isCameraSelected, stageCamera.activeCameraIndex) {
                            val handleHitRadiusPx = 30.dp.toPx()
                            awaitEachGesture {
                                val down = awaitFirstDown(requireUnconsumed = false)
                                val viewScale = if (latestWideView) 0.68f else 1.0f
                                val sx = (size.width.toFloat().coerceAtLeast(1f) / 1280f) * viewScale
                                val sy = (size.height.toFloat().coerceAtLeast(1f) / 720f) * viewScale
                                val scaleToVirtX = 1f / sx
                                val scaleToVirtY = 1f / sy
                                fun toPxX(vx: Float) = size.width * 0.5f + (vx - 640f) * sx
                                fun toPxY(vy: Float) = size.height * 0.5f + (vy - 360f) * sy
                                fun toVirtX(px: Float) = 640f + (px - size.width * 0.5f) / sx
                                fun toVirtY(py: Float) = 360f + (py - size.height * 0.5f) / sy

                                val camMode = latestIsCameraSelected
                                val camAtDown = latestCamera
                                val entsAtDown = latestEntities
                                val slotAtDown = camAtDown.activeSlot()
                                val selAtDown = if (camMode) null else latestSelected
                                val selLocked = selAtDown != null && latestLockedIds.contains(selAtDown.id)

                                if (camMode) {
                                    liveTransform.syncFromCamera(camAtDown, entsAtDown)
                                } else if (selAtDown != null) {
                                    liveTransform.syncFromEntity(selAtDown)
                                }

                                var localX = if (selAtDown != null && liveTransform.entityId == selAtDown.id) liveTransform.x else (selAtDown?.x ?: 640f)
                                var localY = if (selAtDown != null && liveTransform.entityId == selAtDown.id) liveTransform.y else (selAtDown?.y ?: 360f)
                                var localScale = if (selAtDown != null && liveTransform.entityId == selAtDown.id) liveTransform.scale else (selAtDown?.scale ?: 1f)

                                var localSlotX = slotAtDown.x
                                var localSlotY = slotAtDown.y
                                var localCamZoom = slotAtDown.zoom.coerceIn(0.35f, 2.5f)
                                val panFactor = if (slotAtDown.targetKind == "fixed") {
                                    1f
                                } else {
                                    slotAtDown.panStrength.coerceAtLeast(0.25f)
                                }
                                val (initCamPreviewX, initCamPreviewY) = resolveCameraSlotPreviewCenter(camAtDown, slotAtDown, entsAtDown)

                                var resizeHandleMode = false
                                var startScale = localScale
                                var startCamZoom = localCamZoom
                                var startDistPx = 1f
                                var cxPx = 0f
                                var cyPx = 0f

                                if (camMode) {
                                    cxPx = toPxX(initCamPreviewX)
                                    cyPx = toPxY(initCamPreviewY)
                                    val hwPx = ((1280f / localCamZoom) * 0.5f) * sx
                                    val hhPx = ((720f / localCamZoom) * 0.5f) * sy
                                    val handleX = cxPx + hwPx
                                    val handleY = cyPx + hhPx
                                    if (hypot(down.position.x - handleX, down.position.y - handleY) <= handleHitRadiusPx) {
                                        resizeHandleMode = true
                                        liveTransform.beginCameraInteraction(camAtDown, entsAtDown)
                                        startCamZoom = localCamZoom
                                        startDistPx = hypot(down.position.x - cxPx, down.position.y - cyPx).coerceAtLeast(20f)
                                    }
                                } else if (selAtDown != null && selAtDown.visible && !selLocked) {
                                    cxPx = toPxX(localX)
                                    cyPx = toPxY(localY)
                                    val hwPx = (selAtDown.width * localScale * 0.5f) * sx
                                    val hhPx = (selAtDown.height * localScale * 0.5f) * sy
                                    val handleX = cxPx + hwPx
                                    val handleY = cyPx + hhPx
                                    if (hypot(down.position.x - handleX, down.position.y - handleY) <= handleHitRadiusPx) {
                                        resizeHandleMode = true
                                        liveTransform.beginInteraction(selAtDown)
                                        startScale = localScale
                                        startDistPx = hypot(down.position.x - cxPx, down.position.y - cyPx).coerceAtLeast(12f)
                                    }
                                }

                                var moved = resizeHandleMode
                                var didMutate = false
                                var accumX = 0f
                                var accumY = 0f
                                val touchSlop = viewConfiguration.touchSlop

                                while (true) {
                                    val event = awaitPointerEvent()
                                    val pressedChanges = event.changes.filter { it.pressed }
                                    if (pressedChanges.isEmpty()) break

                                    if (camMode) {
                                        if (pressedChanges.size >= 2) {
                                            resizeHandleMode = false
                                            val zoom = event.calculateZoom()
                                            val pan = event.calculatePan()
                                            if (!moved && (abs(zoom - 1f) > 0.01f || pan.getDistance() > touchSlop)) {
                                                moved = true
                                                liveTransform.beginCameraInteraction(camAtDown, entsAtDown)
                                            }
                                            if (moved) {
                                                localCamZoom = (localCamZoom / zoom).coerceIn(0.35f, 2.5f)
                                                localSlotX = (localSlotX + (pan.x * scaleToVirtX) / panFactor).coerceIn(-1200f, 2600f)
                                                localSlotY = (localSlotY + (pan.y * scaleToVirtY) / panFactor).coerceIn(-1200f, 1800f)
                                                liveTransform.updateCameraSlotTransform(
                                                    camAtDown, slotAtDown, entsAtDown, localSlotX, localSlotY, localCamZoom
                                                )
                                                didMutate = true
                                                pressedChanges.forEach { ch ->
                                                    if (ch.positionChanged()) ch.consume()
                                                }
                                            }
                                        } else {
                                            val ch = pressedChanges.first()
                                            val delta = ch.position - ch.previousPosition
                                            if (!moved) {
                                                accumX += delta.x
                                                accumY += delta.y
                                                if (hypot(accumX, accumY) > touchSlop) {
                                                    moved = true
                                                    liveTransform.beginCameraInteraction(camAtDown, entsAtDown)
                                                    localSlotX = (localSlotX + (accumX * scaleToVirtX) / panFactor).coerceIn(-1200f, 2600f)
                                                    localSlotY = (localSlotY + (accumY * scaleToVirtY) / panFactor).coerceIn(-1200f, 1800f)
                                                }
                                            }
                                            if (moved) {
                                                if (resizeHandleMode) {
                                                    val currDist = hypot(ch.position.x - cxPx, ch.position.y - cyPx).coerceAtLeast(20f)
                                                    localCamZoom = (startCamZoom * (startDistPx / currDist)).coerceIn(0.35f, 2.5f)
                                                } else if (delta != Offset.Zero) {
                                                    localSlotX = (localSlotX + (delta.x * scaleToVirtX) / panFactor).coerceIn(-1200f, 2600f)
                                                    localSlotY = (localSlotY + (delta.y * scaleToVirtY) / panFactor).coerceIn(-1200f, 1800f)
                                                }
                                                liveTransform.updateCameraSlotTransform(
                                                    camAtDown, slotAtDown, entsAtDown, localSlotX, localSlotY, localCamZoom
                                                )
                                                didMutate = true
                                                if (ch.positionChanged()) ch.consume()
                                            }
                                        }
                                    } else if (!selLocked) {
                                        val sel = latestSelected
                                        if (pressedChanges.size >= 2 && sel != null) {
                                            resizeHandleMode = false
                                            val zoom = event.calculateZoom()
                                            val pan = event.calculatePan()
                                            if (!moved && (abs(zoom - 1f) > 0.01f || pan.getDistance() > touchSlop)) {
                                                moved = true
                                                liveTransform.beginInteraction(sel)
                                            }
                                            if (moved) {
                                                localScale = (localScale * zoom).coerceIn(0.1f, 5.0f)
                                                localX = (localX + pan.x * scaleToVirtX).coerceIn(-600f, 1880f)
                                                localY = (localY + pan.y * scaleToVirtY).coerceIn(-400f, 1120f)
                                                liveTransform.updatePositionAndScale(localX, localY, localScale)
                                                didMutate = true
                                                pressedChanges.forEach { ch ->
                                                    if (ch.positionChanged()) ch.consume()
                                                }
                                            }
                                        } else {
                                            val ch = pressedChanges.first()
                                            val delta = ch.position - ch.previousPosition
                                            if (!moved) {
                                                accumX += delta.x
                                                accumY += delta.y
                                                if (hypot(accumX, accumY) > touchSlop) {
                                                    moved = true
                                                    if (sel != null) {
                                                        liveTransform.beginInteraction(sel)
                                                        localX = (localX + accumX * scaleToVirtX).coerceIn(-600f, 1880f)
                                                        localY = (localY + accumY * scaleToVirtY).coerceIn(-400f, 1120f)
                                                    }
                                                }
                                            }

                                            if (moved && sel != null) {
                                                if (resizeHandleMode) {
                                                    val currDist = hypot(ch.position.x - cxPx, ch.position.y - cyPx)
                                                    localScale = (startScale * (currDist / startDistPx)).coerceIn(0.1f, 5.0f)
                                                } else if (delta != Offset.Zero) {
                                                    localX = (localX + delta.x * scaleToVirtX).coerceIn(-600f, 1880f)
                                                    localY = (localY + delta.y * scaleToVirtY).coerceIn(-400f, 1120f)
                                                }
                                                liveTransform.updatePositionAndScale(localX, localY, localScale)
                                                didMutate = true
                                                if (ch.positionChanged()) ch.consume()
                                            }
                                        }
                                    }
                                }

                                val selEnd = latestSelected
                                if (camMode && didMutate) {
                                    onUpdateCameraSlot(
                                        slotAtDown.index,
                                        slotAtDown.name,
                                        slotAtDown.targetKind,
                                        slotAtDown.targetEntityId,
                                        localSlotX,
                                        localSlotY,
                                        localCamZoom,
                                        slotAtDown.smoothSpeed,
                                        slotAtDown.panStrength,
                                        true
                                    )
                                } else if (!camMode && didMutate && selEnd != null && !selLocked) {
                                    val effOpacity = if (liveTransform.entityId == selEnd.id) liveTransform.opacity else selEnd.opacity
                                    onUpdateEntity(
                                        selEnd.id,
                                        selEnd.displayName,
                                        selEnd.role,
                                        0f,
                                        0f,
                                        localX,
                                        localY,
                                        true,
                                        localScale,
                                        effOpacity,
                                        selEnd.visible,
                                        selEnd.flipX,
                                        true
                                    )
                                } else if (!moved) {
                                    val vx = toVirtX(down.position.x)
                                    val vy = toVirtY(down.position.y)
                                    var hitId: String? = null
                                    for (idx in latestEntities.indices.reversed()) {
                                        val ent = latestEntities[idx]
                                        if (!ent.visible || latestLockedIds.contains(ent.id)) continue
                                        val isSelEnt = !camMode && ent.id == selEnd?.id && liveTransform.entityId == ent.id
                                        val entScale = if (isSelEnt) liveTransform.scale else ent.scale
                                        val entX = if (isSelEnt) liveTransform.x else ent.x
                                        val entY = if (isSelEnt) liveTransform.y else ent.y
                                        val hw = (ent.width * entScale) * 0.5f
                                        val hh = (ent.height * entScale) * 0.5f
                                        if (abs(vx - entX) <= hw.coerceAtLeast(32f) &&
                                            abs(vy - entY) <= hh.coerceAtLeast(32f)
                                        ) {
                                            hitId = ent.id
                                            break
                                        }
                                    }
                                    if (hitId != null && !camMode) {
                                        onSelectEntity(hitId)
                                    } else {
                                        val matchingSlot = camAtDown.resolvedCameras().firstOrNull { slot ->
                                            val (sxPos, syPos) = resolveCameraSlotPreviewCenter(camAtDown, slot, entsAtDown)
                                            hypot(vx - sxPos, vy - syPos) <= 72f
                                        }
                                        if (matchingSlot != null) {
                                            onSelectCamera()
                                            onSelectCameraSlot(matchingSlot.index)
                                        } else if (hitId != null) {
                                            onSelectEntity(hitId)
                                        }
                                    }
                                }
                                liveTransform.endInteraction()
                                liveTransform.endCameraInteraction()
                            }
                        }
                ) {
                    val viewScale = if (wideView) 0.68f else 1.0f
                    val sx = (size.width / 1280f) * viewScale
                    val sy = (size.height / 720f) * viewScale
                    val stageLeft = size.width * 0.5f - 640f * sx
                    val stageTop = size.height * 0.5f - 360f * sy
                    val stageW = 1280f * sx
                    val stageH = 720f * sy

                    // Rejilla guía de 1280x720
                    if (showGrid) {
                        for (gx in 1..7) {
                            val px = stageLeft + stageW * (gx / 8f)
                            drawLine(
                                StudioColors.BorderSubtle.copy(alpha = if (gx == 4) 0.55f else 0.28f),
                                Offset(px, stageTop),
                                Offset(px, stageTop + stageH),
                                strokeWidth = if (gx == 4) 1.5f else 1f
                            )
                        }
                        for (gy in 1..4) {
                            val py = stageTop + stageH * (gy / 5f)
                            drawLine(
                                StudioColors.BorderSubtle.copy(alpha = 0.28f),
                                Offset(stageLeft, py),
                                Offset(stageLeft + stageW, py)
                            )
                        }
                        drawLine(
                            StudioColors.BorderSubtle.copy(alpha = 0.50f),
                            Offset(stageLeft, stageTop + stageH * 0.5f),
                            Offset(stageLeft + stageW, stageTop + stageH * 0.5f),
                            strokeWidth = 1.3f
                        )
                    }

                    if (wideView) {
                        drawRect(
                            color = StudioColors.TextMuted.copy(alpha = 0.45f),
                            topLeft = Offset(stageLeft, stageTop),
                            size = Size(stageW, stageH),
                            style = Stroke(width = 1.6f, pathEffect = PathEffect.dashPathEffect(floatArrayOf(10f, 8f), 0f))
                        )
                    }

                    val isFastQuality = liveTransform.isInteracting || liveTransform.isInteractingCamera || entities.size > 20
                    val filterQuality = if (isFastQuality) FilterQuality.Low else FilterQuality.Medium
                    val selId = if (isCameraSelected) null else selected?.id
                    val useLive = selId != null && liveTransform.entityId == selId

                    for (i in entities.indices) {
                        val ent = entities[i]
                        val isSel = ent.id == selId
                        val effOpacity = if (isSel && useLive) liveTransform.opacity else ent.opacity
                        if (!ent.visible || effOpacity <= 0.01f) continue

                        val effScale = if (isSel && useLive) liveTransform.scale else ent.scale
                        val cx = stageLeft + (if (isSel && useLive) liveTransform.x else ent.x) * sx
                        val cy = stageTop + (if (isSel && useLive) liveTransform.y else ent.y) * sy
                        val w = ent.width * effScale * sx
                        val h = ent.height * effScale * sy
                        val left = cx - w / 2f
                        val top = cy - h / 2f

                        if (!isSel && (left + w < -24f || left > size.width + 24f || top + h < -24f || top > size.height + 24f)) {
                            continue
                        }

                        val loaded = entityBitmaps[ent.id]

                        withTransform({
                            if (ent.flipX) {
                                scale(scaleX = -1f, scaleY = 1f, pivot = Offset(cx, cy))
                            }
                        }) {
                            if (loaded != null) {
                                val bmp = loaded.bitmap
                                val f = ent.previewFrame
                                if (f != null && f.found && f.width > 0 && f.height > 0) {
                                    val scaleRatioX = bmp.width.toFloat() / loaded.origWidth.toFloat()
                                    val scaleRatioY = bmp.height.toFloat() / loaded.origHeight.toFloat()
                                    val srcX = (f.x * scaleRatioX).roundToInt().coerceIn(0, bmp.width - 1)
                                    val srcY = (f.y * scaleRatioY).roundToInt().coerceIn(0, bmp.height - 1)
                                    val srcW = (f.width * scaleRatioX).roundToInt().coerceIn(1, bmp.width - srcX)
                                    val srcH = (f.height * scaleRatioY).roundToInt().coerceIn(1, bmp.height - srcY)

                                    val visW = if (f.rotated) f.height else f.width
                                    val visH = if (f.rotated) f.width else f.height
                                    val logicalW = f.frameWidth.coerceAtLeast(visW).toFloat().coerceAtLeast(1f)
                                    val logicalH = f.frameHeight.coerceAtLeast(visH).toFloat().coerceAtLeast(1f)
                                    val fit = min(w / logicalW, h / logicalH)

                                    val boxW = logicalW * fit
                                    val boxH = logicalH * fit
                                    val boxLeft = cx - boxW / 2f
                                    val boxTop = (top + h) - boxH

                                    val visDrawW = visW * fit
                                    val visDrawH = visH * fit
                                    val visLeft = boxLeft + (-f.frameX * fit)
                                    val visTop = boxTop + (-f.frameY * fit)
                                    val frameCenterX = visLeft + visDrawW / 2f
                                    val frameCenterY = visTop + visDrawH / 2f

                                    val dstW = (f.width * fit).roundToInt().coerceAtLeast(2)
                                    val dstH = (f.height * fit).roundToInt().coerceAtLeast(2)
                                    val dstLeft = (frameCenterX - dstW / 2f).roundToInt()
                                    val dstTop = (frameCenterY - dstH / 2f).roundToInt()

                                    withTransform({
                                        if (f.rotated) {
                                            rotate(-90f, pivot = Offset(frameCenterX, frameCenterY))
                                        }
                                    }) {
                                        drawImage(
                                            image = bmp,
                                            srcOffset = IntOffset(srcX, srcY),
                                            srcSize = IntSize(srcW, srcH),
                                            dstOffset = IntOffset(dstLeft, dstTop),
                                            dstSize = IntSize(dstW, dstH),
                                            alpha = effOpacity.coerceIn(0f, 1f),
                                            filterQuality = filterQuality
                                        )
                                    }
                                } else {
                                    drawImage(
                                        image = bmp,
                                        srcOffset = IntOffset.Zero,
                                        srcSize = IntSize(bmp.width, bmp.height),
                                        dstOffset = IntOffset(left.roundToInt(), top.roundToInt()),
                                        dstSize = IntSize(w.roundToInt().coerceAtLeast(2), h.roundToInt().coerceAtLeast(2)),
                                        alpha = effOpacity.coerceIn(0f, 1f),
                                        filterQuality = filterQuality
                                    )
                                }
                            } else {
                                val fallbackColor = when (ent.role) {
                                    "background" -> StudioColors.AccentIndigo
                                    "opponent" -> StudioColors.AccentCoral
                                    "player" -> StudioColors.AccentEmerald
                                    else -> StudioColors.AccentCyan
                                }
                                drawRect(
                                    color = fallbackColor.copy(alpha = 0.22f * effOpacity),
                                    topLeft = Offset(left, top),
                                    size = Size(w, h)
                                )
                                drawRect(
                                    color = fallbackColor.copy(alpha = effOpacity),
                                    topLeft = Offset(left, top),
                                    size = Size(w, h),
                                    style = Stroke(width = 2f)
                                )
                            }
                        }

                        if (loaded == null && w > 36f && h > 18f) {
                            val layout = preMeasuredLabels[ent.id]
                            if (layout != null) {
                                drawText(
                                    textLayoutResult = layout,
                                    alpha = effOpacity.coerceIn(0f, 1f),
                                    topLeft = Offset(
                                        (cx - layout.size.width / 2f).coerceIn(left + 4f, (left + w - layout.size.width - 4f).coerceAtLeast(left + 4f)),
                                        (cy - layout.size.height / 2f).coerceIn(top + 2f, (top + h - layout.size.height - 2f).coerceAtLeast(top + 2f))
                                    )
                                )
                            }
                        }

                        if (isSel) {
                            val selBorderColor = if (isSelectedLocked) StudioColors.AccentIndigo else StudioColors.AccentEmerald
                            drawRect(
                                color = selBorderColor,
                                topLeft = Offset(left - 2f, top - 2f),
                                size = Size(w + 4f, h + 4f),
                                style = Stroke(
                                    width = 2.5f,
                                    pathEffect = if (isSelectedLocked) PathEffect.dashPathEffect(floatArrayOf(8f, 6f), 0f) else null
                                )
                            )
                            if (!isSelectedLocked) {
                                val handleCenter = Offset(left + w, top + h)
                                drawCircle(
                                    color = StudioColors.BgDark,
                                    radius = 10.dp.toPx(),
                                    center = handleCenter
                                )
                                drawCircle(
                                    color = StudioColors.AccentEmerald,
                                    radius = 8.dp.toPx(),
                                    center = handleCenter
                                )
                            }
                        }
                    }

                    // Marcos visuales de referencia de las demás ranuras de cámara (cuando la capa Cámara está seleccionada)
                    if (isCameraSelected) {
                        cameraSlots.forEach { slot ->
                            if (slot.index != activeSlot.index) {
                                val (otherX, otherY) = resolveCameraSlotPreviewCenter(stageCamera, slot, entities)
                                val otherZoom = slot.zoom.coerceIn(0.35f, 2.5f)
                                val oW = (1280f / otherZoom) * sx
                                val oH = (720f / otherZoom) * sy
                                val oCx = stageLeft + otherX * sx
                                val oCy = stageTop + otherY * sy
                                val ghostColor = when (slot.targetKind) {
                                    "player" -> StudioColors.AccentEmerald
                                    "opponent" -> StudioColors.AccentCoral
                                    "gf" -> StudioColors.AccentAmber
                                    "entity" -> StudioColors.AccentPurple
                                    else -> StudioColors.AccentCyan
                                }.copy(alpha = 0.36f)
                                drawRect(
                                    color = ghostColor,
                                    topLeft = Offset(oCx - oW * 0.5f, oCy - oH * 0.5f),
                                    size = Size(oW, oH),
                                    style = Stroke(width = 1.4f, pathEffect = PathEffect.dashPathEffect(floatArrayOf(10f, 8f), 0f))
                                )
                                drawCircle(color = ghostColor, radius = 4f, center = Offset(oCx, oCy))
                                preMeasuredCamLabels[slot.index]?.let { lbl ->
                                    drawText(
                                        textLayoutResult = lbl,
                                        alpha = 0.65f,
                                        topLeft = Offset(oCx + 6f, oCy - lbl.size.height - 2f)
                                    )
                                }
                            }
                        }
                    }

                    // Marco visual de captura de la cámara activa (1280/zoom x 720/zoom)
                    val camBoxW = (1280f / activeSlotZoom) * sx
                    val camBoxH = (720f / activeSlotZoom) * sy
                    val camCxPx = stageLeft + resolvedCamX * sx
                    val camCyPx = stageTop + resolvedCamY * sy
                    val camLeft = camCxPx - camBoxW * 0.5f
                    val camTop = camCyPx - camBoxH * 0.5f

                    val camColor = when {
                        !stageCamera.enabled -> StudioColors.AccentCoral.copy(alpha = if (isCameraSelected) 0.9f else 0.45f)
                        isCameraSelected -> StudioColors.AccentCyan
                        else -> StudioColors.AccentCyan.copy(alpha = 0.50f)
                    }
                    val camDash = if (isCameraSelected && stageCamera.enabled) {
                        null
                    } else {
                        PathEffect.dashPathEffect(floatArrayOf(14f, 10f), 0f)
                    }

                    if (isCameraSelected) {
                        drawRect(
                            color = StudioColors.AccentCyan.copy(alpha = 0.06f),
                            topLeft = Offset(camLeft, camTop),
                            size = Size(camBoxW, camBoxH)
                        )
                    }
                    if (stageCamera.vignetteEnabled && stageCamera.vignetteAlpha > 0.01f && camBoxW > 4f && camBoxH > 4f) {
                        val vigColor = parseStageHexColorOrDefault(stageCamera.vignetteColor)
                        val vigAlpha = stageCamera.vignetteAlpha.coerceIn(0f, 1f)
                        val vigSideW = (camBoxW * stageCamera.vignetteWidth.coerceIn(0.05f, 0.50f)).coerceAtLeast(2f)
                        val vigVertH = (camBoxH * 0.14f).coerceAtLeast(2f)
                        clipRect(
                            left = camLeft,
                            top = camTop,
                            right = camLeft + camBoxW,
                            bottom = camTop + camBoxH
                        ) {
                            drawRect(
                                brush = Brush.horizontalGradient(
                                    colors = listOf(vigColor.copy(alpha = vigAlpha), Color.Transparent),
                                    startX = camLeft,
                                    endX = camLeft + vigSideW
                                ),
                                topLeft = Offset(camLeft, camTop),
                                size = Size(vigSideW, camBoxH)
                            )
                            drawRect(
                                brush = Brush.horizontalGradient(
                                    colors = listOf(Color.Transparent, vigColor.copy(alpha = vigAlpha)),
                                    startX = camLeft + camBoxW - vigSideW,
                                    endX = camLeft + camBoxW
                                ),
                                topLeft = Offset(camLeft + camBoxW - vigSideW, camTop),
                                size = Size(vigSideW, camBoxH)
                            )
                            drawRect(
                                brush = Brush.verticalGradient(
                                    colors = listOf(vigColor.copy(alpha = vigAlpha * 0.28f), Color.Transparent),
                                    startY = camTop,
                                    endY = camTop + vigVertH
                                ),
                                topLeft = Offset(camLeft, camTop),
                                size = Size(camBoxW, vigVertH)
                            )
                            drawRect(
                                brush = Brush.verticalGradient(
                                    colors = listOf(Color.Transparent, vigColor.copy(alpha = vigAlpha * 0.28f)),
                                    startY = camTop + camBoxH - vigVertH,
                                    endY = camTop + camBoxH
                                ),
                                topLeft = Offset(camLeft, camTop + camBoxH - vigVertH),
                                size = Size(camBoxW, vigVertH)
                            )
                        }
                    }
                    drawRect(
                        color = camColor,
                        topLeft = Offset(camLeft, camTop),
                        size = Size(camBoxW, camBoxH),
                        style = Stroke(width = if (isCameraSelected) 3f else 1.8f, pathEffect = camDash)
                    )

                    val crossR = if (isCameraSelected) 12f else 8f
                    drawLine(camColor, Offset(camCxPx - crossR, camCyPx), Offset(camCxPx + crossR, camCyPx), strokeWidth = 2f)
                    drawLine(camColor, Offset(camCxPx, camCyPx - crossR), Offset(camCxPx, camCyPx + crossR), strokeWidth = 2f)

                    preMeasuredCamLabels[activeSlot.index]?.let { lbl ->
                        drawText(
                            textLayoutResult = lbl,
                            alpha = if (isCameraSelected) 0.95f else 0.65f,
                            topLeft = Offset(camCxPx + crossR + 4f, camCyPx - lbl.size.height - 2f)
                        )
                    }

                    if (isCameraSelected) {
                        val camHandleCenter = Offset(camLeft + camBoxW, camTop + camBoxH)
                        drawCircle(
                            color = StudioColors.BgDark,
                            radius = 11.dp.toPx(),
                            center = camHandleCenter
                        )
                        drawCircle(
                            color = StudioColors.AccentCyan,
                            radius = 8.5.dp.toPx(),
                            center = camHandleCenter
                        )
                    }
                }
            }

            HorizontalDivider(color = StudioColors.BorderSubtle)

            // Barra inferior de acciones rápidas sin recortes al alcance del pulgar
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

private fun parseStageHexColorOrDefault(raw: String, fallback: Color = Color(0xFFEF4444)): Color {
    val clean = raw.trim().removePrefix("#")
    return try {
        when (clean.length) {
            6 -> Color((0xFF000000L or clean.toLong(16)).toInt())
            8 -> Color(clean.toLong(16).toInt())
            else -> fallback
        }
    } catch (_: Throwable) {
        fallback
    }
}
