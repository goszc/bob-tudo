use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub cpu: f32,
    pub ram: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorSample {
    pub cpu: f32,
    pub ram_used: u64,
    pub ram_total: u64,
    pub disk_used: u64,
    pub disk_total: u64,
    pub disk_bps: f64,
    pub gpu: Option<f32>,
    pub gpu_name: String,
    pub net_down: f64,
    pub net_up: f64,
    pub temp_c: Option<f32>,
    pub processes: Vec<ProcInfo>,
}

struct Keeper {
    sys: System,
    last_rx: u64,
    last_tx: u64,
    last_disk: u64,
    last_at: Instant,
    primed: bool,
    extra_at: Instant,
    gpu: Option<f32>,
    gpu_name: String,
    temp_c: Option<f32>,
    disk_bps: f64,
}

static SAMPLE: Mutex<Option<MonitorSample>> = Mutex::new(None);

pub fn start() {
    std::thread::spawn(|| {
        let mut keeper = Keeper {
            sys: System::new_all(),
            last_rx: 0,
            last_tx: 0,
            last_disk: 0,
            last_at: Instant::now(),
            primed: false,
            extra_at: Instant::now() - Duration::from_secs(30),
            gpu: None,
            gpu_name: String::new(),
            temp_c: None,
            disk_bps: 0.0,
        };
        loop {
            if let Some(sample) = sample_once(&mut keeper) {
                *SAMPLE.lock().unwrap_or_else(|err| err.into_inner()) = Some(sample);
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    });
}

fn sample_once(keeper: &mut Keeper) -> Option<MonitorSample> {
    keeper.sys.refresh_cpu_usage();
    keeper.sys.refresh_memory();
    if !keeper.primed {
        std::thread::sleep(Duration::from_millis(240));
        keeper.sys.refresh_cpu_usage();
        keeper.primed = true;
    }
    keeper.sys.refresh_processes(ProcessesToUpdate::All, true);
    let cpu = keeper.sys.global_cpu_usage();
    let ram_total = keeper.sys.total_memory();
    let ram_used = keeper.sys.used_memory();
    let (disk_total, disk_used) = disk_space();
    let (rx, tx) = net_totals();
    let elapsed = keeper.last_at.elapsed().as_secs_f64().max(0.2);
    let net_down = if keeper.last_rx > 0 { rx.saturating_sub(keeper.last_rx) as f64 / elapsed } else { 0.0 };
    let net_up = if keeper.last_tx > 0 { tx.saturating_sub(keeper.last_tx) as f64 / elapsed } else { 0.0 };
    keeper.last_rx = rx;
    keeper.last_tx = tx;
    let disk_now = disk_bytes();
    if keeper.last_disk > 0 {
        keeper.disk_bps = disk_now.saturating_sub(keeper.last_disk) as f64 / elapsed;
    }
    keeper.last_disk = disk_now;
    keeper.last_at = Instant::now();
    if keeper.extra_at.elapsed() > Duration::from_secs(4) {
        if let Some((gpu, temp, name)) = nvidia() {
            keeper.gpu = Some(gpu);
            keeper.temp_c = Some(temp);
            keeper.gpu_name = name;
        }
        if let Some(extra) = platform_extra() {
            if keeper.gpu.is_none() {
                keeper.gpu = extra.gpu;
            }
            if keeper.temp_c.is_none() {
                keeper.temp_c = extra.temp_c;
            }
            if keeper.gpu_name.is_empty() {
                keeper.gpu_name = extra.gpu_name;
            }
            if extra.disk_bps > 0.0 {
                keeper.disk_bps = extra.disk_bps;
            }
        }
        keeper.extra_at = Instant::now();
    }
    let mut processes: Vec<ProcInfo> = keeper
        .sys
        .processes()
        .values()
        .map(|proc_| ProcInfo {
            pid: proc_.pid().as_u32(),
            name: proc_.name().to_string_lossy().to_string(),
            cpu: proc_.cpu_usage(),
            ram: proc_.memory(),
        })
        .collect();
    processes.sort_by(|a, b| b.cpu.partial_cmp(&a.cpu).unwrap_or(std::cmp::Ordering::Equal));
    processes.truncate(12);
    Some(MonitorSample {
        cpu,
        ram_used,
        ram_total,
        disk_used,
        disk_total,
        disk_bps: keeper.disk_bps,
        gpu: keeper.gpu,
        gpu_name: keeper.gpu_name.clone(),
        net_down,
        net_up,
        temp_c: keeper.temp_c,
        processes,
    })
}

#[tauri::command]
pub fn monitor_sample() -> MonitorSample {
    SAMPLE
        .lock()
        .unwrap_or_else(|err| err.into_inner())
        .clone()
        .unwrap_or(MonitorSample {
            cpu: 0.0,
            ram_used: 0,
            ram_total: 0,
            disk_used: 0,
            disk_total: 0,
            disk_bps: 0.0,
            gpu: None,
            gpu_name: String::new(),
            net_down: 0.0,
            net_up: 0.0,
            temp_c: None,
            processes: Vec::new(),
        })
}

#[tauri::command]
pub fn kill_process(pid: u32) -> Result<(), String> {
    if pid == 0 || pid == std::process::id() {
        return Err("não encerro este processo".into());
    }
    let mut sys = System::new();
    let id = Pid::from_u32(pid);
    sys.refresh_processes(ProcessesToUpdate::Some(&[id]), true);
    let proc_ = sys.process(id).ok_or("processo não encontrado")?;
    if proc_.kill() {
        Ok(())
    } else {
        Err("não consegui encerrar".into())
    }
}

fn disk_space() -> (u64, u64) {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut total = 0u64;
    let mut used = 0u64;
    for disk in disks.list() {
        let whole = disk.total_space();
        let free = disk.available_space();
        total = total.saturating_add(whole);
        used = used.saturating_add(whole.saturating_sub(free));
    }
    (total, used)
}

fn net_totals() -> (u64, u64) {
    let nets = sysinfo::Networks::new_with_refreshed_list();
    let mut rx = 0u64;
    let mut tx = 0u64;
    for (_name, data) in &nets {
        rx = rx.saturating_add(data.total_received());
        tx = tx.saturating_add(data.total_transmitted());
    }
    (rx, tx)
}

fn disk_bytes() -> u64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(text) = std::fs::read_to_string("/proc/diskstats") {
            let (read, write) = diskstats_bytes(&text);
            return read.saturating_add(write);
        }
    }
    0
}

struct Extra {
    gpu: Option<f32>,
    gpu_name: String,
    temp_c: Option<f32>,
    disk_bps: f64,
}

fn platform_extra() -> Option<Extra> {
    #[cfg(windows)]
    {
        let script = r#"
$disk = $null; $gpu = $null; $temp = $null; $name = $null
try { $c = Get-Counter '\PhysicalDisk(_Total)\Disk Bytes/sec' -ErrorAction Stop; $disk = ($c.CounterSamples | Measure-Object CookedValue -Sum).Sum } catch {}
try { $g = Get-Counter '\GPU Engine(*engtype_3D)\Utilization Percentage' -ErrorAction Stop; $gpu = ($g.CounterSamples | Measure-Object CookedValue -Sum).Sum; if ($gpu -gt 100) { $gpu = 100 } } catch {}
try { $t = Get-CimInstance -Namespace root/wmi -ClassName MSAcpi_ThermalZoneTemperature -ErrorAction Stop | Select-Object -First 1; if ($t) { $temp = [math]::Round(($t.CurrentTemperature / 10) - 273.15, 1) } } catch {}
try { $name = (Get-CimInstance Win32_VideoController | Select-Object -First 1 -ExpandProperty Name) } catch {}
@{ diskBps = $disk; gpu = $gpu; tempC = $temp; gpuName = $name } | ConvertTo-Json -Compress
"#;
        let output = crate::hidden::std_command("powershell")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        let value: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
        return Some(Extra {
            gpu: value.get("gpu").and_then(|v| v.as_f64()).map(|n| n as f32),
            gpu_name: value.get("gpuName").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            temp_c: value.get("tempC").and_then(|v| v.as_f64()).map(|n| n as f32),
            disk_bps: value.get("diskBps").and_then(|v| v.as_f64()).unwrap_or(0.0),
        });
    }
    #[cfg(target_os = "linux")]
    {
        let temp = std::fs::read_dir("/sys/class/thermal")
            .ok()?
            .flatten()
            .find_map(|entry| {
                let text = std::fs::read_to_string(entry.path().join("temp")).ok()?;
                let milli: f32 = text.trim().parse().ok()?;
                Some(milli / 1000.0)
            });
        let gpu = std::fs::read_to_string("/sys/class/drm/card0/device/gpu_busy_percent")
            .ok()
            .and_then(|text| text.trim().parse().ok());
        return Some(Extra {
            gpu,
            gpu_name: String::new(),
            temp_c: temp,
            disk_bps: 0.0,
        });
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        None
    }
}

fn nvidia() -> Option<(f32, f32, String)> {
    let output = crate::hidden::std_command("nvidia-smi")
        .args(["--query-gpu=utilization.gpu,temperature.gpu,name", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&output.stdout);
    let mut parts = line.split(',');
    let gpu = parts.next()?.trim().parse().ok()?;
    let temp = parts.next()?.trim().parse().ok()?;
    let name = parts.next()?.trim().to_string();
    Some((gpu, temp, name))
}

#[cfg(target_os = "linux")]
fn diskstats_bytes(text: &str) -> (u64, u64) {
    let mut read = 0u64;
    let mut write = 0u64;
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 || !whole_disk(parts[2]) {
            continue;
        }
        read = read.saturating_add(parts[5].parse::<u64>().unwrap_or(0));
        write = write.saturating_add(parts[9].parse::<u64>().unwrap_or(0));
    }
    (read.saturating_mul(512), write.saturating_mul(512))
}

#[cfg(target_os = "linux")]
fn whole_disk(name: &str) -> bool {
    if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("dm-") {
        return false;
    }
    if let Some(rest) = name.strip_prefix("nvme") {
        return !rest.contains('p');
    }
    if name.starts_with("mmcblk") {
        return !name.contains('p');
    }
    !name.chars().last().is_some_and(|ch| ch.is_ascii_digit())
}
