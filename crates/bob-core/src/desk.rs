//! Peças puras do 0.4.0: pacote, drivers, rede, energia, relatório e agenda.
//! Nada aqui abre processo nem muda o windows.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BundleApp {
    pub source: String,
    pub id: String,
    pub name: String,
    pub version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BundleCollection {
    pub id: String,
    pub name: String,
    pub apps: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PcBundle {
    pub format: u32,
    pub exported_ms: i64,
    pub apps: Vec<BundleApp>,
    pub favorites: Vec<String>,
    pub collections: Vec<BundleCollection>,
    pub theme: String,
    pub game_apps: Vec<String>,
}

pub fn bundle_to_json(bundle: &PcBundle) -> String {
    serde_json::to_string_pretty(bundle).unwrap_or_else(|_| "{}".into())
}

pub fn bundle_from_json(text: &str) -> Result<PcBundle, String> {
    let bundle: PcBundle = serde_json::from_str(text).map_err(|err| err.to_string())?;
    if bundle.format == 0 {
        return Err("pacote sem versão".into());
    }
    Ok(bundle)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DriverInfo {
    pub device: String,
    pub class_name: String,
    pub version: String,
    pub date: String,
    pub old: bool,
    pub suggestion: String,
    pub channel: String,
}

pub fn parse_driver_table(text: &str, now_ms: i64) -> Vec<DriverInfo> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.to_lowercase().starts_with("devicename") || line.starts_with("Name|") {
            continue;
        }
        let parts = split_row(line);
        if parts.len() < 3 {
            continue;
        }
        let device = parts[0].trim().trim_matches('"').to_string();
        if device.is_empty() {
            continue;
        }
        let class_name = parts.get(1).map(|s| s.trim().trim_matches('"').to_string()).unwrap_or_default();
        let version = parts.get(2).map(|s| s.trim().trim_matches('"').to_string()).unwrap_or_default();
        let date = normalize_date(parts.get(3).map(|s| s.trim().trim_matches('"')).unwrap_or(""));
        let (suggestion, channel) = driver_hint(&device, &class_name);
        let old = date_is_old(&date, now_ms, 18);
        out.push(DriverInfo {
            device,
            class_name,
            version,
            date,
            old,
            suggestion,
            channel,
        });
    }
    out.sort_by(|a, b| b.old.cmp(&a.old).then(a.device.cmp(&b.device)));
    out
}

fn split_row(line: &str) -> Vec<String> {
    if line.contains('|') {
        return line.split('|').map(|part| part.trim().to_string()).collect();
    }
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for ch in line.chars() {
        match ch {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                out.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        out.push(current.trim().to_string());
    }
    out
}

pub fn driver_hint(device: &str, class_name: &str) -> (String, String) {
    let text = format!("{device} {class_name}").to_lowercase();
    if text.contains("nvidia") {
        return ("o app da nvidia está no catálogo. não instalo sozinho.".into(), "winget".into());
    }
    if text.contains("amd") || text.contains("radeon") {
        return ("o software da amd está no catálogo. não instalo sozinho.".into(), "winget".into());
    }
    if text.contains("intel") && (text.contains("arc") || text.contains("display") || text.contains("graphics") || class_name.eq_ignore_ascii_case("display")) {
        return ("o app da intel está no catálogo. não instalo sozinho.".into(), "winget".into());
    }
    if class_name.eq_ignore_ascii_case("display") || class_name.eq_ignore_ascii_case("net") || class_name.eq_ignore_ascii_case("media") {
        return ("vale olhar as atualizações opcionais do windows update.".into(), "windows".into());
    }
    ("o fabricante publica a página do driver. eu só aponto.".into(), "vendor".into())
}

fn normalize_date(raw: &str) -> String {
    let digits: String = raw.chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.len() >= 8 {
        return format!("{}-{}-{}", &digits[0..4], &digits[4..6], &digits[6..8]);
    }
    raw.trim().to_string()
}

fn date_is_old(date: &str, now_ms: i64, months: i64) -> bool {
    let Some(ms) = date_ms(date) else {
        return false;
    };
    now_ms.saturating_sub(ms) > months * 30 * 86_400_000
}

fn date_ms(date: &str) -> Option<i64> {
    let mut parts = date.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: i64 = parts.next()?.parse().ok()?;
    let day: i64 = parts.next()?.parse().ok()?;
    if !(1990..=2100).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let days = (year - 1970) * 365 + (year - 1969) / 4 + (month - 1) * 30 + (day - 1);
    Some(days * 86_400_000)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DnsPreset {
    pub id: String,
    pub label: String,
    pub servers: Vec<String>,
}

pub fn dns_presets() -> Vec<DnsPreset> {
    vec![
        DnsPreset { id: "cloudflare".into(), label: "cloudflare".into(), servers: vec!["1.1.1.1".into(), "1.0.0.1".into()] },
        DnsPreset { id: "google".into(), label: "google".into(), servers: vec!["8.8.8.8".into(), "8.8.4.4".into()] },
        DnsPreset { id: "padrao".into(), label: "padrão".into(), servers: Vec::new() },
    ]
}

pub fn parse_ping(text: &str) -> Option<f64> {
    for token in ["time=", "tempo=", "time<", "tempo<"] {
        if let Some(pos) = text.to_lowercase().find(token) {
            let rest = &text[pos + token.len()..];
            let number: String = rest.chars().take_while(|ch| ch.is_ascii_digit() || *ch == '.' || *ch == ',').collect();
            let number = number.replace(',', ".");
            if let Ok(ms) = number.parse::<f64>() {
                return Some(if token.ends_with('<') { ms.max(1.0) } else { ms });
            }
        }
    }
    None
}

pub fn mbps(bytes: u64, seconds: f64) -> f64 {
    if seconds <= 0.05 {
        return 0.0;
    }
    (bytes as f64 * 8.0) / seconds / 1_000_000.0
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PowerPlan {
    pub guid: String,
    pub name: String,
    pub active: bool,
}

pub fn parse_power_plans(text: &str) -> Vec<PowerPlan> {
    let mut out = Vec::new();
    for line in text.lines() {
        let lower = line.to_lowercase();
        let Some(pos) = lower.find("guid") else {
            continue;
        };
        let rest = &line[pos + 4..];
        let guid: String = rest.chars().skip_while(|ch| !ch.is_ascii_hexdigit()).take(36).collect();
        if guid.len() < 36 {
            continue;
        }
        let name = line
            .split('(')
            .nth(1)
            .and_then(|part| part.split(')').next())
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() {
            continue;
        }
        out.push(PowerPlan {
            guid: guid.to_lowercase(),
            name,
            active: line.contains('*'),
        });
    }
    out
}

pub const HIGH_PERFORMANCE: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BatteryReport {
    pub design_mwh: u64,
    pub full_mwh: u64,
    pub cycles: u32,
    pub health: f64,
}

pub fn parse_battery_html(html: &str) -> Option<BatteryReport> {
    let design = labeled_number(html, &["DESIGN CAPACITY", "CAPACIDADE DE PROJETO"])?;
    let full = labeled_number(html, &["FULL CHARGE CAPACITY", "CAPACIDADE DE CARGA COMPLETA"]).unwrap_or(design);
    let cycles = labeled_number(html, &["CYCLE COUNT", "CONTAGEM DE CICLOS"]).unwrap_or(0) as u32;
    let health = if design == 0 { 0.0 } else { (full as f64 / design as f64) * 100.0 };
    Some(BatteryReport { design_mwh: design, full_mwh: full, cycles, health })
}

fn labeled_number(text: &str, labels: &[&str]) -> Option<u64> {
    let upper = text.to_uppercase();
    for label in labels {
        if let Some(pos) = upper.find(label) {
            let rest = &text[pos + label.len()..];
            let digits: String = rest.chars().skip_while(|ch| !ch.is_ascii_digit()).take_while(|ch| ch.is_ascii_digit() || *ch == ',' || *ch == '.' || *ch == ' ').take(16).collect();
            let digits: String = digits.chars().filter(|ch| ch.is_ascii_digit()).collect();
            if let Ok(value) = digits.parse::<u64>() {
                return Some(value);
            }
        }
    }
    None
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcSummary {
    pub computer: String,
    pub windows: String,
    pub cpu: String,
    pub ram: String,
    pub gpu: String,
    pub disks: Vec<String>,
    pub health: Vec<String>,
    pub temp: String,
    pub space: String,
    pub apps: u32,
}

pub fn render_report_html(summary: &PcSummary) -> String {
    let mut body = String::new();
    body.push_str("<article><h1>relatório do pc</h1>");
    push_row(&mut body, "computador", &summary.computer);
    push_row(&mut body, "windows", &summary.windows);
    push_row(&mut body, "cpu", &summary.cpu);
    push_row(&mut body, "memória", &summary.ram);
    push_row(&mut body, "placa", &summary.gpu);
    push_row(&mut body, "espaço", &summary.space);
    push_row(&mut body, "temperatura", &summary.temp);
    push_row(&mut body, "apps", &summary.apps.to_string());
    body.push_str("<h2>discos</h2><ul>");
    for disk in summary.disks.iter().chain(summary.health.iter()) {
        body.push_str("<li>");
        body.push_str(&escape(disk));
        body.push_str("</li>");
    }
    body.push_str("</ul></article>");
    body
}

fn push_row(body: &mut String, label: &str, value: &str) {
    body.push_str("<p><strong>");
    body.push_str(label);
    body.push_str("</strong> ");
    body.push_str(&escape(value));
    body.push_str("</p>");
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

pub fn simple_pdf(title: &str, lines: &[String]) -> Vec<u8> {
    let mut stream = String::from("BT /F1 16 Tf 48 800 Td (");
    stream.push_str(&pdf_escape(title));
    stream.push_str(") Tj 0 -28 Td /F1 11 Tf ");
    for line in lines.iter().take(40) {
        stream.push_str("(");
        stream.push_str(&pdf_escape(line));
        stream.push_str(") Tj 0 -16 Td ");
    }
    stream.push_str("ET");
    let objects = [
        "1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n".to_string(),
        "2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n".to_string(),
        "3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 595 842] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >> endobj\n".to_string(),
        format!("4 0 obj << /Length {} >> stream\n{stream}\nendstream endobj\n", stream.len()),
        "5 0 obj << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> endobj\n".to_string(),
    ];
    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = Vec::new();
    for object in &objects {
        offsets.push(out.len());
        out.push_str(object);
    }
    let xref = out.len();
    out.push_str("xref\n0 6\n0000000000 65535 f \n");
    for offset in offsets {
        out.push_str(&format!("{offset:010} 00000 n \n"));
    }
    out.push_str(&format!("trailer << /Size 6 /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF"));
    out.into_bytes()
}

fn pdf_escape(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            '(' | ')' | '\\' => ' ',
            ch if ch.is_ascii() => ch,
            _ => ' ',
        })
        .collect()
}

pub fn schedule_due(enabled: bool, every_days: u32, last_ms: i64, now_ms: i64) -> bool {
    if !enabled || every_days == 0 || last_ms <= 0 {
        return false;
    }
    now_ms.saturating_sub(last_ms) >= i64::from(every_days) * 86_400_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_roundtrip_and_drivers_and_power() {
        let bundle = PcBundle {
            format: 1,
            exported_ms: 10,
            apps: vec![BundleApp { source: "winget".into(), id: "VideoLAN.VLC".into(), name: "vlc".into(), version: "3".into() }],
            favorites: vec!["wg:gimp".into()],
            collections: vec![BundleCollection { id: "dev".into(), name: "dev".into(), apps: vec!["git".into()] }],
            theme: "dark".into(),
            game_apps: vec!["discord".into()],
        };
        let parsed = bundle_from_json(&bundle_to_json(&bundle)).unwrap();
        assert_eq!(parsed.apps[0].id, "VideoLAN.VLC");
        let drivers = parse_driver_table(
            "NVIDIA GeForce RTX 4060|Display|32.0.15.6094|20230101000000.000000-000\nRealtek Audio|MEDIA|6.0.1.1|2026-01-02\n",
            1_735_689_600_000,
        );
        assert!(drivers[0].old);
        assert_eq!(drivers[0].channel, "winget");
        assert!(!drivers.iter().find(|d| d.device.contains("Realtek")).unwrap().old);
        let plans = parse_power_plans("Power Scheme GUID: 8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c  (Alto desempenho) *\n");
        assert!(plans[0].active);
        assert_eq!(plans[0].guid, HIGH_PERFORMANCE);
        let battery = parse_battery_html("<td>DESIGN CAPACITY</td><td>48,000 mWh</td><td>FULL CHARGE CAPACITY</td><td>44,100 mWh</td><td>CYCLE COUNT</td><td>120</td>").unwrap();
        assert_eq!(battery.design_mwh, 48000);
        assert_eq!(battery.cycles, 120);
        assert!(battery.health < 100.0);
        assert_eq!(parse_ping("Reply from 1.1.1.1: time=12ms").unwrap(), 12.0);
        assert!(mbps(1_000_000, 1.0) > 7.0);
        assert!(!schedule_due(false, 7, 1, 10_000_000_000));
        assert!(schedule_due(true, 7, 1, 8 * 86_400_000));
        let html = render_report_html(&PcSummary {
            computer: "ana".into(),
            windows: "11".into(),
            cpu: "ryzen".into(),
            ram: "16 gb".into(),
            gpu: "rtx".into(),
            disks: vec!["C: 200 gb".into()],
            health: vec!["ssd ok".into()],
            temp: "41".into(),
            space: "200 gb livres".into(),
            apps: 40,
        });
        assert!(html.contains("relatório"));
        assert!(simple_pdf("relatorio", &["cpu ryzen".into()]).starts_with(b"%PDF"));
        assert!(dns_presets().iter().any(|preset| preset.id == "padrao" && preset.servers.is_empty()));
    }
}
