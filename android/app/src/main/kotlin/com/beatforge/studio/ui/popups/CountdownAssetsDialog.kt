// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/CountdownAssetsDialog.kt
// Propósito: Ventana emergente modal para configurar de forma independiente los 3 pasos de
// la cuenta regresiva estilo FNF ("2 / Ready?", "1 / Set?", "¡YA! / Go"): cada paso permite
// importar un PNG/.astc/.webp separado, su atlas Sparrow `.xml` opcional (o usar uno combinado),
// y su efecto de voz separado (.ogg/.wav/.mp3) con botón de prueba en vivo y limpieza por slot.

package com.beatforge.studio.ui.popups

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AudioFile
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Code
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Image
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.RestartAlt
import androidx.compose.material.icons.rounded.Timer
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
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.drawscope.withTransform
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.beatforge.studio.CountdownStepSnapshot
import com.beatforge.studio.ImportKind
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.SparrowFrameInfo
import com.beatforge.studio.ui.theme.StudioColors
import java.util.Locale
import kotlin.math.min
import kotlin.math.roundToInt

private data class CountdownSlotMeta(
    val slotIndex: Int,
    val title: String,
    val badgeText: String,
    val typicalFiles: String,
    val accent: Color
)

private val COUNTDOWN_SLOTS_META = listOf(
    CountdownSlotMeta(
        slotIndex = 0,
        title = "Paso 1 · 2 (Ready?)",
        badgeText = "2",
        typicalFiles = "Sprite: ready.png (+ .xml opcional) · Voz: intro2.ogg",
        accent = StudioColors.AccentAmber
    ),
    CountdownSlotMeta(
        slotIndex = 1,
        title = "Paso 2 · 1 (Set?)",
        badgeText = "1",
        typicalFiles = "Sprite: set.png (+ .xml opcional) · Voz: intro1.ogg",
        accent = StudioColors.AccentCyan
    ),
    CountdownSlotMeta(
        slotIndex = 2,
        title = "Paso 3 · ¡YA! (Go)",
        badgeText = "¡YA!",
        typicalFiles = "Sprite: go.png (+ .xml opcional) · Voz: introGo.ogg",
        accent = StudioColors.AccentEmerald
    )
)

@Composable
fun CountdownAssetsDialog(
    slots: List<CountdownStepSnapshot>,
    countdownBitmaps: Map<Int, LoadedEntityBitmap>,
    onRequestImport: (ImportKind) -> Unit,
    onClearSprite: (slot: Int) -> Unit,
    onClearAudio: (slot: Int) -> Unit,
    onPreviewAudio: (slot: Int) -> Unit,
    onRestoreDefaults: () -> Unit,
    onDismiss: () -> Unit
) {
    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false)
    ) {
        Surface(
            modifier = Modifier
                .fillMaxWidth(0.88f)
                .heightIn(max = 560.dp)
                .padding(14.dp),
            shape = RoundedCornerShape(14.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Column(
                modifier = Modifier.padding(14.dp),
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Timer,
                        contentDescription = null,
                        tint = StudioColors.AccentAmber,
                        modifier = Modifier.size(22.dp)
                    )
                    Spacer(Modifier.width(8.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = "Sprites y Voces de Cuenta Regresiva (2 · 1 · ¡YA!)",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 15.sp
                        )
                        Text(
                            text = "Carga imágenes PNG/ASTC + .xml opcional y audios separados (.ogg, .wav, .mp3) por cada paso",
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp
                        )
                    }
                    IconButton(onClick = onDismiss) {
                        Icon(Icons.Rounded.Close, "Cerrar", tint = StudioColors.TextMuted)
                    }
                }

                Column(
                    modifier = Modifier
                        .weight(1f)
                        .verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    COUNTDOWN_SLOTS_META.forEach { meta ->
                        val step = slots.firstOrNull { it.slotIndex == meta.slotIndex }
                        val loadedBmp = countdownBitmaps[meta.slotIndex]
                        CountdownSlotCard(
                            meta = meta,
                            step = step,
                            loadedBitmap = loadedBmp,
                            onImportImage = { onRequestImport(ImportKind.CountdownImage(meta.slotIndex)) },
                            onImportXml = { onRequestImport(ImportKind.CountdownXml(meta.slotIndex)) },
                            onClearSprite = { onClearSprite(meta.slotIndex) },
                            onImportAudio = { onRequestImport(ImportKind.CountdownAudio(meta.slotIndex)) },
                            onPreviewAudio = { onPreviewAudio(meta.slotIndex) },
                            onClearAudio = { onClearAudio(meta.slotIndex) }
                        )
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    OutlinedButton(
                        onClick = onRestoreDefaults,
                        contentPadding = PaddingValues(horizontal = 12.dp, vertical = 6.dp)
                    ) {
                        Icon(
                            imageVector = Icons.Rounded.RestartAlt,
                            contentDescription = null,
                            tint = StudioColors.AccentAmber,
                            modifier = Modifier.size(16.dp)
                        )
                        Spacer(Modifier.width(6.dp))
                        Text(
                            text = "Restaurar 2-1-¡YA! por defecto",
                            color = StudioColors.AccentAmber,
                            fontSize = 11.5.sp,
                            fontWeight = FontWeight.SemiBold,
                            maxLines = 1,
                            softWrap = false
                        )
                    }

                    Button(
                        onClick = onDismiss,
                        colors = ButtonDefaults.buttonColors(
                            containerColor = StudioColors.AccentCyan,
                            contentColor = Color(0xFF041C24)
                        ),
                        contentPadding = PaddingValues(horizontal = 16.dp, vertical = 6.dp)
                    ) {
                        Text("Listo", fontSize = 12.sp, fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}

@Composable
private fun CountdownSlotCard(
    meta: CountdownSlotMeta,
    step: CountdownStepSnapshot?,
    loadedBitmap: LoadedEntityBitmap?,
    onImportImage: () -> Unit,
    onImportXml: () -> Unit,
    onClearSprite: () -> Unit,
    onImportAudio: () -> Unit,
    onPreviewAudio: () -> Unit,
    onClearAudio: () -> Unit
) {
    val hasSprite = step?.imageName != null || loadedBitmap != null
    val hasXml = step?.xmlName != null
    val hasAudio = step?.audioName != null
    val previewFrame: SparrowFrameInfo? = step?.frames?.firstOrNull { it.found && it.width > 0 && it.height > 0 }

    Surface(
        shape = RoundedCornerShape(10.dp),
        color = StudioColors.BgCard,
        border = BorderStroke(
            1.dp,
            if (hasSprite || hasAudio) meta.accent.copy(alpha = 0.55f) else StudioColors.BorderSubtle
        )
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            // Miniatura visual del sprite o insignia vectorial por defecto
            Box(
                modifier = Modifier
                    .size(64.dp)
                    .background(StudioColors.BgDark, RoundedCornerShape(8.dp)),
                contentAlignment = Alignment.Center
            ) {
                if (loadedBitmap != null) {
                    Canvas(modifier = Modifier.size(56.dp)) {
                        val bmp = loadedBitmap.bitmap
                        if (previewFrame != null) {
                            val scaleX = bmp.width.toFloat() / loadedBitmap.origWidth.coerceAtLeast(1).toFloat()
                            val scaleY = bmp.height.toFloat() / loadedBitmap.origHeight.coerceAtLeast(1).toFloat()
                            val srcX = (previewFrame.x * scaleX).roundToInt().coerceIn(0, bmp.width - 1)
                            val srcY = (previewFrame.y * scaleY).roundToInt().coerceIn(0, bmp.height - 1)
                            val srcW = (previewFrame.width * scaleX).roundToInt().coerceIn(1, bmp.width - srcX)
                            val srcH = (previewFrame.height * scaleY).roundToInt().coerceIn(1, bmp.height - srcY)
                            val visW = if (previewFrame.rotated) previewFrame.height else previewFrame.width
                            val visH = if (previewFrame.rotated) previewFrame.width else previewFrame.height
                            val fit = min(size.width / visW.coerceAtLeast(1), size.height / visH.coerceAtLeast(1))
                            val dstW = (previewFrame.width * fit).roundToInt().coerceAtLeast(1)
                            val dstH = (previewFrame.height * fit).roundToInt().coerceAtLeast(1)
                            val cx = size.width * 0.5f
                            val cy = size.height * 0.5f
                            val dstLeft = (cx - dstW * 0.5f).roundToInt()
                            val dstTop = (cy - dstH * 0.5f).roundToInt()
                            withTransform({
                                if (previewFrame.rotated) {
                                    rotate(-90f, pivot = Offset(cx, cy))
                                }
                            }) {
                                drawImage(
                                    image = bmp,
                                    srcOffset = IntOffset(srcX, srcY),
                                    srcSize = IntSize(srcW, srcH),
                                    dstOffset = IntOffset(dstLeft, dstTop),
                                    dstSize = IntSize(dstW, dstH),
                                    filterQuality = FilterQuality.Medium
                                )
                            }
                        } else {
                            val fit = min(size.width / bmp.width.coerceAtLeast(1), size.height / bmp.height.coerceAtLeast(1))
                            val dstW = (bmp.width * fit).roundToInt().coerceAtLeast(1)
                            val dstH = (bmp.height * fit).roundToInt().coerceAtLeast(1)
                            val dstLeft = ((size.width - dstW) * 0.5f).roundToInt()
                            val dstTop = ((size.height - dstH) * 0.5f).roundToInt()
                            drawImage(
                                image = bmp,
                                srcOffset = IntOffset(0, 0),
                                srcSize = IntSize(bmp.width, bmp.height),
                                dstOffset = IntOffset(dstLeft, dstTop),
                                dstSize = IntSize(dstW, dstH),
                                filterQuality = FilterQuality.Medium
                            )
                        }
                    }
                } else {
                    Text(
                        text = meta.badgeText,
                        color = meta.accent,
                        fontWeight = FontWeight.ExtraBold,
                        fontSize = if (meta.slotIndex == 2) 17.sp else 24.sp
                    )
                }
            }

            Column(
                modifier = Modifier.weight(1f),
                verticalArrangement = Arrangement.spacedBy(4.dp)
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Text(
                        text = meta.title,
                        color = meta.accent,
                        fontWeight = FontWeight.Bold,
                        fontSize = 13.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f)
                    )
                    Text(
                        text = meta.typicalFiles,
                        color = StudioColors.TextMuted,
                        fontSize = 9.5.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }

                // Fila Sprite PNG + XML opcional
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    val spriteSummary = buildString {
                        append(step?.imageName ?: "Sprite vectorial por defecto")
                        val xmlName = step?.xmlName
                        if (step != null && xmlName != null) {
                            append(" · $xmlName (${step.frames.size}f)")
                        }
                    }
                    Text(
                        text = spriteSummary,
                        color = if (hasSprite) StudioColors.TextPrimary else StudioColors.TextMuted,
                        fontSize = 11.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f)
                    )
                    OutlinedButton(
                        onClick = onImportImage,
                        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp),
                        modifier = Modifier.height(28.dp)
                    ) {
                        Icon(Icons.Rounded.Image, null, tint = meta.accent, modifier = Modifier.size(13.dp))
                        Spacer(Modifier.width(4.dp))
                        Text("Imagen", color = StudioColors.TextPrimary, fontSize = 10.5.sp, maxLines = 1, softWrap = false)
                    }
                    OutlinedButton(
                        onClick = onImportXml,
                        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp),
                        modifier = Modifier.height(28.dp)
                    ) {
                        Icon(Icons.Rounded.Code, null, tint = StudioColors.AccentIndigo, modifier = Modifier.size(13.dp))
                        Spacer(Modifier.width(4.dp))
                        Text(
                            text = if (hasXml) ".XML ✓" else "+ .XML",
                            color = if (hasXml) StudioColors.AccentEmerald else StudioColors.TextPrimary,
                            fontSize = 10.5.sp,
                            maxLines = 1,
                            softWrap = false
                        )
                    }
                    if (hasSprite || hasXml) {
                        IconButton(
                            onClick = onClearSprite,
                            modifier = Modifier.size(28.dp)
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.DeleteOutline,
                                contentDescription = "Quitar sprite",
                                tint = StudioColors.AccentCoral,
                                modifier = Modifier.size(15.dp)
                            )
                        }
                    }
                }

                // Fila Audio/Voz separado
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    val audioName = step?.audioName
                    val audioSummary = if (step != null && audioName != null) {
                        val sec = step.audioDurationMs / 1000.0
                        if (sec > 0.0) {
                            String.format(Locale.US, "%s (%.2fs)", audioName, sec)
                        } else {
                            audioName
                        }
                    } else {
                        "Voz/SFX: Tono sintetizado por defecto"
                    }
                    Text(
                        text = audioSummary,
                        color = if (hasAudio) StudioColors.AccentEmerald else StudioColors.TextMuted,
                        fontSize = 11.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f)
                    )
                    OutlinedButton(
                        onClick = onImportAudio,
                        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp),
                        modifier = Modifier.height(28.dp)
                    ) {
                        Icon(Icons.Rounded.AudioFile, null, tint = StudioColors.AccentEmerald, modifier = Modifier.size(13.dp))
                        Spacer(Modifier.width(4.dp))
                        Text("Voz/SFX", color = StudioColors.TextPrimary, fontSize = 10.5.sp, maxLines = 1, softWrap = false)
                    }
                    OutlinedButton(
                        onClick = onPreviewAudio,
                        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp),
                        modifier = Modifier.height(28.dp)
                    ) {
                        Icon(Icons.Rounded.PlayArrow, null, tint = StudioColors.AccentCyan, modifier = Modifier.size(14.dp))
                        Spacer(Modifier.width(2.dp))
                        Text("Probar", color = StudioColors.AccentCyan, fontSize = 10.5.sp, maxLines = 1, softWrap = false)
                    }
                    if (hasAudio) {
                        IconButton(
                            onClick = onClearAudio,
                            modifier = Modifier.size(28.dp)
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.DeleteOutline,
                                contentDescription = "Quitar audio",
                                tint = StudioColors.AccentCoral,
                                modifier = Modifier.size(15.dp)
                            )
                        }
                    }
                }
            }
        }
    }
}
