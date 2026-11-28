// Archivo: android/app/src/main/kotlin/com/beatforge/studio/gfx/GlesShaderProgram.kt
// Propósito: Motor modular de batching 2D en OpenGL ES 3.0 / 2.0 (`GLES20` compatible)
// con compilación dual GLSL (`#version 300 es` con respaldo automático a GLSL ES 1.00),
// pila de matrices afines 2D sin GC y texturas procedurales pre-multiplicadas.

package com.beatforge.studio.gfx

import android.graphics.Bitmap
import android.opengl.GLES20
import android.opengl.GLUtils
import android.opengl.Matrix
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.FloatBuffer
import java.nio.ShortBuffer
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.hypot
import kotlin.math.sin

private const val MAX_BATCH_QUADS = 512
private const val FLOATS_PER_VERTEX = 8 // x, y, u, v, r, g, b, a
private const val VERTICES_PER_QUAD = 4
private const val INDICES_PER_QUAD = 6
private const val MATRIX_STACK_DEPTH = 16

private const val VERTEX_SHADER_ES300 = """#version 300 es
uniform mat4 uMvp;
in vec2 aPos;
in vec2 aUv;
in vec4 aColor;
out vec2 vUv;
out vec4 vColor;
void main() {
    gl_Position = uMvp * vec4(aPos, 0.0, 1.0);
    vUv = aUv;
    vColor = aColor;
}
"""

private const val FRAGMENT_SHADER_ES300 = """#version 300 es
precision mediump float;
uniform sampler2D uTex;
in vec2 vUv;
in vec4 vColor;
out vec4 fragColor;
void main() {
    fragColor = texture(uTex, vUv) * vColor;
}
"""

private const val VERTEX_SHADER_ES100 = """
uniform mat4 uMvp;
attribute vec2 aPos;
attribute vec2 aUv;
attribute vec4 aColor;
varying vec2 vUv;
varying vec4 vColor;
void main() {
    gl_Position = uMvp * vec4(aPos, 0.0, 1.0);
    vUv = aUv;
    vColor = aColor;
}
"""

private const val FRAGMENT_SHADER_ES100 = """
precision mediump float;
uniform sampler2D uTex;
varying vec2 vUv;
varying vec4 vColor;
void main() {
    gl_FragColor = texture2D(uTex, vUv) * vColor;
}
"""

internal class Gles2dBatch {
    private var programId: Int = 0
    private var uMvpLoc: Int = -1
    private var uTexLoc: Int = -1
    private var aPosLoc: Int = -1
    private var aUvLoc: Int = -1
    private var aColorLoc: Int = -1

    var usingEs3Shader: Boolean = false
        private set

    var whiteTexId: Int = 0
        private set
    var circleFillTexId: Int = 0
        private set
    var circleRingTexId: Int = 0
        private set
    var pillTexId: Int = 0
        private set

    private val projMatrix = FloatArray(16)
    private val vertexData = FloatArray(MAX_BATCH_QUADS * VERTICES_PER_QUAD * FLOATS_PER_VERTEX)
    private val vertexBuffer: FloatBuffer = ByteBuffer
        .allocateDirect(vertexData.size * 4)
        .order(ByteOrder.nativeOrder())
        .asFloatBuffer()
    private val indexBuffer: ShortBuffer = ByteBuffer
        .allocateDirect(MAX_BATCH_QUADS * INDICES_PER_QUAD * 2)
        .order(ByteOrder.nativeOrder())
        .asShortBuffer()
        .apply {
            for (q in 0 until MAX_BATCH_QUADS) {
                val base = (q * 4).toShort()
                put(base)
                put((base + 1).toShort())
                put((base + 2).toShort())
                put(base)
                put((base + 2).toShort())
                put((base + 3).toShort())
            }
            position(0)
        }

    // Pila de matrices 2D: [a, b, c, d, tx, ty] sin asignaciones
    private val matStack = FloatArray(MATRIX_STACK_DEPTH * 6)
    private var stackTop = 0

    private var quadCount = 0
    private var currentTextureId = 0
    private var viewportW = 1280f
    private var viewportH = 720f

    init {
        resetMatrix()
    }

    fun initGl(glMajorVersion: Int): Boolean {
        releaseGl()
        val compiledEs3 = if (glMajorVersion >= 3) {
            createProgram(VERTEX_SHADER_ES300, FRAGMENT_SHADER_ES300)
        } else {
            0
        }
        if (compiledEs3 != 0) {
            programId = compiledEs3
            usingEs3Shader = true
        } else {
            val compiledEs1 = createProgram(VERTEX_SHADER_ES100, FRAGMENT_SHADER_ES100)
            if (compiledEs1 == 0) return false
            programId = compiledEs1
            usingEs3Shader = false
        }

        uMvpLoc = GLES20.glGetUniformLocation(programId, "uMvp")
        uTexLoc = GLES20.glGetUniformLocation(programId, "uTex")
        aPosLoc = GLES20.glGetAttribLocation(programId, "aPos")
        aUvLoc = GLES20.glGetAttribLocation(programId, "aUv")
        aColorLoc = GLES20.glGetAttribLocation(programId, "aColor")

        whiteTexId = createWhiteTexture()
        circleFillTexId = createCircleTexture(ringOnly = false)
        circleRingTexId = createCircleTexture(ringOnly = true)
        pillTexId = createPillTexture()
        return programId != 0 && whiteTexId != 0
    }

    fun setViewport(width: Int, height: Int) {
        viewportW = width.coerceAtLeast(1).toFloat()
        viewportH = height.coerceAtLeast(1).toFloat()
        Matrix.orthoM(projMatrix, 0, 0f, viewportW, viewportH, 0f, -1f, 1f)
    }

    fun beginFrame() {
        quadCount = 0
        currentTextureId = 0
        resetMatrix()
    }

    fun endFrame() {
        flush()
        resetMatrix()
    }

    fun resetMatrix() {
        stackTop = 0
        matStack[0] = 1f
        matStack[1] = 0f
        matStack[2] = 0f
        matStack[3] = 1f
        matStack[4] = 0f
        matStack[5] = 0f
    }

    fun pushMatrix() {
        if (stackTop + 1 >= MATRIX_STACK_DEPTH) return
        val src = stackTop * 6
        val dst = (stackTop + 1) * 6
        for (k in 0..5) {
            matStack[dst + k] = matStack[src + k]
        }
        stackTop += 1
    }

    fun popMatrix() {
        if (stackTop > 0) {
            stackTop -= 1
        }
    }

    fun translate(dx: Float, dy: Float) {
        val off = stackTop * 6
        val a = matStack[off]
        val b = matStack[off + 1]
        val c = matStack[off + 2]
        val d = matStack[off + 3]
        matStack[off + 4] += a * dx + c * dy
        matStack[off + 5] += b * dx + d * dy
    }

    fun scale(sx: Float, sy: Float) {
        val off = stackTop * 6
        matStack[off] *= sx
        matStack[off + 1] *= sx
        matStack[off + 2] *= sy
        matStack[off + 3] *= sy
    }

    fun scale(sx: Float, sy: Float, pivotX: Float, pivotY: Float) {
        translate(pivotX, pivotY)
        scale(sx, sy)
        translate(-pivotX, -pivotY)
    }

    fun rotateDeg(deg: Float, pivotX: Float, pivotY: Float) {
        if (deg == 0f) return
        translate(pivotX, pivotY)
        val rad = deg * (PI.toFloat() / 180f)
        val cs = cos(rad)
        val sn = sin(rad)
        val off = stackTop * 6
        val a = matStack[off]
        val b = matStack[off + 1]
        val c = matStack[off + 2]
        val d = matStack[off + 3]
        matStack[off] = a * cs + c * sn
        matStack[off + 1] = b * cs + d * sn
        matStack[off + 2] = c * cs - a * sn
        matStack[off + 3] = d * cs - b * sn
        translate(-pivotX, -pivotY)
    }

    fun drawSolidRect(
        left: Float,
        top: Float,
        width: Float,
        height: Float,
        argb: Int,
        alphaScale: Float = 1f
    ) {
        val a = (((argb ushr 24) and 0xFF) / 255f) * alphaScale
        if (a <= 0.002f || width <= 0f || height <= 0f) return
        val r = ((argb ushr 16) and 0xFF) / 255f
        val g = ((argb ushr 8) and 0xFF) / 255f
        val b = (argb and 0xFF) / 255f
        drawTexturedQuad(
            texId = whiteTexId,
            left = left,
            top = top,
            width = width,
            height = height,
            u0 = 0f,
            v0 = 0f,
            u1 = 1f,
            v1 = 1f,
            tintR = r,
            tintG = g,
            tintB = b,
            alpha = a
        )
    }

    fun drawRectStroke(
        left: Float,
        top: Float,
        width: Float,
        height: Float,
        strokeWidth: Float,
        argb: Int,
        alphaScale: Float = 1f
    ) {
        if (width <= 0f || height <= 0f || strokeWidth <= 0f) return
        val sw = strokeWidth.coerceAtMost(minOf(width, height) * 0.5f)
        drawSolidRect(left, top, width, sw, argb, alphaScale)
        drawSolidRect(left, top + height - sw, width, sw, argb, alphaScale)
        drawSolidRect(left, top + sw, sw, (height - sw * 2f).coerceAtLeast(0f), argb, alphaScale)
        drawSolidRect(left + width - sw, top + sw, sw, (height - sw * 2f).coerceAtLeast(0f), argb, alphaScale)
    }

    fun drawLine(
        x0: Float,
        y0: Float,
        x1: Float,
        y1: Float,
        strokeWidth: Float,
        argb: Int,
        alphaScale: Float = 1f
    ) {
        val a = (((argb ushr 24) and 0xFF) / 255f) * alphaScale
        if (a <= 0.002f || strokeWidth <= 0f) return
        val dx = x1 - x0
        val dy = y1 - y0
        val len = hypot(dx, dy)
        if (len <= 0.001f) return
        val nx = (-dy / len) * (strokeWidth * 0.5f)
        val ny = (dx / len) * (strokeWidth * 0.5f)
        val r = ((argb ushr 16) and 0xFF) / 255f
        val g = ((argb ushr 8) and 0xFF) / 255f
        val b = (argb and 0xFF) / 255f
        val pa = a.coerceIn(0f, 1f)
        pushRawQuad(
            texId = whiteTexId,
            x0 = x0 + nx, y0 = y0 + ny, u0 = 0f, v0 = 0f, r0 = r * pa, g0 = g * pa, b0 = b * pa, a0 = pa,
            x1 = x0 - nx, y1 = y0 - ny, u1 = 0f, v1 = 1f, r1 = r * pa, g1 = g * pa, b1 = b * pa, a1 = pa,
            x2 = x1 - nx, y2 = y1 - ny, u2 = 1f, v2 = 1f, r2 = r * pa, g2 = g * pa, b2 = b * pa, a2 = pa,
            x3 = x1 + nx, y3 = y1 + ny, u3 = 1f, v3 = 0f, r3 = r * pa, g3 = g * pa, b3 = b * pa, a3 = pa
        )
    }

    fun drawGradientRect(
        left: Float,
        top: Float,
        width: Float,
        height: Float,
        rgbArgb: Int,
        alphaTopLeft: Float,
        alphaBottomLeft: Float,
        alphaBottomRight: Float,
        alphaTopRight: Float
    ) {
        if (width <= 0f || height <= 0f) return
        val r = ((rgbArgb ushr 16) and 0xFF) / 255f
        val g = ((rgbArgb ushr 8) and 0xFF) / 255f
        val b = (rgbArgb and 0xFF) / 255f
        val a0 = alphaTopLeft.coerceIn(0f, 1f)
        val a1 = alphaBottomLeft.coerceIn(0f, 1f)
        val a2 = alphaBottomRight.coerceIn(0f, 1f)
        val a3 = alphaTopRight.coerceIn(0f, 1f)
        if (a0 + a1 + a2 + a3 <= 0.002f) return
        pushRawQuad(
            texId = whiteTexId,
            x0 = left, y0 = top, u0 = 0f, v0 = 0f, r0 = r * a0, g0 = g * a0, b0 = b * a0, a0 = a0,
            x1 = left, y1 = top + height, u1 = 0f, v1 = 1f, r1 = r * a1, g1 = g * a1, b1 = b * a1, a1 = a1,
            x2 = left + width, y2 = top + height, u2 = 1f, v2 = 1f, r2 = r * a2, g2 = g * a2, b2 = b * a2, a2 = a2,
            x3 = left + width, y3 = top, u3 = 1f, v3 = 0f, r3 = r * a3, g3 = g * a3, b3 = b * a3, a3 = a3
        )
    }

    fun drawCircleFill(cx: Float, cy: Float, radius: Float, argb: Int, alphaScale: Float = 1f) {
        val a = (((argb ushr 24) and 0xFF) / 255f) * alphaScale
        if (a <= 0.002f || radius <= 0f) return
        // El disco interno ocupa ~58/64 de la textura procedural para suavizar bordes
        val drawRadius = radius * (64f / 57.5f)
        val r = ((argb ushr 16) and 0xFF) / 255f
        val g = ((argb ushr 8) and 0xFF) / 255f
        val b = (argb and 0xFF) / 255f
        drawTexturedQuad(
            texId = circleFillTexId,
            left = cx - drawRadius,
            top = cy - drawRadius,
            width = drawRadius * 2f,
            height = drawRadius * 2f,
            u0 = 0f,
            v0 = 0f,
            u1 = 1f,
            v1 = 1f,
            tintR = r,
            tintG = g,
            tintB = b,
            alpha = a
        )
    }

    fun drawCircleRing(cx: Float, cy: Float, radius: Float, argb: Int, alphaScale: Float = 1f) {
        val a = (((argb ushr 24) and 0xFF) / 255f) * alphaScale
        if (a <= 0.002f || radius <= 0f) return
        val drawRadius = radius * (64f / 55f)
        val r = ((argb ushr 16) and 0xFF) / 255f
        val g = ((argb ushr 8) and 0xFF) / 255f
        val b = (argb and 0xFF) / 255f
        drawTexturedQuad(
            texId = circleRingTexId,
            left = cx - drawRadius,
            top = cy - drawRadius,
            width = drawRadius * 2f,
            height = drawRadius * 2f,
            u0 = 0f,
            v0 = 0f,
            u1 = 1f,
            v1 = 1f,
            tintR = r,
            tintG = g,
            tintB = b,
            alpha = a
        )
    }

    fun drawOvalFill(left: Float, top: Float, width: Float, height: Float, argb: Int, alphaScale: Float = 1f) {
        val a = (((argb ushr 24) and 0xFF) / 255f) * alphaScale
        if (a <= 0.002f || width <= 0f || height <= 0f) return
        val r = ((argb ushr 16) and 0xFF) / 255f
        val g = ((argb ushr 8) and 0xFF) / 255f
        val b = (argb and 0xFF) / 255f
        drawTexturedQuad(
            texId = circleFillTexId,
            left = left,
            top = top,
            width = width,
            height = height,
            u0 = 0f,
            v0 = 0f,
            u1 = 1f,
            v1 = 1f,
            tintR = r,
            tintG = g,
            tintB = b,
            alpha = a
        )
    }

    fun drawRoundedVerticalPill(
        left: Float,
        top: Float,
        width: Float,
        height: Float,
        argb: Int,
        alphaScale: Float = 1f
    ) {
        val a = (((argb ushr 24) and 0xFF) / 255f) * alphaScale
        if (a <= 0.002f || width <= 0f || height <= 0f) return
        val r = ((argb ushr 16) and 0xFF) / 255f
        val g = ((argb ushr 8) and 0xFF) / 255f
        val b = (argb and 0xFF) / 255f

        if (height <= width) {
            drawTexturedQuad(pillTexId, left, top, width, height, 0f, 0f, 1f, 1f, r, g, b, a)
            return
        }
        val capH = width * 0.48f
        val midH = (height - capH * 2f).coerceAtLeast(0f)
        drawTexturedQuad(pillTexId, left, top, width, capH, 0f, 0f, 1f, 0.46f, r, g, b, a)
        if (midH > 0.5f) {
            drawTexturedQuad(pillTexId, left, top + capH, width, midH, 0f, 0.48f, 1f, 0.52f, r, g, b, a)
        }
        drawTexturedQuad(pillTexId, left, top + height - capH, width, capH, 0f, 0.54f, 1f, 1f, r, g, b, a)
    }

    fun drawTexturedQuad(
        texId: Int,
        left: Float,
        top: Float,
        width: Float,
        height: Float,
        u0: Float = 0f,
        v0: Float = 0f,
        u1: Float = 1f,
        v1: Float = 1f,
        tintR: Float = 1f,
        tintG: Float = 1f,
        tintB: Float = 1f,
        alpha: Float = 1f
    ) {
        if (texId == 0 || width <= 0f || height <= 0f) return
        val a = alpha.coerceIn(0f, 1f)
        if (a <= 0.002f) return
        val pr = tintR * a
        val pg = tintG * a
        val pb = tintB * a
        pushRawQuad(
            texId = texId,
            x0 = left, y0 = top, u0 = u0, v0 = v0, r0 = pr, g0 = pg, b0 = pb, a0 = a,
            x1 = left, y1 = top + height, u1 = u0, v1 = v1, r1 = pr, g1 = pg, b1 = pb, a1 = a,
            x2 = left + width, y2 = top + height, u2 = u1, v2 = v1, r2 = pr, g2 = pg, b2 = pb, a2 = a,
            x3 = left + width, y3 = top, u3 = u1, v3 = v0, r3 = pr, g3 = pg, b3 = pb, a3 = a
        )
    }

    private fun pushRawQuad(
        texId: Int,
        x0: Float, y0: Float, u0: Float, v0: Float, r0: Float, g0: Float, b0: Float, a0: Float,
        x1: Float, y1: Float, u1: Float, v1: Float, r1: Float, g1: Float, b1: Float, a1: Float,
        x2: Float, y2: Float, u2: Float, v2: Float, r2: Float, g2: Float, b2: Float, a2: Float,
        x3: Float, y3: Float, u3: Float, v3: Float, r3: Float, g3: Float, b3: Float, a3: Float
    ) {
        if (texId != currentTextureId || quadCount >= MAX_BATCH_QUADS) {
            flush()
            currentTextureId = texId
        }

        val mOff = stackTop * 6
        val ma = matStack[mOff]
        val mb = matStack[mOff + 1]
        val mc = matStack[mOff + 2]
        val md = matStack[mOff + 3]
        val mtx = matStack[mOff + 4]
        val mty = matStack[mOff + 5]

        var idx = quadCount * VERTICES_PER_QUAD * FLOATS_PER_VERTEX
        // V0 (top-left)
        vertexData[idx++] = ma * x0 + mc * y0 + mtx
        vertexData[idx++] = mb * x0 + md * y0 + mty
        vertexData[idx++] = u0
        vertexData[idx++] = v0
        vertexData[idx++] = r0
        vertexData[idx++] = g0
        vertexData[idx++] = b0
        vertexData[idx++] = a0

        // V1 (bottom-left)
        vertexData[idx++] = ma * x1 + mc * y1 + mtx
        vertexData[idx++] = mb * x1 + md * y1 + mty
        vertexData[idx++] = u1
        vertexData[idx++] = v1
        vertexData[idx++] = r1
        vertexData[idx++] = g1
        vertexData[idx++] = b1
        vertexData[idx++] = a1

        // V2 (bottom-right)
        vertexData[idx++] = ma * x2 + mc * y2 + mtx
        vertexData[idx++] = mb * x2 + md * y2 + mty
        vertexData[idx++] = u2
        vertexData[idx++] = v2
        vertexData[idx++] = r2
        vertexData[idx++] = g2
        vertexData[idx++] = b2
        vertexData[idx++] = a2

        // V3 (top-right)
        vertexData[idx++] = ma * x3 + mc * y3 + mtx
        vertexData[idx++] = mb * x3 + md * y3 + mty
        vertexData[idx++] = u3
        vertexData[idx++] = v3
        vertexData[idx++] = r3
        vertexData[idx++] = g3
        vertexData[idx++] = b3
        vertexData[idx] = a3

        quadCount += 1
    }

    fun flush() {
        if (quadCount <= 0 || programId == 0 || currentTextureId == 0) {
            quadCount = 0
            return
        }

        val floatCount = quadCount * VERTICES_PER_QUAD * FLOATS_PER_VERTEX
        vertexBuffer.position(0)
        vertexBuffer.put(vertexData, 0, floatCount)
        vertexBuffer.position(0)

        GLES20.glUseProgram(programId)
        GLES20.glUniformMatrix4fv(uMvpLoc, 1, false, projMatrix, 0)

        GLES20.glActiveTexture(GLES20.GL_TEXTURE0)
        GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, currentTextureId)
        GLES20.glUniform1i(uTexLoc, 0)

        val strideBytes = FLOATS_PER_VERTEX * 4
        vertexBuffer.position(0)
        GLES20.glEnableVertexAttribArray(aPosLoc)
        GLES20.glVertexAttribPointer(aPosLoc, 2, GLES20.GL_FLOAT, false, strideBytes, vertexBuffer)

        vertexBuffer.position(2)
        GLES20.glEnableVertexAttribArray(aUvLoc)
        GLES20.glVertexAttribPointer(aUvLoc, 2, GLES20.GL_FLOAT, false, strideBytes, vertexBuffer)

        vertexBuffer.position(4)
        GLES20.glEnableVertexAttribArray(aColorLoc)
        GLES20.glVertexAttribPointer(aColorLoc, 4, GLES20.GL_FLOAT, false, strideBytes, vertexBuffer)

        indexBuffer.position(0)
        GLES20.glDrawElements(
            GLES20.GL_TRIANGLES,
            quadCount * INDICES_PER_QUAD,
            GLES20.GL_UNSIGNED_SHORT,
            indexBuffer
        )

        GLES20.glDisableVertexAttribArray(aPosLoc)
        GLES20.glDisableVertexAttribArray(aUvLoc)
        GLES20.glDisableVertexAttribArray(aColorLoc)

        quadCount = 0
    }

    fun releaseGl() {
        val texIds = intArrayOf(whiteTexId, circleFillTexId, circleRingTexId, pillTexId)
        val toDelete = texIds.filter { it != 0 }.toIntArray()
        if (toDelete.isNotEmpty()) {
            GLES20.glDeleteTextures(toDelete.size, toDelete, 0)
        }
        whiteTexId = 0
        circleFillTexId = 0
        circleRingTexId = 0
        pillTexId = 0

        if (programId != 0) {
            GLES20.glDeleteProgram(programId)
            programId = 0
        }
    }

    private fun createProgram(vsSource: String, fsSource: String): Int {
        val vs = compileShader(GLES20.GL_VERTEX_SHADER, vsSource)
        if (vs == 0) return 0
        val fs = compileShader(GLES20.GL_FRAGMENT_SHADER, fsSource)
        if (fs == 0) {
            GLES20.glDeleteShader(vs)
            return 0
        }
        val prog = GLES20.glCreateProgram()
        if (prog == 0) {
            GLES20.glDeleteShader(vs)
            GLES20.glDeleteShader(fs)
            return 0
        }
        GLES20.glAttachShader(prog, vs)
        GLES20.glAttachShader(prog, fs)
        GLES20.glLinkProgram(prog)
        val linkStatus = IntArray(1)
        GLES20.glGetProgramiv(prog, GLES20.GL_LINK_STATUS, linkStatus, 0)
        GLES20.glDeleteShader(vs)
        GLES20.glDeleteShader(fs)
        if (linkStatus[0] == 0) {
            GLES20.glDeleteProgram(prog)
            return 0
        }
        return prog
    }

    private fun compileShader(type: Int, src: String): Int {
        val shader = GLES20.glCreateShader(type)
        if (shader == 0) return 0
        GLES20.glShaderSource(shader, src)
        GLES20.glCompileShader(shader)
        val status = IntArray(1)
        GLES20.glGetShaderiv(shader, GLES20.GL_COMPILE_STATUS, status, 0)
        if (status[0] == 0) {
            GLES20.glDeleteShader(shader)
            return 0
        }
        return shader
    }

    private fun createWhiteTexture(): Int {
        val bmp = Bitmap.createBitmap(2, 2, Bitmap.Config.ARGB_8888)
        bmp.eraseColor(android.graphics.Color.WHITE)
        return uploadProceduralBitmap(bmp)
    }

    private fun createCircleTexture(ringOnly: Boolean): Int {
        val size = 128
        val bmp = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
        val pixels = IntArray(size * size)
        val half = (size - 1) * 0.5f
        val outerR = 57.5f
        val innerR = 45.5f
        for (y in 0 until size) {
            val dy = y - half
            for (x in 0 until size) {
                val dx = x - half
                val dist = hypot(dx, dy)
                val alpha = if (!ringOnly) {
                    (outerR + 1.2f - dist).coerceIn(0f, 1f)
                } else {
                    val outerA = (outerR + 1.2f - dist).coerceIn(0f, 1f)
                    val innerA = (dist - (innerR - 1.2f)).coerceIn(0f, 1f)
                    minOf(outerA, innerA)
                }
                val aInt = (alpha * 255f).toInt().coerceIn(0, 255)
                // Premultiplied white (R=G=B=A)
                pixels[y * size + x] = (aInt shl 24) or (aInt shl 16) or (aInt shl 8) or aInt
            }
        }
        bmp.setPixels(pixels, 0, size, 0, 0, size, size)
        return uploadProceduralBitmap(bmp)
    }

    private fun createPillTexture(): Int {
        val size = 64
        val bmp = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888)
        val pixels = IntArray(size * size)
        val halfW = (size - 1) * 0.5f
        val capCenterTop = 28.5f
        val capCenterBottom = size - 1 - 28.5f
        val radius = 27.5f
        for (y in 0 until size) {
            val cy = when {
                y < capCenterTop -> capCenterTop
                y > capCenterBottom -> capCenterBottom
                else -> y.toFloat()
            }
            val dy = y - cy
            for (x in 0 until size) {
                val dx = x - halfW
                val dist = hypot(dx, dy)
                val alpha = (radius + 1.0f - dist).coerceIn(0f, 1f)
                val aInt = (alpha * 255f).toInt().coerceIn(0, 255)
                pixels[y * size + x] = (aInt shl 24) or (aInt shl 16) or (aInt shl 8) or aInt
            }
        }
        bmp.setPixels(pixels, 0, size, 0, 0, size, size)
        return uploadProceduralBitmap(bmp)
    }

    private fun uploadProceduralBitmap(bmp: Bitmap): Int {
        val ids = IntArray(1)
        GLES20.glGenTextures(1, ids, 0)
        val texId = ids[0]
        if (texId != 0) {
            GLES20.glBindTexture(GLES20.GL_TEXTURE_2D, texId)
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MIN_FILTER, GLES20.GL_LINEAR)
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_MAG_FILTER, GLES20.GL_LINEAR)
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_S, GLES20.GL_CLAMP_TO_EDGE)
            GLES20.glTexParameteri(GLES20.GL_TEXTURE_2D, GLES20.GL_TEXTURE_WRAP_T, GLES20.GL_CLAMP_TO_EDGE)
            GLUtils.texImage2D(GLES20.GL_TEXTURE_2D, 0, bmp, 0)
        }
        bmp.recycle()
        return texId
    }
}
