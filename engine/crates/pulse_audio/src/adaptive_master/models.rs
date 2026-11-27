//! Archivo: engine/crates/pulse_audio/src/adaptive_master/models.rs
//! Propósito: Estructuras de datos, rutas de salida Android y modelos JSON de
//! telemetría acústica, análisis espectral y plan DSP para `adaptive_master`.

use serde::{Deserialize, Serialize};

/// Ruta física de salida detectada por Android (`AudioDeviceInfo`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AndroidOutputRoute {
    #[default]
    BuiltinSpeaker,
    WiredHeadphones,
    UsbAudio,
    BluetoothA2dp,
    BluetoothLe,
    HdmiOrExternal,
    Unknown,
}

impl AndroidOutputRoute {
    pub fn from_key(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "builtin_speaker" | "speaker" => Self::BuiltinSpeaker,
            "wired_headphones" | "wired_headset" | "headphones" => Self::WiredHeadphones,
            "usb_audio" | "usb" | "usb_headset" => Self::UsbAudio,
            "bluetooth_a2dp" | "a2dp" => Self::BluetoothA2dp,
            "bluetooth_le" | "ble_audio" => Self::BluetoothLe,
            "hdmi_or_external" | "hdmi" | "external" => Self::HdmiOrExternal,
            _ => Self::Unknown,
        }
    }

    pub fn label_es(self) -> &'static str {
        match self {
            Self::BuiltinSpeaker => "Altavoz del Teléfono",
            Self::WiredHeadphones => "Audífonos por Cable",
            Self::UsbAudio => "DAC / Audífonos USB-C",
            Self::BluetoothA2dp => "Bluetooth A2DP",
            Self::BluetoothLe => "Bluetooth LE Audio",
            Self::HdmiOrExternal => "HDMI / Salida Externa",
            Self::Unknown => "Salida Estándar",
        }
    }
}

/// Datos recolectados en `Dispatchers.IO` desde Android `AudioManager` + `AudioDeviceInfo` + `Build`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceAudioProbeInput {
    #[serde(default)]
    pub route: AndroidOutputRoute,
    #[serde(default)]
    pub device_name: String,
    #[serde(default)]
    pub manufacturer: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_android_api")]
    pub android_api_level: u32,
    #[serde(default = "default_sr")]
    pub os_sample_rate: u32,
    #[serde(default = "default_burst")]
    pub os_frames_per_buffer: u32,
    #[serde(default = "default_channels")]
    pub reported_channel_count: u32,
    #[serde(default)]
    pub is_mono_speaker: bool,
    #[serde(default)]
    pub supports_low_latency_feature: bool,
    #[serde(default)]
    pub supports_pro_audio_feature: bool,
    #[serde(default)]
    pub supported_sample_rates: Vec<u32>,
    #[serde(default)]
    pub aaudio_matched: bool,
}

fn default_android_api() -> u32 {
    26
}
fn default_sr() -> u32 {
    48_000
}
fn default_burst() -> u32 {
    192
}
fn default_channels() -> u32 {
    2
}

impl Default for DeviceAudioProbeInput {
    fn default() -> Self {
        Self {
            route: AndroidOutputRoute::BuiltinSpeaker,
            device_name: "Altavoz Integrado".to_string(),
            manufacturer: "Android".to_string(),
            model: "Generic".to_string(),
            android_api_level: 26,
            os_sample_rate: 48_000,
            os_frames_per_buffer: 192,
            reported_channel_count: 2,
            is_mono_speaker: false,
            supports_low_latency_feature: false,
            supports_pro_audio_feature: false,
            supported_sample_rates: vec![48_000],
            aaudio_matched: false,
        }
    }
}

/// Fusión autoritativa entre el sondeo Java (`AudioManager`) y el stream nativo Rust (`AAudio`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveDeviceAcoustics {
    pub route: AndroidOutputRoute,
    pub route_label: String,
    pub device_name: String,
    pub manufacturer: String,
    pub model: String,
    pub android_api_level: u32,
    pub effective_sample_rate: u32,
    pub hardware_native_sample_rate: u32,
    pub effective_channel_count: u32,
    pub is_mono_speaker: bool,
    pub frames_per_burst: u32,
    pub buffer_capacity_frames: u32,
    pub low_latency_active: bool,
    pub aaudio_active: bool,
    pub aaudio_device_id: i32,
    #[serde(default)]
    pub aaudio_matched: bool,
    #[serde(default)]
    pub estimated_output_latency_ms: f32,
    #[serde(default)]
    pub recommended_audio_offset_ms: f64,
}

/// Distribución de energía espectral calculada con `realfft` + ventana Hann.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpectralBandProfile {
    pub sub_bass_20_90_hz: f32,
    pub bass_90_220_hz: f32,
    pub low_mid_220_600_hz: f32,
    pub presence_2k_4k_hz: f32,
    pub air_6k_14k_hz: f32,
    pub spectral_tilt_db: f32,
}

/// Análisis real de la canción en Rust mediante `ebur128` + `realfft`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SongAcousticAnalysis {
    pub sample_rate: u32,
    pub duration_sec: f32,
    pub integrated_lufs: f32,
    pub loudness_range_lu: f32,
    pub true_peak_dbtp: f32,
    pub crest_factor_db: f32,
    pub stereo_correlation: f32,
    pub bands: SpectralBandProfile,
}

/// Banda individual de ecualización IIR aplicada con `biquad`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EqBandAdjustment {
    pub kind: String,
    pub freq_hz: f32,
    pub gain_db: f32,
    pub q: f32,
    pub reason: String,
}

fn default_stereo_width() -> f32 {
    1.0
}
fn default_lookahead_ms() -> f32 {
    4.0
}
fn default_true() -> bool {
    true
}

/// Plan de masterización adaptativa derivado para este teléfono y canción concretos.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdaptiveDspPlan {
    pub profile_name: String,
    pub adaptation_summary: String,
    pub target_lufs: f32,
    pub applied_makeup_gain_db: f32,
    pub ceiling_dbtp: f32,
    pub highpass_hz: f32,
    pub mono_fold_enabled: bool,
    pub resample_needed: bool,
    pub source_sample_rate: u32,
    pub target_sample_rate: u32,
    #[serde(default = "default_stereo_width")]
    pub stereo_width: f32,
    #[serde(default)]
    pub crossfeed_strength: f32,
    #[serde(default = "default_lookahead_ms")]
    pub limiter_lookahead_ms: f32,
    #[serde(default = "default_true")]
    pub dither_applied: bool,
    #[serde(default)]
    pub high_precision_f32_path: bool,
    pub eq_bands: Vec<EqBandAdjustment>,
}

/// Reporte final enviado a la UI (`AudioAdaptScreen`) tras adaptar o reutilizar caché.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioAdaptationReport {
    pub enabled: bool,
    pub bypassed: bool,
    pub cache_reused: bool,
    pub elapsed_ms: u64,
    pub output_wav_path: Option<String>,
    pub device: EffectiveDeviceAcoustics,
    pub song_analysis: Option<SongAcousticAnalysis>,
    pub dsp_plan: Option<AdaptiveDspPlan>,
    pub output_true_peak_dbtp: f32,
    #[serde(default)]
    pub applied_auto_offset_ms: f64,
}
