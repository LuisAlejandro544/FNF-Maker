// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/popups/ModFilesGuideDialog.kt
// Propósito: Ventana emergente modal que explica de forma clara dónde se importa cada tipo de
// archivo dentro de BeatForge Studio y en qué carpetas estándar de un mod suelen encontrarse
// (spritesheet, atlas XML/JSON, descriptor de posición del personaje, escenario, íconos, audio y chart).

package com.beatforge.studio.ui.popups

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material3.Button
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import com.beatforge.studio.ui.theme.StudioColors

private data class ModFileGuideEntry(
    val title: String,
    val accent: Color,
    val whereInStudio: String,
    val whereInModFolders: String,
    val note: String
)

private val MOD_GUIDE_ENTRIES = listOf(
    ModFileGuideEntry(
        title = "1. Hoja de Sprites (Textura)",
        accent = StudioColors.AccentCyan,
        whereInStudio = "Pestaña Sprites -> Botón PNG / ASTC (o Escena -> Subir PNG/.astc)",
        whereInModFolders = "images/characters/  o  mods/images/characters/  (archivo .png, .webp o .astc; si es fondo/prop: images/backgrounds/ o stages/<nombre>/)",
        note = "Contiene todos los dibujos del personaje o fondo en una sola imagen."
    ),
    ModFileGuideEntry(
        title = "2. Recorte de Animaciones (Atlas XML / JSON)",
        accent = StudioColors.AccentIndigo,
        whereInStudio = "Pestaña Sprites -> Botón XML / JSON",
        whereInModFolders = "En la misma carpeta del PNG: images/characters/<personaje>.xml (Sparrow), .json (TexturePacker), o carpeta con spritemap1.json + Animation.json",
        note = "Le dice al motor dónde recortar cada fotograma (Idle, Left, Down, Up, Right)."
    ),
    ModFileGuideEntry(
        title = "3. Posición del Personaje y Offsets (.json / .xml)",
        accent = StudioColors.AccentEmerald,
        whereInStudio = "Pestaña Sprites -> Botón Posición (.json/.xml)",
        whereInModFolders = "characters/  o  mods/characters/  o  assets/data/characters/  (ej. personaje.json estilo Psych/V-Slice o personaje.xml con etiqueta <character>)",
        note = "Trae position [x, y], scale, flip_x, cámara y los offsets [x, y] de cada pose. Puedes subirlo antes o después del XML."
    ),
    ModFileGuideEntry(
        title = "4. Posición Global del Escenario (.json / .xml)",
        accent = StudioColors.AccentAmber,
        whereInStudio = "Pestaña Escena -> Botón Escena .json/.xml",
        whereInModFolders = "stages/  o  mods/stages/  o  assets/data/stages/  (ej. escenario.json o <stage>.xml)",
        note = "Ubica automáticamente al Player, Enemigo, Cabina/Decoración, zoom inicial de cámara y props del fondo."
    ),
    ModFileGuideEntry(
        title = "5. Íconos de Barra de Vida y Barra SVG",
        accent = StudioColors.AccentPurple,
        whereInStudio = "Pestaña Sprites -> Botón Íconos / Vida",
        whereInModFolders = "images/icons/  (archivo doble icon-<nombre>.png o íconos individuales normal/perdiendo)",
        note = "Se muestran sobre la barra de vida durante gameplay."
    ),
    ModFileGuideEntry(
        title = "6. Pistas de Audio (1 a 3 canales)",
        accent = StudioColors.AccentCyan,
        whereInStudio = "Pestaña Audio -> Pista 1 (Mix/Inst), Pista 2 (Voces Player), Pista 3 (Voces Rival)",
        whereInModFolders = "songs/<cancion>/  o  mods/songs/<cancion>/  (Inst.ogg, Voices.ogg, Voices-Player.ogg, Voices-Opponent.ogg)",
        note = "El motor mezcla hasta 3 pistas simultáneas en Rust."
    ),
    ModFileGuideEntry(
        title = "7. Chart de Notas y Eventos (.json)",
        accent = StudioColors.AccentCoral,
        whereInStudio = "Pestaña Chart -> Botón Importar",
        whereInModFolders = "data/<cancion>/  o  mods/data/<cancion>/  (ej. <cancion>-hard.json, <cancion>.json)",
        note = "Incluye notas, BPM por sección y eventos de cámara/flash/swap."
    ),
    ModFileGuideEntry(
        title = "8. Scripts Modchart (.lua / .ts / .js)",
        accent = StudioColors.AccentEmerald,
        whereInStudio = "Pestaña Scripts -> Importar",
        whereInModFolders = "scripts/, mods/scripts/, data/<cancion>/script.lua o custom_events/",
        note = "Ejecutados en Lua 5.4, oxc TypeScript o QuickJS-NG."
    ),
    ModFileGuideEntry(
        title = "9. Skin de Flechas de Mods (.png + .xml) y Flechas Especiales",
        accent = StudioColors.AccentCoral,
        whereInStudio = "Barra lateral -> Skin -> 1. Hoja .PNG y 2. Atlas .XML",
        whereInModFolders = "images/NOTE_assets.png + images/NOTE_assets.xml, mods/images/NOTE_assets.*, assets/shared/images/NOTE_assets.* o images/noteSkins/",
        note = "El motor recorta automáticamente las 4 direcciones (purple/blue/green/red o left/down/up/right). Las notas peligrosas (Hurt / Instakill) muestran la flecha SVG con una X."
    )
)

@Composable
fun ModFilesGuideDialog(
    onDismiss: () -> Unit
) {
    Dialog(
        onDismissRequest = onDismiss,
        properties = DialogProperties(usePlatformDefaultWidth = false)
    ) {
        Surface(
            modifier = Modifier
                .fillMaxWidth(0.86f)
                .fillMaxHeight(0.88f)
                .padding(12.dp),
            shape = RoundedCornerShape(14.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Column(modifier = Modifier.padding(14.dp)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically
                ) {
                    Icon(
                        imageVector = Icons.Rounded.FolderOpen,
                        contentDescription = null,
                        tint = StudioColors.AccentCyan,
                        modifier = Modifier.size(22.dp)
                    )
                    Spacer(Modifier.width(8.dp))
                    Column(modifier = Modifier.weight(1f)) {
                        Text(
                            text = "Guía de Archivos de Mods: ¿Qué va dónde y dónde se consigue?",
                            color = StudioColors.TextPrimary,
                            fontWeight = FontWeight.Bold,
                            fontSize = 15.sp
                        )
                        Text(
                            text = "Rutas típicas al descomprimir un mod (.zip / carpeta) en tu teléfono",
                            color = StudioColors.TextMuted,
                            fontSize = 11.sp
                        )
                    }
                    IconButton(onClick = onDismiss) {
                        Icon(
                            imageVector = Icons.Rounded.Close,
                            contentDescription = "Cerrar",
                            tint = StudioColors.TextMuted
                        )
                    }
                }

                Spacer(Modifier.height(10.dp))

                Column(
                    modifier = Modifier
                        .weight(1f)
                        .verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(8.dp)
                ) {
                    MOD_GUIDE_ENTRIES.forEach { entry ->
                        Surface(
                            shape = RoundedCornerShape(10.dp),
                            color = StudioColors.BgCard,
                            border = BorderStroke(1.dp, entry.accent.copy(alpha = 0.45f))
                        ) {
                            Column(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(10.dp),
                                verticalArrangement = Arrangement.spacedBy(3.dp)
                            ) {
                                Text(
                                    text = entry.title,
                                    color = entry.accent,
                                    fontWeight = FontWeight.Bold,
                                    fontSize = 12.5.sp
                                )
                                Text(
                                    text = "En BeatForge Studio: ${entry.whereInStudio}",
                                    color = StudioColors.TextPrimary,
                                    fontWeight = FontWeight.SemiBold,
                                    fontSize = 11.sp
                                )
                                Text(
                                    text = "En carpetas de mod: ${entry.whereInModFolders}",
                                    color = StudioColors.AccentAmber,
                                    fontSize = 11.sp
                                )
                                Text(
                                    text = entry.note,
                                    color = StudioColors.TextMuted,
                                    fontSize = 10.5.sp
                                )
                            }
                        }
                    }
                }

                Spacer(Modifier.height(10.dp))

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End
                ) {
                    Button(onClick = onDismiss) {
                        Text("Entendido", fontSize = 12.sp, fontWeight = FontWeight.Bold)
                    }
                }
            }
        }
    }
}
