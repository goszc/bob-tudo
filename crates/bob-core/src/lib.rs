//! Lógica do bob tudo que não depende da janela: varredura, duplicados,
//! parsers e catálogos. O que fala com o windows fica no binário tauri.

mod catalog;
mod clean;
mod dupes;
mod fastwalk;
mod guard;
mod health;
mod impact;
mod kind;
mod ntfs;
mod phash;
mod priority;
mod privacy;
mod scan;
mod suggest;
mod version;
mod winget;

pub use catalog::{oss_catalog, OssApp};
pub use clean::{measure_paths, purge_paths, PurgeReport};
pub use dupes::{find_duplicates, find_duplicates_with, DupeFile, DupeGroup, DupeProgress, HashStore};
pub use guard::protected_path;
pub use health::{assess, parse_smartctl, parse_storage_json, DiskHealth};
pub use impact::impact;
pub use kind::kind_of;
pub use phash::{dhash, hamming};
pub use privacy::{privacy_toggles, PrivacyToggle, RegDesired, RegEntry};
pub use priority::{relax_process_keep_ui, relax_thread};
pub use scan::{
    biggest, cache_from_index, children_of, forget, load_cache, save_cache, scan_path, scan_path_cached, search,
    snapshot_of, DirTreeCache, DiskSnapshot, Entry, FileStamp, ScanIndex, ScanProgress,
};
pub use suggest::{
    build_suggestions, disk_almost_full, junk_worth_mentioning, temp_or_smart, Signals, Suggestion, DISK_FULL_RATIO,
    HOT_TEMP_C, JUNK_THRESHOLD,
};
pub use version::newer;
pub use winget::{parse_winget_export, parse_winget_table, WingetRow};

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn nid(prefix: &str) -> String {
    static N: AtomicU64 = AtomicU64::new(1);
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!(
        "{prefix}{t:x}{:x}",
        N.fetch_add(1, Ordering::Relaxed)
    )
}

pub fn norm_path(path: &std::path::Path) -> String {
    #[cfg(windows)]
    {
        let raw = path.to_string_lossy().to_string();
        let raw = raw.strip_prefix(r"\\?\").unwrap_or(&raw).to_string();
        let mut s = raw.replace('/', "\\");
        if s.len() == 2 && s.ends_with(':') {
            s.push('\\');
            return s;
        }
        while s.len() > 3 && s.ends_with('\\') {
            s.pop();
        }
        return s;
    }
    #[cfg(not(windows))]
    {
        let s = path.to_string_lossy().to_string();
        if s.len() > 1 {
            s.trim_end_matches('/').to_string()
        } else {
            s
        }
    }
}
