//! Lógica do bob tudo que não depende da janela: varredura, duplicados,
//! parsers e catálogos. O que fala com o windows fica no binário tauri.

mod catalog;
mod catalog_db;
mod catalog_model;
mod clean;
mod desk;
mod dupes;
mod fastwalk;
mod files;
mod guard;
mod health;
mod history;
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
pub use catalog_db::{extract_matching, find_index_db, read_winget_index, scoop_from_zip, CatalogIndex, Collection, SearchPage, SearchQuery};
pub use catalog_model::{
    chocolatey_install_script, curated_incoming, fts_query, github_repo, manager_from_probe, manager_name, merge, norm_name,
    paid_label, parse_choco_atom, parse_device_poll, parse_device_start, parse_github_releases, parse_github_repo, parse_scoop,
    render_readme, rfc3339_ms, scoop_install_script, version_from_output, watch_is_newer, winget_installer_assets, Asset,
    CatalogDetail, CatalogHit, DevicePoll, DeviceStart, Incoming, ManagerInfo, Offer, Release, RemoteAsset, APP_INSTALLER_STORE,
};
pub use clean::{measure_paths, purge_paths, PurgeReport};
pub use desk::{
    bundle_from_json, bundle_to_json, dns_presets, driver_hint, mbps, parse_battery_html, parse_driver_table, parse_ping,
    parse_power_plans, render_report_html, schedule_due, simple_pdf, BatteryReport, BundleApp, BundleCollection, DnsPreset,
    DriverInfo, PcBundle, PcSummary, PowerPlan, HIGH_PERFORMANCE,
};
pub use dupes::{find_duplicates, find_duplicates_with, DupeFile, DupeGroup, DupeProgress, HashStore};
pub use files::{
    apply_events, catch_up, file_count, ingest_volume, mark_of, open_files, search_files, seed_volume, set_file_size, FileHit,
    FileQuery, FileTouch, UsnReport,
};
pub use history::{bucket, grew, grew_folders, open_history, record_folders, record_sample, series, Grew, Point};
pub use guard::protected_path;
pub use health::{assess, parse_smartctl, parse_storage_json, DiskHealth};
pub use impact::impact;
pub use kind::kind_of;
pub use phash::{dhash, hamming};
pub use privacy::{privacy_toggles, PrivacyToggle, RegDesired, RegEntry};
pub use priority::{relax_process_keep_ui, relax_thread};
pub use scan::{
    biggest, cache_from_index, children_of, forget, index_from_cache, load_cache, note_file, save_cache, scan_path,
    scan_path_cached, search, snapshot_of, DirTreeCache, DiskSnapshot, Entry, FileStamp, ScanIndex, ScanProgress,
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
