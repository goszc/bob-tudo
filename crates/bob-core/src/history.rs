//! Série local e leve. Só existem pontos enquanto o programa está aberto.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Point {
    pub ts: i64,
    pub value: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Grew {
    pub key: String,
    pub before: f64,
    pub after: f64,
}

pub struct HistoryStore {
    conn: Connection,
}

static HISTORY: Mutex<Option<HistoryStore>> = Mutex::new(None);

pub fn open_history(path: &Path) -> Result<(), String> {
    let store = HistoryStore::open(path)?;
    *HISTORY.lock().unwrap_or_else(|err| err.into_inner()) = Some(store);
    Ok(())
}

pub fn record_sample(ts: i64, kind: &str, key: &str, value: f64) {
    let guard = HISTORY.lock().unwrap_or_else(|err| err.into_inner());
    if let Some(store) = guard.as_ref() {
        let _ = store.record(ts, kind, key, value);
    }
}

pub fn record_folders(ts: i64, folders: &[(String, u64)]) {
    let guard = HISTORY.lock().unwrap_or_else(|err| err.into_inner());
    if let Some(store) = guard.as_ref() {
        let _ = store.folders(ts, folders);
    }
}

pub fn series(kind: &str, key: &str, from: i64, to: i64, buckets: usize) -> Vec<Point> {
    let guard = HISTORY.lock().unwrap_or_else(|err| err.into_inner());
    guard.as_ref().and_then(|store| store.series(kind, key, from, to, buckets).ok()).unwrap_or_default()
}

pub fn grew(kind: &str, from: i64, to: i64) -> Vec<Grew> {
    let guard = HISTORY.lock().unwrap_or_else(|err| err.into_inner());
    guard.as_ref().and_then(|store| store.grew(kind, from, to).ok()).unwrap_or_default()
}

pub fn grew_folders(from: i64, to: i64) -> Vec<Grew> {
    let guard = HISTORY.lock().unwrap_or_else(|err| err.into_inner());
    guard.as_ref().and_then(|store| store.grew_folders(from, to).ok()).unwrap_or_default()
}

impl HistoryStore {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path).map_err(|err| err.to_string())?;
        let store = Self { conn };
        store.prepare()?;
        Ok(store)
    }

    pub fn memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|err| err.to_string())?;
        let store = Self { conn };
        store.prepare()?;
        Ok(store)
    }

    fn prepare(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS samples (
                    ts INTEGER NOT NULL,
                    kind TEXT NOT NULL,
                    key TEXT NOT NULL,
                    value REAL NOT NULL
                );
                CREATE INDEX IF NOT EXISTS samples_lookup ON samples(kind, key, ts);
                CREATE TABLE IF NOT EXISTS folders (
                    ts INTEGER NOT NULL,
                    path TEXT NOT NULL,
                    bytes INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS folders_lookup ON folders(path, ts);",
            )
            .map_err(|err| err.to_string())
    }

    pub fn record(&self, ts: i64, kind: &str, key: &str, value: f64) -> Result<(), String> {
        self.conn
            .execute("INSERT INTO samples (ts, kind, key, value) VALUES (?1, ?2, ?3, ?4)", params![ts, kind, key, value])
            .map(|_| ())
            .map_err(|err| err.to_string())
    }

    pub fn folders(&self, ts: i64, folders: &[(String, u64)]) -> Result<(), String> {
        let tx = self.conn.unchecked_transaction().map_err(|err| err.to_string())?;
        for (path, bytes) in folders {
            tx.execute("INSERT INTO folders (ts, path, bytes) VALUES (?1, ?2, ?3)", params![ts, path, *bytes as i64])
                .map_err(|err| err.to_string())?;
        }
        tx.commit().map_err(|err| err.to_string())
    }

    pub fn series(&self, kind: &str, key: &str, from: i64, to: i64, buckets: usize) -> Result<Vec<Point>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT ts, value FROM samples WHERE kind = ?1 AND key = ?2 AND ts >= ?3 AND ts <= ?4 ORDER BY ts")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![kind, key, from, to], |row| Ok(Point { ts: row.get(0)?, value: row.get(1)? }))
            .map_err(|err| err.to_string())?;
        let points = rows.collect::<Result<Vec<_>, _>>().map_err(|err| err.to_string())?;
        Ok(bucket(&points, from, to, buckets.max(1)))
    }

    pub fn grew(&self, kind: &str, from: i64, to: i64) -> Result<Vec<Grew>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, ts, value FROM samples WHERE kind = ?1 AND ts >= ?2 AND ts <= ?3 ORDER BY key, ts")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![kind, from, to], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, f64>(2)?)))
            .map_err(|err| err.to_string())?;
        let mut grouped: Vec<(String, f64, f64)> = Vec::new();
        for row in rows {
            let (key, _, value) = row.map_err(|err| err.to_string())?;
            if let Some(slot) = grouped.iter_mut().find(|item| item.0 == key) {
                slot.2 = value;
            } else {
                grouped.push((key, value, value));
            }
        }
        let mut out: Vec<Grew> = grouped
            .into_iter()
            .map(|(key, before, after)| Grew { key, before, after })
            .filter(|item| (item.after - item.before).abs() > 0.5)
            .collect();
        out.sort_by(|a, b| (b.after - b.before).partial_cmp(&(a.after - a.before)).unwrap_or(std::cmp::Ordering::Equal));
        Ok(out)
    }

    pub fn grew_folders(&self, from: i64, to: i64) -> Result<Vec<Grew>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, ts, bytes FROM folders WHERE ts >= ?1 AND ts <= ?2 ORDER BY path, ts")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![from, to], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, i64>(2)? as f64)))
            .map_err(|err| err.to_string())?;
        let mut grouped: Vec<(String, f64, f64)> = Vec::new();
        for row in rows {
            let (key, _, value) = row.map_err(|err| err.to_string())?;
            if let Some(slot) = grouped.iter_mut().find(|item| item.0 == key) {
                slot.2 = value;
            } else {
                grouped.push((key, value, value));
            }
        }
        let mut out: Vec<Grew> = grouped.into_iter().map(|(key, before, after)| Grew { key, before, after }).collect();
        out.sort_by(|a, b| (b.after - b.before).partial_cmp(&(a.after - a.before)).unwrap_or(std::cmp::Ordering::Equal));
        out.truncate(12);
        Ok(out)
    }
}

pub fn bucket(points: &[Point], from: i64, to: i64, buckets: usize) -> Vec<Point> {
    if points.is_empty() || buckets == 0 || to <= from {
        return points.to_vec();
    }
    let width = (to - from) as f64 / buckets as f64;
    let mut sums = vec![0.0; buckets];
    let mut counts = vec![0u32; buckets];
    for point in points {
        let mut slot = ((point.ts - from) as f64 / width) as usize;
        if slot >= buckets {
            slot = buckets - 1;
        }
        sums[slot] += point.value;
        counts[slot] += 1;
    }
    (0..buckets)
        .filter(|slot| counts[*slot] > 0)
        .map(|slot| Point {
            ts: from + (width * (slot as f64 + 0.5)) as i64,
            value: sums[slot] / f64::from(counts[slot]),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buckets_and_shows_what_grew() {
        let store = HistoryStore::memory().unwrap();
        store.record(1_000, "disk", "C:", 100.0).unwrap();
        store.record(2_000, "disk", "C:", 180.0).unwrap();
        store.record(1_000, "disk", "D:", 50.0).unwrap();
        store.record(2_000, "disk", "D:", 40.0).unwrap();
        store.record(1_500, "cpu", "", 10.0).unwrap();
        store.record(1_800, "cpu", "", 30.0).unwrap();
        let series = store.series("cpu", "", 1_000, 2_000, 4).unwrap();
        assert!(!series.is_empty());
        assert!(series.iter().all(|point| point.value > 0.0));
        let grew = store.grew("disk", 1_000, 2_000).unwrap();
        assert_eq!(grew[0].key, "C:");
        assert!(grew[0].after > grew[0].before);
        store.folders(1_000, &[("C:\\Users".into(), 10), ("C:\\Games".into(), 20)]).unwrap();
        store.folders(2_000, &[("C:\\Users".into(), 15), ("C:\\Games".into(), 80)]).unwrap();
        let folders = store.grew_folders(1_000, 2_000).unwrap();
        assert_eq!(folders[0].key, "C:\\Games");
        let points = vec![Point { ts: 0, value: 1.0 }, Point { ts: 10, value: 3.0 }];
        assert_eq!(bucket(&points, 0, 10, 2).len(), 2);
    }
}
