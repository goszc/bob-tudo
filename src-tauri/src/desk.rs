//! Busca, pacote, drivers, rede, jogo, energia, relatório e agenda.
//! O que muda o sistema pede confirmação na janela antes de chegar aqui.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use bob_core::{
    bundle_from_json, bundle_to_json, dns_presets, parse_battery_html, parse_driver_table, parse_ping, parse_power_plans,
    render_report_html, schedule_due, search_files, simple_pdf, BundleApp, BundleCollection, DnsPreset, DriverInfo, FileQuery,
    PcBundle, PcSummary, PowerPlan, HIGH_PERFORMANCE,
};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::elevate::{elevate_sync, Task};
use crate::hidden;
use crate::state::{write_json, AppState};

static PAUSED: AtomicBool = AtomicBool::new(false);
static PLAN: Mutex<String> = Mutex::new(String::new());
static CLOSED: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn paused() -> bool {
    PAUSED.load(Ordering::Relaxed)
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameState {
    pub on: bool,
    pub plan: String,
    pub closed: Vec<String>,
}

#[tauri::command]
pub fn file_search(query: FileQuery) -> Result<Vec<bob_core::FileHit>, String> {
    search_files(&query)
}

#[tauri::command]
pub fn file_open(path: String) -> Result<(), String> {
    hidden::gui_command(if cfg!(windows) { "explorer" } else { "xdg-open" })
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn file_reveal(path: String) -> Result<(), String> {
    crate::disk::open_in_explorer(path)
}

#[tauri::command]
pub fn file_copy(path: String) -> Result<(), String> {
    #[cfg(windows)]
    {
        let script = format!("Set-Clipboard -Value {}", crate::elevate::ps_quote(&path));
        let output = hidden::std_command("powershell").args(["-NoProfile", "-Command", &script]).output().map_err(|err| err.to_string())?;
        if output.status.success() { Ok(()) } else { Err("não copiei o caminho".into()) }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err("copiar o caminho é no windows".into())
    }
}

#[tauri::command]
pub fn bundle_preview(app: AppHandle) -> Result<PcBundle, String> {
    Ok(build_bundle(&app))
}

#[tauri::command]
pub fn bundle_save(app: AppHandle, path: String) -> Result<(), String> {
    let text = bundle_to_json(&build_bundle(&app));
    std::fs::write(path, text).map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn bundle_import(app: AppHandle, text: String) -> Result<(), String> {
    let bundle = bundle_from_json(&text)?;
    if let Some(theme) = ["light", "dark", "glass-light", "glass-dark", "system"].into_iter().find(|item| *item == bundle.theme) {
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        shared.config.theme = theme.into();
        shared.config.game_apps = bundle.game_apps.clone();
        crate::state::save_config(&shared);
    }
    for id in &bundle.favorites {
        let _ = crate::catalog::catalog_favorite(app.clone(), id.clone(), true);
    }
    for collection in &bundle.collections {
        if let Ok(created) = crate::catalog::catalog_create_collection(app.clone(), collection.name.clone()) {
            for app_id in &collection.apps {
                let _ = crate::catalog::catalog_collection_app(app.clone(), created.id.clone(), app_id.clone(), true);
            }
        }
    }
    let requests = bundle
        .apps
        .iter()
        .filter(|item| matches!(item.source.as_str(), "winget" | "scoop" | "chocolatey"))
        .map(|item| crate::catalog::InstallReq::from_parts(&item.source, &item.id, &item.version))
        .collect::<Vec<_>>();
    if requests.is_empty() {
        return Ok(());
    }
    crate::catalog::catalog_install(app, requests).await
}

fn build_bundle(app: &AppHandle) -> PcBundle {
    let state = app.state::<AppState>();
    let shared = state.lock();
    let theme = shared.config.theme.clone();
    let game_apps = shared.config.game_apps.clone();
    let fallback = shared
        .upgrades
        .iter()
        .map(|row| BundleApp { source: "winget".into(), id: row.id.clone(), name: row.name.clone(), version: row.version.clone() })
        .collect::<Vec<_>>();
    drop(shared);
    let (installed, favorites, collections) = crate::catalog::catalog_snapshot(app);
    let apps = if installed.is_empty() { fallback } else { installed };
    PcBundle {
        format: 1,
        exported_ms: now_ms(),
        apps,
        favorites,
        collections: collections.into_iter().map(|item| BundleCollection { id: item.id, name: item.name, apps: item.apps }).collect(),
        theme,
        game_apps,
    }
}

#[tauri::command]
pub fn drivers_list(app: AppHandle) -> Vec<DriverInfo> {
    let path = app.state::<AppState>().lock().data_dir.join("drivers.json");
    std::fs::read(&path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default()
}

#[tauri::command]
pub async fn drivers_refresh(app: AppHandle) -> Result<Vec<DriverInfo>, String> {
    let text = tokio::task::spawn_blocking(|| {
        #[cfg(windows)]
        {
            hidden::std_command("powershell")
                .args(["-NoProfile", "-Command", "Get-CimInstance Win32_PnPSignedDriver | Where-Object DeviceName | Select-Object -First 80 DeviceName,DeviceClass,DriverVersion,DriverDate | ForEach-Object { '{0}|{1}|{2}|{3}' -f $_.DeviceName,$_.DeviceClass,$_.DriverVersion,$_.DriverDate }"])
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
                .unwrap_or_default()
        }
        #[cfg(not(windows))]
        {
            String::new()
        }
    })
    .await
    .map_err(|err| err.to_string())?;
    let rows = parse_driver_table(&text, now_ms());
    let path = app.state::<AppState>().lock().data_dir.join("drivers.json");
    let _ = write_json(&path, &rows);
    Ok(rows)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Speed {
    pub mbps: f64,
    pub ping_ms: f64,
}

#[tauri::command]
pub async fn net_speed() -> Result<Speed, String> {
    let started = std::time::Instant::now();
    let client = reqwest::Client::builder().user_agent("bob-tudo").timeout(std::time::Duration::from_secs(20)).build().map_err(|err| err.to_string())?;
    let response = client.get("https://speed.cloudflare.com/__down?bytes=4000000").send().await.map_err(|err| err.to_string())?;
    let bytes = response.bytes().await.map_err(|err| err.to_string())?;
    let mbps = bob_core::mbps(bytes.len() as u64, started.elapsed().as_secs_f64());
    let ping_ms = ping_once().await.unwrap_or(0.0);
    Ok(Speed { mbps, ping_ms })
}

#[tauri::command]
pub async fn net_ping() -> Result<f64, String> {
    ping_once().await.ok_or_else(|| "sem resposta".into())
}

async fn ping_once() -> Option<f64> {
    let output = tokio::task::spawn_blocking(|| {
        #[cfg(windows)]
        let result = hidden::std_command("ping").args(["-n", "1", "-w", "1200", "1.1.1.1"]).output();
        #[cfg(not(windows))]
        let result = hidden::std_command("ping").args(["-c", "1", "-W", "1", "1.1.1.1"]).output();
        result.ok().map(|output| String::from_utf8_lossy(&output.stdout).to_string())
    })
    .await
    .ok()??;
    parse_ping(&output)
}

#[tauri::command]
pub fn net_flush() -> Result<String, String> {
    #[cfg(windows)]
    {
        let output = hidden::std_command("ipconfig").arg("/flushdns").output().map_err(|err| err.to_string())?;
        if output.status.success() { Ok("dns limpo".into()) } else { Err("não limpei o dns".into()) }
    }
    #[cfg(not(windows))]
    {
        Err("limpar o dns é no windows".into())
    }
}

#[tauri::command]
pub fn dns_list() -> Vec<DnsPreset> {
    dns_presets()
}

#[tauri::command]
pub async fn dns_apply(id: String) -> Result<String, String> {
    let preset = dns_presets().into_iter().find(|item| item.id == id).ok_or("preset desconhecido")?;
    #[cfg(windows)]
    {
        let script = if preset.servers.is_empty() {
            "Get-NetAdapter | Where-Object Status -eq 'Up' | ForEach-Object { Set-DnsClientServerAddress -InterfaceIndex $_.ifIndex -ResetServerAddresses }".into()
        } else {
            let list = preset.servers.iter().map(|server| format!("'{}'", server)).collect::<Vec<_>>().join(",");
            format!("$s=@({list}); Get-NetAdapter | Where-Object Status -eq 'Up' | ForEach-Object {{ Set-DnsClientServerAddress -InterfaceIndex $_.ifIndex -ServerAddresses $s }}")
        };
        let result = tokio::task::spawn_blocking(move || elevate_sync(&Task::Powershell { script })).await.map_err(|err| err.to_string())??;
        if result.ok { Ok(format!("{} aplicado", preset.label)) } else { Err(result.message) }
    }
    #[cfg(not(windows))]
    {
        let _ = preset;
        Err("trocar o dns pede o windows".into())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NetProc {
    pub name: String,
    pub pid: u32,
    pub connections: u32,
}

#[tauri::command]
pub async fn net_processes() -> Result<Vec<NetProc>, String> {
    #[cfg(windows)]
    {
        let text = tokio::task::spawn_blocking(|| {
            hidden::std_command("powershell")
                .args(["-NoProfile", "-Command", "Get-NetTCPConnection -State Established -ErrorAction SilentlyContinue | Group-Object OwningProcess | Select-Object -First 12 | ForEach-Object { '{0}|{1}' -f $_.Name,$_.Count }"])
                .output()
                .map(|output| String::from_utf8_lossy(&output.stdout).to_string())
                .unwrap_or_default()
        })
        .await
        .map_err(|err| err.to_string())?;
        let mut rows = Vec::new();
        for line in text.lines() {
            let mut parts = line.split('|');
            let pid: u32 = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
            let connections: u32 = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
            if pid == 0 { continue; }
            rows.push(NetProc { name: format!("pid {pid}"), pid, connections });
        }
        return Ok(rows);
    }
    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

#[tauri::command]
pub fn game_state() -> GameState {
    GameState {
        on: paused(),
        plan: PLAN.lock().unwrap_or_else(|err| err.into_inner()).clone(),
        closed: CLOSED.lock().unwrap_or_else(|err| err.into_inner()).clone(),
    }
}

#[tauri::command]
pub async fn game_enter(app: AppHandle, close: Vec<String>) -> Result<GameState, String> {
    let close: Vec<String> = close.into_iter().map(|name| name.trim().to_string()).filter(|name| !name.is_empty()).collect();
    {
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        shared.config.game_apps = close.clone();
        crate::state::save_config(&shared);
    }
    let current = active_plan();
    *PLAN.lock().unwrap_or_else(|err| err.into_inner()) = current;
    let _ = set_plan(HIGH_PERFORMANCE);
    let mut closed = Vec::new();
    for name in &close {
        if stop_named(name) {
            closed.push(name.clone());
        }
    }
    *CLOSED.lock().unwrap_or_else(|err| err.into_inner()) = closed;
    PAUSED.store(true, Ordering::Relaxed);
    crate::monitor::monitor_watch(false);
    Ok(game_state())
}

#[tauri::command]
pub async fn game_exit(app: AppHandle) -> Result<GameState, String> {
    let plan = PLAN.lock().unwrap_or_else(|err| err.into_inner()).clone();
    if !plan.is_empty() {
        let _ = set_plan(&plan);
    }
    let closed = CLOSED.lock().unwrap_or_else(|err| err.into_inner()).clone();
    for name in closed {
        let _ = hidden::gui_command(name).spawn();
    }
    CLOSED.lock().unwrap_or_else(|err| err.into_inner()).clear();
    PAUSED.store(false, Ordering::Relaxed);
    let _ = app;
    Ok(game_state())
}

fn active_plan() -> String {
    #[cfg(windows)]
    {
        if let Ok(output) = hidden::std_command("powercfg").arg("/getactivescheme").output() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(plan) = parse_power_plans(&text).into_iter().next() {
                return plan.guid;
            }
        }
    }
    String::new()
}

fn set_plan(guid: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        let output = hidden::std_command("powercfg").args(["/setactive", guid]).output().map_err(|err| err.to_string())?;
        if output.status.success() { Ok(()) } else { Err("não troquei o plano".into()) }
    }
    #[cfg(not(windows))]
    {
        let _ = guid;
        Ok(())
    }
}

fn stop_named(name: &str) -> bool {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    let mut stopped = false;
    for process in sys.processes().values() {
        let current = process.name().to_string_lossy().to_lowercase();
        if current.contains(&name.to_lowercase()) && process.pid().as_u32() != std::process::id() && process.kill() {
            stopped = true;
        }
    }
    stopped
}

#[tauri::command]
pub fn power_list() -> Vec<PowerPlan> {
    #[cfg(windows)]
    {
        if let Ok(output) = hidden::std_command("powercfg").arg("/list").output() {
            return parse_power_plans(&String::from_utf8_lossy(&output.stdout));
        }
    }
    Vec::new()
}

#[tauri::command]
pub fn power_set(guid: String) -> Result<(), String> {
    set_plan(&guid)
}

#[tauri::command]
pub fn battery_report() -> Result<bob_core::BatteryReport, String> {
    #[cfg(windows)]
    {
        let path = std::env::temp_dir().join("bob-tudo-bateria.html");
        let path_text = path.display().to_string();
        let _ = hidden::std_command("powercfg").args(["/batteryreport", "/output", &path_text]).output();
        let html = std::fs::read_to_string(&path).map_err(|_| "este pc não gerou relatório de bateria".to_string())?;
        let _ = std::fs::remove_file(&path);
        parse_battery_html(&html).ok_or_else(|| "não achei capacidade da bateria".into())
    }
    #[cfg(not(windows))]
    {
        Err("bateria só no windows".into())
    }
}

#[tauri::command]
pub fn report_build(app: AppHandle) -> PcSummary {
    let shared = app.state::<AppState>();
    let shared = shared.lock();
    let sample = crate::monitor::monitor_sample();
    let space = if let Some(disk) = &shared.disk {
        format!("{} arquivos · {}", disk.files, disk.root)
    } else {
        "ainda sem mapa".into()
    };
    let health = shared.health.iter().map(|disk| format!("{} · {}", disk.name, disk.health)).collect();
    PcSummary {
        computer: whoami(),
        windows: windows_version(),
        cpu: format!("{:.0}% agora", sample.cpu),
        ram: format!("{} / {}", sample.ram_used / (1024 * 1024), sample.ram_total / (1024 * 1024)),
        gpu: if sample.gpu_name.is_empty() { "sem leitura".into() } else { sample.gpu_name },
        disks: crate::monitor::drive_rows().into_iter().map(|(name, used)| format!("{name} · {:.1} gb", used / 1024.0 / 1024.0 / 1024.0)).collect(),
        health,
        temp: sample.temp_c.map(|value| format!("{value:.0} °c")).unwrap_or_else(|| "sem sensor".into()),
        space,
        apps: shared.upgrades.len() as u32,
    }
}

fn whoami() -> String {
    std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_else(|_| "pc".into())
}

fn windows_version() -> String {
    #[cfg(windows)]
    {
        if let Ok(output) = hidden::std_command("cmd").args(["/C", "ver"]).output() {
            return String::from_utf8_lossy(&output.stdout).lines().find(|line| !line.trim().is_empty()).unwrap_or("windows").trim().to_string();
        }
    }
    "windows".into()
}

#[tauri::command]
pub fn report_html(summary: PcSummary) -> String {
    render_report_html(&summary)
}

#[tauri::command]
pub fn report_pdf(summary: PcSummary) -> Vec<u8> {
    let lines = vec![
        format!("computador {}", summary.computer),
        format!("windows {}", summary.windows),
        format!("cpu {}", summary.cpu),
        format!("memoria {}", summary.ram),
        format!("placa {}", summary.gpu),
        format!("espaco {}", summary.space),
        format!("temperatura {}", summary.temp),
        format!("apps {}", summary.apps),
    ];
    simple_pdf("relatorio do pc", &lines)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryView {
    pub cpu: Vec<bob_core::Point>,
    pub ram: Vec<bob_core::Point>,
    pub temp: Vec<bob_core::Point>,
    pub disks: Vec<bob_core::Grew>,
    pub folders: Vec<bob_core::Grew>,
}

#[tauri::command]
pub fn history_view(span: String) -> HistoryView {
    let now = now_ms();
    let window = match span.as_str() {
        "month" | "mes" => 30,
        "week" | "semana" => 7,
        _ => 1,
    } * 86_400_000;
    let from = now.saturating_sub(window);
    HistoryView {
        cpu: bob_core::series("cpu", "", from, now, 24),
        ram: bob_core::series("ram", "", from, now, 24),
        temp: bob_core::series("temp", "", from, now, 24),
        disks: bob_core::grew("disk", from, now),
        folders: bob_core::grew_folders(from, now),
    }
}

pub fn consider_schedules(app: &AppHandle) {
    let config = app.state::<AppState>().lock().config.clone();
    if !config.schedule_enabled {
        return;
    }
    let path = app.state::<AppState>().lock().data_dir.join("schedule.json");
    let mut last: std::collections::HashMap<String, i64> = std::fs::read(&path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
    let now = now_ms();
    for (id, days, body, kind) in [
        ("junk", config.schedule_junk_days, "a limpeza semanal está pronta para você olhar", "junk"),
        ("health", config.schedule_health_days, "vale conferir a saúde dos discos", "smart"),
    ] {
        let seen = last.get(id).copied().unwrap_or(0);
        if seen <= 0 {
            last.insert(id.into(), now);
        } else if schedule_due(true, days, seen, now) {
            crate::system::notify_kind(app, kind, body);
            last.insert(id.into(), now);
        }
    }
    let _ = write_json(&path, &last);
}

#[tauri::command]
pub fn monitor_tray(on: bool) {
    crate::monitor::tray_hot(on);
}
