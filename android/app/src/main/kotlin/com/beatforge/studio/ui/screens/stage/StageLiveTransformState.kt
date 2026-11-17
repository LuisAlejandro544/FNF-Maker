// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/StageLiveTransformState.kt
// Propósito: Estado local observable a 60 Hz para arrastres, pellizco, manija de escala y sliders
// del Editor de Escena (`StageEditorScreen`). Permite que el lienzo (`Canvas`) e inspector se
// actualicen en tiempo real sin copiar `stageEntities` en `StudioUiState` ni disparar JNI/JSON
// hasta que el usuario suelta el gesto (`commitSave = true`).

package com.beatforge.studio.ui.screens.stage

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot

@Stable
class StageLiveTransformState {
    var entityId by mutableStateOf("")
    var isInteracting by mutableStateOf(false)
    var x by mutableFloatStateOf(640f)
    var y by mutableFloatStateOf(360f)
    var scale by mutableFloatStateOf(1f)
    var opacity by mutableFloatStateOf(1f)

    var isInteractingCamera by mutableStateOf(false)
    var camX by mutableFloatStateOf(640f)
    var camY by mutableFloatStateOf(360f)
    var camZoom by mutableFloatStateOf(1f)

    fun syncFromEntity(entity: StageEntitySnapshot?) {
        if (entity == null) {
            entityId = ""
            isInteracting = false
            return
        }
        val switched = entityId != entity.id
        if (switched || !isInteracting) {
            entityId = entity.id
            x = entity.x
            y = entity.y
            scale = entity.scale
            opacity = entity.opacity
            if (switched) {
                isInteracting = false
            }
        }
    }

    fun syncFromCamera(camera: StageCameraSnapshot) {
        if (!isInteractingCamera) {
            camX = camera.baseX
            camY = camera.baseY
            camZoom = camera.baseZoom
        }
    }

    fun beginInteraction(entity: StageEntitySnapshot) {
        if (entityId != entity.id) {
            syncFromEntity(entity)
        }
        isInteracting = true
    }

    fun beginCameraInteraction(camera: StageCameraSnapshot) {
        if (!isInteractingCamera) {
            camX = camera.baseX
            camY = camera.baseY
            camZoom = camera.baseZoom
        }
        isInteractingCamera = true
    }

    fun updatePositionAndScale(nextX: Float, nextY: Float, nextScale: Float) {
        x = nextX.coerceIn(-1000f, 2600f)
        y = nextY.coerceIn(-1000f, 1800f)
        scale = nextScale.coerceIn(0.1f, 5.0f)
    }

    fun updateCamera(nextX: Float, nextY: Float, nextZoom: Float) {
        camX = nextX.coerceIn(-1000f, 2600f)
        camY = nextY.coerceIn(-1000f, 1800f)
        camZoom = nextZoom.coerceIn(0.35f, 2.5f)
    }

    fun updateScale(nextScale: Float) {
        scale = nextScale.coerceIn(0.1f, 5.0f)
    }

    fun updateOpacity(nextOpacity: Float) {
        opacity = nextOpacity.coerceIn(0f, 1f)
    }

    fun endInteraction() {
        isInteracting = false
    }

    fun endCameraInteraction() {
        isInteractingCamera = false
    }
}
