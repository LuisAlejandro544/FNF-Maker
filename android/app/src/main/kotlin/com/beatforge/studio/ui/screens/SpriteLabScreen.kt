// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/SpriteLabScreen.kt
// Propósito: Laboratorio de Sprites, Texturas `.astc`, Atlas `.xml`/`.json` y Descriptores
// de Personaje (`.json`/`.xml`). Permite vincular textura, atlas, archivo de posición/offsets
// del personaje, calibrar manualmente `Offset X/Y` por pose y previsualizar en vivo a 24 FPS.

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
import androidx.compose.material.icons.rounded.Face
import androidx.compose.material.icons.rounded.HelpOutline
import androidx.compose.material.icons.rounded.Image
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.Place
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
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.ImageBitmap
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
    "right" to "Derecha",
    "miss_left" to "Fallo Izquierda",
    "miss_down" to "Fallo Abajo",
    "miss_up" to "Fallo Arriba",
    "miss_right" to "Fallo Derecha"
)

@Composable
fun SpriteLabScreen(
    snapshot: ProjectSnapshot,
    selectedEntityId: String,
    entityBitmaps: Map<String, LoadedEntityBitmap>,
    onSelectEntity: (String) -> Unit,
    onRequestImport: (ImportKind) -> Unit,
    onOpenIconsDialog: () -> Unit,
    onOpenFilesGuide: () -> Unit,
    onSetBinding: (entityId: String, animKind: String, prefix: String) -> Unit,
    onSetAnimOffset: (entityId: String, animKind: String, offsetX: Float, offsetY: Float) -> Unit,
    onQuerySparrowFrame: suspend (entityId: String, animKind: String, elapsedSec: Float) -> SparrowFrameInfo,
    onDecodeRegionFrame: suspend (imagePath: String?, frame: SparrowFrameInfo) -> ImageBitmap?
) {
    val entity = snapshot.stageEntities.find { it.id == selectedEntityId }
        ?: snapshot.stageEntities.firstOrNull()

    var activeAnimKind by remember { mutableStateOf("idle") }
    var playingPreview by remember { mutableStateOf(true) }
    var elapsedSec by remember { mutableFloatStateOf(0f) }
    var currentFrame by remember { mutableStateOf(SparrowFrameInfo()) }
    var regionBitmap by remember { mutableStateOf<ImageBitmap?>(null) }

    LaunchedEffect(entity?.id, activeAnimKind, playingPreview, entity?.imagePath, entity?.xmlPath, entity?.animateJsonPath, entity?.animBindings) {
        if (entity == null) {
            regionBitmap = null
            return@LaunchedEffect
        }
        var lastNs = 0L
        while (true) {
            withFrameNanos { ns ->
                if (lastNs != 0L && playingPreview) {
                    val dt = ((ns - lastNs) / 1_000_000_000f).coerceIn(0f, 0.05f)
                    elapsedSec += dt
                }
                lastNs = ns
            }
            val nextFrame = onQuerySparrowFrame(entity.id, activeAnimKind, elapsedSec)
            currentFrame = nextFrame
            if (nextFrame.found && !entity.imagePath.isNullOrBlank()) {
                val decoded = onDecodeRegionFrame(entity.imagePath, nextFrame)
                if (decoded != null) {
                    regionBitmap = decoded
                }
            } else {
                regionBitmap = null
            }
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
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = "Sprites · PNG/ASTC + XML/JSON",
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 15.sp
                    )
                    OutlinedButton(
                        onClick = onOpenFilesGuide,
                        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp),
                        modifier = Modifier.height(28.dp)
                    ) {
                        Icon(
                            Icons.Rounded.HelpOutline,
                            contentDescription = null,
                            tint = StudioColors.AccentCyan,
                            modifier = Modifier.size(14.dp)
                        )
                        Spacer(Modifier.width(4.dp))
                        Text(
                            text = "¿Dónde van?",
                            color = StudioColors.AccentCyan,
                            fontSize = 10.sp,
                            fontWeight = FontWeight.Bold,
                            maxLines = 1,
                            softWrap = false
                        )
                    }
                }

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
                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            Button(
                                onClick = { onRequestImport(ImportKind.SpritePng(entity.id)) },
                                contentPadding = PaddingValues(horizontal = 8.dp, vertical = 6.dp),
                                colors = ButtonDefaults.buttonColors(
                                    containerColor = StudioColors.AccentCyan,
                                    contentColor = Color(0xFF041C24)
                                ),
                                modifier = Modifier.weight(1f)
                            ) {
                                Icon(Icons.Rounded.Image, null, modifier = Modifier.size(15.dp))
                                Spacer(Modifier.width(4.dp))
                                Text("PNG / ASTC", fontSize = 10.5.sp, fontWeight = FontWeight.Bold, maxLines = 1, softWrap = false)
                            }

                            Button(
                                onClick = { onRequestImport(ImportKind.SparrowXml(entity.id)) },
                                contentPadding = PaddingValues(horizontal = 8.dp, vertical = 6.dp),
                                colors = ButtonDefaults.buttonColors(
                                    containerColor = StudioColors.AccentIndigo,
                                    contentColor = Color.White
                                ),
                                modifier = Modifier.weight(1f)
                            ) {
                                Icon(Icons.Rounded.Code, null, modifier = Modifier.size(15.dp))
                                Spacer(Modifier.width(4.dp))
                                Text("XML / JSON", fontSize = 10.5.sp, fontWeight = FontWeight.Bold, maxLines = 1, softWrap = false)
                            }
                        }

                        Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                            Button(
                                onClick = { onRequestImport(ImportKind.CharacterConfig(entity.id)) },
                                contentPadding = PaddingValues(horizontal = 8.dp, vertical = 6.dp),
                                colors = ButtonDefaults.buttonColors(
                                    containerColor = StudioColors.AccentEmerald,
                                    contentColor = Color(0xFF042217)
                                ),
                                modifier = Modifier.weight(1.15f)
                            ) {
                                Icon(Icons.Rounded.Place, null, modifier = Modifier.size(15.dp))
                                Spacer(Modifier.width(4.dp))
                                Text("Posición (.json/.xml)", fontSize = 10.5.sp, fontWeight = FontWeight.Bold, maxLines = 1, softWrap = false)
                            }

                            OutlinedButton(
                                onClick = onOpenIconsDialog,
                                contentPadding = PaddingValues(horizontal = 8.dp, vertical = 6.dp),
                                modifier = Modifier.weight(0.85f)
                            ) {
                                Icon(Icons.Rounded.Face, null, tint = StudioColors.AccentAmber, modifier = Modifier.size(15.dp))
                                Spacer(Modifier.width(4.dp))
                                Text("Íconos / Vida", fontSize = 10.5.sp, color = StudioColors.AccentAmber, fontWeight = FontWeight.Bold, maxLines = 1, softWrap = false)
                            }
                        }
                    }

                    Surface(
                        shape = RoundedCornerShape(8.dp),
                        color = StudioColors.BgCard,
                        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
                    ) {
                        val texDisplay = entity.rawImagePath ?: entity.imagePath
                        val xmlIsJson = entity.xmlPath?.endsWith(".json", ignoreCase = true) == true
                        Column(modifier = Modifier.padding(9.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                            Text(
                                text = "Textura: ${texDisplay?.substringAfterLast('/') ?: "No asignada"}",
                                color = if (texDisplay != null) StudioColors.AccentEmerald else StudioColors.TextMuted,
                                fontSize = 11.sp
                            )
                            Text(
                                text = if (entity.xmlPath != null) {
                                    "Atlas XML/JSON: ${entity.xmlPath.substringAfterLast('/')} (${entity.animationPrefixes.size} anims)"
                                } else if (entity.animateJsonPath != null) {
                                    "Atlas XML/JSON: Falta subir spritemap1.json en XML / JSON"
                                } else {
                                    "Atlas XML/JSON: No asignado (${entity.animationPrefixes.size} anims)"
                                },
                                color = if (entity.xmlPath != null) StudioColors.AccentCyan else StudioColors.TextMuted,
                                fontSize = 11.sp
                            )
                            Text(
                                text = if (entity.characterConfigPath != null) {
                                    "Posición/Offsets: ${entity.characterConfigPath.substringAfterLast('/')} · Pos (${entity.x.roundToInt()}, ${entity.y.roundToInt()})"
                                } else {
                                    "Posición/Offsets (.json/.xml): Opcional · Pos actual (${entity.x.roundToInt()}, ${entity.y.roundToInt()})"
                                },
                                color = if (entity.characterConfigPath != null) StudioColors.AccentEmerald else StudioColors.TextMuted,
                                fontSize = 11.sp
                            )
                            if (entity.animateJsonPath != null || xmlIsJson) {
                                Text(
                                    text = if (entity.animateJsonPath != null) {
                                        "Animate Rig (2.º JSON): ${entity.animateJsonPath.substringAfterLast('/')}"
                                    } else {
                                        "2.º JSON (opcional Animate): Sube Animation.json en XML / JSON"
                                    },
                                    color = if (entity.animateJsonPath != null) StudioColors.AccentAmber else StudioColors.TextMuted,
                                    fontSize = 11.sp
                                )
                            }
                            Text(
                                text = "Guía rápida de archivos:\n" +
                                    "• PNG / ASTC: hoja de imagen (en mods: images/characters/).\n" +
                                    "• XML / JSON: recorte de frames Sparrow/Atlas (en images/characters/).\n" +
                                    "• Posición (.json/.xml): ubicación en escena y offsets (en characters/ o mods/characters/).",
                                color = StudioColors.TextMuted,
                                fontSize = 10.sp,
                                lineHeight = 14.sp
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
                val sharpFrame = regionBitmap
                val activeOffset = entity?.animOffsets?.get(activeAnimKind) ?: (0f to 0f)
                val offX = activeOffset.first
                val offY = activeOffset.second

                Canvas(
                    modifier = Modifier
                        .fillMaxWidth()
                        .weight(1f)
                        .background(StudioColors.BgDark, RoundedCornerShape(10.dp))
                ) {
                    if (currentFrame.found && currentFrame.width > 0 && currentFrame.height > 0 && (sharpFrame != null || loaded != null)) {
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
                        val visLeft = boxLeft + ((-currentFrame.frameX + offX) * fit)
                        val visTop = boxTop + ((-currentFrame.frameY + offY) * fit)
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
                            if (sharpFrame != null) {
                                // Renderizado 1:1 directo de la región recortada con máxima nitidez
                                drawImage(
                                    image = sharpFrame,
                                    srcOffset = IntOffset(0, 0),
                                    srcSize = IntSize(sharpFrame.width, sharpFrame.height),
                                    dstOffset = IntOffset(dstX, dstY),
                                    dstSize = IntSize(dstW, dstH),
                                    filterQuality = FilterQuality.High
                                )
                            } else if (loaded != null) {
                                val bmp = loaded.bitmap
                                val scaleX = bmp.width.toFloat() / loaded.origWidth.toFloat()
                                val scaleY = bmp.height.toFloat() / loaded.origHeight.toFloat()
                                val srcX = (currentFrame.x * scaleX).roundToInt().coerceIn(0, bmp.width - 1)
                                val srcY = (currentFrame.y * scaleY).roundToInt().coerceIn(0, bmp.height - 1)
                                val srcW = (currentFrame.width * scaleX).roundToInt().coerceIn(1, bmp.width - srcX)
                                val srcH = (currentFrame.height * scaleY).roundToInt().coerceIn(1, bmp.height - srcY)
                                drawImage(
                                    image = bmp,
                                    srcOffset = IntOffset(srcX, srcY),
                                    srcSize = IntSize(srcW, srcH),
                                    dstOffset = IntOffset(dstX, dstY),
                                    dstSize = IntSize(dstW, dstH),
                                    filterQuality = FilterQuality.High
                                )
                            }
                        }
                    } else if (loaded != null) {
                        val bmp = loaded.bitmap
                        val srcW = bmp.width
                        val srcH = bmp.height
                        val fit = min((size.width * 0.85f) / srcW, (size.height * 0.85f) / srcH)
                        val dstW = (srcW * fit).roundToInt().coerceAtLeast(1)
                        val dstH = (srcH * fit).roundToInt().coerceAtLeast(1)
                        val dstX = ((size.width - dstW) / 2f + offX * fit).roundToInt()
                        val dstY = ((size.height - dstH) / 2f + offY * fit).roundToInt()
                        drawImage(
                            image = bmp,
                            srcOffset = IntOffset(0, 0),
                            srcSize = IntSize(srcW, srcH),
                            dstOffset = IntOffset(dstX, dstY),
                            dstSize = IntSize(dstW, dstH),
                            filterQuality = FilterQuality.High
                        )
                    }
                }

                if (entity != null) {
                    Surface(
                        shape = RoundedCornerShape(8.dp),
                        color = StudioColors.BgCard,
                        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
                    ) {
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 8.dp, vertical = 6.dp),
                            verticalArrangement = Arrangement.spacedBy(4.dp)
                        ) {
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    text = "Offset Pose (${activeAnimKind}): X=${offX.roundToInt()}  Y=${offY.roundToInt()}",
                                    color = StudioColors.TextPrimary,
                                    fontWeight = FontWeight.SemiBold,
                                    fontSize = 11.sp
                                )
                                OutlinedButton(
                                    onClick = { onSetAnimOffset(entity.id, activeAnimKind, 0f, 0f) },
                                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 0.dp),
                                    modifier = Modifier.height(24.dp)
                                ) {
                                    Text("Centrar 0", fontSize = 10.sp)
                                }
                            }
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .horizontalScroll(rememberScrollState()),
                                horizontalArrangement = Arrangement.spacedBy(4.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text("X:", color = StudioColors.AccentCyan, fontWeight = FontWeight.Bold, fontSize = 10.5.sp)
                                listOf(-5f, -1f, 1f, 5f).forEach { step ->
                                    OutlinedButton(
                                        onClick = { onSetAnimOffset(entity.id, activeAnimKind, offX + step, offY) },
                                        contentPadding = PaddingValues(horizontal = 6.dp, vertical = 0.dp),
                                        modifier = Modifier.height(24.dp)
                                    ) {
                                        Text(if (step > 0f) "+${step.toInt()}" else "${step.toInt()}", fontSize = 10.sp)
                                    }
                                }
                                Spacer(Modifier.width(6.dp))
                                Text("Y:", color = StudioColors.AccentAmber, fontWeight = FontWeight.Bold, fontSize = 10.5.sp)
                                listOf(-5f, -1f, 1f, 5f).forEach { step ->
                                    OutlinedButton(
                                        onClick = { onSetAnimOffset(entity.id, activeAnimKind, offX, offY + step) },
                                        contentPadding = PaddingValues(horizontal = 6.dp, vertical = 0.dp),
                                        modifier = Modifier.height(24.dp)
                                    ) {
                                        Text(if (step > 0f) "+${step.toInt()}" else "${step.toInt()}", fontSize = 10.sp)
                                    }
                                }
                            }
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
