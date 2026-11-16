// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/HotCompileScreen.kt
// Propósito: Pantalla de carga de Compilación en Caliente mientras Rust (`rayon` + `resvg`)
// descarta objetos invisibles, escala texturas al perfil del dispositivo y prepara el audio.

package com.beatforge.studio.ui.screens

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.RocketLaunch
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.DeviceTier
import com.beatforge.studio.ui.theme.StudioColors

@Composable
fun HotCompileScreen(tier: DeviceTier) {
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.Center
    ) {
        Surface(
            modifier = Modifier
                .fillMaxWidth(0.55f)
                .padding(16.dp),
            shape = RoundedCornerShape(16.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.AccentCyan.copy(alpha = 0.45f))
        ) {
            Column(
                modifier = Modifier.padding(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                Icon(
                    imageVector = Icons.Rounded.RocketLaunch,
                    contentDescription = null,
                    tint = StudioColors.AccentCyan,
                    modifier = Modifier.size(34.dp)
                )
                Text(
                    text = "Compilando Mod en Caliente...",
                    color = StudioColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 18.sp
                )
                Text(
                    text = "Perfil: ${tier.label} · Optimizando sprites en paralelo (Rayon) y rasterizando flechas SVG (resvg)",
                    color = StudioColors.TextMuted,
                    fontSize = 12.sp
                )
                Spacer(Modifier.height(4.dp))
                CircularProgressIndicator(
                    color = StudioColors.AccentEmerald,
                    strokeWidth = 3.dp,
                    modifier = Modifier.size(36.dp)
                )
                LinearProgressIndicator(
                    color = StudioColors.AccentCyan,
                    trackColor = StudioColors.BgDark,
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(6.dp)
                )
            }
        }
    }
}
