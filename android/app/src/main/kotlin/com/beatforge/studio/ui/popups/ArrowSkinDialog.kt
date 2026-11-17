// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/ArrowSkinDialog.kt
// Propósito: Ventana emergente modal para visualizar y reemplazar la skin vectorial SVG
// de las 4 flechas (Izquierda, Abajo, Arriba, Derecha). Cada SVG es rasterizado por
// `resvg` en el motor Rust y devuelto como PNG limpio para vista previa y gameplay.

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
import androidx.compose.material.icons.rounded.Code
import androidx.compose.material.icons.rounded.Image
import androidx.compose.material.icons.rounded.Palette
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
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.beatforge.studio.ui.theme.StudioColors

private val LANE_LABELS = listOf("Izquierda", "Abajo", "Arriba", "Derecha")

@Composable
fun ArrowSkinDialog(
    skinName: String,
    customPaths: Map<String, String>,
    hasCustomPngArrowSkin: Boolean,
    stagedPngPath: String?,
    stagedXmlPath: String?,
    arrowBitmaps: Map<Int, ImageBitmap>,
    hurtArrowXBitmap: ImageBitmap?,
    onImportSvg: (Int) -> Unit,
    onImportArrowAtlasPng: () -> Unit,
    onImportArrowAtlasXml: () -> Unit,
    onClearStagedArrowAtlas: () -> Unit,
    onRestoreDefault: () -> Unit,
    onDismiss: () -> Unit
) {
    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false)
    ) {
        Surface(
            modifier = Modifier
                .fillMaxWidth(0.84f)
                .padding(14.dp),
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
                        imageVector = Icons.Rounded.Palette,
                        contentDescription = null,
                        tint = StudioColors.AccentPurple,
                        modifier = Modifier.size(22.dp)
                    )
                    Spacer(Modifier.width(8.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = "Skin de Flechas (SVG Vectorial / PNG + XML de Mods)",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 15.sp
                        )
                        Text(
                            text = "Skin actual: $skinName${if (hasCustomPngArrowSkin) " (Atlas PNG+XML activo)" else ""}",
                            color = StudioColors.TextMuted,
                            fontSize = 11.5.sp
                        )
                    }
                    IconButton(onClick = onDismiss) {
                        Icon(
                            imageVector = Icons.Rounded.Close,
                            contentDescription = "Cerrar",
                            tint = StudioColors.TextMuted
                        )
                    }
                }

                // Sección para importar skins estándar de mods (.png + .xml / NOTE_assets)
                Surface(
                    shape = RoundedCornerShape(10.dp),
                    color = StudioColors.BgCard,
                    border = BorderStroke(
                        1.dp,
                        if (hasCustomPngArrowSkin) StudioColors.AccentEmerald else StudioColors.AccentCyan.copy(alpha = 0.45f)
                    )
                ) {
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(10.dp),
                        verticalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween
                        ) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = "Importar Skin Estándar de Mod (.PNG + .XML / NOTE_assets)",
                                    color = StudioColors.TextPrimary,
                                    fontWeight = FontWeight.Bold,
                                    fontSize = 12.5.sp
                                )
                                Text(
                                    text = "En mods suelen estar en: images/NOTE_assets.png + .xml, mods/images/NOTE_assets.* o assets/shared/images/",
                                    color = StudioColors.TextMuted,
                                    fontSize = 10.sp
                                )
                            }
                            if (stagedPngPath != null || stagedXmlPath != null) {
                                OutlinedButton(
                                    onClick = onClearStagedArrowAtlas,
                                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp),
                                    modifier = Modifier.height(26.dp)
                                ) {
                                    Text("Limpiar", fontSize = 10.sp)
                                }
                            }
                        }

                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.spacedBy(8.dp)
                        ) {
                            Button(
                                onClick = onImportArrowAtlasPng,
                                shape = RoundedCornerShape(8.dp),
                                contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp),
                                colors = ButtonDefaults.buttonColors(
                                    containerColor = if (stagedPngPath != null) StudioColors.AccentEmerald else StudioColors.AccentCyan,
                                    contentColor = Color(0xFF041C24)
                                ),
                                modifier = Modifier.weight(1f)
                            ) {
                                Icon(Icons.Rounded.Image, contentDescription = null, modifier = Modifier.size(15.dp))
                                Spacer(Modifier.width(5.dp))
                                Text(
                                    text = if (stagedPngPath != null) "1. PNG Listo ✓" else "1. Hoja .PNG",
                                    fontSize = 11.sp,
                                    fontWeight = FontWeight.Bold,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis
                                )
                            }

                            Button(
                                onClick = onImportArrowAtlasXml,
                                shape = RoundedCornerShape(8.dp),
                                contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp),
                                colors = ButtonDefaults.buttonColors(
                                    containerColor = if (stagedXmlPath != null) StudioColors.AccentEmerald else StudioColors.AccentIndigo,
                                    contentColor = Color.White
                                ),
                                modifier = Modifier.weight(1f)
                            ) {
                                Icon(Icons.Rounded.Code, contentDescription = null, modifier = Modifier.size(15.dp))
                                Spacer(Modifier.width(5.dp))
                                Text(
                                    text = if (stagedXmlPath != null) "2. XML Listo ✓" else "2. Atlas .XML",
                                    fontSize = 11.sp,
                                    fontWeight = FontWeight.Bold,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis
                                )
                            }
                        }
                    }
                }

                // Vista previa de las 4 flechas + Nota Especial "X"
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    for (dir in 0..3) {
                        val isCustom = customPaths.containsKey(dir.toString())
                        val bmp = arrowBitmaps[dir]
                        Surface(
                            modifier = Modifier.weight(1f),
                            shape = RoundedCornerShape(10.dp),
                            color = StudioColors.BgCard,
                            border = BorderStroke(
                                1.dp,
                                if (isCustom) StudioColors.LaneColors[dir] else StudioColors.BorderSubtle
                            )
                        ) {
                            Column(
                                modifier = Modifier.padding(8.dp),
                                horizontalAlignment = Alignment.CenterHorizontally
                            ) {
                                Text(
                                    text = LANE_LABELS[dir],
                                    color = StudioColors.LaneColors[dir],
                                    fontWeight = FontWeight.Bold,
                                    fontSize = 12.sp
                                )
                                Spacer(Modifier.height(6.dp))
                                Box(
                                    modifier = Modifier
                                        .size(56.dp)
                                        .background(StudioColors.BgDark, RoundedCornerShape(8.dp)),
                                    contentAlignment = Alignment.Center
                                ) {
                                    if (bmp != null) {
                                        Image(
                                            bitmap = bmp,
                                            contentDescription = LANE_LABELS[dir],
                                            modifier = Modifier.size(44.dp)
                                        )
                                    } else {
                                        Text("SVG", color = StudioColors.TextMuted, fontSize = 11.sp)
                                    }
                                }
                                Spacer(Modifier.height(5.dp))
                                Text(
                                    text = if (isCustom) "Personalizado" else "Original",
                                    color = if (isCustom) StudioColors.AccentEmerald else StudioColors.TextMuted,
                                    fontSize = 10.sp
                                )
                                Spacer(Modifier.height(6.dp))
                                Button(
                                    onClick = { onImportSvg(dir) },
                                    shape = RoundedCornerShape(8.dp),
                                    contentPadding = PaddingValues(horizontal = 6.dp, vertical = 4.dp),
                                    colors = ButtonDefaults.buttonColors(
                                        containerColor = StudioColors.BgElevated,
                                        contentColor = StudioColors.TextPrimary
                                    ),
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .height(30.dp)
                                ) {
                                    Icon(
                                        imageVector = Icons.Rounded.UploadFile,
                                        contentDescription = null,
                                        modifier = Modifier.size(13.dp)
                                    )
                                    Spacer(Modifier.width(4.dp))
                                    Text("Subir .svg", fontSize = 10.sp)
                                }
                            }
                        }
                    }

                    // 5ta tarjeta: Nota Especial Peligrosa "X" (hurt / instakill)
                    Surface(
                        modifier = Modifier.weight(1f),
                        shape = RoundedCornerShape(10.dp),
                        color = StudioColors.BgCard,
                        border = BorderStroke(1.dp, StudioColors.AccentCoral.copy(alpha = 0.7f))
                    ) {
                        Column(
                            modifier = Modifier.padding(8.dp),
                            horizontalAlignment = Alignment.CenterHorizontally
                        ) {
                            Text(
                                text = "Nota X",
                                color = StudioColors.AccentCoral,
                                fontWeight = FontWeight.Bold,
                                fontSize = 12.sp
                            )
                            Spacer(Modifier.height(6.dp))
                            Box(
                                modifier = Modifier
                                    .size(56.dp)
                                    .background(StudioColors.BgDark, RoundedCornerShape(8.dp)),
                                contentAlignment = Alignment.Center
                            ) {
                                if (hurtArrowXBitmap != null) {
                                    Image(
                                        bitmap = hurtArrowXBitmap,
                                        contentDescription = "Nota Especial X",
                                        modifier = Modifier.size(44.dp)
                                    )
                                } else {
                                    Text(
                                        text = "X",
                                        color = StudioColors.AccentCoral,
                                        fontWeight = FontWeight.Black,
                                        fontSize = 24.sp
                                    )
                                }
                            }
                            Spacer(Modifier.height(5.dp))
                            Text(
                                text = "Daño / Muerte",
                                color = StudioColors.AccentCoral,
                                fontSize = 10.sp,
                                maxLines = 1
                            )
                            Spacer(Modifier.height(6.dp))
                            Text(
                                text = "SVG X integrado",
                                color = StudioColors.TextMuted,
                                fontSize = 9.5.sp,
                                maxLines = 1
                            )
                        }
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    OutlinedButton(onClick = onRestoreDefault) {
                        Icon(
                            imageVector = Icons.Rounded.RestartAlt,
                            contentDescription = null,
                            modifier = Modifier.size(16.dp)
                        )
                        Spacer(Modifier.width(6.dp))
                        Text("Restaurar SVG Original", fontSize = 12.sp)
                    }

                    Button(onClick = onDismiss) {
                        Text("Listo", fontSize = 13.sp, fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}
