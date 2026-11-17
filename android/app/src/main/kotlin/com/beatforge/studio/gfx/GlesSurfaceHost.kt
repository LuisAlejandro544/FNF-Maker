// Archivo: android/app/src/main/kotlin/com/beatforge/studio/gfx/GlesSurfaceHost.kt
// Propósito: Host modular basado en `GLSurfaceView` para inicializar un contexto ligero de
// OpenGL ES 2.0 o OpenGL ES 3.x sobre Compose (`AndroidView`). Mantiene el terreno listo
// para el futuro renderizador GPU por lotes manteniendo estabilidad total en 32/64 bits.

package com.beatforge.studio.gfx

import android.opengl.GLES20
import android.opengl.GLSurfaceView
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.viewinterop.AndroidView
import javax.microedition.khronos.egl.EGLConfig
import javax.microedition.khronos.opengles.GL10

class StudioGlesClearRenderer(
    private val red: Float = 0.043f,
    private val green: Float = 0.059f,
    private val blue: Float = 0.098f
) : GLSurfaceView.Renderer {

    @Volatile
    var glVendorInfo: String = ""
        private set

    override fun onSurfaceCreated(gl: GL10?, config: EGLConfig?) {
        GLES20.glClearColor(red, green, blue, 1.0f)
        GLES20.glDisable(GLES20.GL_DEPTH_TEST)
        GLES20.glEnable(GLES20.GL_BLEND)
        GLES20.glBlendFunc(GLES20.GL_SRC_ALPHA, GLES20.GL_ONE_MINUS_SRC_ALPHA)
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
    val safeVersion = if (targetEsVersion >= 3) 3 else 2
    AndroidView(
        modifier = modifier,
        factory = { context ->
            GLSurfaceView(context).apply {
                setEGLContextClientVersion(safeVersion)
                preserveEGLContextOnPause = true
                setRenderer(StudioGlesClearRenderer())
                renderMode = GLSurfaceView.RENDERMODE_WHEN_DIRTY
            }
        }
    )
}
