// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/HubScreen.kt
// Propósito: Pantalla de inicio (Hub) en dos columnas horizontales limpias.
// Permite editar metadatos del mod, sincronización, consultar métricas rápidas y acceder
// a cada editor especializado. La optimización de texturas ahora es automática según el teléfono.

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
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.material.icons.rounded.Memory
import com.beatforge.studio.MobileControlMode
import com.beatforge.studio.ProjectSnapshot
import com.beatforge.studio.StudioScreen
import com.beatforge.studio.gfx.GfxCapabilities
import com.beatforge.studio.gfx.RenderBackendKind
import com.beatforge.studio.ui.theme.StudioColors
import kotlin.math.roundToInt

@Composable
fun HubScreen(
    snapshot: ProjectSnapshot,
    gfxCapabilities: GfxCapabilities = GfxCapabilities(),
    onSelectRenderBackend: (RenderBackendKind) -> Unit = {},
    onUpdateMeta: (
        name: String,
        author: String,
        downscroll: Boolean,
        controlMode: String,
        offsetMs: Double,
        speed: Double
    ) -> Unit,
    onNavigate: (StudioScreen) -> Unit,
    onOpenArrowSkin: () -> Unit,
    onCompileAndPlay: () -> Unit
) {
    var name by remember(snapshot.id) { mutableStateOf(snapshot.name) }
    var author by remember(snapshot.id) { mutableStateOf(snapshot.author) }
    var downscroll by remember(snapshot.id, snapshot.downscroll) { mutableStateOf(snapshot.downscroll) }
    var controlMode by remember(snapshot.id, snapshot.controlMode) { mutableStateOf(snapshot.controlMode) }
    var offsetMs by remember(snapshot.id, snapshot.audioOffsetMs) { mutableDoubleStateOf(snapshot.audioOffsetMs) }
    var scrollSpeed by remember(snapshot.id, snapshot.chart.speed) { mutableDoubleStateOf(snapshot.chart.speed) }

    LaunchedEffect(
        snapshot.name,
        snapshot.author,
        snapshot.downscroll,
        snapshot.controlMode,
        snapshot.audioOffsetMs,
        snapshot.chart.speed
    ) {
        if (name != snapshot.name) name = snapshot.name
        if (author != snapshot.author) author = snapshot.author
        downscroll = snapshot.downscroll
        controlMode = snapshot.controlMode
        offsetMs = snapshot.audioOffsetMs
        scrollSpeed = snapshot.chart.speed
    }

    Row(
        modifier = Modifier
            .fillMaxSize()
            .padding(10.dp),
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        // Columna izquierda: Configuración del proyecto y resumen
        Column(
            modifier = Modifier
                .weight(0.47f)
                .fillMaxHeight()
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Surface(
                shape = RoundedCornerShape(12.dp),
                color = StudioColors.BgPanel,
                border = BorderStroke(1.dp, StudioColors.BorderSubtle)
            ) {
                Column(modifier = Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Icon(
                                imageVector = Icons.Rounded.Tune,
                                contentDescription = null,
                                tint = StudioColors.AccentCyan,
                                modifier = Modifier.size(18.dp)
                            )
                            Spacer(Modifier.width(6.dp))
                            Text(
                                text = "Configuración del Mod",
                                color = StudioColors.TextPrimary,
                                fontWeight = FontWeight.Bold,
                                fontSize = 14.sp
                            )
                        }
                        OutlinedButton(
                            onClick = { onNavigate(StudioScreen.Projects) },
                            shape = RoundedCornerShape(8.dp),
                            contentPadding = PaddingValues(horizontal = 10.dp, vertical = 2.dp),
                            modifier = Modifier.height(28.dp)
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.FolderOpen,
                                contentDescription = null,
                                tint = StudioColors.AccentCyan,
                                modifier = Modifier.size(14.dp)
                            )
                            Spacer(Modifier.width(4.dp))
                            Text("Proyectos", color = StudioColors.TextPrimary, fontSize = 11.sp)
                        }
                    }

                    Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        OutlinedTextField(
                            value = name,
                            onValueChange = {
                                name = it
                                onUpdateMeta(it, author, downscroll, controlMode, offsetMs, scrollSpeed)
                            },
                            label = { Text("Nombre del Mod", fontSize = 11.sp) },
                            singleLine = true,
                            modifier = Modifier.weight(1f)
                        )
                        OutlinedTextField(
                            value = author,
                            onValueChange = {
                                author = it
                                onUpdateMeta(name, it, downscroll, controlMode, offsetMs, scrollSpeed)
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
                            Text("Las flechas caen de arriba hacia abajo", color = StudioColors.TextMuted, fontSize = 11.sp)
                        }
                        Switch(
                            checked = downscroll,
                            onCheckedChange = {
                                downscroll = it
                                onUpdateMeta(name, author, it, controlMode, offsetMs, scrollSpeed)
                            },
                            colors = SwitchDefaults.colors(checkedThumbColor = StudioColors.AccentCyan)
                        )
                    }

                    Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                        Text(
                            text = "Controles Táctiles en Partida",
                            color = StudioColors.TextPrimary,
                            fontSize = 13.sp,
                            fontWeight = FontWeight.SemiBold
                        )
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            MobileControlMode.entries.forEach { mode ->
                                val selected = controlMode == mode.key
                                Surface(
                                    modifier = Modifier
                                        .weight(1f)
                                        .clickable {
                                            controlMode = mode.key
                                            onUpdateMeta(name, author, downscroll, mode.key, offsetMs, scrollSpeed)
                                        },
                                    shape = RoundedCornerShape(10.dp),
                                    color = if (selected) StudioColors.AccentCyan.copy(alpha = 0.14f) else StudioColors.BgCard,
                                    border = BorderStroke(
                                        1.dp,
                                        if (selected) StudioColors.AccentCyan else StudioColors.BorderSubtle
                                    )
                                ) {
                                    Column(modifier = Modifier.padding(8.dp)) {
                                        Text(
                                            text = mode.label,
                                            color = if (selected) StudioColors.AccentCyan else StudioColors.TextPrimary,
                                            fontSize = 11.sp,
                                            fontWeight = FontWeight.Bold
                                        )
                                        Spacer(Modifier.height(2.dp))
                                        Text(
                                            text = mode.description,
                                            color = StudioColors.TextMuted,
                                            fontSize = 10.sp,
                                            lineHeight = 13.sp
                                        )
                                    }
                                }
                            }
                        }
                    }

                    Column {
                        Row(
                            modifier = Modifier.fillMaxWidth(),
                            horizontalArrangement = Arrangement.SpaceBetween
                        ) {
                            Text("Velocidad de Notas", color = StudioColors.TextMuted, fontSize = 12.sp)
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
                                onUpdateMeta(name, author, downscroll, controlMode, offsetMs, scrollSpeed)
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
                                onUpdateMeta(name, author, downscroll, controlMode, offsetMs, scrollSpeed)
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
                    StatBadge("BPM", snapshot.chart.bpm.roundToInt().toString(), StudioColors.AccentAmber)
                    StatBadge("Pistas", "$activeTracks/3", StudioColors.AccentEmerald)
                    StatBadge("Capas", snapshot.stageEntities.size.toString(), StudioColors.AccentIndigo)
                    StatBadge("Scripts", "$enabledScripts/${snapshot.scripts.size}", StudioColors.AccentPurple)
                }
            }

            // Terreno preparado para backends OpenGL ES 2.0 / 3.x y Vulkan
            Surface(
                shape = RoundedCornerShape(12.dp),
                color = StudioColors.BgPanel,
                border = BorderStroke(1.dp, StudioColors.BorderSubtle)
            ) {
                val activeBackend = RenderBackendKind.fromKey(snapshot.preferredRenderBackend)
                Column(
                    modifier = Modifier.padding(10.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp)
                ) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Icon(
                                imageVector = Icons.Rounded.Memory,
                                contentDescription = null,
                                tint = StudioColors.AccentAmber,
                                modifier = Modifier.size(16.dp)
                            )
                            Spacer(Modifier.width(6.dp))
                            Text(
                                text = "Rendimiento Gráfico",
                                color = StudioColors.TextPrimary,
                                fontWeight = FontWeight.Bold,
                                fontSize = 12.sp
                            )
                        }
                        Text(
                            text = if (gfxCapabilities.vulkanSupported) "Gráficos avanzados listos" else gfxCapabilities.glesVersionLabel,
                            color = StudioColors.AccentEmerald,
                            fontSize = 10.sp,
                            fontWeight = FontWeight.SemiBold
                        )
                    }

                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(6.dp)
                    ) {
                        RenderBackendKind.entries.forEach { kind ->
                            val selected = activeBackend == kind
                            val supported = gfxCapabilities.supports(kind)
                            Surface(
                                modifier = Modifier
                                    .weight(1f)
                                    .clickable { onSelectRenderBackend(kind) },
                                shape = RoundedCornerShape(8.dp),
                                color = if (selected) {
                                    StudioColors.AccentAmber.copy(alpha = 0.18f)
                                } else {
                                    StudioColors.BgCard
                                },
                                border = BorderStroke(
                                    1.dp,
                                    if (selected) StudioColors.AccentAmber else StudioColors.BorderSubtle
                                )
                            ) {
                                Column(
                                    modifier = Modifier.padding(horizontal = 6.dp, vertical = 6.dp),
                                    horizontalAlignment = Alignment.CenterHorizontally
                                ) {
                                    Text(
                                        text = kind.shortBadge,
                                        color = if (selected) StudioColors.AccentAmber else StudioColors.TextPrimary,
                                        fontWeight = FontWeight.Bold,
                                        fontSize = 10.sp,
                                        maxLines = 1
                                    )
                                    Text(
                                        text = if (kind == RenderBackendKind.Canvas2d) {
                                            "Activo"
                                        } else if (supported) {
                                            "Compatible"
                                        } else {
                                            "No disp."
                                        },
                                        color = if (supported) StudioColors.AccentEmerald else StudioColors.TextMuted,
                                        fontSize = 9.sp,
                                        maxLines = 1
                                    )
                                }
                            }
                        }
                    }

                    Text(
                        text = "${activeBackend.subtitle} · ${gfxCapabilities.detail}",
                        color = StudioColors.TextMuted,
                        fontSize = 10.sp,
                        lineHeight = 13.sp
                    )
                }
            }
        }

        // Columna derecha: Accesos directos a cada apartado + botones de acción
        Column(
            modifier = Modifier
                .weight(0.53f)
                .fillMaxHeight()
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ModuleShortcutCard(
                    title = "Editor de Chart",
                    subtitle = "Chartea manualmente paso a paso o importa archivos .json",
                    icon = Icons.Rounded.Tune,
                    accent = StudioColors.AccentCyan,
                    onClick = { onNavigate(StudioScreen.Chart) },
                    modifier = Modifier.weight(1f)
                )
                ModuleShortcutCard(
                    title = "Mezclador Audio",
                    subtitle = "Hasta 3 pistas (.ogg/.wav/.mp3) y prueba en vivo",
                    icon = Icons.Rounded.GraphicEq,
                    accent = StudioColors.AccentEmerald,
                    onClick = { onNavigate(StudioScreen.Audio) },
                    modifier = Modifier.weight(1f)
                )
            }

            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ModuleShortcutCard(
                    title = "Sprite Lab",
                    subtitle = "Hojas .png + XML Sparrow automático a 24 FPS",
                    icon = Icons.Rounded.Animation,
                    accent = StudioColors.AccentAmber,
                    onClick = { onNavigate(StudioScreen.SpriteLab) },
                    modifier = Modifier.weight(1f)
                )
                ModuleShortcutCard(
                    title = "Escenario & Capas",
                    subtitle = "Capas Z-order, arrastre táctil y cruceta de precisión",
                    icon = Icons.Rounded.Layers,
                    accent = StudioColors.AccentIndigo,
                    onClick = { onNavigate(StudioScreen.StageEditor) },
                    modifier = Modifier.weight(1f)
                )
            }

            ModuleShortcutCard(
                title = "Script Studio (Lua / TypeScript / JavaScript)",
                subtitle = "Eventos onBeatHit, cámaras y animaciones personalizadas para tu mod",
                icon = Icons.Rounded.Code,
                accent = StudioColors.AccentPurple,
                onClick = { onNavigate(StudioScreen.ScriptStudio) },
                modifier = Modifier.fillMaxWidth()
            )

            Spacer(Modifier.weight(1f, fill = false))

            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
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
                    Text("Skin de Flechas", color = StudioColors.TextPrimary, fontSize = 13.sp, maxLines = 1, softWrap = false)
                }

                Button(
                    onClick = onCompileAndPlay,
                    shape = RoundedCornerShape(10.dp),
                    contentPadding = PaddingValues(vertical = 12.dp, horizontal = 14.dp),
                    colors = ButtonDefaults.buttonColors(
                        containerColor = StudioColors.AccentEmerald,
                        contentColor = Color(0xFF042117)
                    ),
                    modifier = Modifier.weight(1.1f)
                ) {
                    Icon(
                        imageVector = Icons.Rounded.PlayArrow,
                        contentDescription = null,
                        modifier = Modifier.size(20.dp)
                    )
                    Spacer(Modifier.width(6.dp))
                    Text("Jugar Mod", fontSize = 13.sp, fontWeight = FontWeight.Bold, maxLines = 1, softWrap = false)
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
        border = BorderStroke(1.dp, accent.copy(alpha = 0.4f))
    ) {
        Column(modifier = Modifier.padding(11.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    imageVector = icon,
                    contentDescription = title,
                    tint = accent,
                    modifier = Modifier.size(19.dp)
                )
                Spacer(Modifier.width(7.dp))
                Text(
                    text = title,
                    color = StudioColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 13.sp,
                    maxLines = 1
                )
            }
            Spacer(Modifier.height(4.dp))
            Text(
                text = subtitle,
                color = StudioColors.TextMuted,
                fontSize = 11.sp,
                lineHeight = 14.sp
            )
        }
    }
}
