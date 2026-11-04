//! O que o programa pode sugerir. Nada aqui apaga ou atualiza sozinho.

use serde::{Deserialize, Serialize};

pub const JUNK_THRESHOLD: u64 = 500 * 1024 * 1024;
pub const DISK_FULL_RATIO: f64 = 0.90;
pub const HOT_TEMP_C: i64 = 70;

#[derive(Clone, Debug, Default)]
pub struct Signals {
    pub disk_used_ratio: Option<f64>,
    pub disk_free: Option<u64>,
    pub junk_bytes: u64,
    pub upgrade_count: usize,
    pub smart_alert: Option<String>,
    pub temp_c: Option<i64>,
    pub heavy_startup: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub page: String,
    pub tone: String,
}

pub fn build_suggestions(signals: &Signals) -> Vec<Suggestion> {
    let mut out = Vec::new();
    if signals.disk_used_ratio.is_some_and(|ratio| ratio >= DISK_FULL_RATIO) {
        let free = signals.disk_free.unwrap_or(0);
        out.push(Suggestion {
            id: "disk-full".into(),
            title: "disco quase cheio".into(),
            detail: format!("resta cerca de {} livres no volume principal", human_bytes(free)),
            page: "disk".into(),
            tone: "warn".into(),
        });
    }
    if signals.junk_bytes >= JUNK_THRESHOLD {
        out.push(Suggestion {
            id: "junk".into(),
            title: "lixo acumulado".into(),
            detail: format!("{} em temporários e caches, se você quiser limpar", human_bytes(signals.junk_bytes)),
            page: "clean".into(),
            tone: "warn".into(),
        });
    }
    if signals.upgrade_count > 0 {
        let n = signals.upgrade_count;
        out.push(Suggestion {
            id: "upgrades".into(),
            title: "atualizações de apps".into(),
            detail: format!("{n} aplicativo(s) com versão nova no winget"),
            page: "upgrades".into(),
            tone: "info".into(),
        });
    }
    if let Some(alert) = signals.smart_alert.as_ref().filter(|text| !text.is_empty()) {
        out.push(Suggestion {
            id: "smart".into(),
            title: "disco com problema".into(),
            detail: alert.clone(),
            page: "health".into(),
            tone: "warn".into(),
        });
    } else if signals.temp_c.is_some_and(|temp| temp >= HOT_TEMP_C) {
        out.push(Suggestion {
            id: "temp".into(),
            title: "temperatura alta".into(),
            detail: format!("{} °c em um disco", signals.temp_c.unwrap_or(0)),
            page: "health".into(),
            tone: "warn".into(),
        });
    }
    if signals.heavy_startup > 0 {
        out.push(Suggestion {
            id: "startup".into(),
            title: "inicialização pesada".into(),
            detail: format!("{} item(ns) com impacto alto ao ligar o windows", signals.heavy_startup),
            page: "startup".into(),
            tone: "info".into(),
        });
    }
    out
}

pub fn disk_almost_full(ratio: f64) -> bool {
    ratio >= DISK_FULL_RATIO
}

pub fn junk_worth_mentioning(bytes: u64) -> bool {
    bytes >= JUNK_THRESHOLD
}

pub fn temp_or_smart(temp_c: Option<i64>, alert: bool) -> bool {
    alert || temp_c.is_some_and(|temp| temp >= HOT_TEMP_C)
}

fn human_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["b", "kb", "mb", "gb"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_only_above_thresholds() {
        let quiet = build_suggestions(&Signals {
            disk_used_ratio: Some(0.5),
            junk_bytes: 10_000,
            upgrade_count: 0,
            temp_c: Some(40),
            ..Signals::default()
        });
        assert!(quiet.is_empty());

        let loud = build_suggestions(&Signals {
            disk_used_ratio: Some(0.95),
            disk_free: Some(2 * 1024 * 1024 * 1024),
            junk_bytes: JUNK_THRESHOLD + 1,
            upgrade_count: 3,
            smart_alert: Some("o smart reprovou este disco".into()),
            temp_c: Some(90),
            heavy_startup: 2,
        });
        let ids: Vec<_> = loud.iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["disk-full", "junk", "upgrades", "smart", "startup"]);
        assert!(disk_almost_full(0.91));
        assert!(!disk_almost_full(0.2));
        assert!(junk_worth_mentioning(JUNK_THRESHOLD));
        assert!(temp_or_smart(Some(70), false));
        assert!(temp_or_smart(None, true));
        assert!(!temp_or_smart(Some(40), false));
    }
}
