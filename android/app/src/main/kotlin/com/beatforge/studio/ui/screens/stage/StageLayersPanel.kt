// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/StageLayersPanel.kt
// Propósito: Panel izquierdo virtualizado (`LazyColumn`) de capas Z-order del editor de escenario.
// Muestra la capa fija `Cámara (.lua)` y las entidades ordenadas de frente a fondo con indicador
// visual de rol, estado de bloqueo/visibilidad y barra inferior fija para reordenar profundidad Z.

package com.beatforge.studio.ui.screens.stage

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.FlipToBack
import androidx.compose.material.icons.rounded.FlipToFront
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.KeyboardArrowUp
import androidx.compose.material.icons.rounded.Lock
import androidx.compose.material.icons.rounded.Videocam
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material.icons.rounded.VisibilityOff
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StageCameraSnapshot
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

@Composable
fun StageLayersPanel(
    entities: List<StageEntitySnapshot>,
    selected: StageEntitySnapshot?,
    stageCamera: StageCameraSnapshot,
    isCameraSelected: Boolean,
    lockedEntityIds: Set<String>,
    onSelectEntity: (String) -> Unit,
    onSelectCamera: () -> Unit,
    onToggleCameraEnabled: () -> Unit,
    onAddEntity: (name: String, role: String) -> Unit,
    onToggleVisibility: (StageEntitySnapshot) -> Unit,
    onReorderEntity: (entityId: String, action: String) -> Unit,
    modifier: Modifier = Modifier
) {
    val reversedEntities = remember(entities) { entities.asReversed() }
    val selectedId = if (isCameraSelected) null else selected?.id

    Surface(
        modifier = modifier.fillMaxHeight(),
        shape = RoundedCornerShape(12.dp),
        color = StudioColors.BgPanel,
        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
    ) {
        Column(
            modifier = Modifier.padding(8.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp)
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically
            ) {
                Column {
                    Text(
                        text = "Capas",
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 13.sp
                    )
                    Text(
                        text = "${entities.size} objetos",
                        color = StudioColors.TextMuted,
                        fontSize = 10.sp
                    )
                }
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

            StageCameraLayerRow(
                stageCamera = stageCamera,
                isSelected = isCameraSelected,
                onSelect = onSelectCamera,
                onToggleEnabled = onToggleCameraEnabled
            )

            LazyColumn(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                items(
                    items = reversedEntities,
                    key = { it.id }
                ) { ent ->
                    StageLayerRow(
                        entity = ent,
                        isSelected = ent.id == selectedId,
                        isLocked = lockedEntityIds.contains(ent.id),
                        onSelect = { onSelectEntity(ent.id) },
                        onToggleVisibility = { onToggleVisibility(ent) }
                    )
                }
            }

            HorizontalDivider(color = StudioColors.BorderSubtle)

            if (!isCameraSelected && selected != null) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = "Z:${selected.zIndex}",
                        color = StudioColors.TextMuted,
                        fontSize = 10.sp,
                        fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.padding(start = 2.dp)
                    )
                    Row(horizontalArrangement = Arrangement.spacedBy(2.dp)) {
                        IconButton(onClick = { onReorderEntity(selected.id, "forward") }, modifier = Modifier.size(28.dp)) {
                            Icon(Icons.Rounded.KeyboardArrowUp, "Subir capa", tint = StudioColors.TextPrimary, modifier = Modifier.size(18.dp))
                        }
                        IconButton(onClick = { onReorderEntity(selected.id, "backward") }, modifier = Modifier.size(28.dp)) {
                            Icon(Icons.Rounded.KeyboardArrowDown, "Bajar capa", tint = StudioColors.TextPrimary, modifier = Modifier.size(18.dp))
                        }
                        IconButton(onClick = { onReorderEntity(selected.id, "front") }, modifier = Modifier.size(28.dp)) {
                            Icon(Icons.Rounded.FlipToFront, "Al frente", tint = StudioColors.AccentCyan, modifier = Modifier.size(16.dp))
                        }
                        IconButton(onClick = { onReorderEntity(selected.id, "back") }, modifier = Modifier.size(28.dp)) {
                            Icon(Icons.Rounded.FlipToBack, "Al fondo", tint = StudioColors.AccentIndigo, modifier = Modifier.size(16.dp))
                        }
                    }
                }
            } else {
                Text(
                    text = if (isCameraSelected) "Cámara en modo edición" else "Selecciona una capa",
                    color = StudioColors.TextMuted,
                    fontSize = 10.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(vertical = 4.dp)
                )
            }
        }
    }
}

@Composable
private fun StageCameraLayerRow(
    stageCamera: StageCameraSnapshot,
    isSelected: Boolean,
    onSelect: () -> Unit,
    onToggleEnabled: () -> Unit
) {
    val zoomDisplay = ((stageCamera.baseZoom * 100f).roundToInt() / 100f)
    val modeSummary = if (stageCamera.followMode == "sections") {
        "Secciones"
    } else {
        "Cada ${stageCamera.followIntervalBeats}b"
    }

    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onSelect),
        shape = RoundedCornerShape(8.dp),
        color = if (isSelected) StudioColors.AccentCyan.copy(alpha = 0.22f) else StudioColors.BgElevated,
        border = BorderStroke(
            1.dp,
            if (isSelected) StudioColors.AccentCyan else StudioColors.AccentIndigo.copy(alpha = 0.55f)
        )
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 7.dp, vertical = 5.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Icon(
                imageVector = Icons.Rounded.Videocam,
                contentDescription = null,
                tint = if (stageCamera.enabled) StudioColors.AccentCyan else StudioColors.TextMuted,
                modifier = Modifier.size(15.dp)
            )
            Spacer(Modifier.width(5.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = "Cámara (.lua)",
                    color = StudioColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 11.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
                Text(
                    text = "${zoomDisplay}x · $modeSummary",
                    color = StudioColors.TextMuted,
                    fontSize = 9.5.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }
            IconButton(
                onClick = onToggleEnabled,
                modifier = Modifier.size(24.dp)
            ) {
                Icon(
                    imageVector = if (stageCamera.enabled) Icons.Rounded.Visibility else Icons.Rounded.VisibilityOff,
                    contentDescription = "Activar cámara",
                    tint = if (stageCamera.enabled) StudioColors.AccentEmerald else StudioColors.AccentCoral,
                    modifier = Modifier.size(14.dp)
                )
            }
        }
    }
}

@Composable
private fun StageLayerRow(
    entity: StageEntitySnapshot,
    isSelected: Boolean,
    isLocked: Boolean,
    onSelect: () -> Unit,
    onToggleVisibility: () -> Unit
) {
    val roleColor = when (entity.role) {
        "background" -> StudioColors.AccentIndigo
        "opponent" -> StudioColors.AccentCoral
        "player" -> StudioColors.AccentEmerald
        else -> StudioColors.AccentCyan
    }

    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onSelect),
        shape = RoundedCornerShape(8.dp),
        color = if (isSelected) StudioColors.AccentCyan.copy(alpha = 0.16f) else StudioColors.BgCard,
        border = BorderStroke(
            1.dp,
            if (isSelected) StudioColors.AccentCyan else StudioColors.BorderSubtle
        )
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 6.dp, vertical = 5.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Box(
                modifier = Modifier
                    .size(7.dp)
                    .clip(CircleShape)
                    .background(roleColor)
            )
            Spacer(Modifier.width(5.dp))
            Column(modifier = Modifier.weight(1f)) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(3.dp)
                ) {
                    Text(
                        text = entity.displayName,
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.SemiBold,
                        fontSize = 11.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f, fill = false)
                    )
                    if (isLocked) {
                        Icon(
                            imageVector = Icons.Rounded.Lock,
                            contentDescription = "Bloqueado",
                            tint = StudioColors.AccentIndigo,
                            modifier = Modifier.size(11.dp)
                        )
                    }
                }
                Text(
                    text = "Z:${entity.zIndex} · ${entity.roleLabel}",
                    color = StudioColors.TextMuted,
                    fontSize = 9.5.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis
                )
            }
            IconButton(
                onClick = onToggleVisibility,
                modifier = Modifier.size(24.dp)
            ) {
                Icon(
                    imageVector = if (entity.visible) Icons.Rounded.Visibility else Icons.Rounded.VisibilityOff,
                    contentDescription = "Visibilidad",
                    tint = if (entity.visible) StudioColors.AccentEmerald else StudioColors.AccentCoral,
                    modifier = Modifier.size(14.dp)
                )
            }
        }
    }
}
