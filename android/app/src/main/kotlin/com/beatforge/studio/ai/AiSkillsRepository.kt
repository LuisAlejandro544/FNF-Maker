// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ai/AiSkillsRepository.kt
// Propósito: Carga modular de los documentos `SKILL.md` empaquetados en `assets/` desde
// `skills/beatforge_ai/`, extrayendo su nombre, descripción YAML frontmatter y contenido
// para inyectarlos al Asistente IA y consultarlos bajo demanda con `read_skill`.

package com.beatforge.studio.ai

import android.content.Context

class AiSkillsRepository(private val context: Context) {

    fun loadBundledSkills(disabledIds: Set<String> = emptySet()): List<AiSkillDoc> {
        val am = context.assets
        val discoveredDirs = mutableListOf<String>()

        try {
            val rootEntries = am.list("") ?: emptyArray()
            rootEntries.sorted().forEach { entry ->
                val subEntries = try {
                    am.list(entry) ?: emptyArray()
                } catch (_: Exception) {
                    emptyArray()
                }
                if (subEntries.contains("SKILL.md")) {
                    discoveredDirs.add(entry)
                }
            }
        } catch (_: Exception) {
        }

        // Respaldo explícito en caso de compilación sin indexado raíz
        val fallbackDirs = listOf(
            "01_scripting_api",
            "02_psych_migration",
            "03_stage_sprites_camera",
            "04_chart_events_dev_api"
        )
        val dirs = if (discoveredDirs.isNotEmpty()) discoveredDirs else fallbackDirs

        return dirs.mapNotNull { folder ->
            val assetPath = "$folder/SKILL.md"
            try {
                val raw = am.open(assetPath).bufferedReader(Charsets.UTF_8).use { it.readText() }
                parseSkillMarkdown(folder, raw, enabled = !disabledIds.contains(folder))
            } catch (_: Exception) {
                null
            }
        }
    }

    private fun parseSkillMarkdown(folder: String, raw: String, enabled: Boolean): AiSkillDoc {
        var name = folder
        var description = ""
        var body = raw

        if (raw.startsWith("---")) {
            val endIdx = raw.indexOf("\n---", startIndex = 3)
            if (endIdx > 0) {
                val frontmatter = raw.substring(3, endIdx)
                body = raw.substring(endIdx + 4).trimStart()
                frontmatter.lines().forEach { line ->
                    val trimmed = line.trim()
                    when {
                        trimmed.startsWith("name:") -> {
                            name = trimmed.removePrefix("name:").trim().trim('"', '\'')
                        }
                        trimmed.startsWith("description:") -> {
                            description = trimmed.removePrefix("description:").trim().trim('"', '\'')
                        }
                    }
                }
            }
        }

        val headingTitle = body.lines()
            .firstOrNull { it.trim().startsWith("# ") }
            ?.trim()
            ?.removePrefix("# ")
            ?.trim()
            ?: name

        return AiSkillDoc(
            id = folder,
            folderName = folder,
            title = headingTitle,
            description = description,
            content = body,
            enabled = enabled
        )
    }
}
