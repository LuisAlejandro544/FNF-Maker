// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/components/CodeEditorWithLineNumbers.kt
// Propósito: Editor de código fuente en Jetpack Compose con barra lateral sincronizada de números
// de línea, resaltado de línea activa, coloreado sintáctico ligero (Lua 5.4 / TypeScript / JavaScript),
// scroll bidireccional sin desalineación y barra rápida de símbolos para teclado móvil.
// Implementado 100% en Compose con licencia libre propia del proyecto (sin dependencias GPL/LGPL).

package com.beatforge.studio.ui.components

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.OffsetMapping
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.input.TransformedText
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.ui.theme.StudioColors

private val EDITOR_FONT_SIZE = 12.sp
private val EDITOR_LINE_HEIGHT = 18.sp

private val LUA_KEYWORDS = setOf(
    "local", "function", "end", "if", "then", "else", "elseif", "for", "while",
    "do", "repeat", "until", "return", "break", "in", "and", "or", "not", "true",
    "false", "nil"
)

private val JS_TS_KEYWORDS = setOf(
    "function", "const", "let", "var", "if", "else", "for", "while", "do", "return",
    "break", "continue", "switch", "case", "default", "interface", "type", "enum",
    "export", "import", "from", "true", "false", "null", "undefined", "void",
    "number", "string", "boolean", "new", "typeof"
)

private val BEATFORGE_APIS = setOf(
    "onCreate", "onSongStart", "onUpdate", "onBeatHit", "onStepHit", "onCountdownTick",
    "onNoteHit", "onNoteMiss", "onGameOver", "onEvent", "onTweenCompleted", "onTimerCompleted",
    "setCameraPos", "setCameraZoom", "setCameraBaseZoom", "getCameraX", "getCameraY",
    "getCameraZoom", "getCameraBaseZoom", "shakeCamera", "switchCamera", "getActiveCamera",
    "setCameraFollowPos", "clearCameraFollowPos", "setCameraClamp", "setCameraVignette",
    "tweenCameraVignette", "clearCameraVignette", "getCameraVignetteAlpha",
    "getEntityX", "getEntityY", "getEntityProp", "setEntityProp", "playAnim", "playEntityAnim",
    "setStrumProp", "getStrumProp", "tweenStrumProp", "tweenEntityProp", "tweenCameraPos",
    "tweenCameraZoom", "tweenHudProp", "tweenHudOpacity", "tweenArrowsOpacity",
    "tweenScreenBlackout", "cancelTween", "runTimer", "cancelTimer", "preloadEntity",
    "retireEntity", "swapEntity", "setActiveCharacter", "makeGraphic", "makeHudGraphic",
    "makeText", "setTextString", "cloneSprite", "removeSprite", "flashCamera", "flashHud",
    "flashEffect", "setScreenBlackout", "getScreenBlackout", "setHudVisible", "getHudVisible",
    "setHudOpacity", "getHudOpacity", "setHudZoom", "getHudZoom", "setArrowsVisible",
    "getArrowsVisible", "setArrowsOpacity", "getArrowsOpacity", "getHealth", "setHealth",
    "addHealth", "getScore", "setScore", "addScore", "getCombo", "getMisses", "getHits",
    "getAccuracy", "getScrollSpeed", "setScrollSpeed", "getDownscroll", "setDownscroll",
    "getMustHitSection", "getGfSection", "getActiveBpm", "getSongPosMs",
    "setTrackVolume", "setTrackMute", "triggerGameOver", "isGameOver", "log"
)

private class ScriptSyntaxTransformation(
    private val language: String
) : VisualTransformation {

    private val isLua = language.equals("lua", ignoreCase = true)
    private val keywords = if (isLua) LUA_KEYWORDS else JS_TS_KEYWORDS
    private val tokenRegex = Regex(
        pattern = if (isLua) {
            """(--[^\n]*)|("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')|(\b\d+(?:\.\d+)?\b)|(\b[A-Za-z_][A-Za-z0-9_]*\b)"""
        } else {
            """(//[^\n]*|/\*[\s\S]*?\*/)|("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`)|(\b\d+(?:\.\d+)?\b)|(\b[A-Za-z_][A-Za-z0-9_]*\b)"""
        }
    )

    override fun filter(text: AnnotatedString): TransformedText {
        val raw = text.text
        // Evitar trabajo costoso en archivos gigantes manteniendo rendimiento 60fps en móvil
        if (raw.length > 60_000) {
            return TransformedText(text, OffsetMapping.Identity)
        }

        val styled = buildAnnotatedString {
            append(raw)
            for (m in tokenRegex.findAll(raw)) {
                val start = m.range.first
                val end = m.range.last + 1
                when {
                    m.groups[1] != null -> {
                        addStyle(SpanStyle(color = Color(0xFF71717A)), start, end)
                    }
                    m.groups[2] != null -> {
                        addStyle(SpanStyle(color = Color(0xFF34D399)), start, end)
                    }
                    m.groups[3] != null -> {
                        addStyle(SpanStyle(color = Color(0xFFF59E0B)), start, end)
                    }
                    m.groups[4] != null -> {
                        val word = m.value
                        when {
                            keywords.contains(word) -> {
                                addStyle(
                                    SpanStyle(
                                        color = Color(0xFFC084FC),
                                        fontWeight = FontWeight.SemiBold
                                    ),
                                    start,
                                    end
                                )
                            }
                            BEATFORGE_APIS.contains(word) -> {
                                addStyle(
                                    SpanStyle(color = Color(0xFF38BDF8)),
                                    start,
                                    end
                                )
                            }
                        }
                    }
                }
            }
        }
        return TransformedText(styled, OffsetMapping.Identity)
    }
}

@Composable
fun CodeEditorWithLineNumbers(
    scriptId: String,
    language: String,
    sourceCode: String,
    onSourceChange: (String) -> Unit,
    modifier: Modifier = Modifier
) {
    var fieldValue by remember(scriptId) {
        mutableStateOf(TextFieldValue(text = sourceCode))
    }

    // Sincroniza cambios externos (por ejemplo, parches aplicados por el Asistente IA)
    LaunchedEffect(scriptId, sourceCode) {
        if (fieldValue.text != sourceCode) {
            val safePos = fieldValue.selection.start.coerceIn(0, sourceCode.length)
            fieldValue = TextFieldValue(
                text = sourceCode,
                selection = TextRange(safePos)
            )
        }
    }

    val verticalScroll = rememberScrollState()
    val horizontalScroll = rememberScrollState()
    val visualTransformation = remember(language) { ScriptSyntaxTransformation(language) }

    val lines = remember(fieldValue.text) {
        fieldValue.text.split('\n')
    }
    val lineCount = lines.size.coerceAtLeast(1)

    val cursorIdx = fieldValue.selection.start.coerceIn(0, fieldValue.text.length)
    val textBeforeCursor = fieldValue.text.substring(0, cursorIdx)
    val activeLineIndex = textBeforeCursor.count { it == '\n' }
    val lastNewLineIdx = textBeforeCursor.lastIndexOf('\n')
    val activeColumn = if (lastNewLineIdx >= 0) cursorIdx - lastNewLineIdx else cursorIdx + 1

    val gutterWidth = when {
        lineCount >= 1000 -> 48.dp
        lineCount >= 100 -> 40.dp
        else -> 34.dp
    }

    fun insertSnippet(snippet: String, cursorBackOffset: Int = 0) {
        val current = fieldValue
        val start = minOf(current.selection.start, current.selection.end).coerceIn(0, current.text.length)
        val end = maxOf(current.selection.start, current.selection.end).coerceIn(0, current.text.length)
        val nextText = buildString(current.text.length + snippet.length) {
            append(current.text, 0, start)
            append(snippet)
            append(current.text, end, current.text.length)
        }
        val nextCursor = (start + snippet.length - cursorBackOffset).coerceIn(0, nextText.length)
        fieldValue = TextFieldValue(text = nextText, selection = TextRange(nextCursor))
        onSourceChange(nextText)
    }

    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(8.dp),
        color = StudioColors.BgDark,
        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
    ) {
        Column(modifier = Modifier.fillMaxSize()) {
            // Barra superior compacta: posición de cursor + atajos útiles para teclado táctil
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(StudioColors.BgCard)
                    .padding(horizontal = 8.dp, vertical = 4.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Text(
                    text = "Lín ${activeLineIndex + 1}, Col $activeColumn · $lineCount líneas",
                    color = StudioColors.TextMuted,
                    fontSize = 10.sp,
                    fontFamily = FontFamily.Monospace
                )

                Row(
                    modifier = Modifier.horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    val commentToken = if (language.equals("lua", ignoreCase = true)) "-- " else "// "
                    listOf(
                        "Tab" to { insertSnippet("    ") },
                        "()" to { insertSnippet("()", 1) },
                        "{}" to { insertSnippet("{}", 1) },
                        "\"\"" to { insertSnippet("\"\"", 1) },
                        "=" to { insertSnippet(" = ") },
                        commentToken.trim() to { insertSnippet(commentToken) }
                    ).forEach { (label, action) ->
                        Box(
                            modifier = Modifier
                                .clip(RoundedCornerShape(4.dp))
                                .background(StudioColors.BgElevated)
                                .clickable(onClick = action)
                                .padding(horizontal = 7.dp, vertical = 2.dp)
                        ) {
                            Text(
                                text = label,
                                color = StudioColors.AccentCyan,
                                fontSize = 10.sp,
                                fontFamily = FontFamily.Monospace,
                                fontWeight = FontWeight.Bold
                            )
                        }
                    }
                }
            }

            HorizontalDivider(color = StudioColors.BorderSubtle, thickness = 1.dp)

            // Área principal: números de línea sincronizados 1:1 con el texto monoespaciado sin soft-wrap
            Row(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
                    .verticalScroll(verticalScroll)
            ) {
                Column(
                    modifier = Modifier
                        .width(gutterWidth)
                        .background(StudioColors.BgPanel.copy(alpha = 0.72f))
                        .padding(vertical = 8.dp, horizontal = 4.dp),
                    horizontalAlignment = Alignment.End
                ) {
                    for (idx in 0 until lineCount) {
                        val isActive = idx == activeLineIndex
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .background(
                                    if (isActive) StudioColors.AccentPurple.copy(alpha = 0.16f) else Color.Transparent,
                                    RoundedCornerShape(3.dp)
                                )
                                .padding(end = 3.dp),
                            contentAlignment = Alignment.CenterEnd
                        ) {
                            Text(
                                text = "${idx + 1}",
                                color = if (isActive) StudioColors.AccentCyan else StudioColors.TextMuted.copy(alpha = 0.7f),
                                fontFamily = FontFamily.Monospace,
                                fontSize = 11.sp,
                                lineHeight = EDITOR_LINE_HEIGHT,
                                fontWeight = if (isActive) FontWeight.Bold else FontWeight.Normal,
                                textAlign = TextAlign.End
                            )
                        }
                    }
                }

                VerticalDivider(
                    color = StudioColors.BorderSubtle,
                    thickness = 1.dp,
                    modifier = Modifier.fillMaxHeight()
                )

                Box(
                    modifier = Modifier
                        .weight(1f)
                        .horizontalScroll(horizontalScroll)
                        .padding(horizontal = 8.dp, vertical = 8.dp)
                ) {
                    BasicTextField(
                        value = fieldValue,
                        onValueChange = { next ->
                            val textChanged = next.text != fieldValue.text
                            fieldValue = next
                            if (textChanged) {
                                onSourceChange(next.text)
                            }
                        },
                        textStyle = TextStyle(
                            fontFamily = FontFamily.Monospace,
                            fontSize = EDITOR_FONT_SIZE,
                            lineHeight = EDITOR_LINE_HEIGHT,
                            color = StudioColors.TextPrimary
                        ),
                        cursorBrush = SolidColor(StudioColors.AccentCyan),
                        visualTransformation = visualTransformation,
                        modifier = Modifier
                            .widthIn(min = 520.dp)
                            .width(IntrinsicSize.Max)
                    )
                }
            }
        }
    }
}
