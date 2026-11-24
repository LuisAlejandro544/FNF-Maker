// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/StageCameraInspector.kt
// Propósito: Inspector derecho modular para configurar el Sistema Multi-Cámara del escenario
// (`Cam 0` fija inicial al arrancar el mod, `Cam 1` Player, `Cam 2` Enemigo y `Cam 3+` personalizadas).
// Organizado en dos pestañas (`Ranura & FOV` y `Modo & Chart`) para ajustar FOV/zoom, objetivo,
// offset/coordenadas, suavizado por cámara, modo de seguimiento del `chart.json` y `camera_system.lua`.

package com.beatforge.studio.ui.screens.stage

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.ArrowBack
import androidx.compose.material.icons.rounded.ArrowDownward
import androidx.compose.material.icons.rounded.ArrowForward
import androidx.compose.material.icons.rounded.ArrowUpward
import androidx.compose.material.icons.rounded.Code
import androidx.compose.material.icons.rounded.CropFree
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material.icons.rounded.Videocam
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
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
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StageCameraSlotSnapshot
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

private val CAMERA_TARGET_OPTIONS = listOf(
    "fixed" to "Fija (X,Y)",
    "player" to "Player",
    "opponent" to "Enemigo",
    "gf" to "Cabina/GF",
    "entity" to "Entidad"
)

private val VIGNETTE_PRESET_COLORS = listOf(
    "#EF4444" to "Rojo",
    "#DC2626" to "Carmesí",
    "#3B82F6" to "Azul",
    "#06B6D4" to "Cian",
    "#A855F7" to "Morado",
    "#F59E0B" to "Ámbar",
    "#10B981" to "Verde",
    "#09090B" to "Negro"
)

@OptIn(ExperimentalLayoutApi::class)
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

        // Selector horizontal de ranuras: Cam 0 (Fija), Cam 1 (Player), Cam 2 (Enemigo), Cam 3+ (+ Nueva)
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
            if (activeTab == 0) {
                val roleHint = when (activeSlot.index) {
                    0 -> "Cámara inicial del mod: permanece fija hasta que una sección, evento del chart o script mueva el enfoque."
                    1 -> "Cámara del Player (Jugador): ajusta su FOV y encuadre relativo al personaje."
                    2 -> "Cámara del Enemigo (Rival): ajusta su FOV y encuadre relativo al rival."
                    else -> "Cámara extra personalizada: asígnala por sección o en cualquier paso # del chart."
                }
                Text(
                    text = roleHint,
                    color = StudioColors.TextMuted,
                    fontSize = 9.5.sp,
                    lineHeight = 12.5.sp
                )

                // Objetivo de la ranura activa
                Text("Objetivo de Cam ${activeSlot.index}", color = StudioColors.TextMuted, fontSize = 10.5.sp)
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    CAMERA_TARGET_OPTIONS.forEach { (kindKey, label) ->
                        val isSel = activeSlot.targetKind == kindKey
                        Box(
                            modifier = Modifier
                                .height(26.dp)
                                .clip(RoundedCornerShape(6.dp))
                                .background(if (isSel) StudioColors.AccentCyan.copy(alpha = 0.22f) else StudioColors.BgCard)
                                .border(
                                    1.dp,
                                    if (isSel) StudioColors.AccentCyan else StudioColors.BorderSubtle,
                                    RoundedCornerShape(6.dp)
                                )
                                .clickable {
                                    val defaultId = when (kindKey) {
                                        "player" -> entities.firstOrNull { it.role == "player" }?.id ?: "player_main"
                                        "opponent" -> entities.firstOrNull { it.role == "opponent" }?.id ?: "opponent_main"
                                        "gf" -> entities.firstOrNull { it.id == "stage_speakers" }?.id ?: "stage_speakers"
                                        "entity" -> activeSlot.targetEntityId ?: entities.firstOrNull()?.id
                                        else -> null
                                    }
                                    val defaultX = if (kindKey == "fixed") previewX else 0f
                                    val defaultY = if (kindKey == "fixed") previewY else -24f
                                    val defaultPan = if (kindKey == "fixed") 0f else activeSlot.panStrength.takeIf { it > 0.05f } ?: 0.85f
                                    val updatedLabel = when (activeSlot.index) {
                                        0 -> if (kindKey == "fixed") "Cam 0 · Inicial Fija" else "Cam 0 · $label"
                                        1 -> if (kindKey == "player") "Cam 1 · Player" else "Cam 1 · $label"
                                        2 -> if (kindKey == "opponent") "Cam 2 · Enemigo" else "Cam 2 · $label"
                                        else -> "Cam ${activeSlot.index} · $label"
                                    }
                                    commitActiveSlot(
                                        name = updatedLabel,
                                        targetKind = kindKey,
                                        targetEntityId = defaultId,
                                        sx = defaultX,
                                        sy = defaultY,
                                        pan = defaultPan
                                    )
                                }
                                .padding(horizontal = 8.dp),
                            contentAlignment = Alignment.Center
                        ) {
                            Text(
                                text = label,
                                color = if (isSel) StudioColors.AccentCyan else StudioColors.TextPrimary,
                                fontSize = 10.sp,
                                fontWeight = if (isSel) FontWeight.Bold else FontWeight.Medium,
                                maxLines = 1,
                                softWrap = false
                            )
                        }
                    }
                }

                if (activeSlot.targetKind == "entity" && entities.isNotEmpty()) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .horizontalScroll(rememberScrollState()),
                        horizontalArrangement = Arrangement.spacedBy(4.dp)
                    ) {
                        entities.forEach { ent ->
                            val isSel = activeSlot.targetEntityId == ent.id
                            Box(
                                modifier = Modifier
                                    .height(25.dp)
                                    .clip(RoundedCornerShape(6.dp))
                                    .background(if (isSel) StudioColors.AccentPurple.copy(alpha = 0.22f) else StudioColors.BgCard)
                                    .border(
                                        1.dp,
                                        if (isSel) StudioColors.AccentPurple else StudioColors.BorderSubtle,
                                        RoundedCornerShape(6.dp)
                                    )
                                    .clickable {
                                        commitActiveSlot(
                                            targetEntityId = ent.id,
                                            name = "Cam ${activeSlot.index} · ${ent.displayName}"
                                        )
                                    }
                                    .padding(horizontal = 7.dp),
                                contentAlignment = Alignment.Center
                            ) {
                                Text(
                                    text = ent.displayName,
                                    color = if (isSel) StudioColors.AccentPurple else StudioColors.TextPrimary,
                                    fontSize = 9.5.sp,
                                    maxLines = 1,
                                    softWrap = false
                                )
                            }
                        }
                    }
                }

                // Campo de visión (FOV / Zoom) de esta cámara
                Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text("FOV / Captura ${captureW}x${captureH}", color = StudioColors.TextMuted, fontSize = 11.sp)
                        Text(
                            text = "Zoom ${((shownZoom * 100f).roundToInt() / 100f)}x",
                            color = StudioColors.AccentCyan,
                            fontSize = 11.sp,
                            fontWeight = FontWeight.Bold
                        )
                    }
                    Slider(
                        value = shownZoom.coerceIn(0.35f, 2.5f),
                        onValueChange = { z ->
                            isDraggingZoom = true
                            liveTransform.beginCameraInteraction(camera, entities)
                            applyImmediateSlotZoom(z, save = false)
                        },
                        onValueChangeFinished = {
                            isDraggingZoom = false
                            liveTransform.endCameraInteraction()
                            applyImmediateSlotZoom(localZoom, save = true)
                        },
                        valueRange = 0.35f..2.5f,
                        colors = SliderDefaults.colors(
                            thumbColor = StudioColors.AccentCyan,
                            activeTrackColor = StudioColors.AccentCyan
                        )
                    )
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(5.dp)
                    ) {
                        CameraStepChip(
                            label = "- 0.1x",
                            onClick = { applyImmediateSlotZoom(localZoom - 0.1f, save = true) },
                            modifier = Modifier.weight(1f)
                        )
                        CameraStepChip(
                            label = "1.0x",
                            onClick = { applyImmediateSlotZoom(1.0f, save = true) },
                            modifier = Modifier.weight(1f)
                        )
                        CameraStepChip(
                            label = "+ 0.1x",
                            onClick = { applyImmediateSlotZoom(localZoom + 0.1f, save = true) },
                            modifier = Modifier.weight(1f)
                        )
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    val coordLabel = if (activeSlot.targetKind == "fixed") {
                        "Centro (${shownSlotX.roundToInt()}, ${shownSlotY.roundToInt()})"
                    } else {
                        "Offset (${shownSlotX.roundToInt()}, ${shownSlotY.roundToInt()}) · Vista (${previewX.roundToInt()}, ${previewY.roundToInt()})"
                    }
                    Text(
                        text = coordLabel,
                        color = StudioColors.TextPrimary,
                        fontSize = 10.5.sp,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f)
                    )
                    Spacer(Modifier.width(4.dp))
                    Text(
                        text = "Paso ${stepPx}px",
                        color = StudioColors.AccentEmerald,
                        fontSize = 10.sp,
                        fontWeight = FontWeight.Bold
                    )
                }

                Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
                    listOf(1, 10, 35).forEach { px ->
                        val isSel = stepPx == px
                        Box(
                            modifier = Modifier
                                .weight(1f)
                                .height(27.dp)
                                .clip(RoundedCornerShape(6.dp))
                                .background(if (isSel) StudioColors.AccentEmerald.copy(alpha = 0.22f) else StudioColors.BgCard)
                                .border(
                                    1.dp,
                                    if (isSel) StudioColors.AccentEmerald else StudioColors.BorderSubtle,
                                    RoundedCornerShape(6.dp)
                                )
                                .clickable { onStepChange(px) },
                            contentAlignment = Alignment.Center
                        ) {
                            Text(
                                text = "${px}px",
                                color = if (isSel) StudioColors.AccentEmerald else StudioColors.TextPrimary,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.SemiBold,
                                maxLines = 1,
                                softWrap = false
                            )
                        }
                    }
                }

                Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
                    IconButton(
                        onClick = { commitActiveSlot(sx = shownSlotX, sy = shownSlotY - stepPx) },
                        modifier = Modifier.size(32.dp)
                    ) {
                        Icon(Icons.Rounded.ArrowUpward, "Arriba", tint = StudioColors.AccentCyan)
                    }
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        IconButton(
                            onClick = { commitActiveSlot(sx = shownSlotX - stepPx, sy = shownSlotY) },
                            modifier = Modifier.size(32.dp)
                        ) {
                            Icon(Icons.Rounded.ArrowBack, "Izquierda", tint = StudioColors.AccentCyan)
                        }
                        IconButton(
                            onClick = {
                                if (activeSlot.targetKind == "fixed") {
                                    commitActiveSlot(sx = 640f, sy = 360f)
                                } else {
                                    commitActiveSlot(sx = 0f, sy = -24f)
                                }
                            },
                            modifier = Modifier
                                .size(30.dp)
                                .clip(RoundedCornerShape(8.dp))
                                .background(StudioColors.BgCard)
                        ) {
                            Icon(Icons.Rounded.MyLocation, "Restablecer centro", tint = StudioColors.TextMuted, modifier = Modifier.size(16.dp))
                        }
                        IconButton(
                            onClick = { commitActiveSlot(sx = shownSlotX + stepPx, sy = shownSlotY) },
                            modifier = Modifier.size(32.dp)
                        ) {
                            Icon(Icons.Rounded.ArrowForward, "Derecha", tint = StudioColors.AccentCyan)
                        }
                    }
                    IconButton(
                        onClick = { commitActiveSlot(sx = shownSlotX, sy = shownSlotY + stepPx) },
                        modifier = Modifier.size(32.dp)
                    ) {
                        Icon(Icons.Rounded.ArrowDownward, "Abajo", tint = StudioColors.AccentCyan)
                    }
                }

                if (activeSlot.targetKind == "fixed") {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(5.dp)
                    ) {
                        CameraStepChip(
                            label = "Enemigo",
                            onClick = {
                                val tx = opponentEnt?.x ?: 300f
                                val ty = (opponentEnt?.y ?: 415f) - 24f
                                commitActiveSlot(sx = tx, sy = ty)
                            },
                            modifier = Modifier.weight(1f)
                        )
                        CameraStepChip(
                            label = "Centro",
                            onClick = { commitActiveSlot(sx = 640f, sy = 360f) },
                            modifier = Modifier.weight(1f)
                        )
                        CameraStepChip(
                            label = "Jugador",
                            onClick = {
                                val tx = playerEnt?.x ?: 980f
                                val ty = (playerEnt?.y ?: 420f) - 24f
                                commitActiveSlot(sx = tx, sy = ty)
                            },
                            modifier = Modifier.weight(1f)
                        )
                    }
                } else {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(5.dp)
                    ) {
                        CameraStepChip(
                            label = "Encuadre (0, -24)",
                            onClick = { commitActiveSlot(sx = 0f, sy = -24f) },
                            modifier = Modifier.weight(1f)
                        )
                        CameraStepChip(
                            label = "Cabeza (0, -90)",
                            onClick = { commitActiveSlot(sx = 0f, sy = -90f) },
                            modifier = Modifier.weight(1f)
                        )
                    }
                }
            } else if (activeTab == 1) {
                // Pestaña 2: Modo de seguimiento del chart, Intervalo, Suavizado de ranura y Script Lua
                Button(
                    onClick = onOpenCameraScript,
                    contentPadding = PaddingValues(horizontal = 10.dp, vertical = 5.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = StudioColors.AccentPurple,
                        contentColor = Color.White
                    ),
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(34.dp)
                ) {
                    Icon(Icons.Rounded.Code, null, modifier = Modifier.size(15.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("Editar camera_system.lua", fontSize = 11.sp, fontWeight = FontWeight.Bold)
                }

                Text("Modo de seguimiento en Gameplay", color = StudioColors.TextMuted, fontSize = 11.sp)
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    listOf(
                        "fixed" to "Cam 0 Fija",
                        "sections" to "Chart + Ev",
                        "beats" to "Cada Nb"
                    ).forEach { (modeKey, label) ->
                        val isSel = camera.followMode == modeKey
                        Box(
                            modifier = Modifier
                                .weight(1f)
                                .height(30.dp)
                                .clip(RoundedCornerShape(7.dp))
                                .background(if (isSel) StudioColors.AccentCyan.copy(alpha = 0.22f) else StudioColors.BgCard)
                                .border(
                                    1.dp,
                                    if (isSel) StudioColors.AccentCyan else StudioColors.BorderSubtle,
                                    RoundedCornerShape(7.dp)
                                )
                                .clickable { commitGlobalCamera(mode = modeKey) },
                            contentAlignment = Alignment.Center
                        ) {
                            Text(
                                text = label,
                                color = if (isSel) StudioColors.AccentCyan else StudioColors.TextPrimary,
                                fontSize = 10.5.sp,
                                fontWeight = if (isSel) FontWeight.Bold else FontWeight.Medium,
                                maxLines = 1,
                                softWrap = false
                            )
                        }
                    }
                }

                when (camera.followMode) {
                    "fixed" -> {
                        Text(
                            text = "Arranca fija en Cam 0 hasta recibir una ranura explícita de sección, eventos de chart.json (FocusCamera, Switch Camera, Camera Follow Pos, ZoomCamera) o scripts.",
                            color = StudioColors.TextMuted,
                            fontSize = 10.sp,
                            lineHeight = 13.sp
                        )
                    }
                    "beats" -> {
                        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            Text(
                                text = "Arranca en Cam 0 y alterna Cam 1 / Cam 2 cada ${camera.followIntervalBeats} beats (los eventos del chart tienen prioridad).",
                                color = StudioColors.TextMuted,
                                fontSize = 10.sp,
                                lineHeight = 13.sp
                            )
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.spacedBy(4.dp)
                            ) {
                                listOf(2, 4, 8, 16).forEach { beatsVal ->
                                    val isSel = camera.followIntervalBeats == beatsVal
                                    Box(
                                        modifier = Modifier
                                            .weight(1f)
                                            .height(27.dp)
                                            .clip(RoundedCornerShape(6.dp))
                                            .background(if (isSel) StudioColors.AccentEmerald.copy(alpha = 0.22f) else StudioColors.BgCard)
                                            .border(
                                                1.dp,
                                                if (isSel) StudioColors.AccentEmerald else StudioColors.BorderSubtle,
                                                RoundedCornerShape(6.dp)
                                            )
                                            .clickable { commitGlobalCamera(beats = beatsVal) },
                                        contentAlignment = Alignment.Center
                                    ) {
                                        Text(
                                            text = "${beatsVal}b",
                                            color = if (isSel) StudioColors.AccentEmerald else StudioColors.TextPrimary,
                                            fontSize = 11.sp,
                                            fontWeight = FontWeight.SemiBold
                                        )
                                    }
                                }
                            }
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.spacedBy(6.dp)
                            ) {
                                CameraStepChip(
                                    label = "- 1 beat",
                                    onClick = { commitGlobalCamera(beats = (camera.followIntervalBeats - 1).coerceIn(1, 32)) },
                                    modifier = Modifier.weight(1f)
                                )
                                CameraStepChip(
                                    label = "+ 1 beat",
                                    onClick = { commitGlobalCamera(beats = (camera.followIntervalBeats + 1).coerceIn(1, 32)) },
                                    modifier = Modifier.weight(1f)
                                )
                            }
                        }
                    }
                    else -> {
                        Text(
                            text = "Arranca en Cam 0 al empezar el conteo; usa mustHitSection, gfSection, ranuras Cam N de cada sección y eventos directos de chart.json (FocusCamera, Camera Follow Pos, ZoomCamera, Add Camera Zoom, Switch Camera).",
                            color = StudioColors.TextMuted,
                            fontSize = 10.sp,
                            lineHeight = 13.sp
                        )
                    }
                }

                Column {
                    Text(
                        text = "Suavizado de Cam ${activeSlot.index}: ${((localSmooth * 10f).roundToInt() / 10f)}",
                        color = StudioColors.TextMuted,
                        fontSize = 11.sp
                    )
                    Slider(
                        value = localSmooth.coerceIn(1f, 20f),
                        onValueChange = { s ->
                            isDraggingSmooth = true
                            localSmooth = s
                        },
                        onValueChangeFinished = {
                            isDraggingSmooth = false
                            commitActiveSlot(smooth = localSmooth, save = true)
                        },
                        valueRange = 1f..20f,
                        colors = SliderDefaults.colors(
                            thumbColor = StudioColors.AccentIndigo,
                            activeTrackColor = StudioColors.AccentIndigo
                        )
                    )
                }

                if (activeSlot.targetKind != "fixed") {
                    Column {
                        Text(
                            text = "Pan hacia objetivo (Cam ${activeSlot.index}): ${(localPan * 100f).roundToInt()}%",
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp
                        )
                        Slider(
                            value = localPan.coerceIn(0f, 1f),
                            onValueChange = { p ->
                                isDraggingPan = true
                                localPan = p
                            },
                            onValueChangeFinished = {
                                isDraggingPan = false
                                commitActiveSlot(pan = localPan, save = true)
                            },
                            valueRange = 0f..1f,
                            colors = SliderDefaults.colors(
                                thumbColor = StudioColors.AccentEmerald,
                                activeTrackColor = StudioColors.AccentEmerald
                            )
                        )
                    }
                }

                // Límite automático contra el fondo para no mostrar bordes negros en extremos
                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = StudioColors.BgCard,
                    border = BorderStroke(
                        1.dp,
                        if (camera.clampToBackground) StudioColors.AccentCyan.copy(alpha = 0.45f) else StudioColors.BorderSubtle
                    )
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 8.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = "Limitar cámara al fondo",
                                color = StudioColors.TextPrimary,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.SemiBold,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis
                            )
                            Text(
                                text = "Evita bordes negros cuando el personaje está en un extremo",
                                color = StudioColors.TextMuted,
                                fontSize = 9.5.sp,
                                lineHeight = 12.sp
                            )
                        }
                        Spacer(Modifier.width(6.dp))
                        Switch(
                            checked = camera.clampToBackground,
                            onCheckedChange = { nextClamp ->
                                commitGlobalCamera(clampToBackground = nextClamp)
                            }
                        )
                    }
                }
            } else {
                // Pestaña 3: Viñeta / Brillo Lateral que sigue a la cámara
                Text(
                    text = "Resplandor o viñeta lateral que acompaña siempre el encuadre de la cámara. Puedes activarla desde el inicio o hacer que aparezca/desaparezca en cualquier paso # del Chart.",
                    color = StudioColors.TextMuted,
                    fontSize = 9.5.sp,
                    lineHeight = 12.5.sp
                )

                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = StudioColors.BgCard,
                    border = BorderStroke(
                        1.dp,
                        if (camera.vignetteEnabled) StudioColors.AccentCoral.copy(alpha = 0.65f) else StudioColors.BorderSubtle
                    )
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 8.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = "Activar viñeta lateral base",
                                color = StudioColors.TextPrimary,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.SemiBold,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis
                            )
                            Text(
                                text = "Visible desde el arranque (o desactívala y lánzala por pasos en el Chart)",
                                color = StudioColors.TextMuted,
                                fontSize = 9.5.sp,
                                lineHeight = 12.sp
                            )
                        }
                        Spacer(Modifier.width(6.dp))
                        Switch(
                            checked = camera.vignetteEnabled,
                            onCheckedChange = { nextEnabled ->
                                commitVignette(enabled = nextEnabled, save = true)
                            }
                        )
                    }
                }

                Text("Color de los bordes laterales", color = StudioColors.TextMuted, fontSize = 10.5.sp)
                FlowRow(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(5.dp),
                    verticalArrangement = Arrangement.spacedBy(5.dp)
                ) {
                    VIGNETTE_PRESET_COLORS.forEach { (hex, label) ->
                        val parsed = parseInspectorHexColor(hex)
                        val isSel = localVignetteColor.equals(hex, ignoreCase = true)
                        Row(
                            modifier = Modifier
                                .height(26.dp)
                                .clip(RoundedCornerShape(6.dp))
                                .background(if (isSel) parsed.copy(alpha = 0.24f) else StudioColors.BgCard)
                                .border(
                                    1.dp,
                                    if (isSel) parsed else StudioColors.BorderSubtle,
                                    RoundedCornerShape(6.dp)
                                )
                                .clickable {
                                    localVignetteColor = hex
                                    commitVignette(enabled = true, colorHex = hex, save = true)
                                }
                                .padding(horizontal = 7.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(4.dp)
                        ) {
                            Box(
                                modifier = Modifier
                                    .size(10.dp)
                                    .clip(CircleShape)
                                    .background(parsed)
                                    .border(0.8.dp, Color.White.copy(alpha = 0.45f), CircleShape)
                            )
                            Text(
                                text = label,
                                color = StudioColors.TextPrimary,
                                fontSize = 10.sp,
                                fontWeight = if (isSel) FontWeight.Bold else FontWeight.Medium,
                                maxLines = 1,
                                softWrap = false
                            )
                        }
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    OutlinedTextField(
                        value = localVignetteColor,
                        onValueChange = { raw ->
                            localVignetteColor = raw
                            val clean = raw.trim().removePrefix("#")
                            if (clean.length == 6 || clean.length == 8) {
                                val normalized = "#$clean"
                                commitVignette(colorHex = normalized, save = false)
                            }
                        },
                        label = { Text("Color Hex (#RRGGBB)", fontSize = 10.sp) },
                        singleLine = true,
                        modifier = Modifier.weight(1f)
                    )
                    OutlinedButton(
                        onClick = { commitVignette(enabled = true, colorHex = localVignetteColor, save = true) },
                        contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                        modifier = Modifier.height(34.dp)
                    ) {
                        Text("Fijar", fontSize = 10.5.sp, fontWeight = FontWeight.Bold)
                    }
                }

                Column {
                    Text(
                        text = "Intensidad lateral: ${(localVignetteAlpha * 100f).roundToInt()}%",
                        color = StudioColors.TextMuted,
                        fontSize = 11.sp
                    )
                    Slider(
                        value = localVignetteAlpha.coerceIn(0f, 1f),
                        onValueChange = { nextA ->
                            isDraggingVigAlpha = true
                            localVignetteAlpha = nextA
                            commitVignette(alpha = nextA, save = false)
                        },
                        onValueChangeFinished = {
                            isDraggingVigAlpha = false
                            commitVignette(alpha = localVignetteAlpha, save = true)
                        },
                        valueRange = 0f..1f,
                        colors = SliderDefaults.colors(
                            thumbColor = StudioColors.AccentCoral,
                            activeTrackColor = StudioColors.AccentCoral
                        )
                    )
                }

                Column {
                    Text(
                        text = "Ancho de borde lateral: ${(localVignetteWidth * 100f).roundToInt()}%",
                        color = StudioColors.TextMuted,
                        fontSize = 11.sp
                    )
                    Slider(
                        value = localVignetteWidth.coerceIn(0.05f, 0.50f),
                        onValueChange = { nextW ->
                            isDraggingVigWidth = true
                            localVignetteWidth = nextW
                            commitVignette(width = nextW, save = false)
                        },
                        onValueChangeFinished = {
                            isDraggingVigWidth = false
                            commitVignette(width = localVignetteWidth, save = true)
                        },
                        valueRange = 0.05f..0.50f,
                        colors = SliderDefaults.colors(
                            thumbColor = StudioColors.AccentCyan,
                            activeTrackColor = StudioColors.AccentCyan
                        )
                    )
                }

                Column {
                    Text(
                        text = "Pulso al Beat / Shake: ${(localVignettePulse * 100f).roundToInt()}%",
                        color = StudioColors.TextMuted,
                        fontSize = 11.sp
                    )
                    Slider(
                        value = localVignettePulse.coerceIn(0f, 1f),
                        onValueChange = { nextP ->
                            isDraggingVigPulse = true
                            localVignettePulse = nextP
                            commitVignette(pulse = nextP, save = false)
                        },
                        onValueChangeFinished = {
                            isDraggingVigPulse = false
                            commitVignette(pulse = localVignettePulse, save = true)
                        },
                        valueRange = 0f..1f,
                        colors = SliderDefaults.colors(
                            thumbColor = StudioColors.AccentAmber,
                            activeTrackColor = StudioColors.AccentAmber
                        )
                    )
                }

                Text("Capa visual de la viñeta", color = StudioColors.TextMuted, fontSize = 10.5.sp)
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    listOf(false to "Escenario (detrás HUD)", true to "Sobre HUD / Flechas").forEach { (over, label) ->
                        val isSel = camera.vignetteOverHud == over
                        Box(
                            modifier = Modifier
                                .weight(1f)
                                .height(29.dp)
                                .clip(RoundedCornerShape(7.dp))
                                .background(if (isSel) StudioColors.AccentCyan.copy(alpha = 0.22f) else StudioColors.BgCard)
                                .border(
                                    1.dp,
                                    if (isSel) StudioColors.AccentCyan else StudioColors.BorderSubtle,
                                    RoundedCornerShape(7.dp)
                                )
                                .clickable { commitVignette(overHud = over, save = true) },
                            contentAlignment = Alignment.Center
                        ) {
                            Text(
                                text = label,
                                color = if (isSel) StudioColors.AccentCyan else StudioColors.TextPrimary,
                                fontSize = 10.sp,
                                fontWeight = if (isSel) FontWeight.Bold else FontWeight.Medium,
                                maxLines = 1,
                                softWrap = false
                            )
                        }
                    }
                }

                Button(
                    onClick = onOpenCameraScript,
                    contentPadding = PaddingValues(horizontal = 10.dp, vertical = 5.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = StudioColors.AccentPurple,
                        contentColor = Color.White
                    ),
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(34.dp)
                ) {
                    Icon(Icons.Rounded.Code, null, modifier = Modifier.size(15.dp))
                    Spacer(Modifier.width(6.dp))
                    Text("Editar camera_system.lua", fontSize = 11.sp, fontWeight = FontWeight.Bold)
                }
            }
        }
    }
}

private fun parseInspectorHexColor(raw: String): Color {
    val clean = raw.trim().removePrefix("#")
    return try {
        when (clean.length) {
            6 -> Color((0xFF000000L or clean.toLong(16)).toInt())
            8 -> Color(clean.toLong(16).toInt())
            else -> StudioColors.AccentCoral
        }
    } catch (_: Throwable) {
        StudioColors.AccentCoral
    }
}

private fun cameraSlotAccentColor(slot: StageCameraSlotSnapshot): Color = when (slot.targetKind) {
    "player" -> StudioColors.AccentEmerald
    "opponent" -> StudioColors.AccentCoral
    "gf" -> StudioColors.AccentAmber
    "entity" -> StudioColors.AccentPurple
    else -> StudioColors.AccentCyan
}

private fun cameraSlotRoleShortTag(slot: StageCameraSlotSnapshot): String = when (slot.targetKind) {
    "player" -> "Jug"
    "opponent" -> "Riv"
    "gf" -> "GF"
    "entity" -> "Ent"
    else -> "Fija"
}

@Composable
private fun CameraTabChip(
    label: String,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    selected: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier
            .height(29.dp)
            .clip(RoundedCornerShape(7.dp))
            .background(if (selected) StudioColors.AccentCyan.copy(alpha = 0.22f) else StudioColors.BgCard)
            .border(
                1.dp,
                if (selected) StudioColors.AccentCyan else StudioColors.BorderSubtle,
                RoundedCornerShape(7.dp)
            )
            .clickable(onClick = onClick),
        horizontalArrangement = Arrangement.Center,
        verticalAlignment = Alignment.CenterVertically
    ) {
        Icon(
            imageVector = icon,
            contentDescription = null,
            tint = if (selected) StudioColors.AccentCyan else StudioColors.TextMuted,
            modifier = Modifier.size(13.dp)
        )
        Spacer(Modifier.width(4.dp))
        Text(
            text = label,
            color = if (selected) StudioColors.AccentCyan else StudioColors.TextPrimary,
            fontSize = 11.sp,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
            maxLines = 1,
            softWrap = false
        )
    }
}

@Composable
private fun CameraStepChip(
    label: String,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    Box(
        modifier = modifier
            .height(27.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(StudioColors.BgCard)
            .border(1.dp, StudioColors.BorderSubtle, RoundedCornerShape(6.dp))
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        Text(
            text = label,
            color = StudioColors.TextPrimary,
            fontSize = 10.5.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
            softWrap = false
        )
    }
}
