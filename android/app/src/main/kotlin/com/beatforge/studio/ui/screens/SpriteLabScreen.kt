// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/SpriteLabScreen.kt
// Propósito: Laboratorio de Sprites, Texturas `.astc` y Atlas `.xml` / `.json`.
// Permite seleccionar una entidad del escenario, vincularle su textura (`.png` o `.astc`)
// y su descriptor (`.xml` Sparrow, TexturePacker `.json`, `spritemap1.json` y `Animation.json`),
// reasignar animaciones y previsualizar cada cuadro recortado en vivo a 24 FPS en Compose Canvas.

package com.beatforge.studio.ui.screens

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
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
import androidx.compose.material.icons.rounded.Animation
import androidx.compose.material.icons.rounded.Code
import androidx.compose.material.icons.rounded.Image
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.ImportKind
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.ProjectSnapshot
import com.beatforge.studio.SparrowFrameInfo
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.min
import kotlin.math.roundToInt

private val ANIM_SLOTS = listOf(
    "idle" to "Reposo (Idle)",
    "left" to "Izquierda",
    "down" to "Abajo",
    "up" to "Arriba",
    "right" to "Derecha"
)

@Composable
fun SpriteLabScreen(
    snapshot: ProjectSnapshot,
    selectedEntityId: String,
    entityBitmaps: Map<String, LoadedEntityBitmap>,
    onSelectEntity: (String) -> Unit,
    onRequestImport: (ImportKind) -> Unit,
    onSetBinding: (entityId: String, animKind: String, prefix: String) -> Unit,
    onQuerySparrowFrame: suspend (entityId: String, animKind: String, elapsedSec: Float) -> SparrowFrameInfo
) {
    val entity = snapshot.stageEntities.find { it.id == selectedEntityId }
        ?: snapshot.stageEntities.firstOrNull()

    var activeAnimKind by remember { mutableStateOf("idle") }
    var playingPreview by remember { mutableStateOf(true) }
    var elapsedSec by remember { mutableFloatStateOf(0f) }
    var currentFrame by remember { mutableStateOf(SparrowFrameInfo()) }

    LaunchedEffect(entity?.id, activeAnimKind, playingPreview, entity?.xmlPath, entity?.animateJsonPath, entity?.animBindings) {
        if (entity == null) return@LaunchedEffect
        var lastNs = 0L
        while (true) {
            withFrameNanos { ns ->
                if (lastNs != 0L && playingPreview) {
                    val dt = ((ns - lastNs) / 1_000_000_000f).coerceIn(0f, 0.05f)
                    elapsedSec += dt
                }
                lastNs = ns
            }
            currentFrame = onQuerySparrowFrame(entity.id, activeAnimKind, elapsedSec)
        }
    }

    Row(
        modifier = Modifier
            .fillMaxSize()
            .padding(12.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // Panel izquierdo: Selección de entidad, carga PNG/ASTC + XML/JSON y asignación de prefijos
        Surface(
            modifier = Modifier
                .weight(0.5f)
                .fillMaxHeight(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Column(
                modifier = Modifier
                    .padding(12.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                Text(
                    text = "Sprites · PNG/ASTC + XML/JSON",
                    color = StudioColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 15.sp
                )

                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    snapshot.stageEntities.forEach { ent ->
                        FilterChip(
                            selected = ent.id == entity?.id,
                            onClick = { onSelectEntity(ent.id) },
                            label = { Text(ent.displayName, fontSize = 11.sp) },
                            colors = FilterChipDefaults.filterChipColors(
                                selectedContainerColor = StudioColors.AccentAmber.copy(alpha = 0.22f),
                                selectedLabelColor = StudioColors.AccentAmber
                            )
                        )
                    }
                }

                if (entity != null) {
                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(
                            onClick = { onRequestImport(ImportKind.SpritePng(entity.id)) },
                            contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp),
                            colors = ButtonDefaults.buttonColors(
                                containerColor = StudioColors.AccentCyan,
                                contentColor = Color(0xFF041C24)
                            ),
                            modifier = Modifier.weight(1f)
                        ) {
                            Icon(Icons.Rounded.Image, null, modifier = Modifier.size(16.dp))
                            Spacer(Modifier.width(5.dp))
                            Text("PNG / ASTC", fontSize = 11.sp, fontWeight = FontWeight.Bold, maxLines = 1, softWrap = false)
                        }

                        Button(
                            onClick = { onRequestImport(ImportKind.SparrowXml(entity.id)) },
                            contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp),
                            colors = ButtonDefaults.buttonColors(
                                containerColor = StudioColors.AccentIndigo,
                                contentColor = Color.White
                            ),
                            modifier = Modifier.weight(1f)
                        ) {
                            Icon(Icons.Rounded.Code, null, modifier = Modifier.size(16.dp))
                            Spacer(Modifier.width(5.dp))
                            Text("XML / JSON", fontSize = 11.sp, fontWeight = FontWeight.Bold, maxLines = 1, softWrap = false)
                        }
                    }

                    Surface(
                        shape = RoundedCornerShape(8.dp),
                        color = StudioColors.BgCard,
                        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
                    ) {
                        val texDisplay = entity.rawImagePath ?: entity.imagePath
                        Column(modifier = Modifier.padding(9.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                            Text(
                                text = "Textura: ${texDisplay?.substringAfterLast('/') ?: "No asignada"}",
                                color = if (texDisplay != null) StudioColors.AccentEmerald else StudioColors.TextMuted,
                                fontSize = 11.sp
                            )
                            Text(
                                text = "Atlas XML/Spritemap: ${entity.xmlPath?.substringAfterLast('/') ?: "No asignado"} (${entity.animationPrefixes.size} anims)",
                                color = if (entity.xmlPath != null) StudioColors.AccentCyan else StudioColors.TextMuted,
                                fontSize = 11.sp
                            )
                            val xmlIsJson = entity.xmlPath?.endsWith(".json", ignoreCase = true) == true
                            if (entity.animateJsonPath != null || xmlIsJson) {
                                Text(
                                    text = "Animate Rig: ${entity.animateJsonPath?.substringAfterLast('/') ?: "Falta importar Animation.json"}",
                                    color = if (entity.animateJsonPath != null) StudioColors.AccentAmber else StudioColors.TextMuted,
                                    fontSize = 11.sp
                                )
                            }
                            Text(
                                text = "Compatible con .xml Sparrow, .json y 3 archivos Animate (Animation.json + spritemap1.json + PNG/.astc)",
                                color = StudioColors.TextMuted,
                                fontSize = 10.sp,
                                lineHeight = 13.sp
                            )
                        }
                    }

                    Text(
                        text = "Asignación de Animaciones",
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.SemiBold,
                        fontSize = 13.sp
                    )

                    ANIM_SLOTS.forEach { (kindKey, label) ->
                        var expanded by remember(entity.id, kindKey) { mutableStateOf(false) }
                        val currentPrefix = entity.animBindings[kindKey] ?: "(Sin asignar)"

                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween
                        ) {
                            Text(
                                text = label,
                                color = StudioColors.TextMuted,
                                fontSize = 12.sp,
                                modifier = Modifier.weight(0.42f)
                            )
                            Box(modifier = Modifier.weight(0.58f)) {
                                OutlinedButton(
                                    onClick = {
                                        activeAnimKind = kindKey
                                        if (entity.animationPrefixes.isNotEmpty()) {
                                            expanded = true
                                        }
                                    },
                                    contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .height(34.dp)
                                ) {
                                    Text(
                                        text = currentPrefix,
                                        color = StudioColors.TextPrimary,
                                        fontSize = 11.sp,
                                        maxLines = 1
                                    )
                                }
                                DropdownMenu(
                                    expanded = expanded,
                                    onDismissRequest = { expanded = false }
                                ) {
                                    entity.animationPrefixes.forEach { prefix ->
                                        DropdownMenuItem(
                                            text = { Text(prefix, fontSize = 12.sp) },
                                            onClick = {
                                                expanded = false
                                                activeAnimKind = kindKey
                                                onSetBinding(entity.id, kindKey, prefix)
                                            }
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Panel derecho: Previsualización en vivo a 24 FPS
        Surface(
            modifier = Modifier
                .weight(0.5f)
                .fillMaxHeight(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Column(
                modifier = Modifier.padding(12.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(Icons.Rounded.Animation, null, tint = StudioColors.AccentAmber, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(6.dp))
                        Text(
                            text = "Vista Previa 24 FPS",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 14.sp
                        )
                    }

                    Button(
                        onClick = { playingPreview = !playingPreview },
                        contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = StudioColors.BgCard,
                            contentColor = StudioColors.AccentCyan
                        ),
                        modifier = Modifier.height(32.dp)
                    ) {
                        Icon(
                            imageVector = if (playingPreview) Icons.Rounded.Pause else Icons.Rounded.PlayArrow,
                            contentDescription = null,
                            modifier = Modifier.size(15.dp)
                        )
                        Spacer(Modifier.width(4.dp))
                        Text(if (playingPreview) "Pausar" else "Reproducir", fontSize = 11.sp)
                    }
                }

                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    ANIM_SLOTS.forEach { (kindKey, label) ->
                        FilterChip(
                            selected = activeAnimKind == kindKey,
                            onClick = {
                                activeAnimKind = kindKey
                                elapsedSec = 0f
                            },
                            label = { Text(label.substringBefore(" "), fontSize = 11.sp) }
                        )
                    }
                }

                val loaded = entity?.let { entityBitmaps[it.id] }
                Canvas(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        .background(StudioColors.BgDark, RoundedCornerShape(10.dp))
                ) {
                    if (loaded != null) {
                        val bmp = loaded.bitmap
                        val scaleX = bmp.width.toFloat() / loaded.origWidth.toFloat()
                        val scaleY = bmp.height.toFloat() / loaded.origHeight.toFloat()

                        val srcX: Int
                        val srcY: Int
                        val srcW: Int
                        val srcH: Int
                        if (currentFrame.found && currentFrame.width > 0 && currentFrame.height > 0) {
                            srcX = (currentFrame.x * scaleX).roundToInt().coerceIn(0, bmp.width - 1)
                            srcY = (currentFrame.y * scaleY).roundToInt().coerceIn(0, bmp.height - 1)
                            srcW = (currentFrame.width * scaleX).roundToInt().coerceIn(1, bmp.width - srcX)
                            srcH = (currentFrame.height * scaleY).roundToInt().coerceIn(1, bmp.height - srcY)

                            val visW = if (currentFrame.rotated) currentFrame.height else currentFrame.width
                            val visH = if (currentFrame.rotated) currentFrame.width else currentFrame.height
                            val logicalW = currentFrame.frameWidth.coerceAtLeast(visW).toFloat().coerceAtLeast(1f)
                            val logicalH = currentFrame.frameHeight.coerceAtLeast(visH).toFloat().coerceAtLeast(1f)
                            val fit = min((size.width * 0.85f) / logicalW, (size.height * 0.85f) / logicalH)

                            val boxW = logicalW * fit
                            val boxH = logicalH * fit
                            val boxLeft = (size.width - boxW) / 2f
                            val boxTop = (size.height - boxH) / 2f

                            val visDrawW = visW * fit
                            val visDrawH = visH * fit
                            val visLeft = boxLeft + (-currentFrame.frameX * fit)
                            val visTop = boxTop + (-currentFrame.frameY * fit)
                            val centerX = visLeft + visDrawW / 2f
                            val centerY = visTop + visDrawH / 2f

                            val dstW = (currentFrame.width * fit).roundToInt().coerceAtLeast(1)
                            val dstH = (currentFrame.height * fit).roundToInt().coerceAtLeast(1)
                            val dstX = (centerX - dstW / 2f).roundToInt()
                            val dstY = (centerY - dstH / 2f).roundToInt()

                            withTransform({
                                if (currentFrame.rotated) {
                                    rotate(-90f, pivot = Offset(centerX, centerY))
                                }
                            }) {
                                drawImage(
                                    image = bmp,
                                    srcOffset = IntOffset(srcX, srcY),
                                    srcSize = IntSize(srcW, srcH),
                                    dstOffset = IntOffset(dstX, dstY),
                                    dstSize = IntSize(dstW, dstH)
                                )
                            }
                        } else {
                            srcX = 0
                            srcY = 0
                            srcW = bmp.width
                            srcH = bmp.height
                            val fit = min((size.width * 0.85f) / srcW, (size.height * 0.85f) / srcH)
                            val dstW = (srcW * fit).roundToInt().coerceAtLeast(1)
                            val dstH = (srcH * fit).roundToInt().coerceAtLeast(1)
                            val dstX = ((size.width - dstW) / 2f).roundToInt()
                            val dstY = ((size.height - dstH) / 2f).roundToInt()
                            drawImage(
                                image = bmp,
                                srcOffset = IntOffset(srcX, srcY),
                                srcSize = IntSize(srcW, srcH),
                                dstOffset = IntOffset(dstX, dstY),
                                dstSize = IntSize(dstW, dstH)
                            )
                        }
                    }
                }

                Text(
                    text = if (currentFrame.found) {
                        "Frame: ${currentFrame.rawName} (${currentFrame.width}x${currentFrame.height} px)"
                    } else if (loaded != null) {
                        "Mostrando textura completa (${loaded.origWidth}x${loaded.origHeight} px)"
                    } else {
                        "Importa PNG/.astc y su descriptor XML/.json para previsualizar en vivo"
                    },
                    color = StudioColors.TextMuted,
                    fontSize = 11.sp
                )
            }
        }
    }
}
