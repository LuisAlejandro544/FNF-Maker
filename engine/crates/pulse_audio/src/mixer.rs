//! Archivo: engine/crates/pulse_audio/src/mixer.rs
//! Propósito: Mezclador en tiempo real lock-free para hasta 3 pistas simultáneas
//! (1: Mix completo o Instrumental, 2: Voces Player, 3: Voces Enemigo).
//! Incluye cuenta regresiva sincronizada al BPM ("3, 2, 1, ¡YA!") con síntesis
//! de tonos/formantes vocales libres de derechos antes de iniciar la pista en `0.0 ms`,
//! manteniendo un reloj maestro basado en frames (`AtomicU64`) compatible con 32 y 64 bits.

use crate::decoder::DecodedTrack;
use parking_lot::RwLock;
use pulse_core::project::MAX_AUDIO_TRACKS;
use std::f32::consts::PI;
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

/// Estado compartido entre el hilo de renderizado (UI/Gameplay) y el callback de audio AAudio.
pub struct AudioMixerState {
    pub sample_rate: AtomicU32,
    pub is_playing: AtomicBool,
    pub use_demo_synth: AtomicBool,
    pub demo_bpm_bits: AtomicU32,
    pub player_missed_recently: AtomicBool,
    pub frame_cursor: AtomicU64,
    pub countdown_total_frames: AtomicU64,
    pub countdown_remaining_frames: AtomicU64,
    pub countdown_beat_frames: AtomicU64,
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
            countdown_total_frames: AtomicU64::new(0),
            countdown_remaining_frames: AtomicU64::new(0),
            countdown_beat_frames: AtomicU64::new(0),
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

    /// Cancela cualquier cuenta regresiva pendiente (útil al previsualizar en el editor de chart/audio).
    pub fn clear_countdown(&self) {
        self.countdown_remaining_frames.store(0, Ordering::Release);
        self.countdown_total_frames.store(0, Ordering::Release);
    }

    /// Inicia una cuenta regresiva de 4 pulsos ("3, 2, 1, ¡YA!") sincronizada exactamente al BPM.
    pub fn start_countdown(&self, bpm: f64) {
        let sr = self.sample_rate.load(Ordering::Relaxed).max(8000) as f64;
        let safe_bpm = bpm.clamp(40.0, 320.0);
        let beat_frames = ((60.0 / safe_bpm) * sr).round().max(1.0) as u64;
        let total_frames = beat_frames * 4;
        self.frame_cursor.store(0, Ordering::Release);
        self.countdown_beat_frames.store(beat_frames, Ordering::Release);
        self.countdown_total_frames.store(total_frames, Ordering::Release);
        self.countdown_remaining_frames.store(total_frames, Ordering::Release);
    }

    pub fn seek_ms(&self, ms: f64) {
        self.clear_countdown();
        let sr = self.sample_rate.load(Ordering::Relaxed) as f64;
        let frame = ((ms.max(0.0) / 1000.0) * sr).round() as u64;
        self.frame_cursor.store(frame, Ordering::Release);
    }

    /// Devuelve el tiempo actual en milisegundos (negativo durante la cuenta regresiva 3-2-1-¡YA!).
    pub fn position_ms(&self) -> f64 {
        let sr = self.sample_rate.load(Ordering::Relaxed).max(1) as f64;
        let rem = self.countdown_remaining_frames.load(Ordering::Acquire);
        if rem > 0 {
            return -((rem as f64) / sr) * 1000.0;
        }
        let frames = self.frame_cursor.load(Ordering::Acquire) as f64;
        (frames / sr) * 1000.0
    }

    /// Llena el buffer de salida estéreo (llamado directamente desde el callback de audio AAudio).
    pub fn render_audio_frames(&self, out_interleaved: &mut [f32]) {
        if !self.is_playing.load(Ordering::Acquire) {
            out_interleaved.fill(0.0);
            return;
        }

        let total_requested = out_interleaved.len() / 2;
        if total_requested == 0 {
            return;
        }
        let sr = self.sample_rate.load(Ordering::Relaxed).max(1) as f32;
        out_interleaved.fill(0.0);

        let rem_cd = self.countdown_remaining_frames.load(Ordering::Acquire);
        let mut music_offset_frames = 0usize;

        if rem_cd > 0 {
            let total_cd = self.countdown_total_frames.load(Ordering::Relaxed).max(rem_cd);
            let beat_frames = self.countdown_beat_frames.load(Ordering::Relaxed).max(1);
            let cd_frames = total_requested.min(rem_cd as usize);
            let elapsed_start = total_cd.saturating_sub(rem_cd);

            for f in 0..cd_frames {
                let elapsed = elapsed_start + (f as u64);
                let beat_idx = (elapsed / beat_frames).min(3);
                let in_beat = elapsed % beat_frames;
                let t = (in_beat as f32) / sr;
                let sample = synthesize_countdown_cue(beat_idx as u8, t);
                let clipped = sample.tanh();
                out_interleaved[f * 2] = clipped;
                out_interleaved[f * 2 + 1] = clipped;
            }

            let new_rem = rem_cd.saturating_sub(cd_frames as u64);
            self.countdown_remaining_frames.store(new_rem, Ordering::Release);
            music_offset_frames = cd_frames;
            if music_offset_frames >= total_requested {
                return;
            }
        }

        let frames_requested = total_requested - music_offset_frames;
        let slice_out = &mut out_interleaved[music_offset_frames * 2..];
        let cursor = self.frame_cursor.load(Ordering::Relaxed) as usize;
        let miss_duck = self.player_missed_recently.load(Ordering::Relaxed);

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
                        slice_out[f * 2] += track.stereo_samples[idx * 2] * gain;
                        slice_out[f * 2 + 1] += track.stereo_samples[idx * 2 + 1] * gain;
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
                let tone = (2.0 * PI * freq * (sample_pos / sr)).sin() * env * 0.25;
                slice_out[f * 2] += tone;
                slice_out[f * 2 + 1] += tone;
            }
        }

        // Limitador suave (soft clip) para evitar distorsión al sumar 3 pistas fuertes.
        for s in slice_out.iter_mut() {
            *s = (*s).tanh();
        }

        self.frame_cursor
            .fetch_add(frames_requested as u64, Ordering::Release);
    }
}

/// Sintetiza de forma determinista y libre de derechos las 4 señales de voz/formante:
/// 0 -> "3", 1 -> "2", 2 -> "1", 3 -> "¡YA!" (acorde ascendente brillante).
#[inline]
fn synthesize_countdown_cue(beat_idx: u8, t: f32) -> f32 {
    if t > 0.34 {
        return 0.0;
    }
    let attack = (t / 0.012).min(1.0);
    let decay_rate = if beat_idx >= 3 { 9.5 } else { 14.0 };
    let env = attack * (-t * decay_rate).exp();

    match beat_idx {
        0 => {
            // "3" — tono cálido con formante vocal grave-medio
            let f0 = 329.63; // E4
            let base = (2.0 * PI * f0 * t).sin() * 0.55;
            let formant1 = (2.0 * PI * (f0 * 2.0) * t).sin() * 0.30;
            let formant2 = (2.0 * PI * 980.0 * t).sin() * 0.16 * (-t * 22.0).exp();
            (base + formant1 + formant2) * env * 0.42
        }
        1 => {
            // "2" — tono medio ascendente
            let f0 = 392.00; // G4
            let base = (2.0 * PI * f0 * t).sin() * 0.55;
            let formant1 = (2.0 * PI * (f0 * 2.0) * t).sin() * 0.30;
            let formant2 = (2.0 * PI * 1180.0 * t).sin() * 0.16 * (-t * 22.0).exp();
            (base + formant1 + formant2) * env * 0.44
        }
        2 => {
            // "1" — tono firme de preparación
            let f0 = 440.00; // A4
            let base = (2.0 * PI * f0 * t).sin() * 0.55;
            let formant1 = (2.0 * PI * (f0 * 2.0) * t).sin() * 0.32;
            let formant2 = (2.0 * PI * 1350.0 * t).sin() * 0.18 * (-t * 20.0).exp();
            (base + formant1 + formant2) * env * 0.46
        }
        _ => {
            // "¡YA!" — acorde brillante de arranque con barrido vocal ascendente
            let sweep = 523.25 + 90.0 * (1.0 - (-t * 16.0).exp());
            let c1 = (2.0 * PI * sweep * t).sin() * 0.48;
            let c2 = (2.0 * PI * 659.25 * t).sin() * 0.32; // E5
            let c3 = (2.0 * PI * 783.99 * t).sin() * 0.25; // G5
            let shimmer = (2.0 * PI * 1567.98 * t).sin() * 0.14 * (-t * 18.0).exp();
            (c1 + c2 + c3 + shimmer) * env * 0.48
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_respects_bpm_and_transitions_seamlessly_to_zero() {
        let mixer = AudioMixerState::new(48_000);
        // 120 BPM => 500 ms por beat => 4 beats = 2000 ms totales (-2000.0 ms al iniciar)
        mixer.start_countdown(120.0);
        mixer.play();

        assert!((mixer.position_ms() - (-2000.0)).abs() < 0.1);

        // Renderizar exactamente 1 segundo (48,000 frames stereo) -> mitad de cuenta regresiva
        let mut buf = vec![0.0_f32; 48_000 * 2];
        mixer.render_audio_frames(&mut buf);
        assert!((mixer.position_ms() - (-1000.0)).abs() < 0.1);
        assert!(buf.iter().any(|s| s.abs() > 0.01));

        // Renderizar otro segundo + 240 frames (5 ms) -> debe cruzar a +5.0 ms en música
        let mut buf2 = vec![0.0_f32; (48_000 + 240) * 2];
        mixer.render_audio_frames(&mut buf2);
        assert!((mixer.position_ms() - 5.0).abs() < 0.1);
    }
}
