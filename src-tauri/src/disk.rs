use std::path::Path;
use std::sync::atomic::Ordering;

use bob_core::{biggest, cache_from_index, catch_up, children_of, forget, index_from_cache, mark_of, note_file, relax_thread, scan_path_cached, search, seed_volume, set_file_size, Entry, FileHit};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::hidden;
use crate::state::{AppState, BobEvent};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub root: String,
    pub total: u64,
    pub files: u64,
    pub errors: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteReport {
    pub deleted: Vec<String>,
    pub failed: Vec<DeleteFail>,
}

#[derive(Serialize)]
pub struct DeleteFail {
    pub path: String,
    pub error: String,
}

#[tauri::command]
pub fn list_roots() -> Vec<String> {
    #[cfg(windows)]
    {
        let mut roots = Vec::new();
        for letter in b'A'..=b'Z' {
            let path = format!("{}:\\", letter as char);
            if Path::new(&path).exists() {
                roots.push(path);
            }
        }
        if roots.is_empty() {
            roots.push("C:\\".into());
        }
        roots
    }
    #[cfg(not(windows))]
    {
        let mut roots = vec!["/".into()];
        if let Some(home) = dirs::home_dir() {
            roots.push(home.display().to_string());
        }
        roots
    }
}

#[tauri::command]
pub async fn start_scan(app: AppHandle, path: String) -> Result<ScanSummary, String> {
    run_disk(app, path, false).await
}

pub async fn run_disk(app: AppHandle, path: String, quiet: bool) -> Result<ScanSummary, String> {
    if app.state::<AppState>().scanning.swap(true, Ordering::SeqCst) {
        return Err("já existe uma varredura".into());
    }
    let id = crate::jobs::begin(&app, "mapa do disco", "disk", quiet);
    if let Some(summary) = apply_usn(&app, &path) {
        crate::jobs::mark_updating(&app, "disk", false);
        crate::jobs::finish(&app, &id, true, "mapa ao vivo");
        app.state::<AppState>().scanning.store(false, Ordering::SeqCst);
        return Ok(summary);
    }
    crate::jobs::mark_updating(&app, "disk", true);
    let cancel = {
        let flag = app.state::<AppState>().lock().scan_cancel.clone();
        flag.store(false, Ordering::SeqCst);
        flag
    };
    let progress = app.clone();
    let job_id = id.clone();
    let tree_path = app.state::<AppState>().lock().data_dir.join("disk-tree.json");
    let scanned = path.clone();
    let result = tokio::task::spawn_blocking(move || {
        relax_thread();
        let cache = crate::preload::load_tree(&tree_path);
        let index = scan_path_cached(Path::new(&scanned), &cancel, |step| {
            crate::jobs::tick(
                &progress,
                &job_id,
                -1.0,
                &format!("{} arquivos", step.files),
            );
            let _ = progress.emit(
                "bob",
                BobEvent::ScanProgress {
                    files: step.files,
                    bytes: step.bytes,
                    path: step.current,
                },
            );
        }, cache.as_ref())?;
        crate::preload::store_tree(&tree_path, &index);
        Ok::<_, String>(index)
    })
    .await
    .map_err(|err| err.to_string());
    app.state::<AppState>().scanning.store(false, Ordering::SeqCst);
    let index = match result {
        Ok(Ok(index)) => index,
        Ok(Err(err)) => {
            crate::jobs::mark_updating(&app, "disk", false);
            crate::jobs::finish(&app, &id, false, &err);
            return Err(err);
        }
        Err(err) => {
            crate::jobs::mark_updating(&app, "disk", false);
            crate::jobs::finish(&app, &id, false, &err);
            return Err(err);
        }
    };
    let summary = ScanSummary {
        root: index.root.clone(),
        total: index.total,
        files: index.files,
        errors: index.errors,
    };
    seed_names(&path, &index);
    remember_folders(&index);
    crate::preload::store_snapshot(&app, &index);
    crate::jobs::mark_updating(&app, "disk", false);
    crate::jobs::finish(&app, &id, true, format!("{} arquivos", summary.files));
    crate::preload::refresh_suggestions(&app);
    Ok(summary)
}

#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) {
    state.lock().scan_cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
pub fn scan_children(state: State<'_, AppState>, path: String) -> Result<Vec<Entry>, String> {
    let shared = state.lock();
    let index = shared.scan.as_ref().ok_or("nenhuma varredura")?;
    Ok(children_of(index, &path))
}

#[tauri::command]
pub fn scan_biggest(state: State<'_, AppState>, folders: bool, query: String) -> Result<Vec<Entry>, String> {
    let shared = state.lock();
    let index = shared.scan.as_ref().ok_or("nenhuma varredura")?;
    if query.trim().is_empty() {
        Ok(biggest(index, folders, ""))
    } else if folders {
        Ok(biggest(index, true, &query))
    } else {
        Ok(search(index, &query))
    }
}

#[tauri::command]
pub fn delete_to_trash(state: State<'_, AppState>, paths: Vec<String>) -> Result<DeleteReport, String> {
    if paths.is_empty() {
        return Err("nada selecionado".into());
    }
    let mut deleted = Vec::new();
    let mut failed = Vec::new();
    for path in paths {
        if path.is_empty() || bob_core::protected_path(Path::new(&path)) {
            failed.push(DeleteFail {
                path,
                error: "caminho protegido".into(),
            });
            continue;
        }
        match trash::delete(Path::new(&path)) {
            Ok(()) => {
                if let Some(index) = state.lock().scan.as_mut() {
                    bob_core::forget(index, &path);
                }
                deleted.push(path);
            }
            Err(err) => failed.push(DeleteFail {
                path,
                error: err.to_string(),
            }),
        }
    }
    Ok(DeleteReport { deleted, failed })
}

#[tauri::command]
pub fn open_in_explorer(path: String) -> Result<(), String> {
    if path.is_empty() {
        return Err("caminho vazio".into());
    }
    #[cfg(windows)]
    {
        hidden::std_command("explorer")
            .arg(format!("/select,{path}"))
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    #[cfg(not(windows))]
    {
        let target = if Path::new(&path).is_dir() {
            path
        } else {
            Path::new(&path)
                .parent()
                .map(|parent| parent.display().to_string())
                .unwrap_or(path)
        };
        hidden::std_command("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

fn apply_usn(app: &AppHandle, path: &str) -> Option<crate::disk::ScanSummary> {
    let report = catch_up(path)?;
    let tree_path = app.state::<AppState>().lock().data_dir.join("disk-tree.json");
    let cache = crate::preload::load_tree(&tree_path)?;
    if cache.root != path && !path.is_empty() {
        return None;
    }
    let mut index = index_from_cache(&cache);
    for touch in &report.touches {
        if touch.alive {
            let meta = std::fs::metadata(&touch.path).ok();
            let size = meta.as_ref().map(|item| item.len()).unwrap_or(touch.size);
            let modified = meta
                .and_then(|item| item.modified().ok())
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|time| time.as_millis() as u64)
                .unwrap_or(0);
            set_file_size(path, &touch.path, size, modified);
            if size > 0 {
                note_file(&mut index, &touch.path, &touch.name, size, &touch.kind);
            }
        } else {
            let _ = forget(&mut index, &touch.path);
        }
    }
    crate::preload::store_tree(&tree_path, &index);
    remember_folders(&index);
    crate::preload::store_snapshot(app, &index);
    app.state::<AppState>().lock().disk_live = true;
    Some(ScanSummary {
        root: index.root.clone(),
        total: index.total,
        files: index.files,
        errors: index.errors,
    })
}

fn seed_names(volume: &str, index: &bob_core::ScanIndex) {
    if mark_of(volume).is_some_and(|(_, _, source)| source == "mft") {
        return;
    }
    let cache = cache_from_index(index);
    let mut hits = Vec::new();
    for (path, dir) in &cache.dirs {
        let name = std::path::Path::new(path).file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_else(|| path.clone());
        hits.push(FileHit {
            name,
            path: path.clone(),
            parent: String::new(),
            size: dir.size,
            modified_ms: dir.mtime_ms,
            dir: true,
            kind: "pasta".into(),
        });
        for item in dir.items.iter().filter(|item| !item.synthetic) {
            hits.push(FileHit {
                name: item.name.clone(),
                path: item.path.clone(),
                parent: path.clone(),
                size: item.size,
                modified_ms: 0,
                dir: item.dir,
                kind: item.kind.clone(),
            });
        }
    }
    let _ = seed_volume(volume, &hits);
}

fn remember_folders(index: &bob_core::ScanIndex) {
    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    let children = children_of(index, &index.root);
    let folders: Vec<(String, u64)> = children.into_iter().filter(|entry| entry.dir).take(24).map(|entry| (entry.path, entry.size)).collect();
    bob_core::record_folders(ts, &folders);
}
