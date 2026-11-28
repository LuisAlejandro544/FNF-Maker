// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/ProjectsScreen.kt
// Propósito: Menú dedicado de proyectos guardados en almacenamiento interno (`filesDir`).
// Permite ver los mods recientes con su fecha de última modificación, crear un proyecto
// en blanco, importar o exportar paquetes portátiles Vanilla (`.mod` + Zstd), abrir
// cualquier mod guardado, duplicarlo o eliminarlo con confirmación.

package com.beatforge.studio.ui.screens

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
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
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.Schedule
import androidx.compose.material.icons.rounded.UploadFile
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.material.icons.rounded.Memory
import com.beatforge.studio.ProjectSummary
import com.beatforge.studio.gfx.GfxCapabilities
import com.beatforge.studio.gfx.RenderBackendKind
import com.beatforge.studio.ui.theme.StudioColors
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.math.roundToInt

@Composable
fun ProjectsScreen(
    projects: List<ProjectSummary>,
    activeProjectName: String,
    isLobbyMode: Boolean = false,
    gfxCapabilities: GfxCapabilities = GfxCapabilities(),
    activeRenderBackend: String = "canvas2d",
    onReturnToLobby: (() -> Unit)? = null,
    onCreateProject: (name: String, author: String) -> Unit,
    onImportModPackage: () -> Unit,
    onOpenProject: (projectId: String) -> Unit,
    onExportProject: (projectId: String, projectName: String) -> Unit,
    onDuplicateProject: (projectId: String) -> Unit,
    onDeleteProject: (projectId: String) -> Unit
) {
    var createDialogOpen by remember { mutableStateOf(false) }
    var deleteCandidate by remember { mutableStateOf<ProjectSummary?>(null) }

    val dateFormatter = remember {
        SimpleDateFormat("dd/MM/yyyy HH:mm", Locale.getDefault())
    }
    val backendKind = RenderBackendKind.fromKey(activeRenderBackend)

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(12.dp),
        verticalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        // Cabecera superior con modo Menú Principal o Gestor de Proyectos del Estudio
        Surface(
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(
                1.dp,
                if (isLobbyMode) StudioColors.AccentCyan.copy(alpha = 0.45f) else StudioColors.BorderSubtle
            )
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 14.dp, vertical = 10.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    modifier = Modifier.weight(1f)
                ) {
                    Icon(
                        imageVector = Icons.Rounded.FolderOpen,
                        contentDescription = null,
                        tint = StudioColors.AccentCyan,
                        modifier = Modifier.size(22.dp)
                    )
                    Spacer(Modifier.width(10.dp))
                    Column {
                        Text(
                            text = if (isLobbyMode) {
                                "BeatForge Studio · Menú Principal (${projects.size})"
                            } else {
                                "Proyectos Guardados (${projects.size})"
                            },
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 15.sp
                        )
                        Text(
                            text = if (isLobbyMode) {
                                "Elige un proyecto para cargarlo y empezar a editar tu mod"
                            } else {
                                "Activo: $activeProjectName · Guardado instantáneo en memoria interna"
                            },
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis
                        )
                    }
                }

                Spacer(Modifier.width(8.dp))

                Surface(
                    shape = RoundedCornerShape(8.dp),
                    color = StudioColors.BgCard,
                    border = BorderStroke(1.dp, StudioColors.BorderSubtle)
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 8.dp, vertical = 5.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.spacedBy(5.dp)
                    ) {
                        Icon(
                            imageVector = Icons.Rounded.Memory,
                            contentDescription = null,
                            tint = StudioColors.AccentAmber,
                            modifier = Modifier.size(14.dp)
                        )
                        Text(
                            text = "${backendKind.shortBadge} · ${gfxCapabilities.glesVersionLabel}${if (gfxCapabilities.vulkanSupported) " + Vulkan" else ""}",
                            color = StudioColors.TextMuted,
                            fontSize = 10.sp,
                            maxLines = 1
                        )
                    }
                }

                Spacer(Modifier.width(8.dp))

                Row(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    if (!isLobbyMode && onReturnToLobby != null) {
                        OutlinedButton(
                            onClick = onReturnToLobby,
                            shape = RoundedCornerShape(10.dp),
                            border = BorderStroke(1.dp, StudioColors.BorderSubtle),
                            contentPadding = PaddingValues(horizontal = 10.dp, vertical = 8.dp)
                        ) {
                            Text(
                                "Menú Principal",
                                color = StudioColors.TextPrimary,
                                fontWeight = FontWeight.Medium,
                                fontSize = 12.sp,
                                maxLines = 1,
                                softWrap = false
                            )
                        }
                    }

                    OutlinedButton(
                        onClick = onImportModPackage,
                        shape = RoundedCornerShape(10.dp),
                        border = BorderStroke(1.dp, StudioColors.AccentEmerald.copy(alpha = 0.7f)),
                        contentPadding = PaddingValues(horizontal = 12.dp, vertical = 8.dp)
                    ) {
                        Icon(
                            Icons.Rounded.UploadFile,
                            contentDescription = null,
                            tint = StudioColors.AccentEmerald,
                            modifier = Modifier.size(17.dp)
                        )
                        Spacer(Modifier.width(5.dp))
                        Text(
                            "Importar .mod",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.SemiBold,
                            fontSize = 12.sp,
                            maxLines = 1,
                            softWrap = false
                        )
                    }

                    Button(
                        onClick = { createDialogOpen = true },
                        shape = RoundedCornerShape(10.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = StudioColors.AccentCyan,
                            contentColor = Color(0xFF1F0C00)
                        ),
                        contentPadding = PaddingValues(horizontal = 14.dp, vertical = 8.dp)
                    ) {
                        Icon(Icons.Rounded.Add, contentDescription = null, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(5.dp))
                        Text("Nuevo Proyecto", fontWeight = FontWeight.Bold, fontSize = 12.sp, maxLines = 1, softWrap = false)
                    }
                }
            }
        }

        if (projects.isEmpty()) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Text(
                    text = "No hay proyectos guardados todavía.",
                    color = StudioColors.TextMuted,
                    fontSize = 13.sp
                )
            }
        } else {
            Column(
                modifier = Modifier
                    .weight(1f)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                projects.chunked(2).forEach { rowItems ->
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(10.dp)
                    ) {
                        rowItems.forEach { proj ->
                            ProjectCard(
                                project = proj,
                                isLobbyMode = isLobbyMode,
                                formattedDate = if (proj.updatedAtMs > 0L) {
                                    dateFormatter.format(Date(proj.updatedAtMs))
                                } else {
                                    "Reciente"
                                },
                                canDelete = projects.size > 1,
                                onOpen = { onOpenProject(proj.id) },
                                onExportMod = { onExportProject(proj.id, proj.name) },
                                onDuplicate = { onDuplicateProject(proj.id) },
                                onRequestDelete = { deleteCandidate = proj },
                                modifier = Modifier.weight(1f)
                            )
                        }
                        if (rowItems.size == 1) {
                            Spacer(Modifier.weight(1f))
                        }
                    }
                }
            }
        }
    }

    if (createDialogOpen) {
        NewProjectDialog(
            onDismiss = { createDialogOpen = false },
            onConfirm = { name, author ->
                createDialogOpen = false
                onCreateProject(name, author)
            }
        )
    }

    val pendingDelete = deleteCandidate
    if (pendingDelete != null) {
        AlertDialog(
            onDismissRequest = { deleteCandidate = null },
            containerColor = StudioColors.BgPanel,
            titleContentColor = StudioColors.TextPrimary,
            textContentColor = StudioColors.TextMuted,
            title = {
                Text("Eliminar proyecto", fontWeight = FontWeight.Bold, fontSize = 16.sp)
            },
            text = {
                Text(
                    "¿Seguro que quieres eliminar \"${pendingDelete.name}\"? Se borrarán su chart, capas y texturas guardadas de forma permanente.",
                    fontSize = 13.sp
                )
            },
            confirmButton = {
                Button(
                    onClick = {
                        val id = pendingDelete.id
                        deleteCandidate = null
                        onDeleteProject(id)
                    },
                    colors = ButtonDefaults.buttonColors(
                        containerColor = StudioColors.AccentCoral,
                        contentColor = Color.White
                    )
                ) {
                    Text("Eliminar", fontWeight = FontWeight.Bold)
                }
            },
            dismissButton = {
                TextButton(onClick = { deleteCandidate = null }) {
                    Text("Cancelar", color = StudioColors.TextPrimary)
                }
            }
        )
    }
}

@Composable
private fun ProjectCard(
    project: ProjectSummary,
    isLobbyMode: Boolean,
    formattedDate: String,
    canDelete: Boolean,
    onOpen: () -> Unit,
    onExportMod: () -> Unit,
    onDuplicate: () -> Unit,
    onRequestDelete: () -> Unit,
    modifier: Modifier = Modifier
) {
    val showActiveBadge = !isLobbyMode && project.isActive
    val borderColor = if (showActiveBadge || (isLobbyMode && project.isActive)) {
        StudioColors.AccentEmerald
    } else {
        StudioColors.BorderSubtle
    }

    Surface(
        modifier = modifier,
        shape = RoundedCornerShape(12.dp),
        color = StudioColors.BgCard,
        border = BorderStroke(if (borderColor == StudioColors.AccentEmerald) 1.5.dp else 1.dp, borderColor)
    ) {
        Column(
            modifier = Modifier.padding(12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.Top
            ) {
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = project.name,
                        color = StudioColors.TextPrimary,
                        fontWeight = FontWeight.Bold,
                        fontSize = 14.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                    Text(
                        text = "Por ${project.author} · ${project.chartTitle} (${project.bpm.roundToInt()} BPM)",
                        color = StudioColors.TextMuted,
                        fontSize = 11.sp,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis
                    )
                }

                if (showActiveBadge || (isLobbyMode && project.isActive)) {
                    Spacer(Modifier.width(8.dp))
                    Surface(
                        shape = RoundedCornerShape(6.dp),
                        color = StudioColors.AccentEmerald.copy(alpha = 0.18f),
                        border = BorderStroke(1.dp, StudioColors.AccentEmerald)
                    ) {
                        Row(
                            modifier = Modifier.padding(horizontal = 7.dp, vertical = 3.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Icon(
                                imageVector = Icons.Rounded.CheckCircle,
                                contentDescription = null,
                                tint = StudioColors.AccentEmerald,
                                modifier = Modifier.size(12.dp)
                            )
                            Spacer(Modifier.width(4.dp))
                            Text(
                                text = if (isLobbyMode) "RECIENTE" else "ACTIVO",
                                color = StudioColors.AccentEmerald,
                                fontSize = 10.sp,
                                fontWeight = FontWeight.Bold
                            )
                        }
                    }
                }
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                MiniStatChip("Notas", project.noteCount.toString(), StudioColors.AccentCyan)
                MiniStatChip("Pistas", "${project.trackCount}/3", StudioColors.AccentEmerald)
                MiniStatChip("Capas", project.entityCount.toString(), StudioColors.AccentIndigo)
                MiniStatChip("Scripts", project.scriptCount.toString(), StudioColors.AccentPurple)
                MiniStatChip(
                    "API",
                    "v${project.devApiVersion}",
                    if (project.devApiVersion >= project.currentDevApiVersion) {
                        StudioColors.AccentEmerald
                    } else {
                        StudioColors.AccentAmber
                    }
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Icon(
                        imageVector = Icons.Rounded.Schedule,
                        contentDescription = null,
                        tint = StudioColors.TextMuted,
                        modifier = Modifier.size(13.dp)
                    )
                    Spacer(Modifier.width(4.dp))
                    Text(
                        text = formattedDate,
                        color = StudioColors.TextMuted,
                        fontSize = 11.sp
                    )
                }
                Text(
                    text = "Contenedor Vanilla (.mod)",
                    color = StudioColors.TextMuted,
                    fontSize = 10.sp
                )
            }

            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                if (isLobbyMode || !project.isActive) {
                    Button(
                        onClick = onOpen,
                        shape = RoundedCornerShape(8.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = StudioColors.AccentEmerald,
                            contentColor = Color(0xFF042117)
                        ),
                        contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                        modifier = Modifier.height(30.dp)
                    ) {
                        Text(
                            text = if (isLobbyMode) "Editar Mod" else "Abrir",
                            fontSize = 11.sp,
                            fontWeight = FontWeight.Bold,
                            maxLines = 1,
                            softWrap = false
                        )
                    }
                }

                OutlinedButton(
                    onClick = onExportMod,
                    shape = RoundedCornerShape(8.dp),
                    border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.65f)),
                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                    modifier = Modifier.height(30.dp)
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Download,
                        contentDescription = "Exportar .mod",
                        tint = StudioColors.AccentCyan,
                        modifier = Modifier.size(13.dp)
                    )
                    Spacer(Modifier.width(4.dp))
                    Text("Exportar .mod", color = StudioColors.TextPrimary, fontSize = 11.sp, maxLines = 1, softWrap = false)
                }

                OutlinedButton(
                    onClick = onDuplicate,
                    shape = RoundedCornerShape(8.dp),
                    border = BorderStroke(1.dp, StudioColors.BorderSubtle),
                    contentPadding = PaddingValues(horizontal = 8.dp, vertical = 4.dp),
                    modifier = Modifier.height(30.dp)
                ) {
                    Icon(
                        imageVector = Icons.Rounded.ContentCopy,
                        contentDescription = "Duplicar",
                        tint = StudioColors.TextPrimary,
                        modifier = Modifier.size(13.dp)
                    )
                    Spacer(Modifier.width(4.dp))
                    Text("Duplicar", color = StudioColors.TextPrimary, fontSize = 11.sp, maxLines = 1, softWrap = false)
                }

                Spacer(Modifier.weight(1f))

                IconButton(
                    onClick = onRequestDelete,
                    enabled = canDelete,
                    modifier = Modifier.size(30.dp)
                ) {
                    Icon(
                        imageVector = Icons.Rounded.DeleteOutline,
                        contentDescription = "Eliminar",
                        tint = if (canDelete) StudioColors.AccentCoral else StudioColors.BorderSubtle,
                        modifier = Modifier.size(17.dp)
                    )
                }
            }
        }
    }
}

@Composable
private fun MiniStatChip(label: String, value: String, color: Color) {
    Surface(
        shape = RoundedCornerShape(6.dp),
        color = StudioColors.BgPanel,
        border = BorderStroke(1.dp, StudioColors.BorderSubtle)
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 7.dp, vertical = 3.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            Text(text = label, color = StudioColors.TextMuted, fontSize = 10.sp)
            Text(text = value, color = color, fontSize = 10.sp, fontWeight = FontWeight.Bold)
        }
    }
}

@Composable
private fun NewProjectDialog(
    onDismiss: () -> Unit,
    onConfirm: (name: String, author: String) -> Unit
) {
    var name by remember { mutableStateOf("Nuevo Mod BeatForge") }
    var author by remember { mutableStateOf("Creador") }

    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = StudioColors.BgPanel,
        titleContentColor = StudioColors.TextPrimary,
        textContentColor = StudioColors.TextPrimary,
        title = {
            Text("Crear Nuevo Proyecto", fontWeight = FontWeight.Bold, fontSize = 16.sp)
        },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = name,
                    onValueChange = { name = it },
                    label = { Text("Nombre del Mod", fontSize = 11.sp) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
                OutlinedTextField(
                    value = author,
                    onValueChange = { author = it },
                    label = { Text("Autor", fontSize = 11.sp) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth()
                )
            }
        },
        confirmButton = {
            Button(
                onClick = {
                    val cleanName = name.trim().ifBlank { "Nuevo Mod BeatForge" }
                    val cleanAuthor = author.trim().ifBlank { "Creador" }
                    onConfirm(cleanName, cleanAuthor)
                },
                colors = ButtonDefaults.buttonColors(
                    containerColor = StudioColors.AccentCyan,
                    contentColor = Color(0xFF1F0C00)
                )
            ) {
                Text("Crear", fontWeight = FontWeight.Bold)
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancelar", color = StudioColors.TextMuted)
            }
        }
    )
}
