// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ai/AiPreferencesStore.kt
// Propósito: Persistencia local segura en `SharedPreferences` de la configuración del Asistente IA
// (API Key de OpenRouter únicamente en almacenamiento privado del dispositivo, modelo seleccionado,
// lista de modelos personalizados, System Prompt editable y Skills activados).

package com.beatforge.studio.ai

import android.content.Context
import org.json.JSONArray

class AiPreferencesStore(context: Context) {

    private val prefs = context.getSharedPreferences("beatforge_ai_prefs", Context.MODE_PRIVATE)

    fun load(): AiSettings {
        val apiKey = prefs.getString(KEY_API_KEY, "") ?: ""
        val selectedModel = prefs.getString(KEY_SELECTED_MODEL, DEFAULT_OPENROUTER_MODEL)
            ?.takeIf { it.isNotBlank() } ?: DEFAULT_OPENROUTER_MODEL
        val systemPrompt = prefs.getString(KEY_SYSTEM_PROMPT, null)
            ?.takeIf { it.isNotBlank() } ?: DEFAULT_AI_SYSTEM_PROMPT

        val customModels = mutableListOf<String>()
        val rawCustom = prefs.getString(KEY_CUSTOM_MODELS, "[]") ?: "[]"
        try {
            val arr = JSONArray(rawCustom)
            for (i in 0 until arr.length()) {
                val id = arr.optString(i, "").trim()
                if (id.isNotBlank() && !customModels.contains(id)) {
                    customModels.add(id)
                }
            }
        } catch (_: Exception) {
        }

        val disabledSkills = prefs.getStringSet(KEY_DISABLED_SKILLS, emptySet())?.toSet() ?: emptySet()

        return AiSettings(
            provider = "openrouter",
            apiKey = apiKey,
            selectedModel = selectedModel,
            customModels = customModels,
            systemPrompt = systemPrompt,
            disabledSkillIds = disabledSkills
        )
    }

    fun save(settings: AiSettings) {
        val customArr = JSONArray()
        settings.customModels.forEach { customArr.put(it) }
        prefs.edit()
            .putString(KEY_API_KEY, settings.apiKey.trim())
            .putString(
                KEY_SELECTED_MODEL,
                settings.selectedModel.trim().ifBlank { DEFAULT_OPENROUTER_MODEL }
            )
            .putString(KEY_CUSTOM_MODELS, customArr.toString())
            .putString(
                KEY_SYSTEM_PROMPT,
                settings.systemPrompt.ifBlank { DEFAULT_AI_SYSTEM_PROMPT }
            )
            .putStringSet(KEY_DISABLED_SKILLS, settings.disabledSkillIds)
            .apply()
    }

    companion object {
        private const val KEY_API_KEY = "openrouter_api_key"
        private const val KEY_SELECTED_MODEL = "openrouter_selected_model"
        private const val KEY_CUSTOM_MODELS = "openrouter_custom_models"
        private const val KEY_SYSTEM_PROMPT = "ai_system_prompt"
        private const val KEY_DISABLED_SKILLS = "ai_disabled_skills"
    }
}
