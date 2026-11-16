// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/components/StudioNavBar.kt
// Propósito: Barra de navegación superior modular para pantallas horizontales del estudio.
// Incluye iconos de Material Icons Extended para cambiar de apartado sin amontonar controles
// y el botón principal para compilar en caliente e iniciar la prueba jugable.

package com.beatforge.studio.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Animation
import androidx.compose.material.icons.rounded.Dashboard
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material.icons.rounded.LibraryMusic
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Terminal
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StudioScreen
import com.beatforge.studio.ui.theme.StudioColors

private data class NavTabItem(
    val screen: StudioScreen,
    val icon: ImageVector
)

private val NAV_TABS = listOf(
    NavTabItem(StudioScreen.Hub, Icons.Rounded.Dashboard),
    NavTabItem(StudioScreen.ChartAudio, Icons.Rounded.LibraryMusic),
    NavTabItem(StudioScreen.SpriteLab, Icons.Rounded.Animation),
    NavTabItem(StudioScreen.StageEditor, Icons.Rounded.Layers),
    NavTabItem(StudioScreen.ScriptStudio, Icons.Rounded.Terminal)
)

@Composable
fun StudioNavBar(
    currentScreen: StudioScreen,
    onNavigate: (StudioScreen) -> Unit,
    onOpenArrowSkin: () -> Unit,
    onCompileAndPlay: () -> Unit
) {
    Surface(color = StudioColors.BgPanel) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .height(54.dp)
                .padding(horizontal = 12.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier
                    .background(StudioColors.BgCard, RoundedCornerShape(8.dp))
                    .padding(horizontal = 10.dp, vertical = 6.dp)
            ) {
                Icon(
                    imageVector = Icons.Rounded.GraphicEq,
                    contentDescription = null,
                    tint = StudioColors.AccentCyan,
                    modifier = Modifier.size(18.dp)
                )
                Spacer(Modifier.width(6.dp))
                Text(
                    text = "BeatForge",
                    color = StudioColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 14.sp
                )
            }

            Spacer(Modifier.width(10.dp))

            Row(
                modifier = Modifier
                    .weight(1f)
                    .horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(6.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                NAV_TABS.forEach { tab ->
                    val selected = currentScreen == tab.screen
                    Button(
                        onClick = { onNavigate(tab.screen) },
                        shape = RoundedCornerShape(8.dp),
                        contentPadding = PaddingValues(horizontal = 11.dp, vertical = 6.dp),
                        colors = ButtonDefaults.buttonColors(
                            containerColor = if (selected) StudioColors.AccentCyan.copy(alpha = 0.2f) else StudioColors.BgCard,
                            contentColor = if (selected) StudioColors.AccentCyan else StudioColors.TextMuted
                        ),
                        modifier = Modifier.height(36.dp)
                    ) {
                        Icon(
                            imageVector = tab.icon,
                            contentDescription = tab.screen.label,
                            modifier = Modifier.size(16.dp)
                        )
                        Spacer(Modifier.width(6.dp))
                        Text(
                            text = tab.screen.label,
                            fontSize = 12.sp,
                            fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium
                        )
                    }
                }
            }

            Spacer(Modifier.width(8.dp))

            OutlinedButton(
                onClick = onOpenArrowSkin,
                shape = RoundedCornerShape(8.dp),
                contentPadding = PaddingValues(horizontal = 10.dp, vertical = 6.dp),
                modifier = Modifier.height(36.dp)
            ) {
                Icon(
                    imageVector = Icons.Rounded.Palette,
                    contentDescription = "Skin SVG",
                    tint = StudioColors.AccentPurple,
                    modifier = Modifier.size(16.dp)
                )
                Spacer(Modifier.width(5.dp))
                Text(
                    text = "Skin SVG",
                    color = StudioColors.TextPrimary,
                    fontSize = 12.sp
                )
            }

            Spacer(Modifier.width(8.dp))

            Button(
                onClick = onCompileAndPlay,
                shape = RoundedCornerShape(8.dp),
                contentPadding = PaddingValues(horizontal = 14.dp, vertical = 6.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = StudioColors.AccentEmerald,
                    contentColor = Color(0xFF042419)
                ),
                modifier = Modifier.height(36.dp)
            ) {
                Icon(
                    imageVector = Icons.Rounded.PlayArrow,
                    contentDescription = "Compilar y Jugar",
                    modifier = Modifier.size(18.dp)
                )
                Spacer(Modifier.width(4.dp))
                Text(
                    text = "Compilar y Jugar",
                    fontSize = 12.sp,
                    fontWeight = FontWeight.Bold
                )
            }
        }
    }
    HorizontalDivider(color = StudioColors.BorderSubtle, thickness = 1.dp)
}
