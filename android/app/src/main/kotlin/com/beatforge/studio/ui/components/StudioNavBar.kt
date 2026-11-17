// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/components/StudioNavBar.kt
// Propósito: Barra de navegación lateral compacta (Navigation Rail) optimizada para teléfonos
// en modo horizontal. Aprovecha al máximo el ancho horizontal sin recortar pestañas y libera
// toda la altura vertical de la pantalla para el chart, sprites, escenario y código.

package com.beatforge.studio.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
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
import androidx.compose.material.icons.rounded.Animation
import androidx.compose.material.icons.rounded.Dashboard
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.GraphicEq
import androidx.compose.material.icons.rounded.Layers
import androidx.compose.material.icons.rounded.Palette
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Terminal
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material3.Icon
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.VerticalDivider
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.StudioScreen
import com.beatforge.studio.ui.theme.StudioColors

private data class NavRailItem(
    val screen: StudioScreen,
    val icon: ImageVector
)

private val RAIL_TABS = listOf(
    NavRailItem(StudioScreen.Hub, Icons.Rounded.Dashboard),
    NavRailItem(StudioScreen.Projects, Icons.Rounded.FolderOpen),
    NavRailItem(StudioScreen.Chart, Icons.Rounded.Tune),
    NavRailItem(StudioScreen.Audio, Icons.Rounded.GraphicEq),
    NavRailItem(StudioScreen.SpriteLab, Icons.Rounded.Animation),
    NavRailItem(StudioScreen.StageEditor, Icons.Rounded.Layers),
    NavRailItem(StudioScreen.ScriptStudio, Icons.Rounded.Terminal)
)

@Composable
fun StudioNavRail(
    currentScreen: StudioScreen,
    onNavigate: (StudioScreen) -> Unit,
    onOpenArrowSkin: () -> Unit,
    onCompileAndPlay: () -> Unit
) {
    Surface(
        color = StudioColors.BgPanel,
        modifier = Modifier
            .width(66.dp)
            .fillMaxHeight()
    ) {
        Column(
            modifier = Modifier
                .fillMaxHeight()
                .padding(vertical = 8.dp, horizontal = 5.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            // Navegación principal desplazable si la altura del teléfono es pequeña
            Column(
                modifier = Modifier
                    .weight(1f)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(4.dp),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                RAIL_TABS.forEach { tab ->
                    val selected = currentScreen == tab.screen
                    RailButton(
                        label = tab.screen.label,
                        icon = tab.icon,
                        selected = selected,
                        accent = StudioColors.AccentCyan,
                        onClick = { onNavigate(tab.screen) }
                    )
                }
            }

            Spacer(Modifier.height(4.dp))

            // Acceso rápido a Skin SVG
            RailButton(
                label = "Skin",
                icon = Icons.Rounded.Palette,
                selected = false,
                accent = StudioColors.AccentPurple,
                onClick = onOpenArrowSkin
            )

            Spacer(Modifier.height(5.dp))

            // Botón principal de Compilar y Jugar (siempre visible abajo)
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(10.dp))
                    .background(StudioColors.AccentEmerald)
                    .clickable(onClick = onCompileAndPlay)
                    .padding(vertical = 7.dp, horizontal = 2.dp),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                Icon(
                    imageVector = Icons.Rounded.PlayArrow,
                    contentDescription = "Jugar",
                    tint = Color(0xFF042117),
                    modifier = Modifier.size(20.dp)
                )
                Text(
                    text = "Jugar",
                    color = Color(0xFF042117),
                    fontSize = 10.sp,
                    fontWeight = FontWeight.Bold,
                    maxLines = 1,
                    softWrap = false
                )
            }
        }
    }
    VerticalDivider(color = StudioColors.BorderSubtle, thickness = 1.dp)
}

@Composable
private fun RailButton(
    label: String,
    icon: ImageVector,
    selected: Boolean,
    accent: Color,
    onClick: () -> Unit
) {
    val bgColor = if (selected) accent.copy(alpha = 0.18f) else StudioColors.BgCard
    val borderColor = if (selected) accent else Color.Transparent
    val fgColor = if (selected) accent else StudioColors.TextMuted

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(9.dp))
            .background(bgColor)
            .border(1.dp, borderColor, RoundedCornerShape(9.dp))
            .clickable(onClick = onClick)
            .padding(vertical = 6.dp, horizontal = 2.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Icon(
            imageVector = icon,
            contentDescription = label,
            tint = if (selected) accent else StudioColors.TextPrimary,
            modifier = Modifier.size(18.dp)
        )
        Spacer(Modifier.height(2.dp))
        Text(
            text = label,
            color = fgColor,
            fontSize = 9.sp,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
            maxLines = 1,
            softWrap = false,
            overflow = TextOverflow.Clip
        )
    }
}
