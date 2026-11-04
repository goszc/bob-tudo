use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bob_core::{oss_catalog, parse_winget_table, protected_path, OssApp, WingetRow};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::elevate::{elevate_sync, Task};
use crate::hidden;
use crate::state::BobEvent;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub id: String,
    pub name: String,
    pub version: String,
    pub publisher: String,
    pub location: String,
    pub uninstall: String,
    pub quiet: String,
    pub winget_id: String,
    pub source: String,
    pub size: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Leftover {
    pub path: String,
    pub kind: String,
    pub bytes: u64,
}

#[tauri::command]
pub async fn winget_search(query: String) -> Result<Vec<WingetRow>, String> {
    let query = query.trim();
    if query.is_empty() {
        return Err("digite um nome".into());
    }
    let text = winget_text(&["search", "--query", query, "--accept-source-agreements", "--disable-interactivity"]).await?;
    let rows = parse_winget_table(&text);
    if rows.is_empty() && !text.to_lowercase().contains("name") && !text.to_lowercase().contains("nome") {
        return Err(text.lines().next().unwrap_or("o winget não devolveu resultados").into());
    }
    Ok(rows)
}

#[tauri::command]
pub async fn winget_install(app: AppHandle, ids: Vec<String>) -> Result<(), String> {
    if ids.is_empty() {
        return Err("nada selecionado".into());
    }
    tokio::spawn(async move {
        for id in ids {
            let _ = app.emit(
                "bob",
                BobEvent::JobLog {
                    line: format!("instalando {id}"),
                },
            );
            let args = vec![
                "install".into(),
                "--id".into(),
                id.clone(),
                "--exact".into(),
                "--accept-package-agreements".into(),
                "--accept-source-agreements".into(),
                "--disable-interactivity".into(),
                "--silent".into(),
            ];
            if let Err(err) = stream(&app, "winget", &args).await {
                let _ = app.emit("bob", BobEvent::JobLog { line: format!("{id}: {err}") });
            } else {
                let _ = app.emit("bob", BobEvent::JobLog { line: format!("{id} instalado") });
            }
        }
        crate::system::notify(&app, "instalação concluída");
        let _ = app.emit(
            "bob",
            BobEvent::Toast {
                message: "instalação concluída".into(),
            },
        );
    });
    Ok(())
}

#[tauri::command]
pub async fn winget_upgrades() -> Result<Vec<WingetRow>, String> {
    let text = winget_text(&[
        "upgrade",
        "--include-unknown",
        "--accept-source-agreements",
        "--disable-interactivity",
    ])
    .await?;
    Ok(parse_winget_table(&text))
}

#[tauri::command]
pub async fn winget_upgrade(app: AppHandle, ids: Vec<String>, all: bool) -> Result<(), String> {
    let args = if all {
        vec![
            "upgrade".into(),
            "--all".into(),
            "--include-unknown".into(),
            "--accept-package-agreements".into(),
            "--accept-source-agreements".into(),
            "--disable-interactivity".into(),
            "--silent".into(),
        ]
    } else if ids.len() == 1 {
        vec![
            "upgrade".into(),
            "--id".into(),
            ids[0].clone(),
            "--exact".into(),
            "--accept-package-agreements".into(),
            "--accept-source-agreements".into(),
            "--disable-interactivity".into(),
            "--silent".into(),
        ]
    } else if ids.is_empty() {
        return Err("nada selecionado".into());
    } else {
        tokio::spawn(async move {
            for id in ids {
                let args = vec![
                    "upgrade".into(),
                    "--id".into(),
                    id.clone(),
                    "--exact".into(),
                    "--accept-package-agreements".into(),
                    "--accept-source-agreements".into(),
                    "--disable-interactivity".into(),
                    "--silent".into(),
                ];
                let _ = stream(&app, "winget", &args).await;
            }
            crate::system::notify(&app, "atualizações concluídas");
        });
        return Ok(());
    };
    let app2 = app.clone();
    tokio::spawn(async move {
        let _ = stream(&app2, "winget", &args).await;
        crate::system::notify(&app2, "atualizações concluídas");
        let _ = app2.emit(
            "bob",
            BobEvent::Toast {
                message: "atualizações concluídas".into(),
            },
        );
    });
    Ok(())
}

#[tauri::command]
pub async fn oss_catalog_cmd() -> Result<Vec<OssApp>, String> {
    Ok(with_stars(oss_catalog()).await)
}

#[tauri::command]
pub async fn oss_search(query: String) -> Result<Vec<OssApp>, String> {
    let query = query.trim();
    if query.is_empty() {
        return oss_catalog_cmd().await;
    }
    let q = format!("{query} windows");
    let items = github_search(&q).await?;
    let catalog = oss_catalog();
    let mut out = Vec::new();
    for item in items {
        let repo = item.get("full_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let known = catalog.iter().find(|app| app.repo.eq_ignore_ascii_case(&repo));
        let description = known
            .map(|app| app.description.clone())
            .unwrap_or_else(|| item.get("description").and_then(|v| v.as_str()).unwrap_or("").to_lowercase());
        out.push(OssApp {
            name: item.get("name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase(),
            description,
            category: known.map(|app| app.category.clone()).unwrap_or_else(|| "código aberto".into()),
            repo,
            winget_id: known.map(|app| app.winget_id.clone()).unwrap_or_default(),
            alternative_to: known.map(|app| app.alternative_to.clone()).unwrap_or_default(),
            stars: item.get("stargazers_count").and_then(|v| v.as_u64()).unwrap_or(0),
        });
    }
    if out.is_empty() {
        let needle = query.to_lowercase();
        out = catalog
            .into_iter()
            .filter(|app| {
                app.name.contains(&needle)
                    || app.description.contains(&needle)
                    || app.category.contains(&needle)
                    || app.alternative_to.contains(&needle)
            })
            .collect();
    }
    Ok(out)
}

#[tauri::command]
pub async fn apps_list() -> Result<Vec<InstalledApp>, String> {
    let mut apps = uninstall_keys();
    if let Ok(text) = winget_text(&["list", "--accept-source-agreements", "--disable-interactivity"]).await {
        for row in parse_winget_table(&text) {
            if let Some(app) = apps.iter_mut().find(|app| app.name.eq_ignore_ascii_case(&row.name) || (!row.id.is_empty() && app.winget_id == row.id)) {
                if app.winget_id.is_empty() {
                    app.winget_id = row.id;
                }
                if app.source.is_empty() {
                    app.source = "winget".into();
                }
            } else if !row.name.is_empty() {
                apps.push(InstalledApp {
                    id: format!("winget:{}", row.id),
                    name: row.name,
                    version: row.version,
                    publisher: String::new(),
                    location: String::new(),
                    uninstall: String::new(),
                    quiet: String::new(),
                    winget_id: row.id,
                    source: "winget".into(),
                    size: 0,
                });
            }
        }
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(apps)
}

#[tauri::command]
pub async fn apps_uninstall(app: AppHandle, id: String) -> Result<(), String> {
    let apps = apps_list().await?;
    let app_info = apps.into_iter().find(|item| item.id == id).ok_or("aplicativo não encontrado")?;
    if !app_info.winget_id.is_empty() {
        let args = vec![
            "uninstall".into(),
            "--id".into(),
            app_info.winget_id.clone(),
            "--exact".into(),
            "--accept-source-agreements".into(),
            "--disable-interactivity".into(),
            "--silent".into(),
        ];
        stream(&app, "winget", &args).await?;
        return Ok(());
    }
    let command = if !app_info.quiet.is_empty() {
        app_info.quiet.clone()
    } else {
        app_info.uninstall.clone()
    };
    if command.is_empty() {
        return Err("sem comando de desinstalação".into());
    }
    let (program, args) = split_command(&command);
    let mut args = args;
    let lower = command.to_lowercase();
    if lower.contains("msiexec") && !lower.contains("/qn") {
        args.push("/qn".into());
        args.push("/norestart".into());
    }
    let arg_refs: Vec<String> = args;
    stream(&app, &program, &arg_refs).await?;
    Ok(())
}

#[tauri::command]
pub fn apps_leftovers(name: String, location: String, registry_id: String) -> Vec<Leftover> {
    suggest_leftovers(&name, &location, &registry_id)
}

#[tauri::command]
pub fn apps_remove_leftovers(paths: Vec<String>) -> Result<bob_core::PurgeReport, String> {
    let mut files = Vec::new();
    let mut regs = Vec::new();
    for path in paths {
        if let Some(reg) = path.strip_prefix("registro:") {
            if !reg.to_lowercase().contains("\\uninstall\\") || reg.contains("..") {
                return Err(format!("registro recusado: {reg}"));
            }
            regs.push(reg.to_string());
        } else {
            if protected_path(Path::new(&path)) {
                return Err(format!("caminho protegido: {path}"));
            }
            files.push(PathBuf::from(path));
        }
    }
    let report = bob_core::purge_paths(&files);
    for reg in regs {
        let root = if reg.to_lowercase().starts_with("hklm") { true } else { false };
        let result = if root {
            elevate_sync(&Task::Powershell {
                script: format!("reg delete {} /f", crate::elevate::ps_quote(&reg)),
            })
        } else {
            let output = hidden::std_command("reg").args(["delete", &reg, "/f"]).output();
            match output {
                Ok(output) if output.status.success() => Ok(crate::elevate::TaskResult {
                    ok: true,
                    message: String::new(),
                    output: String::new(),
                }),
                Ok(output) => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
                Err(err) => Err(err.to_string()),
            }
        };
        match result {
            Ok(task) if task.ok => {}
            Ok(task) => return Err(task.message),
            Err(err) => return Err(err),
        }
    }
    Ok(report)
}

fn suggest_leftovers(name: &str, location: &str, registry_id: &str) -> Vec<Leftover> {
    let mut out = Vec::new();
    if !location.is_empty() {
        let path = PathBuf::from(location);
        if path.exists() && !protected_path(&path) {
            let (bytes, _) = bob_core::measure_paths(&[path.clone()]);
            out.push(Leftover {
                path: path.display().to_string(),
                kind: "pasta".into(),
                bytes,
            });
        }
    }
    let token = normalize_name(name);
    if token.len() >= 3 {
        for root in leftover_roots() {
            let direct = root.join(&token);
            push_dir(&mut out, &direct);
            let Ok(entries) = std::fs::read_dir(&root) else { continue };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let folder = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                if folder == token || (token.len() >= 5 && folder.contains(&token)) {
                    push_dir(&mut out, &path);
                }
            }
        }
    }
    if registry_id.to_lowercase().contains("\\uninstall\\") {
        out.push(Leftover {
            path: format!("registro:{registry_id}"),
            kind: "registro".into(),
            bytes: 0,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out.dedup_by(|a, b| a.path == b.path);
    out
}

fn push_dir(out: &mut Vec<Leftover>, path: &Path) {
    if path.exists() && !protected_path(path) && path.components().count() >= 3 {
        let (bytes, _) = bob_core::measure_paths(&[path]);
        out.push(Leftover {
            path: path.display().to_string(),
            kind: "pasta".into(),
            bytes,
        });
    }
}

fn leftover_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(local) = dirs::data_local_dir() {
        roots.push(local);
    }
    if let Some(roaming) = dirs::config_dir() {
        roots.push(roaming);
    }
    roots.push(PathBuf::from(r"C:\Program Files"));
    roots.push(PathBuf::from(r"C:\Program Files (x86)"));
    roots.push(PathBuf::from(r"C:\ProgramData"));
    roots
}

fn normalize_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut out = String::new();
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() || ch == ' ' || ch == '-' {
            out.push(ch);
        }
    }
    for drop in [" x64", " x86", " (64-bit)", " (32-bit)", " desktop", " version"] {
        out = out.replace(drop, "");
    }
    out.split_whitespace().next().unwrap_or("").trim().to_string()
}

fn uninstall_keys() -> Vec<InstalledApp> {
    #[cfg(not(windows))]
    {
        return Vec::new();
    }
    #[cfg(windows)]
    {
        use winreg::enums::*;
        use winreg::RegKey;
        let mut apps = Vec::new();
        let views = [
            (RegKey::predef(HKEY_LOCAL_MACHINE), r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall", "HKLM"),
            (
                RegKey::predef(HKEY_LOCAL_MACHINE),
                r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
                "HKLM",
            ),
            (RegKey::predef(HKEY_CURRENT_USER), r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall", "HKCU"),
        ];
        for (root, path, hive) in views {
            let Ok(key) = root.open_subkey_with_flags(path, KEY_READ | KEY_WOW64_64KEY) else {
                continue;
            };
            for name in key.enum_keys().flatten() {
                let Ok(sub) = key.open_subkey(&name) else { continue };
                let display: String = sub.get_value("DisplayName").unwrap_or_default();
                if display.trim().is_empty() {
                    continue;
                }
                let system: u32 = sub.get_value("SystemComponent").unwrap_or(0);
                let parent: String = sub.get_value("ParentKeyName").unwrap_or_default();
                if system == 1 || !parent.is_empty() {
                    continue;
                }
                let size_kb: u32 = sub.get_value("EstimatedSize").unwrap_or(0);
                apps.push(InstalledApp {
                    id: format!("{hive}\\{path}\\{name}"),
                    name: display,
                    version: sub.get_value("DisplayVersion").unwrap_or_default(),
                    publisher: sub.get_value("Publisher").unwrap_or_default(),
                    location: sub.get_value("InstallLocation").unwrap_or_default(),
                    uninstall: sub.get_value("UninstallString").unwrap_or_default(),
                    quiet: sub.get_value("QuietUninstallString").unwrap_or_default(),
                    winget_id: String::new(),
                    source: "registro".into(),
                    size: (size_kb as u64).saturating_mul(1024),
                });
            }
        }
        apps
    }
}

async fn winget_text(args: &[&str]) -> Result<String, String> {
    let output = hidden::tokio_command("winget")
        .args(args)
        .output()
        .await
        .map_err(|_| "o winget não está instalado. no windows 11 ele vem com o sistema.".to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() && stdout.trim().is_empty() {
        return Err(if stderr.trim().is_empty() {
            "o winget falhou".into()
        } else {
            stderr
        });
    }
    Ok(stdout)
}

async fn stream(app: &AppHandle, program: &str, args: &[String]) -> Result<(), String> {
    let mut child = hidden::tokio_command(program)
        .args(args)
        .spawn()
        .map_err(|err| err.to_string())?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let app_out = app.clone();
    let app_err = app.clone();
    let out = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim().to_string();
                if !line.is_empty() {
                    let _ = app_out.emit("bob", BobEvent::JobLog { line });
                }
            }
        }
    });
    let err = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim().to_string();
                if !line.is_empty() {
                    let _ = app_err.emit("bob", BobEvent::JobLog { line });
                }
            }
        }
    });
    let status = child.wait().await.map_err(|err| err.to_string())?;
    let _ = out.await;
    let _ = err.await;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} saiu com erro"))
    }
}

async fn with_stars(mut apps: Vec<OssApp>) -> Vec<OssApp> {
    let repos: Vec<String> = apps.iter().filter(|app| !app.repo.is_empty()).map(|app| app.repo.clone()).collect();
    if let Ok(map) = fetch_stars(&repos).await {
        for app in &mut apps {
            if let Some(stars) = map.get(&app.repo.to_lowercase()) {
                app.stars = *stars;
            }
        }
    }
    apps
}

async fn fetch_stars(repos: &[String]) -> Result<HashMap<String, u64>, String> {
    let mut map = HashMap::new();
    for chunk in repos.chunks(6) {
        let q = chunk
            .iter()
            .map(|repo| format!("repo:{repo}"))
            .collect::<Vec<_>>()
            .join(" OR ");
        for item in github_search(&q).await.unwrap_or_default() {
            let repo = item.get("full_name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let stars = item.get("stargazers_count").and_then(|v| v.as_u64()).unwrap_or(0);
            if !repo.is_empty() {
                map.insert(repo, stars);
            }
        }
    }
    Ok(map)
}

async fn github_search(query: &str) -> Result<Vec<Value>, String> {
    let client = reqwest::Client::builder()
        .user_agent("bob-tudo")
        .timeout(std::time::Duration::from_secs(12))
        .build()
        .map_err(|err| err.to_string())?;
    let url = format!(
        "https://api.github.com/search/repositories?q={}&sort=stars&order=desc&per_page=20",
        encode(query)
    );
    let response = client.get(url).send().await.map_err(|err| err.to_string())?;
    if !response.status().is_success() {
        return Err(format!("github {}", response.status()));
    }
    let value: Value = response.json().await.map_err(|err| err.to_string())?;
    Ok(value.get("items").and_then(|v| v.as_array()).cloned().unwrap_or_default())
}

fn encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            b' ' => out.push_str("%20"),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn split_command(line: &str) -> (String, Vec<String>) {
    let mut chars = line.trim().chars().peekable();
    let mut program = String::new();
    if chars.peek() == Some(&'"') {
        chars.next();
        for ch in chars.by_ref() {
            if ch == '"' {
                break;
            }
            program.push(ch);
        }
    } else {
        for ch in chars.by_ref() {
            if ch == ' ' {
                break;
            }
            program.push(ch);
        }
    }
    let rest: String = chars.collect();
    let args = rest.split_whitespace().map(|part| part.trim_matches('"').to_string()).filter(|part| !part.is_empty()).collect();
    (program, args)
}
