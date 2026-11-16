// Archivo: android/app/src/main/kotlin/com/beatforge/studio/ui/screens/ScriptStudioScreen.kt
// Propósito: Estudio de Scripts multi-lenguaje (Lua 5.4, TypeScript transpilado con `oxc`
// y JavaScript ES2022 ejecutado con `rquickjs`). Usa editor nativo de Android con soporte
// completo de teclado virtual y validación real de sintaxis ejecutada en Rust.

package com.beatforge.studio.ui.screens

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
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
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Code
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material.icons.rounded.Save
import androidx.compose.material.icons.rounded.UploadFile
import androidx.compose.material.icons.rounded.Verified
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CheckboxDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.beatforge.studio.ImportKind
import com.beatforge.studio.ProjectSnapshot
import com.beatforge.studio.ui.theme.StudioColors

@Composable
fun ScriptStudioScreen(
    snapshot: ProjectSnapshot,
    selectedScriptId: String,
    validationMsg: String?,
    validationOk: Boolean,
    onSelectScript: (String) -> Unit,
    onAddScript: (language: String) -> Unit,
    onRequestImportScript: () -> Unit,
    onUpdateScript: (scriptId: String, enabled: Boolean, sourceCode: String, silent: Boolean) -> Unit,
    onValidateScript: (scriptId: String, enabled: Boolean, sourceCode: String) -> Unit
) {
    val activeScript = snapshot.scripts.find { it.id == selectedScriptId }
        ?: snapshot.scripts.firstOrNull()

    var editedSource by remember(activeScript?.id) {
        mutableStateOf(activeScript?.sourceCode ?: "")
    }

    Row(
        modifier = Modifier
            .fillMaxSize()
            .padding(12.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // Columna izquierda: Lista de scripts del proyecto
        Surface(
            modifier = Modifier
                .weight(0.30f)
                .fillMaxHeight(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            Column(modifier = Modifier.padding(10.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = "Scripts del Mod",
                    color = StudioColors.TextPrimary,
                    fontWeight = FontWeight.Bold,
                    fontSize = 14.sp
                )

                Row(horizontalArrangement = Arrangement.spacedBy(4.dp), modifier = Modifier.fillMaxWidth()) {
                    listOf("lua" to "+Lua", "ts" to "+TS", "js" to "+JS").forEach { (lang, label) ->
                        Button(
                            onClick = { onAddScript(lang) },
                            contentPadding = PaddingValues(horizontal = 6.dp, vertical = 2.dp),
                            colors = ButtonDefaults.buttonColors(
                                containerColor = StudioColors.BgElevated,
                                contentColor = StudioColors.AccentCyan
                            ),
                            modifier = Modifier
                                .weight(1f)
                                .height(30.dp)
                        ) {
                            Text(label, fontSize = 11.sp, fontWeight = FontWeight.Bold)
                        }
                    }
                }

                OutlinedButton(
                    onClick = onRequestImportScript,
                    contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(34.dp)
                ) {
                    Icon(Icons.Rounded.UploadFile, null, modifier = Modifier.size(15.dp))
                    Spacer(Modifier.width(5.dp))
                    Text("Importar .lua / .ts / .js", fontSize = 11.sp)
                }

                Column(
                    modifier = Modifier
                        .weight(1f)
                        .verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(5.dp)
                ) {
                    snapshot.scripts.forEach { script ->
                        val isSel = script.id == activeScript?.id
                        val badgeColor = when (script.language) {
                            "ts" -> StudioColors.AccentCyan
                            "js" -> StudioColors.AccentAmber
                            else -> StudioColors.AccentIndigo
                        }
                        Surface(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onSelectScript(script.id) },
                            shape = RoundedCornerShape(8.dp),
                            color = if (isSel) StudioColors.AccentPurple.copy(alpha = 0.16f) else StudioColors.BgCard,
                            border = BorderStroke(
                                1.dp,
                                if (isSel) StudioColors.AccentPurple else StudioColors.BorderSubtle
                            )
                        ) {
                            Row(
                                modifier = Modifier.padding(horizontal = 8.dp, vertical = 6.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Text(
                                    text = script.language.uppercase(),
                                    color = badgeColor,
                                    fontWeight = FontWeight.Bold,
                                    fontSize = 10.sp,
                                    modifier = Modifier
                                        .background(badgeColor.copy(alpha = 0.16f), RoundedCornerShape(4.dp))
                                        .padding(horizontal = 5.dp, vertical = 2.dp)
                                )
                                Spacer(Modifier.width(6.dp))
                                Text(
                                    text = script.name,
                                    color = StudioColors.TextPrimary,
                                    fontSize = 12.sp,
                                    maxLines = 1,
                                    modifier = Modifier.weight(1f)
                                )
                                Checkbox(
                                    checked = script.enabled,
                                    onCheckedChange = { checked ->
                                        val codeToSave = if (isSel) editedSource else script.sourceCode
                                        onUpdateScript(script.id, checked, codeToSave, true)
                                    },
                                    colors = CheckboxDefaults.colors(checkedColor = StudioColors.AccentEmerald),
                                    modifier = Modifier.size(24.dp)
                                )
                            }
                        }
                    }
                }
            }
        }

        // Columna derecha: Editor monoespaciado + validación real en Rust
        Surface(
            modifier = Modifier
                .weight(0.70f)
                .fillMaxHeight(),
            shape = RoundedCornerShape(12.dp),
            color = StudioColors.BgPanel,
            border = BorderStroke(1.dp, StudioColors.BorderSubtle)
        ) {
            if (activeScript == null) {
                Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                    Text("Crea o importa un script para comenzar", color = StudioColors.TextMuted)
                }
            } else {
                Column(modifier = Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically) {
                            Icon(Icons.Rounded.Code, null, tint = StudioColors.AccentPurple, modifier = Modifier.size(18.dp))
                            Spacer(Modifier.width(6.dp))
                            Text(
                                text = "${activeScript.name} (${activeScript.languageLabel})",
                                color = StudioColors.TextPrimary,
                                fontWeight = FontWeight.Bold,
                                fontSize = 14.sp
                            )
                        }

                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            OutlinedButton(
                                onClick = {
                                    onUpdateScript(activeScript.id, activeScript.enabled, editedSource, false)
                                },
                                contentPadding = PaddingValues(horizontal = 10.dp, vertical = 4.dp),
                                modifier = Modifier.height(34.dp)
                            ) {
                                Icon(Icons.Rounded.Save, null, modifier = Modifier.size(15.dp))
                                Spacer(Modifier.width(4.dp))
                                Text("Guardar", fontSize = 11.sp)
                            }

                            Button(
                                onClick = {
                                    onValidateScript(activeScript.id, activeScript.enabled, editedSource)
                                },
                                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 4.dp),
                                colors = ButtonDefaults.buttonColors(
                                    containerColor = StudioColors.AccentPurple,
                                    contentColor = Color.White
                                ),
                                modifier = Modifier.height(34.dp)
                            ) {
                                Icon(Icons.Rounded.Verified, null, modifier = Modifier.size(15.dp))
                                Spacer(Modifier.width(4.dp))
                                Text("Verificar Sintaxis", fontSize = 11.sp, fontWeight = FontWeight.Bold)
                            }
                        }
                    }

                    if (!validationMsg.isNullOrBlank()) {
                        Surface(
                            shape = RoundedCornerShape(8.dp),
                            color = if (validationOk) StudioColors.AccentEmerald.copy(alpha = 0.14f) else StudioColors.AccentCoral.copy(alpha = 0.14f),
                            border = BorderStroke(
                                1.dp,
                                if (validationOk) StudioColors.AccentEmerald else StudioColors.AccentCoral
                            )
                        ) {
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .padding(horizontal = 10.dp, vertical = 6.dp),
                                verticalAlignment = Alignment.CenterVertically
                            ) {
                                Icon(
                                    imageVector = if (validationOk) Icons.Rounded.CheckCircle else Icons.Rounded.ErrorOutline,
                                    contentDescription = null,
                                    tint = if (validationOk) StudioColors.AccentEmerald else StudioColors.AccentCoral,
                                    modifier = Modifier.size(16.dp)
                                )
                                Spacer(Modifier.width(6.dp))
                                Text(
                                    text = validationMsg,
                                    color = StudioColors.TextPrimary,
                                    fontSize = 12.sp
                                )
                            }
                        }
                    }

                    OutlinedTextField(
                        value = editedSource,
                        onValueChange = {
                            editedSource = it
                            onUpdateScript(activeScript.id, activeScript.enabled, it, true)
                        },
                        textStyle = TextStyle(
                            fontFamily = FontFamily.Monospace,
                            fontSize = 12.sp,
                            color = StudioColors.TextPrimary
                        ),
                        modifier = Modifier
                            .fillMaxWidth()
                            .weight(1f)
                            .background(StudioColors.BgDark, RoundedCornerShape(8.dp))
                    )

                    Text(
                        text = "API: onCreate() · onBeatHit(beat) · onUpdate(dt) · onNoteHit(lane, isPlayer) · setCameraZoom(z) · shakeCamera(px) · setEntityProp(id, prop, val) · playEntityAnim(id, anim)",
                        color = StudioColors.TextMuted,
                        fontSize = 10.sp
                    )
                }
            }
        }
    }
}
