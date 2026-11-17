// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/StageEditorScreen.kt
// Propósito: Coordinador del editor visual de escenario en 3 columnas modulares:
// capas Z-order (`StageLayersPanel`), lienzo interactivo 16:9 (`StageCanvasPreview`)
// e inspector de propiedades (`StageEntityInspector`).

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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.ProjectSnapshot
import com.beatforge.studio.ui.screens.stage.StageCanvasPreview
import com.beatforge.studio.ui.screens.stage.StageEntityInspector
import com.beatforge.studio.ui.screens.stage.StageLayersPanel
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
        flipX: Boolean
    ) -> Unit,
    onReorderEntity: (entityId: String, action: String) -> Unit,
    onRequestImportPng: (entityId: String) -> Unit
) {
    val entities = snapshot.stageEntities
    val selected = entities.find { it.id == selectedEntityId } ?: entities.firstOrNull()
    var stepPx by remember { mutableIntStateOf(10) }

    Row(
        modifier = Modifier
            .fillMaxSize()
            .padding(10.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        StageLayersPanel(
            entities = entities,
            selected = selected,
            onSelectEntity = onSelectEntity,
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
                    ent.flipX
                )
            },
            onReorderEntity = onReorderEntity,
            modifier = Modifier.weight(0.24f)
        )

        StageCanvasPreview(
            entities = entities,
            selected = selected,
            entityBitmaps = entityBitmaps,
            onSelectEntity = onSelectEntity,
            onUpdateEntity = onUpdateEntity,
            modifier = Modifier.weight(0.43f)
        )

        // Columna derecha: Inspector del objeto seleccionado
        Surface(
            modifier = Modifier
                .weight(0.33f)
                .fillMaxHeight(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            if (selected == null) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text("Selecciona una capa", color = StudioColors.TextMuted, fontSize = 12.sp)
                }
            } else {
                StageEntityInspector(
                    entity = selected,
                    stepPx = stepPx,
                    onStepChange = { stepPx = it },
                    onUpdate = onUpdateEntity,
                    onRequestImportPng = { onRequestImportPng(selected.id) }
                )
            }
        }
    }
}
