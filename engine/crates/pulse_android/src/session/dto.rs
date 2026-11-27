//! Archivo: engine/crates/pulse_android/src/session/dto.rs
//! Propósito: Modelos de transferencia JSON (`serde::Serialize` camelCase) compartidos entre
//! Rust y Kotlin Compose, junto con la construcción de `ProjectSnapshotJson` y respuestas estándar.

use super::{EngineSession, anim_kind_key, control_mode_to_key, lang_to_key, role_to_key};
use pulse_audio::AudioAdaptationReport;
use pulse_core::frame_memory::{FrameBumpArena, FrameVisibleNote, NoteSlidingWindow};
use pulse_core::hot_compile::{CompiledBundle, EntityLifecycleWindow};
use pulse_core::project::{COUNTDOWN_STEP_COUNT, MAX_AUDIO_TRACKS, countdown_step_label};
use pulse_core::sparrow::CharacterAnimKind;
use pulse_core::NoteSpecialKind;
use pulse_scripting::ModScriptRuntime;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResultJson<T: Serialize> {
    pub ok: bool,
    pub message: String,
    pub data: Option<T>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummaryJson {
    pub id: String,
    pub name: String,
    pub author: String,
    pub chart_title: String,
    pub bpm: f64,
    pub note_count: usize,
    pub track_count: usize,
    pub entity_count: usize,
    pub script_count: usize,
    pub updated_at_ms: u64,
    pub is_active: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageCameraSlotSnapshotJson {
    pub index: usize,
    pub name: String,
    pub target_kind: String,
    pub target_entity_id: Option<String>,
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
    pub smooth_speed: f32,
    pub pan_strength: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageCameraSnapshotJson {
    pub enabled: bool,
    pub base_x: f32,
    pub base_y: f32,
    pub base_zoom: f32,
    pub follow_mode: String,
    pub follow_interval_beats: u32,
    pub smooth_speed: f32,
    pub pan_strength: f32,
    pub active_camera_index: usize,
    pub clamp_to_background: bool,
    pub vignette_enabled: bool,
    pub vignette_color: String,
    pub vignette_alpha: f32,
    pub vignette_width: f32,
    pub vignette_pulse: f32,
    pub vignette_over_hud: bool,
    pub cameras: Vec<StageCameraSlotSnapshotJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSnapshotJson {
    pub id: String,
    pub name: String,
    pub author: String,
    pub downscroll: bool,
    pub control_mode: String,
    pub audio_offset_ms: f64,
    pub auto_audio_offset_ms: f64,
    pub chart: ChartSnapshotJson,
    pub audio_tracks: Vec<AudioTrackSnapshotJson>,
    pub stage_camera: StageCameraSnapshotJson,
    pub stage_entities: Vec<StageEntitySnapshotJson>,
    pub scripts: Vec<ScriptSnapshotJson>,
    pub arrow_skin_name: String,
    pub custom_arrow_paths: HashMap<String, String>,
    pub has_custom_png_arrow_skin: bool,
    pub health_bar_svg_path: Option<String>,
    pub preferred_render_backend: String,
    pub audio_adapt_enabled: bool,
    pub countdown_slots: Vec<CountdownStepSnapshotJson>,
    pub last_compile_report: Option<CompileReportJson>,
    pub last_audio_adapt_report: Option<AudioAdaptationReport>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CountdownStepSnapshotJson {
    pub slot_index: usize,
    pub label: String,
    pub image_path: Option<String>,
    pub image_name: Option<String>,
    pub xml_name: Option<String>,
    pub frames: Vec<SparrowFrameJson>,
    pub fps: f32,
    pub audio_name: Option<String>,
    pub audio_duration_ms: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartEventJson {
    pub time_ms: f64,
    pub name: String,
    pub value1: String,
    pub value2: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSectionSnapshotJson {
    pub index: usize,
    pub start_ms: f64,
    pub length_in_steps: u32,
    pub must_hit_section: bool,
    pub bpm: f64,
    pub change_bpm: bool,
    pub alt_anim: bool,
    pub gf_section: bool,
    pub camera_slot: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartIntroUiSnapshotJson {
    pub start_blackout: bool,
    pub blackout_alpha: f32,
    pub hide_hud: bool,
    pub hide_arrows: bool,
    pub reveal_on_song_start: bool,
    pub reveal_delay_ms: f64,
    pub fade_sec: f32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChartSnapshotJson {
    pub title: String,
    pub bpm: f64,
    pub speed: f64,
    pub needs_voices: bool,
    pub player1: String,
    pub player2: String,
    pub gf_version: String,
    pub stage: String,
    pub section_count: usize,
    pub total_notes: usize,
    pub player_notes: usize,
    pub opponent_notes: usize,
    pub sustain_notes: usize,
    pub duration_ms: f64,
    pub density_buckets: Vec<u16>,
    pub source_path: Option<String>,
    pub intro_ui: ChartIntroUiSnapshotJson,
    pub sections: Vec<ChartSectionSnapshotJson>,
    pub notes: Vec<VisibleNoteJson>,
    pub events: Vec<ChartEventJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTrackSnapshotJson {
    pub slot_index: usize,
    pub role_label: String,
    pub file_name: Option<String>,
    pub gain: f32,
    pub muted: bool,
    pub duck_on_player_miss: bool,
    pub duration_ms: f64,
    pub waveform_peaks: Vec<f32>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageEntitySnapshotJson {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub role_label: String,
    pub image_path: Option<String>,
    pub raw_image_path: Option<String>,
    pub xml_path: Option<String>,
    pub animate_json_path: Option<String>,
    pub character_config_path: Option<String>,
    pub icon_normal_path: Option<String>,
    pub icon_losing_path: Option<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub opacity: f32,
    pub flip_x: bool,
    pub z_index: i32,
    pub visible: bool,
    pub animation_prefixes: Vec<String>,
    pub anim_bindings: HashMap<String, String>,
    pub anim_offsets: HashMap<String, [f32; 2]>,
    pub preview_frame: Option<SparrowFrameJson>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptSnapshotJson {
    pub id: String,
    pub name: String,
    pub language: String,
    pub language_label: String,
    pub source_code: String,
    pub enabled: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileReportJson {
    pub elapsed_ms: u64,
    pub kept_entities: usize,
    pub discarded_invisible_entities: Vec<String>,
    pub downscaled_textures: usize,
    pub skipped_optimal_textures: usize,
    pub cached_textures_reused: usize,
    pub webp_encoded_textures: usize,
    pub discarded_heavy_loops: usize,
    pub neutralized_unbounded_loops: usize,
    pub estimated_vram_saved_kb: usize,
    pub dead_frames_removed: usize,
    pub duplicate_frames_merged: usize,
    pub trimmed_frames: usize,
    pub pruned_unused_anims: usize,
    pub repacked_atlases: usize,
    pub repacked_entity_ids: Vec<String>,
    pub rgb565_textures: usize,
    pub quantized_textures: usize,
    pub split_atlas_pages_total: usize,
    pub scheduled_preload_entities: usize,
    pub scheduled_retire_entities: usize,
    pub compiled_countdown_count: usize,
    pub compiled_icon_count: usize,
    pub merged_audio_tracks: usize,
    pub compiled_audio_path: Option<String>,
    pub estimated_audio_ram_saved_kb: usize,
    pub cached_audio_reused: bool,
    pub precompiled_scripts: usize,
    pub cached_scripts_reused: usize,
    pub script_precompile_errors: Vec<String>,
    pub compiled_texture_paths: HashMap<String, String>,
    pub compiled_texture_pages: HashMap<String, Vec<String>>,
    pub texture_pixel_formats: HashMap<String, String>,
    pub compiled_countdown_paths: HashMap<String, String>,
    pub compiled_icon_normal_paths: HashMap<String, String>,
    pub compiled_icon_losing_paths: HashMap<String, String>,
    pub timeline_windows: Vec<EntityLifecycleWindow>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SparrowFrameJson {
    pub found: bool,
    pub raw_name: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub frame_x: i32,
    pub frame_y: i32,
    pub frame_width: u32,
    pub frame_height: u32,
    pub rotated: bool,
    pub page_index: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplayEntityFrameJson {
    pub id: String,
    pub display_name: String,
    pub role: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    pub opacity: f32,
    pub rotation_deg: f32,
    pub flip_x: bool,
    pub visible: bool,
    pub preload_requested: bool,
    pub retired: bool,
    pub texture_path: Option<String>,
    pub frame: Option<SparrowFrameJson>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StrumOverrideJson {
    pub offset_x: f32,
    pub offset_y: f32,
    pub angle: f32,
    pub scale: f32,
    pub alpha: f32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DynamicScriptItemJson {
    pub id: String,
    pub kind: String,
    pub is_hud: bool,
    pub visible: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub opacity: f32,
    pub angle: f32,
    pub color_argb: u32,
    pub text: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VisibleNoteJson {
    pub id: u64,
    pub time_ms: f64,
    pub direction: u8,
    pub is_player: bool,
    pub sustain_ms: f64,
    pub head_hit: bool,
    pub note_type: String,
    pub note_kind_code: u8,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameplayStepJson {
    pub active: bool,
    pub song_pos_ms: f64,
    pub duration_ms: f64,
    pub scroll_speed: f64,
    pub active_bpm: f64,
    pub beat_phase: f32,
    pub downscroll: bool,
    pub control_mode: String,
    pub holding_lanes_mask: u8,
    pub opponent_holding_lanes_mask: u8,
    pub countdown_stage: i32,
    pub countdown_label: String,
    pub countdown_progress: f32,
    pub camera_zoom: f32,
    pub camera_shake: f32,
    pub camera_x: f32,
    pub camera_y: f32,
    pub virtual_width: f32,
    pub virtual_height: f32,
    pub score: i64,
    pub combo: u32,
    pub max_combo: u32,
    pub hits: u32,
    pub misses: u32,
    pub accuracy_pct: f32,
    pub health: f32,
    pub last_judgement: String,
    pub rating_code: u8,
    pub rating_event_id: u32,
    pub splash_event_id: u32,
    pub splash_lane: u8,
    pub hud_visible: bool,
    pub hud_opacity: f32,
    pub hud_zoom: f32,
    pub arrows_visible: bool,
    pub arrows_opacity: f32,
    pub screen_blackout_alpha: f32,
    pub flash_color_argb: u32,
    pub flash_alpha: f32,
    pub flash_over_hud: bool,
    pub vignette_color_argb: u32,
    pub vignette_alpha: f32,
    pub vignette_width: f32,
    pub vignette_pulse: f32,
    pub vignette_over_hud: bool,
    pub active_player_idx: i32,
    pub active_opponent_idx: i32,
    pub paused: bool,
    pub finished: bool,
    pub game_over: bool,
    pub game_over_phase: u8,
    pub botplay: bool,
    pub practice_mode: bool,
    pub script_errors: Vec<String>,
    pub script_logs: Vec<String>,
    pub entities: Vec<GameplayEntityFrameJson>,
    pub visible_notes: Vec<VisibleNoteJson>,
    pub strum_overrides: Vec<StrumOverrideJson>,
    pub dynamic_items: Vec<DynamicScriptItemJson>,
}

#[derive(Clone, Copy)]
pub struct StrumOverrideState {
    pub offset_x: f32,
    pub offset_y: f32,
    pub angle: f32,
    pub scale: f32,
    pub alpha: f32,
}

impl Default for StrumOverrideState {
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            angle: 0.0,
            scale: 1.0,
            alpha: 1.0,
        }
    }
}

#[derive(Clone)]
pub struct DynamicDrawableState {
    pub id: String,
    pub is_text: bool,
    pub is_hud: bool,
    pub visible: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub opacity: f32,
    pub angle: f32,
    pub color_argb: u32,
    pub text: String,
}

#[derive(Clone)]
pub enum TweenTarget {
    EntityProp { entity_id: String, prop: String },
    CameraPosX,
    CameraPosY,
    CameraZoom,
    StrumProp { strum_idx: usize, prop: String },
    HudProp { prop: String },
    ArrowsProp { prop: String },
    ScreenBlackout,
    CameraVignetteAlpha,
    CameraVignetteWidth,
}

#[derive(Clone)]
pub struct ActiveTweenState {
    pub tag: String,
    pub target: TweenTarget,
    pub start_val: f32,
    pub end_val: f32,
    pub elapsed_sec: f32,
    pub duration_sec: f32,
    pub ease: String,
}

#[derive(Clone)]
pub struct ActiveTimerState {
    pub tag: String,
    pub remaining_sec: f32,
}

pub struct GameplayRuntimeState {
    pub bundle: CompiledBundle,
    pub compiled_texture_paths: HashMap<String, String>,
    pub compiled_texture_pages: HashMap<String, Vec<String>>,
    pub runtime: ModScriptRuntime,
    pub script_errors: Vec<String>,
    pub note_window: NoteSlidingWindow,
    pub frame_arena: FrameBumpArena,
    pub visible_note_scratch: Vec<FrameVisibleNote>,
    pub step_entities_scratch: Vec<GameplayEntityFrameJson>,
    pub step_notes_scratch: Vec<VisibleNoteJson>,
    pub strum_overrides: [StrumOverrideState; 8],
    pub dynamic_items: Vec<DynamicDrawableState>,
    pub active_tweens: Vec<ActiveTweenState>,
    pub active_timers: Vec<ActiveTimerState>,
    pub score: i64,
    pub combo: u32,
    pub max_combo: u32,
    pub hits: u32,
    pub misses: u32,
    pub accuracy_sum: f64,
    pub judged_count: u32,
    pub accuracy_pct: f32,
    pub health: f32,
    pub last_judgement: String,
    pub last_rating_code: u8,
    pub rating_event_id: u32,
    pub splash_event_id: u32,
    pub splash_lane: u8,
    pub hud_visible: bool,
    pub hud_opacity: f32,
    pub hud_zoom: f32,
    pub arrows_visible: bool,
    pub arrows_opacity: f32,
    pub screen_blackout_alpha: f32,
    pub active_player_id: String,
    pub active_opponent_id: String,
    pub active_camera_slot: usize,
    pub camera_slot_zoom_override: Option<f32>,
    pub camera_follow_lock: Option<[f32; 2]>,
    pub camera_gf_override: bool,
    pub camera_offset_override: [f32; 2],
    pub last_section_idx: isize,
    pub last_cam_beat_bucket: i32,
    pub flash_color_argb: u32,
    pub flash_alpha: f32,
    pub flash_decay_per_sec: f32,
    pub flash_over_hud: bool,
    pub preload_requested_entities: HashSet<String>,
    pub retired_entities: HashSet<String>,
    pub reused_later_entities: HashSet<String>,
    pub active_scroll_speed: f64,
    pub active_downscroll: bool,
    pub song_started: bool,
    pub last_beat: i32,
    pub last_step: i32,
    pub next_event_idx: usize,
    pub next_preload_event_idx: usize,
    pub timeline_preload_queue: Vec<(f64, String)>,
    pub next_timeline_preload_idx: usize,
    pub timeline_retire_queue: Vec<(f64, String)>,
    pub next_timeline_retire_idx: usize,
    pub paused: bool,
    pub finished: bool,
    pub game_over: bool,
    pub game_over_phase: u8,
    pub game_over_timer_sec: f32,
    pub botplay: bool,
    pub practice_mode: bool,
    pub last_logs: Vec<String>,
    pub last_countdown_stage: i32,
    pub hud_revision: u32,
    pub last_binary_hud_revision: u32,
}

impl EngineSession {
    pub fn snapshot_json_string(&self) -> String {
        let snap = self.build_snapshot();
        serde_json::to_string(&snap).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn build_snapshot(&self) -> ProjectSnapshotJson {
        let audio_tracks = (0..MAX_AUDIO_TRACKS)
            .map(|slot| {
                let cfg = &self.project.audio_tracks[slot];
                let decoded = self.decoded_tracks[slot].as_ref();
                AudioTrackSnapshotJson {
                    slot_index: slot,
                    role_label: cfg.role_label.clone(),
                    file_name: cfg.file_name.clone(),
                    gain: cfg.gain,
                    muted: cfg.muted,
                    duck_on_player_miss: cfg.duck_on_player_miss,
                    duration_ms: decoded.map(|d| d.duration_ms()).unwrap_or(0.0),
                    waveform_peaks: decoded.map(|d| d.waveform_peaks.clone()).unwrap_or_default(),
                }
            })
            .collect();

        let stage_entities = self
            .project
            .stage
            .entities
            .iter()
            .map(|ent| {
                let prefixes = ent
                    .atlas
                    .as_ref()
                    .map(|a| a.animations.keys().cloned().collect())
                    .unwrap_or_default();
                let mut bindings = HashMap::new();
                if let Some(atlas) = &ent.atlas {
                    for (kind, pref) in &atlas.bindings {
                        bindings.insert(anim_kind_key(*kind).to_string(), pref.clone());
                    }
                }
                let mut anim_offsets = HashMap::new();
                for (kind, off) in &ent.anim_offsets {
                    anim_offsets.insert(anim_kind_key(*kind).to_string(), *off);
                }
                let preview_frame = ent
                    .atlas
                    .as_ref()
                    .and_then(|a| {
                        a.frame_at(CharacterAnimKind::Idle, 0.0).or_else(|| {
                            a.animations.values().next().and_then(|anim| anim.frames.first())
                        })
                    })
                    .map(|f| SparrowFrameJson {
                        found: true,
                        raw_name: f.raw_name.clone(),
                        x: f.x,
                        y: f.y,
                        width: f.width,
                        height: f.height,
                        frame_x: f.frame_x,
                        frame_y: f.frame_y,
                        frame_width: f.frame_width,
                        frame_height: f.frame_height,
                        rotated: f.rotated,
                        page_index: f.page_index,
                    });
                StageEntitySnapshotJson {
                    id: ent.id.clone(),
                    display_name: ent.display_name.clone(),
                    role: role_to_key(ent.role).to_string(),
                    role_label: ent.role.label_es().to_string(),
                    image_path: ent
                        .image_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                    raw_image_path: ent
                        .raw_image_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                    xml_path: ent
                        .xml_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                    animate_json_path: ent
                        .animate_json_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                    character_config_path: ent
                        .character_config_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                    icon_normal_path: ent
                        .icon_normal_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                    icon_losing_path: ent
                        .icon_losing_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                    x: ent.x,
                    y: ent.y,
                    width: ent.width,
                    height: ent.height,
                    scale: ent.scale,
                    opacity: ent.opacity,
                    flip_x: ent.flip_x,
                    z_index: ent.z_index,
                    visible: ent.visible,
                    animation_prefixes: prefixes,
                    anim_bindings: bindings,
                    anim_offsets,
                    preview_frame,
                }
            })
            .collect();

        let scripts = self
            .project
            .scripts
            .iter()
            .map(|s| ScriptSnapshotJson {
                id: s.id.clone(),
                name: s.name.clone(),
                language: lang_to_key(s.language).to_string(),
                language_label: s.language.label().to_string(),
                source_code: s.source_code.clone(),
                enabled: s.enabled,
            })
            .collect();

        let mut custom_arrow_paths = HashMap::new();
        for (dir, path) in &self.project.arrow_skin.custom_svg_paths {
            custom_arrow_paths.insert(
                format!("{}", *dir as u8),
                path.to_string_lossy().into_owned(),
            );
        }

        let last_compile_report = self.last_compile_report.as_ref().map(|r| CompileReportJson {
            elapsed_ms: r.elapsed_ms,
            kept_entities: r.kept_entities,
            discarded_invisible_entities: r.discarded_invisible_entities.clone(),
            downscaled_textures: r.downscaled_textures,
            skipped_optimal_textures: r.skipped_optimal_textures,
            cached_textures_reused: r.cached_textures_reused,
            webp_encoded_textures: r.webp_encoded_textures,
            discarded_heavy_loops: r.discarded_heavy_loops,
            neutralized_unbounded_loops: r.neutralized_unbounded_loops,
            estimated_vram_saved_kb: r.estimated_vram_saved_kb,
            dead_frames_removed: r.dead_frames_removed,
            duplicate_frames_merged: r.duplicate_frames_merged,
            trimmed_frames: r.trimmed_frames,
            pruned_unused_anims: r.pruned_unused_anims,
            repacked_atlases: r.repacked_atlases,
            repacked_entity_ids: r.repacked_entity_ids.clone(),
            rgb565_textures: r.rgb565_textures,
            quantized_textures: r.quantized_textures,
            split_atlas_pages_total: r.split_atlas_pages_total,
            scheduled_preload_entities: r.scheduled_preload_entities,
            scheduled_retire_entities: r.scheduled_retire_entities,
            compiled_countdown_count: r.compiled_countdown_count,
            compiled_icon_count: r.compiled_icon_count,
            merged_audio_tracks: r.merged_audio_tracks,
            compiled_audio_path: r
                .compiled_audio_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            estimated_audio_ram_saved_kb: r.estimated_audio_ram_saved_kb,
            cached_audio_reused: r.cached_audio_reused,
            precompiled_scripts: r.precompiled_scripts,
            cached_scripts_reused: r.cached_scripts_reused,
            script_precompile_errors: r.script_precompile_errors.clone(),
            compiled_texture_paths: self.compiled_texture_paths.clone(),
            compiled_texture_pages: self.compiled_texture_pages.clone(),
            texture_pixel_formats: self.texture_pixel_formats.clone(),
            compiled_countdown_paths: self.compiled_countdown_paths.clone(),
            compiled_icon_normal_paths: self.compiled_icon_normal_paths.clone(),
            compiled_icon_losing_paths: self.compiled_icon_losing_paths.clone(),
            timeline_windows: self.last_timeline_windows.clone(),
        });

        let cam_cfg = &self.project.stage.camera;
        let cameras: Vec<StageCameraSlotSnapshotJson> = cam_cfg
            .cameras
            .iter()
            .map(|c| StageCameraSlotSnapshotJson {
                index: c.index,
                name: c.name.clone(),
                target_kind: c.target_kind.as_key().to_string(),
                target_entity_id: c.target_entity_id.clone(),
                x: c.x,
                y: c.y,
                zoom: c.zoom,
                smooth_speed: c.smooth_speed,
                pan_strength: c.pan_strength,
            })
            .collect();
        let stage_camera = StageCameraSnapshotJson {
            enabled: cam_cfg.enabled,
            base_x: cam_cfg.base_x,
            base_y: cam_cfg.base_y,
            base_zoom: cam_cfg.base_zoom,
            follow_mode: cam_cfg.follow_mode.as_lua_str().to_string(),
            follow_interval_beats: cam_cfg.follow_interval_beats,
            smooth_speed: cam_cfg.smooth_speed,
            pan_strength: cam_cfg.pan_strength,
            active_camera_index: cam_cfg.active_camera_index,
            clamp_to_background: cam_cfg.clamp_to_background,
            vignette_enabled: cam_cfg.vignette_enabled,
            vignette_color: cam_cfg.vignette_color.clone(),
            vignette_alpha: cam_cfg.vignette_alpha,
            vignette_width: cam_cfg.vignette_width,
            vignette_pulse: cam_cfg.vignette_pulse,
            vignette_over_hud: cam_cfg.vignette_over_hud,
            cameras,
        };

        let countdown_slots: Vec<CountdownStepSnapshotJson> = (0..COUNTDOWN_STEP_COUNT)
            .map(|slot| {
                let cd = &self.project.countdown.slots[slot];
                let decoded_audio = self.decoded_countdown_cues[slot].as_ref();
                let frames = cd
                    .frames
                    .iter()
                    .map(|f| SparrowFrameJson {
                        found: true,
                        raw_name: f.raw_name.clone(),
                        x: f.x,
                        y: f.y,
                        width: f.width,
                        height: f.height,
                        frame_x: f.frame_x,
                        frame_y: f.frame_y,
                        frame_width: f.frame_width,
                        frame_height: f.frame_height,
                        rotated: f.rotated,
                        page_index: f.page_index,
                    })
                    .collect();
                CountdownStepSnapshotJson {
                    slot_index: slot,
                    label: countdown_step_label(slot).to_string(),
                    image_path: cd
                        .image_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().into_owned()),
                    image_name: cd.image_name.clone(),
                    xml_name: cd
                        .xml_path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .and_then(|n| n.to_str())
                        .map(|s| s.to_string()),
                    frames,
                    fps: cd.fps,
                    audio_name: cd.audio_name.clone(),
                    audio_duration_ms: decoded_audio.map(|d| d.duration_ms()).unwrap_or(0.0),
                }
            })
            .collect();

        ProjectSnapshotJson {
            id: self.project.id.clone(),
            name: self.project.name.clone(),
            author: self.project.author.clone(),
            downscroll: self.project.downscroll,
            control_mode: control_mode_to_key(self.project.control_mode).to_string(),
            audio_offset_ms: self.project.audio_offset_ms,
            auto_audio_offset_ms: self.auto_audio_offset_ms,
            chart: ChartSnapshotJson {
                title: self.project.chart.title.clone(),
                bpm: self.project.chart.bpm,
                speed: self.project.chart.speed,
                needs_voices: self.project.chart.needs_voices,
                player1: self.project.chart.player1.clone(),
                player2: self.project.chart.player2.clone(),
                gf_version: self.project.chart.gf_version.clone(),
                stage: self.project.chart.stage.clone(),
                section_count: self.project.chart.sections.len(),
                total_notes: self.project.chart.stats.total_notes,
                player_notes: self.project.chart.stats.player_notes,
                opponent_notes: self.project.chart.stats.opponent_notes,
                sustain_notes: self.project.chart.stats.sustain_notes,
                duration_ms: self.project.chart.stats.duration_ms,
                density_buckets: self.project.chart.stats.density_buckets.clone(),
                source_path: self
                    .project
                    .chart_source_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned()),
                intro_ui: ChartIntroUiSnapshotJson {
                    start_blackout: self.project.chart.intro_ui.start_blackout,
                    blackout_alpha: self.project.chart.intro_ui.blackout_alpha,
                    hide_hud: self.project.chart.intro_ui.hide_hud,
                    hide_arrows: self.project.chart.intro_ui.hide_arrows,
                    reveal_on_song_start: self.project.chart.intro_ui.reveal_on_song_start,
                    reveal_delay_ms: self.project.chart.intro_ui.reveal_delay_ms,
                    fade_sec: self.project.chart.intro_ui.fade_sec,
                },
                sections: self
                    .project
                    .chart
                    .sections
                    .iter()
                    .map(|s| ChartSectionSnapshotJson {
                        index: s.index,
                        start_ms: s.start_ms,
                        length_in_steps: s.length_in_steps,
                        must_hit_section: s.must_hit_section,
                        bpm: s.bpm,
                        change_bpm: s.change_bpm,
                        alt_anim: s.alt_anim,
                        gf_section: s.gf_section,
                        camera_slot: s.camera_slot,
                    })
                    .collect(),
                notes: self
                    .project
                    .chart
                    .notes
                    .iter()
                    .map(|n| VisibleNoteJson {
                        id: n.id,
                        time_ms: n.time_ms,
                        direction: n.direction as u8,
                        is_player: n.is_player,
                        sustain_ms: n.sustain_ms,
                        head_hit: false,
                        note_type: n.note_type.clone(),
                        note_kind_code: NoteSpecialKind::from_str_loose(&n.note_type).code(),
                    })
                    .collect(),
                events: self
                    .project
                    .chart
                    .events
                    .iter()
                    .map(|e| ChartEventJson {
                        time_ms: e.time_ms,
                        name: e.name.clone(),
                        value1: e.value1.clone(),
                        value2: e.value2.clone(),
                    })
                    .collect(),
            },
            audio_tracks,
            stage_camera,
            stage_entities,
            scripts,
            arrow_skin_name: self.project.arrow_skin.skin_name.clone(),
            custom_arrow_paths,
            has_custom_png_arrow_skin: self.project.arrow_skin.has_custom_png_atlas(),
            health_bar_svg_path: self
                .project
                .health_bar_svg_path
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned()),
            preferred_render_backend: self.project.preferred_render_backend.as_key().to_string(),
            audio_adapt_enabled: self.project.audio_adapt_enabled,
            countdown_slots,
            last_compile_report,
            last_audio_adapt_report: self.last_audio_adapt_report.clone(),
        }
    }

    pub fn ok_response(&self, message: impl Into<String>) -> String {
        let res = ActionResultJson {
            ok: true,
            message: message.into(),
            data: Some(self.build_snapshot()),
        };
        serde_json::to_string(&res).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn err_response(&self, message: impl Into<String>) -> String {
        let res = ActionResultJson {
            ok: false,
            message: message.into(),
            data: Some(self.build_snapshot()),
        };
        serde_json::to_string(&res).unwrap_or_else(|_| "{}".to_string())
    }
}
