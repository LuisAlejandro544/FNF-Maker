//! Archivo: engine/crates/pulse_audio/src/mixer.rs
//! Propósito: Mezclador en tiempo real lock-free para:
//! - Modo estudio/editor: hasta 3 pistas compactas `i16` en RAM (`tracks`) con soft-clip `tanh()`.
//! - Modo gameplay compilado: 1 único stream en disco (`stream_source`) vía buffer circular
//!   lock-free de ~512 KB, pasando linealmente sin `tanh()` para preservar la masterización True Peak.
//! Incluye cuenta regresiva sincronizada al BPM ("3, 2, 1, ¡YA!") con reloj maestro `AtomicU64`.

use crate::decoder::DecodedTrack;
use crate::streaming::StreamingAudioSource;
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
    pub gameover_sfx_total_frames: AtomicU64,
    pub gameover_sfx_remaining_frames: AtomicU64,
    pub max_track_duration_ms_bits: AtomicU64,
    pub chart_duration_ms_bits: AtomicU64,
    pub tracks: RwLock<[Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS]>,
    pub stream_source: RwLock<Option<Arc<StreamingAudioSource>>>,
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
            gameover_sfx_total_frames: AtomicU64::new(0),
            gameover_sfx_remaining_frames: AtomicU64::new(0),
            max_track_duration_ms_bits: AtomicU64::new(0.0_f64.to_bits()),
            chart_duration_ms_bits: AtomicU64::new(0.0_f64.to_bits()),
            tracks: RwLock::new([None, None, None]),
            stream_source: RwLock::new(None),
            configs: [
                TrackRuntimeConfig::default(),
                TrackRuntimeConfig::default(),
                TrackRuntimeConfig::default(),
            ],
        })
    }

    pub fn set_track(&self, slot: usize, track: Option<Arc<DecodedTrack>>) {
        if slot < MAX_AUDIO_TRACKS {
            {
                let mut guard = self.tracks.write();
                guard[slot] = track;
            }
            self.update_max_track_duration_ms();
        }
    }

    pub fn clear_tracks(&self) {
        {
            let mut guard = self.tracks.write();
            for slot in guard.iter_mut() {
                *slot = None;
            }
        }
        self.update_max_track_duration_ms();
    }

    pub fn set_stream_source(&self, source: Option<Arc<StreamingAudioSource>>) {
        let mut guard = self.stream_source.write();
        *guard = source;
    }

    pub fn clear_stream_source(&self) {
        self.set_stream_source(None);
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

    /// Dispara el efecto de sonido sintetizado de Game Over (audible incluso con el mezclador pausado).
    pub fn trigger_gameover_sfx(&self) {
        let sr = self.sample_rate.load(Ordering::Relaxed).max(8000) as f64;
        let frames = (sr * 0.68).round().max(1.0) as u64;
        self.gameover_sfx_total_frames.store(frames, Ordering::Release);
        self.gameover_sfx_remaining_frames.store(frames, Ordering::Release);
    }

    pub fn clear_gameover_sfx(&self) {
        self.gameover_sfx_remaining_frames.store(0, Ordering::Release);
        self.gameover_sfx_total_frames.store(0, Ordering::Release);
    }

    /// Inicia una cuenta regresiva de 4 pulsos ("3, 2, 1, ¡YA!") sincronizada exactamente al BPM.
    pub fn start_countdown(&self, bpm: f64) {
        let sr = self.sample_rate.load(Ordering::Relaxed).max(8000) as f64;
        let safe_bpm = bpm.clamp(40.0, 320.0);
        let beat_frames = ((60.0 / safe_bpm) * sr).round().max(1.0) as u64;
        let total_frames = beat_frames * 4;
        self.frame_cursor.store(0, Ordering::Release);
        if let Some(stream) = self.stream_source.read().as_ref() {
            stream.seek_frame(0);
        }
        self.countdown_beat_frames.store(beat_frames, Ordering::Release);
        self.countdown_total_frames.store(total_frames, Ordering::Release);
        self.countdown_remaining_frames.store(total_frames, Ordering::Release);
    }

    pub fn seek_ms(&self, ms: f64) {
        self.clear_countdown();
        let sr = self.sample_rate.load(Ordering::Relaxed) as f64;
        let frame = ((ms.max(0.0) / 1000.0) * sr).round() as u64;
        self.frame_cursor.store(frame, Ordering::Release);
        if let Some(stream) = self.stream_source.read().as_ref() {
            stream.seek_frame(frame);
        }
    }

    /// Devuelve la duración máxima (en ms) de las pistas cargadas en modo estudio.
    pub fn max_track_duration_ms(&self) -> f64 {
        let sr = self.sample_rate.load(Ordering::Relaxed).max(1) as f64;
        let guard = self.tracks.read();
        let max_frames = guard
            .iter()
            .flatten()
            .map(|track| track.frame_count())
            .max()
            .unwrap_or(0);
        (max_frames as f64 / sr) * 1000.0
    }

    /// Actualiza la duración máxima cacheada en `AtomicU64` para lecturas lock-free a 60 FPS.
    pub fn update_max_track_duration_ms(&self) {
        let dur = self.max_track_duration_ms();
        self.max_track_duration_ms_bits
            .store(dur.to_bits(), Ordering::Release);
    }

    /// Actualiza la duración total cacheada del chart activo en `AtomicU64`.
    pub fn set_chart_duration_ms(&self, duration_ms: f64) {
        let safe = if duration_ms.is_finite() && duration_ms > 0.0 {
            duration_ms
        } else {
            0.0
        };
        self.chart_duration_ms_bits
            .store(safe.to_bits(), Ordering::Release);
    }

    /// Actualiza la frecuencia de muestreo y recalcula la duración cacheada de pistas.
    pub fn set_sample_rate(&self, sample_rate: u32) {
        self.sample_rate
            .store(sample_rate.max(8000), Ordering::Release);
        self.update_max_track_duration_ms();
    }

    /// Lectura 100% libre de locks (`Mutex`/`RwLock`) para el hilo de seguimiento en Chart Editor.
    /// Devuelve `(position_ms, is_playing, total_duration_ms)` y detiene al final automáticamente.
    pub fn read_preview_state_lockfree(&self) -> (f64, bool, f64) {
        let track_dur = f64::from_bits(self.max_track_duration_ms_bits.load(Ordering::Acquire));
        let chart_dur = f64::from_bits(self.chart_duration_ms_bits.load(Ordering::Acquire));
        let duration_ms = track_dur.max(chart_dur).max(0.0);
        let mut pos_ms = self.position_ms().max(0.0);
        let mut playing = self.is_playing.load(Ordering::Acquire);

        if playing && duration_ms > 0.0 && pos_ms >= duration_ms {
            self.is_playing.store(false, Ordering::Release);
            let sr = self.sample_rate.load(Ordering::Relaxed).max(1) as f64;
            let frame = ((duration_ms / 1000.0) * sr).round() as u64;
            self.frame_cursor.store(frame, Ordering::Release);
            pos_ms = duration_ms;
            playing = false;
        }

        (pos_ms, playing, duration_ms)
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
        let total_requested = out_interleaved.len() / 2;
        if total_requested == 0 {
            return;
        }
        let sr = self.sample_rate.load(Ordering::Relaxed).max(1) as f32;

        if !self.is_playing.load(Ordering::Acquire) {
            out_interleaved.fill(0.0);
            let rem_go = self.gameover_sfx_remaining_frames.load(Ordering::Acquire);
            if rem_go > 0 {
                let total_go = self.gameover_sfx_total_frames.load(Ordering::Relaxed).max(rem_go);
                let frames_to_mix = total_requested.min(rem_go as usize);
                let elapsed_start = total_go.saturating_sub(rem_go);
                for f in 0..frames_to_mix {
                    let t = ((elapsed_start + f as u64) as f32) / sr;
                    let s = synthesize_gameover_cue(t).tanh();
                    out_interleaved[f * 2] = s;
                    out_interleaved[f * 2 + 1] = s;
                }
                self.gameover_sfx_remaining_frames
                    .store(rem_go.saturating_sub(frames_to_mix as u64), Ordering::Release);
            }
            return;
        }
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
        let mut used_compiled_stream = false;

        // Prioridad 1: archivo único compilado en streaming desde disco (~512 KB RAM)
        if let Some(stream_guard) = self.stream_source.try_read() {
            if let Some(stream) = stream_guard.as_ref() {
                any_track = true;
                used_compiled_stream = true;
                stream.read_frames_add(slice_out, 1.0);
            }
        }

        // Prioridad 2 (solo estudio/editor): pistas compactas `i16` en memoria
        if !any_track {
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
                        let scale = gain / 32768.0;

                        let total_frames = track.frame_count();
                        for f in 0..frames_requested {
                            let idx = cursor + f;
                            if idx >= total_frames {
                                break;
                            }
                            slice_out[f * 2] += (track.stereo_samples[idx * 2] as f32) * scale;
                            slice_out[f * 2 + 1] +=
                                (track.stereo_samples[idx * 2 + 1] as f32) * scale;
                        }
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

        if used_compiled_stream {
            // El stream único ya viene fusionado/masterizado con limitador True Peak;
            // evitar `tanh()` elimina compresión no lineal y distorsión armónica innecesaria.
            for s in slice_out.iter_mut() {
                *s = s.clamp(-1.0, 1.0);
            }
        } else {
            // Limitador suave (soft clip) únicamente al mezclar en vivo hasta 3 pistas del editor o sinte demo.
            for s in slice_out.iter_mut() {
                *s = (*s).tanh();
            }
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

/// Sintetiza el golpe y caída clásica de Game Over (libre de derechos) cuando el jugador pierde.
#[inline]
fn synthesize_gameover_cue(t: f32) -> f32 {
    if t > 0.68 {
        return 0.0;
    }
    let thud_env = (-t * 11.0).exp();
    let thud_freq = 155.0 * (-t * 5.2).exp() + 44.0;
    let thud = (2.0 * PI * thud_freq * t).sin() * thud_env * 0.62;

    let drop_env = (t / 0.02).min(1.0) * (-t * 4.2).exp();
    let drop_freq = if t < 0.22 {
        311.13 // Eb4
    } else if t < 0.42 {
        277.18 // Db4
    } else {
        220.00 // A3
    };
    let voice = ((2.0 * PI * drop_freq * t).sin() * 0.40
        + (2.0 * PI * (drop_freq * 0.5) * t).sin() * 0.25)
        * drop_env;
    thud + voice
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

    #[test]
    fn preview_pause_preserves_cursor_for_resume_and_reports_track_duration() {
        let mixer = AudioMixerState::new(48_000);
        let track = Arc::new(DecodedTrack {
            sample_rate: 48_000,
            stereo_samples: vec![1000_i16; 96_000 * 2], // 2000 ms
            waveform_peaks: Vec::new(),
        });
        mixer.set_track(0, Some(track));
        assert!((mixer.max_track_duration_ms() - 2000.0).abs() < 0.1);

        mixer.seek_ms(500.0);
        mixer.play();
        let mut buf = vec![0.0_f32; 24_000 * 2]; // +500 ms
        mixer.render_audio_frames(&mut buf);
        assert!((mixer.position_ms() - 1000.0).abs() < 0.1);

        // Pausar no debe perder la posición ni avanzar mientras está en pausa
        mixer.pause();
        mixer.render_audio_frames(&mut buf);
        assert!((mixer.position_ms() - 1000.0).abs() < 0.1);

        // Reanudar continúa exactamente desde 1000 ms
        mixer.play();
        mixer.render_audio_frames(&mut buf);
        assert!((mixer.position_ms() - 1500.0).abs() < 0.1);
    }

    #[test]
    fn gameover_sfx_renders_even_while_song_mixer_is_paused() {
        let mixer = AudioMixerState::new(48_000);
        mixer.pause();
        mixer.trigger_gameover_sfx();
        let mut buf = vec![0.0_f32; 4_800 * 2];
        mixer.render_audio_frames(&mut buf);
        assert!(buf.iter().any(|s| s.abs() > 0.05));
    }

    #[test]
    fn lockfree_preview_state_tracks_durations_and_auto_pauses_at_end() {
        let mixer = AudioMixerState::new(48_000);
        mixer.set_chart_duration_ms(2500.0);
        let track = Arc::new(DecodedTrack {
            sample_rate: 48_000,
            stereo_samples: vec![1000_i16; 96_000 * 2], // 2000 ms
            waveform_peaks: Vec::new(),
        });
        mixer.set_track(0, Some(track));

        let (pos0, playing0, dur0) = mixer.read_preview_state_lockfree();
        assert!(!playing0);
        assert!((pos0 - 0.0).abs() < 0.1);
        assert!((dur0 - 2500.0).abs() < 0.1);

        mixer.seek_ms(2600.0);
        mixer.play();
        let (pos1, playing1, dur1) = mixer.read_preview_state_lockfree();
        assert!(!playing1);
        assert!((dur1 - 2500.0).abs() < 0.1);
        assert!((pos1 - 2500.0).abs() < 0.1);
    }

    #[test]
    fn stream_source_passes_linearly_without_tanh_while_multitrack_soft_clips() {
        let tmp = std::env::temp_dir().join("beatforge_mixer_linear_stream_test.wav");
        let _ = std::fs::remove_file(&tmp);

        let spec = hound::WavSpec {
            channels: 2,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        // 0.85 * 32768 ~= 27853; sin tanh() debe salir ~0.85 (no tanh(0.85) ~= 0.691)
        let amp_i16 = 27_853_i16;
        {
            let mut w = hound::WavWriter::create(&tmp, spec).unwrap();
            for _ in 0..1024 {
                w.write_sample(amp_i16).unwrap();
                w.write_sample(-amp_i16).unwrap();
            }
            w.finalize().unwrap();
        }

        let mixer = AudioMixerState::new(48_000);
        let stream = StreamingAudioSource::open(&tmp).unwrap();
        mixer.set_stream_source(Some(stream));
        mixer.play();

        let mut buf = vec![0.0_f32; 256 * 2];
        mixer.render_audio_frames(&mut buf);
        assert!((buf[0] - 0.85).abs() < 0.01);
        assert!((buf[1] + 0.85).abs() < 0.01);

        // Sin stream_source, dos pistas fuertes sumadas (0.9 + 0.9 = 1.8) sí aplican tanh() < 1.0
        mixer.clear_stream_source();
        let loud = Arc::new(DecodedTrack {
            sample_rate: 48_000,
            stereo_samples: vec![29_491_i16; 1024 * 2], // ~0.90
            waveform_peaks: Vec::new(),
        });
        mixer.set_track(0, Some(Arc::clone(&loud)));
        mixer.set_track(1, Some(loud));
        mixer.seek_ms(0.0);
        mixer.play();
        let mut buf_mix = vec![0.0_f32; 256 * 2];
        mixer.render_audio_frames(&mut buf_mix);
        assert!(buf_mix[0] > 0.90 && buf_mix[0] < 0.97);

        let _ = std::fs::remove_file(&tmp);
    }
}
