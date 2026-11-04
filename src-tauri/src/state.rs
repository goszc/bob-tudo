use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use bob_core::{DirTreeCache, DiskHealth, DiskSnapshot, DupeGroup, HashStore, ScanIndex, Suggestion, WingetRow};
use serde::{Deserialize, Serialize};

use crate::clean::CleanCategory;
use crate::system::StartupItem;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub theme: String,
    #[serde(default)]
    pub theme_glass: bool,
    #[serde(default = "yes")]
    pub minimize_to_tray: bool,
    #[serde(default = "yes")]
    pub notifications: bool,
    #[serde(default = "yes")]
    pub auto_update: bool,
    #[serde(default)]
    pub update_url: String,
    #[serde(default = "yes")]
    pub start_with_windows: bool,
    #[serde(default = "yes")]
    pub notify_disk_full: bool,
    #[serde(default = "yes")]
    pub notify_app_updates: bool,
    #[serde(default = "yes")]
    pub notify_junk: bool,
    #[serde(default = "yes")]
    pub notify_smart: bool,
    #[serde(default = "yes")]
    pub notify_task_done: bool,
}

fn yes() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            theme_glass: false,
            minimize_to_tray: true,
            notifications: true,
            auto_update: true,
            update_url: String::new(),
            start_with_windows: true,
            notify_disk_full: true,
            notify_app_updates: true,
            notify_junk: true,
            notify_smart: true,
            notify_task_done: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UndoOp {
    Reg {
        hive: String,
        path: String,
        name: String,
        existed: bool,
        dword: Option<u32>,
        sz: Option<String>,
    },
    Rename {
        from: String,
        to: String,
    },
    Service {
        name: String,
        start: String,
    },
    Task {
        name: String,
        enabled: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub id: String,
    pub at: String,
    pub summary: String,
    pub undone: bool,
    pub ops: Vec<UndoOp>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BobEvent {
    ScanProgress { files: u64, bytes: u64, path: String },
    DupeProgress { phase: String, done: u64, total: u64 },
    JobLog { line: String },
    Toast { message: String },
    Update { version: String },
    Jobs { jobs: Vec<JobView> },
    Navigate { page: String },
    Disk { snapshot: DiskSnapshot },
    Clean { items: Vec<CleanCategory> },
    Health { disks: Vec<DiskHealth> },
    Startup { items: Vec<StartupItem> },
    Upgrades { rows: Vec<WingetRow> },
    Suggestions { items: Vec<Suggestion> },
    Updating { area: String, on: bool },
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: String,
    pub title: String,
    pub page: String,
    pub progress: f32,
    pub detail: String,
    pub state: String,
    #[serde(skip)]
    pub quiet: bool,
}

pub struct Shared {
    pub config: Config,
    pub data_dir: PathBuf,
    pub scan: Option<ScanIndex>,
    pub scan_cancel: Arc<AtomicBool>,
    pub dupe_cancel: Arc<AtomicBool>,
    pub dupes: Vec<DupeGroup>,
    pub pending_installer: Option<PathBuf>,
    pub changes: Vec<Change>,
    pub last_restore: Option<Instant>,
    pub privacy_saved: Vec<SavedPrivacy>,
    pub disabled_startup: Vec<DisabledStartup>,
    pub jobs: Vec<JobView>,
    pub disk: Option<DiskSnapshot>,
    pub tree: Option<DirTreeCache>,
    pub clean: Vec<CleanCategory>,
    pub health: Vec<DiskHealth>,
    pub startup: Vec<StartupItem>,
    pub upgrades: Vec<WingetRow>,
    pub suggestions: Vec<Suggestion>,
    pub updating: Vec<String>,
    pub upgrades_fresh: bool,
    pub search_cache: std::collections::HashMap<String, Vec<WingetRow>>,
    pub hash_store: HashStore,
    pub notified: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPrivacy {
    pub id: String,
    pub ops: Vec<UndoOp>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisabledStartup {
    pub id: String,
    pub name: String,
    pub command: String,
}

pub struct AppState {
    pub shared: Mutex<Shared>,
    pub scanning: AtomicBool,
    pub duping: AtomicBool,
}

impl AppState {
    pub fn new() -> Self {
        let data_dir = dirs::data_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("bob tudo");
        let _ = std::fs::create_dir_all(&data_dir);
        let config = read_json(&data_dir.join("config.json")).unwrap_or_default();
        let changes = read_json(&data_dir.join("journal.json")).unwrap_or_default();
        let privacy_saved = read_json(&data_dir.join("privacy.json")).unwrap_or_default();
        let disabled_startup = read_json(&data_dir.join("startup-off.json")).unwrap_or_default();
        let disk = read_json(&data_dir.join("disk-snapshot.json"));
        let clean = read_json(&data_dir.join("clean.json")).unwrap_or_default();
        let health = read_json(&data_dir.join("health.json")).unwrap_or_default();
        let startup = read_json(&data_dir.join("startup.json")).unwrap_or_default();
        let upgrades = read_json(&data_dir.join("upgrades.json")).unwrap_or_default();
        let suggestions = read_json(&data_dir.join("suggestions.json")).unwrap_or_default();
        let hash_store = read_json(&data_dir.join("hash-cache.json")).unwrap_or_default();
        let search_cache = read_json(&data_dir.join("winget-search.json")).unwrap_or_default();
        Self {
            shared: Mutex::new(Shared {
                config,
                data_dir,
                scan: None,
                scan_cancel: Arc::new(AtomicBool::new(false)),
                dupe_cancel: Arc::new(AtomicBool::new(false)),
                dupes: Vec::new(),
                pending_installer: None,
                changes,
                last_restore: None,
                privacy_saved,
                disabled_startup,
                jobs: Vec::new(),
                disk,
                tree: None,
                clean,
                health,
                startup,
                upgrades,
                suggestions,
                updating: Vec::new(),
                upgrades_fresh: false,
                search_cache,
                hash_store,
                notified: Vec::new(),
            }),
            scanning: AtomicBool::new(false),
            duping: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|err| err.into_inner())
    }
}

pub fn save_config(shared: &Shared) {
    let _ = std::fs::write(
        shared.data_dir.join("config.json"),
        serde_json::to_vec_pretty(&shared.config).unwrap_or_default(),
    );
}

pub fn save_changes(shared: &Shared) {
    let _ = std::fs::write(
        shared.data_dir.join("journal.json"),
        serde_json::to_vec_pretty(&shared.changes).unwrap_or_default(),
    );
}

pub fn save_privacy(shared: &Shared) {
    let _ = std::fs::write(
        shared.data_dir.join("privacy.json"),
        serde_json::to_vec_pretty(&shared.privacy_saved).unwrap_or_default(),
    );
}

pub fn save_startup(shared: &Shared) {
    let _ = std::fs::write(
        shared.data_dir.join("startup-off.json"),
        serde_json::to_vec_pretty(&shared.disabled_startup).unwrap_or_default(),
    );
}

pub fn push_change(shared: &mut Shared, summary: impl Into<String>, ops: Vec<UndoOp>) {
    if ops.is_empty() {
        return;
    }
    shared.changes.insert(
        0,
        Change {
            id: bob_core::nid("c"),
            at: chrono::Utc::now().to_rfc3339(),
            summary: summary.into(),
            undone: false,
            ops,
        },
    );
    save_changes(shared);
}

pub fn write_json<T: Serialize>(path: &std::path::Path, value: &T) {
    if let Ok(bytes) = serde_json::to_vec_pretty(value) {
        let _ = std::fs::write(path, bytes);
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &std::path::Path) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}
