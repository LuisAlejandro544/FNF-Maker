// Archivo: android/app/src/main/kotlin/com/beatforge/studio/StudioModels.kt
// Propósito: Definiciones núcleo de navegación, perfiles de hardware, modos de control móvil,
// tipos de notas especiales, clases de importación SAF y resultados base de acciones JNI.
// El resto de modelos y parsers JSON se organiza modularmente en `com/beatforge/studio/models/`.

package com.beatforge.studio

const val CAMERA_SELECTION_ID = "__camera__"

enum class StudioScreen(val label: String) {
    Hub("Inicio"),
    Projects("Proyectos"),
    Chart("Chart"),
    Audio("Audio"),
    SpriteLab("Sprites"),
    StageEditor("Escena"),
    ScriptStudio("Scripts"),
    AiAssistant("Asistente IA"),
    HotCompile("Compilar"),
    AudioAdapt("Acústica"),
    Gameplay("Juego")
}

enum class DeviceTier(val code: Int, val label: String, val subtitle: String) {
    LowRam32Bit(0, "32-bit / RAM Baja", "Fondos <=1920px, atlas <=2560px y audio streaming"),
    Balanced(1, "Equilibrado HD", "Atlas hasta 4096px y fondos <=2560px en 32/64 bits"),
    HighQuality(2, "Alta Calidad", "Texturas y atlas hasta 4096px, máxima nitidez HD")
}

enum class MobileControlMode(val key: String, val label: String, val description: String) {
    TapArrows(
        key = "tap_arrows",
        label = "Tocar Flechas (Por defecto)",
        description = "4 flechas táctiles fijas abajo (notas cayendo estilo FNF Mobile)"
    ),
    ClassicHitbox(
        key = "classic_hitbox",
        label = "Hitbox Clásico (Psych Mobile)",
        description = "4 columnas táctiles verticales de pantalla completa"
    );

    companion object {
        fun fromKey(raw: String): MobileControlMode {
            return entries.firstOrNull { it.key == raw } ?: TapArrows
        }
    }
}

enum class NoteSpecialType(
    val key: String,
    val code: Int,
    val label: String,
    val shortTag: String
) {
    Normal("default", 0, "Normal", ""),
    Hurt("hurt", 1, "Nota X (Daño)", "X"),
    Instakill("instakill", 2, "Muerte X", "💀"),
    Dodge("dodge", 3, "Esquive", "⚡"),
    AltAnim("alt", 4, "Alt Anim", "A"),
    NoAnim("no_anim", 5, "Sin Anim", "∅");

    companion object {
        fun fromCode(code: Int): NoteSpecialType {
            return entries.firstOrNull { it.code == code } ?: Normal
        }
    }
}

sealed class ImportKind(val tag: String, val mimeTypes: Array<String>) {
    data object Chart : ImportKind("chart", arrayOf("application/json", "text/plain", "*/*"))
    data class AudioTrack(val slot: Int) : ImportKind("audio_$slot", arrayOf("audio/*", "application/ogg", "*/*"))
    data class SpritePng(val entityId: String) : ImportKind("sprite_png", arrayOf("image/png", "image/*", "application/octet-stream", "*/*"))
    data class SparrowXml(val entityId: String) : ImportKind("sparrow_xml", arrayOf("text/xml", "application/xml", "application/json", "text/plain", "*/*"))
    data class CharacterConfig(val entityId: String) : ImportKind("char_cfg", arrayOf("application/json", "text/xml", "application/xml", "text/plain", "*/*"))
    data object StageConfig : ImportKind("stage_cfg", arrayOf("application/json", "text/xml", "application/xml", "text/plain", "*/*"))
    data object Script : ImportKind("script", arrayOf("text/plain", "application/javascript", "*/*"))
    data class ArrowSvg(val direction: Int) : ImportKind("arrow_$direction", arrayOf("image/svg+xml", "*/*"))
    data object ArrowAtlasPng : ImportKind("arrow_atlas_png", arrayOf("image/png", "image/webp", "image/*", "*/*"))
    data object ArrowAtlasXml : ImportKind("arrow_atlas_xml", arrayOf("text/xml", "application/xml", "text/plain", "*/*"))
    data class CharacterIcon(val entityId: String, val isLosing: Boolean) : ImportKind(
        "icon_${entityId}_${if (isLosing) "losing" else "normal"}",
        arrayOf("image/png", "image/*", "*/*")
    )
    data object HealthBarSvg : ImportKind("health_bar_svg", arrayOf("image/svg+xml", "*/*"))
    data class CountdownImage(val slot: Int) : ImportKind("cd_img_$slot", arrayOf("image/png", "image/webp", "image/*", "application/octet-stream", "*/*"))
    data class CountdownXml(val slot: Int) : ImportKind("cd_xml_$slot", arrayOf("text/xml", "application/xml", "text/plain", "*/*"))
    data class CountdownAudio(val slot: Int) : ImportKind("cd_aud_$slot", arrayOf("audio/*", "application/ogg", "*/*"))
    data object ModPackage : ImportKind("mod_pkg", arrayOf("*/*"))
}

data class ProjectSummary(
    val id: String = "",
    val name: String = "",
    val author: String = "",
    val chartTitle: String = "",
    val bpm: Double = 120.0,
    val noteCount: Int = 0,
    val trackCount: Int = 0,
    val entityCount: Int = 0,
    val scriptCount: Int = 0,
    val devApiVersion: Int = 1,
    val currentDevApiVersion: Int = 2,
    val updatedAtMs: Long = 0L,
    val isActive: Boolean = false
)

data class ActionResult(
    val ok: Boolean,
    val message: String,
    val snapshot: ProjectSnapshot?
)

data class ValidationResult(
    val ok: Boolean,
    val message: String
)
