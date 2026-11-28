// Archivo: android/app/src/main/kotlin/com/beatforge/studio/gfx/GlesSurfaceHost.kt
// Propósito: Host modular basado en `GLSurfaceView` para inicializar un contexto GPU
// OpenGL ES 3.0 por defecto con fallback automático y seguro a OpenGL ES 2.0 sobre
// Compose (`AndroidView`). Garantiza compatibilidad 32/64 bits en Android 8.0+.

package com.beatforge.studio.gfx

import android.opengl.GLES20
import android.opengl.GLSurfaceView
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import javax.microedition.khronos.egl.EGL10
import javax.microedition.khronos.egl.EGLConfig
import javax.microedition.khronos.egl.EGLContext
import javax.microedition.khronos.egl.EGLDisplay
import javax.microedition.khronos.opengles.GL10

private const val EGL_CONTEXT_CLIENT_VERSION = 0x3098
private const val EGL_OPENGL_ES2_BIT = 4

/**
 * Selector de configuración EGL RGBA8888 compatible tanto con OpenGL ES 3.0 como con 2.0.
 */
class StudioGlesConfigChooser : GLSurfaceView.EGLConfigChooser {
    override fun chooseConfig(egl: EGL10, display: EGLDisplay): EGLConfig {
        val numConfigs = IntArray(1)
        val primarySpec = intArrayOf(
            EGL10.EGL_RED_SIZE, 8,
            EGL10.EGL_GREEN_SIZE, 8,
            EGL10.EGL_BLUE_SIZE, 8,
            EGL10.EGL_ALPHA_SIZE, 8,
            EGL10.EGL_DEPTH_SIZE, 0,
            EGL10.EGL_STENCIL_SIZE, 0,
            EGL10.EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT,
            EGL10.EGL_NONE
        )

        val configs = arrayOfNulls<EGLConfig>(16)
        if (egl.eglChooseConfig(display, primarySpec, configs, configs.size, numConfigs) &&
            numConfigs[0] > 0
        ) {
            configs[0]?.let { return it }
        }

        // Respaldo amplio en dispositivos o emuladores con soporte RGB565 / sin canal alfa EGL
        val fallbackSpec = intArrayOf(
            EGL10.EGL_RENDERABLE_TYPE, EGL_OPENGL_ES2_BIT,
            EGL10.EGL_NONE
        )
        if (egl.eglChooseConfig(display, fallbackSpec, configs, configs.size, numConfigs) &&
            numConfigs[0] > 0
        ) {
            configs[0]?.let { return it }
        }

        throw IllegalStateException("No se encontró ninguna configuración EGL compatible con OpenGL ES.")
    }
}

/**
 * Factoría de contexto EGL que intenta crear OpenGL ES 3.0 primero cuando `preferEs3 = true`
 * y realiza fallback automático a OpenGL ES 2.0 si el controlador rechaza la versión 3.0.
 */
class StudioGlesContextFactory(
    private val preferEs3: Boolean = true,
    private val onContextResolved: ((actualMajor: Int, fellBackToEs2: Boolean) -> Unit)? = null
) : GLSurfaceView.EGLContextFactory {

    override fun createContext(
        egl: EGL10,
        display: EGLDisplay,
        eglConfig: EGLConfig
    ): EGLContext {
        if (preferEs3) {
            val es3Attribs = intArrayOf(EGL_CONTEXT_CLIENT_VERSION, 3, EGL10.EGL_NONE)
            val es3Ctx = egl.eglCreateContext(display, eglConfig, EGL10.EGL_NO_CONTEXT, es3Attribs)
            val err = egl.eglGetError()
            if (es3Ctx != null && es3Ctx !== EGL10.EGL_NO_CONTEXT && err == EGL10.EGL_SUCCESS) {
                onContextResolved?.invoke(3, false)
                return es3Ctx
            }
            if (es3Ctx != null && es3Ctx !== EGL10.EGL_NO_CONTEXT) {
                egl.eglDestroyContext(display, es3Ctx)
            }
        }

        val es2Attribs = intArrayOf(EGL_CONTEXT_CLIENT_VERSION, 2, EGL10.EGL_NONE)
        val es2Ctx = egl.eglCreateContext(display, eglConfig, EGL10.EGL_NO_CONTEXT, es2Attribs)
        val err2 = egl.eglGetError()
        if (es2Ctx != null && es2Ctx !== EGL10.EGL_NO_CONTEXT && err2 == EGL10.EGL_SUCCESS) {
            onContextResolved?.invoke(2, preferEs3)
            return es2Ctx
        }
        if (es2Ctx != null && es2Ctx !== EGL10.EGL_NO_CONTEXT) {
            egl.eglDestroyContext(display, es2Ctx)
        }

        throw IllegalStateException("No se pudo crear contexto OpenGL ES 3.0 ni 2.0 (EGL error=$err2).")
    }

    override fun destroyContext(egl: EGL10, display: EGLDisplay, context: EGLContext) {
        egl.eglDestroyContext(display, context)
    }
}

fun parseGlMajorVersion(versionStr: String): Int {
    val match = Regex("""OpenGL\s+ES(?:-CM)?\s+(\d+)""").find(versionStr)
    val parsed = match?.groupValues?.getOrNull(1)?.toIntOrNull()
    return if (parsed != null && parsed >= 3) 3 else 2
}

class StudioGlesClearRenderer(
    private val red: Float = 0.063f,
    private val green: Float = 0.063f,
    private val blue: Float = 0.071f
) : GLSurfaceView.Renderer {

    @Volatile
    var glVendorInfo: String = ""
        private set

    override fun onSurfaceCreated(gl: GL10?, config: EGLConfig?) {
        GLES20.glClearColor(red, green, blue, 1.0f)
        GLES20.glDisable(GLES20.GL_DEPTH_TEST)
        GLES20.glEnable(GLES20.GL_BLEND)
        GLES20.glBlendFunc(GLES20.GL_ONE, GLES20.GL_ONE_MINUS_SRC_ALPHA)
        val version = GLES20.glGetString(GLES20.GL_VERSION).orEmpty()
        val renderer = GLES20.glGetString(GLES20.GL_RENDERER).orEmpty()
        glVendorInfo = "$renderer ($version)"
    }

    override fun onSurfaceChanged(gl: GL10?, width: Int, height: Int) {
        GLES20.glViewport(0, 0, width.coerceAtLeast(1), height.coerceAtLeast(1))
    }

    override fun onDrawFrame(gl: GL10?) {
        GLES20.glClear(GLES20.GL_COLOR_BUFFER_BIT)
    }
}

@Composable
fun GlesSurfaceHost(
    targetEsVersion: Int = 3,
    modifier: Modifier = Modifier
) {
    val preferEs3 = targetEsVersion >= 3
    AndroidView(
        modifier = modifier,
        factory = { context ->
            GLSurfaceView(context).apply {
                setEGLConfigChooser(StudioGlesConfigChooser())
                setEGLContextFactory(StudioGlesContextFactory(preferEs3 = preferEs3))
                preserveEGLContextOnPause = true
                setRenderer(StudioGlesClearRenderer())
                renderMode = GLSurfaceView.RENDERMODE_WHEN_DIRTY
            }
        }
    )
}
