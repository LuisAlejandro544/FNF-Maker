// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/StageCanvasPreview.kt
// Propósito: Coordinador del lienzo interactivo 16:9 (espacio virtual 1280x720) del editor de escenario,
// enlazando las barras superior/inferior (`StageCanvasBars`), gestos multi-touch (`StageCanvasGestures`)
// y subrutinas de renderizado `DrawScope` (`StageCanvasRenderer`) sobre `stage/canvas/*`.

package com.beatforge.studio.ui.screens.stage

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.screens.stage.canvas.StageCanvasBottomBar
import com.beatforge.studio.ui.screens.stage.canvas.StageCanvasTopBar
import com.beatforge.studio.ui.screens.stage.canvas.drawActiveCameraAndVignette
import com.beatforge.studio.ui.screens.stage.canvas.drawSecondaryCameraFrames
import com.beatforge.studio.ui.screens.stage.canvas.drawStageEntities
import com.beatforge.studio.ui.screens.stage.canvas.drawStageGrid
import com.beatforge.studio.ui.screens.stage.canvas.stageCanvasPointerGestures
import com.beatforge.studio.ui.theme.StudioColors

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
            StageCanvasTopBar(
                isCameraSelected = isCameraSelected,
                activeSlotIndex = activeSlot.index,
                resolvedCamX = resolvedCamX,
                resolvedCamY = resolvedCamY,
                activeSlotZoom = activeSlotZoom,
                selected = selected,
                liveTransform = liveTransform,
                showGrid = showGrid,
                wideView = wideView,
                isSelectedLocked = isSelectedLocked,
                onToggleGrid = onToggleGrid,
                onToggleWideView = onToggleWideView,
                onToggleSelectedLock = onToggleSelectedLock
            )

            HorizontalDivider(color = StudioColors.BorderSubtle)

            // Lienzo 16:9
            BoxWithConstraints(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .padding(horizontal = 6.dp, vertical = 4.dp),
                contentAlignment = Alignment.Center
            ) {
                val targetW: Dp
                val targetH: Dp
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
                        .stageCanvasPointerGestures(
                            entities = entities,
                            selected = selected,
                            stageCamera = stageCamera,
                            isCameraSelected = isCameraSelected,
                            wideView = wideView,
                            lockedEntityIds = lockedEntityIds,
                            liveTransform = liveTransform,
                            onSelectEntity = onSelectEntity,
                            onSelectCamera = onSelectCamera,
                            onSelectCameraSlot = onSelectCameraSlot,
                            onUpdateEntity = onUpdateEntity,
                            onUpdateCameraSlot = onUpdateCameraSlot
                        )
                ) {
                    val viewScale = if (wideView) 0.68f else 1.0f
                    val sx = (size.width / 1280f) * viewScale
                    val sy = (size.height / 720f) * viewScale
                    val stageLeft = size.width * 0.5f - 640f * sx
                    val stageTop = size.height * 0.5f - 360f * sy
                    val stageW = 1280f * sx
                    val stageH = 720f * sy

                    drawStageGrid(
                        stageLeft = stageLeft,
                        stageTop = stageTop,
                        stageW = stageW,
                        stageH = stageH,
                        showGrid = showGrid,
                        wideView = wideView
                    )

                    drawStageEntities(
                        entities = entities,
                        selectedId = selected?.id,
                        isCameraSelected = isCameraSelected,
                        isSelectedLocked = isSelectedLocked,
                        entityBitmaps = entityBitmaps,
                        liveTransform = liveTransform,
                        preMeasuredLabels = preMeasuredLabels,
                        stageLeft = stageLeft,
                        stageTop = stageTop,
                        sx = sx,
                        sy = sy
                    )

                    if (isCameraSelected) {
                        drawSecondaryCameraFrames(
                            cameraSlots = cameraSlots,
                            activeSlotIndex = activeSlot.index,
                            stageCamera = stageCamera,
                            entities = entities,
                            preMeasuredCamLabels = preMeasuredCamLabels,
                            stageLeft = stageLeft,
                            stageTop = stageTop,
                            sx = sx,
                            sy = sy
                        )
                    }

                    drawActiveCameraAndVignette(
                        stageCamera = stageCamera,
                        activeSlotIndex = activeSlot.index,
                        activeSlotZoom = activeSlotZoom,
                        resolvedCamX = resolvedCamX,
                        resolvedCamY = resolvedCamY,
                        isCameraSelected = isCameraSelected,
                        preMeasuredCamLabels = preMeasuredCamLabels,
                        stageLeft = stageLeft,
                        stageTop = stageTop,
                        sx = sx,
                        sy = sy
                    )
                }
            }

            HorizontalDivider(color = StudioColors.BorderSubtle)

            StageCanvasBottomBar(
                isCameraSelected = isCameraSelected,
                cameraSlots = cameraSlots,
                activeSlot = activeSlot,
                activeSlotX = activeSlotX,
                activeSlotY = activeSlotY,
                activeSlotZoom = activeSlotZoom,
                selected = selected,
                liveTransform = liveTransform,
                onSelectCameraSlot = onSelectCameraSlot,
                onUpdateCameraSlot = onUpdateCameraSlot,
                onUpdateEntity = onUpdateEntity
            )
        }
    }
}
