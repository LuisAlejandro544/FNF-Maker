// Archivo: android/app/src/main/kotlin/com/beatforge/studio/gfx/GlesTextureCache.kt
// Propósito: Caché LRU y gestor de carga/liberación de texturas OpenGL ES en `GLThread`
// para `ImageBitmap` (atlas, íconos, ratings, `RGB_565` y multipágina) y textos dinámicos
// generados por scripts (`makeText`) sin fugas de memoria VRAM en 32 y 64 bits.

package com.beatforge.studio.gfx

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Paint
import android.graphics.Typeface
import android.opengl.GLES20
import android.opengl.GLUtils
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asAndroidBitmap
import java.lang.ref.WeakReference
import kotlin.math.ceil
import kotlin.math.roundToInt

internal class CachedBitmapTexture(
    val texId: Int,
    val width: Int,
    val height: Int,
    val generationId: Int,
    val bitmapRef: WeakReference<Bitmap>,
    var lastFrameUsed: Long
)

internal class CachedTextTexture(
    val texId: Int,
    val width: Int,
    val height: Int,
    val baselineYOffset: Float
)

private data class TextTextureKey(
    val text: String,
    val fontSizeBucket: Int,
    val colorRgb: Int
)

internal class GlesTextureCache(
    private val maxTextEntries: Int = 48
) {
    private val bitmapTextures = HashMap<Int, CachedBitmapTexture>(64)
    private val textTextures = object : LinkedHashMap<TextTextureKey, CachedTextTexture>(32, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<TextTextureKey, CachedTextTexture>?): Boolean {
            if (size <= maxTextEntries) return false
            val removed = eldest?.value ?: return false
            if (removed.texId != 0) {
                deleteTexture(removed.texId)
            }
            return true
        }
    }

    private val scratchPaint = Paint(Paint.ANTI_ALIAS_FLAG).apply {
        typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
        setShadowLayer(4f, 2f, 2f, android.graphics.Color.BLACK)
    }

    private var frameCounter: Long = 0L

    fun beginFrame() {
        frameCounter += 1L
        if ((frameCounter % 60L) == 0L) {
            pruneRecycledBitmaps()
        }
    }

    fun getOrUploadBitmap(imageBitmap: ImageBitmap?): CachedBitmapTexture? {
        if (imageBitmap == null) return null
        val androidBmp = try {
            imageBitmap.asAndroidBitmap()
        } catch (_: Throwable) {
            return null
        }
        if (androidBmp.isRecycled || androidBmp.width <= 0 || androidBmp.height <= 0) {
            return null
        }

        val identityKey = System.identityHashCode(androidBmp)
        val genId = androidBmp.generationId
        val existing = bitmapTextures[identityKey]
        if (existing != null) {
            val tracked = existing.bitmapRef.get()
            if (tracked === androidBmp &&
                !tracked.isRecycled &&
                existing.generationId == genId &&
                existing.width == androidBmp.width &&
                existing.height == androidBmp.height
            ) {
                existing.lastFrameUsed = frameCounter
                return existing
            }
            deleteTexture(existing.texId)
            bitmapTextures.remove(identityKey)
        }

        val texId = uploadBitmapToGl(androidBmp)
        if (texId == 0) return null
        val entry = CachedBitmapTexture(
            texId = texId,
            width = androidBmp.width,
            height = androidBmp.height,
            generationId = genId,
            bitmapRef = WeakReference(androidBmp),
            lastFrameUsed = frameCounter
        )
        bitmapTextures[identityKey] = entry
        return entry
    }

    fun getOrCreateTextTexture(
        text: String,
        fontSizePx: Float,
        colorRgb: Int
    ): CachedTextTexture? {
        if (text.isBlank()) return null
        val bucketSize = fontSizePx.roundToInt().coerceIn(10, 128)
        val cleanRgb = colorRgb and 0x00FFFFFF
        val key = TextTextureKey(text = text, fontSizeBucket = bucketSize, colorRgb = cleanRgb)
        textTextures[key]?.let { return it }

        scratchPaint.textSize = bucketSize.toFloat()
        scratchPaint.color = (0xFF shl 24) or cleanRgb
        val fm = scratchPaint.fontMetrics
        val textW = ceil(scratchPaint.measureText(text) + 10f).toInt().coerceIn(8, 2048)
        val textH = ceil((fm.bottom - fm.top) + 10f).toInt().coerceIn(8, 512)
        val baselineY = -fm.top + 3f

        val bmp = Bitmap.createBitmap(textW, textH, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bmp)
        canvas.drawText(text, 4f, baselineY, scratchPaint)
        val texId = uploadBitmapToGl(bmp)
        bmp.recycle()
        if (texId == 0) return null

        val entry = CachedTextTexture(texId = texId, width = textW, height = textH, baselineYOffset = baselineY)
        textTextures[key] = entry
        return entry
    }

    private fun pruneRecycledBitmaps() {
        val iter = bitmapTextures.entries.iterator()
        while (iter.hasNext()) {
            val entry = iter.next().value
            val bmp = entry.bitmapRef.get()
            if (bmp == null || bmp.isRecycled) {
                deleteTexture(entry.texId)
                iter.remove()
            }
        }
    }

    fun releaseAll() {
        val ids = ArrayList<Int>(bitmapTextures.size + textTextures.size)
        for (entry in bitmapTextures.values) {
            if (entry.texId != 0) ids.add(entry.texId)
        }
        for (entry in textTextures.values) {
            if (entry.texId != 0) ids.add(entry.texId)
        }
        bitmapTextures.clear()
        textTextures.clear()
        if (ids.isNotEmpty()) {
            val arr = ids.toIntArray()
            GLES20.glDeleteTextures(arr.size, arr, 0)
        }
    }

    private fun uploadBitmapToGl(bmp: Bitmap): Int {
        val ids = IntArray(1)
        GLES20.glGenTextures(1, ids, 0)
        val texId = ids[0]
        if (texId == 0) return 0
        GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, texId)
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR)
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR)
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_S, GLES20.GL_CLAMP_TO_EDGE)
        GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_T, GLES20.GL_CLAMP_TO_EDGE)
        GLUtils.texImage2D(GLES20.GL_TEXTURE_2D, 0, bmp, 0)
        return texId
    }

    private fun deleteTexture(texId: Int) {
        if (texId != 0) {
            val arr = intArrayOf(texId)
            GLES20.glDeleteTextures(1, arr, 0)
        }
    }
}
