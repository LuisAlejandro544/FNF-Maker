// Archivo: android/app/src/main/kotlin/com/beatforge/studio/viewmodel/StudioBitmapLoader.kt
// Propósito: Cargador y caché de bitmaps/assets gráficos en `Dispatchers.IO` para
// `StudioViewModel`: detecta gama de hardware 32/64-bit, decodifica regiones 1:1 de Sparrow
// con `BitmapRegionDecoder` + `LruCache` (20 MB), recorta tiras de íconos 2-frame, soporta
// `Bitmap.Config.RGB_565` para fondos opacos con dithering y carga mega-atlas multipágina.

package com.beatforge.studio.viewmodel

import android.app.ActivityManager
import android.app.Application
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.BitmapRegionDecoder
import android.graphics.Rect
import android.os.Build
import android.util.LruCache
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import com.beatforge.studio.DeviceTier
import com.beatforge.studio.LoadedEntityBitmap
import com.beatforge.studio.NativeEngineBridge
import com.beatforge.studio.SparrowFrameInfo
import com.beatforge.studio.StageEntitySnapshot
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.io.File

internal data class HudSvgAssets(
    val healthBar: ImageBitmap?,
    val ratings: Map<Int, ImageBitmap>,
    val noteSplash: ImageBitmap?,
    val playerNormal: ImageBitmap?,
    val playerLosing: ImageBitmap?,
    val opponentNormal: ImageBitmap?,
    val opponentLosing: ImageBitmap?
)

internal class StudioBitmapLoader {

    // Caché LRU en KB para cuadros individuales decodificados 1:1 en SpriteLab (evita vista borrosa sin OOM en 32-bit)
    private val regionFrameCache = object : LruCache<String, ImageBitmap>(20 * 1024) {
        override fun sizeOf(key: String, value: ImageBitmap): Int {
            return ((value.width.toLong() * value.height.toLong() * 4L) / 1024L).toInt().coerceAtLeast(1)
        }
    }
    private val regionDecoderLock = Any()

    fun evictRegionCache() {
        synchronized(regionDecoderLock) { regionFrameCache.evictAll() }
    }

    /**
     * Detecta automáticamente la capacidad del teléfono (arquitectura 32/64 bits y RAM total)
     * para elegir el límite óptimo de texturas durante la compilación en caliente.
     */
    fun detectOptimalDeviceTier(app: Application): DeviceTier {
        return try {
            val am = app.getSystemService(Context.ACTIVITY_SERVICE) as? ActivityManager
            val memInfo = ActivityManager.MemoryInfo()
            am?.getMemoryInfo(memInfo)
            val totalGb = memInfo.totalMem / (1024.0 * 1024.0 * 1024.0)
            val is32BitOnly = Build.SUPPORTED_64_BIT_ABIS.isEmpty()

            when {
                is32BitOnly || memInfo.lowMemory || totalGb <= 3.5 -> DeviceTier.LowRam32Bit
                totalGb >= 7.5 -> DeviceTier.HighQuality
                else -> DeviceTier.Balanced
            }
        } catch (_: Throwable) {
            DeviceTier.Balanced
        }
    }

    /**
     * Decodifica únicamente la región exacta 1:1 del frame activo en el sprite sheet usando
     * `BitmapRegionDecoder` (LRU caché en `Dispatchers.IO`).
     */
    suspend fun decodeSparrowRegionBitmap(
        imagePath: String?,
        frame: SparrowFrameInfo,
        tier: DeviceTier
    ): ImageBitmap? {
        if (imagePath.isNullOrBlank() || !frame.found || frame.width <= 0 || frame.height <= 0) {
            return null
        }
        val cacheKey = "$imagePath|${frame.x},${frame.y},${frame.width},${frame.height}"
        regionFrameCache.get(cacheKey)?.let { return it }

        return withContext(Dispatchers.IO) {
            regionFrameCache.get(cacheKey)?.let { return@withContext it }
            val file = File(imagePath)
            if (!file.exists()) return@withContext null

            val maxFrameSide = if (tier == DeviceTier.LowRam32Bit) 900 else 1280
            var sampleSize = 1
            val longest = maxOf(frame.width, frame.height)
            while (longest / sampleSize > maxFrameSide) {
                sampleSize *= 2
            }

            val rect = Rect(
                frame.x.coerceAtLeast(0),
                frame.y.coerceAtLeast(0),
                (frame.x + frame.width).coerceAtLeast(frame.x + 1),
                (frame.y + frame.height).coerceAtLeast(frame.y + 1)
            )
            val opts = BitmapFactory.Options().apply {
                inSampleSize = sampleSize
                inPreferredConfig = Bitmap.Config.ARGB_8888
                inScaled = false
            }

            val decoded: Bitmap? = synchronized(regionDecoderLock) {
                try {
                    val decoder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                        BitmapRegionDecoder.newInstance(imagePath)
                    } else {
                        @Suppress("DEPRECATION")
                        BitmapRegionDecoder.newInstance(imagePath, false)
                    }
                    try {
                        decoder?.decodeRegion(rect, opts)
                    } finally {
                        decoder?.recycle()
                    }
                } catch (_: Throwable) {
                    null
                }
            }

            val img = decoded?.asImageBitmap()
            if (img != null) {
                regionFrameCache.put(cacheKey, img)
            }
            img
        }
    }

    fun loadArrowBitmaps(): Map<Int, ImageBitmap> {
        val out = linkedMapOf<Int, ImageBitmap>()
        for (dir in 0..3) {
            decodePngBytes(NativeEngineBridge.nativeGetArrowPngBytes(dir))?.let {
                out[dir] = it
            }
        }
        return out
    }

    fun loadReceptorBitmaps(): Map<Int, ImageBitmap> {
        val out = linkedMapOf<Int, ImageBitmap>()
        for (dir in 0..3) {
            decodePngBytes(NativeEngineBridge.nativeGetReceptorPngBytes(dir))?.let {
                out[dir] = it
            }
        }
        return out
    }

    fun loadHurtArrowXBitmap(): ImageBitmap? {
        return decodePngBytes(NativeEngineBridge.nativeGetHurtArrowPngBytes())
    }

    fun loadHealthBarBitmap(): ImageBitmap? {
        return decodePngBytes(NativeEngineBridge.nativeGetHealthBarPngBytes())
    }

    fun loadHudSvgAssets(): HudSvgAssets {
        val healthBar = loadHealthBarBitmap()
        val ratings = linkedMapOf<Int, ImageBitmap>()
        for (code in 1..4) {
            decodePngBytes(NativeEngineBridge.nativeGetRatingPngBytes(code))?.let {
                ratings[code] = it
            }
        }
        val noteSplash = decodePngBytes(NativeEngineBridge.nativeGetNoteSplashPngBytes())
        val playerNormal = decodePngBytes(NativeEngineBridge.nativeGetDefaultIconPngBytes(isPlayer = true, isLosing = false))
        val playerLosing = decodePngBytes(NativeEngineBridge.nativeGetDefaultIconPngBytes(isPlayer = true, isLosing = true))
        val opponentNormal = decodePngBytes(NativeEngineBridge.nativeGetDefaultIconPngBytes(isPlayer = false, isLosing = false))
        val opponentLosing = decodePngBytes(NativeEngineBridge.nativeGetDefaultIconPngBytes(isPlayer = false, isLosing = true))
        return HudSvgAssets(
            healthBar = healthBar,
            ratings = ratings,
            noteSplash = noteSplash,
            playerNormal = playerNormal,
            playerLosing = playerLosing,
            opponentNormal = opponentNormal,
            opponentLosing = opponentLosing
        )
    }

    /**
     * Carga íconos personalizados de personaje. Si el usuario sube una tira horizontal clásica
     * de FNF (`width >= height * 1.85`), extrae automáticamente la mitad izquierda como Normal
     * y la mitad derecha como Perdiendo (<20% de vida).
     */
    fun loadEntityIcons(
        entities: List<StageEntitySnapshot>
    ): Pair<Map<String, ImageBitmap>, Map<String, ImageBitmap>> {
        val normalMap = linkedMapOf<String, ImageBitmap>()
        val losingMap = linkedMapOf<String, ImageBitmap>()

        for (ent in entities) {
            val normalPath = ent.iconNormalPath
            val losingPath = ent.iconLosingPath

            if (!normalPath.isNullOrBlank()) {
                decodeIconFileCapped(normalPath)?.let { bmp ->
                    if (bmp.width >= (bmp.height * 1.85f).toInt() && bmp.width >= 4) {
                        val halfW = bmp.width / 2
                        val leftBmp = Bitmap.createBitmap(bmp, 0, 0, halfW, bmp.height)
                        normalMap[ent.id] = leftBmp.asImageBitmap()
                        if (losingPath.isNullOrBlank()) {
                            val rightBmp = Bitmap.createBitmap(bmp, halfW, 0, bmp.width - halfW, bmp.height)
                            losingMap[ent.id] = rightBmp.asImageBitmap()
                        }
                    } else {
                        normalMap[ent.id] = bmp.asImageBitmap()
                    }
                }
            }

            if (!losingPath.isNullOrBlank()) {
                decodeIconFileCapped(losingPath)?.let { bmp ->
                    if (bmp.width >= (bmp.height * 1.85f).toInt() && bmp.width >= 4) {
                        val halfW = bmp.width / 2
                        val rightBmp = Bitmap.createBitmap(bmp, halfW, 0, bmp.width - halfW, bmp.height)
                        losingMap[ent.id] = rightBmp.asImageBitmap()
                    } else {
                        losingMap[ent.id] = bmp.asImageBitmap()
                    }
                }
            }
        }
        return normalMap to losingMap
    }

    fun loadEditorEntityBitmaps(
        entities: List<StageEntitySnapshot>,
        tier: DeviceTier
    ): Map<String, LoadedEntityBitmap> {
        val out = linkedMapOf<String, LoadedEntityBitmap>()
        for (ent in entities) {
            val path = ent.imagePath ?: continue
            val hasAtlas = ent.xmlPath != null ||
                ent.animateJsonPath != null ||
                ent.animationPrefixes.isNotEmpty() ||
                (ent.previewFrame?.found == true)
            val isBackground = ent.role == "background"
            val cap = editorPreviewMaxSide(tier, hasAtlas, isBackground)
            decodeEntityBitmapWithOrigDims(path, path, maxDecodeSide = cap)?.let { loaded ->
                out[ent.id] = loaded
            }
        }
        return out
    }

    fun loadCompiledEntityBitmaps(
        compiledPaths: Map<String, String>,
        entities: List<StageEntitySnapshot>,
        tier: DeviceTier,
        onlyInitiallyVisible: Boolean = true,
        repackedEntityIds: Set<String> = emptySet(),
        compiledPages: Map<String, List<String>> = emptyMap(),
        texturePixelFormats: Map<String, String> = emptyMap()
    ): Map<String, LoadedEntityBitmap> {
        val out = linkedMapOf<String, LoadedEntityBitmap>()
        val origPaths = entities.associate { it.id to it.imagePath }
        for (ent in entities) {
            if (onlyInitiallyVisible && (!ent.visible || ent.opacity <= 0.005f)) {
                continue
            }
            decodeCompiledPagesForEntity(
                entityId = ent.id,
                ent = ent,
                origPath = origPaths[ent.id],
                compiledPaths = compiledPaths,
                compiledPages = compiledPages,
                texturePixelFormats = texturePixelFormats,
                tier = tier,
                repackedEntityIds = repackedEntityIds
            )?.let { loaded ->
                out[ent.id] = loaded
            }
        }
        return out
    }

    fun decodeSingleCompiledEntityBitmap(
        entityId: String,
        compiledPaths: Map<String, String>,
        entities: List<StageEntitySnapshot>,
        tier: DeviceTier,
        repackedEntityIds: Set<String> = emptySet(),
        compiledPages: Map<String, List<String>> = emptyMap(),
        texturePixelFormats: Map<String, String> = emptyMap()
    ): LoadedEntityBitmap? {
        val ent = entities.firstOrNull { it.id == entityId } ?: return null
        return decodeCompiledPagesForEntity(
            entityId = entityId,
            ent = ent,
            origPath = ent.imagePath,
            compiledPaths = compiledPaths,
            compiledPages = compiledPages,
            texturePixelFormats = texturePixelFormats,
            tier = tier,
            repackedEntityIds = repackedEntityIds
        )
    }

    private fun decodeCompiledPagesForEntity(
        entityId: String,
        ent: StageEntitySnapshot,
        origPath: String?,
        compiledPaths: Map<String, String>,
        compiledPages: Map<String, List<String>>,
        texturePixelFormats: Map<String, String>,
        tier: DeviceTier,
        repackedEntityIds: Set<String>
    ): LoadedEntityBitmap? {
        val pages = compiledPages[entityId]?.takeIf { it.isNotEmpty() }
        val cPath = pages?.firstOrNull() ?: compiledPaths[entityId] ?: origPath ?: return null
        // Si el atlas fue reempaquetado en caliente, las coordenadas de sus frames ya pertenecen al WebP compilado.
        val oPath = if (entityId in repackedEntityIds) cPath else (origPath ?: cPath)
        val cap = compiledGameplayMaxSide(tier, ent)
        val fmtKey = texturePixelFormats[entityId] ?: "argb8888"

        val baseLoaded = decodeEntityBitmapWithOrigDims(
            loadPath = cPath,
            origPath = oPath,
            maxDecodeSide = cap,
            pixelFormatKey = fmtKey
        ) ?: return null

        if (pages == null || pages.size <= 1) {
            return baseLoaded
        }

        val extras = ArrayList<LoadedEntityBitmap>(pages.size - 1)
        for (pIdx in 1 until pages.size) {
            val extraPath = pages[pIdx]
            val extraLoaded = decodeEntityBitmapWithOrigDims(
                loadPath = extraPath,
                origPath = extraPath,
                maxDecodeSide = cap,
                pixelFormatKey = fmtKey
            ) ?: break
            extras.add(extraLoaded)
        }
        return if (extras.isEmpty()) baseLoaded else baseLoaded.copy(extraPages = extras)
    }

    private fun decodePngBytes(bytes: ByteArray): ImageBitmap? {
        if (bytes.isEmpty()) return null
        return BitmapFactory.decodeByteArray(bytes, 0, bytes.size)?.asImageBitmap()
    }

    private fun editorPreviewMaxSide(
        tier: DeviceTier,
        hasAtlas: Boolean,
        isBackground: Boolean
    ): Int {
        return when (tier) {
            DeviceTier.LowRam32Bit -> when {
                hasAtlas -> 1792
                isBackground -> 1600
                else -> 1024
            }
            DeviceTier.Balanced -> when {
                hasAtlas -> 2560
                isBackground -> 2048
                else -> 1536
            }
            DeviceTier.HighQuality -> when {
                hasAtlas -> 3072
                isBackground -> 2560
                else -> 2048
            }
        }
    }

    private fun compiledGameplayMaxSide(tier: DeviceTier, ent: StageEntitySnapshot): Int {
        val hasAtlas = ent.xmlPath != null ||
            ent.animateJsonPath != null ||
            ent.animationPrefixes.isNotEmpty() ||
            (ent.previewFrame?.found == true)
        val isBackground = ent.role == "background"
        return when (tier) {
            DeviceTier.LowRam32Bit -> when {
                hasAtlas -> 2560
                isBackground -> 2048
                else -> 1536
            }
            DeviceTier.Balanced -> if (hasAtlas) 4096 else 2560
            DeviceTier.HighQuality -> 4096
        }
    }

    private fun decodeIconFileCapped(path: String): Bitmap? {
        val file = File(path)
        if (!file.exists()) return null
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(path, bounds)
        var sample = 1
        val maxSide = maxOf(bounds.outWidth, bounds.outHeight)
        while (maxSide / sample > 512) {
            sample *= 2
        }
        val opts = BitmapFactory.Options().apply {
            inSampleSize = sample
            inPreferredConfig = Bitmap.Config.ARGB_8888
            inScaled = false
        }
        return BitmapFactory.decodeFile(path, opts)
    }

    private fun decodeEntityBitmapWithOrigDims(
        loadPath: String,
        origPath: String,
        maxDecodeSide: Int,
        pixelFormatKey: String = "argb8888"
    ): LoadedEntityBitmap? {
        val file = File(loadPath)
        if (!file.exists()) return null

        val origBounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(origPath, origBounds)
        val origW = origBounds.outWidth.coerceAtLeast(1)
        val origH = origBounds.outHeight.coerceAtLeast(1)

        val loadBounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeFile(loadPath, loadBounds)
        var sampleSize = 1
        val maxSide = maxOf(loadBounds.outWidth, loadBounds.outHeight)
        while (maxSide / sampleSize > maxDecodeSide) {
            sampleSize *= 2
        }

        val preferredConfig = if (pixelFormatKey == "rgb565") {
            Bitmap.Config.RGB_565
        } else {
            Bitmap.Config.ARGB_8888
        }
        val decodeOpts = BitmapFactory.Options().apply {
            inSampleSize = sampleSize
            inPreferredConfig = preferredConfig
            inScaled = false
        }
        val bmp = BitmapFactory.decodeFile(loadPath, decodeOpts) ?: return null
        return LoadedEntityBitmap(
            bitmap = bmp.asImageBitmap(),
            origWidth = origW,
            origHeight = origH
        )
    }
}
