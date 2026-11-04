use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskHealth {
    pub name: String,
    pub media: String,
    pub health: String,
    pub temp_c: Option<i64>,
    pub hours: Option<i64>,
    pub wear: Option<i64>,
    pub tbw_gb: Option<f64>,
    pub read_errors: Option<i64>,
    pub write_errors: Option<i64>,
    pub smart_ok: Option<bool>,
    pub size: Option<u64>,
    pub source: String,
    pub alert: Option<String>,
}

pub fn assess(disk: &DiskHealth) -> Option<String> {
    let health = disk.health.to_lowercase();
    if health.contains("unhealthy")
        || health.contains("warning")
        || health.contains("fail")
        || health.contains("bad")
        || health.contains("crítico")
        || health.contains("critico")
    {
        return Some("este disco não está saudável".into());
    }
    if disk.smart_ok == Some(false) {
        return Some("o smart reprovou este disco".into());
    }
    if disk.wear.unwrap_or(0) >= 90 {
        return Some("desgaste alto".into());
    }
    if disk.temp_c.unwrap_or(0) >= 70 {
        return Some("temperatura alta".into());
    }
    None
}

pub fn parse_storage_json(text: &str) -> Vec<DiskHealth> {
    let Some(value) = parse_json_slice(text) else {
        return Vec::new();
    };
    let items = match value {
        Value::Array(items) => items,
        Value::Object(_) => vec![value],
        _ => return Vec::new(),
    };
    items
        .into_iter()
        .filter_map(|item| {
            let name = string(&item, &["name", "friendlyName", "FriendlyName"])?;
            let mut disk = DiskHealth {
                name,
                media: string(&item, &["media", "MediaType"]).unwrap_or_else(|| "disco".into()),
                health: string(&item, &["health", "HealthStatus"]).unwrap_or_else(|| "desconhecido".into()),
                temp_c: number(&item, &["temp", "temperature", "Temperature"]).map(|n| n as i64),
                hours: number(&item, &["hours", "powerOnHours", "PowerOnHours"]).map(|n| n as i64),
                wear: number(&item, &["wear", "Wear"]).map(|n| n as i64),
                tbw_gb: number(&item, &["tbwGb", "tbw"]),
                read_errors: number(&item, &["readErrors", "ReadErrorsTotal"]).map(|n| n as i64),
                write_errors: number(&item, &["writeErrors", "WriteErrorsTotal"]).map(|n| n as i64),
                smart_ok: None,
                size: number(&item, &["size", "Size"]).map(|n| n as u64),
                source: "msft".into(),
                alert: None,
            };
            disk.alert = assess(&disk);
            Some(disk)
        })
        .collect()
}

pub fn parse_smartctl(text: &str) -> Option<DiskHealth> {
    let value = parse_json_slice(text)?;
    let name = string(&value, &["model_name", "model"])
        .or_else(|| value.pointer("/device/name").and_then(|v| v.as_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| "disco".into());
    let temp = value
        .pointer("/temperature/current")
        .and_then(|v| v.as_i64())
        .or_else(|| number(&value, &["temperature"]).map(|n| n as i64));
    let hours = value.pointer("/power_on_time/hours").and_then(|v| v.as_i64());
    let smart_ok = value.pointer("/smart_status/passed").and_then(|v| v.as_bool());
    let nvme = value.get("nvme_smart_health_information_log");
    let wear = nvme
        .and_then(|v| v.get("percentage_used"))
        .and_then(|v| v.as_i64())
        .or_else(|| {
            value
                .pointer("/ata_smart_attributes/table")
                .and_then(|v| v.as_array())
                .and_then(|rows| {
                    rows.iter().find_map(|row| {
                        let id = row.get("id").and_then(|v| v.as_i64());
                        if id == Some(177) || id == Some(231) || id == Some(233) {
                            row.get("value").and_then(|v| v.as_i64())
                        } else {
                            None
                        }
                    })
                })
        });
    let tbw_gb = nvme.and_then(|v| v.get("data_units_written")).and_then(|v| v.as_f64()).map(|units| units * 512_000.0 / 1_000_000_000.0);
    let media = if nvme.is_some() { "ssd" } else { "disco" };
    let mut disk = DiskHealth {
        name,
        media: media.into(),
        health: if smart_ok == Some(false) {
            "reprovado".into()
        } else if smart_ok == Some(true) {
            "saudável".into()
        } else {
            "desconhecido".into()
        },
        temp_c: temp,
        hours,
        wear,
        tbw_gb,
        read_errors: nvme.and_then(|v| v.get("media_errors")).and_then(|v| v.as_i64()),
        write_errors: None,
        smart_ok,
        size: number(&value, &["user_capacity"]).map(|n| n as u64).or_else(|| {
            value.pointer("/user_capacity/bytes").and_then(|v| v.as_u64())
        }),
        source: "smartctl".into(),
        alert: None,
    };
    disk.alert = assess(&disk);
    Some(disk)
}

fn parse_json_slice(text: &str) -> Option<Value> {
    let start = text.find(['{', '['])?;
    serde_json::from_str::<Value>(text[start..].trim()).ok()
}

fn string(value: &Value, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value.get(*key).and_then(|v| v.as_str()) {
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }
    None
}

fn number(value: &Value, keys: &[&str]) -> Option<f64> {
    for key in keys {
        if let Some(item) = value.get(*key) {
            if let Some(n) = item.as_f64() {
                return Some(n);
            }
            if let Some(n) = item.as_i64() {
                return Some(n as f64);
            }
            if let Some(text) = item.as_str() {
                if let Ok(n) = text.trim().replace(',', ".").parse::<f64>() {
                    return Some(n);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_storage_and_smartctl() {
        let storage = r#"[{"name":"Samsung SSD 980","media":"SSD","health":"Healthy","temp":41,"hours":1200,"wear":6}]"#;
        let disks = parse_storage_json(storage);
        assert_eq!(disks[0].name, "Samsung SSD 980");
        assert!(disks[0].alert.is_none());

        let bad = r#"[{"name":"Velho","media":"HDD","health":"Warning","temp":72,"hours":10}]"#;
        let disks = parse_storage_json(bad);
        assert!(disks[0].alert.is_some());

        let smart = r#"{"model_name":"WD Blue","smart_status":{"passed":true},"temperature":{"current":36},"power_on_time":{"hours":400},"nvme_smart_health_information_log":{"percentage_used":2,"data_units_written":1000}}"#;
        let disk = parse_smartctl(smart).unwrap();
        assert_eq!(disk.wear, Some(2));
        assert!(disk.tbw_gb.unwrap() > 0.0);
        assert!(disk.alert.is_none());
    }
}
