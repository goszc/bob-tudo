use std::path::Path;
use std::sync::Arc;

use bob_core::{build_suggestions, cache_from_index, load_cache, save_cache, snapshot_of, Signals, JUNK_THRESHOLD};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Semaphore;

use crate::jobs;
use crate::state::{write_json, AppState, BobEvent};

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let permits = Arc::new(Semaphore::new(2));
        let mut tasks = Vec::new();
        for kind in ["disk", "clean", "upgrades", "health", "startup"] {
            let app = app.clone();
            let permits = permits.clone();
            tasks.push(tauri::async_runtime::spawn(async move {
                let _permit = permits.acquire_owned().await;
                match kind {
                    "disk" => disk(app).await,
                    "clean" => clean(app).await,
                    "upgrades" => upgrades(app).await,
                    "health" => health(app).await,
                    "startup" => startup(app).await,
                    _ => {}
                }
            }));
        }
        for task in tasks {
            let _ = task.await;
        }
    });
}

async fn disk(app: AppHandle) {
    let root = default_root();
    let _ = crate::disk::run_disk(app, root, true).await;
}

async fn clean(app: AppHandle) {
    let id = jobs::begin(&app, "limpeza", "clean", true);
    jobs::mark_updating(&app, "clean", true);
    let items = tokio::task::spawn_blocking(crate::clean::clean_preview)
        .await
        .unwrap_or_default();
    let junk: u64 = items.iter().filter(|item| item.available).map(|item| item.bytes).sum();
    {
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        write_json(&shared.data_dir.join("clean.json"), &items);
        shared.clean = items.clone();
    }
    let _ = app.emit("bob", BobEvent::Clean { items });
    jobs::mark_updating(&app, "clean", false);
    jobs::finish(&app, &id, true, "medido");
    if junk >= JUNK_THRESHOLD {
        note(
            &app,
            "junk",
            "junk",
            &format!("tem cerca de {} de lixo acumulado", human(junk)),
        );
    }
    refresh_suggestions(&app);
}

async fn upgrades(app: AppHandle) {
    let id = jobs::begin(&app, "atualizações", "upgrades", true);
    jobs::mark_updating(&app, "upgrades", true);
    let rows = crate::apps::refresh_upgrades(&app).await.unwrap_or_default();
    jobs::mark_updating(&app, "upgrades", false);
    jobs::finish(&app, &id, true, format!("{} apps", rows.len()));
    if !rows.is_empty() {
        note(
            &app,
            "updates",
            "updates",
            &format!("{} aplicativo(s) com atualização", rows.len()),
        );
    }
    refresh_suggestions(&app);
}

async fn health(app: AppHandle) {
    let id = jobs::begin(&app, "saúde dos discos", "health", true);
    jobs::mark_updating(&app, "health", true);
    let disks = crate::system::disk_health(Some(false)).await.unwrap_or_default();
    {
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        write_json(&shared.data_dir.join("health.json"), &disks);
        shared.health = disks.clone();
    }
    let _ = app.emit("bob", BobEvent::Health { disks: disks.clone() });
    jobs::mark_updating(&app, "health", false);
    jobs::finish(&app, &id, true, format!("{} discos", disks.len()));
    if let Some(alert) = disks.iter().find_map(|disk| disk.alert.clone()) {
        note(&app, "smart", "smart", &alert);
    } else if let Some(temp) = disks.iter().filter_map(|disk| disk.temp_c).find(|temp| *temp >= bob_core::HOT_TEMP_C) {
        note(&app, "smart", "smart", &format!("disco a {temp} °c"));
    }
    refresh_suggestions(&app);
}

async fn startup(app: AppHandle) {
    let id = jobs::begin(&app, "inicialização", "startup", true);
    jobs::mark_updating(&app, "startup", true);
    let items = crate::system::startup_list(app.clone()).await.unwrap_or_default();
    {
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        write_json(&shared.data_dir.join("startup.json"), &items);
        shared.startup = items.clone();
    }
    let _ = app.emit("bob", BobEvent::Startup { items });
    jobs::mark_updating(&app, "startup", false);
    jobs::finish(&app, &id, true, "lido");
    refresh_suggestions(&app);
}

pub fn refresh_suggestions(app: &AppHandle) {
    let (junk, upgrades, health, disk_root) = {
        let state = app.state::<AppState>();
        let shared = state.lock();
        let junk = shared.clean.iter().filter(|item| item.available).map(|item| item.bytes).sum();
        let upgrades = shared.upgrades.len();
        let alert = shared.health.iter().find_map(|disk| disk.alert.clone());
        let temp = shared.health.iter().filter_map(|disk| disk.temp_c).max();
        let heavy = shared.startup.iter().filter(|item| item.impact == "alto" && item.enabled).count();
        let root = shared.disk.as_ref().map(|disk| disk.root.clone());
        (junk, upgrades, (alert, temp, heavy), root)
    };
    let (ratio, free) = disk_root.as_deref().and_then(volume_usage).unwrap_or((None, None));
    let signals = Signals {
        disk_used_ratio: ratio,
        disk_free: free,
        junk_bytes: junk,
        upgrade_count: upgrades,
        smart_alert: health.0,
        temp_c: health.1,
        heavy_startup: health.2,
    };
    if let Some(ratio) = ratio {
        if bob_core::disk_almost_full(ratio) {
            note(
                app,
                "disk",
                "disk",
                &format!("disco quase cheio, restam {}", human(free.unwrap_or(0))),
            );
        }
    }
    let suggestions = build_suggestions(&signals);
    {
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        write_json(&shared.data_dir.join("suggestions.json"), &suggestions);
        shared.suggestions = suggestions.clone();
    }
    let _ = app.emit("bob", BobEvent::Suggestions { items: suggestions });
}

fn note(app: &AppHandle, key: &str, kind: &str, body: &str) {
    let state = app.state::<AppState>();
    let mut shared = state.lock();
    if shared.notified.iter().any(|item| item == key) {
        return;
    }
    shared.notified.push(key.into());
    drop(shared);
    crate::system::notify_kind(app, kind, body);
}

pub fn default_root() -> String {
    #[cfg(windows)]
    {
        if Path::new("C:\\").exists() {
            return "C:\\".into();
        }
    }
    crate::disk::list_roots().into_iter().next().unwrap_or_else(|| "/".into())
}

fn volume_usage(root: &str) -> Option<(Option<f64>, Option<u64>)> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut best: Option<(usize, f64, u64)> = None;
    for disk in disks.list() {
        let mount = disk.mount_point().to_string_lossy().replace('/', "\\");
        let root_key = root.replace('/', "\\");
        if !root_key.starts_with(mount.trim_end_matches('\\')) && !mount.starts_with(root_key.trim_end_matches('\\')) {
            continue;
        }
        let total = disk.total_space();
        if total == 0 {
            continue;
        }
        let free = disk.available_space();
        let ratio = total.saturating_sub(free) as f64 / total as f64;
        let score = mount.len();
        if best.as_ref().map(|item| score >= item.0).unwrap_or(true) {
            best = Some((score, ratio, free));
        }
    }
    best.map(|(_, ratio, free)| (Some(ratio), Some(free)))
}

fn human(bytes: u64) -> String {
    let mut value = bytes as f64;
    let units = ["b", "kb", "mb", "gb"];
    let mut unit = 0;
    while value >= 1024.0 && unit < 3 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", units[unit])
    } else {
        format!("{value:.0} {}", units[unit])
    }
}

pub fn load_tree(path: &Path) -> Option<bob_core::DirTreeCache> {
    let bytes = std::fs::read(path).ok()?;
    load_cache(&bytes)
}

pub fn store_tree(path: &Path, index: &bob_core::ScanIndex) {
    let cache = cache_from_index(index);
    let _ = std::fs::write(path, save_cache(&cache));
}

pub fn store_snapshot(app: &AppHandle, index: &bob_core::ScanIndex) {
    let snapshot = snapshot_of(index);
    {
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        write_json(&shared.data_dir.join("disk-snapshot.json"), &snapshot);
        shared.disk = Some(snapshot.clone());
        shared.scan = Some(index.clone());
    }
    let _ = app.emit("bob", BobEvent::Disk { snapshot });
}
