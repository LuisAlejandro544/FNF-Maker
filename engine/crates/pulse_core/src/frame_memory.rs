//! Archivo: engine/crates/pulse_core/src/frame_memory.rs
//! Propósito: Gestión determinista de memoria en tiempo real para Rust sin Garbage Collector:
//! 1. `FrameBumpArena`: Arena temporal por frame (Bump Allocation) con bloques alineados a 16 bytes
//!    y reinicio en O(1) (`reset()`) al inicio de cada frame sin liberar capacidad al sistema.
//! 2. `NoteSlidingWindow`: Descarte progresivo por ventana deslizante para saltar en O(1) notas
//!    ya tocadas o que salieron de pantalla sin recorrer todo el chart en cada frame.
//! 3. `LruCache`: Caché LRU determinista con presupuesto máximo de bytes y entradas para
//!    desechar automáticamente texturas/recursos menos usados en teléfonos con poca RAM.

use crate::chart::ChartNote;
use std::collections::HashMap;
use std::hash::Hash;

/// Bloque alineado a 16 bytes para garantizar alineación segura en ARM 32-bit (`armeabi-v7a`) y 64-bit (`arm64-v8a`).
#[repr(C, align(16))]
#[derive(Clone, Copy)]
struct AlignedBlock16([u8; 16]);

/// Identificador tipado de un slice almacenado dentro de `FrameBumpArena`.
#[derive(Debug)]
pub struct ArenaSpan<T: Copy> {
    chunk_idx: usize,
    start_byte: usize,
    len: usize,
    _marker: std::marker::PhantomData<T>,
}

impl<T: Copy> Clone for ArenaSpan<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Copy> Copy for ArenaSpan<T> {}

impl<T: Copy> ArenaSpan<T> {
    pub fn empty() -> Self {
        Self {
            chunk_idx: 0,
            start_byte: 0,
            len: 0,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// Arena temporal por frame (Bump Allocator por bloques fijos que nunca invalidan spans previos).
/// Al finalizar o empezar cada frame, `reset()` reinicia el puntero a 0 en una sola operación.
pub struct FrameBumpArena {
    chunks: Vec<Vec<AlignedBlock16>>,
    chunk_bytes: usize,
    current_chunk: usize,
    byte_offset: usize,
    bytes_used_frame: usize,
    peak_bytes: usize,
    reset_count: u64,
}

impl FrameBumpArena {
    /// Crea una arena con un tamaño de bloque inicial (mínimo 4 KB).
    pub fn new(initial_chunk_bytes: usize) -> Self {
        let aligned_chunk = initial_chunk_bytes.max(4096).div_ceil(16) * 16;
        let blocks = aligned_chunk / 16;
        Self {
            chunks: vec![vec![AlignedBlock16([0u8; 16]); blocks]],
            chunk_bytes: aligned_chunk,
            current_chunk: 0,
            byte_offset: 0,
            bytes_used_frame: 0,
            peak_bytes: 0,
            reset_count: 0,
        }
    }

    /// Reinicia la arena para el siguiente frame en O(1), manteniendo los bloques calientes en RAM.
    #[inline]
    pub fn reset(&mut self) {
        self.peak_bytes = self.peak_bytes.max(self.bytes_used_frame);
        self.current_chunk = 0;
        self.byte_offset = 0;
        self.bytes_used_frame = 0;
        self.reset_count += 1;
    }

    /// Asigna espacio alineado dentro de la arena y copia los elementos de un iterador `ExactSizeIterator` o slice.
    pub fn alloc_slice_copy<T: Copy>(&mut self, items: &[T]) -> ArenaSpan<T> {
        if items.is_empty() || std::mem::size_of::<T>() == 0 {
            return ArenaSpan::empty();
        }
        let align = std::mem::align_of::<T>().max(1);
        assert!(
            align <= 16,
            "FrameBumpArena soporta alineación máxima de 16 bytes"
        );
        let elem_size = std::mem::size_of::<T>();
        let total_bytes = elem_size
            .checked_mul(items.len())
            .expect("Desbordamiento en FrameBumpArena");

        let (chunk_idx, start_byte) = self.reserve_bytes(total_bytes, align);
        let chunk_slice = &mut self.chunks[chunk_idx];
        let base_ptr = chunk_slice.as_mut_ptr() as *mut u8;

        // Seguridad: `reserve_bytes` garantiza que `start_byte` está alineado a `align` (<=16),
        // que `start_byte + total_bytes <= chunk_slice.len() * 16`, y `T: Copy` no tiene destructor.
        unsafe {
            let dst = base_ptr.add(start_byte) as *mut T;
            std::ptr::copy_nonoverlapping(items.as_ptr(), dst, items.len());
        }

        ArenaSpan {
            chunk_idx,
            start_byte,
            len: items.len(),
            _marker: std::marker::PhantomData,
        }
    }

    /// Construye un slice en la arena usando un vector scratch interno o iterador sin crecer heap repetidamente.
    pub fn alloc_from_iter<T: Copy, I: IntoIterator<Item = T>>(
        &mut self,
        iter: I,
        scratch: &mut Vec<T>,
    ) -> ArenaSpan<T> {
        scratch.clear();
        scratch.extend(iter);
        self.alloc_slice_copy(scratch)
    }

    /// Lee un slice previamente asignado dentro del frame actual.
    #[inline]
    pub fn get_slice<T: Copy>(&self, span: ArenaSpan<T>) -> &[T] {
        if span.len == 0 || std::mem::size_of::<T>() == 0 {
            return &[];
        }
        let chunk = &self.chunks[span.chunk_idx];
        let total_bytes = std::mem::size_of::<T>() * span.len;
        debug_assert!(span.start_byte + total_bytes <= chunk.len() * 16);
        let base_ptr = chunk.as_ptr() as *const u8;
        unsafe {
            let ptr = base_ptr.add(span.start_byte) as *const T;
            std::slice::from_raw_parts(ptr, span.len)
        }
    }

    pub fn bytes_used_this_frame(&self) -> usize {
        self.bytes_used_frame
    }

    pub fn peak_bytes(&self) -> usize {
        self.peak_bytes.max(self.bytes_used_frame)
    }

    pub fn total_capacity_bytes(&self) -> usize {
        self.chunks.iter().map(|c| c.len() * 16).sum()
    }

    pub fn reset_count(&self) -> u64 {
        self.reset_count
    }

    fn reserve_bytes(&mut self, bytes: usize, align: usize) -> (usize, usize) {
        loop {
            let chunk_cap_bytes = self.chunks[self.current_chunk].len() * 16;
            let aligned = (self.byte_offset + align - 1) & !(align - 1);
            if aligned + bytes <= chunk_cap_bytes {
                let old_offset = self.byte_offset;
                self.byte_offset = aligned + bytes;
                self.bytes_used_frame += self.byte_offset - old_offset;
                return (self.current_chunk, aligned);
            }

            // Pasar al siguiente chunk ya preasignado o crear uno nuevo lo bastante grande.
            if self.current_chunk + 1 < self.chunks.len() {
                self.current_chunk += 1;
                self.byte_offset = 0;
                let next_cap = self.chunks[self.current_chunk].len() * 16;
                if bytes <= next_cap {
                    self.byte_offset = bytes;
                    self.bytes_used_frame += bytes;
                    return (self.current_chunk, 0);
                }
            }

            let new_chunk_bytes = self.chunk_bytes.max(bytes).div_ceil(16) * 16;
            self.chunks
                .push(vec![AlignedBlock16([0u8; 16]); new_chunk_bytes / 16]);
            self.current_chunk = self.chunks.len() - 1;
            self.byte_offset = bytes;
            self.bytes_used_frame += bytes;
            return (self.current_chunk, 0);
        }
    }
}

/// Estado individual de una nota en el runtime sin heap extra por nota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotePlayState {
    Pending,
    Holding,
    Hit,
    Missed,
}

/// Representación compacta `Copy` de una nota visible dentro de la arena temporal por frame.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameVisibleNote {
    pub id: u64,
    pub time_ms: f64,
    pub sustain_ms: f64,
    pub direction: u8,
    pub is_player: bool,
    pub is_holding: bool,
}

/// Ventana deslizante progresiva sobre las notas ordenadas por `time_ms`.
/// Evita iterar en cada frame sobre notas que ya fueron golpeadas o quedaron atrás en la canción.
#[derive(Debug, Clone)]
pub struct NoteSlidingWindow {
    pub active_start_idx: usize,
    pub states: Vec<NotePlayState>,
    pub discarded_count: usize,
}

impl NoteSlidingWindow {
    pub fn new(note_count: usize) -> Self {
        Self {
            active_start_idx: 0,
            states: vec![NotePlayState::Pending; note_count],
            discarded_count: 0,
        }
    }

    pub fn reset(&mut self, note_count: usize) {
        self.active_start_idx = 0;
        self.discarded_count = 0;
        self.states.clear();
        self.states.resize(note_count, NotePlayState::Pending);
    }

    #[inline]
    pub fn is_resolved(&self, idx: usize) -> bool {
        self.states
            .get(idx)
            .map(|s| matches!(s, NotePlayState::Hit | NotePlayState::Missed))
            .unwrap_or(true)
    }

    #[inline]
    pub fn state(&self, idx: usize) -> NotePlayState {
        self.states
            .get(idx)
            .copied()
            .unwrap_or(NotePlayState::Hit)
    }

    #[inline]
    pub fn mark_holding(&mut self, idx: usize) {
        if let Some(slot) = self.states.get_mut(idx) {
            *slot = NotePlayState::Holding;
        }
    }

    #[inline]
    pub fn mark_hit(&mut self, idx: usize) {
        if let Some(slot) = self.states.get_mut(idx) {
            *slot = NotePlayState::Hit;
        }
    }

    #[inline]
    pub fn mark_missed(&mut self, idx: usize) {
        if let Some(slot) = self.states.get_mut(idx) {
            *slot = NotePlayState::Missed;
        }
    }

    /// Avanza `active_start_idx` descartando progresivamente notas ya resueltas y fuera de pantalla.
    pub fn advance_and_discard(&mut self, notes: &[ChartNote], song_pos_ms: f64, lookback_ms: f64) {
        let len = notes.len().min(self.states.len());
        while self.active_start_idx < len {
            let idx = self.active_start_idx;
            let state = self.states[idx];
            let note = &notes[idx];
            let end_ms = note.time_ms + note.sustain_ms;

            let can_discard = match state {
                NotePlayState::Hit | NotePlayState::Missed => end_ms < song_pos_ms - lookback_ms,
                NotePlayState::Pending | NotePlayState::Holding => false,
            };

            if can_discard {
                self.active_start_idx += 1;
                self.discarded_count += 1;
            } else {
                break;
            }
        }
    }

    /// Recopila dentro de `FrameBumpArena` únicamente las notas dentro de la ventana `[lookback, lookahead]`.
    /// Incluye notas en estado `Holding` para que la cola sustain siga bajando hasta terminar.
    pub fn collect_visible_into_arena(
        &self,
        notes: &[ChartNote],
        song_pos_ms: f64,
        lookback_ms: f64,
        lookahead_ms: f64,
        arena: &mut FrameBumpArena,
        scratch: &mut Vec<FrameVisibleNote>,
    ) -> ArenaSpan<FrameVisibleNote> {
        scratch.clear();
        let len = notes.len().min(self.states.len());
        let min_time = song_pos_ms - lookback_ms;
        let max_time = song_pos_ms + lookahead_ms;

        for idx in self.active_start_idx..len {
            let note = &notes[idx];
            if note.time_ms > max_time {
                break;
            }
            let st = self.states[idx];
            if st == NotePlayState::Hit {
                continue;
            }
            if note.time_ms + note.sustain_ms < min_time {
                continue;
            }
            scratch.push(FrameVisibleNote {
                id: note.id,
                time_ms: note.time_ms,
                sustain_ms: note.sustain_ms,
                direction: note.direction as u8,
                is_player: note.is_player,
                is_holding: st == NotePlayState::Holding,
            });
        }

        arena.alloc_slice_copy(scratch)
    }
}

#[derive(Debug)]
struct LruSlot<V> {
    value: V,
    bytes: usize,
    last_used_tick: u64,
}

/// Caché LRU determinista controlada por presupuesto de bytes y límite de entradas.
/// Evicta automáticamente los recursos menos usados recientemente para evitar saturar RAM.
#[derive(Debug)]
pub struct LruCache<K, V>
where
    K: Eq + Hash + Clone,
{
    entries: HashMap<K, LruSlot<V>>,
    max_bytes: usize,
    max_entries: usize,
    current_bytes: usize,
    tick: u64,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

impl<K, V> LruCache<K, V>
where
    K: Eq + Hash + Clone,
{
    pub fn new(max_bytes: usize, max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_bytes: max_bytes.max(1024),
            max_entries: max_entries.max(1),
            current_bytes: 0,
            tick: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    pub fn set_budget(&mut self, max_bytes: usize, max_entries: usize) {
        self.max_bytes = max_bytes.max(1024);
        self.max_entries = max_entries.max(1);
        self.evict_until_within_budget();
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.tick = self.tick.wrapping_add(1);
        let tick = self.tick;
        if let Some(slot) = self.entries.get_mut(key) {
            slot.last_used_tick = tick;
            self.hits += 1;
            Some(&slot.value)
        } else {
            self.misses += 1;
            None
        }
    }

    pub fn contains(&self, key: &K) -> bool {
        self.entries.contains_key(key)
    }

    /// Inserta o reemplaza una entrada y expulsa los elementos menos recientes hasta respetar el presupuesto.
    pub fn insert(&mut self, key: K, value: V, size_bytes: usize) -> Vec<K> {
        if let Some(prev) = self.entries.remove(&key) {
            self.current_bytes = self.current_bytes.saturating_sub(prev.bytes);
        }

        self.tick = self.tick.wrapping_add(1);
        self.current_bytes += size_bytes;
        self.entries.insert(
            key,
            LruSlot {
                value,
                bytes: size_bytes,
                last_used_tick: self.tick,
            },
        );

        self.evict_until_within_budget()
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.entries.remove(key).map(|slot| {
            self.current_bytes = self.current_bytes.saturating_sub(slot.bytes);
            slot.value
        })
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_bytes = 0;
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn current_bytes(&self) -> usize {
        self.current_bytes
    }

    pub fn max_bytes(&self) -> usize {
        self.max_bytes
    }

    fn evict_until_within_budget(&mut self) -> Vec<K> {
        let mut evicted = Vec::new();
        while (self.current_bytes > self.max_bytes || self.entries.len() > self.max_entries)
            && self.entries.len() > 1
        {
            let oldest_key = self
                .entries
                .iter()
                .min_by_key(|(_, slot)| slot.last_used_tick)
                .map(|(k, _)| k.clone());

            let Some(k) = oldest_key else { break };
            if let Some(removed) = self.entries.remove(&k) {
                self.current_bytes = self.current_bytes.saturating_sub(removed.bytes);
                self.evictions += 1;
                evicted.push(k);
            }
        }
        evicted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::NoteDirection;

    fn dummy_note(id: u64, time_ms: f64, is_player: bool) -> ChartNote {
        ChartNote {
            id,
            time_ms,
            direction: NoteDirection::Left,
            is_player,
            sustain_ms: 0.0,
            note_type: String::new(),
            hit: false,
            missed: false,
        }
    }

    #[test]
    fn frame_bump_arena_resets_in_o1_without_growing_capacity() {
        let mut arena = FrameBumpArena::new(4096);
        let items = [
            FrameVisibleNote {
                id: 1,
                time_ms: 500.0,
                sustain_ms: 0.0,
                direction: 0,
                is_player: true,
                is_holding: false,
            },
            FrameVisibleNote {
                id: 2,
                time_ms: 800.0,
                sustain_ms: 150.0,
                direction: 2,
                is_player: false,
                is_holding: true,
            },
        ];

        let initial_cap = arena.total_capacity_bytes();
        for _ in 0..100 {
            arena.reset();
            let span = arena.alloc_slice_copy(&items);
            let slice = arena.get_slice(span);
            assert_eq!(slice.len(), 2);
            assert_eq!(slice[0].id, 1);
            assert_eq!(slice[1].direction, 2);
        }
        assert_eq!(arena.total_capacity_bytes(), initial_cap);
        assert_eq!(arena.reset_count(), 100);
    }

    #[test]
    fn sliding_window_discards_passed_notes_and_early_breaks_future_notes() {
        let notes = vec![
            dummy_note(1, 500.0, true),
            dummy_note(2, 1000.0, false),
            dummy_note(3, 1800.0, true),
            dummy_note(4, 9000.0, true),
        ];
        let mut win = NoteSlidingWindow::new(notes.len());
        win.mark_hit(0);
        win.mark_missed(1);
        win.advance_and_discard(&notes, 1400.0, 240.0);

        assert_eq!(win.active_start_idx, 2);
        assert_eq!(win.discarded_count, 2);

        let mut arena = FrameBumpArena::new(4096);
        let mut scratch = Vec::new();
        let span = win.collect_visible_into_arena(&notes, 1400.0, 240.0, 1800.0, &mut arena, &mut scratch);
        let visible = arena.get_slice(span);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, 3);
    }

    #[test]
    fn lru_cache_evicts_least_recently_used_under_byte_budget() {
        let mut cache = LruCache::new(1024, 10);
        cache.insert("tex_a".to_string(), vec![1u8; 500], 500);
        cache.insert("tex_b".to_string(), vec![2u8; 500], 500);
        // Tocar tex_a para que tex_b pase a ser la menos usada recientemente
        assert!(cache.get(&"tex_a".to_string()).is_some());

        let evicted = cache.insert("tex_c".to_string(), vec![3u8; 500], 500);
        assert_eq!(evicted, vec!["tex_b".to_string()]);
        assert!(cache.contains(&"tex_a".to_string()));
        assert!(cache.contains(&"tex_c".to_string()));
        assert!(!cache.contains(&"tex_b".to_string()));
        assert_eq!(cache.current_bytes(), 1000);
    }
}
