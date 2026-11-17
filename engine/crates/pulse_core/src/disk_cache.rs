//! Archivo: engine/crates/pulse_core/src/disk_cache.rs
//! Propósito: Gestión pura en Rust de cuotas automáticas de caché en disco y expulsión LRU
//! (`Least Recently Used`) tanto para directorios multi-proyecto (`compiled_cache/<projectId>/`)
//! como para archivos individuales (`audio_cache/*.bfaud`).

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Cuota predeterminada si Android aún no informó `StorageManager.getCacheQuotaBytes` (160 MiB).
pub const DEFAULT_TOTAL_CACHE_QUOTA_BYTES: u64 = 160 * 1024 * 1024;
/// Límite inferior para garantizar que al menos el mod activo completo quepa en caché (48 MiB).
pub const MIN_CACHE_QUOTA_BYTES: u64 = 48 * 1024 * 1024;
/// Tope superior para evitar acumulación excesiva en dispositivos con mucho almacenamiento (384 MiB).
pub const MAX_CACHE_QUOTA_BYTES: u64 = 384 * 1024 * 1024;

/// Archivo marcador ligero dentro de cada subdirectorio compilado para registrar su último uso real.
const LAST_USED_MARKER_FILE: &str = ".last_used";

/// Reparto de cuota entre caché de compilación multi-proyecto y caché de audio decodificado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheBudgetSplit {
    pub total_bytes: u64,
    pub compiled_cache_bytes: u64,
    pub audio_cache_bytes: u64,
}

/// Resultado estadístico tras ejecutar una pasada de limpieza LRU en disco.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LruEvictionReport {
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub evicted_entries: usize,
    pub bytes_reclaimed: u64,
}

/// Normaliza la cuota reportada por Android dentro de `[48 MiB, 384 MiB]`
/// y calcula el presupuesto asignado a `compiled_cache` (65%) y `audio_cache` (35%).
pub fn compute_cache_budget_split(reported_quota_bytes: u64) -> CacheBudgetSplit {
    let base = if reported_quota_bytes == 0 {
        DEFAULT_TOTAL_CACHE_QUOTA_BYTES
    } else {
        reported_quota_bytes
    };
    let total_bytes = base.clamp(MIN_CACHE_QUOTA_BYTES, MAX_CACHE_QUOTA_BYTES);
    let compiled_cache_bytes = ((total_bytes as u128 * 65) / 100) as u64;
    let compiled_cache_bytes = compiled_cache_bytes.max(32 * 1024 * 1024);
    let audio_cache_bytes = total_bytes
        .saturating_sub(compiled_cache_bytes)
        .max(16 * 1024 * 1024);
    CacheBudgetSplit {
        total_bytes,
        compiled_cache_bytes,
        audio_cache_bytes,
    }
}

/// Calcula de forma iterativa el tamaño total en bytes de todos los archivos dentro de `path`.
pub fn dir_size_bytes(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    if path.is_file() {
        return fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    }
    let mut total = 0_u64;
    let mut stack: Vec<PathBuf> = vec![path.to_path_buf()];
    while let Some(curr) = stack.pop() {
        let Ok(entries) = fs::read_dir(&curr) else {
            continue;
        };
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
            } else if let Ok(meta) = entry.metadata() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    total
}

/// Actualiza la marca temporal de último uso dentro de un subdirectorio de proyecto compilado.
pub fn touch_dir_last_used(dir: &Path) {
    if fs::create_dir_all(dir).is_err() {
        return;
    }
    let marker = dir.join(LAST_USED_MARKER_FILE);
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis();
    let _ = fs::write(&marker, now_ms.to_string().as_bytes());
    touch_file_last_used(&marker);
}

/// Actualiza el `mtime` de un archivo en disco al instante actual para mantener orden LRU exacto.
pub fn touch_file_last_used(file: &Path) {
    if let Ok(handle) = OpenOptions::new().write(true).open(file) {
        let _ = handle.set_modified(SystemTime::now());
    }
}

fn entry_last_used_time(dir: &Path) -> SystemTime {
    let marker = dir.join(LAST_USED_MARKER_FILE);
    if let Ok(meta) = fs::metadata(&marker) {
        if let Ok(modified) = meta.modified() {
            return modified;
        }
    }
    let manifest = dir.join("manifest.json");
    if let Ok(meta) = fs::metadata(&manifest) {
        if let Ok(modified) = meta.modified() {
            return modified;
        }
    }
    fs::metadata(dir)
        .and_then(|m| m.modified())
        .unwrap_or(UNIX_EPOCH)
}

/// Elimina artefactos planos heredados de versiones anteriores situados directamente en `compiled_cache/`
/// antes de adoptar la estructura multi-proyecto `compiled_cache/<projectId>/`.
pub fn cleanup_legacy_flat_compiled_cache(compiled_root: &Path) {
    if !compiled_root.is_dir() {
        return;
    }
    let legacy_manifest = compiled_root.join("manifest.json");
    let legacy_mix = compiled_root.join("compiled_mix.wav");
    let legacy_scripts = compiled_root.join("scripts");
    if !legacy_manifest.exists() && !legacy_mix.exists() && !legacy_scripts.exists() {
        return;
    }
    let Ok(entries) = fs::read_dir(compiled_root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if path.is_dir() && name == "scripts" {
            let _ = fs::remove_dir_all(&path);
        } else if path.is_file() {
            let _ = fs::remove_file(&path);
        }
    }
}

/// Aplica expulsión LRU sobre subdirectorios de `root_dir` hasta no exceder `max_bytes`,
/// protegiendo siempre los directorios cuyo nombre coincida con `protected_names` (proyecto activo).
pub fn evict_lru_subdirs(
    root_dir: &Path,
    max_bytes: u64,
    protected_names: &[&str],
) -> LruEvictionReport {
    if !root_dir.is_dir() {
        return LruEvictionReport::default();
    }

    struct DirCandidate {
        name: String,
        path: PathBuf,
        size_bytes: u64,
        last_used: SystemTime,
        protected: bool,
    }

    let Ok(entries) = fs::read_dir(root_dir) else {
        return LruEvictionReport::default();
    };

    let mut total_bytes = 0_u64;
    let mut candidates: Vec<DirCandidate> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        let size_bytes = dir_size_bytes(&path);
        total_bytes = total_bytes.saturating_add(size_bytes);
        let protected = protected_names.iter().any(|p| *p == name);
        candidates.push(DirCandidate {
            name,
            last_used: entry_last_used_time(&path),
            path,
            size_bytes,
            protected,
        });
    }

    let bytes_before = total_bytes;
    if total_bytes <= max_bytes {
        return LruEvictionReport {
            bytes_before,
            bytes_after: total_bytes,
            evicted_entries: 0,
            bytes_reclaimed: 0,
        };
    }

    // Ordenar del más antiguo al más reciente (desempate determinista por nombre).
    candidates.sort_by(|a, b| {
        a.last_used
            .cmp(&b.last_used)
            .then_with(|| a.name.cmp(&b.name))
    });

    let mut evicted_entries = 0_usize;
    let mut bytes_reclaimed = 0_u64;
    for cand in candidates {
        if total_bytes <= max_bytes {
            break;
        }
        if cand.protected {
            continue;
        }
        if fs::remove_dir_all(&cand.path).is_ok() {
            total_bytes = total_bytes.saturating_sub(cand.size_bytes);
            bytes_reclaimed = bytes_reclaimed.saturating_add(cand.size_bytes);
            evicted_entries += 1;
        }
    }

    LruEvictionReport {
        bytes_before,
        bytes_after: total_bytes,
        evicted_entries,
        bytes_reclaimed,
    }
}

/// Aplica expulsión LRU sobre archivos individuales dentro de `root_dir` (por ejemplo `.bfaud` en `audio_cache/`),
/// manteniendo protegidos los caminos listados en `protected_paths`.
pub fn evict_lru_files(
    root_dir: &Path,
    max_bytes: u64,
    protected_paths: &[&Path],
) -> LruEvictionReport {
    if !root_dir.is_dir() {
        return LruEvictionReport::default();
    }

    struct FileCandidate {
        path: PathBuf,
        size_bytes: u64,
        last_used: SystemTime,
        protected: bool,
    }

    let Ok(entries) = fs::read_dir(root_dir) else {
        return LruEvictionReport::default();
    };

    let mut total_bytes = 0_u64;
    let mut candidates: Vec<FileCandidate> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let size_bytes = meta.len();
        total_bytes = total_bytes.saturating_add(size_bytes);
        let protected = protected_paths.iter().any(|p| *p == path.as_path());
        let last_used = meta.modified().unwrap_or(UNIX_EPOCH);
        candidates.push(FileCandidate {
            path,
            size_bytes,
            last_used,
            protected,
        });
    }

    let bytes_before = total_bytes;
    if total_bytes <= max_bytes {
        return LruEvictionReport {
            bytes_before,
            bytes_after: total_bytes,
            evicted_entries: 0,
            bytes_reclaimed: 0,
        };
    }

    candidates.sort_by(|a, b| {
        a.last_used
            .cmp(&b.last_used)
            .then_with(|| a.path.cmp(&b.path))
    });

    let mut evicted_entries = 0_usize;
    let mut bytes_reclaimed = 0_u64;
    for cand in candidates {
        if total_bytes <= max_bytes {
            break;
        }
        if cand.protected {
            continue;
        }
        if fs::remove_file(&cand.path).is_ok() {
            total_bytes = total_bytes.saturating_sub(cand.size_bytes);
            bytes_reclaimed = bytes_reclaimed.saturating_add(cand.size_bytes);
            evicted_entries += 1;
        }
    }

    LruEvictionReport {
        bytes_before,
        bytes_after: total_bytes,
        evicted_entries,
        bytes_reclaimed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_split_clamps_within_mobile_bounds() {
        let tiny = compute_cache_budget_split(4 * 1024 * 1024);
        assert_eq!(tiny.total_bytes, MIN_CACHE_QUOTA_BYTES);
        assert!(tiny.compiled_cache_bytes >= 32 * 1024 * 1024);
        assert!(tiny.audio_cache_bytes >= 16 * 1024 * 1024);

        let huge = compute_cache_budget_split(2 * 1024 * 1024 * 1024);
        assert_eq!(huge.total_bytes, MAX_CACHE_QUOTA_BYTES);
    }

    #[test]
    fn evict_lru_subdirs_removes_oldest_and_protects_active_project() {
        let root = std::env::temp_dir().join("bf_disk_cache_lru_test");
        let _ = fs::remove_dir_all(&root);
        let dir_old = root.join("proj_old");
        let dir_mid = root.join("proj_mid");
        let dir_active = root.join("proj_active");
        fs::create_dir_all(&dir_old).unwrap();
        fs::create_dir_all(&dir_mid).unwrap();
        fs::create_dir_all(&dir_active).unwrap();

        fs::write(dir_old.join("a.bin"), vec![1u8; 600]).unwrap();
        fs::write(dir_mid.join("b.bin"), vec![2u8; 600]).unwrap();
        fs::write(dir_active.join("c.bin"), vec![3u8; 600]).unwrap();

        // Establecer timestamps deterministas: old < active < mid
        let old_marker = dir_old.join(LAST_USED_MARKER_FILE);
        let active_marker = dir_active.join(LAST_USED_MARKER_FILE);
        let mid_marker = dir_mid.join(LAST_USED_MARKER_FILE);
        fs::write(&old_marker, b"1").unwrap();
        fs::write(&active_marker, b"2").unwrap();
        fs::write(&mid_marker, b"3").unwrap();

        let base = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        OpenOptions::new()
            .write(true)
            .open(&old_marker)
            .unwrap()
            .set_modified(base)
            .unwrap();
        OpenOptions::new()
            .write(true)
            .open(&active_marker)
            .unwrap()
            .set_modified(base + Duration::from_secs(10))
            .unwrap();
        OpenOptions::new()
            .write(true)
            .open(&mid_marker)
            .unwrap()
            .set_modified(base + Duration::from_secs(20))
            .unwrap();

        let rep = evict_lru_subdirs(&root, 1250, &["proj_active"]);
        assert_eq!(rep.evicted_entries, 1);
        assert!(!dir_old.exists());
        assert!(dir_active.exists());
        assert!(dir_mid.exists());

        let _ = fs::remove_dir_all(&root);
    }
}
