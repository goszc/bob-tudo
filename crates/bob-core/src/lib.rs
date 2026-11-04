//! Lógica do bob tudo que não depende da janela: varredura, duplicados,
//! parsers e catálogos. O que fala com o windows fica no binário tauri.

mod catalog;
mod clean;
mod guard;
mod health;
mod impact;
mod kind;
mod phash;
mod privacy;
mod scan;
mod dupes;
mod version;
mod winget;

pub use catalog::{oss_catalog, OssApp};
pub use clean::{measure_paths, purge_paths, PurgeReport};
pub use dupes::{find_duplicates, DupeFile, DupeGroup, DupeProgress};
pub use guard::protected_path;
pub use health::{assess, parse_smartctl, parse_storage_json, DiskHealth};
pub use impact::impact;
pub use kind::kind_of;
pub use phash::{dhash, hamming};
pub use privacy::{privacy_toggles, PrivacyToggle, RegDesired, RegEntry};
pub use scan::{
    biggest, children_of, forget, search, scan_path, Entry, ScanIndex, ScanProgress,
};
pub use version::newer;
pub use winget::{parse_winget_table, WingetRow};

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
