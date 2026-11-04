use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::hidden;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum Task {
    RestorePoint { description: String },
    RegApply { ops: Vec<RegOp> },
    Service { name: String, start: String },
    ScheduledTask { name: String, enabled: bool },
    DismAnalyze,
    DismClean,
    WindowsUpdateClean,
    RestoreComputer { sequence: u32 },
    DeleteTree { paths: Vec<String> },
    EmptyRecycle,
    Powershell { script: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegOp {
    pub hive: String,
    pub path: String,
    pub name: String,
    #[serde(default)]
    pub delete: bool,
    pub dword: Option<u32>,
    pub sz: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskResult {
    pub ok: bool,
    pub message: String,
    pub output: String,
}

impl TaskResult {
    fn ok_msg(message: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            ok: true,
            message: message.into(),
            output: output.into(),
        }
    }

    fn fail(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
            output: String::new(),
        }
    }
}

pub fn run(path: &str) {
    let task_path = PathBuf::from(path);
    let result = match std::fs::read_to_string(&task_path) {
        Ok(text) => match serde_json::from_str::<Task>(&text) {
            Ok(task) => execute(&task),
            Err(err) => TaskResult::fail(err.to_string()),
        },
        Err(err) => TaskResult::fail(err.to_string()),
    };
    let result_path = task_path.with_extension("result.json");
    let _ = std::fs::write(result_path, serde_json::to_vec(&result).unwrap_or_default());
}

pub fn elevate_sync(task: &Task) -> Result<TaskResult, String> {
    #[cfg(not(windows))]
    {
        let _ = task;
        return Err("esta ação pede o windows".into());
    }
    #[cfg(windows)]
    {
        let dir = std::env::temp_dir().join("bob-tudo-tasks");
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let id = bob_core::nid("task");
        let path = dir.join(format!("{id}.json"));
        std::fs::write(&path, serde_json::to_vec(task).map_err(|err| err.to_string())?).map_err(|err| err.to_string())?;
        let exe = std::env::current_exe().map_err(|err| err.to_string())?;
        let exe = ps_quote(&exe.display().to_string());
        let arg = ps_quote(&path.display().to_string());
        let script = format!("Start-Process -FilePath {exe} -ArgumentList @('--elevated-task',{arg}) -Verb RunAs -Wait");
        let output = hidden::std_command("powershell")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script])
            .output()
            .map_err(|err| err.to_string())?;
        let result_path = path.with_extension("result.json");
        if let Ok(text) = std::fs::read_to_string(&result_path) {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(&result_path);
            return serde_json::from_str(&text).map_err(|err| err.to_string());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let _ = std::fs::remove_file(&path);
        Err(if stderr.trim().is_empty() && stdout.trim().is_empty() {
            "permissão de administrador negada".into()
        } else {
            format!("{stderr} {stdout}").trim().to_string()
        })
    }
}

fn execute(task: &Task) -> TaskResult {
    match task {
        Task::RestorePoint { description } => restore_point(description),
        Task::RegApply { ops } => apply_ops(ops),
        Task::Service { name, start } => service(name, start),
        Task::ScheduledTask { name, enabled } => scheduled_task(name, *enabled),
        Task::DismAnalyze => command("dism.exe", &["/Online", "/Cleanup-Image", "/AnalyzeComponentStore"]),
        Task::DismClean => command("dism.exe", &["/Online", "/Cleanup-Image", "/StartComponentCleanup"]),
        Task::WindowsUpdateClean => windows_update_clean(),
        Task::RestoreComputer { sequence } => command(
            "powershell",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &format!("Restore-Computer -RestorePoint {sequence} -Confirm:$false"),
            ],
        ),
        Task::DeleteTree { paths } => delete_tree(paths),
        Task::EmptyRecycle => command(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", "Clear-RecycleBin -Force -ErrorAction SilentlyContinue"],
        ),
        Task::Powershell { script } => command(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script],
        ),
    }
}

fn restore_point(description: &str) -> TaskResult {
    let desc = ps_quote(description);
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
$k = 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\SystemRestore'
$prev = $null
$had = $false
try {{
  $prev = (Get-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -ErrorAction Stop).SystemRestorePointCreationFrequency
  $had = $true
}} catch {{}}
New-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -Value 0 -PropertyType DWord -Force | Out-Null
try {{
  Checkpoint-Computer -Description {desc} -RestorePointType MODIFY_SETTINGS
  'ok'
}} finally {{
  if ($had) {{
    New-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -Value $prev -PropertyType DWord -Force | Out-Null
  }} else {{
    Remove-ItemProperty -Path $k -Name SystemRestorePointCreationFrequency -ErrorAction SilentlyContinue
  }}
}}
"#
    );
    command("powershell", &["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", &script])
}

fn windows_update_clean() -> TaskResult {
    let _ = command("net", &["stop", "wuauserv"]);
    let _ = command("net", &["stop", "bits"]);
    let path = PathBuf::from(r"C:\Windows\SoftwareDistribution\Download");
    let report = if path.exists() {
        bob_core::purge_paths(&[path])
    } else {
        bob_core::PurgeReport {
            bytes: 0,
            files: 0,
            skipped: 0,
        }
    };
    let _ = command("net", &["start", "bits"]);
    let _ = command("net", &["start", "wuauserv"]);
    TaskResult::ok_msg(
        format!("limpeza do windows update: {} arquivos", report.files),
        format!("{} bytes, {} ignorados", report.bytes, report.skipped),
    )
}

fn delete_tree(paths: &[String]) -> TaskResult {
    let mut files = 0u64;
    let mut skipped = 0u64;
    let mut bytes = 0u64;
    for path in paths {
        let path = Path::new(path);
        if bob_core::protected_path(path) {
            skipped += 1;
            continue;
        }
        let report = bob_core::purge_paths(&[path]);
        files += report.files;
        skipped += report.skipped;
        bytes += report.bytes;
        if path.is_dir() {
            let _ = std::fs::remove_dir_all(path);
        }
    }
    TaskResult::ok_msg("pastas apagadas", format!("{files} arquivos, {bytes} bytes, {skipped} ignorados"))
}

fn service(name: &str, start: &str) -> TaskResult {
    if !matches!(start, "auto" | "demand" | "disabled") || !safe_token(name) {
        return TaskResult::fail("serviço recusado");
    }
    if start == "disabled" {
        let _ = command("sc.exe", &["stop", name]);
    }
    let configured = command("sc.exe", &["config", name, &format!("start= {start}")]);
    if start == "auto" || start == "demand" {
        let _ = command("sc.exe", &["start", name]);
    }
    configured
}

fn scheduled_task(name: &str, enabled: bool) -> TaskResult {
    if name.contains('"') || name.contains('\n') {
        return TaskResult::fail("tarefa recusada");
    }
    let flag = if enabled { "/ENABLE" } else { "/DISABLE" };
    command("schtasks", &["/Change", "/TN", name, flag])
}

pub fn apply_user(ops: &[RegOp]) -> Result<(), String> {
    let result = apply_ops(ops);
    if result.ok {
        Ok(())
    } else {
        Err(result.message)
    }
}

fn apply_ops(ops: &[RegOp]) -> TaskResult {
    let mut errors = Vec::new();
    for op in ops {
        if let Err(err) = apply_one(op) {
            errors.push(err);
        }
    }
    if errors.is_empty() {
        TaskResult::ok_msg("registro atualizado", "")
    } else {
        TaskResult::fail(errors.join("; "))
    }
}

#[cfg(windows)]
fn apply_one(op: &RegOp) -> Result<(), String> {
    use winreg::enums::*;
    let root = hive_key(&op.hive)?;
    if op.delete {
        let key = root.open_subkey_with_flags(&op.path, KEY_SET_VALUE | KEY_WOW64_64KEY).map_err(|err| err.to_string())?;
        match key.delete_value(&op.name) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.to_string()),
        }
    } else {
        let (key, _) = root
            .create_subkey_with_flags(&op.path, KEY_SET_VALUE | KEY_WOW64_64KEY)
            .map_err(|err| err.to_string())?;
        if let Some(value) = op.dword {
            key.set_value(&op.name, &value).map_err(|err| err.to_string())
        } else if let Some(value) = &op.sz {
            key.set_value(&op.name, value).map_err(|err| err.to_string())
        } else {
            Err("operação vazia".into())
        }
    }
}

#[cfg(not(windows))]
fn apply_one(op: &RegOp) -> Result<(), String> {
    let _ = op;
    Err("registro do windows indisponível".into())
}

#[cfg(windows)]
pub fn read_reg(hive: &str, path: &str, name: &str) -> Result<Option<crate::state::UndoOp>, String> {
    use winreg::enums::*;
    let root = hive_key(hive)?;
    let key = match root.open_subkey_with_flags(path, KEY_READ | KEY_WOW64_64KEY) {
        Ok(key) => key,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Some(crate::state::UndoOp::Reg {
                hive: hive.into(),
                path: path.into(),
                name: name.into(),
                existed: false,
                dword: None,
                sz: None,
            }));
        }
        Err(err) => return Err(err.to_string()),
    };
    if let Ok(value) = key.get_value::<u32, _>(name) {
        return Ok(Some(crate::state::UndoOp::Reg {
            hive: hive.into(),
            path: path.into(),
            name: name.into(),
            existed: true,
            dword: Some(value),
            sz: None,
        }));
    }
    if let Ok(value) = key.get_value::<String, _>(name) {
        return Ok(Some(crate::state::UndoOp::Reg {
            hive: hive.into(),
            path: path.into(),
            name: name.into(),
            existed: true,
            dword: None,
            sz: Some(value),
        }));
    }
    Ok(Some(crate::state::UndoOp::Reg {
        hive: hive.into(),
        path: path.into(),
        name: name.into(),
        existed: false,
        dword: None,
        sz: None,
    }))
}

#[cfg(not(windows))]
pub fn read_reg(hive: &str, path: &str, name: &str) -> Result<Option<crate::state::UndoOp>, String> {
    let _ = (hive, path, name);
    Err("registro do windows indisponível".into())
}

#[cfg(windows)]
fn hive_key(hive: &str) -> Result<winreg::RegKey, String> {
    use winreg::enums::*;
    use winreg::RegKey;
    match hive.to_ascii_lowercase().as_str() {
        "hkcu" => Ok(RegKey::predef(HKEY_CURRENT_USER)),
        "hklm" => Ok(RegKey::predef(HKEY_LOCAL_MACHINE)),
        _ => Err("hive desconhecida".into()),
    }
}

#[cfg(windows)]
pub fn enum_run(hive: &str, path: &str) -> Result<Vec<(String, String)>, String> {
    use winreg::enums::*;
    let root = hive_key(hive)?;
    let key = match root.open_subkey_with_flags(path, KEY_READ | KEY_WOW64_64KEY) {
        Ok(key) => key,
        Err(_) => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    for item in key.enum_values() {
        let Ok((name, value)) = item else { continue };
        out.push((name, value.to_string()));
    }
    Ok(out)
}

#[cfg(not(windows))]
pub fn enum_run(hive: &str, path: &str) -> Result<Vec<(String, String)>, String> {
    let _ = (hive, path);
    Ok(Vec::new())
}

fn command(program: &str, args: &[&str]) -> TaskResult {
    match hidden::std_command(program).args(args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
                TaskResult::ok_msg("ok", format!("{stdout}{stderr}"))
            } else {
                let message = format!("{stderr}\n{stdout}").trim().to_string();
                TaskResult::fail(if message.is_empty() {
                    format!("{program} falhou")
                } else {
                    message
                })
            }
        }
        Err(err) => TaskResult::fail(err.to_string()),
    }
}

fn safe_token(value: &str) -> bool {
    !value.is_empty()
        && value.len() < 80
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.')
}

pub fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

pub fn undo_to_op(op: &crate::state::UndoOp) -> Option<RegOp> {
    let crate::state::UndoOp::Reg {
        hive,
        path,
        name,
        existed,
        dword,
        sz,
    } = op
    else {
        return None;
    };
    Some(RegOp {
        hive: hive.clone(),
        path: path.clone(),
        name: name.clone(),
        delete: !existed,
        dword: *dword,
        sz: sz.clone(),
    })
}
