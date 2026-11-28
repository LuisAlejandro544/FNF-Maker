// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/ScriptAiAssistantScreen.kt
// Propósito: Pantalla dedicada del Asistente IA para Script Studio (separada del editor para
// evitar saturar una sola vista). Incluye pestaña de Chat en streaming SSE con registro de
// herramientas ejecutadas (lectura/parcheo parcial, sprites sin imágenes, tiempos del chart) y
// pestaña de Configuración (OpenRouter API Key local, selector de modelos con
// `baidu/ernie-5.5-flash-preview:free`, System Prompt editable y visor/toggle de `SKILL.md`).

package com.beatforge.studio.ui.screens

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.Chat
import androidx.compose.material.icons.automirrored.rounded.MenuBook
import androidx.compose.material.icons.automirrored.rounded.Send
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.ArrowDropDown
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.Build
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.ChevronLeft
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.DeleteSweep
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.Key
import androidx.compose.material.icons.rounded.Restore
import androidx.compose.material.icons.rounded.Settings
import androidx.compose.material.icons.rounded.SmartToy
import androidx.compose.material.icons.rounded.Stop
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material.icons.rounded.VisibilityOff
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardCapitalization
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.ProjectSnapshot
import com.beatforge.studio.ai.AiAssistantUiState
import com.beatforge.studio.ai.AiChatMessage
import com.beatforge.studio.ai.AiSkillDoc
import com.beatforge.studio.ai.DEFAULT_OPENROUTER_MODEL
import com.beatforge.studio.ai.OPENROUTER_PRESET_MODELS
import com.beatforge.studio.ui.theme.StudioColors

private val QUICK_PROMPTS = listOf(
    "Traducir Psych Lua" to "Inspecciona el script abierto con read_script y traduce cualquier función de Psych Engine a la API oficial de BeatForge usando patch_script.",
    "Ver sprites y escena" to "Usa list_stage_sprites y get_chart_timing_info para decirme exactamente qué sprites, personajes, cámaras y secciones tenemos en este mod.",
    "Animar sprite en beat" to "Usa list_stage_sprites para ver los sprites disponibles y agrega en stage_events.lua (con patch_script) un efecto visual o movimiento en onBeatHit sobre un sprite real.",
    "Revisar y corregir" to "Lista los scripts con list_scripts, lee el script activo y corrige cualquier error de sintaxis o API usando patch_script."
)

@Composable
fun ScriptAiAssistantScreen(
    snapshot: ProjectSnapshot,
    selectedScriptId: String,
    aiState: AiAssistantUiState,
    onBackToScripts: () -> Unit,
    onSendMessage: (String) -> Unit,
    onStopStreaming: () -> Unit,
    onClearChat: () -> Unit,
    onUpdateApiKey: (String) -> Unit,
    onSelectModel: (String) -> Unit,
    onAddCustomModel: (String) -> Unit,
    onRemoveCustomModel: (String) -> Unit,
    onUpdateSystemPrompt: (String) -> Unit,
    onRestoreDefaultSystemPrompt: () -> Unit,
    onToggleSkill: (skillId: String, enabled: Boolean) -> Unit
) {
    var activeTab by remember { mutableIntStateOf(0) }
    var inspectedSkill by remember { mutableStateOf<AiSkillDoc?>(null) }
    val isImeVisible = WindowInsets.ime.getBottom(LocalDensity.current) > 0

    val activeScript = snapshot.scripts.find { it.id == selectedScriptId }
        ?: snapshot.scripts.firstOrNull()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 10.dp, vertical = if (isImeVisible) 5.dp else 10.dp),
        verticalArrangement = Arrangement.spacedBy(if (isImeVisible) 5.dp else 8.dp)
    ) {
        // Cabecera compacta con botón de retorno, contexto actual y pestañas (se comprime al abrir teclado)
        Surface(
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(
                        horizontal = 10.dp,
                        vertical = if (isImeVisible && activeTab == 0) 5.dp else 8.dp
                    ),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.weight(1f)
                ) {
                    OutlinedButton(
                        onClick = onBackToScripts,
                        shape = RoundedCornerShape(8.dp),
                        contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                        modifier = Modifier.height(if (isImeVisible && activeTab == 0) 28.dp else 32.dp)
                    ) {
                        Icon(Icons.Rounded.ChevronLeft, null, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(2.dp))
                        Text("Scripts", fontSize = 11.sp, fontWeight = FontWeight.SemiBold, softWrap = false)
                    }

                    Spacer(Modifier.width(10.dp))

                    Icon(
                        imageVector = Icons.Rounded.AutoAwesome,
                        contentDescription = null,
                        tint = StudioColors.AccentPurple,
                        modifier = Modifier.size(18.dp)
                    )
                    Spacer(Modifier.width(6.dp))
                    Column {
                        Text(
                            text = "Asistente IA · OpenRouter",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = if (isImeVisible && activeTab == 0) 13.sp else 14.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis
                        )
                        if (!(isImeVisible && activeTab == 0)) {
                            val scriptLabel = activeScript?.name ?: "Sin script"
                            val enabledSkillsCount = aiState.skills.count { it.enabled }
                            Text(
                                text = "Script: $scriptLabel · Modelo: ${aiState.settings.selectedModel} · $enabledSkillsCount/${aiState.skills.size} skills",
                                color = StudioColors.TextMuted,
                                fontSize = 10.sp,
                                maxLines = 1,
                                overflow = TextOverflow.Ellipsis
                            )
                        }
                    }
                }

                Spacer(Modifier.width(8.dp))

                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    TabPillButton(
                        label = "Chat y Herramientas",
                        icon = Icons.AutoMirrored.Rounded.Chat,
                        selected = activeTab == 0,
                        accent = StudioColors.AccentPurple,
                        onClick = { activeTab = 0 }
                    )
                    TabPillButton(
                        label = "Configuración y Skills",
                        icon = Icons.Rounded.Settings,
                        selected = activeTab == 1,
                        accent = StudioColors.AccentCyan,
                        onClick = { activeTab = 1 }
                    )
                }
            }
        }

        if (activeTab == 0) {
            AiChatWorkspaceTab(
                aiState = aiState,
                isImeVisible = isImeVisible,
                onOpenSettings = { activeTab = 1 },
                onSendMessage = onSendMessage,
                onStopStreaming = onStopStreaming,
                onClearChat = onClearChat,
                modifier = Modifier.weight(1f)
            )
        } else {
            AiSettingsAndSkillsTab(
                aiState = aiState,
                onUpdateApiKey = onUpdateApiKey,
                onSelectModel = onSelectModel,
                onAddCustomModel = onAddCustomModel,
                onRemoveCustomModel = onRemoveCustomModel,
                onUpdateSystemPrompt = onUpdateSystemPrompt,
                onRestoreDefaultSystemPrompt = onRestoreDefaultSystemPrompt,
                onToggleSkill = onToggleSkill,
                onInspectSkill = { inspectedSkill = it },
                modifier = Modifier.weight(1f)
            )
        }
    }

    val previewSkill = inspectedSkill
    if (previewSkill != null) {
        AlertDialog(
            onDismissRequest = { inspectedSkill = null },
            containerColor = StudioColors.BgPanel,
            titleContentColor = StudioColors.TextPrimary,
            textContentColor = StudioColors.TextPrimary,
            title = {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.AutoMirrored.Rounded.MenuBook, null, tint = StudioColors.AccentCyan, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(8.dp))
                    Text("${previewSkill.folderName}/SKILL.md", fontSize = 15.sp, fontWeight = FontWeight.Bold)
                }
            },
            text = {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(260.dp)
                        .background(StudioColors.BgDark, RoundedCornerShape(8.dp))
                        .padding(10.dp)
                        .verticalScroll(rememberScrollState())
                ) {
                    SelectionContainer {
                        Text(
                            text = previewSkill.content,
                            color = StudioColors.TextPrimary,
                            fontFamily = FontFamily.Monospace,
                            fontSize = 11.sp,
                            lineHeight = 16.sp
                        )
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { inspectedSkill = null }) {
                    Text("Cerrar", color = StudioColors.AccentCyan, fontWeight = FontWeight.Bold)
                }
            }
        )
    }
}

@Composable
private fun TabPillButton(
    label: String,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    selected: Boolean,
    accent: Color,
    onClick: () -> Unit
) {
    Surface(
        modifier = Modifier.clickable(onClick = onClick),
        shape = RoundedCornerShape(8.dp),
        color = if (selected) accent.copy(alpha = 0.18f) else StudioColors.BgCard,
        border = BorderStroke(1.dp, if (selected) accent else StudioColors.BorderSubtle)
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 10.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(5.dp)
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = if (selected) accent else StudioColors.TextMuted,
                modifier = Modifier.size(14.dp)
            )
            Text(
                text = label,
                color = if (selected) StudioColors.TextPrimary else StudioColors.TextMuted,
                fontSize = 11.sp,
                fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
                maxLines = 1
            )
        }
    }
}

@Composable
private fun AiChatWorkspaceTab(
    aiState: AiAssistantUiState,
    isImeVisible: Boolean,
    onOpenSettings: () -> Unit,
    onSendMessage: (String) -> Unit,
    onStopStreaming: () -> Unit,
    onClearChat: () -> Unit,
    modifier: Modifier = Modifier
) {
    var promptInput by remember { mutableStateOf("") }
    val scrollState = rememberScrollState()

    val lastMsgLength = aiState.messages.lastOrNull()?.content?.length ?: 0
    LaunchedEffect(aiState.messages.size, lastMsgLength, isImeVisible) {
        scrollState.animateScrollTo(scrollState.maxValue)
    }

    Column(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(if (isImeVisible) 5.dp else 8.dp)
    ) {
        if (aiState.settings.apiKey.isBlank() && !isImeVisible) {
            Surface(
                shape = RoundedCornerShape(10.dp),
                color = StudioColors.AccentAmber.copy(alpha = 0.12f),
                border = BorderStroke(1.dp, StudioColors.AccentAmber.copy(alpha = 0.6f))
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 12.dp, vertical = 7.dp),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.SpaceBetween
                ) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.weight(1f)
                    ) {
                        Icon(Icons.Rounded.Key, null, tint = StudioColors.AccentAmber, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(8.dp))
                        Text(
                            text = "Falta configurar tu API Key de OpenRouter para chatear y parchear scripts en vivo.",
                            color = StudioColors.TextPrimary,
                            fontSize = 11.sp,
                            maxLines = 2,
                            overflow = TextOverflow.Ellipsis
                        )
                    }
                    Spacer(Modifier.width(8.dp))
                    Button(
                        onClick = onOpenSettings,
                        contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = StudioColors.AccentAmber,
                            contentColor = Color(0xFF231800)
                        ),
                        modifier = Modifier.height(28.dp)
                    ) {
                        Text("Configurar Key", fontSize = 11.sp, fontWeight = FontWeight.Bold, softWrap = false)
                    }
                }
            }
        }

        // Atajos rápidos de flujo de trabajo (ocultos al abrir el teclado para priorizar chat + entrada)
        if (!isImeVisible) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                QUICK_PROMPTS.forEach { (title, fullPrompt) ->
                    Surface(
                        modifier = Modifier.clickable(enabled = !aiState.isStreaming) {
                            promptInput = fullPrompt
                        },
                        shape = RoundedCornerShape(8.dp),
                        color = StudioColors.BgCard,
                        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
                    ) {
                        Row(
                            modifier = Modifier.padding(horizontal = 10.dp, vertical = 5.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(5.dp)
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.AutoAwesome,
                                contentDescription = null,
                                tint = StudioColors.AccentPurple,
                                modifier = Modifier.size(12.dp)
                            )
                            Text(
                                text = title,
                                color = StudioColors.AccentCyan,
                                fontSize = 11.sp,
                                fontWeight = FontWeight.SemiBold,
                                softWrap = false
                            )
                        }
                    }
                }
            }
        }

        // Historial de conversación + streaming en vivo
        Surface(
            modifier = Modifier
                .weight(1f)
                .fillMaxWidth(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            if (aiState.messages.isEmpty()) {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .verticalScroll(rememberScrollState())
                        .padding(horizontal = 18.dp, vertical = 12.dp),
                    contentAlignment = Alignment.Center
                ) {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(7.dp)
                    ) {
                        Surface(
                            shape = CircleShape,
                            color = StudioColors.AccentPurple.copy(alpha = 0.16f),
                            border = BorderStroke(1.dp, StudioColors.AccentPurple.copy(alpha = 0.55f))
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.SmartToy,
                                contentDescription = null,
                                tint = StudioColors.AccentPurple,
                                modifier = Modifier
                                    .padding(8.dp)
                                    .size(24.dp)
                            )
                        }
                        Text(
                            text = "Asistente de Scripts y Escenario",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 14.sp,
                            textAlign = TextAlign.Center
                        )
                        Text(
                            text = "Lee tus archivos .lua / .ts / .js, aplica parches quirúrgicos sin sobrescribir todo y consulta sprites o tiempos del chart.",
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp,
                            lineHeight = 15.sp,
                            textAlign = TextAlign.Center,
                            modifier = Modifier.widthIn(max = 520.dp)
                        )
                        Row(
                            horizontalArrangement = Arrangement.spacedBy(6.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            listOf(
                                "Parches quirúrgicos" to StudioColors.AccentEmerald,
                                "Nombres y posiciones" to StudioColors.AccentCyan,
                                "Tiempos del Chart" to StudioColors.AccentAmber
                            ).forEach { (badge, tint) ->
                                Surface(
                                    shape = RoundedCornerShape(6.dp),
                                    color = tint.copy(alpha = 0.10f),
                                    border = BorderStroke(1.dp, tint.copy(alpha = 0.45f))
                                ) {
                                    Text(
                                        text = badge,
                                        color = tint,
                                        fontSize = 10.sp,
                                        fontWeight = FontWeight.SemiBold,
                                        modifier = Modifier.padding(horizontal = 7.dp, vertical = 3.dp)
                                    )
                                }
                            }
                        }
                    }
                }
            } else {
                Column(
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(10.dp)
                        .verticalScroll(scrollState),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    aiState.messages.forEach { msg ->
                        ChatMessageBubble(
                            message = msg,
                            activeStatus = if (msg.isStreaming) aiState.activeStatus else ""
                        )
                    }
                }
            }
        }

        // Tarjeta inferior de envío y control de streaming siempre visible sobre el teclado
        Surface(
            modifier = Modifier.fillMaxWidth(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                IconButton(
                    onClick = onClearChat,
                    enabled = aiState.messages.isNotEmpty(),
                    modifier = Modifier.size(36.dp)
                ) {
                    Icon(
                        imageVector = Icons.Rounded.DeleteSweep,
                        contentDescription = "Limpiar chat",
                        tint = if (aiState.messages.isNotEmpty()) StudioColors.TextMuted else StudioColors.BorderSubtle
                    )
                }

                OutlinedTextField(
                    value = promptInput,
                    onValueChange = { promptInput = it },
                    placeholder = {
                        Text(
                            text = "Pide traducir un Lua, mover cierto sprite en el beat 32 o corregir errores...",
                            fontSize = 12.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis
                        )
                    },
                    maxLines = 3,
                    keyboardOptions = KeyboardOptions(capitalization = KeyboardCapitalization.Sentences),
                    textStyle = TextStyle(fontSize = 12.sp, color = StudioColors.TextPrimary),
                    modifier = Modifier.weight(1f)
                )

                if (aiState.isStreaming) {
                    Button(
                        onClick = onStopStreaming,
                        colors = ButtonDefaults.buttonColors(
                            containerColor = StudioColors.AccentCoral,
                            contentColor = Color.White
                        ),
                        contentPadding = PaddingValues(horizontal = 12.dp, vertical = 8.dp),
                        modifier = Modifier.height(42.dp)
                    ) {
                        Icon(Icons.Rounded.Stop, null, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(4.dp))
                        Text("Detener", fontSize = 12.sp, fontWeight = FontWeight.Bold, softWrap = false)
                    }
                } else {
                    Button(
                        onClick = {
                            val toSend = promptInput.trim()
                            if (toSend.isNotEmpty()) {
                                promptInput = ""
                                onSendMessage(toSend)
                            }
                        },
                        enabled = promptInput.isNotBlank(),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = StudioColors.AccentPurple,
                            contentColor = Color.White
                        ),
                        contentPadding = PaddingValues(horizontal = 14.dp, vertical = 8.dp),
                        modifier = Modifier.height(42.dp)
                    ) {
                        Icon(Icons.AutoMirrored.Rounded.Send, null, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(5.dp))
                        Text("Enviar", fontSize = 12.sp, fontWeight = FontWeight.Bold, softWrap = false)
                    }
                }
            }
        }
    }
}

@Composable
private fun ChatMessageBubble(
    message: AiChatMessage,
    activeStatus: String
) {
    val isUser = message.role == "user"
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = if (isUser) Arrangement.End else Arrangement.Start
    ) {
        Surface(
            modifier = Modifier.fillMaxWidth(0.88f),
            shape = RoundedCornerShape(10.dp),
            color = when {
                isUser -> StudioColors.AccentCyan.copy(alpha = 0.14f)
                message.errorMessage != null -> StudioColors.AccentCoral.copy(alpha = 0.10f)
                else -> StudioColors.BgCard
            },
            border = BorderStroke(
                1.dp,
                when {
                    isUser -> StudioColors.AccentCyan.copy(alpha = 0.65f)
                    message.errorMessage != null -> StudioColors.AccentCoral
                    else -> StudioColors.BorderSubtle
                }
            )
        ) {
            Column(
                modifier = Modifier.padding(10.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp)
            ) {
                Text(
                    text = if (isUser) "Tú" else "Asistente BeatForge",
                    color = if (isUser) StudioColors.AccentCyan else StudioColors.AccentPurple,
                    fontSize = 10.sp,
                    fontWeight = FontWeight.Bold
                )

                if (message.toolCalls.isNotEmpty()) {
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        message.toolCalls.forEach { tc ->
                            Surface(
                                shape = RoundedCornerShape(6.dp),
                                color = StudioColors.BgDark,
                                border = BorderStroke(
                                    1.dp,
                                    if (tc.ok) StudioColors.AccentEmerald.copy(alpha = 0.45f) else StudioColors.AccentCoral.copy(alpha = 0.6f)
                                )
                            ) {
                                Row(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .padding(horizontal = 8.dp, vertical = 4.dp),
                                    verticalAlignment = Alignment.CenterVertically,
                                    horizontalArrangement = Arrangement.spacedBy(6.dp)
                                ) {
                                    Icon(
                                        imageVector = if (tc.ok) Icons.Rounded.Build else Icons.Rounded.ErrorOutline,
                                        contentDescription = null,
                                        tint = if (tc.ok) StudioColors.AccentEmerald else StudioColors.AccentCoral,
                                        modifier = Modifier.size(13.dp)
                                    )
                                    Text(
                                        text = "${tc.toolName}: ${tc.resultSummary}",
                                        color = StudioColors.TextPrimary,
                                        fontFamily = FontFamily.Monospace,
                                        fontSize = 10.sp,
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis
                                    )
                                }
                            }
                        }
                    }
                }

                if (message.content.isNotBlank()) {
                    SelectionContainer {
                        Text(
                            text = message.content,
                            color = StudioColors.TextPrimary,
                            fontSize = 12.sp,
                            lineHeight = 17.sp
                        )
                    }
                }

                if (message.isStreaming) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(13.dp),
                            strokeWidth = 2.dp,
                            color = StudioColors.AccentPurple
                        )
                        Text(
                            text = activeStatus.ifBlank { "Generando respuesta..." },
                            color = StudioColors.TextMuted,
                            fontSize = 10.sp
                        )
                    }
                }

                if (!message.errorMessage.isNullOrBlank()) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        Icon(
                            imageVector = Icons.Rounded.ErrorOutline,
                            contentDescription = null,
                            tint = StudioColors.AccentCoral,
                            modifier = Modifier.size(15.dp)
                        )
                        Text(
                            text = message.errorMessage,
                            color = StudioColors.AccentCoral,
                            fontSize = 11.sp
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun AiSettingsAndSkillsTab(
    aiState: AiAssistantUiState,
    onUpdateApiKey: (String) -> Unit,
    onSelectModel: (String) -> Unit,
    onAddCustomModel: (String) -> Unit,
    onRemoveCustomModel: (String) -> Unit,
    onUpdateSystemPrompt: (String) -> Unit,
    onRestoreDefaultSystemPrompt: () -> Unit,
    onToggleSkill: (skillId: String, enabled: Boolean) -> Unit,
    onInspectSkill: (AiSkillDoc) -> Unit,
    modifier: Modifier = Modifier
) {
    var apiKeyText by remember(aiState.settings.apiKey) { mutableStateOf(aiState.settings.apiKey) }
    var showApiKey by remember { mutableStateOf(false) }
    var customModelDraft by remember { mutableStateOf("") }
    var modelDropdownOpen by remember { mutableStateOf(false) }
    var systemPromptDraft by remember(aiState.settings.systemPrompt) { mutableStateOf(aiState.settings.systemPrompt) }

    val allModels = remember(aiState.settings.customModels) {
        val presets = OPENROUTER_PRESET_MODELS.map { it.id }
        val customOnly = aiState.settings.customModels.filterNot { presets.contains(it) }
        OPENROUTER_PRESET_MODELS + customOnly.map { id ->
            com.beatforge.studio.ai.AiModelOption(id = id, label = id, badge = "Personalizado")
        }
    }

    Row(
        modifier = modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        // Columna izquierda: API Key OpenRouter, Selector de Modelo y System Prompt editable
        Column(
            modifier = Modifier
                .weight(0.52f)
                .fillMaxHeight()
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Surface(
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
                        Text(
                            text = "Proveedor: OpenRouter",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 13.sp
                        )
                        Text(
                            text = "Solo local en tu teléfono",
                            color = StudioColors.AccentEmerald,
                            fontSize = 10.sp,
                            fontWeight = FontWeight.SemiBold
                        )
                    }

                    OutlinedTextField(
                        value = apiKeyText,
                        onValueChange = {
                            apiKeyText = it
                            onUpdateApiKey(it)
                        },
                        label = { Text("OpenRouter API Key (sk-or-...)", fontSize = 11.sp) },
                        singleLine = true,
                        visualTransformation = if (showApiKey) VisualTransformation.None else PasswordVisualTransformation(),
                        trailingIcon = {
                            IconButton(onClick = { showApiKey = !showApiKey }) {
                                Icon(
                                    imageVector = if (showApiKey) Icons.Rounded.VisibilityOff else Icons.Rounded.Visibility,
                                    contentDescription = "Mostrar u ocultar key",
                                    tint = StudioColors.TextMuted
                                )
                            }
                        },
                        textStyle = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 12.sp, color = StudioColors.TextPrimary),
                        modifier = Modifier.fillMaxWidth()
                    )

                    // Selector de modelos con baidu/ernie-5.5-flash-preview:free por defecto
                    Text(
                        text = "Modelo activo",
                        color = StudioColors.TextMuted,
                        fontSize = 11.sp,
                        fontWeight = FontWeight.SemiBold
                    )

                    Box(modifier = Modifier.fillMaxWidth()) {
                        Surface(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { modelDropdownOpen = true },
                            shape = RoundedCornerShape(8.dp),
                            color = StudioColors.BgCard,
                            border = BorderStroke(1.dp, StudioColors.AccentPurple.copy(alpha = 0.65f))
                        ) {
                            Row(
                                modifier = Modifier.padding(horizontal = 10.dp, vertical = 9.dp),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.SpaceBetween
                            ) {
                                Text(
                                    text = aiState.settings.selectedModel.ifBlank { DEFAULT_OPENROUTER_MODEL },
                                    color = StudioColors.TextPrimary,
                                    fontFamily = FontFamily.Monospace,
                                    fontSize = 11.sp,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                    modifier = Modifier.weight(1f)
                                )
                                Icon(Icons.Rounded.ArrowDropDown, null, tint = StudioColors.AccentPurple)
                            }
                        }

                        DropdownMenu(
                            expanded = modelDropdownOpen,
                            onDismissRequest = { modelDropdownOpen = false }
                        ) {
                            allModels.forEach { option ->
                                val isCustom = option.badge == "Personalizado"
                                DropdownMenuItem(
                                    text = {
                                        Row(
                                            verticalAlignment = Alignment.CenterVertically,
                                            horizontalArrangement = Arrangement.spacedBy(6.dp)
                                        ) {
                                            Column(modifier = Modifier.weight(1f)) {
                                                Text(option.label, fontSize = 12.sp, fontWeight = FontWeight.SemiBold)
                                                Text(option.id, fontSize = 10.sp, fontFamily = FontFamily.Monospace, color = StudioColors.TextMuted)
                                            }
                                            if (option.badge.isNotBlank()) {
                                                Text(option.badge, fontSize = 9.sp, color = StudioColors.AccentCyan)
                                            }
                                            if (isCustom) {
                                                IconButton(
                                                    onClick = { onRemoveCustomModel(option.id) },
                                                    modifier = Modifier.size(22.dp)
                                                ) {
                                                    Icon(Icons.Rounded.Close, "Quitar", modifier = Modifier.size(14.dp))
                                                }
                                            }
                                        }
                                    },
                                    onClick = {
                                        modelDropdownOpen = false
                                        onSelectModel(option.id)
                                    }
                                )
                            }
                        }
                    }

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(6.dp),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        OutlinedTextField(
                            value = customModelDraft,
                            onValueChange = { customModelDraft = it },
                            label = { Text("Agregar modelo OpenRouter futuro", fontSize = 10.sp) },
                            singleLine = true,
                            textStyle = TextStyle(fontFamily = FontFamily.Monospace, fontSize = 11.sp, color = StudioColors.TextPrimary),
                            modifier = Modifier.weight(1f)
                        )
                        Button(
                            onClick = {
                                onAddCustomModel(customModelDraft)
                                customModelDraft = ""
                            },
                            enabled = customModelDraft.isNotBlank(),
                            colors = ButtonDefaults.buttonColors(
                                containerColor = StudioColors.AccentCyan,
                                contentColor = Color(0xFF1F0C00)
                            ),
                            contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp),
                            modifier = Modifier.height(40.dp)
                        ) {
                            Icon(Icons.Rounded.Add, null, modifier = Modifier.size(15.dp))
                            Spacer(Modifier.width(3.dp))
                            Text("Usar", fontSize = 11.sp, fontWeight = FontWeight.Bold)
                        }
                    }
                }
            }

            // System Prompt personalizable
            Surface(
                shape = RoundedCornerShape(12.dp),
                color = StudioColors.BgPanel,
                border = BorderStroke(1.dp, StudioColors.BorderSubtle)
            ) {
                Column(
                    modifier = Modifier.padding(12.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text(
                            text = "System Prompt (Editable)",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 13.sp
                        )
                        OutlinedButton(
                            onClick = onRestoreDefaultSystemPrompt,
                            contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp),
                            modifier = Modifier.height(28.dp)
                        ) {
                            Icon(Icons.Rounded.Restore, null, modifier = Modifier.size(13.dp))
                            Spacer(Modifier.width(4.dp))
                            Text("Restaurar", fontSize = 10.sp)
                        }
                    }

                    OutlinedTextField(
                        value = systemPromptDraft,
                        onValueChange = {
                            systemPromptDraft = it
                            onUpdateSystemPrompt(it)
                        },
                        textStyle = TextStyle(fontSize = 11.sp, color = StudioColors.TextPrimary, lineHeight = 15.sp),
                        modifier = Modifier
                            .fillMaxWidth()
                            .heightIn(min = 110.dp, max = 160.dp)
                    )
                }
            }
        }

        // Columna derecha: Skills modulares .md incluidos en el proyecto
        Surface(
            modifier = Modifier
                .weight(0.48f)
                .fillMaxHeight(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Column(
                modifier = Modifier
                    .padding(12.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(8.dp)
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(Icons.AutoMirrored.Rounded.MenuBook, null, tint = StudioColors.AccentEmerald, modifier = Modifier.size(17.dp))
                    Spacer(Modifier.width(6.dp))
                    Text(
                        text = "Skills Modulares (SKILL.md)",
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 13.sp
                    )
                }

                Text(
                    text = "Documentación modular de BeatForge Studio inyectada al asistente. Desactiva los que no necesites para ahorrar tokens.",
                    color = StudioColors.TextMuted,
                    fontSize = 10.sp,
                    lineHeight = 13.sp
                )

                aiState.skills.forEach { skill ->
                    Surface(
                        shape = RoundedCornerShape(10.dp),
                        color = StudioColors.BgCard,
                        border = BorderStroke(
                            1.dp,
                            if (skill.enabled) StudioColors.AccentEmerald.copy(alpha = 0.55f) else StudioColors.BorderSubtle
                        )
                    ) {
                        Column(
                            modifier = Modifier.padding(10.dp),
                            verticalArrangement = Arrangement.spacedBy(5.dp)
                        ) {
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                verticalAlignment = Alignment.CenterVertically,
                                horizontalArrangement = Arrangement.SpaceBetween
                            ) {
                                Column(modifier = Modifier.weight(1f)) {
                                    Text(
                                        text = skill.title,
                                        color = StudioColors.TextPrimary,
                                        fontWeight = FontWeight.Bold,
                                        fontSize = 12.sp
                                    )
                                    Text(
                                        text = "${skill.folderName}/SKILL.md",
                                        color = StudioColors.AccentCyan,
                                        fontFamily = FontFamily.Monospace,
                                        fontSize = 10.sp
                                    )
                                }
                                Switch(
                                    checked = skill.enabled,
                                    onCheckedChange = { checked -> onToggleSkill(skill.id, checked) },
                                    colors = SwitchDefaults.colors(checkedThumbColor = StudioColors.AccentEmerald)
                                )
                            }

                            if (skill.description.isNotBlank()) {
                                Text(
                                    text = skill.description,
                                    color = StudioColors.TextMuted,
                                    fontSize = 10.sp,
                                    lineHeight = 13.sp
                                )
                            }

                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Row(verticalAlignment = Alignment.CenterVertically) {
                                    Icon(
                                        imageVector = Icons.Rounded.CheckCircle,
                                        contentDescription = null,
                                        tint = if (skill.enabled) StudioColors.AccentEmerald else StudioColors.TextMuted,
                                        modifier = Modifier.size(12.dp)
                                    )
                                    Spacer(Modifier.width(4.dp))
                                    Text(
                                        text = if (skill.enabled) "Activo en contexto" else "Solo bajo demanda",
                                        color = StudioColors.TextMuted,
                                        fontSize = 10.sp
                                    )
                                }

                                TextButton(
                                    onClick = { onInspectSkill(skill) },
                                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 2.dp),
                                    modifier = Modifier.height(26.dp)
                                ) {
                                    Text("Ver SKILL.md", fontSize = 10.sp, color = StudioColors.AccentPurple, fontWeight = FontWeight.Bold)
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
