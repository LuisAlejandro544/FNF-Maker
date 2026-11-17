//! Archivo: engine/crates/pulse_audio/src/streaming.rs
//! Propósito: Fusión de 1–3 pistas de audio en un único archivo WAV compilado en caliente
//! (`merge_tracks_to_wav`) y lector de streaming desde disco con buffer circular SPSC
//! lock-free (`StreamingAudioSource`) de ~512 KB. Permite liberar decenas/cientos de MB
//! de PCM en RAM al jugar, crítico en teléfonos de 32 bits y 2–3 GB de RAM.

use crate::decoder::DecodedTrack;
use parking_lot::RwLock;
use pulse_core::project::{AudioTrackSlot, MAX_AUDIO_TRACKS};
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

/// Capacidad del buffer circular en frames estéreo (potencia de 2):
/// 131,072 frames * 2 canales * 2 bytes = 512 KB (~2.73 segundos a 48 kHz).
pub const STREAM_RING_FRAMES: usize = 131_072;
const STREAM_RING_MASK: usize = STREAM_RING_FRAMES - 1;
const STREAM_RING_BYTES: usize = STREAM_RING_FRAMES * 4;
const PREFILL_FRAMES: usize = 24_576; // ~0.51 s listos de inmediato al abrir o buscar

/// Resultado de fusionar las pistas activas en un solo archivo de audio compilado.
#[derive(Debug, Clone)]
pub struct AudioMergeReport {
    pub output_path: PathBuf,
    pub merged_tracks: usize,
    pub total_frames: usize,
    pub sample_rate: u32,
    pub source_ram_bytes: usize,
    pub estimated_ram_saved_kb: usize,
}

/// Mezcla de 1 a 3 pistas activas (aplicando ganancia, omitiendo silenciadas y aplicando
/// limitador suave `tanh`) en un único archivo WAV estéreo de 16 bits sobre disco.
pub fn merge_tracks_to_wav(
    tracks: &[Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS],
    configs: &[AudioTrackSlot; MAX_AUDIO_TRACKS],
    sample_rate: u32,
    out_path: &Path,
) -> Result<Option<AudioMergeReport>, String> {
    let mut active_indices = Vec::with_capacity(MAX_AUDIO_TRACKS);
    let mut source_ram_bytes = 0usize;
    let mut max_frames = 0usize;

    for slot in 0..MAX_AUDIO_TRACKS {
        if let Some(track) = &tracks[slot] {
            source_ram_bytes += track.memory_bytes();
            if !configs[slot].muted {
                max_frames = max_frames.max(track.frame_count());
                active_indices.push(slot);
            }
        }
    }

    if active_indices.is_empty() || max_frames == 0 {
        return Ok(None);
    }

    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let tmp_path = out_path.with_extension("wav.tmp");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: sample_rate.max(8000),
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    {
        let file = File::create(&tmp_path)
            .map_err(|e| format!("No se pudo crear archivo de mezcla temporal: {}", e))?;
        let buf_writer = BufWriter::with_capacity(64 * 1024, file);
        let mut writer = hound::WavWriter::new(buf_writer, spec)
            .map_err(|e| format!("Error iniciando cabecera WAV compilada: {}", e))?;

        let chunk_frames = 8192usize;
        let mut mix_l = vec![0.0_f32; chunk_frames];
        let mut mix_r = vec![0.0_f32; chunk_frames];
        let mut start = 0usize;

        while start < max_frames {
            let len = (max_frames - start).min(chunk_frames);
            mix_l[..len].fill(0.0);
            mix_r[..len].fill(0.0);

            for &slot in &active_indices {
                if let Some(track) = &tracks[slot] {
                    let gain = configs[slot].gain.clamp(0.0, 2.0) / 32768.0;
                    let track_frames = track.frame_count();
                    if start >= track_frames {
                        continue;
                    }
                    let avail = (track_frames - start).min(len);
                    let samples = &track.stereo_samples;
                    for i in 0..avail {
                        let base = (start + i) * 2;
                        mix_l[i] += (samples[base] as f32) * gain;
                        mix_r[i] += (samples[base + 1] as f32) * gain;
                    }
                }
            }

            for i in 0..len {
                let l_i16 = (mix_l[i].tanh().clamp(-1.0, 1.0) * 32767.0).round() as i16;
                let r_i16 = (mix_r[i].tanh().clamp(-1.0, 1.0) * 32767.0).round() as i16;
                writer
                    .write_sample(l_i16)
                    .map_err(|e| format!("Error escribiendo muestra L WAV: {}", e))?;
                writer
                    .write_sample(r_i16)
                    .map_err(|e| format!("Error escribiendo muestra R WAV: {}", e))?;
            }

            start += len;
        }

        writer
            .finalize()
            .map_err(|e| format!("Error finalizando WAV compilado: {}", e))?;
    }

    if std::fs::rename(&tmp_path, out_path).is_err() {
        std::fs::copy(&tmp_path, out_path)
            .map_err(|e| format!("Error moviendo WAV compilado final: {}", e))?;
        let _ = std::fs::remove_file(&tmp_path);
    }

    let saved_bytes = source_ram_bytes.saturating_sub(STREAM_RING_BYTES);
    Ok(Some(AudioMergeReport {
        output_path: out_path.to_path_buf(),
        merged_tracks: active_indices.len(),
        total_frames: max_frames,
        sample_rate: spec.sample_rate,
        source_ram_bytes,
        estimated_ram_saved_kb: saved_bytes / 1024,
    }))
}

#[derive(Debug, Clone, Copy)]
struct WavDataLayout {
    data_offset: u64,
    total_frames: u64,
    sample_rate: u32,
}

/// Escanea los chunks RIFF/WAVE para obtener el offset exacto del bloque `data` PCM 16-bit estéreo
/// y permitir `seek()` O(1) directo sin cargar el archivo completo a memoria.
fn inspect_stereo_i16_wav(file: &mut File) -> Result<WavDataLayout, String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|e| format!("Error haciendo seek al inicio WAV: {}", e))?;
    let mut riff = [0u8; 12];
    file.read_exact(&mut riff)
        .map_err(|e| format!("Cabecera RIFF inválida: {}", e))?;
    if &riff[0..4] != b"RIFF" || &riff[8..12] != b"WAVE" {
        return Err("El archivo no es un contenedor RIFF/WAVE válido".to_string());
    }

    let mut sample_rate = 48_000_u32;
    let mut channels = 0_u16;
    let mut bits_per_sample = 0_u16;
    let mut data_offset = None;
    let mut data_size = 0_u64;

    loop {
        let mut hdr = [0u8; 8];
        if file.read_exact(&mut hdr).is_err() {
            break;
        }
        let chunk_id = &hdr[0..4];
        let chunk_size = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as u64;
        let chunk_start = file.stream_position().unwrap_or(0);

        if chunk_id == b"fmt " && chunk_size >= 16 {
            let mut fmt = [0u8; 16];
            file.read_exact(&mut fmt)
                .map_err(|e| format!("Error leyendo chunk fmt WAV: {}", e))?;
            channels = u16::from_le_bytes([fmt[2], fmt[3]]);
            sample_rate = u32::from_le_bytes([fmt[4], fmt[5], fmt[6], fmt[7]]);
            bits_per_sample = u16::from_le_bytes([fmt[14], fmt[15]]);
        } else if chunk_id == b"data" {
            data_offset = Some(chunk_start);
            data_size = chunk_size;
            break;
        }

        let padded = chunk_size + (chunk_size & 1);
        file.seek(SeekFrom::Start(chunk_start + padded))
            .map_err(|e| format!("Error avanzando chunk WAV: {}", e))?;
    }

    let offset = data_offset.ok_or_else(|| "El WAV no contiene chunk data".to_string())?;
    if channels != 2 || bits_per_sample != 16 {
        return Err(format!(
            "Streaming WAV requiere estéreo 16-bit (recibido {} ch, {} bits)",
            channels, bits_per_sample
        ));
    }

    Ok(WavDataLayout {
        data_offset: offset,
        total_frames: data_size / 4,
        sample_rate,
    })
}

struct StreamShared {
    path: PathBuf,
    layout: WavDataLayout,
    ring: Box<[i16]>,
    /// Posiciones monótonas dentro del ciclo actual del buffer circular (en frames).
    read_pos: AtomicU64,
    write_pos: AtomicU64,
    /// Coordinación de seek sin mutex bloqueante en el hilo de audio (`try_read`).
    seek_lock: RwLock<()>,
    seek_gen: AtomicU64,
    producer_gen: AtomicU64,
    seek_target_frame: AtomicU64,
    stop_flag: AtomicBool,
}

// `ring` se modifica únicamente en las ranuras libres `[write_pos .. read_pos + STREAM_RING_FRAMES)`
// por el hilo productor y se lee únicamente en `[read_pos .. write_pos)` por el hilo consumidor
// cuando `seek_gen == producer_gen`.
unsafe impl Sync for StreamShared {}

impl StreamShared {
    #[inline]
    fn ring_mut_ptr(&self) -> *mut i16 {
        self.ring.as_ptr() as *mut i16
    }
}

/// Fuente de audio en streaming desde disco con buffer circular SPSC lock-free (~512 KB RAM).
pub struct StreamingAudioSource {
    shared: Arc<StreamShared>,
    reader_handle: Option<JoinHandle<()>>,
}

impl StreamingAudioSource {
    pub fn open(path: &Path) -> Result<Arc<Self>, String> {
        let mut file = File::open(path)
            .map_err(|e| format!("No se pudo abrir audio compilado '{}': {}", path.display(), e))?;
        let layout = inspect_stereo_i16_wav(&mut file)?;
        let ring = vec![0_i16; STREAM_RING_FRAMES * 2].into_boxed_slice();

        let shared = Arc::new(StreamShared {
            path: path.to_path_buf(),
            layout,
            ring,
            read_pos: AtomicU64::new(0),
            write_pos: AtomicU64::new(0),
            seek_lock: RwLock::new(()),
            seek_gen: AtomicU64::new(0),
            producer_gen: AtomicU64::new(0),
            seek_target_frame: AtomicU64::new(0),
            stop_flag: AtomicBool::new(false),
        });

        // Pre-llenar síncronamente los primeros frames para arranque instantáneo sin silencio inicial
        let initial_frames = prefill_file_into_ring(&mut file, &shared, 0, PREFILL_FRAMES);
        shared.write_pos.store(initial_frames as u64, Ordering::Release);

        let shared_bg = Arc::clone(&shared);
        let initial_file_frame = initial_frames as u64;
        let handle = std::thread::Builder::new()
            .name("pulse_audio_streamer".to_string())
            .spawn(move || {
                streamer_loop(shared_bg, file, initial_file_frame);
            })
            .map_err(|e| format!("No se pudo iniciar hilo de streaming audio: {}", e))?;

        Ok(Arc::new(Self {
            shared,
            reader_handle: Some(handle),
        }))
    }

    pub fn sample_rate(&self) -> u32 {
        self.shared.layout.sample_rate
    }

    pub fn total_frames(&self) -> u64 {
        self.shared.layout.total_frames
    }

    /// Solicita un salto instantáneo a `frame` (p. ej. `0` en reinicio o `seek_ms`).
    pub fn seek_frame(&self, frame: u64) {
        let clamped = frame.min(self.shared.layout.total_frames);
        self.shared
            .seek_target_frame
            .store(clamped, Ordering::Release);
        self.shared.seek_gen.fetch_add(1, Ordering::AcqRel);
        if let Some(h) = &self.reader_handle {
            h.thread().unpark();
        }
    }

    /// Lee hasta `frames_requested` frames estéreo del buffer circular y los suma a `out_interleaved`.
    /// 100% no bloqueante: nunca espera disco ni mutex en el hilo de tiempo real AAudio.
    pub fn read_frames_add(&self, out_interleaved: &mut [f32], gain: f32) -> usize {
        let frames_requested = out_interleaved.len() / 2;
        if frames_requested == 0 {
            return 0;
        }
        let Some(_guard) = self.shared.seek_lock.try_read() else {
            return 0;
        };

        let sg = self.shared.seek_gen.load(Ordering::Acquire);
        let pg = self.shared.producer_gen.load(Ordering::Acquire);
        if sg != pg {
            return 0;
        }

        let r = self.shared.read_pos.load(Ordering::Relaxed);
        let w = self.shared.write_pos.load(Ordering::Acquire);
        let avail = (w.saturating_sub(r) as usize).min(STREAM_RING_FRAMES);
        let to_read = frames_requested.min(avail);
        if to_read == 0 {
            return 0;
        }

        let scale = gain / 32768.0;
        let ring = &self.shared.ring;
        for f in 0..to_read {
            let slot = ((r as usize) + f) & STREAM_RING_MASK;
            let base = slot * 2;
            out_interleaved[f * 2] += (ring[base] as f32) * scale;
            out_interleaved[f * 2 + 1] += (ring[base + 1] as f32) * scale;
        }

        self.shared
            .read_pos
            .store(r + (to_read as u64), Ordering::Release);
        if avail.saturating_sub(to_read) < (STREAM_RING_FRAMES / 2) {
            if let Some(h) = &self.reader_handle {
                h.thread().unpark();
            }
        }
        to_read
    }
}

impl Drop for StreamingAudioSource {
    fn drop(&mut self) {
        self.shared.stop_flag.store(true, Ordering::Release);
        if let Some(h) = self.reader_handle.take() {
            h.thread().unpark();
            let _ = h.join();
        }
    }
}

fn prefill_file_into_ring(
    file: &mut File,
    shared: &StreamShared,
    start_frame: u64,
    max_frames: usize,
) -> usize {
    let remaining_in_file = shared.layout.total_frames.saturating_sub(start_frame);
    let want_frames = (max_frames as u64).min(remaining_in_file) as usize;
    if want_frames == 0 {
        return 0;
    }

    let seek_byte = shared.layout.data_offset + start_frame * 4;
    if file.seek(SeekFrom::Start(seek_byte)).is_err() {
        return 0;
    }

    let mut raw = vec![0u8; want_frames * 4];
    let mut read_bytes = 0usize;
    while read_bytes < raw.len() {
        match file.read(&mut raw[read_bytes..]) {
            Ok(0) => break,
            Ok(n) => read_bytes += n,
            Err(_) => break,
        }
    }

    let decoded_frames = read_bytes / 4;
    let ring_ptr = shared.ring_mut_ptr();
    for f in 0..decoded_frames {
        let b = f * 4;
        let l = i16::from_le_bytes([raw[b], raw[b + 1]]);
        let r = i16::from_le_bytes([raw[b + 2], raw[b + 3]]);
        unsafe {
            *ring_ptr.add(f * 2) = l;
            *ring_ptr.add(f * 2 + 1) = r;
        }
    }
    decoded_frames
}

fn streamer_loop(shared: Arc<StreamShared>, mut file: File, mut file_frame: u64) {
    let mut local_gen = 0_u64;
    let chunk_frames = 8192usize;
    let mut byte_buf = vec![0u8; chunk_frames * 4];

    while !shared.stop_flag.load(Ordering::Relaxed) {
        let sg = shared.seek_gen.load(Ordering::Acquire);
        if sg != local_gen {
            let target = shared
                .seek_target_frame
                .load(Ordering::Acquire)
                .min(shared.layout.total_frames);
            let seek_byte = shared.layout.data_offset + target * 4;
            if file.seek(SeekFrom::Start(seek_byte)).is_ok() {
                file_frame = target;
            }

            {
                let _reset_guard = shared.seek_lock.write();
                shared.read_pos.store(0, Ordering::Release);
                shared.write_pos.store(0, Ordering::Release);
                shared.producer_gen.store(sg, Ordering::Release);
            }
            local_gen = sg;
            continue;
        }

        let r = shared.read_pos.load(Ordering::Acquire);
        let w = shared.write_pos.load(Ordering::Relaxed);
        let buffered = w.saturating_sub(r) as usize;
        let free_frames = STREAM_RING_FRAMES.saturating_sub(buffered);
        let remaining_file = shared.layout.total_frames.saturating_sub(file_frame);

        if free_frames < 2048 || remaining_file == 0 {
            std::thread::park_timeout(Duration::from_millis(12));
            continue;
        }

        let to_read_frames = free_frames.min(chunk_frames).min(remaining_file as usize);
        let want_bytes = to_read_frames * 4;
        let mut total_read = 0usize;
        while total_read < want_bytes {
            match file.read(&mut byte_buf[total_read..want_bytes]) {
                Ok(0) => break,
                Ok(n) => total_read += n,
                Err(_) => break,
            }
        }

        let got_frames = total_read / 4;
        if got_frames == 0 {
            std::thread::park_timeout(Duration::from_millis(12));
            continue;
        }

        // Si hubo otro seek mientras leíamos disco, descartamos este chunk sin contaminar el ring
        if shared.seek_gen.load(Ordering::Acquire) != local_gen {
            continue;
        }

        let ring_ptr = shared.ring_mut_ptr();
        for f in 0..got_frames {
            let slot = ((w as usize) + f) & STREAM_RING_MASK;
            let b = f * 4;
            let l = i16::from_le_bytes([byte_buf[b], byte_buf[b + 1]]);
            let r_samp = i16::from_le_bytes([byte_buf[b + 2], byte_buf[b + 3]]);
            unsafe {
                *ring_ptr.add(slot * 2) = l;
                *ring_ptr.add(slot * 2 + 1) = r_samp;
            }
        }

        file_frame += got_frames as u64;
        shared
            .write_pos
            .store(w + (got_frames as u64), Ordering::Release);
    }

    let _ = &shared.path;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_track(sr: u32, frames: usize, amp_i16: i16) -> Arc<DecodedTrack> {
        let mut stereo_samples = Vec::with_capacity(frames * 2);
        for _ in 0..frames {
            stereo_samples.push(amp_i16);
            stereo_samples.push(-amp_i16);
        }
        Arc::new(DecodedTrack {
            sample_rate: sr,
            stereo_samples,
            waveform_peaks: vec![0.5; 64],
        })
    }

    #[test]
    fn merge_tracks_combines_active_slots_and_streams_cleanly_with_seek() {
        let tmp = std::env::temp_dir().join("beatforge_audio_merge_stream_test.wav");
        let _ = std::fs::remove_file(&tmp);

        let mut configs = AudioTrackSlot::default_three_slots();
        configs[0].gain = 0.5;
        configs[1].gain = 0.5;
        configs[2].muted = true; // silenciada: no debe sumarse

        let tracks: [Option<Arc<DecodedTrack>>; MAX_AUDIO_TRACKS] = [
            Some(make_track(48_000, 48_000, 8_000)),
            Some(make_track(48_000, 48_000, 4_000)),
            Some(make_track(48_000, 48_000, 12_000)),
        ];

        let report = merge_tracks_to_wav(&tracks, &configs, 48_000, &tmp)
            .expect("merge ok")
            .expect("some report");
        assert_eq!(report.merged_tracks, 2);
        assert_eq!(report.total_frames, 48_000);
        assert!(tmp.exists());

        let stream = StreamingAudioSource::open(&tmp).expect("open stream ok");
        assert_eq!(stream.sample_rate(), 48_000);
        assert_eq!(stream.total_frames(), 48_000);

        let mut out = vec![0.0_f32; 512 * 2];
        let n = stream.read_frames_add(&mut out, 1.0);
        assert_eq!(n, 512);
        // Suma esperada: (8000*0.5 + 4000*0.5)/32768 ~= +0.183 L, -0.183 R
        assert!((out[0] - 0.183).abs() < 0.02);
        assert!((out[1] + 0.183).abs() < 0.02);

        // Probar seek a frame 1024 y volver a leer
        stream.seek_frame(1024);
        std::thread::sleep(Duration::from_millis(25));
        let mut out_seek = vec![0.0_f32; 256 * 2];
        let n2 = stream.read_frames_add(&mut out_seek, 1.0);
        assert_eq!(n2, 256);
        assert!((out_seek[0] - 0.183).abs() < 0.02);

        drop(stream);
        let _ = std::fs::remove_file(&tmp);
    }
}
