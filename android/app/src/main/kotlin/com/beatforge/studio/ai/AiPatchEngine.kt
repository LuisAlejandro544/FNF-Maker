// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ai/AiPatchEngine.kt
// Propósito: Define el esquema de herramientas OpenRouter (function calling) y ejecuta las
// operaciones locales sobre el proyecto activo: listar/leer scripts por rango de líneas, aplicar
// parches quirúrgicos (`patch_script` sin sobrescribir todo), crear scripts nuevos, inspeccionar
// sprites y cámaras del escenario SIN enviar imágenes, consultar tiempos del chart y leer Skills.

package com.beatforge.studio.ai

import com.beatforge.studio.NativeEngineBridge
import com.beatforge.studio.ProjectSnapshot
import com.beatforge.studio.StudioJsonParser
import org.json.JSONArray
import org.json.JSONObject

data class AiToolExecutionOutput(
    val toolResultJson: String,
    val summary: String,
    val ok: Boolean,
    val modifiedScriptId: String? = null
)

class AiPatchEngine(
    private val getSnapshot: () -> ProjectSnapshot,
    private val getSkills: () -> List<AiSkillDoc>,
    private val onSnapshotMutated: (ProjectSnapshot, selectedScriptId: String?) -> Unit
) {

    fun buildToolsSchema(): JSONArray {
        return JSONArray().apply {
            put(
                toolDef(
                    name = "list_scripts",
                    description = "Lista todos los archivos de script del mod activo (id, nombre, lenguaje, estado y cantidad de líneas).",
                    properties = JSONObject(),
                    required = emptyList()
                )
            )
            put(
                toolDef(
                    name = "read_script",
                    description = "Lee el código fuente de un script específico con números de línea. Usa start_line y end_line para leer solo una sección si es largo.",
                    properties = JSONObject().apply {
                        put("script_id", strProp("ID o nombre del script (ej. 'stage_events.lua' o 'script_stage_events')"))
                        put("start_line", intProp("Línea inicial (1-indexed, opcional)"))
                        put("end_line", intProp("Línea final (1-indexed, opcional)"))
                    },
                    required = listOf("script_id")
                )
            )
            put(
                toolDef(
                    name = "patch_script",
                    description = "Reemplaza una parte específica (old_text -> new_text) dentro de un script existente sin sobrescribir el resto del archivo, guarda en Rust y valida sintaxis.",
                    properties = JSONObject().apply {
                        put("script_id", strProp("ID o nombre del script a modificar"))
                        put("old_text", strProp("Fragmento exacto actual que debe reemplazarse (debe coincidir de forma única en el archivo)"))
                        put("new_text", strProp("Nuevo código que reemplazará exactamente a old_text"))
                    },
                    required = listOf("script_id", "old_text", "new_text")
                )
            )
            put(
                toolDef(
                    name = "create_script",
                    description = "Crea un nuevo archivo de script en el proyecto (lua, ts o js), escribe su código inicial y valida la sintaxis en Rust.",
                    properties = JSONObject().apply {
                        put("language", strProp("Lenguaje: 'lua', 'ts' o 'js'"))
                        put("source_code", strProp("Código inicial completo del nuevo script"))
                    },
                    required = listOf("language", "source_code")
                )
            )
            put(
                toolDef(
                    name = "list_stage_sprites",
                    description = "Devuelve los nombres, IDs, roles, coordenadas, escala, opacidad, visibilidad, animaciones y cámaras del escenario actual SIN imágenes.",
                    properties = JSONObject(),
                    required = emptyList()
                )
            )
            put(
                toolDef(
                    name = "get_chart_timing_info",
                    description = "Devuelve BPM, duración, secciones (index, startMs, bpm, mustHitSection, gfSection) y eventos del chart actual.",
                    properties = JSONObject(),
                    required = emptyList()
                )
            )
            put(
                toolDef(
                    name = "read_skill",
                    description = "Lee el contenido completo de un SKILL.md interno de BeatForge Studio.",
                    properties = JSONObject().apply {
                        put("skill_id", strProp("ID del skill (ej. '01_scripting_api', '02_psych_migration', '03_stage_sprites_camera', '04_chart_events_dev_api')"))
                    },
                    required = listOf("skill_id")
                )
            )
        }
    }

    fun executeTool(name: String, rawArguments: String): AiToolExecutionOutput {
        val args = try {
            if (rawArguments.isBlank()) JSONObject() else JSONObject(rawArguments)
        } catch (e: Exception) {
            return AiToolExecutionOutput(
                toolResultJson = errorJson("Argumentos JSON inválidos: ${e.message}"),
                summary = "Error en argumentos de $name",
                ok = false
            )
        }

        return when (name) {
            "list_scripts" -> handleListScripts()
            "read_script" -> handleReadScript(args)
            "patch_script" -> handlePatchScript(args)
            "create_script" -> handleCreateScript(args)
            "list_stage_sprites" -> handleListStageSprites()
            "get_chart_timing_info" -> handleGetChartTimingInfo()
            "read_skill" -> handleReadSkill(args)
            else -> AiToolExecutionOutput(
                toolResultJson = errorJson("Herramienta desconocida: $name"),
                summary = "Herramienta no soportada: $name",
                ok = false
            )
        }
    }

    private fun handleListScripts(): AiToolExecutionOutput {
        val snap = getSnapshot()
        val arr = JSONArray()
        snap.scripts.forEach { s ->
            val lines = if (s.sourceCode.isEmpty()) 0 else s.sourceCode.split('\n').size
            arr.put(
                JSONObject().apply {
                    put("id", s.id)
                    put("name", s.name)
                    put("language", s.language)
                    put("enabled", s.enabled)
                    put("lineCount", lines)
                }
            )
        }
        val payload = JSONObject().apply {
            put("ok", true)
            put("devApiVersion", snap.devApiVersion)
            put("scripts", arr)
        }.toString()
        return AiToolExecutionOutput(
            toolResultJson = payload,
            summary = "${snap.scripts.size} scripts listados",
            ok = true
        )
    }

    private fun handleReadScript(args: JSONObject): AiToolExecutionOutput {
        val query = args.optString("script_id", "").trim()
        val snap = getSnapshot()
        val script = snap.scripts.find {
            it.id.equals(query, ignoreCase = true) || it.name.equals(query, ignoreCase = true)
        } ?: return AiToolExecutionOutput(
            toolResultJson = errorJson("Script '$query' no encontrado. Usa list_scripts primero."),
            summary = "No existe '$query'",
            ok = false
        )

        val allLines = script.sourceCode.split('\n')
        val startLine = args.optInt("start_line", 1).coerceIn(1, allLines.size.coerceAtLeast(1))
        val endLine = args.optInt("end_line", allLines.size).coerceIn(startLine, allLines.size.coerceAtLeast(1))

        val numbered = buildString {
            for (lineNum in startLine..endLine) {
                append(lineNum).append(": ").append(allLines[lineNum - 1])
                if (lineNum < endLine) append('\n')
            }
        }

        val payload = JSONObject().apply {
            put("ok", true)
            put("id", script.id)
            put("name", script.name)
            put("language", script.language)
            put("enabled", script.enabled)
            put("totalLines", allLines.size)
            put("range", "$startLine-$endLine")
            put("content", numbered)
        }.toString()

        return AiToolExecutionOutput(
            toolResultJson = payload,
            summary = "Leído ${script.name} (lín $startLine-$endLine)",
            ok = true
        )
    }

    private fun handlePatchScript(args: JSONObject): AiToolExecutionOutput {
        val query = args.optString("script_id", "").trim()
        val oldText = args.optString("old_text", "")
        val newText = args.optString("new_text", "")

        if (oldText.isEmpty()) {
            return AiToolExecutionOutput(
                toolResultJson = errorJson("'old_text' no puede estar vacío en patch_script."),
                summary = "Parche inválido: old_text vacío",
                ok = false
            )
        }

        val snap = getSnapshot()
        val script = snap.scripts.find {
            it.id.equals(query, ignoreCase = true) || it.name.equals(query, ignoreCase = true)
        } ?: return AiToolExecutionOutput(
            toolResultJson = errorJson("Script '$query' no encontrado."),
            summary = "No existe '$query'",
            ok = false
        )

        val firstIdx = script.sourceCode.indexOf(oldText)
        if (firstIdx < 0) {
            return AiToolExecutionOutput(
                toolResultJson = errorJson(
                    "No se encontró 'old_text' exactamente en ${script.name}. Llama a read_script para copiar el fragmento exacto."
                ),
                summary = "Fragmento no encontrado en ${script.name}",
                ok = false
            )
        }

        val secondIdx = script.sourceCode.indexOf(oldText, startIndex = firstIdx + oldText.length)
        if (secondIdx >= 0) {
            return AiToolExecutionOutput(
                toolResultJson = errorJson(
                    "El fragmento 'old_text' aparece más de una vez en ${script.name}. Envía un poco más de contexto alrededor para hacerlo único."
                ),
                summary = "Coincidencia múltiple en ${script.name}",
                ok = false
            )
        }

        val updatedSource = buildString(script.sourceCode.length - oldText.length + newText.length) {
            append(script.sourceCode, 0, firstIdx)
            append(newText)
            append(script.sourceCode, firstIdx + oldText.length, script.sourceCode.length)
        }

        val updateRaw = NativeEngineBridge.nativeUpdateScript(script.id, script.enabled, updatedSource)
        val actionRes = StudioJsonParser.parseActionResult(updateRaw)
        val validationRes = StudioJsonParser.parseValidationResult(
            NativeEngineBridge.nativeValidateScript(script.id)
        )
        val refreshedSnap = StudioJsonParser.parseProjectSnapshot(NativeEngineBridge.nativeGetSnapshot())
        onSnapshotMutated(refreshedSnap, script.id)

        val payload = JSONObject().apply {
            put("ok", actionRes.ok)
            put("scriptId", script.id)
            put("scriptName", script.name)
            put("syntaxValid", validationRes.ok)
            put("validationMessage", validationRes.message)
        }.toString()

        val statusNote = if (validationRes.ok) "sintaxis OK" else "aviso sintaxis: ${validationRes.message}"
        return AiToolExecutionOutput(
            toolResultJson = payload,
            summary = "Parche en ${script.name} ($statusNote)",
            ok = actionRes.ok && validationRes.ok,
            modifiedScriptId = script.id
        )
    }

    private fun handleCreateScript(args: JSONObject): AiToolExecutionOutput {
        val rawLang = args.optString("language", "lua").trim().lowercase()
        val lang = when (rawLang) {
            "ts", "typescript" -> "ts"
            "js", "javascript" -> "js"
            else -> "lua"
        }
        val sourceCode = args.optString("source_code", "")
        if (sourceCode.isBlank()) {
            return AiToolExecutionOutput(
                toolResultJson = errorJson("'source_code' no puede estar vacío."),
                summary = "Código vacío al crear script",
                ok = false
            )
        }

        val addRaw = NativeEngineBridge.nativeAddScript(lang)
        val addRes = StudioJsonParser.parseActionResult(addRaw)
        val created = addRes.snapshot?.scripts?.lastOrNull()
            ?: return AiToolExecutionOutput(
                toolResultJson = errorJson("No se pudo crear el script en Rust: ${addRes.message}"),
                summary = "Fallo creando script",
                ok = false
            )

        NativeEngineBridge.nativeUpdateScript(created.id, true, sourceCode)
        val validationRes = StudioJsonParser.parseValidationResult(
            NativeEngineBridge.nativeValidateScript(created.id)
        )
        val refreshedSnap = StudioJsonParser.parseProjectSnapshot(NativeEngineBridge.nativeGetSnapshot())
        onSnapshotMutated(refreshedSnap, created.id)

        val payload = JSONObject().apply {
            put("ok", true)
            put("scriptId", created.id)
            put("scriptName", created.name)
            put("language", created.language)
            put("syntaxValid", validationRes.ok)
            put("validationMessage", validationRes.message)
        }.toString()

        return AiToolExecutionOutput(
            toolResultJson = payload,
            summary = "Creado ${created.name}",
            ok = validationRes.ok,
            modifiedScriptId = created.id
        )
    }

    private fun handleListStageSprites(): AiToolExecutionOutput {
        val snap = getSnapshot()
        val spritesArr = JSONArray()
        snap.stageEntities.forEach { ent ->
            spritesArr.put(
                JSONObject().apply {
                    put("id", ent.id)
                    put("displayName", ent.displayName)
                    put("role", ent.role)
                    put("roleLabel", ent.roleLabel)
                    put("x", ent.x.toDouble())
                    put("y", ent.y.toDouble())
                    put("width", ent.width.toDouble())
                    put("height", ent.height.toDouble())
                    put("scale", ent.scale.toDouble())
                    put("opacity", ent.opacity.toDouble())
                    put("visible", ent.visible)
                    put("flipX", ent.flipX)
                    put("zIndex", ent.zIndex)
                    put("hasAtlas", ent.xmlPath != null || ent.animateJsonPath != null)
                    put("animations", JSONArray(ent.animationPrefixes))
                    put("animBindings", JSONObject(ent.animBindings))
                }
            )
        }

        val camsArr = JSONArray()
        snap.stageCamera.resolvedCameras().forEach { cam ->
            camsArr.put(
                JSONObject().apply {
                    put("slotIndex", cam.index)
                    put("name", cam.name)
                    put("targetKind", cam.targetKind)
                    put("targetEntityId", cam.targetEntityId ?: JSONObject.NULL)
                    put("x", cam.x.toDouble())
                    put("y", cam.y.toDouble())
                    put("zoom", cam.zoom.toDouble())
                }
            )
        }

        val payload = JSONObject().apply {
            put("ok", true)
            put("virtualCanvas", "1280x720")
            put("sprites", spritesArr)
            put("cameras", camsArr)
        }.toString()

        return AiToolExecutionOutput(
            toolResultJson = payload,
            summary = "${snap.stageEntities.size} sprites y ${camsArr.length()} cámaras leídos",
            ok = true
        )
    }

    private fun handleGetChartTimingInfo(): AiToolExecutionOutput {
        val snap = getSnapshot()
        val ch = snap.chart
        val secArr = JSONArray()
        ch.sections.take(160).forEach { s ->
            secArr.put(
                JSONObject().apply {
                    put("index", s.index)
                    put("startMs", s.startMs)
                    put("bpm", s.bpm)
                    put("lengthInSteps", s.lengthInSteps)
                    put("mustHitSection", s.mustHitSection)
                    put("gfSection", s.gfSection)
                    put("cameraSlot", s.cameraSlot ?: JSONObject.NULL)
                }
            )
        }
        val evArr = JSONArray()
        ch.events.take(120).forEach { e ->
            evArr.put(
                JSONObject().apply {
                    put("timeMs", e.timeMs)
                    put("name", e.name)
                    put("value1", e.value1)
                    put("value2", e.value2)
                }
            )
        }
        val payload = JSONObject().apply {
            put("ok", true)
            put("title", ch.title)
            put("bpm", ch.bpm)
            put("speed", ch.speed)
            put("durationMs", ch.durationMs)
            put("sectionCount", ch.sectionCount)
            put("totalNotes", ch.totalNotes)
            put("sections", secArr)
            put("events", evArr)
        }.toString()

        return AiToolExecutionOutput(
            toolResultJson = payload,
            summary = "${ch.title} (${ch.bpm} BPM, ${ch.sectionCount} secciones)",
            ok = true
        )
    }

    private fun handleReadSkill(args: JSONObject): AiToolExecutionOutput {
        val query = args.optString("skill_id", "").trim()
        val skills = getSkills()
        val skill = skills.find {
            it.id.equals(query, ignoreCase = true) ||
                it.folderName.equals(query, ignoreCase = true) ||
                it.title.contains(query, ignoreCase = true)
        } ?: return AiToolExecutionOutput(
            toolResultJson = errorJson(
                "Skill '$query' no encontrado. Disponibles: ${skills.joinToString { it.id }}"
            ),
            summary = "Skill '$query' no encontrado",
            ok = false
        )

        val payload = JSONObject().apply {
            put("ok", true)
            put("id", skill.id)
            put("title", skill.title)
            put("content", skill.content)
        }.toString()

        return AiToolExecutionOutput(
            toolResultJson = payload,
            summary = "Skill ${skill.id} leído",
            ok = true
        )
    }

    private fun toolDef(
        name: String,
        description: String,
        properties: JSONObject,
        required: List<String>
    ): JSONObject {
        return JSONObject().apply {
            put("type", "function")
            put(
                "function",
                JSONObject().apply {
                    put("name", name)
                    put("description", description)
                    put(
                        "parameters",
                        JSONObject().apply {
                            put("type", "object")
                            put("properties", properties)
                            if (required.isNotEmpty()) {
                                put("required", JSONArray(required))
                            }
                        }
                    )
                }
            )
        }
    }

    private fun strProp(desc: String) = JSONObject().apply {
        put("type", "string")
        put("description", desc)
    }

    private fun intProp(desc: String) = JSONObject().apply {
        put("type", "integer")
        put("description", desc)
    }

    private fun errorJson(msg: String): String =
        JSONObject().apply {
            put("ok", false)
            put("error", msg)
        }.toString()
}
