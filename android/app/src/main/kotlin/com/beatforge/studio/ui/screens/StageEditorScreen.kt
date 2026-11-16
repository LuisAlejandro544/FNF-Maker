// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/StageEditorScreen.kt
// Propósito: Editor visual de escenario en 3 columnas (Capas Z-order, Lienzo interactivo 16:9
// con etiquetas visibles y arrastre táctil, e Inspector con selector de rol en cuadrícula 2x2,
// cruceta de precisión 1/10/35 px, escala, opacidad, visibilidad y volteo horizontal).

package com.beatforge.studio.ui.screens

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
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
import androidx.compose.material.icons.rounded.ArrowBack
import androidx.compose.material.icons.rounded.ArrowDownward
import androidx.compose.material.icons.rounded.ArrowForward
import androidx.compose.material.icons.rounded.ArrowUpward
import androidx.compose.material.icons.rounded.FlipToBack
import androidx.compose.material.icons.rounded.FlipToFront
import androidx.compose.material.icons.rounded.Image
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.KeyboardArrowUp
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material.icons.rounded.VisibilityOff
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.ProjectSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.abs
import kotlin.math.roundToInt

private val STAGE_ROLES = listOf(
    "background" to "Fondo",
    "opponent" to "Oponente",
    "player" to "Jugador",
    "decoration" to "Decoración"
)

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

    val latestEntities by rememberUpdatedState(entities)
    val latestSelected by rememberUpdatedState(selected)

    var dragX by remember(selected?.id, selected?.x) { mutableFloatStateOf(selected?.x ?: 640f) }
    var dragY by remember(selected?.id, selected?.y) { mutableFloatStateOf(selected?.y ?: 360f) }

    val textMeasurer = rememberTextMeasurer()

    Row(
        modifier = Modifier
            .fillMaxSize()
            .padding(10.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        // Columna izquierda: Capas Z-order
        Surface(
            modifier = Modifier
                .weight(0.24f)
                .fillMaxHeight(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Column(modifier = Modifier.padding(9.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = "Capas",
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 13.sp
                    )
                    Button(
                        onClick = { onAddEntity("Decoración", "decoration") },
                        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = StudioColors.AccentCyan,
                            contentColor = Color(0xFF1F0C00)
                        ),
                        modifier = Modifier.height(28.dp)
                    ) {
                        Icon(Icons.Rounded.Add, null, modifier = Modifier.size(14.dp))
                        Spacer(Modifier.width(2.dp))
                        Text("Objeto", fontSize = 10.sp, fontWeight = FontWeight.Bold, maxLines = 1, softWrap = false)
                    }
                }

                Column(
                    modifier = Modifier
                        .weight(1f)
                        .verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(4.dp)
                ) {
                    entities.reversed().forEach { ent ->
                        val isSel = ent.id == selected?.id
                        Surface(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onSelectEntity(ent.id) },
                            shape = RoundedCornerShape(8.dp),
                            color = if (isSel) StudioColors.AccentCyan.copy(alpha = 0.16f) else StudioColors.BgCard,
                            border = BorderStroke(
                                1.dp,
                                if (isSel) StudioColors.AccentCyan else StudioColors.BorderSubtle
                            )
                        ) {
                            Row(
                                modifier = Modifier.padding(horizontal = 7.dp, vertical = 6.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(
                                        text = ent.displayName,
                                        color = StudioColors.TextPrimary,
                                        fontWeight = FontWeight.SemiBold,
                                        fontSize = 11.sp,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis
                                    )
                                    Text(
                                        text = "Z:${ent.zIndex} · ${ent.roleLabel}",
                                        color = StudioColors.TextMuted,
                                        fontSize = 10.sp,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis
                                    )
                                }
                                IconButton(
                                    onClick = {
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
                                    modifier = Modifier.size(26.dp)
                                ) {
                                    Icon(
                                        imageVector = if (ent.visible) Icons.Rounded.Visibility else Icons.Rounded.VisibilityOff,
                                        contentDescription = "Visibilidad",
                                        tint = if (ent.visible) StudioColors.AccentEmerald else StudioColors.AccentCoral,
                                        modifier = Modifier.size(15.dp)
                                    )
                                }
                            }
                        }
                    }
                }

                if (selected != null) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceEvenly
                    ) {
                        IconButton(onClick = { onReorderEntity(selected.id, "forward") }, modifier = Modifier.size(30.dp)) {
                            Icon(Icons.Rounded.KeyboardArrowUp, "Subir", tint = StudioColors.TextPrimary)
                        }
                        IconButton(onClick = { onReorderEntity(selected.id, "backward") }, modifier = Modifier.size(30.dp)) {
                            Icon(Icons.Rounded.KeyboardArrowDown, "Bajar", tint = StudioColors.TextPrimary)
                        }
                        IconButton(onClick = { onReorderEntity(selected.id, "front") }, modifier = Modifier.size(30.dp)) {
                            Icon(Icons.Rounded.FlipToFront, "Frente", tint = StudioColors.AccentCyan)
                        }
                        IconButton(onClick = { onReorderEntity(selected.id, "back") }, modifier = Modifier.size(30.dp)) {
                            Icon(Icons.Rounded.FlipToBack, "Fondo", tint = StudioColors.AccentIndigo)
                        }
                    }
                }
            }
        }

        // Columna central: Lienzo 16:9 interactivo
        Surface(
            modifier = Modifier
                .weight(0.43f)
                .fillMaxHeight(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .padding(10.dp),
                contentAlignment = Alignment.Center
            ) {
                Canvas(
                    modifier = Modifier
                        .fillMaxWidth()
                        .aspectRatio(16f / 9f)
                        .background(StudioColors.BgDark, RoundedCornerShape(8.dp))
                        .pointerInput(Unit) {
                            detectTapGestures { tap ->
                                val scaleToVirtX = 1280f / size.width.toFloat()
                                val scaleToVirtY = 720f / size.height.toFloat()
                                val vx = tap.x * scaleToVirtX
                                val vy = tap.y * scaleToVirtY
                                val hit = latestEntities.reversed().firstOrNull { ent ->
                                    if (!ent.visible) return@firstOrNull false
                                    val hw = (ent.width * ent.scale) * 0.5f
                                    val hh = (ent.height * ent.scale) * 0.5f
                                    abs(vx - ent.x) <= hw.coerceAtLeast(32f) &&
                                        abs(vy - ent.y) <= hh.coerceAtLeast(32f)
                                }
                                if (hit != null) {
                                    onSelectEntity(hit.id)
                                }
                            }
                        }
                        .pointerInput(selected?.id) {
                            detectDragGestures(
                                onDrag = { change, dragAmount ->
                                    change.consume()
                                    val sel = latestSelected ?: return@detectDragGestures
                                    val scaleToVirtX = 1280f / size.width.toFloat()
                                    val scaleToVirtY = 720f / size.height.toFloat()
                                    dragX = (dragX + dragAmount.x * scaleToVirtX).coerceIn(-600f, 1880f)
                                    dragY = (dragY + dragAmount.y * scaleToVirtY).coerceIn(-400f, 1120f)
                                    onUpdateEntity(
                                        sel.id,
                                        sel.displayName,
                                        sel.role,
                                        0f,
                                        0f,
                                        dragX,
                                        dragY,
                                        true,
                                        sel.scale,
                                        sel.opacity,
                                        sel.visible,
                                        sel.flipX
                                    )
                                }
                            )
                        }
                ) {
                    val sx = size.width / 1280f
                    val sy = size.height / 720f

                    // Rejilla guía de 1280x720
                    for (gx in 1..7) {
                        val px = size.width * (gx / 8f)
                        drawLine(StudioColors.BorderSubtle.copy(alpha = 0.3f), Offset(px, 0f), Offset(px, size.height))
                    }
                    for (gy in 1..4) {
                        val py = size.height * (gy / 5f)
                        drawLine(StudioColors.BorderSubtle.copy(alpha = 0.3f), Offset(0f, py), Offset(size.width, py))
                    }

                    entities.forEach { ent ->
                        if (!ent.visible || ent.opacity <= 0.01f) return@forEach
                        val cx = (if (ent.id == selected?.id) dragX else ent.x) * sx
                        val cy = (if (ent.id == selected?.id) dragY else ent.y) * sy
                        val w = ent.width * ent.scale * sx
                        val h = ent.height * ent.scale * sy
                        val left = cx - w / 2f
                        val top = cy - h / 2f
                        val loaded = entityBitmaps[ent.id]

                        withTransform({
                            if (ent.flipX) {
                                scale(scaleX = -1f, scaleY = 1f, pivot = Offset(cx, cy))
                            }
                        }) {
                            if (loaded != null) {
                                drawImage(
                                    image = loaded.bitmap,
                                    srcOffset = IntOffset.Zero,
                                    srcSize = IntSize(loaded.bitmap.width, loaded.bitmap.height),
                                    dstOffset = IntOffset(left.roundToInt(), top.roundToInt()),
                                    dstSize = IntSize(w.roundToInt().coerceAtLeast(2), h.roundToInt().coerceAtLeast(2)),
                                    alpha = ent.opacity.coerceIn(0f, 1f)
                                )
                            } else {
                                val fallbackColor = when (ent.role) {
                                    "background" -> StudioColors.AccentIndigo
                                    "opponent" -> StudioColors.AccentCoral
                                    "player" -> StudioColors.AccentEmerald
                                    else -> StudioColors.AccentCyan
                                }
                                drawRect(
                                    color = fallbackColor.copy(alpha = 0.22f * ent.opacity),
                                    topLeft = Offset(left, top),
                                    size = Size(w, h)
                                )
                                drawRect(
                                    color = fallbackColor.copy(alpha = ent.opacity),
                                    topLeft = Offset(left, top),
                                    size = Size(w, h),
                                    style = Stroke(width = 2f)
                                )
                            }
                        }

                        if (loaded == null && w > 36f && h > 18f) {
                            val layout = textMeasurer.measure(
                                text = ent.displayName,
                                style = TextStyle(
                                    color = StudioColors.TextPrimary.copy(alpha = ent.opacity),
                                    fontSize = 9.sp,
                                    fontWeight = FontWeight.SemiBold
                                )
                            )
                            drawText(
                                textLayoutResult = layout,
                                topLeft = Offset(
                                    (cx - layout.size.width / 2f).coerceIn(left + 4f, (left + w - layout.size.width - 4f).coerceAtLeast(left + 4f)),
                                    (cy - layout.size.height / 2f).coerceIn(top + 2f, (top + h - layout.size.height - 2f).coerceAtLeast(top + 2f))
                                )
                            )
                        }

                        if (ent.id == selected?.id) {
                            drawRect(
                                color = StudioColors.AccentEmerald,
                                topLeft = Offset(left - 2f, top - 2f),
                                size = Size(w + 4f, h + 4f),
                                style = Stroke(width = 2.5f)
                            )
                        }
                    }
                }
            }
        }

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

@Composable
private fun StageEntityInspector(
    entity: StageEntitySnapshot,
    stepPx: Int,
    onStepChange: (Int) -> Unit,
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
        flipX: Boolean
    ) -> Unit,
    onRequestImportPng: () -> Unit
) {
    var localName by remember(entity.id, entity.displayName) { mutableStateOf(entity.displayName) }

    Column(
        modifier = Modifier
            .padding(10.dp)
            .verticalScroll(rememberScrollState()),
        verticalArrangement = Arrangement.spacedBy(8.dp)
    ) {
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
                    entity.scale,
                    entity.opacity,
                    entity.visible,
                    entity.flipX
                )
            },
            label = { Text("Nombre", fontSize = 11.sp) },
            singleLine = true,
            modifier = Modifier.fillMaxWidth()
        )

        // Cuadrícula 2x2 limpia para los 4 roles sin amontonar ni partir palabras
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
                                        entity.scale,
                                        entity.opacity,
                                        entity.visible,
                                        entity.flipX
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
            text = "Posición (${entity.x.roundToInt()}, ${entity.y.roundToInt()}) · Paso ${stepPx}px",
            color = StudioColors.TextMuted,
            fontSize = 11.sp
        )

        Row(horizontalArrangement = Arrangement.spacedBy(5.dp), modifier = Modifier.fillMaxWidth()) {
            listOf(1, 10, 35).forEach { px ->
                val isSel = stepPx == px
                Box(
                    modifier = Modifier
                        .weight(1f)
                        .height(28.dp)
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

        // Cruceta de desplazamiento
        Column(horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxWidth()) {
            IconButton(
                onClick = {
                    onUpdate(entity.id, localName, entity.role, 0f, -stepPx.toFloat(), 0f, 0f, false, entity.scale, entity.opacity, entity.visible, entity.flipX)
                },
                modifier = Modifier.size(34.dp)
            ) {
                Icon(Icons.Rounded.ArrowUpward, "Arriba", tint = StudioColors.AccentCyan)
            }
            Row(horizontalArrangement = Arrangement.spacedBy(18.dp)) {
                IconButton(
                    onClick = {
                        onUpdate(entity.id, localName, entity.role, -stepPx.toFloat(), 0f, 0f, 0f, false, entity.scale, entity.opacity, entity.visible, entity.flipX)
                    },
                    modifier = Modifier.size(34.dp)
                ) {
                    Icon(Icons.Rounded.ArrowBack, "Izquierda", tint = StudioColors.AccentCyan)
                }
                IconButton(
                    onClick = {
                        onUpdate(entity.id, localName, entity.role, stepPx.toFloat(), 0f, 0f, 0f, false, entity.scale, entity.opacity, entity.visible, entity.flipX)
                    },
                    modifier = Modifier.size(34.dp)
                ) {
                    Icon(Icons.Rounded.ArrowForward, "Derecha", tint = StudioColors.AccentCyan)
                }
            }
            IconButton(
                onClick = {
                    onUpdate(entity.id, localName, entity.role, 0f, stepPx.toFloat(), 0f, 0f, false, entity.scale, entity.opacity, entity.visible, entity.flipX)
                },
                modifier = Modifier.size(34.dp)
            ) {
                Icon(Icons.Rounded.ArrowDownward, "Abajo", tint = StudioColors.AccentCyan)
            }
        }

        Column {
            Text("Escala: ${((entity.scale * 100).roundToInt() / 100f)}x", color = StudioColors.TextMuted, fontSize = 11.sp)
            Slider(
                value = entity.scale,
                onValueChange = { s ->
                    onUpdate(entity.id, localName, entity.role, 0f, 0f, 0f, 0f, false, s, entity.opacity, entity.visible, entity.flipX)
                },
                valueRange = 0.1f..3.0f,
                colors = SliderDefaults.colors(thumbColor = StudioColors.AccentCyan, activeTrackColor = StudioColors.AccentCyan)
            )
        }

        Column {
            Text("Opacidad: ${(entity.opacity * 100).roundToInt()}%", color = StudioColors.TextMuted, fontSize = 11.sp)
            Slider(
                value = entity.opacity,
                onValueChange = { op ->
                    onUpdate(entity.id, localName, entity.role, 0f, 0f, 0f, 0f, false, entity.scale, op, entity.visible, entity.flipX)
                },
                valueRange = 0f..1f,
                colors = SliderDefaults.colors(thumbColor = StudioColors.AccentIndigo, activeTrackColor = StudioColors.AccentIndigo)
            )
        }

        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Text("Visible", color = StudioColors.TextPrimary, fontSize = 12.sp)
            Switch(
                checked = entity.visible,
                onCheckedChange = { vis ->
                    onUpdate(entity.id, localName, entity.role, 0f, 0f, 0f, 0f, false, entity.scale, entity.opacity, vis, entity.flipX)
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
                    onUpdate(entity.id, localName, entity.role, 0f, 0f, 0f, 0f, false, entity.scale, entity.opacity, entity.visible, fx)
                }
            )
        }
    }
}
