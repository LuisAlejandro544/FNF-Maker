// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ai/AiModels.kt
// Propósito: Modelos de datos para el Asistente IA de Script Studio (configuración de proveedor
// OpenRouter, selector de modelos con `baidu/ernie-5.5-flash-preview:free` por defecto,
// system prompt editable, skills modulares `.md`, historial de chat y llamadas a herramientas).

package com.beatforge.studio.ai

const val DEFAULT_OPENROUTER_MODEL = "baidu/ernie-5.5-flash-preview:free"

val OPENROUTER_PRESET_MODELS: List<AiModelOption> = listOf(
    AiModelOption(
        id = "baidu/ernie-5.5-flash-preview:free",
        label = "ERNIE 5.5 Flash Preview (Free)",
        badge = "Por defecto"
    ),
    AiModelOption(
        id = "qwen/qwen3-coder:free",
        label = "Qwen 3 Coder (Free)",
        badge = "Código"
    ),
    AiModelOption(
        id = "deepseek/deepseek-chat-v3-0324:free",
        label = "DeepSeek V3 0324 (Free)",
        badge = "Razonamiento"
    ),
    AiModelOption(
        id = "google/gemini-2.5-flash",
        label = "Gemini 2.5 Flash",
        badge = "Rápido"
    ),
    AiModelOption(
        id = "anthropic/claude-3.7-sonnet",
        label = "Claude 3.7 Sonnet",
        badge = "Avanzado"
    )
)

const val DEFAULT_AI_SYSTEM_PROMPT = """Eres el Asistente Experto de Scripting e Integración de BeatForge Studio (motor de ritmo móvil en Rust + Lua 5.4 / TypeScript / JavaScript).

Reglas obligatorias de trabajo:
1. Antes de editar o responder sobre un mod, inspecciona el contexto real usando tus herramientas:
   - Usa `list_scripts` y `read_script` para ver qué archivos existen y su código con números de línea.
   - Usa `list_stage_sprites` cuando necesites conocer nombres, IDs, roles, posiciones, visibilidad o animaciones de sprites/personajes y cámaras del escenario (no recibes imágenes, solo sus metadatos reales).
   - Usa `get_chart_timing_info` si el usuario pide activar efectos en cierta sección, beat, BPM o evento del chart.
   - Usa `read_skill` si necesitas consultar algún Skill `.md` de documentación interna.
2. Cuando modifiques un archivo existente, usa SIEMPRE `patch_script` enviando únicamente el fragmento exacto (`old_text` -> `new_text`) en lugar de sobrescribir todo el archivo. Usa `create_script` solo cuando haga falta crear un script nuevo.
3. Nunca elimines ni rompas bloques protegidos `-- [CAM_CFG_START]..-- [CAM_CFG_END]`, `-- [STAGE_CUES_START]..-- [STAGE_CUES_END]` ni `-- [UI_BLACKOUT_START]..-- [UI_BLACKOUT_END]`.
4. Si traduces scripts de Psych Engine u otros motores FNF, adapta cada función a la API oficial de BeatForge Studio (nunca inventes APIs ni asumas `health` en 0..2; en BeatForge `health` va de 0.0 a 1.0).
5. Responde en español, de forma clara, directa y explicando brevemente qué archivos o líneas modificaste."""

data class AiModelOption(
    val id: String,
    val label: String,
    val badge: String = ""
)

data class AiSkillDoc(
    val id: String,
    val folderName: String,
    val title: String,
    val description: String,
    val content: String,
    val enabled: Boolean = true
)

data class AiSettings(
    val provider: String = "openrouter",
    val apiKey: String = "",
    val selectedModel: String = DEFAULT_OPENROUTER_MODEL,
    val customModels: List<String> = emptyList(),
    val systemPrompt: String = DEFAULT_AI_SYSTEM_PROMPT,
    val disabledSkillIds: Set<String> = emptySet()
)

data class AiToolCallRecord(
    val id: String,
    val toolName: String,
    val argumentsSummary: String,
    val resultSummary: String,
    val ok: Boolean
)

data class AiChatMessage(
    val id: String,
    val role: String, // "user" | "assistant"
    val content: String,
    val toolCalls: List<AiToolCallRecord> = emptyList(),
    val isStreaming: Boolean = false,
    val errorMessage: String? = null
)

data class AiAssistantUiState(
    val loaded: Boolean = false,
    val settings: AiSettings = AiSettings(),
    val skills: List<AiSkillDoc> = emptyList(),
    val messages: List<AiChatMessage> = emptyList(),
    val isStreaming: Boolean = false,
    val activeStatus: String = ""
)
