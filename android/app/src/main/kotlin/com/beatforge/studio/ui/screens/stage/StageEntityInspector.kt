// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/StageEntityInspector.kt
// Propósito: Inspector derecho de propiedades de la capa seleccionada en el escenario,
// organizado en dos pestañas claras (`Mover` y `Ajustes`) para evitar scroll largo en pantalla
// horizontal: cruceta con centrado directo, escala continua/precisa, opacidad, rol 2x2,
// importación PNG/ASTC, visibilidad, volteo horizontal y bloqueo de arrastre.

package com.beatforge.studio.ui.screens.stage

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
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
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.ArrowForward
import androidx.compose.material.icons.rounded.ArrowDownward
import androidx.compose.material.icons.rounded.ArrowUpward
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Image
import androidx.compose.material.icons.rounded.MyLocation
import androidx.compose.material.icons.rounded.OpenWith
import androidx.compose.material.icons.rounded.Tune
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

private val STAGE_ROLES = listOf(
    "background" to "Fondo",
    "opponent" to "Oponente",
    "player" to "Jugador",
    "decoration" to "Decoración"
)

@Composable
fun StageEntityInspector(
    entity: StageEntitySnapshot,
    stepPx: Int,
    isLocked: Boolean,
    liveTransform: StageLiveTransformState,
    onStepChange: (Int) -> Unit,
    onToggleLock: () -> Unit,
    onUpdate: (
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
    onRequestImportPng: () -> Unit,
    onRemoveEntity: () -> Unit
) {
    var localName by remember(entity.id) { mutableStateOf(entity.displayName) }
    var localScale by remember(entity.id) { mutableFloatStateOf(entity.scale) }
    var isDraggingScale by remember(entity.id) { mutableStateOf(false) }
    var localOpacity by remember(entity.id) { mutableFloatStateOf(entity.opacity) }
    var isDraggingOpacity by remember(entity.id) { mutableStateOf(false) }
    var activeTab by remember { mutableIntStateOf(0) } // 0 = Mover, 1 = Ajustes

    LaunchedEffect(entity.id, entity.displayName) {
        if (localName != entity.displayName) {
            localName = entity.displayName
        }
    }
    LaunchedEffect(entity.id, entity.scale) {
        if (!isDraggingScale) {
            localScale = entity.scale
        }
    }
    LaunchedEffect(entity.id, entity.opacity) {
        if (!isDraggingOpacity) {
            localOpacity = entity.opacity
        }
    }

    fun applyImmediateScale(nextScale: Float, commit: Boolean) {
        val clamped = nextScale.coerceIn(0.1f, 5.0f)
        localScale = clamped
        if (liveTransform.entityId == entity.id) {
            liveTransform.updateScale(clamped)
        }
        if (commit) {
            onUpdate(
                entity.id,
                localName,
                entity.role,
                0f,
                0f,
                0f,
                0f,
                false,
                clamped,
                localOpacity,
                entity.visible,
                entity.flipX,
                true
            )
        }
    }

    val shownX = if (liveTransform.entityId == entity.id) liveTransform.x else entity.x
    val shownY = if (liveTransform.entityId == entity.id) liveTransform.y else entity.y
    val shownScale = if (!isDraggingScale && liveTransform.entityId == entity.id && liveTransform.isInteracting) {
        liveTransform.scale
    } else {
        localScale
    }

    Column(
        modifier = Modifier.padding(9.dp),
        verticalArrangement = Arrangement.spacedBy(7.dp)
    ) {
        // Encabezado compacto con nombre, rol y Z
        Surface(
            shape = RoundedCornerShape(8.dp),
            color = StudioColors.BgElevated,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp, vertical = 6.dp),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = entity.displayName,
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 12.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    Text(
                        text = "${entity.roleLabel} · Z:${entity.zIndex}",
                        color = StudioColors.TextMuted,
                        fontSize = 10.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }
                Text(
                    text = "${((shownScale * 100f).roundToInt() / 100f)}x",
                    color = StudioColors.AccentCyan,
                    fontSize = 11.sp,
                    fontWeight = FontWeight.Bold
                )
            }
        }

        // Pestañas ergonómicas: Mover / Ajustes
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            InspectorTabChip(
                label = "Mover",
                icon = Icons.Rounded.OpenWith,
                selected = activeTab == 0,
                onClick = { activeTab = 0 },
                modifier = Modifier.weight(1f)
            )
            InspectorTabChip(
                label = "Ajustes",
                icon = Icons.Rounded.Tune,
                selected = activeTab == 1,
                onClick = { activeTab = 1 },
                modifier = Modifier.weight(1f)
            )
        }

        Column(
            modifier = Modifier
                .weight(1f)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(7.dp)
        ) {
            if (activeTab == 0) {
                // Coordenadas y selector de paso
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = "Posición (${shownX.roundToInt()}, ${shownY.roundToInt()})",
                        color = StudioColors.TextPrimary,
                        fontSize = 11.sp,
                        fontWeight = FontWeight.SemiBold
                    )
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

                // Cruceta compacta con botón central para centrar en (640, 360)
                Column(
                    horizontalAlignment = Alignment.CenterHorizontally,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    IconButton(
                        onClick = {
                            onUpdate(entity.id, localName, entity.role, 0f, -stepPx.toFloat(), 0f, 0f, false, localScale, localOpacity, entity.visible, entity.flipX, true)
                        },
                        modifier = Modifier.size(32.dp)
                    ) {
                        Icon(Icons.Rounded.ArrowUpward, "Arriba", tint = StudioColors.AccentCyan)
                    }
                    Row(
                        horizontalArrangement = Arrangement.spacedBy(10.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        IconButton(
                            onClick = {
                                onUpdate(entity.id, localName, entity.role, -stepPx.toFloat(), 0f, 0f, 0f, false, localScale, localOpacity, entity.visible, entity.flipX, true)
                            },
                            modifier = Modifier.size(32.dp)
                        ) {
                            Icon(Icons.AutoMirrored.Rounded.ArrowBack, "Izquierda", tint = StudioColors.AccentCyan)
                        }
                        IconButton(
                            onClick = {
                                onUpdate(entity.id, localName, entity.role, 0f, 0f, 640f, 360f, true, localScale, localOpacity, entity.visible, entity.flipX, true)
                            },
                            modifier = Modifier
                                .size(30.dp)
                                .clip(RoundedCornerShape(8.dp))
                                .background(StudioColors.BgCard)
                        ) {
                            Icon(Icons.Rounded.MyLocation, "Centrar", tint = StudioColors.TextMuted, modifier = Modifier.size(16.dp))
                        }
                        IconButton(
                            onClick = {
                                onUpdate(entity.id, localName, entity.role, stepPx.toFloat(), 0f, 0f, 0f, false, localScale, localOpacity, entity.visible, entity.flipX, true)
                            },
                            modifier = Modifier.size(32.dp)
                        ) {
                            Icon(Icons.AutoMirrored.Rounded.ArrowForward, "Derecha", tint = StudioColors.AccentCyan)
                        }
                    }
                    IconButton(
                        onClick = {
                            onUpdate(entity.id, localName, entity.role, 0f, stepPx.toFloat(), 0f, 0f, false, localScale, localOpacity, entity.visible, entity.flipX, true)
                        },
                        modifier = Modifier.size(32.dp)
                    ) {
                        Icon(Icons.Rounded.ArrowDownward, "Abajo", tint = StudioColors.AccentCyan)
                    }
                }

                // Escala continua + ajuste fino
                Column(verticalArrangement = Arrangement.spacedBy(3.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text("Escala", color = StudioColors.TextMuted, fontSize = 11.sp)
                        Text(
                            text = "${((shownScale * 100f).roundToInt() / 100f)}x",
                            color = StudioColors.AccentCyan,
                            fontSize = 11.sp,
                            fontWeight = FontWeight.Bold
                        )
                    }
                    Slider(
                        value = shownScale.coerceIn(0.1f, 5.0f),
                        onValueChange = { s ->
                            isDraggingScale = true
                            liveTransform.beginInteraction(entity)
                            applyImmediateScale(s, commit = false)
                        },
                        onValueChangeFinished = {
                            isDraggingScale = false
                            liveTransform.endInteraction()
                            applyImmediateScale(localScale, commit = true)
                        },
                        valueRange = 0.1f..5.0f,
                        colors = SliderDefaults.colors(
                            thumbColor = StudioColors.AccentCyan,
                            activeTrackColor = StudioColors.AccentCyan
                        )
                    )
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(5.dp)
                    ) {
                        ScaleStepButton(
                            label = "- 0.1x",
                            onClick = { applyImmediateScale(localScale - 0.1f, commit = true) },
                            modifier = Modifier.weight(1f)
                        )
                        ScaleStepButton(
                            label = "1.0x",
                            onClick = { applyImmediateScale(1.0f, commit = true) },
                            modifier = Modifier.weight(1f)
                        )
                        ScaleStepButton(
                            label = "+ 0.1x",
                            onClick = { applyImmediateScale(localScale + 0.1f, commit = true) },
                            modifier = Modifier.weight(1f)
                        )
                    }
                }

                // Opacidad
                Column {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Text("Opacidad", color = StudioColors.TextMuted, fontSize = 11.sp)
                        Text(
                            text = "${(localOpacity * 100f).roundToInt()}%",
                            color = StudioColors.AccentIndigo,
                            fontSize = 11.sp,
                            fontWeight = FontWeight.Bold
                        )
                    }
                    Slider(
                        value = localOpacity.coerceIn(0f, 1f),
                        onValueChange = { op ->
                            isDraggingOpacity = true
                            localOpacity = op
                            liveTransform.beginInteraction(entity)
                            liveTransform.updateOpacity(op)
                        },
                        onValueChangeFinished = {
                            isDraggingOpacity = false
                            liveTransform.endInteraction()
                            onUpdate(entity.id, localName, entity.role, 0f, 0f, 0f, 0f, false, localScale, localOpacity, entity.visible, entity.flipX, true)
                        },
                        valueRange = 0f..1f,
                        colors = SliderDefaults.colors(
                            thumbColor = StudioColors.AccentIndigo,
                            activeTrackColor = StudioColors.AccentIndigo
                        )
                    )
                }
            } else {
                // Pestaña 2: Nombre, Rol, Imagen y Switches
                OutlinedTextField(
                    value = localName,
                    onValueChange = {
                        localName = it
                        onUpdate(
                            entity.id,
                            it,
                            entity.role,
                            0f,
                            0f,
                            entity.x,
                            entity.y,
                            false,
                            localScale,
                            localOpacity,
                            entity.visible,
                            entity.flipX,
                            true
                        )
                    },
                    label = { Text("Nombre", fontSize = 11.sp) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )

                Text("Rol en el escenario", color = StudioColors.TextMuted, fontSize = 11.sp)
                Column(verticalArrangement = Arrangement.spacedBy(5.dp)) {
                    STAGE_ROLES.chunked(2).forEach { pair ->
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(5.dp)
                        ) {
                            pair.forEach { (key, label) ->
                                val isSel = entity.role == key
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
                                        .clickable {
                                            onUpdate(
                                                entity.id,
                                                localName,
                                                key,
                                                0f,
                                                0f,
                                                entity.x,
                                                entity.y,
                                                false,
                                                localScale,
                                                localOpacity,
                                                entity.visible,
                                                entity.flipX,
                                                true
                                            )
                                        },
                                    contentAlignment = Alignment.Center
                                ) {
                                    Text(
                                        text = label,
                                        color = if (isSel) StudioColors.AccentCyan else StudioColors.TextPrimary,
                                        fontSize = 11.sp,
                                        fontWeight = if (isSel) FontWeight.Bold else FontWeight.Medium,
                                        maxLines = 1,
                                        softWrap = false
                                    )
                                }
                            }
                        }
                    }
                }

                OutlinedButton(
                    onClick = onRequestImportPng,
                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(34.dp)
                ) {
                    Icon(Icons.Rounded.Image, null, modifier = Modifier.size(15.dp))
                    Spacer(Modifier.width(5.dp))
                    Text("Imagen PNG / ASTC", fontSize = 11.sp, maxLines = 1, softWrap = false)
                }

                Text(
                    text = "Para sprites animados (.xml o 1–2 .json), ármalos en la pestaña Sprites.",
                    color = StudioColors.TextMuted,
                    fontSize = 10.sp,
                    lineHeight = 13.sp
                )

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Visible", color = StudioColors.TextPrimary, fontSize = 12.sp)
                    Switch(
                        checked = entity.visible,
                        onCheckedChange = { vis ->
                            onUpdate(entity.id, localName, entity.role, 0f, 0f, 0f, 0f, false, localScale, localOpacity, vis, entity.flipX, true)
                        }
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text("Voltear X (Espejo)", color = StudioColors.TextPrimary, fontSize = 12.sp)
                    Switch(
                        checked = entity.flipX,
                        onCheckedChange = { fx ->
                            onUpdate(entity.id, localName, entity.role, 0f, 0f, 0f, 0f, false, localScale, localOpacity, entity.visible, fx, true)
                        }
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text("Bloquear arrastre", color = StudioColors.TextPrimary, fontSize = 12.sp)
                        Text("Evita mover el fondo o capa por accidente", color = StudioColors.TextMuted, fontSize = 9.5.sp)
                    }
                    Switch(
                        checked = isLocked,
                        onCheckedChange = { onToggleLock() }
                    )
                }

                Spacer(Modifier.height(4.dp))

                OutlinedButton(
                    onClick = onRemoveEntity,
                    border = BorderStroke(1.dp, StudioColors.AccentCoral.copy(alpha = 0.65f)),
                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(34.dp)
                ) {
                    Icon(
                        imageVector = Icons.Rounded.DeleteOutline,
                        contentDescription = null,
                        tint = StudioColors.AccentCoral,
                        modifier = Modifier.size(15.dp)
                    )
                    Spacer(Modifier.width(5.dp))
                    Text(
                        text = "Eliminar capa de la escena",
                        color = StudioColors.AccentCoral,
                        fontSize = 11.sp,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        softWrap = false
                    )
                }
            }
            Spacer(Modifier.height(6.dp))
        }
    }
}

@Composable
private fun InspectorTabChip(
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
private fun ScaleStepButton(
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
