// Archivo: android/app/src/main/kotlin/com/beatforge/studio/models/ChartModels.kt
// Propósito: Snapshots inmutables relacionados con el chart rítmico: notas visibles,
// eventos por step, secciones, configuración de blackout/HUD inicial y resumen del chart.

package com.beatforge.studio

data class ChartEventSnapshot(
    val timeMs: Double = 0.0,
    val name: String = "",
    val value1: String = "",
    val value2: String = ""
)

data class ChartSectionSnapshot(
    val index: Int = 0,
    val startMs: Double = 0.0,
    val lengthInSteps: Int = 16,
    val mustHitSection: Boolean = true,
    val bpm: Double = 128.0,
    val changeBpm: Boolean = false,
    val altAnim: Boolean = false,
    val gfSection: Boolean = false,
    val cameraSlot: Int? = null
)

data class ChartIntroUiSnapshot(
    val startBlackout: Boolean = false,
    val blackoutAlpha: Float = 1.0f,
    val hideHud: Boolean = false,
    val hideArrows: Boolean = false,
    val revealOnSongStart: Boolean = true,
    val revealDelayMs: Double = 0.0,
    val fadeSec: Float = 0.65f
)

data class VisibleNote(
    val id: Long,
    val timeMs: Double,
    val direction: Int,
    val isPlayer: Boolean,
    val sustainMs: Double,
    val headHit: Boolean = false,
    val noteType: String = "default",
    val noteKindCode: Int = 0
)

data class ChartSnapshot(
    val title: String = "Demo Pulse",
    val bpm: Double = 120.0,
    val speed: Double = 1.8,
    val needsVoices: Boolean = true,
    val player1: String = "player_main",
    val player2: String = "opponent_main",
    val gfVersion: String = "stage_speakers",
    val stage: String = "main_stage",
    val sectionCount: Int = 0,
    val totalNotes: Int = 0,
    val playerNotes: Int = 0,
    val opponentNotes: Int = 0,
    val sustainNotes: Int = 0,
    val durationMs: Double = 0.0,
    val densityBuckets: List<Int> = emptyList(),
    val sourcePath: String? = null,
    val introUi: ChartIntroUiSnapshot = ChartIntroUiSnapshot(),
    val sections: List<ChartSectionSnapshot> = emptyList(),
    val notes: List<VisibleNote> = emptyList(),
    val events: List<ChartEventSnapshot> = emptyList()
)
