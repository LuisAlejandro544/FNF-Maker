// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/HubScreen.kt
// Propósito: Pantalla principal (Hub del Mod) en distribución horizontal de dos columnas.
// Permite editar metadatos del proyecto (nombre, autor, scroll, downscroll, offset de audio),
// inspeccionar estadísticas rápidas del mod y acceder a los apartados modulares especializados.

package com.beatforge.studio.ui.screens

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Animation
import androidx.compose.material.icons.rounded.Code
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.Memory
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Speed
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.RadioButtonDefaults
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableDoubleStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.DeviceTier
import com.beatforge.studio.ProjectSnapshot
import com.beatforge.studio.StudioScreen
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

@Composable
fun HubScreen(
    snapshot: ProjectSnapshot,
    selectedTier: DeviceTier,
    onSelectTier: (DeviceTier) -> Unit,
    onUpdateMeta: (name: String, author: String, downscroll: Boolean, offsetMs: Double, speed: Double) -> Unit,
    onNavigate: (StudioScreen) -> Unit,
    onOpenArrowSkin: () -> Unit,
    onCompileAndPlay: () -> Unit
) {
    var name by remember(snapshot.id) { mutableStateOf(snapshot.name) }
    var author by remember(snapshot.id) { mutableStateOf(snapshot.author) }
    var downscroll by remember(snapshot.id, snapshot.downscroll) { mutableStateOf(snapshot.downscroll) }
    var offsetMs by remember(snapshot.id, snapshot.audioOffsetMs) { mutableDoubleStateOf(snapshot.audioOffsetMs) }
    var scrollSpeed by remember(snapshot.id, snapshot.chart.speed) { mutableDoubleStateOf(snapshot.chart.speed) }

    LaunchedEffect(snapshot.name, snapshot.author, snapshot.downscroll, snapshot.audioOffsetMs, snapshot.chart.speed) {
        if (name != snapshot.name) name = snapshot.name
        if (author != snapshot.author) author = snapshot.author
        downscroll = snapshot.downscroll
        offsetMs = snapshot.audioOffsetMs
        scrollSpeed = snapshot.chart.speed
    }

    Row(
        modifier = Modifier
            .fillMaxSize()
            .padding(12.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // Columna izquierda: Metadatos, estadísticas y último reporte de compilación
        Column(
            modifier = Modifier
                .weight(0.46f)
                .fillMaxHeight()
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Surface(
                shape = RoundedCornerShape(12.dp),
                color = StudioColors.BgPanel,
                border = BorderStroke(1.dp, StudioColors.BorderSubtle)
            ) {
                Column(modifier = Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(
                            imageVector = Icons.Rounded.Tune,
                            contentDescription = null,
                            tint = StudioColors.AccentCyan,
                            modifier = Modifier.size(18.dp)
                        )
                        Spacer(Modifier.width(6.dp))
                        Text(
                            text = "Proyecto & Sincronización",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 14.sp
                        )
                    }

                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedTextField(
                            value = name,
                            onValueChange = {
                                name = it
                                onUpdateMeta(it, author, downscroll, offsetMs, scrollSpeed)
                            },
                            label = { Text("Nombre del Mod", fontSize = 11.sp) },
                            singleLine = true,
                            modifier = Modifier.weight(1f)
                        )
                        OutlinedTextField(
                            value = author,
                            onValueChange = {
                                author = it
                                onUpdateMeta(name, it, downscroll, offsetMs, scrollSpeed)
                            },
                            label = { Text("Autor", fontSize = 11.sp) },
                            singleLine = true,
                            modifier = Modifier.weight(0.85f)
                        )
                    }

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Column {
                            Text("Modo Downscroll", color = StudioColors.TextPrimary, fontSize = 13.sp, fontWeight = FontWeight.SemiBold)
                            Text("Las notas caen de arriba hacia abajo", color = StudioColors.TextMuted, fontSize = 11.sp)
                        }
                        Switch(
                            checked = downscroll,
                            onCheckedChange = {
                                downscroll = it
                                onUpdateMeta(name, author, it, offsetMs, scrollSpeed)
                            },
                            colors = SwitchDefaults.colors(checkedThumbColor = StudioColors.AccentCyan)
                        )
                    }

                    Column {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween
                        ) {
                            Text("Velocidad de Scroll", color = StudioColors.TextMuted, fontSize = 12.sp)
                            Text(
                                "${((scrollSpeed * 10.0).roundToInt() / 10.0)}x",
                                color = StudioColors.AccentCyan,
                                fontSize = 12.sp,
                                fontWeight = FontWeight.Bold
                            )
                        }
                        Slider(
                            value = scrollSpeed.toFloat(),
                            onValueChange = { scrollSpeed = it.toDouble() },
                            onValueChangeFinished = {
                                onUpdateMeta(name, author, downscroll, offsetMs, scrollSpeed)
                            },
                            valueRange = 0.8f..4.2f,
                            colors = SliderDefaults.colors(
                                thumbColor = StudioColors.AccentCyan,
                                activeTrackColor = StudioColors.AccentCyan
                            )
                        )
                    }

                    Column {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween
                        ) {
                            Text("Offset de Audio (latencia)", color = StudioColors.TextMuted, fontSize = 12.sp)
                            Text(
                                "${offsetMs.roundToInt()} ms",
                                color = StudioColors.AccentEmerald,
                                fontSize = 12.sp,
                                fontWeight = FontWeight.Bold
                            )
                        }
                        Slider(
                            value = offsetMs.toFloat(),
                            onValueChange = { offsetMs = it.toDouble() },
                            onValueChangeFinished = {
                                onUpdateMeta(name, author, downscroll, offsetMs, scrollSpeed)
                            },
                            valueRange = -250f..250f,
                            colors = SliderDefaults.colors(
                                thumbColor = StudioColors.AccentEmerald,
                                activeTrackColor = StudioColors.AccentEmerald
                            )
                        )
                    }
                }
            }

            // Tarjeta de métricas rápidas del mod
            val activeTracks = snapshot.audioTracks.count { it.fileName != null }
            val enabledScripts = snapshot.scripts.count { it.enabled }
            Surface(
                shape = RoundedCornerShape(12.dp),
                color = StudioColors.BgPanel,
                border = BorderStroke(1.dp, StudioColors.BorderSubtle)
            ) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(10.dp),
                    horizontalArrangement = Arrangement.SpaceEvenly
                ) {
                    StatBadge("Notas", snapshot.chart.totalNotes.toString(), StudioColors.AccentCyan)
                    StatBadge("BPM", snapshot.chart.bpm.roundToInt().toString(), StudioColors.AccentEmerald)
                    StatBadge("Pistas", "$activeTracks/3", StudioColors.AccentAmber)
                    StatBadge("Capas", snapshot.stageEntities.size.toString(), StudioColors.AccentIndigo)
                    StatBadge("Scripts", "$enabledScripts/${snapshot.scripts.size}", StudioColors.AccentPurple)
                }
            }

            snapshot.lastCompileReport?.let { rep ->
                Surface(
                    shape = RoundedCornerShape(12.dp),
                    color = StudioColors.BgCard,
                    border = BorderStroke(1.dp, StudioColors.AccentEmerald.copy(alpha = 0.45f))
                ) {
                    Column(modifier = Modifier.padding(10.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Icon(
                                imageVector = Icons.Rounded.Speed,
                                contentDescription = null,
                                tint = StudioColors.AccentEmerald,
                                modifier = Modifier.size(16.dp)
                            )
                            Spacer(Modifier.width(6.dp))
                            Text(
                                text = "Última Compilación (${rep.elapsedMs} ms)",
                                color = StudioColors.AccentEmerald,
                                fontWeight = FontWeight.Bold,
                                fontSize = 12.sp
                            )
                        }
                        Text(
                            text = "Activos: ${rep.keptEntities} | Descartados: ${rep.discardedInvisibleEntities.size} | Escalados: ${rep.downscaledTextures} | VRAM ahorrada: ${rep.estimatedVramSavedKb} KB",
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp
                        )
                    }
                }
            }
        }

        // Columna derecha: Accesos a módulos, perfil de dispositivo y CTAs
        Column(
            modifier = Modifier
                .weight(0.54f)
                .fillMaxHeight()
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                ModuleShortcutCard(
                    title = "Chart & Audio",
                    subtitle = "JSON, hasta 3 pistas .ogg/.wav/.mp3 e histograma",
                    icon = Icons.Rounded.LibraryMusic,
                    accent = StudioColors.AccentCyan,
                    onClick = { onNavigate(StudioScreen.ChartAudio) },
                    modifier = Modifier.weight(1f)
                )
                ModuleShortcutCard(
                    title = "Sprite Lab",
                    subtitle = "Spritesheet PNG + XML Sparrow automático a 24 FPS",
                    icon = Icons.Rounded.Animation,
                    accent = StudioColors.AccentAmber,
                    onClick = { onNavigate(StudioScreen.SpriteLab) },
                    modifier = Modifier.weight(1f)
                )
            }

            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                ModuleShortcutCard(
                    title = "Escenario & Capas",
                    subtitle = "Editor Z-order, arrastre táctil y cruceta de precisión",
                    icon = Icons.Rounded.Layers,
                    accent = StudioColors.AccentIndigo,
                    onClick = { onNavigate(StudioScreen.StageEditor) },
                    modifier = Modifier.weight(1f)
                )
                ModuleShortcutCard(
                    title = "Script Studio",
                    subtitle = "Lua 5.4, TypeScript (oxc) y JavaScript (QuickJS)",
                    icon = Icons.Rounded.Code,
                    accent = StudioColors.AccentPurple,
                    onClick = { onNavigate(StudioScreen.ScriptStudio) },
                    modifier = Modifier.weight(1f)
                )
            }

            // Selector de perfil de recursos para compilación en caliente
            Surface(
                shape = RoundedCornerShape(12.dp),
                color = StudioColors.BgPanel,
                border = BorderStroke(1.dp, StudioColors.BorderSubtle)
            ) {
                Column(modifier = Modifier.padding(10.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(
                            imageVector = Icons.Rounded.Memory,
                            contentDescription = null,
                            tint = StudioColors.AccentIndigo,
                            modifier = Modifier.size(16.dp)
                        )
                        Spacer(Modifier.width(6.dp))
                        Text(
                            text = "Perfil de Recursos (Compilación Paralela Rayon)",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 13.sp
                        )
                    }

                    DeviceTier.entries.forEach { tier ->
                        val selected = tier == selectedTier
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onSelectTier(tier) }
                                .padding(vertical = 2.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            RadioButton(
                                selected = selected,
                                onClick = { onSelectTier(tier) },
                                colors = RadioButtonDefaults.colors(selectedColor = StudioColors.AccentCyan),
                                modifier = Modifier.size(26.dp)
                            )
                            Spacer(Modifier.width(6.dp))
                            Text(
                                text = tier.label,
                                color = if (selected) StudioColors.AccentCyan else StudioColors.TextPrimary,
                                fontWeight = FontWeight.SemiBold,
                                fontSize = 12.sp
                            )
                            Spacer(Modifier.width(8.dp))
                            Text(
                                text = tier.subtitle,
                                color = StudioColors.TextMuted,
                                fontSize = 11.sp
                            )
                        }
                    }
                }
            }

            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                OutlinedButton(
                    onClick = onOpenArrowSkin,
                    shape = RoundedCornerShape(10.dp),
                    contentPadding = PaddingValues(vertical = 12.dp, horizontal = 12.dp),
                    modifier = Modifier.weight(0.9f)
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Palette,
                        contentDescription = null,
                        tint = StudioColors.AccentPurple,
                        modifier = Modifier.size(18.dp)
                    )
                    Spacer(Modifier.width(6.dp))
                    Text("Cambiar Skin SVG", color = StudioColors.TextPrimary, fontSize = 13.sp)
                }

                Button(
                    onClick = onCompileAndPlay,
                    shape = RoundedCornerShape(10.dp),
                    contentPadding = PaddingValues(vertical = 12.dp, horizontal = 14.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = StudioColors.AccentEmerald,
                        contentColor = Color(0xFF042419)
                    ),
                    modifier = Modifier.weight(1.1f)
                ) {
                    Icon(
                        imageVector = Icons.Rounded.PlayArrow,
                        contentDescription = null,
                        modifier = Modifier.size(20.dp)
                    )
                    Spacer(Modifier.width(6.dp))
                    Text("Compilar y Jugar", fontSize = 13.sp, fontWeight = FontWeight.Bold)
                }
            }
        }
    }
}

@Composable
private fun StatBadge(label: String, value: String, color: Color) {
    Column(horizontalAlignment = Alignment.CenterHorizontally) {
        Text(text = value, color = color, fontWeight = FontWeight.Bold, fontSize = 15.sp)
        Text(text = label, color = StudioColors.TextMuted, fontSize = 11.sp)
    }
}

@Composable
private fun ModuleShortcutCard(
    title: String,
    subtitle: String,
    icon: ImageVector,
    accent: Color,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    Surface(
        modifier = modifier.clickable(onClick = onClick),
        shape = RoundedCornerShape(12.dp),
        color = StudioColors.BgCard,
        border = BorderStroke(1.dp, accent.copy(alpha = 0.45f))
    ) {
        Column(modifier = Modifier.padding(11.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    imageVector = icon,
                    contentDescription = title,
                    tint = accent,
                    modifier = Modifier.size(20.dp)
                )
                Spacer(Modifier.width(7.dp))
                Text(
                    text = title,
                    color = StudioColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 13.sp
                )
            }
            Spacer(Modifier.height(5.dp))
            Text(
                text = subtitle,
                color = StudioColors.TextMuted,
                fontSize = 11.sp,
                lineHeight = 14.sp
            )
        }
    }
}
