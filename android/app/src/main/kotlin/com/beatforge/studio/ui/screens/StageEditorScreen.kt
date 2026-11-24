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
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.ui.graphics.Color
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
import com.beatforge.studio.StageEntitySnapshot
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
    onRemoveEntity: (entityId: String) -> Unit,
    onClearScene: () -> Unit,
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
        clampToBackground: Boolean,
        commitSave: Boolean
    ) -> Unit,
    onUpdateCameraVignette: (
        enabled: Boolean,
        colorHex: String,
        alpha: Float,
        width: Float,
        pulse: Float,
        overHud: Boolean,
        commitSave: Boolean
    ) -> Unit,
    onSelectCameraSlot: (slotIndex: Int) -> Unit,
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
    onAddCustomCamera: (
        name: String,
        targetKind: String,
        targetEntityId: String?,
        x: Float,
        y: Float,
        zoom: Float,
        smoothSpeed: Float,
        panStrength: Float
    ) -> Unit,
    onRemoveCustomCamera: (slotIndex: Int) -> Unit,
    onOpenCameraScript: () -> Unit,
    onImportStageConfig: () -> Unit,
    onOpenFilesGuide: () -> Unit
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
    var pendingDeleteEntity by remember { mutableStateOf<StageEntitySnapshot?>(null) }
    var confirmClearScene by remember { mutableStateOf(false) }
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
        stageCamera,
        entities
    ) {
        liveTransform.syncFromCamera(stageCamera, entities)
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
                    stageCamera.clampToBackground,
                    true
                )
            },
            onAddEntity = onAddEntity,
            onImportStageConfig = onImportStageConfig,
            onOpenFilesGuide = onOpenFilesGuide,
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
            onRemoveEntity = { ent -> pendingDeleteEntity = ent },
            onClearScene = {
                if (entities.isNotEmpty()) {
                    confirmClearScene = true
                }
            },
            modifier = Modifier.weight(0.24f)
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
            onSelectCameraSlot = onSelectCameraSlot,
            onUpdateEntity = onUpdateEntity,
            onUpdateCameraSlot = onUpdateCameraSlot,
            modifier = Modifier.weight(0.45f)
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
                    onUpdateCameraVignette = onUpdateCameraVignette,
                    onSelectCameraSlot = onSelectCameraSlot,
                    onUpdateCameraSlot = onUpdateCameraSlot,
                    onAddCustomCamera = onAddCustomCamera,
                    onRemoveCustomCamera = onRemoveCustomCamera,
                    onOpenCameraScript = onOpenCameraScript
                )
            } else if (selected == null) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text(
                        text = if (entities.isEmpty()) "Escena vacía · Usa + Objeto" else "Selecciona una capa",
                        color = StudioColors.TextMuted,
                        fontSize = 12.sp
                    )
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
                    onRequestImportPng = { onRequestImportPng(selected.id) },
                    onRemoveEntity = { pendingDeleteEntity = selected }
                )
            }
        }
    }

    pendingDeleteEntity?.let { target ->
        AlertDialog(
            onDismissRequest = { pendingDeleteEntity = null },
            containerColor = StudioColors.BgPanel,
            title = {
                Text(
                    text = "¿Eliminar capa de la escena?",
                    color = StudioColors.TextPrimary,
                    fontSize = 15.sp
                )
            },
            text = {
                Text(
                    text = "Se quitará \"${target.displayName}\" (${target.roleLabel}) de la escena actual y se limpiará cualquier enfoque directo de cámara a esta capa.",
                    color = StudioColors.TextMuted,
                    fontSize = 12.sp
                )
            },
            confirmButton = {
                Button(
                    onClick = {
                        pendingDeleteEntity = null
                        onRemoveEntity(target.id)
                    },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = StudioColors.AccentCoral,
                        contentColor = Color.White
                    )
                ) {
                    Text("Eliminar", fontSize = 12.sp)
                }
            },
            dismissButton = {
                OutlinedButton(onClick = { pendingDeleteEntity = null }) {
                    Text("Cancelar", color = StudioColors.TextPrimary, fontSize = 12.sp)
                }
            }
        )
    }

    if (confirmClearScene) {
        AlertDialog(
            onDismissRequest = { confirmClearScene = false },
            containerColor = StudioColors.BgPanel,
            title = {
                Text(
                    text = "¿Vaciar toda la escena?",
                    color = StudioColors.TextPrimary,
                    fontSize = 15.sp
                )
            },
            text = {
                Text(
                    text = "Se eliminarán las ${entities.size} capas de la escena actual para que puedas armarla desde cero. Las pistas de audio, chart y scripts se conservarán intactos.",
                    color = StudioColors.TextMuted,
                    fontSize = 12.sp
                )
            },
            confirmButton = {
                Button(
                    onClick = {
                        confirmClearScene = false
                        onClearScene()
                    },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = StudioColors.AccentCoral,
                        contentColor = Color.White
                    )
                ) {
                    Text("Vaciar escena", fontSize = 12.sp)
                }
            },
            dismissButton = {
                OutlinedButton(onClick = { confirmClearScene = false }) {
                    Text("Cancelar", color = StudioColors.TextPrimary, fontSize = 12.sp)
                }
            }
        )
    }
}
