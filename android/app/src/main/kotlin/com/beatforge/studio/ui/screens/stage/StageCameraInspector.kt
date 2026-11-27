// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/StageCameraInspector.kt
// Propósito: Coordinador del inspector derecho para el Sistema Multi-Cámara del escenario
// (`Cam 0` fija inicial, `Cam 1` Player, `Cam 2` Enemigo y `Cam 3+` personalizadas).
// Delega sus tres pestañas en `ui/screens/stage/camera/` (`CameraSlotFovTab`,
// `CameraModeChartTab` y `CameraVignetteTab`).

package com.beatforge.studio.ui.screens.stage

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.CropFree
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material.icons.rounded.Videocam
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.screens.stage.camera.CameraModeChartTab
import com.beatforge.studio.ui.screens.stage.camera.CameraSlotFovTab
import com.beatforge.studio.ui.screens.stage.camera.CameraTabChip
import com.beatforge.studio.ui.screens.stage.camera.CameraVignetteTab
import com.beatforge.studio.ui.screens.stage.camera.cameraSlotAccentColor
import com.beatforge.studio.ui.screens.stage.camera.cameraSlotRoleShortTag
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

@Composable
fun StageCameraInspector(
    camera: StageCameraSnapshot,
    entities: List<StageEntitySnapshot>,
    stepPx: Int,
    liveTransform: StageLiveTransformState,
    onStepChange: (Int) -> Unit,
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
    onOpenCameraScript: () -> Unit
) {
    val slots = camera.resolvedCameras()
    val activeSlot = camera.activeSlot()

    var localZoom by remember(activeSlot.index) { mutableFloatStateOf(activeSlot.zoom) }
    var isDraggingZoom by remember { mutableStateOf(false) }
    var localSmooth by remember(activeSlot.index) { mutableFloatStateOf(activeSlot.smoothSpeed) }
    var isDraggingSmooth by remember { mutableStateOf(false) }
    var localPan by remember(activeSlot.index) { mutableFloatStateOf(activeSlot.panStrength) }
    var isDraggingPan by remember { mutableStateOf(false) }
    var localVignetteColor by remember { mutableStateOf(camera.vignetteColor) }
    var localVignetteAlpha by remember { mutableFloatStateOf(camera.vignetteAlpha) }
    var isDraggingVigAlpha by remember { mutableStateOf(false) }
    var localVignetteWidth by remember { mutableFloatStateOf(camera.vignetteWidth) }
    var isDraggingVigWidth by remember { mutableStateOf(false) }
    var localVignettePulse by remember { mutableFloatStateOf(camera.vignettePulse) }
    var isDraggingVigPulse by remember { mutableStateOf(false) }
    var activeTab by remember { mutableIntStateOf(0) } // 0 = Ranura & FOV, 1 = Modo & Chart, 2 = Viñeta

    LaunchedEffect(activeSlot.index, activeSlot.zoom) {
        if (!isDraggingZoom) localZoom = activeSlot.zoom
    }
    LaunchedEffect(activeSlot.index, activeSlot.smoothSpeed) {
        if (!isDraggingSmooth) localSmooth = activeSlot.smoothSpeed
    }
    LaunchedEffect(activeSlot.index, activeSlot.panStrength) {
        if (!isDraggingPan) localPan = activeSlot.panStrength
    }
    LaunchedEffect(camera.vignetteColor) {
        localVignetteColor = camera.vignetteColor
    }
    LaunchedEffect(camera.vignetteAlpha) {
        if (!isDraggingVigAlpha) localVignetteAlpha = camera.vignetteAlpha
    }
    LaunchedEffect(camera.vignetteWidth) {
        if (!isDraggingVigWidth) localVignetteWidth = camera.vignetteWidth
    }
    LaunchedEffect(camera.vignettePulse) {
        if (!isDraggingVigPulse) localVignettePulse = camera.vignettePulse
    }

    val useLiveSlot = liveTransform.isInteractingCamera && liveTransform.camSlotIndex == activeSlot.index
    val shownSlotX = if (useLiveSlot) liveTransform.camSlotX else activeSlot.x
    val shownSlotY = if (useLiveSlot) liveTransform.camSlotY else activeSlot.y
    val shownZoom = if (!isDraggingZoom && useLiveSlot) {
        liveTransform.camZoom
    } else {
        localZoom
    }.coerceIn(0.35f, 2.5f)

    val (previewX, previewY) = remember(camera, activeSlot, shownSlotX, shownSlotY, shownZoom, entities) {
        resolveCameraSlotPreviewCenter(
            camera = camera,
            slot = activeSlot.copy(x = shownSlotX, y = shownSlotY, zoom = shownZoom),
            entities = entities
        )
    }

    val captureW = (1280f / shownZoom).roundToInt()
    val captureH = (720f / shownZoom).roundToInt()

    fun commitGlobalCamera(
        enabled: Boolean = camera.enabled,
        mode: String = camera.followMode,
        beats: Int = camera.followIntervalBeats,
        clampToBackground: Boolean = camera.clampToBackground
    ) {
        onUpdateCamera(
            enabled,
            camera.baseX,
            camera.baseY,
            camera.baseZoom,
            mode,
            beats,
            camera.smoothSpeed,
            camera.panStrength,
            clampToBackground,
            true
        )
    }

    fun commitVignette(
        enabled: Boolean = camera.vignetteEnabled,
        colorHex: String = localVignetteColor,
        alpha: Float = localVignetteAlpha,
        width: Float = localVignetteWidth,
        pulse: Float = localVignettePulse,
        overHud: Boolean = camera.vignetteOverHud,
        save: Boolean = true
    ) {
        onUpdateCameraVignette(
            enabled,
            colorHex,
            alpha.coerceIn(0f, 1f),
            width.coerceIn(0.05f, 0.50f),
            pulse.coerceIn(0f, 1f),
            overHud,
            save
        )
    }

    fun commitActiveSlot(
        name: String = activeSlot.name,
        targetKind: String = activeSlot.targetKind,
        targetEntityId: String? = activeSlot.targetEntityId,
        sx: Float = shownSlotX,
        sy: Float = shownSlotY,
        zoom: Float = shownZoom,
        smooth: Float = localSmooth,
        pan: Float = localPan,
        save: Boolean = true
    ) {
        onUpdateCameraSlot(
            activeSlot.index,
            name,
            targetKind,
            targetEntityId,
            sx,
            sy,
            zoom,
            smooth,
            pan,
            save
        )
    }

    fun applyImmediateSlotZoom(nextZoom: Float, save: Boolean) {
        val clamped = nextZoom.coerceIn(0.35f, 2.5f)
        localZoom = clamped
        liveTransform.updateCameraSlotTransform(
            camera = camera,
            slot = activeSlot,
            entities = entities,
            nextSlotX = shownSlotX,
            nextSlotY = shownSlotY,
            nextZoom = clamped
        )
        if (save) {
            commitActiveSlot(zoom = clamped, save = true)
        }
    }

    val opponentEnt = remember(entities) { entities.firstOrNull { it.role == "opponent" } }
    val playerEnt = remember(entities) { entities.firstOrNull { it.role == "player" } }

    Column(
        modifier = Modifier.padding(9.dp),
        verticalArrangement = Arrangement.spacedBy(7.dp)
    ) {
        // Cabecera del sistema multi-cámara
        Surface(
            shape = RoundedCornerShape(9.dp),
            color = StudioColors.BgElevated,
            border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.45f))
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                Icon(
                    imageVector = Icons.Rounded.Videocam,
                    contentDescription = null,
                    tint = StudioColors.AccentCyan,
                    modifier = Modifier.size(17.dp)
                )
                Spacer(Modifier.width(6.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = "Sistema Multi-Cámara (${slots.size})",
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 12.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    Text(
                        text = activeSlot.name,
                        color = StudioColors.AccentCyan,
                        fontSize = 10.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
                if (activeSlot.index >= 3) {
                    IconButton(
                        onClick = { onRemoveCustomCamera(activeSlot.index) },
                        modifier = Modifier.size(28.dp)
                    ) {
                        Icon(
                            imageVector = Icons.Rounded.Delete,
                            contentDescription = "Eliminar cámara",
                            tint = StudioColors.AccentCoral,
                            modifier = Modifier.size(15.dp)
                        )
                    }
                }
                Switch(
                    checked = camera.enabled,
                    onCheckedChange = { nextEnabled -> commitGlobalCamera(enabled = nextEnabled) }
                )
            }
        }

        // Selector horizontal de ranuras: Cam 0..N + Nueva
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(5.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            slots.forEach { slot ->
                val isSel = slot.index == activeSlot.index
                val accent = cameraSlotAccentColor(slot)
                Row(
                    modifier = Modifier
                        .height(28.dp)
                        .clip(RoundedCornerShape(7.dp))
                        .background(if (isSel) accent.copy(alpha = 0.22f) else StudioColors.BgCard)
                        .border(
                            1.dp,
                            if (isSel) accent else StudioColors.BorderSubtle,
                            RoundedCornerShape(7.dp)
                        )
                        .clickable { onSelectCameraSlot(slot.index) }
                        .padding(horizontal = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    Text(
                        text = "Cam ${slot.index}",
                        color = if (isSel) accent else StudioColors.TextPrimary,
                        fontSize = 10.5.sp,
                        fontWeight = if (isSel) FontWeight.Bold else FontWeight.SemiBold,
                        maxLines = 1,
                        softWrap = false
                    )
                    Text(
                        text = cameraSlotRoleShortTag(slot),
                        color = StudioColors.TextMuted,
                        fontSize = 9.sp,
                        maxLines = 1,
                        softWrap = false
                    )
                }
            }

            Row(
                modifier = Modifier
                    .height(28.dp)
                    .clip(RoundedCornerShape(7.dp))
                    .background(StudioColors.AccentCyan.copy(alpha = 0.16f))
                    .border(1.dp, StudioColors.AccentCyan.copy(alpha = 0.65f), RoundedCornerShape(7.dp))
                    .clickable {
                        val nextIdx = slots.size
                        onAddCustomCamera(
                            "Cam $nextIdx · Custom",
                            "fixed",
                            null,
                            640f,
                            360f,
                            1.15f,
                            6f,
                            0.85f
                        )
                    }
                    .padding(horizontal = 8.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(3.dp)
            ) {
                Icon(Icons.Rounded.Add, null, tint = StudioColors.AccentCyan, modifier = Modifier.size(13.dp))
                Text(
                    text = "Cam",
                    color = StudioColors.AccentCyan,
                    fontSize = 10.5.sp,
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                    softWrap = false
                )
            }
        }

        // Pestañas ergonómicas: Ranura & FOV / Modo & Chart / Viñeta
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(5.dp)
        ) {
            CameraTabChip(
                label = "Ranura & FOV",
                icon = Icons.Rounded.CropFree,
                selected = activeTab == 0,
                onClick = { activeTab = 0 },
                modifier = Modifier.weight(1.1f)
            )
            CameraTabChip(
                label = "Modo",
                icon = Icons.Rounded.Tune,
                selected = activeTab == 1,
                onClick = { activeTab = 1 },
                modifier = Modifier.weight(0.85f)
            )
            CameraTabChip(
                label = "Viñeta",
                icon = Icons.Rounded.Palette,
                selected = activeTab == 2,
                onClick = { activeTab = 2 },
                modifier = Modifier.weight(0.9f)
            )
        }

        Column(
            modifier = Modifier
                .weight(1f)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            when (activeTab) {
                0 -> CameraSlotFovTab(
                    camera = camera,
                    activeSlot = activeSlot,
                    entities = entities,
                    stepPx = stepPx,
                    shownSlotX = shownSlotX,
                    shownSlotY = shownSlotY,
                    shownZoom = shownZoom,
                    localZoom = localZoom,
                    previewX = previewX,
                    previewY = previewY,
                    captureW = captureW,
                    captureH = captureH,
                    opponentEnt = opponentEnt,
                    playerEnt = playerEnt,
                    liveTransform = liveTransform,
                    onStepChange = onStepChange,
                    onZoomDragChanged = { isDraggingZoom = it },
                    onApplyImmediateSlotZoom = ::applyImmediateSlotZoom,
                    onCommitActiveSlot = { name, targetKind, targetEntityId, sx, sy, zoom, smooth, pan, save ->
                        commitActiveSlot(name, targetKind, targetEntityId, sx, sy, zoom, smooth, pan, save)
                    }
                )
                1 -> CameraModeChartTab(
                    camera = camera,
                    activeSlot = activeSlot,
                    localSmooth = localSmooth,
                    localPan = localPan,
                    onSmoothChange = { s, dragging ->
                        isDraggingSmooth = dragging
                        localSmooth = s
                    },
                    onPanChange = { p, dragging ->
                        isDraggingPan = dragging
                        localPan = p
                    },
                    onCommitGlobalCamera = { enabled, mode, beats, clampToBg ->
                        commitGlobalCamera(enabled, mode, beats, clampToBg)
                    },
                    onCommitSlotSmoothPan = { smooth, pan, save ->
                        commitActiveSlot(smooth = smooth, pan = pan, save = save)
                    },
                    onOpenCameraScript = onOpenCameraScript
                )
                else -> CameraVignetteTab(
                    camera = camera,
                    localVignetteColor = localVignetteColor,
                    localVignetteAlpha = localVignetteAlpha,
                    localVignetteWidth = localVignetteWidth,
                    localVignettePulse = localVignettePulse,
                    onVignetteColorChange = { localVignetteColor = it },
                    onVignetteAlphaChange = { a, dragging ->
                        isDraggingVigAlpha = dragging
                        localVignetteAlpha = a
                    },
                    onVignetteWidthChange = { w, dragging ->
                        isDraggingVigWidth = dragging
                        localVignetteWidth = w
                    },
                    onVignettePulseChange = { p, dragging ->
                        isDraggingVigPulse = dragging
                        localVignettePulse = p
                    },
                    onCommitVignette = { enabled, colorHex, alpha, width, pulse, overHud, save ->
                        commitVignette(enabled, colorHex, alpha, width, pulse, overHud, save)
                    },
                    onOpenCameraScript = onOpenCameraScript
                )
            }
        }
    }
}
