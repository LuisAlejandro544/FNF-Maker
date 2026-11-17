// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/stage/StageLayersPanel.kt
// Propósito: Panel izquierdo de capas Z-order del editor de escenario. Permite añadir objetos
// decorativos, alternar su visibilidad y reordenar la profundidad (subir, bajar, frente o fondo).

package com.beatforge.studio.ui.screens.stage

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
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
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.FlipToBack
import androidx.compose.material.icons.rounded.FlipToFront
import androidx.compose.material.icons.rounded.KeyboardArrowDown
import androidx.compose.material.icons.rounded.KeyboardArrowUp
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material.icons.rounded.VisibilityOff
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.theme.StudioColors

@Composable
fun StageLayersPanel(
    entities: List<StageEntitySnapshot>,
    selected: StageEntitySnapshot?,
    onSelectEntity: (String) -> Unit,
    onAddEntity: (name: String, role: String) -> Unit,
    onToggleVisibility: (StageEntitySnapshot) -> Unit,
    onReorderEntity: (entityId: String, action: String) -> Unit,
    modifier: Modifier = Modifier
) {
    Surface(
        modifier = modifier.fillMaxHeight(),
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
                                onClick = { onToggleVisibility(ent) },
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
}
