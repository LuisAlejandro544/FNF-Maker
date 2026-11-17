// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/StageEditorScreen.kt
// Propósito: Coordinador del editor visual de escenario en 3 columnas modulares:
// capas Z-order virtualizadas (`StageLayersPanel`), lienzo interactivo 16:9 con barras rápidas
// (`StageCanvasPreview`) e inspector con pestañas (`StageEntityInspector` / `StageCameraInspector`)
// sincronizados con `StageLiveTransformState` para fluidez de 60 FPS en pantalla horizontal.

package com.beatforge.studio.ui.screens

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.CAMERA_SELECTION_ID
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.ProjectSnapshot
import com.beatforge.studio.ui.screens.stage.StageCameraInspector
import com.beatforge.studio.ui.screens.stage.StageCanvasPreview
import com.beatforge.studio.ui.screens.stage.StageEntityInspector
import com.beatforge.studio.ui.screens.stage.StageLayersPanel
import com.beatforge.studio.ui.screens.stage.StageLiveTransformState
import com.beatforge.studio.ui.theme.StudioColors

@Composable
fun StageEditorScreen(
    snapshot: ProjectSnapshot,
    selectedEntityId: String,
    entityBitmaps: Map<String, LoadedEntityBitmap>,
    onSelectEntity: (String) -> Unit,
    onAddEntity: (name: String, role: String) -> Unit,
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
    onReorderEntity: (entityId: String, action: String) -> Unit,
    onRequestImportPng: (entityId: String) -> Unit,
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
    val entities = snapshot.stageEntities
    val stageCamera = snapshot.stageCamera
    val isCameraSelected = selectedEntityId == CAMERA_SELECTION_ID
    val selected = remember(entities, selectedEntityId, isCameraSelected) {
        if (isCameraSelected) null else (entities.find { it.id == selectedEntityId } ?: entities.firstOrNull())
    }
    var stepPx by remember { mutableIntStateOf(10) }
    var showGrid by remember { mutableStateOf(true) }
    var wideView by remember { mutableStateOf(false) }
    var lockedEntityIds by remember { mutableStateOf(emptySet<String>()) }
    val liveTransform = remember { StageLiveTransformState() }

    fun toggleLock(id: String) {
        lockedEntityIds = if (lockedEntityIds.contains(id)) {
            lockedEntityIds - id
        } else {
            lockedEntityIds + id
        }
    }

    LaunchedEffect(
        selected?.id,
        selected?.x,
        selected?.y,
        selected?.scale,
        selected?.opacity
    ) {
        liveTransform.syncFromEntity(selected)
    }

    LaunchedEffect(
        stageCamera.baseX,
        stageCamera.baseY,
        stageCamera.baseZoom
    ) {
        liveTransform.syncFromCamera(stageCamera)
    }

    Row(
        modifier = Modifier
            .fillMaxSize()
            .padding(8.dp),
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        StageLayersPanel(
            entities = entities,
            selected = selected,
            stageCamera = stageCamera,
            isCameraSelected = isCameraSelected,
            lockedEntityIds = lockedEntityIds,
            onSelectEntity = onSelectEntity,
            onSelectCamera = { onSelectEntity(CAMERA_SELECTION_ID) },
            onToggleCameraEnabled = {
                onUpdateCamera(
                    !stageCamera.enabled,
                    stageCamera.baseX,
                    stageCamera.baseY,
                    stageCamera.baseZoom,
                    stageCamera.followMode,
                    stageCamera.followIntervalBeats,
                    stageCamera.smoothSpeed,
                    stageCamera.panStrength,
                    true
                )
            },
            onAddEntity = onAddEntity,
            onToggleVisibility = { ent ->
                onUpdateEntity(
                    ent.id,
                    ent.displayName,
                    ent.role,
                    0f,
                    0f,
                    ent.x,
                    ent.y,
                    false,
                    ent.scale,
                    ent.opacity,
                    !ent.visible,
                    ent.flipX,
                    true
                )
            },
            onReorderEntity = onReorderEntity,
            modifier = Modifier.weight(0.22f)
        )

        StageCanvasPreview(
            entities = entities,
            selected = selected,
            stageCamera = stageCamera,
            isCameraSelected = isCameraSelected,
            showGrid = showGrid,
            wideView = wideView,
            lockedEntityIds = lockedEntityIds,
            entityBitmaps = entityBitmaps,
            liveTransform = liveTransform,
            onToggleGrid = { showGrid = !showGrid },
            onToggleWideView = { wideView = !wideView },
            onToggleSelectedLock = { selected?.let { toggleLock(it.id) } },
            onSelectEntity = onSelectEntity,
            onSelectCamera = { onSelectEntity(CAMERA_SELECTION_ID) },
            onUpdateEntity = onUpdateEntity,
            onUpdateCamera = { bx, by, bz, commit ->
                onUpdateCamera(
                    stageCamera.enabled,
                    bx,
                    by,
                    bz,
                    stageCamera.followMode,
                    stageCamera.followIntervalBeats,
                    stageCamera.smoothSpeed,
                    stageCamera.panStrength,
                    commit
                )
            },
            modifier = Modifier.weight(0.47f)
        )

        // Columna derecha: Inspector de Cámara o del objeto seleccionado
        Surface(
            modifier = Modifier
                .weight(0.31f)
                .fillMaxHeight(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            if (isCameraSelected) {
                StageCameraInspector(
                    camera = stageCamera,
                    entities = entities,
                    stepPx = stepPx,
                    liveTransform = liveTransform,
                    onStepChange = { stepPx = it },
                    onUpdateCamera = onUpdateCamera,
                    onOpenCameraScript = onOpenCameraScript
                )
            } else if (selected == null) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text("Selecciona una capa", color = StudioColors.TextMuted, fontSize = 12.sp)
                }
            } else {
                StageEntityInspector(
                    entity = selected,
                    stepPx = stepPx,
                    isLocked = lockedEntityIds.contains(selected.id),
                    liveTransform = liveTransform,
                    onStepChange = { stepPx = it },
                    onToggleLock = { toggleLock(selected.id) },
                    onUpdate = onUpdateEntity,
                    onRequestImportPng = { onRequestImportPng(selected.id) }
                )
            }
        }
    }
}
