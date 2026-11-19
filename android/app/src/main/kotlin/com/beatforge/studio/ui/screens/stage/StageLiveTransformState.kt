// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/StageLiveTransformState.kt
// Propósito: Estado local observable a 60 Hz para arrastres, pellizco, manija de escala y sliders
// del Editor de Escena (`StageEditorScreen`). También resuelve en tiempo real la vista previa
// multi-cámara (`Cam 0/1/2/3+`) sincronizando offsets, coordenadas fijas y FOV/zoom sin disparar
// llamadas JNI/JSON hasta finalizar el gesto (`commitSave = true`).

package com.beatforge.studio.ui.screens.stage

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.beatforge.studio.StageCameraSlotSnapshot
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot

/**
 * Calcula las coordenadas virtuales `(x, y)` reales en escenario (1280x720) donde apunta
 * una ranura de cámara (`Cam 0/1/2/3+`), replicando exactamente la mezcla `panStrength`
 * del motor Rust (`StageScene::resolve_slot_against_entities`).
 */
fun resolveCameraSlotPreviewCenter(
    camera: StageCameraSnapshot,
    slot: StageCameraSlotSnapshot,
    entities: List<StageEntitySnapshot>
): Pair<Float, Float> {
    val cam0 = camera.resolvedCameras().firstOrNull { it.targetKind == "fixed" }
    val cam0X = cam0?.x ?: camera.baseX
    val cam0Y = cam0?.y ?: camera.baseY

    if (slot.targetKind == "fixed") {
        return slot.x.coerceIn(-1000f, 2600f) to slot.y.coerceIn(-1000f, 1800f)
    }

    val anchor = when (slot.targetKind) {
        "player" -> entities.firstOrNull { it.role == "player" && it.visible }
            ?: entities.firstOrNull { it.role == "player" }
        "opponent" -> entities.firstOrNull { it.role == "opponent" && it.visible }
            ?: entities.firstOrNull { it.role == "opponent" }
        "gf" -> entities.firstOrNull {
            it.role == "decoration" && (
                it.id.equals("stage_speakers", ignoreCase = true) ||
                    it.id.contains("gf", ignoreCase = true) ||
                    it.displayName.contains("cabina", ignoreCase = true)
                )
        } ?: entities.firstOrNull { it.role == "decoration" && it.visible }
        "entity" -> slot.targetEntityId?.let { tid -> entities.firstOrNull { it.id == tid } }
            ?: entities.firstOrNull()
        else -> null
    }

    return if (anchor != null) {
        val destX = anchor.x + slot.x
        val destY = anchor.y + slot.y
        val pan = slot.panStrength.coerceIn(0f, 1f)
        val tx = (cam0X + (destX - cam0X) * pan).coerceIn(-1000f, 2600f)
        val ty = (cam0Y + (destY - cam0Y) * pan).coerceIn(-1000f, 1800f)
        tx to ty
    } else {
        (cam0X + slot.x).coerceIn(-1000f, 2600f) to (cam0Y + slot.y).coerceIn(-1000f, 1800f)
    }
}

@Stable
class StageLiveTransformState {
    var entityId by mutableStateOf("")
    var isInteracting by mutableStateOf(false)
    var x by mutableFloatStateOf(640f)
    var y by mutableFloatStateOf(360f)
    var scale by mutableFloatStateOf(1f)
    var opacity by mutableFloatStateOf(1f)

    var isInteractingCamera by mutableStateOf(false)
    var camSlotIndex by mutableIntStateOf(0)
    var camSlotX by mutableFloatStateOf(640f)
    var camSlotY by mutableFloatStateOf(360f)
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

    fun syncFromCamera(camera: StageCameraSnapshot, entities: List<StageEntitySnapshot>) {
        val activeSlot = camera.activeSlot()
        val switchedSlot = camSlotIndex != activeSlot.index
        if (switchedSlot || !isInteractingCamera) {
            camSlotIndex = activeSlot.index
            camSlotX = activeSlot.x
            camSlotY = activeSlot.y
            val (rx, ry) = resolveCameraSlotPreviewCenter(camera, activeSlot, entities)
            camX = rx
            camY = ry
            camZoom = activeSlot.zoom.coerceIn(0.35f, 2.5f)
            if (switchedSlot) {
                isInteractingCamera = false
            }
        }
    }

    fun beginInteraction(entity: StageEntitySnapshot) {
        if (entityId != entity.id) {
            syncFromEntity(entity)
        }
        isInteracting = true
    }

    fun beginCameraInteraction(camera: StageCameraSnapshot, entities: List<StageEntitySnapshot>) {
        val activeSlot = camera.activeSlot()
        if (!isInteractingCamera || camSlotIndex != activeSlot.index) {
            syncFromCamera(camera, entities)
        }
        isInteractingCamera = true
    }

    fun updatePositionAndScale(nextX: Float, nextY: Float, nextScale: Float) {
        x = nextX.coerceIn(-1000f, 2600f)
        y = nextY.coerceIn(-1000f, 1800f)
        scale = nextScale.coerceIn(0.1f, 5.0f)
    }

    fun updateScale(nextScale: Float) {
        scale = nextScale.coerceIn(0.1f, 5.0f)
    }

    fun updateOpacity(nextOpacity: Float) {
        opacity = nextOpacity.coerceIn(0f, 1f)
    }

    fun updateCameraSlotTransform(
        camera: StageCameraSnapshot,
        slot: StageCameraSlotSnapshot,
        entities: List<StageEntitySnapshot>,
        nextSlotX: Float,
        nextSlotY: Float,
        nextZoom: Float
    ) {
        camSlotIndex = slot.index
        camSlotX = nextSlotX.coerceIn(-1200f, 2600f)
        camSlotY = nextSlotY.coerceIn(-1200f, 1800f)
        val previewSlot = slot.copy(x = camSlotX, y = camSlotY, zoom = nextZoom.coerceIn(0.35f, 2.5f))
        val (rx, ry) = resolveCameraSlotPreviewCenter(camera, previewSlot, entities)
        camX = rx
        camY = ry
        camZoom = previewSlot.zoom
    }

    fun endInteraction() {
        isInteracting = false
    }

    fun endCameraInteraction() {
        isInteractingCamera = false
    }
}
