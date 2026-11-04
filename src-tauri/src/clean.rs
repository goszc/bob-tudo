use std::path::{Path, PathBuf};

use bob_core::{measure_paths, purge_paths, PurgeReport};
use serde::Serialize;

use tauri::Manager;

use crate::elevate::{elevate_sync, Task};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanCategory {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub needs_admin: bool,
    pub bytes: u64,
    pub files: u64,
    pub measured: bool,
    pub available: bool,
    pub checked: bool,
}

#[tauri::command]
pub fn clean_preview() -> Vec<CleanCategory> {
    specs()
        .into_iter()
        .map(|spec| {
            let (bytes, files, measured) = match spec.kind {
                Kind::Paths | Kind::WinUpdate => {
                    let existing: Vec<_> = spec.paths.iter().filter(|path| path.exists()).cloned().collect();
                    if existing.is_empty() {
                        (0, 0, true)
                    } else {
                        let (bytes, files) = measure_paths(&existing);
                        (bytes, files, true)
                    }
                }
                Kind::Recycle => recycle_size(),
                Kind::Dism => (0, 0, false),
            };
            let available = match spec.kind {
                Kind::Dism => cfg!(windows),
                Kind::WinUpdate => cfg!(windows),
                _ => files > 0 || bytes > 0,
            };
            CleanCategory {
                id: spec.id,
                title: spec.title,
                detail: spec.detail,
                needs_admin: spec.needs_admin,
                bytes,
                files,
                measured,
                available,
                checked: spec.checked && available,
            }
        })
        .collect()
}

#[tauri::command]
pub async fn clean_run(app: tauri::AppHandle, ids: Vec<String>) -> Result<Vec<CleanDone>, String> {
    if ids.is_empty() {
        return Err("nada selecionado".into());
    }
    let mut done = Vec::new();
    for spec in specs() {
        if !ids.iter().any(|id| id == &spec.id) {
            continue;
        }
        let result = match spec.kind {
            Kind::Paths => {
                let existing: Vec<_> = spec.paths.iter().filter(|path| path.exists()).cloned().collect();
                let mut report = purge_paths(&existing);
                if report.skipped > 0 && spec.needs_admin {
                    let paths = existing.iter().map(|path| path.display().to_string()).collect();
                    match elevate_sync(&Task::DeleteTree { paths }) {
                        Ok(task) if task.ok => report.skipped = 0,
                        Ok(task) => return Err(task.message),
                        Err(err) => return Err(err),
                    }
                }
                report
            }
            Kind::Recycle => match elevate_sync(&Task::EmptyRecycle) {
                Ok(task) if task.ok => PurgeReport {
                    bytes: 0,
                    files: 0,
                    skipped: 0,
                },
                Ok(task) => {
                    let direct = hidden_recycle();
                    if direct.ok {
                        PurgeReport {
                            bytes: 0,
                            files: 0,
                            skipped: 0,
                        }
                    } else {
                        return Err(task.message);
                    }
                }
                Err(_) => {
                    let direct = hidden_recycle();
                    if direct.ok {
                        PurgeReport {
                            bytes: 0,
                            files: 0,
                            skipped: 0,
                        }
                    } else {
                        return Err(direct.message);
                    }
                }
            },
            Kind::WinUpdate => {
                let _ = crate::system::ensure_restore_point_state(app.state::<crate::state::AppState>().inner(), "limpeza do windows update");
                match elevate_sync(&Task::WindowsUpdateClean) {
                    Ok(task) if task.ok => PurgeReport {
                        bytes: 0,
                        files: 1,
                        skipped: 0,
                    },
                    Ok(task) => return Err(task.message),
                    Err(err) => return Err(err),
                }
            }
            Kind::Dism => {
                let _ = crate::system::ensure_restore_point_state(app.state::<crate::state::AppState>().inner(), "limpeza de componentes");
                match elevate_sync(&Task::DismClean) {
                    Ok(task) if task.ok => PurgeReport {
                        bytes: 0,
                        files: 1,
                        skipped: 0,
                    },
                    Ok(task) => return Err(task.message),
                    Err(err) => return Err(err),
                }
            }
        };
        done.push(CleanDone {
            id: spec.id,
            bytes: result.bytes,
            files: result.files,
            skipped: result.skipped,
        });
    }
    Ok(done)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanDone {
    pub id: String,
    pub bytes: u64,
    pub files: u64,
    pub skipped: u64,
}

struct Spec {
    id: String,
    title: String,
    detail: String,
    needs_admin: bool,
    checked: bool,
    kind: Kind,
    paths: Vec<PathBuf>,
}

enum Kind {
    Paths,
    Recycle,
    WinUpdate,
    Dism,
}

fn specs() -> Vec<Spec> {
    let local = dirs::data_local_dir().unwrap_or_default();
    let roaming = dirs::config_dir().unwrap_or_default();
    vec![
        Spec {
            id: "temp".into(),
            title: "arquivos temporários".into(),
            detail: "pastas temp do usuário e do windows. apagados de vez, sem ir para a lixeira.".into(),
            needs_admin: true,
            checked: true,
            kind: Kind::Paths,
            paths: {
                let mut paths = vec![std::env::temp_dir()];
                let user = local.join("Temp");
                if user != std::env::temp_dir() {
                    paths.push(user);
                }
                paths.push(PathBuf::from(r"C:\Windows\Temp"));
                paths
            },
        },
        browser("chrome", "cache do chrome", local.join(r"Google\Chrome\User Data")),
        browser("edge", "cache do edge", local.join(r"Microsoft\Edge\User Data")),
        browser("brave", "cache do brave", local.join(r"BraveSoftware\Brave-Browser\User Data")),
        Spec {
            id: "firefox".into(),
            title: "cache do firefox".into(),
            detail: "cache dos perfis do firefox. o histórico e as senhas ficam.".into(),
            needs_admin: false,
            checked: true,
            kind: Kind::Paths,
            paths: firefox_caches(&local),
        },
        Spec {
            id: "opera".into(),
            title: "cache do opera".into(),
            detail: "cache do opera e do opera gx.".into(),
            needs_admin: false,
            checked: true,
            kind: Kind::Paths,
            paths: opera_caches(&roaming),
        },
        Spec {
            id: "recycle".into(),
            title: "lixeira".into(),
            detail: "esvazia a lixeira do usuário.".into(),
            needs_admin: false,
            checked: false,
            kind: Kind::Recycle,
            paths: Vec::new(),
        },
        Spec {
            id: "winupdate".into(),
            title: "arquivos antigos do windows update".into(),
            detail: "esvazia a pasta de download do windows update. pede administrador.".into(),
            needs_admin: true,
            checked: false,
            kind: Kind::WinUpdate,
            paths: vec![PathBuf::from(r"C:\Windows\SoftwareDistribution\Download")],
        },
        Spec {
            id: "dism".into(),
            title: "limpeza de componentes".into(),
            detail: "roda o dism startcomponentcleanup. não usa resetbase. pede administrador.".into(),
            needs_admin: true,
            checked: false,
            kind: Kind::Dism,
            paths: Vec::new(),
        },
        Spec {
            id: "logs".into(),
            title: "logs do windows".into(),
            detail: "logs do windows update e do cbs. o windows cria de novo. pede administrador.".into(),
            needs_admin: true,
            checked: false,
            kind: Kind::Paths,
            paths: vec![
                PathBuf::from(r"C:\Windows\Logs\WindowsUpdate"),
                PathBuf::from(r"C:\Windows\Logs\CBS"),
            ],
        },
        Spec {
            id: "thumbs".into(),
            title: "miniaturas".into(),
            detail: "cache de miniaturas e ícones do explorador. o windows recria ao abrir as pastas.".into(),
            needs_admin: false,
            checked: true,
            kind: Kind::Paths,
            paths: thumb_files(&local),
        },
        Spec {
            id: "dumps".into(),
            title: "despejos de memória".into(),
            detail: "relatórios de falha do usuário e minidumps, quando der para ler.".into(),
            needs_admin: true,
            checked: true,
            kind: Kind::Paths,
            paths: dump_paths(&local),
        },
    ]
}

fn browser(id: &str, title: &str, root: PathBuf) -> Spec {
    Spec {
        id: id.into(),
        title: title.into(),
        detail: "só o cache. cookies, histórico e senhas ficam.".into(),
        needs_admin: false,
        checked: true,
        kind: Kind::Paths,
        paths: profile_caches(&root),
    }
}

fn profile_caches(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return out;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        for sub in ["Cache", "Code Cache", "GPUCache", r"Service Worker\CacheStorage", "ShaderCache", "GrShaderCache"] {
            let path = dir.join(sub);
            if path.exists() {
                out.push(path);
            }
        }
    }
    out
}

fn firefox_caches(local: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let profiles = local.join(r"Mozilla\Firefox\Profiles");
    let Ok(entries) = std::fs::read_dir(profiles) else {
        return out;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        for sub in ["cache2", "startupCache", "thumbnails"] {
            let path = dir.join(sub);
            if path.exists() {
                out.push(path);
            }
        }
    }
    out
}

fn opera_caches(roaming: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for root in ["Opera Software\\Opera Stable", "Opera Software\\Opera GX Stable"] {
        for sub in ["Cache", "GPUCache", "Code Cache"] {
            let path = roaming.join(root).join(sub);
            if path.exists() {
                out.push(path);
            }
        }
    }
    out
}

fn thumb_files(local: &Path) -> Vec<PathBuf> {
    let dir = local.join(r"Microsoft\Windows\Explorer");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
            name.starts_with("thumbcache") || name.starts_with("iconcache")
        })
        .collect()
}

fn dump_paths(local: &Path) -> Vec<PathBuf> {
    let mut paths = vec![
        local.join("CrashDumps"),
        local.join(r"Microsoft\Windows\WER\ReportArchive"),
        local.join(r"Microsoft\Windows\WER\ReportQueue"),
        PathBuf::from(r"C:\Windows\Minidump"),
    ];
    paths.retain(|path| path.exists());
    paths
}

fn recycle_size() -> (u64, u64, bool) {
    #[cfg(windows)]
    {
        let mut paths = Vec::new();
        for letter in b'A'..=b'Z' {
            let path = PathBuf::from(format!("{}:\\$Recycle.Bin", letter as char));
            if path.exists() {
                paths.push(path);
            }
        }
        let (bytes, files) = measure_paths(&paths);
        (bytes, files, true)
    }
    #[cfg(not(windows))]
    {
        (0, 0, true)
    }
}

fn hidden_recycle() -> crate::elevate::TaskResult {
    crate::elevate::elevate_sync(&Task::EmptyRecycle).unwrap_or(crate::elevate::TaskResult {
        ok: false,
        message: "não consegui esvaziar a lixeira".into(),
        output: String::new(),
    })
}
