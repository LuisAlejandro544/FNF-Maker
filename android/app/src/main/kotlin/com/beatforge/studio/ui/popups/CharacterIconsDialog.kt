// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/CharacterIconsDialog.kt
// Propósito: Ventana emergente modal para asignar los íconos normal y perdiendo (<20% vida)
// de las entidades del escenario (Jugador y Enemigo) e importar o restaurar el SVG
// personalizado de la barra de vida rasterizado con `resvg` en Rust.

package com.beatforge.studio.ui.popups

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
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
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Face
import androidx.compose.material.icons.rounded.Favorite
import androidx.compose.material.icons.rounded.RestartAlt
import androidx.compose.material.icons.rounded.UploadFile
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.beatforge.studio.ImportKind
import com.beatforge.studio.StageEntitySnapshot
import com.beatforge.studio.ui.theme.StudioColors

@Composable
fun CharacterIconsDialog(
    entities: List<StageEntitySnapshot>,
    healthBarSvgPath: String?,
    healthBarBitmap: ImageBitmap?,
    entityNormalIcons: Map<String, ImageBitmap>,
    entityLosingIcons: Map<String, ImageBitmap>,
    defaultPlayerNormal: ImageBitmap?,
    defaultPlayerLosing: ImageBitmap?,
    defaultOpponentNormal: ImageBitmap?,
    defaultOpponentLosing: ImageBitmap?,
    onRequestImport: (ImportKind) -> Unit,
    onClearEntityIcons: (entityId: String) -> Unit,
    onRestoreDefaultHealthBar: () -> Unit,
    onDismiss: () -> Unit
) {
    val iconEntities = entities.filter { it.role == "player" || it.role == "opponent" }
        .ifEmpty { entities.take(2) }

    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false)
    ) {
        Surface(
            modifier = Modifier
                .fillMaxWidth(0.84f)
                .padding(16.dp),
            shape = RoundedCornerShape(14.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Column(
                modifier = Modifier
                    .padding(16.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Face,
                        contentDescription = null,
                        tint = StudioColors.AccentCyan,
                        modifier = Modifier.size(22.dp)
                    )
                    Spacer(Modifier.width(8.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = "Íconos de Personajes y Barra de Vida (SVG)",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 15.sp
                        )
                        Text(
                            text = "Acepta PNG/WebP individuales o tiras dobles (normal + perdiendo)",
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp
                        )
                    }
                    IconButton(onClick = onDismiss) {
                        Icon(Icons.Rounded.Close, "Cerrar", tint = StudioColors.TextMuted)
                    }
                }

                // Sección de barra de vida SVG
                Surface(
                    shape = RoundedCornerShape(10.dp),
                    color = StudioColors.BgCard,
                    border = BorderStroke(1.dp, StudioColors.BorderSubtle)
                ) {
                    Column(
                        modifier = Modifier.padding(10.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp)
                    ) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Icon(Icons.Rounded.Favorite, null, tint = StudioColors.AccentCoral, modifier = Modifier.size(16.dp))
                                Spacer(Modifier.width(6.dp))
                                Text(
                                    text = if (healthBarSvgPath != null) {
                                        "Barra SVG personalizada: ${healthBarSvgPath.substringAfterLast('/')}"
                                    } else {
                                        "Marco de Barra de Vida SVG (Original Vector)"
                                    },
                                    color = StudioColors.TextPrimary,
                                    fontWeight = FontWeight.SemiBold,
                                    fontSize = 12.sp
                                )
                            }
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                OutlinedButton(
                                    onClick = onRestoreDefaultHealthBar,
                                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                                    modifier = Modifier.height(30.dp)
                                ) {
                                    Icon(Icons.Rounded.RestartAlt, null, modifier = Modifier.size(14.dp))
                                    Spacer(Modifier.width(4.dp))
                                    Text("Original", fontSize = 11.sp)
                                }
                                Button(
                                    onClick = { onRequestImport(ImportKind.HealthBarSvg) },
                                    contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                                    colors = ButtonDefaults.buttonColors(
                                        containerColor = StudioColors.AccentCyan,
                                        contentColor = Color(0xFF041C24)
                                    ),
                                    modifier = Modifier.height(30.dp)
                                ) {
                                    Icon(Icons.Rounded.UploadFile, null, modifier = Modifier.size(14.dp))
                                    Spacer(Modifier.width(4.dp))
                                    Text("Cargar .svg", fontSize = 11.sp, fontWeight = FontWeight.Bold)
                                }
                            }
                        }

                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .height(34.dp)
                                .background(StudioColors.BgDark, RoundedCornerShape(8.dp)),
                            contentAlignment = Alignment.Center
                        ) {
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth(0.75f)
                                    .height(14.dp)
                            ) {
                                Box(
                                    modifier = Modifier
                                        .weight(1f)
                                        .height(14.dp)
                                        .background(Color(0xFFE11D48))
                                )
                                Box(
                                    modifier = Modifier
                                        .weight(1f)
                                        .height(14.dp)
                                        .background(Color(0xFF22C55E))
                                )
                            }
                            if (healthBarBitmap != null) {
                                Image(
                                    bitmap = healthBarBitmap,
                                    contentDescription = "Barra de vida SVG",
                                    filterQuality = FilterQuality.High,
                                    modifier = Modifier
                                        .fillMaxWidth(0.78f)
                                        .height(22.dp)
                                )
                            }
                        }
                    }
                }

                // Tarjetas de íconos por personaje
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                    iconEntities.forEach { ent ->
                        val isPlayer = ent.role == "player"
                        val normalBmp = entityNormalIcons[ent.id]
                            ?: if (isPlayer) defaultPlayerNormal else defaultOpponentNormal
                        val losingBmp = entityLosingIcons[ent.id]
                            ?: if (isPlayer) defaultPlayerLosing else defaultOpponentLosing
                        val accent = if (isPlayer) StudioColors.AccentEmerald else StudioColors.AccentCoral

                        Surface(
                            modifier = Modifier.weight(1f),
                            shape = RoundedCornerShape(10.dp),
                            color = StudioColors.BgCard,
                            border = BorderStroke(1.dp, accent.copy(alpha = 0.5f))
                        ) {
                            Column(
                                modifier = Modifier.padding(10.dp),
                                verticalArrangement = Arrangement.spacedBy(8.dp)
                            ) {
                                Row(
                                    modifier = Modifier.fillMaxWidth(),
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                    verticalAlignment = Alignment.CenterVertically
                                ) {
                                    Text(
                                        text = "${ent.displayName} (${if (isPlayer) "Jugador" else "Enemigo"})",
                                        color = accent,
                                        fontWeight = FontWeight.Bold,
                                        fontSize = 12.sp
                                    )
                                    if (ent.iconNormalPath != null || ent.iconLosingPath != null) {
                                        IconButton(
                                            onClick = { onClearEntityIcons(ent.id) },
                                            modifier = Modifier.size(24.dp)
                                        ) {
                                            Icon(
                                                Icons.Rounded.DeleteOutline,
                                                "Restaurar íconos originales",
                                                tint = StudioColors.TextMuted,
                                                modifier = Modifier.size(16.dp)
                                            )
                                        }
                                    }
                                }

                                Row(
                                    modifier = Modifier.fillMaxWidth(),
                                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                                ) {
                                    IconSlotPreview(
                                        title = "Normal",
                                        bitmap = normalBmp,
                                        isCustom = ent.iconNormalPath != null,
                                        onUpload = {
                                            onRequestImport(ImportKind.CharacterIcon(ent.id, isLosing = false))
                                        },
                                        modifier = Modifier.weight(1f)
                                    )
                                    IconSlotPreview(
                                        title = "Perdiendo (<20%)",
                                        bitmap = losingBmp,
                                        isCustom = ent.iconLosingPath != null,
                                        onUpload = {
                                            onRequestImport(ImportKind.CharacterIcon(ent.id, isLosing = true))
                                        },
                                        modifier = Modifier.weight(1f)
                                    )
                                }
                            }
                        }
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End
                ) {
                    Button(onClick = onDismiss) {
                        Text("Listo", fontSize = 12.sp, fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}

@Composable
private fun IconSlotPreview(
    title: String,
    bitmap: ImageBitmap?,
    isCustom: Boolean,
    onUpload: () -> Unit,
    modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(5.dp)
    ) {
        Text(title, color = StudioColors.TextMuted, fontSize = 10.sp, maxLines = 1)
        Box(
            modifier = Modifier
                .size(54.dp)
                .background(StudioColors.BgDark, RoundedCornerShape(8.dp)),
            contentAlignment = Alignment.Center
        ) {
            if (bitmap != null) {
                Image(
                    bitmap = bitmap,
                    contentDescription = title,
                    filterQuality = FilterQuality.High,
                    modifier = Modifier.size(46.dp)
                )
            } else {
                Text("SVG", color = StudioColors.TextMuted, fontSize = 10.sp)
            }
        }
        Button(
            onClick = onUpload,
            contentPadding = PaddingValues(horizontal = 6.dp, vertical = 2.dp),
            colors = ButtonDefaults.buttonColors(
                containerColor = if (isCustom) StudioColors.AccentIndigo else StudioColors.BgElevated,
                contentColor = StudioColors.TextPrimary
            ),
            modifier = Modifier
                .fillMaxWidth()
                .height(28.dp)
        ) {
            Text(if (isCustom) "Cambiar PNG" else "Subir PNG", fontSize = 10.sp, maxLines = 1)
        }
    }
}
