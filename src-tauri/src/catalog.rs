//! Catálogo unificado. A janela só pergunta; o índice e os downloads ficam aqui.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bob_core::{
    parse_choco_atom, parse_device_poll, parse_device_start, parse_github_releases, parse_github_repo, read_winget_index,
    render_readme, scoop_from_zip, watch_is_newer, CatalogDetail, CatalogHit, DevicePoll, SearchPage, SearchQuery,
};
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::apps::stream;
use crate::elevate::{elevate_sync, ps_quote, Task};
use crate::hidden;
use crate::jobs;
use crate::secrets;
use crate::state::{AppState, BobEvent};

struct PendingLogin {
    device_code: String,
    interval: u64,
    client_id: String,
}

static LOGIN: Mutex<Option<PendingLogin>> = Mutex::new(None);

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallReq {
    source: String,
    source_id: String,
    version: String,
    #[serde(default)]
    asset_url: String,
    #[serde(default)]
    asset_name: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginBegin {
    user_code: String,
    verification_uri: String,
    interval: u64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginState {
    connected: bool,
    login: String,
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        refresh(&app).await;
    });
}

pub async fn refresh(app: &AppHandle) {
    if app.state::<AppState>().lock().updating.iter().any(|area| area == "catalog") {
        return;
    }
    let job = jobs::begin(app, "catálogo", "catalog", true);
    jobs::mark_updating(app, "catalog", true);
    let result = refresh_inner(app, &job).await;
    jobs::mark_updating(app, "catalog", false);
    match result {
        Ok(count) => jobs::finish(app, &job, true, format!("{count} apps")),
        Err(err) => jobs::finish(app, &job, false, err),
    }
}

fn with_index<T>(app: &AppHandle, f: impl FnOnce(&bob_core::CatalogIndex) -> T) -> T {
    let state = app.state::<AppState>();
    let index = state.catalog.lock().unwrap_or_else(|err| err.into_inner());
    f(&index)
}

async fn refresh_inner(app: &AppHandle, job: &str) -> Result<usize, String> {
    jobs::tick(app, job, 0.02, "curadoria");
    {
        let state = app.state::<AppState>();
        let index = state.catalog.lock().unwrap_or_else(|err| err.into_inner());
        index.replace_source("curadoria", &bob_core::curated_incoming())?;
        index.rebuild()?;
    }
    jobs::tick(app, job, 0.15, "winget");
    if let Err(err) = refresh_winget(app).await {
        jobs::tick(app, job, 0.3, &err);
    }
    jobs::tick(app, job, 0.45, "scoop");
    if let Err(err) = refresh_scoop(app).await {
        jobs::tick(app, job, 0.55, &err);
    }
    jobs::tick(app, job, 0.6, "chocolatey");
    if let Err(err) = refresh_choco(app).await {
        jobs::tick(app, job, 0.7, &err);
    }
    {
        let state = app.state::<AppState>();
        let index = state.catalog.lock().unwrap_or_else(|err| err.into_inner());
        index.rebuild()?;
    }
    jobs::tick(app, job, 0.82, "github");
    let _ = refresh_github_meta(app).await;
    let _ = sync_installed(app).await;
    let _ = notify_watches(app).await;
    let count = with_index(app, |index| index.app_count().unwrap_or(0) as usize);
    Ok(count)
}

async fn refresh_winget(app: &AppHandle) -> Result<(), String> {
    let url = "https://cdn.winget.microsoft.com/cache/source2.msix";
    let dir = app.state::<AppState>().lock().data_dir.join("catalog");
    std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    let dest = dir.join("source2.msix");
    let etag = with_index(app, |index| index.meta_get("winget_etag").unwrap_or_default());
    let (body_path, new_etag, changed) = download_conditional(url, &dest, &etag).await?;
    if !changed && body_path.exists() {
        return Ok(());
    }
    let db = bob_core::find_index_db(&body_path, &dir.join("winget-index"))?;
    let rows = tokio::task::spawn_blocking(move || read_winget_index(&db)).await.map_err(|err| err.to_string())??;
    let state = app.state::<AppState>();
    let index = state.catalog.lock().unwrap_or_else(|err| err.into_inner());
    index.replace_source("winget", &rows)?;
    index.meta_set("winget_etag", &new_etag)?;
    Ok(())
}

async fn refresh_scoop(app: &AppHandle) -> Result<(), String> {
    let urls = [
        "https://github.com/ScoopInstaller/Main/archive/refs/heads/master.zip",
        "https://github.com/ScoopInstaller/Extras/archive/refs/heads/master.zip",
    ];
    let mut rows = Vec::new();
    for url in urls {
        let name = if url.contains("Extras") { "scoop-extras.zip" } else { "scoop-main.zip" };
        let dest = app.state::<AppState>().lock().data_dir.join("catalog").join(name);
        let key = format!("etag:{url}");
        let etag = with_index(app, |index| index.meta_get(&key).unwrap_or_default());
        let (path, new_etag, changed) = download_conditional(url, &dest, &etag).await?;
        if !changed && rows.is_empty() && path.exists() {
            // still parse: the other zip may be new, and a cache hit should be indexed once
        }
        let bytes = tokio::fs::read(&path).await.map_err(|err| err.to_string())?;
        rows.extend(scoop_from_zip(&bytes)?);
        with_index(app, |index| index.meta_set(&key, &new_etag))?;
        let _ = changed;
    }
    with_index(app, |index| index.replace_source("scoop", &rows))?;
    Ok(())
}

async fn refresh_choco(app: &AppHandle) -> Result<(), String> {
    let done = with_index(app, |index| index.meta_get("choco_done").unwrap_or_default());
    let first_url = choco_page(0);
    let cached = with_index(app, |index| index.http_lookup(&first_url))?;
    let body = cached_text(app, &first_url).await?;
    if done == "1" {
        if let Some((_, previous)) = cached {
            if previous == body {
                return Ok(());
            }
        }
    }
    let mut skip = if done == "1" { 0 } else { with_index(app, |index| index.meta_get("choco_skip").unwrap_or_default().parse().unwrap_or(0)) };
    let mut all = Vec::new();
    if skip == 0 {
        all.extend(parse_choco_atom(&body));
        skip = 100;
    }
    for _ in 0..25 {
        let page = cached_text(app, &choco_page(skip)).await?;
        let rows = parse_choco_atom(&page);
        if rows.is_empty() {
            with_index(app, |index| index.meta_set("choco_done", "1"))?;
            break;
        }
        all.extend(rows);
        skip += 100;
        with_index(app, |index| index.meta_set("choco_skip", &skip.to_string()))?;
    }
    if !all.is_empty() {
        if skip == 100 || with_index(app, |index| index.meta_get("choco_done").unwrap_or_default() == "1" ) {
            with_index(app, |index| index.replace_source("chocolatey", &all))?;
        } else {
            let mut existing = with_index(app, |index| index.load_incoming())?;
            existing.retain(|item| item.source != "chocolatey");
            existing.extend(all);
            let choco: Vec<_> = existing.into_iter().filter(|item| item.source == "chocolatey").collect();
            with_index(app, |index| index.replace_source("chocolatey", &choco))?;
        }
    }
    Ok(())
}

fn choco_page(skip: u32) -> String {
    format!("https://community.chocolatey.org/api/v2/Packages()?$filter=IsLatestVersion%20eq%20true&$orderby=Id&$top=100&$skip={skip}")
}

async fn refresh_github_meta(app: &AppHandle) -> Result<(), String> {
    let repos = with_index(app, |index| index.repos_missing_stars(8))?;
    for (_id, repo) in repos {
        let url = format!("https://api.github.com/repos/{repo}");
        let body = match cached_text(app, &url).await {
            Ok(body) => body,
            Err(err) if err.contains("403") || err.contains("429") => break,
            Err(_) => continue,
        };
        if let Some((stars, description, pushed, license)) = parse_github_repo(&body) {
            let _ = with_index(app, |index| index.update_repo(&repo, stars, &description, pushed, &license));
        }
    }
    Ok(())
}

async fn sync_installed(app: &AppHandle) -> Result<(), String> {
    let apps = crate::apps::apps_list().await.unwrap_or_default();
    let ids: Vec<String> = apps.iter().map(|item| item.winget_id.clone()).filter(|id| !id.is_empty()).collect();
    let names: Vec<String> = apps.iter().map(|item| item.name.clone()).collect();
    with_index(app, |index| index.set_installed(&ids, &names))
}

async fn notify_watches(app: &AppHandle) -> Result<(), String> {
    let watches = with_index(app, |index| index.watches())?;
    for (id, seen, latest, name) in watches {
        if !watch_is_newer(&latest, &seen) {
            let repo = with_index(app, |index| index.hit(&id).ok().flatten().map(|hit| hit.repo).unwrap_or_default());
            if repo.contains('/') {
                if let Ok(body) = cached_text(app, &format!("https://api.github.com/repos/{repo}/releases?per_page=1")).await {
                    let releases = parse_github_releases(&body);
                    if let Some(release) = releases.first() {
                        if watch_is_newer(&release.version, &seen) {
                            crate::system::notify_kind(app, "watch", &format!("{name} tem a versão {}", release.version));
                            let _ = with_index(app, |index| index.set_watch(&id, &release.version, true));
                        }
                    }
                }
            }
            continue;
        }
        crate::system::notify_kind(app, "watch", &format!("{name} tem a versão {latest}"));
        let _ = with_index(app, |index| index.set_watch(&id, &latest, true));
    }
    Ok(())
}

async fn cached_text(app: &AppHandle, url: &str) -> Result<String, String> {
    let cached = with_index(app, |index| index.http_lookup(url))?;
    let client = reqwest::Client::builder()
        .user_agent("bob-tudo")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .build()
        .map_err(|err| err.to_string())?;
    let mut request = client.get(url).header("accept", "application/vnd.github+json, application/xml, */*");
    if url.contains("api.github.com") {
        if let Some(token) = secrets::read_token(&app.state::<AppState>().lock().data_dir) {
            request = request.bearer_auth(token);
        }
    }
    if let Some((etag, _)) = &cached {
        if !etag.is_empty() {
            request = request.header("if-none-match", etag);
        }
    }
    let response = request.send().await.map_err(|err| err.to_string())?;
    let status = response.status();
    if status.as_u16() == 304 {
        return cached.map(|(_, body)| body).ok_or_else(|| "cache vazia".into());
    }
    if status.as_u16() == 403 || status.as_u16() == 429 {
        return Err(format!("{status}"));
    }
    if !status.is_success() {
        if let Some((_, body)) = cached {
            return Ok(body);
        }
        return Err(format!("http {status}"));
    }
    let etag = response.headers().get("etag").and_then(|value| value.to_str().ok()).unwrap_or("").to_string();
    let body = response.text().await.map_err(|err| err.to_string())?;
    let _ = with_index(app, |index| index.http_store(url, &etag, &body, now_ms()));
    Ok(body)
}

async fn download_conditional(url: &str, dest: &PathBuf, etag: &str) -> Result<(PathBuf, String, bool), String> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let client = reqwest::Client::builder().user_agent("bob-tudo").build().map_err(|err| err.to_string())?;
    let mut request = client.get(url);
    if !etag.is_empty() && dest.exists() {
        request = request.header("if-none-match", etag);
    }
    let response = request.send().await.map_err(|err| err.to_string())?;
    if response.status().as_u16() == 304 && dest.exists() {
        return Ok((dest.clone(), etag.to_string(), false));
    }
    if !response.status().is_success() {
        if dest.exists() {
            return Ok((dest.clone(), etag.to_string(), false));
        }
        return Err(format!("download {}", response.status()));
    }
    let new_etag = response.headers().get("etag").and_then(|value| value.to_str().ok()).unwrap_or("").to_string();
    let bytes = response.bytes().await.map_err(|err| err.to_string())?;
    tokio::fs::write(dest, &bytes).await.map_err(|err| err.to_string())?;
    Ok((dest.clone(), new_etag, true))
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

#[tauri::command]
pub fn catalog_search(app: AppHandle, query: SearchQuery) -> Result<SearchPage, String> {
    let mut query = query;
    if query.now_ms == 0 {
        query.now_ms = now_ms();
    }
    with_index(&app, |index| index.search(&query))
}

#[tauri::command]
pub async fn catalog_detail(app: AppHandle, id: String) -> Result<CatalogDetail, String> {
    let current = with_index(&app, |index| index.detail(&id)?.ok_or_else(|| "app não encontrado".to_string()))?;
    if current.hit.repo.contains('/') {
        if let Ok(detail) = load_remote(&app, &current.hit).await {
            return Ok(detail);
        }
    }
    Ok(current)
}

async fn load_remote(app: &AppHandle, hit: &CatalogHit) -> Result<CatalogDetail, String> {
    let repo = &hit.repo;
    let readme = cached_text(app, &format!("https://raw.githubusercontent.com/{repo}/HEAD/README.md")).await.unwrap_or_default();
    let releases_body = cached_text(app, &format!("https://api.github.com/repos/{repo}/releases?per_page=8")).await.unwrap_or_else(|_| "[]".into());
    let releases = parse_github_releases(&releases_body);
    let (html, images) = render_readme(&readme, &format!("https://raw.githubusercontent.com/{repo}/HEAD/"));
    with_index(app, |index| index.save_detail(&hit.id, &html, &images, &releases))?;
    if hit.stars == 0 {
        if let Ok(body) = cached_text(app, &format!("https://api.github.com/repos/{repo}")).await {
            if let Some((stars, description, pushed, license)) = parse_github_repo(&body) {
                let _ = with_index(app, |index| index.update_repo(repo, stars, &description, pushed, &license));
            }
        }
    }
    with_index(app, |index| index.detail(&hit.id)?.ok_or_else(|| "app não encontrado".to_string()))
}

#[tauri::command]
pub fn catalog_favorite(app: AppHandle, id: String, on: bool) -> Result<(), String> {
    with_index(&app, |index| index.set_favorite(&id, on))
}

#[tauri::command]
pub fn catalog_watch(app: AppHandle, id: String, version: String, on: bool) -> Result<(), String> {
    with_index(&app, |index| index.set_watch(&id, &version, on))
}

#[tauri::command]
pub fn catalog_collections(app: AppHandle) -> Result<Vec<bob_core::Collection>, String> {
    with_index(&app, |index| index.collections())
}

#[tauri::command]
pub fn catalog_create_collection(app: AppHandle, name: String) -> Result<bob_core::Collection, String> {
    with_index(&app, |index| index.create_collection(&name))
}

#[tauri::command]
pub fn catalog_collection_app(app: AppHandle, collection_id: String, app_id: String, on: bool) -> Result<(), String> {
    with_index(&app, |index| index.set_collection_app(&collection_id, &app_id, on))
}

#[tauri::command]
pub async fn catalog_install(app: AppHandle, requests: Vec<InstallReq>) -> Result<(), String> {
    if requests.is_empty() {
        return Err("nada selecionado".into());
    }
    let job = jobs::begin(&app, "instalação", "catalog", false);
    tokio::spawn(async move {
        let total = requests.len().max(1) as f32;
        let mut failed = 0;
        for (index, request) in requests.iter().enumerate() {
            jobs::tick(&app, &job, index as f32 / total, &request.source_id);
            if let Err(err) = install_one(&app, request).await {
                failed += 1;
                let _ = app.emit("bob", BobEvent::JobLog { line: format!("{}: {err}", request.source_id) });
            }
        }
        if failed == 0 {
            jobs::finish(&app, &job, true, "concluída");
        } else {
            jobs::finish(&app, &job, false, format!("{failed} falharam"));
        }
        let _ = sync_installed(&app).await;
    });
    Ok(())
}

async fn install_one(app: &AppHandle, request: &InstallReq) -> Result<(), String> {
    match request.source.as_str() {
        "winget" => {
            if !crate::managers::installed(app, "winget") {
                return Err("winget não está instalado".into());
            }
            let mut args = vec![
                "install".into(),
                "--id".into(),
                request.source_id.clone(),
                "--exact".into(),
                "--accept-package-agreements".into(),
                "--accept-source-agreements".into(),
                "--disable-interactivity".into(),
                "--silent".into(),
            ];
            if !request.version.is_empty() {
                args.push("--version".into());
                args.push(request.version.clone());
            }
            stream(app, "winget", &args).await
        }
        "scoop" => {
            if !crate::managers::installed(app, "scoop") {
                return Err("scoop não está instalado".into());
            }
            let spec = if request.version.is_empty() {
                request.source_id.clone()
            } else {
                format!("{}@{}", request.source_id, request.version)
            };
            let program = crate::managers::cached(app)
                .into_iter()
                .find(|item| item.id == "scoop")
                .map(|item| item.path)
                .filter(|path| !path.is_empty())
                .unwrap_or_else(|| "scoop".into());
            stream(app, &program, &[String::from("install"), spec]).await
        }
        "chocolatey" => {
            if !crate::managers::installed(app, "chocolatey") {
                return Err("chocolatey não está instalado".into());
            }
            let mut script = format!("choco install {} -y --no-progress", ps_quote(&request.source_id));
            if !request.version.is_empty() {
                script.push_str(&format!(" --version {}", ps_quote(&request.version)));
            }
            let result = tokio::task::spawn_blocking(move || elevate_sync(&Task::Powershell { script }))
                .await
                .map_err(|err| err.to_string())?;
            let result = result?;
            if result.ok { Ok(()) } else { Err(result.message) }
        }
        "github" => download_asset(app, &request.asset_url, &request.asset_name).await,
        _ => Err("fonte desconhecida".into()),
    }
}

async fn download_asset(_app: &AppHandle, url: &str, name: &str) -> Result<(), String> {
    if url.is_empty() {
        return Err("sem arquivo nesta versão".into());
    }
    let downloads = dirs::download_dir().unwrap_or_else(std::env::temp_dir);
    let _ = std::fs::create_dir_all(&downloads);
    let file_name = if name.is_empty() { "bob-tudo-download".into() } else { name.to_string() };
    let dest = downloads.join(&file_name);
    let client = reqwest::Client::builder().user_agent("bob-tudo").build().map_err(|err| err.to_string())?;
    let mut response = client.get(url).send().await.map_err(|err| err.to_string())?;
    if !response.status().is_success() {
        return Err(format!("download {}", response.status()));
    }
    let mut file = tokio::fs::File::create(&dest).await.map_err(|err| err.to_string())?;
    while let Some(chunk) = response.chunk().await.map_err(|err| err.to_string())? {
        use tokio::io::AsyncWriteExt;
        file.write_all(&chunk).await.map_err(|err| err.to_string())?;
    }
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".exe") {
        let _ = hidden::gui_command(&dest).spawn();
    } else if lower.ends_with(".msi") {
        let _ = hidden::gui_command("msiexec").arg("/i").arg(&dest).spawn();
    }
    Ok(())
}

#[tauri::command]
pub async fn github_login_begin(app: AppHandle) -> Result<LoginBegin, String> {
    let client_id = app.state::<AppState>().lock().config.github_client_id.trim().to_string();
    if client_id.is_empty() {
        return Err("informe o client id do aplicativo oauth em ajustes".into());
    }
    let client = reqwest::Client::builder().user_agent("bob-tudo").build().map_err(|err| err.to_string())?;
    let response = client
        .post("https://github.com/login/device/code")
        .header("accept", "application/json")
        .form(&[("client_id", client_id.as_str()), ("scope", "")])
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let body = response.text().await.map_err(|err| err.to_string())?;
    let start = parse_device_start(&body)?;
    if start.device_code.is_empty() || start.user_code.is_empty() {
        return Err("o github não devolveu o código".into());
    }
    *LOGIN.lock().unwrap_or_else(|err| err.into_inner()) = Some(PendingLogin {
        device_code: start.device_code,
        interval: start.interval,
        client_id,
    });
    Ok(LoginBegin {
        user_code: start.user_code,
        verification_uri: start.verification_uri,
        interval: start.interval,
    })
}

#[tauri::command]
pub async fn github_login_poll(app: AppHandle) -> Result<LoginState, String> {
    let pending = LOGIN.lock().unwrap_or_else(|err| err.into_inner()).clone();
    let Some(pending) = pending else {
        return github_status(app);
    };
    let client = reqwest::Client::builder().user_agent("bob-tudo").build().map_err(|err| err.to_string())?;
    let response = client
        .post("https://github.com/login/oauth/access_token")
        .header("accept", "application/json")
        .form(&[
            ("client_id", pending.client_id.as_str()),
            ("device_code", pending.device_code.as_str()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let body = response.text().await.map_err(|err| err.to_string())?;
    match parse_device_poll(&body) {
        DevicePoll::Pending | DevicePoll::SlowDown => Ok(LoginState { connected: false, login: String::new() }),
        DevicePoll::Denied(message) => {
            *LOGIN.lock().unwrap_or_else(|err| err.into_inner()) = None;
            Err(message)
        }
        DevicePoll::Granted { token } => {
            let data_dir = app.state::<AppState>().lock().data_dir.clone();
            secrets::store_token(&data_dir, &token)?;
            *LOGIN.lock().unwrap_or_else(|err| err.into_inner()) = None;
            let login = github_login_name(&token).await.unwrap_or_default();
            let _ = with_index(&app, |index| index.meta_set("github_login", &login));
            Ok(LoginState { connected: true, login })
        }
    }
}

#[tauri::command]
pub fn github_status(app: AppHandle) -> Result<LoginState, String> {
    let data_dir = app.state::<AppState>().lock().data_dir.clone();
    let connected = secrets::read_token(&data_dir).is_some();
    let login = if connected { with_index(&app, |index| index.meta_get("github_login").unwrap_or_default()) } else { String::new() };
    Ok(LoginState { connected, login })
}

#[tauri::command]
pub fn github_logout(app: AppHandle) -> Result<(), String> {
    let data_dir = app.state::<AppState>().lock().data_dir.clone();
    secrets::delete_token(&data_dir);
    let _ = with_index(&app, |index| index.meta_set("github_login", ""));
    Ok(())
}

async fn github_login_name(token: &str) -> Result<String, String> {
    let client = reqwest::Client::builder().user_agent("bob-tudo").build().map_err(|err| err.to_string())?;
    let response = client
        .get("https://api.github.com/user")
        .bearer_auth(token)
        .header("accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|err| err.to_string())?;
    let body = response.text().await.map_err(|err| err.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
    Ok(value.get("login").and_then(|v| v.as_str()).unwrap_or("").to_string())
}

impl Clone for PendingLogin {
    fn clone(&self) -> Self {
        Self {
            device_code: self.device_code.clone(),
            interval: self.interval,
            client_id: self.client_id.clone(),
        }
    }
}
