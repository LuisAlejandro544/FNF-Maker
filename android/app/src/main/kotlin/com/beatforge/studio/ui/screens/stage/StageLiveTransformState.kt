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
 * Calcula el rectángulo delimitador `[minX, minY, maxX, maxY]` cubierto por las capas
 * de fondo del escenario, replicando `StageScene::compute_background_bounds` en Rust.
 */
fun resolveStageBackgroundBounds(entities: List<StageEntitySnapshot>): FloatArray? {
    var candidates = entities.filter { it.role == "background" && it.visible && it.opacity > 0.01f }
    if (candidates.isEmpty()) {
        candidates = entities.filter { it.role == "background" }
    }
    if (candidates.isEmpty()) return null

    val largeOnly = candidates.filter {
        kotlin.math.abs(it.width * it.scale) >= 320f && kotlin.math.abs(it.height * it.scale) >= 180f
    }
    val chosen = largeOnly.ifEmpty { candidates }

    var minX = Float.POSITIVE_INFINITY
    var minY = Float.POSITIVE_INFINITY
    var maxX = Float.NEGATIVE_INFINITY
    var maxY = Float.NEGATIVE_INFINITY
    for (bg in chosen) {
        val hw = kotlin.math.abs(bg.width * bg.scale) * 0.5f
        val hh = kotlin.math.abs(bg.height * bg.scale) * 0.5f
        minX = minOf(minX, bg.x - hw)
        maxX = maxOf(maxX, bg.x + hw)
        minY = minOf(minY, bg.y - hh)
        maxY = maxOf(maxY, bg.y + hh)
    }

    return if (minX.isFinite() && minY.isFinite() && maxX > minX && maxY > minY) {
        floatArrayOf(minX, minY, maxX, maxY)
    } else {
        null
    }
}

/**
 * Restringe `(x, y)` al área cubierta por el fondo del escenario según el `zoom` activo,
 * evitando mostrar márgenes negros fuera del escenario.
 */
fun clampCameraToBackground(
    x: Float,
    y: Float,
    zoom: Float,
    bounds: FloatArray?,
    virtualW: Float = 1280f,
    virtualH: Float = 720f
): Pair<Float, Float> {
    val cx = if (x.isFinite()) x.coerceIn(-1000f, 2600f) else 640f
    val cy = if (y.isFinite()) y.coerceIn(-1000f, 1800f) else 360f
    if (bounds == null || bounds.size < 4) return cx to cy

    val minX = bounds[0]
    val minY = bounds[1]
    val maxX = bounds[2]
    val maxY = bounds[3]
    val z = if (zoom.isFinite()) zoom.coerceIn(0.35f, 2.5f) else 1f
    val halfVw = (virtualW.coerceAtLeast(320f) * 0.5f) / z
    val halfVh = (virtualH.coerceAtLeast(180f) * 0.5f) / z

    val clampedX = if (minX + halfVw <= maxX - halfVw) {
        cx.coerceIn(minX + halfVw, maxX - halfVw)
    } else {
        (minX + maxX) * 0.5f
    }
    val clampedY = if (minY + halfVh <= maxY - halfVh) {
        cy.coerceIn(minY + halfVh, maxY - halfVh)
    } else {
        (minY + maxY) * 0.5f
    }
    return clampedX.coerceIn(-1000f, 2600f) to clampedY.coerceIn(-1000f, 1800f)
}

/**
 * Calcula las coordenadas virtuales `(x, y)` reales en escenario (1280x720) donde apunta
 * una ranura de cámara (`Cam 0/1/2/3+`), replicando exactamente la mezcla `panStrength`
 * y el recorte al fondo del motor Rust (`StageScene::resolve_slot_against_entities`).
 */
fun resolveCameraSlotPreviewCenter(
    camera: StageCameraSnapshot,
    slot: StageCameraSlotSnapshot,
    entities: List<StageEntitySnapshot>
): Pair<Float, Float> {
    val cam0 = camera.resolvedCameras().firstOrNull { it.targetKind == "fixed" }
    val cam0X = cam0?.x ?: camera.baseX
    val cam0Y = cam0?.y ?: camera.baseY
    val slotZoom = slot.zoom.coerceIn(0.35f, 2.5f)

    val (rawX, rawY) = if (slot.targetKind == "fixed") {
        slot.x.coerceIn(-1000f, 2600f) to slot.y.coerceIn(-1000f, 1800f)
    } else {
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

        if (anchor != null) {
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

    return if (camera.clampToBackground) {
        clampCameraToBackground(rawX, rawY, slotZoom, resolveStageBackgroundBounds(entities))
    } else {
        rawX to rawY
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
