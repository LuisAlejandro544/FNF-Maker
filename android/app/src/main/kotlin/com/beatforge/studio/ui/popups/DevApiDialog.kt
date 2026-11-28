// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/DevApiDialog.kt
// Propósito: Ventana modal de la "API de desarrollo" de BeatForge Studio. Muestra la versión
// activa del mod frente a la versión vigente del motor (`CURRENT_DEV_API_VERSION`), detalla
// las capacidades de cada versión y permite migrar proyectos antiguos de forma explícita
// sin perder sus charts, pistas, sprites ni scripts.

package com.beatforge.studio.ui.popups

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.SystemUpdateAlt
import androidx.compose.material.icons.rounded.Upgrade
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.ProjectSnapshot
import com.beatforge.studio.ui.theme.StudioColors

@Composable
fun DevApiDialog(
    snapshot: ProjectSnapshot,
    onUpgrade: () -> Unit,
    onDismiss: () -> Unit
) {
    val isUpToDate = snapshot.devApiVersion >= snapshot.currentDevApiVersion

    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = StudioColors.BgPanel,
        titleContentColor = StudioColors.TextPrimary,
        textContentColor = StudioColors.TextPrimary,
        title = {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    imageVector = if (isUpToDate) Icons.Rounded.AutoAwesome else Icons.Rounded.SystemUpdateAlt,
                    contentDescription = null,
                    tint = if (isUpToDate) StudioColors.AccentEmerald else StudioColors.AccentAmber,
                    modifier = Modifier.size(20.dp)
                )
                Spacer(Modifier.width(8.dp))
                Text(
                    text = "API de desarrollo · Mod",
                    fontWeight = FontWeight.Bold,
                    fontSize = 16.sp
                )
            }
        },
        text = {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(10.dp)
            ) {
                Surface(
                    shape = RoundedCornerShape(10.dp),
                    color = if (isUpToDate) StudioColors.AccentEmerald.copy(alpha = 0.12f) else StudioColors.AccentAmber.copy(alpha = 0.14f),
                    border = BorderStroke(
                        1.dp,
                        if (isUpToDate) StudioColors.AccentEmerald else StudioColors.AccentAmber
                    )
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(10.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = "Proyecto actual: API v${snapshot.devApiVersion}",
                                color = StudioColors.TextPrimary,
                                fontWeight = FontWeight.Bold,
                                fontSize = 13.sp
                            )
                            Text(
                                text = "${snapshot.devApiLabel} · Motor vigente v${snapshot.currentDevApiVersion}",
                                color = StudioColors.TextMuted,
                                fontSize = 11.sp
                            )
                        }
                        Icon(
                            imageVector = if (isUpToDate) Icons.Rounded.CheckCircle else Icons.Rounded.Upgrade,
                            contentDescription = null,
                            tint = if (isUpToDate) StudioColors.AccentEmerald else StudioColors.AccentAmber,
                            modifier = Modifier.size(20.dp)
                        )
                    }
                }

                Text(
                    text = "Cada vez que el estudio recibe funciones nuevas de charteo, eventos, cámara o scripting, se incrementa la versión de la API de desarrollo. Los mods nuevos nacen automáticamente en la versión vigente.",
                    color = StudioColors.TextMuted,
                    fontSize = 11.sp,
                    lineHeight = 15.sp
                )

                DevApiChangelogCard(
                    version = "API v2 · Scripting Pro & Multicámara (Vigente)",
                    active = snapshot.devApiVersion >= 2,
                    bullets = listOf(
                        "Cuenta regresiva oficial de 3 pasos (2 · 1 · ¡YA!) y assets custom por paso.",
                        "Sistema Multicámara (Cam 0..3+), Viñeta y eventos del chart con bloques protegidos en Lua.",
                        "Tweens, Timers, precarga/retiro de sprites en RAM, HUD/Strum scripting y gráficos dinámicos.",
                        "Editor de código con números de línea y Asistente IA modular con SKILL.md + OpenRouter."
                    )
                )

                DevApiChangelogCard(
                    version = "API v1 · Base Inicial",
                    active = snapshot.devApiVersion == 1,
                    bullets = listOf(
                        "Estructura clásica de chart por secciones, 3 pistas de audio y escenario por capas Z-order.",
                        "Callbacks iniciales (onCreate, onUpdate, onBeatHit, onStepHit) y Hot Compile base."
                    )
                )

                if (!isUpToDate) {
                    Text(
                        text = "Migrar a API v${snapshot.currentDevApiVersion} conserva tus notas, audios, sprites y scripts actuales intactos, habilitando la firma de la versión más reciente.",
                        color = StudioColors.AccentEmerald,
                        fontSize = 11.sp,
                        lineHeight = 14.sp
                    )
                }
            }
        },
        confirmButton = {
            if (!isUpToDate) {
                Button(
                    onClick = onUpgrade,
                    colors = ButtonDefaults.buttonColors(
                        containerColor = StudioColors.AccentEmerald,
                        contentColor = Color(0xFF042117)
                    )
                ) {
                    Icon(Icons.Rounded.Upgrade, null, modifier = Modifier.size(16.dp))
                    Spacer(Modifier.width(4.dp))
                    Text("Migrar a API v${snapshot.currentDevApiVersion}", fontWeight = FontWeight.Bold)
                }
            } else {
                Button(
                    onClick = onDismiss,
                    colors = ButtonDefaults.buttonColors(
                        containerColor = StudioColors.AccentCyan,
                        contentColor = Color(0xFF1F0C00)
                    )
                ) {
                    Text("Entendido", fontWeight = FontWeight.Bold)
                }
            }
        },
        dismissButton = {
            if (!isUpToDate) {
                TextButton(onClick = onDismiss) {
                    Text("Mantener en v${snapshot.devApiVersion}", color = StudioColors.TextMuted)
                }
            }
        }
    )
}

@Composable
private fun DevApiChangelogCard(
    version: String,
    active: Boolean,
    bullets: List<String>
) {
    Surface(
        shape = RoundedCornerShape(10.dp),
        color = StudioColors.BgCard,
        border = BorderStroke(1.dp, if (active) StudioColors.AccentCyan.copy(alpha = 0.55f) else StudioColors.BorderSubtle)
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(10.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp)
        ) {
            Text(
                text = version,
                color = if (active) StudioColors.AccentCyan else StudioColors.TextPrimary,
                fontWeight = FontWeight.Bold,
                fontSize = 12.sp
            )
            bullets.forEach { item ->
                Text(
                    text = "• $item",
                    color = StudioColors.TextMuted,
                    fontSize = 11.sp,
                    lineHeight = 14.sp
                )
            }
        }
    }
}
