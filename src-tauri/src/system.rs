use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bob_core::{impact, privacy_toggles, DiskHealth, RegDesired};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::elevate::{self, elevate_sync, undo_to_op, RegOp, Task};
use crate::hidden;
use crate::state::{AppState, DisabledStartup, SavedPrivacy, UndoOp};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupItem {
    pub id: String,
    pub name: String,
    pub command: String,
    pub source: String,
    pub enabled: bool,
    pub impact: String,
    pub can_toggle: bool,
    pub needs_admin: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyItem {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub needs_admin: bool,
    pub enabled: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestorePoint {
    pub sequence: u32,
    pub description: String,
    pub when: String,
}

pub fn ensure_restore_point_state(state: &AppState, reason: &str) -> String {
    let recent = state
        .lock()
        .last_restore
        .is_some_and(|instant| instant.elapsed() < Duration::from_secs(600));
    if recent {
        return "ponto de restauração recente reutilizado".into();
    }
    match elevate_sync(&Task::RestorePoint {
        description: format!("bob tudo: {reason}"),
    }) {
        Ok(result) if result.ok => {
            state.lock().last_restore = Some(Instant::now());
            "ponto de restauração criado".into()
        }
        Ok(result) => format!("sem ponto de restauração: {}", result.message),
        Err(err) => format!("sem ponto de restauração: {err}"),
    }
}

#[tauri::command]
pub async fn startup_list(app: AppHandle) -> Result<Vec<StartupItem>, String> {
    let mut items = Vec::new();
    for (hive, needs_admin) in [("hkcu", false), ("hklm", true)] {
        let path = r"Software\Microsoft\Windows\CurrentVersion\Run";
        for (name, command) in elevate::enum_run(hive, path)? {
            items.push(StartupItem {
                id: format!("run:{hive}:{name}"),
                impact: impact(&name, &command).into(),
                name,
                command,
                source: if hive == "hkcu" { "registro do usuário" } else { "registro da máquina" }.into(),
                enabled: true,
                can_toggle: true,
                needs_admin,
            });
        }
    }
    let disabled = app.state::<AppState>().lock().disabled_startup.clone();
    for item in disabled {
        if items.iter().any(|existing| existing.id == item.id) {
            continue;
        }
        let needs_admin = item.id.starts_with("run:hklm:");
        items.push(StartupItem {
            impact: impact(&item.name, &item.command).into(),
            id: item.id,
            name: item.name,
            command: item.command,
            source: "desligado pelo bob tudo".into(),
            enabled: false,
            can_toggle: true,
            needs_admin,
        });
    }
    items.extend(startup_folders());
    if let Ok(tasks) = logon_tasks().await {
        items.extend(tasks);
    }
    if let Ok(services) = safe_services().await {
        items.extend(services);
    }
    Ok(items)
}

fn startup_folders() -> Vec<StartupItem> {
    let mut folders = Vec::new();
    if let Some(roaming) = dirs::config_dir() {
        folders.push((
            roaming.join(r"Microsoft\Windows\Start Menu\Programs\Startup"),
            false,
        ));
    }
    folders.push((
        PathBuf::from(r"C:\ProgramData\Microsoft\Windows\Start Menu\Programs\Startup"),
        true,
    ));
    let mut items = Vec::new();
    for (folder, needs_admin) in folders {
        let Ok(entries) = std::fs::read_dir(&folder) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if lower.ends_with(".ini") {
                continue;
            }
            let enabled = !lower.contains(".bobtudo-off");
            items.push(StartupItem {
                id: format!("folder:{}", path.display()),
                impact: impact(&name, &path.display().to_string()).into(),
                name,
                command: path.display().to_string(),
                source: "pasta de inicialização".into(),
                enabled,
                can_toggle: true,
                needs_admin,
            });
        }
    }
    items
}

async fn logon_tasks() -> Result<Vec<StartupItem>, String> {
    #[cfg(not(windows))]
    {
        return Ok(Vec::new());
    }
    #[cfg(windows)]
    {
        let script = r#"
Get-ScheduledTask | ForEach-Object {
  $logon = $false
  foreach ($t in $_.Triggers) {
    if ($t.CimClass.CimClassName -match 'Logon') { $logon = $true }
  }
  if ($logon -and $_.TaskPath -notlike '\Microsoft\*') {
    [pscustomobject]@{
      name = $_.TaskName
      path = ($_.TaskPath + $_.TaskName)
      state = [string]$_.State
    }
  }
} | ConvertTo-Json -Compress
"#;
        let text = ps(script).await?;
        let items = json_array(&text);
        Ok(items
            .into_iter()
            .filter_map(|item| {
                let name = item.get("name")?.as_str()?.to_string();
                let path = item.get("path")?.as_str()?.to_string();
                let state = item.get("state").and_then(|v| v.as_str()).unwrap_or("");
                Some(StartupItem {
                    id: format!("task:{path}"),
                    impact: impact(&name, &path).into(),
                    name,
                    command: path,
                    source: "tarefa agendada".into(),
                    enabled: !state.eq_ignore_ascii_case("disabled"),
                    can_toggle: true,
                    needs_admin: true,
                })
            })
            .collect())
    }
}

async fn safe_services() -> Result<Vec<StartupItem>, String> {
    #[cfg(not(windows))]
    {
        return Ok(Vec::new());
    }
    #[cfg(windows)]
    {
        let script = r#"
Get-CimInstance Win32_Service | Select-Object Name, DisplayName, StartMode, State, PathName | ConvertTo-Json -Compress
"#;
        let text = ps(script).await?;
        let mut items = Vec::new();
        for item in json_array(&text) {
            let name = item.get("Name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let display = item.get("DisplayName").and_then(|v| v.as_str()).unwrap_or(&name).to_string();
            let mode = item.get("StartMode").and_then(|v| v.as_str()).unwrap_or("");
            let path = item.get("PathName").and_then(|v| v.as_str()).unwrap_or("").to_string();
            if !service_is_safe(&name, &path) {
                continue;
            }
            if !mode.eq_ignore_ascii_case("auto") && !mode.eq_ignore_ascii_case("disabled") {
                continue;
            }
            items.push(StartupItem {
                id: format!("svc:{name}"),
                impact: impact(&display, &path).into(),
                name: display,
                command: path,
                source: "serviço".into(),
                enabled: !mode.eq_ignore_ascii_case("disabled"),
                can_toggle: true,
                needs_admin: true,
            });
        }
        Ok(items)
    }
}

fn service_is_safe(name: &str, path: &str) -> bool {
    let name = name.to_ascii_lowercase();
    const BLOCK: &[&str] = &[
        "rpcss", "dcomlaunch", "samss", "winmgmt", "schedule", "eventlog", "dhcp", "dnscache",
        "nlasvc", "bfe", "mpssvc", "windefend", "wscsvc", "cryptsvc", "wuauserv", "bits", "themes",
        "audiosrv", "spooler", "eventsystem", "power", "profsvc", "usermanager", "staterepository",
        "appxsvc", "camsvc", "securityhealthservice", "wsearch", "dps",
    ];
    if BLOCK.contains(&name.as_str()) || path.is_empty() {
        return false;
    }
    let lower = path.to_lowercase();
    if lower.contains(r"\windows\system32\") || lower.contains(r"\windows\syswow64\") || lower.contains(r"\windows\systemapps\") {
        return false;
    }
    true
}

#[tauri::command]
pub async fn startup_set(app: AppHandle, id: String, enabled: bool) -> Result<(), String> {
    if id.starts_with("run:") {
        set_run(&app, &id, enabled)
    } else if let Some(path) = id.strip_prefix("folder:") {
        set_folder(&app, path, enabled)
    } else if let Some(name) = id.strip_prefix("task:") {
        let _ = ensure_restore_point_state(app.state::<AppState>().inner(), "tarefa de inicialização");
        let result = elevate_sync(&Task::ScheduledTask {
            name: name.to_string(),
            enabled,
        })?;
        if !result.ok {
            return Err(result.message);
        }
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        crate::state::push_change(
            &mut shared,
            format!("{} tarefa {name}", if enabled { "ligou" } else { "desligou" }),
            vec![UndoOp::Task {
                name: name.into(),
                enabled: !enabled,
            }],
        );
        Ok(())
    } else if let Some(name) = id.strip_prefix("svc:") {
        if !service_is_safe(name, "outside") && !service_is_safe(name, r"C:\Program Files\app\svc.exe") {
            return Err("serviço protegido".into());
        }
        let _ = ensure_restore_point_state(app.state::<AppState>().inner(), "serviço");
        let start = if enabled { "auto" } else { "disabled" };
        let result = elevate_sync(&Task::Service {
            name: name.into(),
            start: start.into(),
        })?;
        if !result.ok {
            return Err(result.message);
        }
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        crate::state::push_change(
            &mut shared,
            format!("{} serviço {name}", if enabled { "ligou" } else { "desligou" }),
            vec![UndoOp::Service {
                name: name.into(),
                start: if enabled { "disabled" } else { "auto" }.into(),
            }],
        );
        Ok(())
    } else {
        Err("item desconhecido".into())
    }
}

fn set_run(app: &AppHandle, id: &str, enabled: bool) -> Result<(), String> {
    let mut parts = id.splitn(3, ':');
    let _ = parts.next();
    let hive = parts.next().unwrap_or("");
    let name = parts.next().unwrap_or("");
    if hive.is_empty() || name.is_empty() {
        return Err("entrada inválida".into());
    }
    let path = r"Software\Microsoft\Windows\CurrentVersion\Run";
    let needs_admin = hive == "hklm";
    if needs_admin {
        let _ = ensure_restore_point_state(app.state::<AppState>().inner(), "inicialização");
    }
    if enabled {
        let saved = app
            .state::<AppState>()
            .lock()
            .disabled_startup
            .iter()
            .find(|item| item.id == id)
            .cloned();
        let Some(saved) = saved else {
            return Err("não há valor guardado para religar".into());
        };
        let op = RegOp {
            hive: hive.into(),
            path: path.into(),
            name: name.into(),
            delete: false,
            dword: None,
            sz: Some(saved.command.clone()),
        };
        write_ops(&[op], needs_admin)?;
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        shared.disabled_startup.retain(|item| item.id != id);
        crate::state::save_startup(&shared);
        crate::state::push_change(&mut shared, format!("religou {name}"), vec![]);
        Ok(())
    } else {
        let current = elevate::enum_run(hive, path)?;
        let Some((_, command)) = current.into_iter().find(|(item, _)| item == name) else {
            return Err("entrada não encontrada".into());
        };
        let op = RegOp {
            hive: hive.into(),
            path: path.into(),
            name: name.into(),
            delete: true,
            dword: None,
            sz: None,
        };
        write_ops(&[op], needs_admin)?;
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        shared.disabled_startup.push(DisabledStartup {
            id: id.into(),
            name: name.into(),
            command: command.clone(),
        });
        crate::state::save_startup(&shared);
        crate::state::push_change(
            &mut shared,
            format!("desligou {name}"),
            vec![UndoOp::Reg {
                hive: hive.into(),
                path: path.into(),
                name: name.into(),
                existed: true,
                dword: None,
                sz: Some(command),
            }],
        );
        Ok(())
    }
}

fn set_folder(app: &AppHandle, path: &str, enabled: bool) -> Result<(), String> {
    let path = PathBuf::from(path);
    let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
    let target = if enabled {
        let text = path.display().to_string();
        PathBuf::from(text.replace(".bobtudo-off", ""))
    } else if name.contains(".bobtudo-off") {
        path.clone()
    } else {
        PathBuf::from(format!("{}.bobtudo-off", path.display()))
    };
    if path != target {
        std::fs::rename(&path, &target).map_err(|err| err.to_string())?;
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        crate::state::push_change(
            &mut shared,
            format!("{} {}", if enabled { "religou" } else { "desligou" }, name),
            vec![UndoOp::Rename {
                from: target.display().to_string(),
                to: path.display().to_string(),
            }],
        );
    }
    Ok(())
}

fn write_ops(ops: &[RegOp], admin: bool) -> Result<(), String> {
    if admin {
        let result = elevate_sync(&Task::RegApply { ops: ops.to_vec() })?;
        if result.ok {
            Ok(())
        } else {
            Err(result.message)
        }
    } else {
        match elevate::apply_user(ops) {
            Ok(()) => Ok(()),
            Err(err) if err.contains("acesso") || err.contains("denied") || err.contains("5") => {
                let result = elevate_sync(&Task::RegApply { ops: ops.to_vec() })?;
                if result.ok {
                    Ok(())
                } else {
                    Err(result.message)
                }
            }
            Err(err) => Err(err),
        }
    }
}

#[tauri::command]
pub fn privacy_list() -> Vec<PrivacyItem> {
    privacy_toggles()
        .iter()
        .map(|toggle| PrivacyItem {
            id: toggle.id.into(),
            title: toggle.title.into(),
            detail: toggle.detail.into(),
            needs_admin: toggle.needs_admin,
            enabled: toggle.entries.iter().all(|entry| entry_matches(entry)),
        })
        .collect()
}

fn entry_matches(entry: &bob_core::RegEntry) -> bool {
    let Ok(Some(UndoOp::Reg { existed, dword, sz, .. })) = elevate::read_reg(entry.hive, entry.path, entry.name) else {
        return false;
    };
    if !existed {
        return false;
    }
    match entry.when_on {
        RegDesired::Dword(value) => dword == Some(value),
        RegDesired::Sz(value) => sz.as_deref() == Some(value),
    }
}

#[tauri::command]
pub fn privacy_set(app: AppHandle, id: String, enabled: bool) -> Result<String, String> {
    let toggle = privacy_toggles()
        .iter()
        .find(|toggle| toggle.id == id)
        .ok_or("opção desconhecida")?;
    let note = if toggle.needs_admin {
        ensure_restore_point_state(app.state::<AppState>().inner(), toggle.title)
    } else {
        String::new()
    };
    export_keys(app.state::<AppState>().inner(), toggle.entries);
    if enabled {
        let mut previous = Vec::new();
        let mut ops = Vec::new();
        for entry in toggle.entries {
            if let Ok(Some(op)) = elevate::read_reg(entry.hive, entry.path, entry.name) {
                previous.push(op);
            }
            ops.push(desired_op(entry));
        }
        write_ops(&ops, toggle.needs_admin)?;
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        shared.privacy_saved.retain(|item| item.id != toggle.id);
        shared.privacy_saved.push(SavedPrivacy {
            id: toggle.id.into(),
            ops: previous.clone(),
        });
        crate::state::save_privacy(&shared);
        crate::state::push_change(&mut shared, format!("ligou {}", toggle.title), previous);
    } else {
        let saved = app
            .state::<AppState>()
            .lock()
            .privacy_saved
            .iter()
            .find(|item| item.id == toggle.id)
            .cloned();
        let ops = if let Some(saved) = saved {
            saved.ops.iter().filter_map(undo_to_op).collect::<Vec<_>>()
        } else {
            toggle
                .entries
                .iter()
                .map(|entry| RegOp {
                    hive: entry.hive.into(),
                    path: entry.path.into(),
                    name: entry.name.into(),
                    delete: true,
                    dword: None,
                    sz: None,
                })
                .collect()
        };
        write_ops(&ops, toggle.needs_admin)?;
        let state = app.state::<AppState>();
        let mut shared = state.lock();
        shared.privacy_saved.retain(|item| item.id != toggle.id);
        crate::state::save_privacy(&shared);
        crate::state::push_change(&mut shared, format!("desligou {}", toggle.title), ops.iter().filter_map(|op| {
            Some(UndoOp::Reg {
                hive: op.hive.clone(),
                path: op.path.clone(),
                name: op.name.clone(),
                existed: !op.delete,
                dword: op.dword,
                sz: op.sz.clone(),
            })
        }).collect());
    }
    Ok(note)
}

fn desired_op(entry: &bob_core::RegEntry) -> RegOp {
    match entry.when_on {
        RegDesired::Dword(value) => RegOp {
            hive: entry.hive.into(),
            path: entry.path.into(),
            name: entry.name.into(),
            delete: false,
            dword: Some(value),
            sz: None,
        },
        RegDesired::Sz(value) => RegOp {
            hive: entry.hive.into(),
            path: entry.path.into(),
            name: entry.name.into(),
            delete: false,
            dword: None,
            sz: Some(value.into()),
        },
    }
}

fn export_keys(state: &AppState, entries: &[bob_core::RegEntry]) {
    let dir = state.lock().data_dir.join("backups");
    let _ = std::fs::create_dir_all(&dir);
    let mut seen = std::collections::HashSet::new();
    for entry in entries {
        let key = format!("{}\\{}", entry.hive, entry.path);
        if !seen.insert(key.clone()) {
            continue;
        }
        let root = if entry.hive.eq_ignore_ascii_case("hklm") { "HKLM" } else { "HKCU" };
        let file = dir.join(format!("{}.reg", bob_core::nid("reg")));
        let _ = hidden::std_command("reg")
            .args(["export", &format!("{root}\\{}", entry.path), &file.display().to_string(), "/y"])
            .output();
    }
}

#[tauri::command]
pub async fn disk_health(admin: Option<bool>) -> Result<Vec<DiskHealth>, String> {
    #[cfg(not(windows))]
    {
        let _ = admin;
        return Err("a leitura smart roda no windows, via msft_storagereliabilitycounter ou smartctl".into());
    }
    #[cfg(windows)]
    {
        let script = r#"
$disks = @()
Get-PhysicalDisk | ForEach-Object {
  $rel = $_ | Get-StorageReliabilityCounter -ErrorAction SilentlyContinue
  $disks += [pscustomobject]@{
    name = $_.FriendlyName
    media = [string]$_.MediaType
    health = [string]$_.HealthStatus
    temp = if ($rel) { $rel.Temperature } else { $null }
    hours = if ($rel) { $rel.PowerOnHours } else { $null }
    wear = if ($rel) { $rel.Wear } else { $null }
    readErrors = if ($rel) { $rel.ReadErrorsTotal } else { $null }
    writeErrors = if ($rel) { $rel.WriteErrorsTotal } else { $null }
    size = $_.Size
  }
}
$disks | ConvertTo-Json -Compress
"#;
        let text = if admin.unwrap_or(false) {
            let result = elevate_sync(&Task::Powershell {
                script: script.into(),
            })?;
            if !result.ok {
                return Err(result.message);
            }
            result.output
        } else {
            ps(script).await?
        };
        let mut disks = bob_core::parse_storage_json(&text);
        if disks.is_empty() {
            disks = smartctl_disks().await.unwrap_or_default();
        }
        if disks.is_empty() {
            return Err("não li o smart. tente de novo como administrador, ou deixe o smartctl no path.".into());
        }
        Ok(disks)
    }
}

async fn smartctl_disks() -> Result<Vec<DiskHealth>, String> {
    let bin = which_smartctl();
    let Some(bin) = bin else {
        return Ok(Vec::new());
    };
    let output = hidden::tokio_command(&bin)
        .args(["--scan-open", "-j"])
        .output()
        .await
        .map_err(|err| err.to_string())?;
    let text = String::from_utf8_lossy(&output.stdout);
    let value: Value = serde_json::from_str(text.trim()).unwrap_or(Value::Null);
    let mut names = Vec::new();
    if let Some(devices) = value.get("devices").and_then(|v| v.as_array()) {
        for device in devices {
            if let Some(name) = device.get("name").and_then(|v| v.as_str()) {
                names.push(name.to_string());
            }
        }
    }
    let mut disks = Vec::new();
    for name in names {
        let output = hidden::tokio_command(&bin)
            .args(["-a", "-j", &name])
            .output()
            .await
            .map_err(|err| err.to_string())?;
        let text = String::from_utf8_lossy(&output.stdout);
        if let Some(disk) = bob_core::parse_smartctl(&text) {
            disks.push(disk);
        }
    }
    Ok(disks)
}

fn which_smartctl() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("smartctl"),
        PathBuf::from(r"C:\Program Files\smartmontools\bin\smartctl.exe"),
        dirs::data_local_dir().unwrap_or_default().join(r"bob tudo\smartctl.exe"),
    ];
    for candidate in candidates {
        if candidate.exists() || candidate == Path::new("smartctl") {
            return Some(candidate);
        }
    }
    None
}

#[tauri::command]
pub async fn restore_list(app: AppHandle) -> Result<RestoreSnapshot, String> {
    let changes = app
        .state::<AppState>()
        .lock()
        .changes
        .iter()
        .filter(|change| !change.undone)
        .cloned()
        .collect();
    let points = list_points().await.unwrap_or_default();
    Ok(RestoreSnapshot { points, changes })
}

#[derive(Serialize)]
pub struct RestoreSnapshot {
    pub points: Vec<RestorePoint>,
    pub changes: Vec<crate::state::Change>,
}

async fn list_points() -> Result<Vec<RestorePoint>, String> {
    #[cfg(not(windows))]
    {
        return Ok(Vec::new());
    }
    #[cfg(windows)]
    {
        let script = r#"
Get-ComputerRestorePoint | ForEach-Object {
  [pscustomobject]@{
    sequence = $_.SequenceNumber
    description = $_.Description
    when = ([Management.ManagementDateTimeConverter]::ToDateTime($_.CreationTime)).ToString('o')
  }
} | ConvertTo-Json -Compress
"#;
        let text = ps(script).await?;
        Ok(json_array(&text)
            .into_iter()
            .filter_map(|item| {
                Some(RestorePoint {
                    sequence: item.get("sequence")?.as_u64()? as u32,
                    description: item.get("description")?.as_str().unwrap_or("").to_string(),
                    when: item.get("when")?.as_str().unwrap_or("").to_string(),
                })
            })
            .collect())
    }
}

#[tauri::command]
pub fn restore_create() -> Result<String, String> {
    let result = elevate_sync(&Task::RestorePoint {
        description: "bob tudo: ponto manual".into(),
    })?;
    if result.ok {
        Ok("ponto de restauração criado".into())
    } else {
        Err(result.message)
    }
}

#[tauri::command]
pub fn restore_undo(app: AppHandle, id: String) -> Result<(), String> {
    let change = app
        .state::<AppState>()
        .lock()
        .changes
        .iter()
        .find(|change| change.id == id)
        .cloned()
        .ok_or("alteração não encontrada")?;
    let mut reg_ops = Vec::new();
    for op in &change.ops {
        match op {
            UndoOp::Reg { .. } => {
                if let Some(reg) = undo_to_op(op) {
                    reg_ops.push(reg);
                }
            }
            UndoOp::Rename { from, to } => {
                std::fs::rename(from, to).map_err(|err| err.to_string())?;
            }
            UndoOp::Service { name, start } => {
                let result = elevate_sync(&Task::Service {
                    name: name.clone(),
                    start: start.clone(),
                })?;
                if !result.ok {
                    return Err(result.message);
                }
            }
            UndoOp::Task { name, enabled } => {
                let result = elevate_sync(&Task::ScheduledTask {
                    name: name.clone(),
                    enabled: *enabled,
                })?;
                if !result.ok {
                    return Err(result.message);
                }
            }
        }
    }
    if !reg_ops.is_empty() {
        let admin = reg_ops.iter().any(|op| op.hive.eq_ignore_ascii_case("hklm"));
        write_ops(&reg_ops, admin)?;
    }
    let state = app.state::<AppState>();
    let mut shared = state.lock();
    if let Some(change) = shared.changes.iter_mut().find(|change| change.id == id) {
        change.undone = true;
    }
    crate::state::save_changes(&shared);
    Ok(())
}

#[tauri::command]
pub fn restore_system(sequence: u32) -> Result<String, String> {
    let result = elevate_sync(&Task::RestoreComputer { sequence })?;
    if result.ok {
        Ok("o windows vai restaurar e reiniciar".into())
    } else {
        Err(result.message)
    }
}

pub async fn ps(script: &str) -> Result<String, String> {
    let wrapped = format!(
        "[Console]::OutputEncoding = New-Object System.Text.UTF8Encoding $false; $OutputEncoding = [Console]::OutputEncoding; {script}"
    );
    let output = hidden::tokio_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &wrapped])
        .output()
        .await
        .map_err(|err| err.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    if !output.status.success() {
        let message = format!("{stderr}\n{stdout}").trim().to_string();
        return Err(if message.is_empty() { "powershell falhou".into() } else { message });
    }
    Ok(stdout)
}

fn json_array(text: &str) -> Vec<Value> {
    let start = text.find(['{', '[']);
    let Some(start) = start else {
        return Vec::new();
    };
    match serde_json::from_str::<Value>(text[start..].trim()) {
        Ok(Value::Array(items)) => items,
        Ok(value @ Value::Object(_)) => vec![value],
        _ => Vec::new(),
    }
}

pub fn notify(app: &AppHandle, body: &str) {
    notify_kind(app, "task", body);
}

pub fn notify_kind(app: &AppHandle, kind: &str, body: &str) {
    let state = app.state::<AppState>();
    let config = state.lock().config.clone();
    let allow = if !config.notifications {
        false
    } else {
        match kind {
            "disk" => config.notify_disk_full,
            "updates" => config.notify_app_updates,
            "junk" => config.notify_junk,
            "smart" => config.notify_smart,
            "task" => config.notify_task_done,
            _ => true,
        }
    };
    if !allow || body.trim().is_empty() {
        return;
    }
    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification().builder().title("bob tudo").body(body).show();
}
