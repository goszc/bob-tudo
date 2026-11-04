use std::path::Path;
use std::sync::atomic::Ordering;

use bob_core::{children_of, biggest, search, Entry};
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
    if app.state::<AppState>().scanning.swap(true, Ordering::SeqCst) {
        return Err("já existe uma varredura".into());
    }
    let cancel = {
        let flag = app.state::<AppState>().lock().scan_cancel.clone();
        flag.store(false, Ordering::SeqCst);
        flag
    };
    let progress = app.clone();
    let scanned = path.clone();
    let result = tokio::task::spawn_blocking(move || {
        bob_core::scan_path(Path::new(&scanned), &cancel, |step| {
            let _ = progress.emit(
                "bob",
                BobEvent::ScanProgress {
                    files: step.files,
                    bytes: step.bytes,
                    path: step.current,
                },
            );
        })
    })
    .await
    .map_err(|err| err.to_string());
    app.state::<AppState>().scanning.store(false, Ordering::SeqCst);
    let index = result??;
    let summary = ScanSummary {
        root: index.root.clone(),
        total: index.total,
        files: index.files,
        errors: index.errors,
    };
    app.state::<AppState>().lock().scan = Some(index);
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
