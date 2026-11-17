// Archivo: android/app/src/main/kotlin/com/beatforge/studio/MainActivity.kt
// Propósito: Actividad principal (`ComponentActivity` + Jetpack Compose) conectada al motor
// nativo Rust (`libpulse_android.so`) vía JNI a través de `StudioViewModel`.
// Configura pantalla horizontal de borde a borde respetando muescas/cámara (`displayCutoutPadding`),
// barra de navegación lateral compacta (`StudioNavRail`) y selector nativo SAF en `Dispatchers.IO`.

package com.beatforge.studio

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Process
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.viewModels
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.displayCutoutPadding
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.ErrorOutline
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import com.beatforge.studio.ui.components.StudioNavRail
import com.beatforge.studio.ui.popups.ArrowSkinDialog
import com.beatforge.studio.ui.popups.CharacterIconsDialog
import com.beatforge.studio.ui.popups.ModFilesGuideDialog
import com.beatforge.studio.ui.screens.AudioScreen
import com.beatforge.studio.ui.screens.ChartScreen
import com.beatforge.studio.ui.screens.GameplayScreen
import com.beatforge.studio.ui.screens.HotCompileScreen
import com.beatforge.studio.ui.screens.HubScreen
import com.beatforge.studio.ui.screens.ProjectsScreen
import com.beatforge.studio.ui.screens.ScriptStudioScreen
import com.beatforge.studio.ui.screens.SpriteLabScreen
import com.beatforge.studio.ui.screens.StageEditorScreen
import com.beatforge.studio.ui.theme.BeatForgeTheme
import com.beatforge.studio.ui.theme.StudioColors
import java.io.File

class MainActivity : ComponentActivity() {

    private val studioViewModel: StudioViewModel by viewModels()

    private val openDocumentLauncher =
        registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
            if (uri != null) {
                studioViewModel.onUriPicked(uri)
            }
        }

    private val createChartDocumentLauncher =
        registerForActivityResult(ActivityResultContracts.CreateDocument("application/json")) { uri: Uri? ->
            if (uri != null) {
                studioViewModel.exportChartToUri(uri)
            }
        }

    private val createModDocumentLauncher =
        registerForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri: Uri? ->
            if (uri != null) {
                studioViewModel.onExportModUriPicked(uri)
            }
        }

    override fun onCreate(savedInstanceState: Bundle?) {
        Thread.setDefaultUncaughtExceptionHandler { _, throwable ->
            launchCrashScreen(
                "Error inesperado en BeatForge Studio",
                throwable.stackTraceToString()
            )
            try {
                Thread.sleep(150)
            } catch (_: InterruptedException) {
            }
            Process.killProcess(Process.myPid())
        }

        try {
            NativeEngineBridge.ensureLoaded()
        } catch (t: Throwable) {
            launchCrashScreen(
                "No se pudo cargar el motor nativo Rust (libpulse_android.so)",
                t.stackTraceToString()
            )
            finish()
            return
        }

        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        configureImmersiveLandscape()

        studioViewModel.registerPickerLauncher { mimeTypes ->
            openDocumentLauncher.launch(mimeTypes)
        }
        studioViewModel.registerExportChartLauncher { suggestedName ->
            createChartDocumentLauncher.launch(suggestedName)
        }
        studioViewModel.registerExportModLauncher { suggestedName ->
            createModDocumentLauncher.launch(suggestedName)
        }

        setContent {
            BeatForgeTheme {
                val state by studioViewModel.uiState.collectAsStateWithLifecycle()
                BeatForgeRootContent(state = state, viewModel = studioViewModel)
            }
        }
    }

    override fun onPause() {
        super.onPause()
        studioViewModel.flushSave()
    }

    override fun onStop() {
        studioViewModel.flushSave()
        super.onStop()
    }

    private fun configureImmersiveLandscape() {
        WindowCompat.setDecorFitsSystemWindows(window, false)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            window.attributes = window.attributes.apply {
                layoutInDisplayCutoutMode =
                    WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
            }
        }
        val controller = WindowInsetsControllerCompat(window, window.decorView)
        controller.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        controller.hide(WindowInsetsCompat.Type.systemBars())
    }

    private fun launchCrashScreen(title: String, details: String) {
        try {
            File(filesDir, "last_java_crash.txt").writeText("$title\n\n$details")
        } catch (_: Throwable) {
        }
        val intent = Intent(this, CrashReportActivity::class.java).apply {
            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
            putExtra(CrashReportActivity.EXTRA_TITLE, title)
            putExtra(CrashReportActivity.EXTRA_DETAILS, details)
        }
        startActivity(intent)
    }
}

@Composable
private fun BeatForgeRootContent(
    state: StudioUiState,
    viewModel: StudioViewModel
) {
    Row(
        modifier = Modifier
            .fillMaxSize()
            .background(StudioColors.BgDark)
            .displayCutoutPadding()
    ) {
        if (!state.initialized) {
            Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                Column(horizontalAlignment = Alignment.CenterHorizontally) {
                    CircularProgressIndicator(color = StudioColors.AccentCyan)
                    Text(
                        text = "Inicializando motor Rust...",
                        color = StudioColors.TextMuted,
                        fontSize = 13.sp,
                        modifier = Modifier.padding(top = 12.dp)
                    )
                }
            }
            return@Row
        }

        if (state.currentScreen != StudioScreen.Gameplay && state.currentScreen != StudioScreen.HotCompile) {
            StudioNavRail(
                currentScreen = state.currentScreen,
                onNavigate = { viewModel.navigateTo(it) },
                onOpenArrowSkin = { viewModel.setArrowSkinDialogOpen(true) },
                onCompileAndPlay = { viewModel.startCompileAndPlay() }
            )
        }

        Column(
            modifier = Modifier
                .weight(1f)
                .fillMaxHeight()
        ) {
            if (state.isBusy && state.currentScreen != StudioScreen.HotCompile) {
                LinearProgressIndicator(
                    color = StudioColors.AccentCyan,
                    trackColor = StudioColors.BgPanel,
                    modifier = Modifier.fillMaxWidth()
                )
            }

            Box(
                modifier = Modifier
                    .weight(1f)
                    .fillMaxWidth()
            ) {
                when (state.currentScreen) {
                    StudioScreen.Hub -> HubScreen(
                        snapshot = state.snapshot,
                        onUpdateMeta = { name, author, downscroll, controlMode, offset, speed ->
                            viewModel.updateProjectMeta(name, author, downscroll, controlMode, offset, speed)
                        },
                        onNavigate = { viewModel.navigateTo(it) },
                        onOpenArrowSkin = { viewModel.setArrowSkinDialogOpen(true) },
                        onCompileAndPlay = { viewModel.startCompileAndPlay() }
                    )

                    StudioScreen.Projects -> ProjectsScreen(
                        projects = state.projects,
                        activeProjectName = state.snapshot.name,
                        onCreateProject = { name, author -> viewModel.createProject(name, author) },
                        onImportModPackage = { viewModel.requestImport(ImportKind.ModPackage) },
                        onOpenProject = { id -> viewModel.openProject(id) },
                        onExportProject = { id, name -> viewModel.requestExportModPackage(id, name) },
                        onDuplicateProject = { id -> viewModel.duplicateProject(id) },
                        onDeleteProject = { id -> viewModel.deleteProject(id) }
                    )

                    StudioScreen.Chart -> ChartScreen(
                        snapshot = state.snapshot,
                        audioPreviewPlaying = state.audioPreviewPlaying,
                        onRequestImport = { viewModel.requestImport(it) },
                        onRequestExportJson = { viewModel.requestExportChartJson() },
                        onRestoreDemoChart = { viewModel.restoreDemoChart() },
                        onSetChartBpm = { viewModel.setChartBpm(it) },
                        onUpdateChartDetails = { title, speed, p1, p2, gf, stage, voices ->
                            viewModel.updateChartDetails(title, speed, p1, p2, gf, stage, voices)
                        },
                        onSetSectionBpm = { secIdx, bpm, changeBpm ->
                            viewModel.setSectionBpm(secIdx, bpm, changeBpm)
                        },
                        onAddChartEvent = { timeMs, name, v1, v2 ->
                            viewModel.addChartEvent(timeMs, name, v1, v2)
                        },
                        onRemoveChartEvent = { idx -> viewModel.removeChartEvent(idx) },
                        onOpenStageEventsScript = { viewModel.openStageEventsScriptInStudio() },
                        onOpenUiBlackoutScript = { viewModel.openUiBlackoutScriptInStudio() },
                        onAddSection = { viewModel.addChartSection(it) },
                        onToggleSectionMustHit = { viewModel.toggleSectionMustHit(it) },
                        onToggleChartNote = { sec, step, col, sustain ->
                            viewModel.toggleChartNote(sec, step, col, sustain)
                        },
                        onSetAudioPreviewTransport = { play, seekMs ->
                            viewModel.setAudioPreviewTransport(play, seekMs)
                        },
                        onReadAudioPreviewState = { viewModel.readAudioPreviewState() }
                    )

                    StudioScreen.Audio -> AudioScreen(
                        snapshot = state.snapshot,
                        audioPreviewPlaying = state.audioPreviewPlaying,
                        onRequestImport = { viewModel.requestImport(it) },
                        onToggleAudioPreview = { viewModel.toggleAudioPreview(0.0) },
                        onUpdateTrackConfig = { slot, gain, muted, duck ->
                            viewModel.updateAudioTrackConfig(slot, gain, muted, duck)
                        },
                        onRemoveTrack = { viewModel.removeAudioTrack(it) }
                    )

                    StudioScreen.SpriteLab -> SpriteLabScreen(
                        snapshot = state.snapshot,
                        selectedEntityId = state.selectedEntityId,
                        entityBitmaps = state.entityBitmaps,
                        onSelectEntity = { viewModel.selectEntity(it) },
                        onRequestImport = { viewModel.requestImport(it) },
                        onOpenIconsDialog = { viewModel.setCharacterIconsDialogOpen(true) },
                        onOpenFilesGuide = { viewModel.setModFilesGuideDialogOpen(true) },
                        onSetBinding = { entityId, kind, prefix ->
                            viewModel.setSparrowBinding(entityId, kind, prefix)
                        },
                        onSetAnimOffset = { entityId, kind, ox, oy ->
                            viewModel.setEntityAnimOffset(entityId, kind, ox, oy)
                        },
                        onQuerySparrowFrame = { entityId, kind, elapsed ->
                            viewModel.querySparrowFrame(entityId, kind, elapsed)
                        },
                        onDecodeRegionFrame = { imagePath, frame ->
                            viewModel.decodeSparrowRegionBitmap(imagePath, frame)
                        }
                    )

                    StudioScreen.StageEditor -> StageEditorScreen(
                        snapshot = state.snapshot,
                        selectedEntityId = state.selectedEntityId,
                        entityBitmaps = state.entityBitmaps,
                        onSelectEntity = { viewModel.selectEntity(it) },
                        onAddEntity = { name, role -> viewModel.addStageEntity(name, role) },
                        onUpdateEntity = { id, name, role, dx, dy, ax, ay, useAbs, scale, op, vis, fx, commitSave ->
                            viewModel.updateStageEntity(
                                id, name, role, dx, dy, ax, ay, useAbs, scale, op, vis, fx, commitSave
                            )
                        },
                        onReorderEntity = { id, action -> viewModel.reorderStageEntity(id, action) },
                        onRequestImportPng = { entityId ->
                            viewModel.requestImport(ImportKind.SpritePng(entityId))
                        },
                        onUpdateCamera = { enabled, bx, by, bz, mode, beats, smooth, pan, commit ->
                            viewModel.updateStageCamera(enabled, bx, by, bz, mode, beats, smooth, pan, commit)
                        },
                        onOpenCameraScript = { viewModel.openCameraScriptInStudio() },
                        onImportStageConfig = { viewModel.requestImport(ImportKind.StageConfig) },
                        onOpenFilesGuide = { viewModel.setModFilesGuideDialogOpen(true) }
                    )

                    StudioScreen.ScriptStudio -> ScriptStudioScreen(
                        snapshot = state.snapshot,
                        selectedScriptId = state.selectedScriptId,
                        validationMsg = state.scriptValidationMsg,
                        validationOk = state.scriptValidationOk,
                        onSelectScript = { viewModel.selectScript(it) },
                        onAddScript = { viewModel.addScript(it) },
                        onRequestImportScript = { viewModel.requestImport(ImportKind.Script) },
                        onUpdateScript = { id, enabled, src, silent ->
                            viewModel.updateScript(id, enabled, src, silent)
                        },
                        onValidateScript = { id, enabled, src ->
                            viewModel.validateScript(id, enabled, src)
                        }
                    )

                    StudioScreen.HotCompile -> HotCompileScreen(
                        tier = state.selectedTier,
                        progress = state.hotCompileProgress
                    )

                    StudioScreen.Gameplay -> {
                        val playerEntId = state.snapshot.stageEntities.firstOrNull { it.role == "player" }?.id
                        val oppEntId = state.snapshot.stageEntities.firstOrNull { it.role == "opponent" }?.id
                        GameplayScreen(
                            frameState = viewModel.gameplayFrameState,
                            arrowBitmaps = state.arrowBitmaps,
                            receptorBitmaps = state.receptorBitmaps,
                            healthBarBitmap = state.healthBarBitmap,
                            ratingBitmaps = state.ratingBitmaps,
                            noteSplashBitmap = state.noteSplashBitmap,
                            fallbackPlayerId = playerEntId,
                            fallbackOpponentId = oppEntId,
                            entityNormalIcons = state.entityNormalIcons,
                            entityLosingIcons = state.entityLosingIcons,
                            defaultPlayerNormalIcon = state.defaultPlayerNormalIcon,
                            defaultPlayerLosingIcon = state.defaultPlayerLosingIcon,
                            defaultOpponentNormalIcon = state.defaultOpponentNormalIcon,
                            defaultOpponentLosingIcon = state.defaultOpponentLosingIcon,
                            compiledBitmapsByIndex = state.compiledBitmapsByIndex,
                            onStepFrame = { dt, pressed, justPressed ->
                                viewModel.stepGameplayFrame(dt, pressed, justPressed)
                            },
                            onChangeControlMode = { mode -> viewModel.updateControlMode(mode) },
                            onSetPaused = { viewModel.setGameplayPaused(it) },
                            onRestart = { viewModel.restartGameplay() },
                            onExitToStudio = { viewModel.stopGameplayAndNavigate(StudioScreen.Hub) }
                        )
                    }
                }

                // Banner de notificación flotante
                val banner = state.statusBanner
                if (!banner.isNullOrBlank() && state.currentScreen != StudioScreen.Gameplay) {
                    Surface(
                        modifier = Modifier
                            .align(Alignment.BottomCenter)
                            .padding(12.dp),
                        shape = RoundedCornerShape(10.dp),
                        color = StudioColors.BgElevated,
                        border = BorderStroke(
                            1.dp,
                            if (state.statusIsError) StudioColors.AccentCoral else StudioColors.AccentEmerald
                        )
                    ) {
                        Row(
                            modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically
                        ) {
                            Icon(
                                imageVector = if (state.statusIsError) Icons.Rounded.ErrorOutline else Icons.Rounded.CheckCircle,
                                contentDescription = null,
                                tint = if (state.statusIsError) StudioColors.AccentCoral else StudioColors.AccentEmerald,
                                modifier = Modifier.size(16.dp)
                            )
                            Spacer(Modifier.width(8.dp))
                            Text(banner, color = StudioColors.TextPrimary, fontSize = 12.sp)
                            Spacer(Modifier.width(8.dp))
                            IconButton(
                                onClick = { viewModel.dismissBanner() },
                                modifier = Modifier.size(24.dp)
                            ) {
                                Icon(Icons.Rounded.Close, "Cerrar", tint = StudioColors.TextMuted, modifier = Modifier.size(14.dp))
                            }
                        }
                    }
                }
            }
        }
    }

    if (state.arrowSkinDialogOpen) {
        ArrowSkinDialog(
            skinName = state.snapshot.arrowSkinName,
            customPaths = state.snapshot.customArrowPaths,
            arrowBitmaps = state.arrowBitmaps,
            onImportSvg = { dir -> viewModel.requestImport(ImportKind.ArrowSvg(dir)) },
            onRestoreDefault = { viewModel.restoreDefaultArrowSvg() },
            onDismiss = { viewModel.setArrowSkinDialogOpen(false) }
        )
    }

    if (state.characterIconsDialogOpen) {
        CharacterIconsDialog(
            entities = state.snapshot.stageEntities,
            healthBarSvgPath = state.snapshot.healthBarSvgPath,
            healthBarBitmap = state.healthBarBitmap,
            entityNormalIcons = state.entityNormalIcons,
            entityLosingIcons = state.entityLosingIcons,
            defaultPlayerNormal = state.defaultPlayerNormalIcon,
            defaultPlayerLosing = state.defaultPlayerLosingIcon,
            defaultOpponentNormal = state.defaultOpponentNormalIcon,
            defaultOpponentLosing = state.defaultOpponentLosingIcon,
            onRequestImport = { kind -> viewModel.requestImport(kind) },
            onClearEntityIcons = { entId -> viewModel.clearCharacterIcons(entId) },
            onRestoreDefaultHealthBar = { viewModel.restoreDefaultHealthBarSvg() },
            onDismiss = { viewModel.setCharacterIconsDialogOpen(false) }
        )
    }

    if (state.modFilesGuideDialogOpen) {
        ModFilesGuideDialog(
            onDismiss = { viewModel.setModFilesGuideDialogOpen(false) }
        )
    }
}
