//! Archivo: engine/crates/pulse_audio/src/mixer.rs
//! Propósito: Mezclador en tiempo real lock-free para hasta 3 pistas simultáneas
//! (1: Mix completo o Instrumental, 2: Voces Player, 3: Voces Enemigo).
//! Mantiene un reloj maestro basado en frames reproducidos (`AtomicU64`) compatible
//! con CPU de 32-bit y 64-bit para que la caída de las flechas esté perfectamente
//! sincronizada con el audio de Oboe.

use crate::decoder::DecodedTrack;
use parking_lot::RwLock;
use pulse_core::project::MAX_AUDIO_TRACKS;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// Configuración dinámica de ganancia/mute para una pista en tiempo real.
#[derive(Debug)]
pub struct TrackRuntimeConfig {
    pub gain_bits: AtomicU32,
    pub muted: AtomicBool,
    pub duck_on_miss: AtomicBool,
}

impl Default for TrackRuntimeConfig {
    fn default() -> Self {
        Self {
            gain_bits: AtomicU32::new(1.0_f32.to_bits()),
            muted: AtomicBool::new(false),
            duck_on_miss: AtomicBool::new(false),
        }
    }
}

impl TrackRuntimeConfig {
    pub fn set_gain(&self, gain: f32) {
        self.gain_bits
            .store(gain.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
    }

    pub fn gain(&self) -> f32 {
        f32::from_bits(self.gain_bits.load(Ordering::Relaxed))
    }
}

/// Estado compartido entre el hilo de renderizado (UI/Gameplay) y el callback de Oboe.
pub struct AudioMixerState {
    pub sample_rate: AtomicU32,
    pub is_playing: AtomicBool,
    pub use_demo_synth: AtomicBool,
    pub demo_bpm_bits: AtomicU32,
    pub player_missed_recently: AtomicBool,
    pub frame_cursor: AtomicU64,
    pub tracks: RwLock<[Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS]>,
    pub configs: [TrackRuntimeConfig; MAX_AUDIO_TRACKS],
}

impl AudioMixerState {
    pub fn new(sample_rate: u32) -> Arc<Self> {
        Arc::new(Self {
            sample_rate: AtomicU32::new(sample_rate),
            is_playing: AtomicBool::new(false),
            use_demo_synth: AtomicBool::new(false),
            demo_bpm_bits: AtomicU32::new(128.0_f32.to_bits()),
            player_missed_recently: AtomicBool::new(false),
            frame_cursor: AtomicU64::new(0),
            tracks: RwLock::new([None, None, None]),
            configs: [
                TrackRuntimeConfig::default(),
                TrackRuntimeConfig::default(),
                TrackRuntimeConfig::default(),
            ],
        })
    }

    pub fn set_track(&self, slot: usize, track: Option<Arc<DecodedTrack>>) {
        if slot < MAX_AUDIO_TRACKS {
            let mut guard = self.tracks.write();
            guard[slot] = track;
        }
    }

    pub fn play(&self) {
        self.is_playing.store(true, Ordering::Release);
    }

    pub fn pause(&self) {
        self.is_playing.store(false, Ordering::Release);
    }

    pub fn seek_ms(&self, ms: f64) {
        let sr = self.sample_rate.load(Ordering::Relaxed) as f64;
        let frame = ((ms.max(0.0) / 1000.0) * sr).round() as u64;
        self.frame_cursor.store(frame, Ordering::Release);
    }

    /// Devuelve el tiempo actual de la canción en milisegundos con precisión de muestra.
    pub fn position_ms(&self) -> f64 {
        let frames = self.frame_cursor.load(Ordering::Acquire) as f64;
        let sr = self.sample_rate.load(Ordering::Relaxed).max(1) as f64;
        (frames / sr) * 1000.0
    }

    /// Llena el buffer de salida estéreo (llamado directamente desde el hilo de audio Oboe).
    pub fn render_audio_frames(&self, out_interleaved: &mut [f32]) {
        if !self.is_playing.load(Ordering::Acquire) {
            out_interleaved.fill(0.0);
            return;
        }

        let frames_requested = out_interleaved.len() / 2;
        let cursor = self.frame_cursor.load(Ordering::Relaxed) as usize;
        let sr = self.sample_rate.load(Ordering::Relaxed).max(1) as f32;
        let miss_duck = self.player_missed_recently.load(Ordering::Relaxed);

        out_interleaved.fill(0.0);

        let mut any_track = false;
        if let Some(guard) = self.tracks.try_read() {
            for slot in 0..MAX_AUDIO_TRACKS {
                if let Some(track) = &guard[slot] {
                    any_track = true;
                    if self.configs[slot].muted.load(Ordering::Relaxed) {
                        continue;
                    }
                    let mut gain = self.configs[slot].gain();
                    if miss_duck && self.configs[slot].duck_on_miss.load(Ordering::Relaxed) {
                        gain *= 0.12;
                    }

                    let total_frames = track.frame_count();
                    for f in 0..frames_requested {
                        let idx = cursor + f;
                        if idx >= total_frames {
                            break;
                        }
                        out_interleaved[f * 2] += track.stereo_samples[idx * 2] * gain;
                        out_interleaved[f * 2 + 1] += track.stereo_samples[idx * 2 + 1] * gain;
                    }
                }
            }
        }

        // Si el usuario está probando el chart demo sin pistas importadas, sintetiza un beat limpio.
        if !any_track && self.use_demo_synth.load(Ordering::Relaxed) {
            let bpm = f32::from_bits(self.demo_bpm_bits.load(Ordering::Relaxed)).max(60.0);
            let beat_samples = (60.0 / bpm) * sr;
            for f in 0..frames_requested {
                let sample_pos = (cursor + f) as f32;
                let in_beat = sample_pos % beat_samples;
                let env = (-in_beat / (sr * 0.045)).exp();
                let freq = if ((sample_pos / beat_samples) as u32) % 4 == 0 {
                    220.0
                } else {
                    165.0
                };
                let tone = (2.0 * std::f32::consts::PI * freq * (sample_pos / sr)).sin() * env * 0.25;
                out_interleaved[f * 2] += tone;
                out_interleaved[f * 2 + 1] += tone;
            }
        }

        // Limitador suave (soft clip) para evitar distorsión al sumar 3 pistas fuertes.
        for s in out_interleaved.iter_mut() {
            *s = (*s).tanh();
        }

        self.frame_cursor
            .fetch_add(frames_requested as u64, Ordering::Release);
    }
}
