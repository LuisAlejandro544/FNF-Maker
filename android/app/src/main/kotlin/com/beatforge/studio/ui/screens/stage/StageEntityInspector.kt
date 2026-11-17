// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/StageEntityInspector.kt
// Propósito: Inspector derecho de propiedades de la capa seleccionada en el escenario:
// edición de nombre, cuadrícula 2x2 de roles, carga de imagen PNG/ASTC, selector de paso
// (1/10/35 px), cruceta de desplazamiento, sliders de escala/opacidad y switches de visibilidad/flipX.

package com.beatforge.studio.ui.screens.stage

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
import androidx.compose.material.icons.rounded.ArrowBack
import androidx.compose.material.icons.rounded.ArrowDownward
import androidx.compose.material.icons.rounded.ArrowForward
import androidx.compose.material.icons.rounded.ArrowUpward
import androidx.compose.material.icons.rounded.Image
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
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
