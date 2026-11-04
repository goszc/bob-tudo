use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::time::Instant;

use jwalk::WalkDir;
use serde::Serialize;

use crate::kind::kind_of;
use crate::norm_path;

const BIG_FILE: u64 = 256 * 1024;
const TOP: usize = 300;

#[derive(Clone, Debug)]
pub struct ScanProgress {
    pub files: u64,
    pub bytes: u64,
    pub current: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub kind: String,
    pub dir: bool,
    pub synthetic: bool,
}

#[derive(Clone, Debug)]
struct RawDir {
    size: u64,
    files: u64,
    subdirs: Vec<String>,
    items: Vec<Entry>,
    small_size: u64,
    small_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HeapItem {
    size: u64,
    path: String,
    name: String,
    kind: String,
}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> Ordering {
        self.size.cmp(&other.size).then(self.path.cmp(&other.path))
    }
}

#[derive(Clone, Debug)]
pub struct ScanIndex {
    pub root: String,
    pub total: u64,
    pub files: u64,
    pub errors: u64,
    dirs: HashMap<String, RawDir>,
    biggest_files: Vec<Entry>,
}

pub fn scan_path(
    root: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(ScanProgress),
) -> Result<ScanIndex, String> {
    if !root.exists() {
        return Err("pasta não encontrada".into());
    }
    let root_key = norm_path(root);
    let mut dirs: HashMap<String, RawDir> = HashMap::new();
    ensure(&mut dirs, &root_key);
    let mut heap: BinaryHeap<std::cmp::Reverse<HeapItem>> = BinaryHeap::new();
    let mut files = 0u64;
    let mut errors = 0u64;
    let mut last = Instant::now();

    for entry in WalkDir::new(root).skip_hidden(false).follow_links(false) {
        if cancel.load(AtomicOrdering::Relaxed) {
            return Err("varredura cancelada".into());
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        let path = entry.path();
        let key = norm_path(&path);
        let meta = match entry.metadata() {
            Ok(meta) => meta,
            Err(_) => {
                errors += 1;
                continue;
            }
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            ensure(&mut dirs, &key);
            continue;
        }
        if !meta.is_file() {
            continue;
        }
        let size = meta.len();
        let parent = path
            .parent()
            .map(norm_path)
            .filter(|parent| !parent.is_empty())
            .unwrap_or_else(|| root_key.clone());
        ensure(&mut dirs, &parent);
        add_size(&mut dirs, &parent, size, 1);
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| key.clone());
        let kind = kind_of(&path).to_string();
        if size >= BIG_FILE {
            if let Some(dir) = dirs.get_mut(&parent) {
                dir.items.push(Entry {
                    name: name.clone(),
                    path: key.clone(),
                    size,
                    kind: kind.clone(),
                    dir: false,
                    synthetic: false,
                });
            }
            let item = HeapItem {
                size,
                path: key.clone(),
                name,
                kind,
            };
            if heap.len() < TOP {
                heap.push(std::cmp::Reverse(item));
            } else if heap.peek().is_some_and(|top| top.0.size < size) {
                heap.pop();
                heap.push(std::cmp::Reverse(item));
            }
        } else if let Some(dir) = dirs.get_mut(&parent) {
            dir.small_size += size;
            dir.small_count += 1;
        }
        files += 1;
        if files % 2000 == 0 || last.elapsed().as_millis() > 180 {
            last = Instant::now();
            let total = dirs.get(&root_key).map(|dir| dir.size).unwrap_or(0);
            progress(ScanProgress {
                files,
                bytes: total,
                current: key,
            });
        }
    }

    for dir in dirs.values_mut() {
        if dir.small_count > 0 {
            dir.items.push(Entry {
                name: format!("arquivos pequenos ({})", dir.small_count),
                path: String::new(),
                size: dir.small_size,
                kind: "outro".into(),
                dir: false,
                synthetic: true,
            });
        }
        dir.items.sort_by(|a, b| b.size.cmp(&a.size));
        dir.subdirs.sort();
        dir.subdirs.dedup();
    }

    let total = dirs.get(&root_key).map(|dir| dir.size).unwrap_or(0);
    let mut biggest_files: Vec<Entry> = heap
        .into_iter()
        .map(|item| Entry {
            name: item.0.name,
            path: item.0.path,
            size: item.0.size,
            kind: item.0.kind,
            dir: false,
            synthetic: false,
        })
        .collect();
    biggest_files.sort_by(|a, b| b.size.cmp(&a.size));
    Ok(ScanIndex {
        root: root_key,
        total,
        files,
        errors,
        dirs,
        biggest_files,
    })
}

fn ensure(dirs: &mut HashMap<String, RawDir>, key: &str) {
    if key.is_empty() || dirs.contains_key(key) {
        return;
    }
    dirs.insert(
        key.to_string(),
        RawDir {
            size: 0,
            files: 0,
            subdirs: Vec::new(),
            items: Vec::new(),
            small_size: 0,
            small_count: 0,
        },
    );
    if let Some(parent) = parent_key(key) {
        ensure(dirs, &parent);
        if let Some(dir) = dirs.get_mut(&parent) {
            dir.subdirs.push(key.to_string());
        }
    }
}

fn parent_key(key: &str) -> Option<String> {
    let path = Path::new(key);
    let parent = path.parent()?;
    if parent.as_os_str().is_empty() {
        return None;
    }
    let parent = norm_path(parent);
    if parent == key {
        None
    } else {
        Some(parent)
    }
}

fn sub_size(dirs: &mut HashMap<String, RawDir>, start: &str, size: u64, files: u64) {
    let mut current = Some(start.to_string());
    let mut guard = 0;
    while let Some(key) = current {
        if let Some(dir) = dirs.get_mut(&key) {
            dir.size = dir.size.saturating_sub(size);
            dir.files = dir.files.saturating_sub(files);
        }
        current = parent_key(&key);
        guard += 1;
        if guard > 80 {
            break;
        }
    }
}

fn add_size(dirs: &mut HashMap<String, RawDir>, start: &str, size: u64, files: u64) {
    let mut current = Some(start.to_string());
    let mut guard = 0;
    while let Some(key) = current {
        if let Some(dir) = dirs.get_mut(&key) {
            dir.size = dir.size.saturating_add(size);
            dir.files = dir.files.saturating_add(files);
        }
        current = parent_key(&key);
        guard += 1;
        if guard > 80 {
            break;
        }
    }
}

pub fn children_of(index: &ScanIndex, path: &str) -> Vec<Entry> {
    let key = if path.is_empty() { index.root.clone() } else { path.to_string() };
    let Some(dir) = index.dirs.get(&key) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for sub in &dir.subdirs {
        if let Some(child) = index.dirs.get(sub) {
            let name = Path::new(sub)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| sub.clone());
            out.push(Entry {
                name,
                path: sub.clone(),
                size: child.size,
                kind: "pasta".into(),
                dir: true,
                synthetic: false,
            });
        }
    }
    out.extend(dir.items.iter().cloned());
    out.sort_by(|a, b| b.size.cmp(&a.size).then(a.name.cmp(&b.name)));
    out
}

pub fn biggest(index: &ScanIndex, folders: bool, query: &str) -> Vec<Entry> {
    if folders {
        let needle = query.trim().to_lowercase();
        let mut dirs: Vec<Entry> = index
            .dirs
            .iter()
            .filter(|(path, _)| path.as_str() != index.root)
            .filter(|(path, _)| needle.is_empty() || path.to_lowercase().contains(&needle))
            .map(|(path, dir)| Entry {
                name: Path::new(path)
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone()),
                path: path.clone(),
                size: dir.size,
                kind: "pasta".into(),
                dir: true,
                synthetic: false,
            })
            .collect();
        dirs.sort_by(|a, b| b.size.cmp(&a.size));
        dirs.truncate(200);
        dirs
    } else if query.trim().is_empty() {
        index.biggest_files.clone()
    } else {
        search(index, query)
    }
}

pub fn search(index: &ScanIndex, query: &str) -> Vec<Entry> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return index.biggest_files.clone();
    }
    let mut hits: Vec<Entry> = index
        .biggest_files
        .iter()
        .filter(|entry| {
            entry.name.to_lowercase().contains(&needle) || entry.path.to_lowercase().contains(&needle)
        })
        .cloned()
        .collect();
    if hits.len() < 40 {
        for (path, dir) in &index.dirs {
            if path.to_lowercase().contains(&needle) {
                hits.push(Entry {
                    name: Path::new(path)
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.clone()),
                    path: path.clone(),
                    size: dir.size,
                    kind: "pasta".into(),
                    dir: true,
                    synthetic: false,
                });
            }
            if hits.len() >= 80 {
                break;
            }
        }
    }
    hits.sort_by(|a, b| b.size.cmp(&a.size));
    hits.truncate(80);
    hits
}

pub fn forget(index: &mut ScanIndex, path: &str) -> u64 {
    if path.is_empty() || protected_self(path) {
        return 0;
    }
    if index.dirs.contains_key(path) {
        let size = index.dirs.get(path).map(|dir| dir.size).unwrap_or(0);
        let prefix = format!("{path}\\");
        let prefix_unix = format!("{path}/");
        index.dirs.retain(|key, _| key != path && !key.starts_with(&prefix) && !key.starts_with(&prefix_unix));
        if let Some(parent) = parent_key(path) {
            if let Some(dir) = index.dirs.get_mut(&parent) {
                dir.subdirs.retain(|sub| sub != path);
            }
            sub_size(&mut index.dirs, &parent, size, 0);
        }
        index.total = index.total.saturating_sub(size);
        index.biggest_files.retain(|entry| entry.path != path && !entry.path.starts_with(&prefix));
        return size;
    }
    let Some(parent) = parent_key(path) else {
        return 0;
    };
    let size = {
        let Some(dir) = index.dirs.get_mut(&parent) else {
            return 0;
        };
        let Some(pos) = dir.items.iter().position(|item| item.path == path) else {
            return 0;
        };
        dir.items.remove(pos).size
    };
    sub_size(&mut index.dirs, &parent, size, 1);
    index.total = index.total.saturating_sub(size);
    index.biggest_files.retain(|entry| entry.path != path);
    size
}

fn protected_self(path: &str) -> bool {
    crate::protected_path(Path::new(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn scans_nested_files() {
        let root = std::env::temp_dir().join(format!("bob-tudo-scan-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("fotos")).unwrap();
        fs::write(root.join("fotos").join("a.jpg"), vec![1u8; 300_000]).unwrap();
        fs::write(root.join("nota.txt"), b"oi").unwrap();
        let index = scan_path(&root, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(index.total >= 300_000);
        assert!(index.files >= 2);
        let children = children_of(&index, &index.root);
        assert!(children.iter().any(|entry| entry.name == "fotos"));
        let _ = fs::remove_dir_all(&root);
    }
}
