//! Winget, Scoop e Chocolatey: detectar rápido, instalar sob demanda.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bob_core::{
    chocolatey_install_script, extract_matching, manager_from_probe, scoop_install_script, winget_installer_assets, ManagerInfo,
    APP_INSTALLER_STORE,
};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::AsyncWriteExt;

use crate::elevate::{elevate_sync, Task};
use crate::hidden;
use crate::jobs;
use crate::state::{write_json, AppState, BobEvent};

pub fn cached(app: &AppHandle) -> Vec<ManagerInfo> {
    app.state::<AppState>().lock().managers.clone()
}

pub async fn refresh(app: &AppHandle) {
    jobs::mark_updating(app, "managers", true);
    let found = detect_all().await;
    {
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        write_json(&shared.data_dir.join("managers.json"), &found);
        shared.managers = found.clone();
    }
    let _ = app.emit("bob", BobEvent::Managers { managers: found });
    jobs::mark_updating(app, "managers", false);
}

pub fn installed(app: &AppHandle, id: &str) -> bool {
    cached(app).iter().any(|item| item.id == id && item.installed)
}

#[tauri::command]
pub fn managers_list(app: AppHandle) -> Vec<ManagerInfo> {
    cached(&app)
}

#[tauri::command]
pub async fn managers_refresh(app: AppHandle) -> Result<Vec<ManagerInfo>, String> {
    refresh(&app).await;
    Ok(cached(&app))
}

#[tauri::command]
pub async fn manager_install(app: AppHandle, id: String) -> Result<(), String> {
    spawn_manager_job(app, id, false)
}

#[tauri::command]
pub async fn manager_update(app: AppHandle, id: String) -> Result<(), String> {
    spawn_manager_job(app, id, true)
}

fn spawn_manager_job(app: AppHandle, id: String, update: bool) -> Result<(), String> {
    if !matches!(id.as_str(), "winget" | "scoop" | "chocolatey") {
        return Err("gerenciador desconhecido".into());
    }
    let title = if update { format!("atualizar {id}") } else { format!("instalar {id}") };
    let job = jobs::begin(&app, &title, "settings", false);
    tokio::spawn(async move {
        let result = if update { update_one(&app, &id, &job).await } else { install_one(&app, &id, &job).await };
        match result {
            Ok(message) => {
                refresh(&app).await;
                jobs::finish(&app, &job, true, message);
            }
            Err(err) => {
                refresh(&app).await;
                jobs::finish(&app, &job, false, err);
            }
        }
    });
    Ok(())
}

async fn install_one(app: &AppHandle, id: &str, job: &str) -> Result<String, String> {
    match id {
        "winget" => install_winget(app, job).await,
        "scoop" => {
            jobs::tick(app, job, 0.2, "baixando o instalador oficial");
            let script = scoop_install_script();
            run_powershell(script).await?;
            Ok("scoop instalado".into())
        }
        "chocolatey" => {
            jobs::tick(app, job, 0.3, "esperando a permissão");
            let script = chocolatey_install_script().to_string();
            let result = tokio::task::spawn_blocking(move || elevate_sync(&Task::Powershell { script }))
                .await
                .map_err(|err| err.to_string())?;
            let result = result?;
            if result.ok {
                Ok("chocolatey instalado".into())
            } else {
                Err(if result.message.is_empty() { "o chocolatey não instalou".into() } else { result.message })
            }
        }
        _ => Err("gerenciador desconhecido".into()),
    }
}

async fn update_one(app: &AppHandle, id: &str, job: &str) -> Result<String, String> {
    match id {
        "winget" => install_winget(app, job).await,
        "scoop" => {
            jobs::tick(app, job, 0.4, "scoop update");
            let info = cached(app).into_iter().find(|item| item.id == "scoop");
            let program = info.and_then(|item| if item.path.is_empty() { None } else { Some(item.path) }).unwrap_or_else(|| "scoop".into());
            run_program(&program, &["update".into()]).await?;
            Ok("scoop atualizado".into())
        }
        "chocolatey" => {
            jobs::tick(app, job, 0.3, "esperando a permissão");
            let script = "choco upgrade chocolatey -y --no-progress".to_string();
            let result = tokio::task::spawn_blocking(move || elevate_sync(&Task::Powershell { script }))
                .await
                .map_err(|err| err.to_string())?;
            let result = result?;
            if result.ok { Ok("chocolatey atualizado".into()) } else { Err(result.message) }
        }
        _ => Err("gerenciador desconhecido".into()),
    }
}

async fn install_winget(app: &AppHandle, job: &str) -> Result<String, String> {
    jobs::tick(app, job, 0.05, "procurando o app installer");
    let release = match github_latest_winget().await {
        Ok(text) => text,
        Err(_) => {
            open_store()?;
            return Ok("não baixei o pacote. abri a microsoft store".into());
        }
    };
    let assets = winget_installer_assets(&release);
    if assets.is_empty() {
        open_store()?;
        return Ok("a release não trouxe o pacote. abri a microsoft store".into());
    }
    let dir = app.state::<AppState>().lock().data_dir.join("pkg").join("winget");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let total = assets.len().max(1);
    for (index, asset) in assets.iter().enumerate() {
        let fraction = 0.1 + 0.6 * (index as f32 / total as f32);
        jobs::tick(app, job, fraction, &format!("baixando {}", asset.name));
        let dest = dir.join(&asset.name);
        if download(&asset.url, &dest, app, job, fraction).await.is_err() {
            open_store()?;
            return Ok("o download falhou. abri a microsoft store".into());
        }
        if asset.name.to_ascii_lowercase().ends_with(".zip") {
            let _ = extract_matching(&dest, &dir, &[".msix", ".msixbundle"]);
        }
    }
    jobs::tick(app, job, 0.85, "instalando o app installer");
    let script = format!(
        "$ErrorActionPreference = 'Stop'; Get-ChildItem -LiteralPath {} -Filter *.msix | ForEach-Object {{ Add-AppxPackage -Path $_.FullName }}; Get-ChildItem -LiteralPath {} -Filter *.msixbundle | ForEach-Object {{ Add-AppxPackage -Path $_.FullName }}",
        crate::elevate::ps_quote(&dir.display().to_string()),
        crate::elevate::ps_quote(&dir.display().to_string()),
    );
    if run_powershell(&script).await.is_err() {
        open_store()?;
        return Ok("o app installer não entrou. abri a microsoft store".into());
    }
    Ok("winget instalado".into())
}

async fn github_latest_winget() -> Result<String, String> {
    let client = reqwest::Client::builder().user_agent("bob-tudo").timeout(Duration::from_secs(30)).build().map_err(|err| err.to_string())?;
    let response = client
        .get("https://api.github.com/repos/microsoft/winget-cli/releases/latest")
        .header("accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if !response.status().is_success() {
        return Err(format!("github {}", response.status()));
    }
    response.text().await.map_err(|err| err.to_string())
}

async fn download(url: &str, dest: &Path, app: &AppHandle, job: &str, base: f32) -> Result<(), String> {
    let client = reqwest::Client::builder().user_agent("bob-tudo").build().map_err(|err| err.to_string())?;
    let response = client.get(url).send().await.map_err(|err| err.to_string())?;
    if !response.status().is_success() {
        return Err(format!("download {}", response.status()));
    }
    let total = response.content_length().unwrap_or(0);
    let mut file = tokio::fs::File::create(dest).await.map_err(|err| err.to_string())?;
    let mut got = 0u64;
    let mut response = response;
    while let Some(chunk) = response.chunk().await.map_err(|err| err.to_string())? {
        got += chunk.len() as u64;
        file.write_all(&chunk).await.map_err(|err| err.to_string())?;
        if total > 0 {
            let fraction = base + 0.15 * (got as f32 / total as f32);
            jobs::tick(app, job, fraction.min(0.84), dest.file_name().and_then(|n| n.to_str()).unwrap_or("baixando"));
        }
    }
    file.flush().await.map_err(|err| err.to_string())
}

fn open_store() -> Result<(), String> {
    hidden::std_command("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &format!("Start-Process {}", crate::elevate::ps_quote(APP_INSTALLER_STORE))])
        .spawn()
        .map(|_| ())
        .map_err(|err| err.to_string())
}

async fn run_powershell(script: &str) -> Result<(), String> {
    run_program("powershell", &["-NoProfile".into(), "-ExecutionPolicy".into(), "Bypass".into(), "-Command".into(), script.into()]).await
}

async fn run_program(program: &str, args: &[String]) -> Result<(), String> {
    let output = hidden::tokio_command(program).args(args).output().await.map_err(|err| err.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let text = format!("{stderr} {stdout}").trim().to_string();
        Err(if text.is_empty() { format!("{program} falhou") } else { text })
    }
}

async fn detect_all() -> Vec<ManagerInfo> {
    let mut out = Vec::new();
    for id in ["winget", "scoop", "chocolatey"] {
        out.push(detect_one(id).await);
    }
    out
}

async fn detect_one(id: &str) -> ManagerInfo {
    let path = find_path(id).await.unwrap_or_default();
    if path.is_empty() {
        return manager_from_probe(id, "", "");
    }
    let output = version_output(id, &path).await.unwrap_or_default();
    manager_from_probe(id, &path, &output)
}

async fn find_path(id: &str) -> Option<String> {
    for candidate in candidates(id) {
        if candidate.is_file() {
            return Some(candidate.display().to_string());
        }
    }
    which(match id {
        "winget" => "winget",
        "scoop" => "scoop",
        "chocolatey" => "choco",
        _ => return None,
    })
    .await
}

fn candidates(id: &str) -> Vec<PathBuf> {
    let home = dirs::home_dir();
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    match id {
        "winget" => local
            .map(|dir| vec![dir.join("Microsoft").join("WindowsApps").join("winget.exe")])
            .unwrap_or_default(),
        "scoop" => home
            .map(|dir| vec![dir.join("scoop").join("shims").join("scoop.cmd"), dir.join("scoop").join("shims").join("scoop.ps1")])
            .unwrap_or_default(),
        "chocolatey" => vec![PathBuf::from(r"C:\ProgramData\chocolatey\bin\choco.exe")],
        _ => Vec::new(),
    }
}

async fn which(name: &str) -> Option<String> {
    let program = if cfg!(windows) { "where" } else { "which" };
    let output = tokio::time::timeout(Duration::from_secs(3), hidden::tokio_command(program).arg(name).output()).await.ok()?.ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().find(|line| !line.trim().is_empty())?.trim().to_string();
    if line.is_empty() { None } else { Some(line) }
}

async fn version_output(id: &str, path: &str) -> Result<String, String> {
    let args: Vec<String> = match id {
        "scoop" if path.to_ascii_lowercase().ends_with(".ps1") => {
            return powershell_version(path).await;
        }
        _ => vec!["--version".into()],
    };
    let output = tokio::time::timeout(Duration::from_secs(6), hidden::tokio_command(path).args(&args).output())
        .await
        .map_err(|_| "tempo esgotado".to_string())?
        .map_err(|err| err.to_string())?;
    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&output.stderr).to_string();
    }
    Ok(text)
}

async fn powershell_version(path: &str) -> Result<String, String> {
    let output = tokio::time::timeout(
        Duration::from_secs(8),
        hidden::tokio_command("powershell").args(["-NoProfile", "-File", path, "--version"]).output(),
    )
    .await
    .map_err(|_| "tempo esgotado".to_string())?
    .map_err(|err| err.to_string())?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}
