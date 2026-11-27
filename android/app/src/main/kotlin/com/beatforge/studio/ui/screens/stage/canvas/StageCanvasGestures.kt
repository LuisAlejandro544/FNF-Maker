// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/canvas/StageCanvasGestures.kt
// Propósito: Manejo táctil multi-touch (arrastre, pellizco de escala/zoom, asa de esquina
// y selección inversa Z-order de capas o ranuras de cámara) sobre el lienzo 16:9.

package com.beatforge.studio.ui.screens.stage.canvas

import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.calculatePan
import androidx.compose.foundation.gestures.calculateZoom
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.input.pointer.positionChanged
import androidx.compose.ui.unit.dp
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.screens.stage.StageLiveTransformState
import com.beatforge.studio.ui.screens.stage.resolveCameraSlotPreviewCenter
import kotlin.math.abs
import kotlin.math.hypot

@Composable
internal fun Modifier.stageCanvasPointerGestures(
    entities: List<StageEntitySnapshot>,
    selected: StageEntitySnapshot?,
    stageCamera: StageCameraSnapshot,
    isCameraSelected: Boolean,
    wideView: Boolean,
    lockedEntityIds: Set<String>,
    liveTransform: StageLiveTransformState,
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
    ) -> Unit
): Modifier {
    val latestEntities by rememberUpdatedState(entities)
    val latestSelected by rememberUpdatedState(selected)
    val latestCamera by rememberUpdatedState(stageCamera)
    val latestIsCameraSelected by rememberUpdatedState(isCameraSelected)
    val latestWideView by rememberUpdatedState(wideView)
    val latestLockedIds by rememberUpdatedState(lockedEntityIds)

    return this.pointerInput(selected?.id, isCameraSelected, stageCamera.activeCameraIndex) {
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
}
