//! Índice de nomes para a busca instantânea. O USN entra como delta de verdade:
//! cria, apaga e renomeia pelo FRN, e o mapa do disco recebe o mesmo toque.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use crate::kind::kind_of;
use crate::ntfs::{FlatNode, UsnEvent, ATTR_DIRECTORY, REASON_FILE_CREATE, REASON_FILE_DELETE, REASON_RENAME_NEW, REASON_RENAME_OLD};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHit {
    pub name: String,
    pub path: String,
    pub parent: String,
    pub size: u64,
    pub modified_ms: u64,
    pub dir: bool,
    pub kind: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FileQuery {
    pub text: String,
    pub kind: String,
    pub min_size: u64,
    pub max_size: u64,
    pub after_ms: u64,
    pub before_ms: u64,
}

#[derive(Clone, Debug)]
pub struct FileTouch {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub kind: String,
    pub alive: bool,
}

#[derive(Clone, Debug, Default)]
pub struct UsnReport {
    pub added: u32,
    pub removed: u32,
    pub renamed: u32,
    pub next_usn: i64,
    pub touches: Vec<FileTouch>,
}

pub struct FileIndex {
    conn: Connection,
}

static FILES: Mutex<Option<FileIndex>> = Mutex::new(None);

pub fn open_files(path: &Path) -> Result<(), String> {
    let index = FileIndex::open(path)?;
    *FILES.lock().unwrap_or_else(|err| err.into_inner()) = Some(index);
    Ok(())
}

pub fn ingest_volume(volume: &str, nodes: &[FlatNode], journal_id: u64, next_usn: i64) {
    let guard = FILES.lock().unwrap_or_else(|err| err.into_inner());
    let Some(index) = guard.as_ref() else {
        return;
    };
    let _ = index.replace_volume(volume, nodes, journal_id, next_usn);
}

pub fn search_files(query: &FileQuery) -> Result<Vec<FileHit>, String> {
    let guard = FILES.lock().unwrap_or_else(|err| err.into_inner());
    let Some(index) = guard.as_ref() else {
        return Ok(Vec::new());
    };
    index.search(query)
}

pub fn file_count() -> u64 {
    let guard = FILES.lock().unwrap_or_else(|err| err.into_inner());
    guard.as_ref().and_then(|index| index.count().ok()).unwrap_or(0)
}

pub fn mark_of(volume: &str) -> Option<(u64, i64, String)> {
    let guard = FILES.lock().unwrap_or_else(|err| err.into_inner());
    guard.as_ref().and_then(|index| index.mark(volume).ok()).flatten()
}

pub fn apply_events(volume: &str, events: &[UsnEvent], next_usn: i64) -> Result<UsnReport, String> {
    let guard = FILES.lock().unwrap_or_else(|err| err.into_inner());
    let Some(index) = guard.as_ref() else {
        return Err("índice fechado".into());
    };
    index.apply(volume, events, next_usn)
}

pub fn seed_volume(volume: &str, hits: &[FileHit]) -> Result<(), String> {
    let guard = FILES.lock().unwrap_or_else(|err| err.into_inner());
    let Some(index) = guard.as_ref() else {
        return Ok(());
    };
    index.seed_paths(volume, hits)
}

pub fn catch_up(volume: &str) -> Option<UsnReport> {
    #[cfg(windows)]
    {
        let letter = volume.chars().next()?;
        let (journal, next, source) = mark_of(volume)?;
        if source != "mft" || journal == 0 {
            return None;
        }
        let (cursor, events) = crate::ntfs::read_usn_events(letter, journal, next)?;
        apply_events(volume, &events, cursor).ok()
    }
    #[cfg(not(windows))]
    {
        let _ = volume;
        None
    }
}

pub fn set_file_size(volume: &str, path: &str, size: u64, modified_ms: u64) {
    let guard = FILES.lock().unwrap_or_else(|err| err.into_inner());
    if let Some(index) = guard.as_ref() {
        let _ = index.set_size(volume, path, size, modified_ms);
    }
}

impl FileIndex {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(path).map_err(|err| err.to_string())?;
        let index = Self { conn };
        index.prepare()?;
        Ok(index)
    }

    pub fn memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|err| err.to_string())?;
        let index = Self { conn };
        index.prepare()?;
        Ok(index)
    }

    fn prepare(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS files (
                    volume TEXT NOT NULL,
                    record INTEGER NOT NULL,
                    name TEXT NOT NULL,
                    path TEXT NOT NULL,
                    parent TEXT NOT NULL,
                    size INTEGER NOT NULL,
                    mtime INTEGER NOT NULL,
                    dir INTEGER NOT NULL,
                    kind TEXT NOT NULL,
                    PRIMARY KEY (volume, record)
                );
                CREATE INDEX IF NOT EXISTS files_name ON files(name COLLATE NOCASE);
                CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
            )
            .map_err(|err| err.to_string())
    }

    pub fn replace_volume(&self, volume: &str, nodes: &[FlatNode], journal_id: u64, next_usn: i64) -> Result<(), String> {
        let tx = self.conn.unchecked_transaction().map_err(|err| err.to_string())?;
        tx.execute("DELETE FROM files WHERE volume = ?1", [volume]).map_err(|err| err.to_string())?;
        let root = volume_root(volume);
        tx.execute(
            "INSERT INTO files (volume, record, name, path, parent, size, mtime, dir, kind) VALUES (?1, 5, ?2, ?3, ?3, 0, 0, 1, 'pasta')",
            params![volume, root.trim_end_matches('\\'), root],
        )
        .map_err(|err| err.to_string())?;
        for node in nodes {
            if node.record == 0 || node.record == 5 || node.name.starts_with('$') {
                continue;
            }
            let kind = if node.is_dir { "pasta" } else { kind_of(Path::new(&node.path)) };
            tx.execute(
                "INSERT OR REPLACE INTO files (volume, record, name, path, parent, size, mtime, dir, kind) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![volume, node.record as i64, node.name, node.path, node.parent, node.size as i64, node.mtime_ms as i64, node.is_dir as i64, kind],
            )
            .map_err(|err| err.to_string())?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            params![format!("vol:{volume}"), format!("{journal_id},{next_usn},mft")],
        )
        .map_err(|err| err.to_string())?;
        tx.commit().map_err(|err| err.to_string())
    }

    pub fn seed_paths(&self, volume: &str, hits: &[FileHit]) -> Result<(), String> {
        let tx = self.conn.unchecked_transaction().map_err(|err| err.to_string())?;
        tx.execute("DELETE FROM files WHERE volume = ?1", [volume]).map_err(|err| err.to_string())?;
        for (index, hit) in hits.iter().enumerate() {
            tx.execute(
                "INSERT OR REPLACE INTO files (volume, record, name, path, parent, size, mtime, dir, kind) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![volume, index as i64 + 1, hit.name, hit.path, hit.parent, hit.size as i64, hit.modified_ms as i64, hit.dir as i64, hit.kind],
            )
            .map_err(|err| err.to_string())?;
        }
        tx.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
            params![format!("vol:{volume}"), "0,0,mapa"],
        )
        .map_err(|err| err.to_string())?;
        tx.commit().map_err(|err| err.to_string())
    }

    pub fn mark(&self, volume: &str) -> Result<Option<(u64, i64, String)>, String> {
        let value: Option<String> = self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [format!("vol:{volume}")], |row| row.get(0))
            .ok();
        let Some(value) = value else {
            return Ok(None);
        };
        let mut parts = value.splitn(3, ',');
        let journal = parts.next().unwrap_or("0").parse().unwrap_or(0);
        let next = parts.next().unwrap_or("0").parse().unwrap_or(0);
        let source = parts.next().unwrap_or("mapa").to_string();
        Ok(Some((journal, next, source)))
    }

    pub fn search(&self, query: &FileQuery) -> Result<Vec<FileHit>, String> {
        let text = query.text.trim().to_lowercase();
        let mut sql = String::from(
            "SELECT name, path, parent, size, mtime, dir, kind FROM files WHERE name NOT LIKE '$%' ",
        );
        let mut args: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        if !text.is_empty() {
            sql.push_str(" AND lower(name) LIKE ? ");
            args.push(Box::new(format!("%{text}%")));
        }
        if !query.kind.is_empty() {
            sql.push_str(" AND kind = ? ");
            args.push(Box::new(query.kind.clone()));
        }
        if query.min_size > 0 {
            sql.push_str(" AND size >= ? ");
            args.push(Box::new(query.min_size as i64));
        }
        if query.max_size > 0 {
            sql.push_str(" AND size <= ? ");
            args.push(Box::new(query.max_size as i64));
        }
        if query.after_ms > 0 {
            sql.push_str(" AND mtime >= ? ");
            args.push(Box::new(query.after_ms as i64));
        }
        if query.before_ms > 0 {
            sql.push_str(" AND mtime <= ? ");
            args.push(Box::new(query.before_ms as i64));
        }
        sql.push_str(" ORDER BY length(name), name COLLATE NOCASE LIMIT 80");
        let mut stmt = self.conn.prepare(&sql).map_err(|err| err.to_string())?;
        let refs: Vec<&dyn rusqlite::types::ToSql> = args.iter().map(|arg| arg.as_ref()).collect();
        let rows = stmt
            .query_map(refs.as_slice(), |row| {
                Ok(FileHit {
                    name: row.get(0)?,
                    path: row.get(1)?,
                    parent: row.get(2)?,
                    size: row.get::<_, i64>(3)? as u64,
                    modified_ms: row.get::<_, i64>(4)? as u64,
                    dir: row.get::<_, i64>(5)? != 0,
                    kind: row.get(6)?,
                })
            })
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|err| err.to_string())
    }

    pub fn count(&self) -> Result<u64, String> {
        self.conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get::<_, i64>(0)).map(|n| n as u64).map_err(|err| err.to_string())
    }

    pub fn apply(&self, volume: &str, events: &[UsnEvent], next_usn: i64) -> Result<UsnReport, String> {
        let mut report = UsnReport { next_usn, ..UsnReport::default() };
        let saved = self.mark(volume)?;
        let tx = self.conn.unchecked_transaction().map_err(|err| err.to_string())?;
        for event in events {
            if event.record == 0 || event.name.is_empty() || event.name.starts_with('$') {
                continue;
            }
            let delete = event.reason & REASON_FILE_DELETE != 0 || (event.reason & REASON_RENAME_OLD != 0 && event.reason & REASON_RENAME_NEW == 0);
            if delete {
                if let Some(hit) = lookup(&tx, volume, event.record)? {
                    tx.execute("DELETE FROM files WHERE volume = ?1 AND record = ?2", params![volume, event.record as i64]).map_err(|err| err.to_string())?;
                    report.removed += 1;
                    if event.reason & REASON_RENAME_OLD != 0 {
                        report.renamed += 1;
                    }
                    report.touches.push(FileTouch {
                        path: hit.path,
                        name: hit.name,
                        size: hit.size,
                        kind: hit.kind,
                        alive: false,
                    });
                }
                continue;
            }
            let useful = event.reason & (REASON_FILE_CREATE | REASON_RENAME_NEW | REASON_FILE_DELETE) != 0
                || event.reason & (crate::ntfs::REASON_CLOSE | crate::ntfs::REASON_DATA_EXTEND | crate::ntfs::REASON_DATA_OVERWRITE | crate::ntfs::REASON_DATA_TRUNCATION) != 0;
            if !useful {
                continue;
            }
            let parent = lookup(&tx, volume, event.parent)?.map(|hit| hit.path).unwrap_or_else(|| volume_root(volume));
            let path = join_path(&parent, &event.name);
            let existing = lookup(&tx, volume, event.record)?;
            let dir = event.attributes & ATTR_DIRECTORY != 0;
            let size = existing.as_ref().map(|hit| hit.size).unwrap_or(0);
            let kind = if dir { "pasta".to_string() } else { kind_of(Path::new(&path)).to_string() };
            if let Some(old) = &existing {
                if old.path != path {
                    report.renamed += 1;
                    report.touches.push(FileTouch {
                        path: old.path.clone(),
                        name: old.name.clone(),
                        size: old.size,
                        kind: old.kind.clone(),
                        alive: false,
                    });
                }
            } else {
                report.added += 1;
            }
            tx.execute(
                "INSERT OR REPLACE INTO files (volume, record, name, path, parent, size, mtime, dir, kind) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![volume, event.record as i64, event.name, path, parent, size as i64, event.time_ms as i64, dir as i64, kind],
            )
            .map_err(|err| err.to_string())?;
            report.touches.push(FileTouch {
                path,
                name: event.name.clone(),
                size,
                kind,
                alive: true,
            });
        }
        if let Some((journal, _, source)) = saved {
            tx.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES (?1, ?2)",
                params![format!("vol:{volume}"), format!("{journal},{next_usn},{source}")],
            )
            .map_err(|err| err.to_string())?;
        }
        tx.commit().map_err(|err| err.to_string())?;
        Ok(report)
    }

    fn set_size(&self, volume: &str, path: &str, size: u64, modified_ms: u64) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE files SET size = ?1, mtime = ?2 WHERE volume = ?3 AND path = ?4",
                params![size as i64, modified_ms as i64, volume, path],
            )
            .map(|_| ())
            .map_err(|err| err.to_string())
    }
}

fn lookup(conn: &Connection, volume: &str, record: u64) -> Result<Option<FileHit>, String> {
    conn.query_row(
        "SELECT name, path, parent, size, mtime, dir, kind FROM files WHERE volume = ?1 AND record = ?2",
        params![volume, record as i64],
        |row| {
            Ok(FileHit {
                name: row.get(0)?,
                path: row.get(1)?,
                parent: row.get(2)?,
                size: row.get::<_, i64>(3)? as u64,
                modified_ms: row.get::<_, i64>(4)? as u64,
                dir: row.get::<_, i64>(5)? != 0,
                kind: row.get(6)?,
            })
        },
    )
    .map(Some)
    .or_else(|err| if err == rusqlite::Error::QueryReturnedNoRows { Ok(None) } else { Err(err.to_string()) })
}

fn volume_root(volume: &str) -> String {
    let trimmed = volume.trim().trim_end_matches(['\\', '/']);
    if trimmed.len() == 2 && trimmed.as_bytes().get(1) == Some(&b':') {
        format!("{}\\", trimmed.to_uppercase())
    } else if trimmed.is_empty() {
        "C:\\".into()
    } else {
        trimmed.to_string()
    }
}

fn join_path(parent: &str, name: &str) -> String {
    if parent.ends_with('\\') || parent.ends_with('/') {
        format!("{parent}{name}")
    } else {
        format!("{parent}\\{name}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ntfs::{REASON_CLOSE, REASON_FILE_CREATE, REASON_FILE_DELETE, REASON_RENAME_NEW, REASON_RENAME_OLD};

    fn event(record: u64, parent: u64, reason: u32, name: &str, dir: bool) -> UsnEvent {
        UsnEvent {
            record,
            parent,
            usn: 1,
            reason,
            attributes: if dir { ATTR_DIRECTORY } else { 0 },
            time_ms: 1_700_000_000_000,
            name: name.into(),
        }
    }

    #[test]
    fn usn_creates_renames_and_deletes_and_search_filters() {
        let index = FileIndex::memory().unwrap();
        let nodes = vec![
            FlatNode { path: "C:\\docs".into(), name: "docs".into(), parent: "C:\\".into(), size: 0, is_dir: true, mtime_ms: 10, record: 10 },
            FlatNode { path: "C:\\docs\\a.bin".into(), name: "a.bin".into(), parent: "C:\\docs".into(), size: 500_000, is_dir: false, mtime_ms: 20, record: 11 },
        ];
        index.replace_volume("C:\\", &nodes, 9, 100).unwrap();
        let hits = index.search(&FileQuery { text: "a.bin".into(), ..FileQuery::default() }).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].kind, "outro");
        let created = index
            .apply("C:\\", &[event(20, 10, REASON_FILE_CREATE | REASON_CLOSE, "nota.txt", false)], 120)
            .unwrap();
        assert_eq!(created.added, 1);
        assert!(created.touches.iter().any(|touch| touch.alive && touch.path.ends_with("nota.txt")));
        let renamed = index
            .apply(
                "C:\\",
                &[
                    event(20, 10, REASON_RENAME_OLD, "nota.txt", false),
                    event(20, 5, REASON_RENAME_NEW | REASON_CLOSE, "outro.txt", false),
                ],
                140,
            )
            .unwrap();
        assert!(renamed.renamed >= 1);
        let found = index.search(&FileQuery { text: "outro".into(), ..FileQuery::default() }).unwrap();
        assert_eq!(found[0].path, "C:\\outro.txt");
        let removed = index.apply("C:\\", &[event(20, 5, REASON_FILE_DELETE | REASON_CLOSE, "outro.txt", false)], 150).unwrap();
        assert_eq!(removed.removed, 1);
        assert!(index.search(&FileQuery { text: "outro".into(), ..FileQuery::default() }).unwrap().is_empty());
        let big = index
            .search(&FileQuery { min_size: 1000, kind: "outro".into(), ..FileQuery::default() })
            .unwrap();
        assert!(big.iter().any(|hit| hit.name == "a.bin"));
        let (journal, next, source) = index.mark("C:\\").unwrap().unwrap();
        assert_eq!((journal, source), (9, "mft".into()));
        assert_eq!(next, 150);
    }
}
