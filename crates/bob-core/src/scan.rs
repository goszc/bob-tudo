use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::fastwalk::{dir_mtime_ms, list_dir};
use crate::kind::kind_of;
use crate::norm_path;
use crate::ntfs::FlatNode;
use crate::priority::{relax_thread, WORKERS};

const BIG_FILE: u64 = 256 * 1024;
const TOP: usize = 300;

#[derive(Clone, Debug)]
pub struct ScanProgress {
    pub files: u64,
    pub bytes: u64,
    pub current: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub kind: String,
    pub dir: bool,
    pub synthetic: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStamp {
    pub path: String,
    pub size: u64,
    pub modified_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedDir {
    pub mtime_ms: u64,
    pub size: u64,
    pub files: u64,
    pub direct_size: u64,
    pub direct_files: u64,
    pub subdirs: Vec<String>,
    pub items: Vec<Entry>,
    pub small_size: u64,
    pub small_count: u64,
    pub stamps: Vec<FileStamp>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DirTreeCache {
    pub root: String,
    pub dirs: HashMap<String, CachedDir>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskSnapshot {
    pub root: String,
    pub total: u64,
    pub files: u64,
    pub errors: u64,
    pub children: Vec<Entry>,
    pub biggest: Vec<Entry>,
}

#[derive(Clone, Debug)]
struct RawDir {
    size: u64,
    files: u64,
    subdirs: Vec<String>,
    items: Vec<Entry>,
    small_size: u64,
    small_count: u64,
    mtime_ms: u64,
    direct_size: u64,
    direct_files: u64,
    stamps: Vec<FileStamp>,
    reused: bool,
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
    pub reused_dirs: u64,
    dirs: HashMap<String, RawDir>,
    biggest_files: Vec<Entry>,
    pub file_stamps: Vec<FileStamp>,
}

pub fn scan_path(
    root: &Path,
    cancel: &AtomicBool,
    progress: impl FnMut(ScanProgress),
) -> Result<ScanIndex, String> {
    scan_path_cached(root, cancel, progress, None)
}

pub fn scan_path_cached(
    root: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(ScanProgress),
    cache: Option<&DirTreeCache>,
) -> Result<ScanIndex, String> {
    if !root.exists() {
        return Err("pasta não encontrada".into());
    }
    relax_thread();
    let root_key = norm_path(root);
    #[cfg(windows)]
    if is_volume_root(&root_key) {
        if let Some((nodes, cursor)) = crate::ntfs::try_scan_volume(root) {
            if !nodes.is_empty() {
                crate::files::ingest_volume(&root_key, &nodes, cursor.journal_id, cursor.next_usn);
                return Ok(index_from_nodes(&root_key, nodes));
            }
        }
    }
    let cache = cache.filter(|cache| cache.root == root_key);
    let mut dirs: HashMap<String, RawDir> = HashMap::new();
    let mut errors = 0u64;
    let mut reused_dirs = 0u64;
    let mut files_seen = 0u64;
    let mut last = Instant::now();
    let mut wave = vec![root_key.clone()];
    ensure(&mut dirs, &root_key);
    if let Some(dir) = dirs.get_mut(&root_key) {
        dir.mtime_ms = dir_mtime_ms(root);
    }

    while !wave.is_empty() {
        if cancel.load(AtomicOrdering::Relaxed) {
            return Err("varredura cancelada".into());
        }
        let batch = std::mem::take(&mut wave);
        let listed = map_workers(&batch, |path| classify(path, cache));
        for (path, outcome) in batch.into_iter().zip(listed) {
            match outcome {
                Outcome::Reuse => {
                    if reuse_files(cache, &path, &mut dirs, &mut wave) {
                        reused_dirs += 1;
                    }
                }
                Outcome::Listed(entries) => {
                    let mut next = Vec::new();
                    ingest(&path, entries, &mut dirs, &mut next, &mut files_seen, &mut errors);
                    wave.append(&mut next);
                }
                Outcome::Missing => {
                    errors += 1;
                }
            }
            if files_seen % 2000 == 0 || last.elapsed().as_millis() > 180 {
                last = Instant::now();
                progress(ScanProgress {
                    files: files_seen,
                    bytes: 0,
                    current: path,
                });
            }
        }
    }

    dedup_subdirs(&mut dirs);
    let (total, files) = rollup(&root_key, &mut dirs);
    let mut heap: BinaryHeap<std::cmp::Reverse<HeapItem>> = BinaryHeap::new();
    let mut file_stamps = Vec::new();
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
        dir.items.sort_by(|a, b| b.size.cmp(&a.size).then(a.name.cmp(&b.name)));
        dir.subdirs.sort();
        dir.subdirs.dedup();
        for stamp in &dir.stamps {
            consider_heap(&mut heap, stamp);
        }
        file_stamps.append(&mut dir.stamps);
    }
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
    progress(ScanProgress {
        files,
        bytes: total,
        current: root_key.clone(),
    });
    Ok(ScanIndex {
        root: root_key,
        total,
        files,
        errors,
        reused_dirs,
        dirs,
        biggest_files,
        file_stamps,
    })
}

enum Outcome {
    Reuse,
    Listed(Vec<crate::fastwalk::Listed>),
    Missing,
}

fn classify(path: &str, cache: Option<&DirTreeCache>) -> Outcome {
    let mtime = dir_mtime_ms(Path::new(path));
    if let Some(cache) = cache {
        if let Some(dir) = cache.dirs.get(path) {
            if dir.mtime_ms != 0 && dir.mtime_ms == mtime {
                return Outcome::Reuse;
            }
        }
    }
    match list_dir(Path::new(path)) {
        Ok(entries) => Outcome::Listed(entries),
        Err(_) => Outcome::Missing,
    }
}

fn ingest(
    path: &str,
    entries: Vec<crate::fastwalk::Listed>,
    dirs: &mut HashMap<String, RawDir>,
    next: &mut Vec<String>,
    files_seen: &mut u64,
    errors: &mut u64,
) {
    ensure(dirs, path);
    let mtime = dir_mtime_ms(Path::new(path));
    if let Some(dir) = dirs.get_mut(path) {
        dir.mtime_ms = mtime;
        dir.reused = false;
    }
    for entry in entries {
        if entry.is_dir {
            ensure(dirs, &entry.path);
            if let Some(dir) = dirs.get_mut(path) {
                dir.subdirs.push(entry.path.clone());
            }
            if let Some(dir) = dirs.get_mut(&entry.path) {
                dir.mtime_ms = entry.modified_ms;
            }
            next.push(entry.path);
            continue;
        }
        let kind = kind_of(Path::new(&entry.path)).to_string();
        if let Some(dir) = dirs.get_mut(path) {
            dir.direct_size = dir.direct_size.saturating_add(entry.size);
            dir.direct_files = dir.direct_files.saturating_add(1);
            if entry.size >= 4096 {
                dir.stamps.push(FileStamp {
                    path: entry.path.clone(),
                    size: entry.size,
                    modified_ms: entry.modified_ms,
                });
            }
            if entry.size >= BIG_FILE {
                dir.items.push(Entry {
                    name: entry.name,
                    path: entry.path,
                    size: entry.size,
                    kind,
                    dir: false,
                    synthetic: false,
                });
            } else {
                dir.small_size = dir.small_size.saturating_add(entry.size);
                dir.small_count = dir.small_count.saturating_add(1);
            }
        } else {
            *errors = errors.saturating_add(1);
        }
        *files_seen = files_seen.saturating_add(1);
    }
    let _ = errors;
}

fn reuse_files(cache: Option<&DirTreeCache>, path: &str, dirs: &mut HashMap<String, RawDir>, next: &mut Vec<String>) -> bool {
    let Some(cached) = cache.and_then(|cache| cache.dirs.get(path)) else {
        return false;
    };
    ensure(dirs, path);
    if let Some(dir) = dirs.get_mut(path) {
        dir.mtime_ms = cached.mtime_ms;
        dir.direct_size = cached.direct_size;
        dir.direct_files = cached.direct_files;
        dir.items = cached.items.clone();
        dir.small_size = cached.small_size;
        dir.small_count = cached.small_count;
        dir.stamps = cached.stamps.clone();
        dir.subdirs = cached.subdirs.clone();
        dir.reused = false;
    }
    next.extend(cached.subdirs.iter().cloned());
    true
}

fn dedup_subdirs(dirs: &mut HashMap<String, RawDir>) {
    for dir in dirs.values_mut() {
        dir.subdirs.sort();
        dir.subdirs.dedup();
    }
}

fn rollup(key: &str, dirs: &mut HashMap<String, RawDir>) -> (u64, u64) {
    let Some(dir) = dirs.get(key) else {
        return (0, 0);
    };
    if dir.reused {
        return (dir.size, dir.files);
    }
    let subdirs = dir.subdirs.clone();
    let mut size = dir.direct_size;
    let mut files = dir.direct_files;
    for sub in subdirs {
        let (child_size, child_files) = rollup(&sub, dirs);
        size = size.saturating_add(child_size);
        files = files.saturating_add(child_files);
    }
    if let Some(dir) = dirs.get_mut(key) {
        dir.size = size;
        dir.files = files;
    }
    (size, files)
}

fn map_workers<T>(items: &[String], map: impl Fn(&str) -> T + Sync) -> Vec<T>
where
    T: Send,
{
    if items.len() <= 1 || WORKERS < 2 {
        return items.iter().map(|item| map(item)).collect();
    }
    let mut out = Vec::with_capacity(items.len());
    let mid = items.len().div_ceil(2);
    let (left, right) = items.split_at(mid.min(items.len()));
    std::thread::scope(|scope| {
        let right_handle = scope.spawn(|| right.iter().map(|item| map(item)).collect::<Vec<_>>());
        let mut left_out: Vec<T> = left.iter().map(|item| map(item)).collect();
        let right_out = right_handle.join().unwrap_or_default();
        out.append(&mut left_out);
        out.extend(right_out);
    });
    out
}

fn consider_heap(heap: &mut BinaryHeap<std::cmp::Reverse<HeapItem>>, stamp: &FileStamp) {
    if stamp.size < BIG_FILE {
        return;
    }
    let name = Path::new(&stamp.path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| stamp.path.clone());
    let item = HeapItem {
        size: stamp.size,
        path: stamp.path.clone(),
        name,
        kind: kind_of(Path::new(&stamp.path)).to_string(),
    };
    if heap.len() < TOP {
        heap.push(std::cmp::Reverse(item));
    } else if heap.peek().is_some_and(|top| top.0.size < stamp.size) {
        heap.pop();
        heap.push(std::cmp::Reverse(item));
    }
}

pub fn index_from_nodes(root: &str, nodes: Vec<FlatNode>) -> ScanIndex {
    let root_key = norm_path(Path::new(root));
    let mut dirs: HashMap<String, RawDir> = HashMap::new();
    ensure(&mut dirs, &root_key);
    let mut files = 0u64;
    for node in nodes {
        if node.is_dir {
            ensure(&mut dirs, &node.path);
            ensure(&mut dirs, &node.parent);
            if let Some(dir) = dirs.get_mut(&node.parent) {
                dir.subdirs.push(node.path.clone());
            }
            continue;
        }
        ensure(&mut dirs, &node.parent);
        if let Some(dir) = dirs.get_mut(&node.parent) {
            dir.direct_size = dir.direct_size.saturating_add(node.size);
            dir.direct_files = dir.direct_files.saturating_add(1);
            if node.size >= 4096 {
                dir.stamps.push(FileStamp {
                    path: node.path.clone(),
                    size: node.size,
                    modified_ms: node.mtime_ms,
                });
            }
            if node.size >= BIG_FILE {
                dir.items.push(Entry {
                    name: node.name,
                    path: node.path.clone(),
                    size: node.size,
                    kind: kind_of(Path::new(&node.path)).to_string(),
                    dir: false,
                    synthetic: false,
                });
            } else {
                dir.small_size = dir.small_size.saturating_add(node.size);
                dir.small_count = dir.small_count.saturating_add(1);
            }
        }
        files += 1;
    }
    dedup_subdirs(&mut dirs);
    let (total, rolled) = rollup(&root_key, &mut dirs);
    let mut heap = BinaryHeap::new();
    let mut file_stamps = Vec::new();
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
        for stamp in &dir.stamps {
            consider_heap(&mut heap, stamp);
        }
        file_stamps.append(&mut dir.stamps);
    }
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
    ScanIndex {
        root: root_key,
        total,
        files: if rolled > 0 { rolled } else { files },
        errors: 0,
        reused_dirs: 0,
        dirs,
        biggest_files,
        file_stamps,
    }
}

pub fn cache_from_index(index: &ScanIndex) -> DirTreeCache {
    let mut grouped: HashMap<String, Vec<FileStamp>> = HashMap::new();
    for stamp in &index.file_stamps {
        let parent = parent_key(&stamp.path).unwrap_or_else(|| index.root.clone());
        grouped.entry(parent).or_default().push(stamp.clone());
    }
    DirTreeCache {
        root: index.root.clone(),
        dirs: index
            .dirs
            .iter()
            .map(|(path, dir)| {
                (
                    path.clone(),
                    CachedDir {
                        mtime_ms: dir.mtime_ms,
                        size: dir.size,
                        files: dir.files,
                        subdirs: dir.subdirs.clone(),
                        direct_size: dir.direct_size,
                        direct_files: dir.direct_files,
                        items: dir
                            .items
                            .iter()
                            .filter(|item| !item.synthetic)
                            .cloned()
                            .collect(),
                        small_size: dir.small_size,
                        small_count: dir.small_count,
                        stamps: grouped.remove(path).unwrap_or_default(),
                    },
                )
            })
            .collect(),
    }
}

pub fn index_from_cache(cache: &DirTreeCache) -> ScanIndex {
    let mut dirs: HashMap<String, RawDir> = HashMap::new();
    let mut heap: BinaryHeap<std::cmp::Reverse<HeapItem>> = BinaryHeap::new();
    let mut file_stamps = Vec::new();
    let mut total = 0u64;
    let mut files = 0u64;
    for (path, dir) in &cache.dirs {
        if path == &cache.root {
            total = dir.size;
            files = dir.files;
        }
        for stamp in &dir.stamps {
            consider_heap(&mut heap, stamp);
        }
        file_stamps.extend(dir.stamps.iter().cloned());
        dirs.insert(
            path.clone(),
            RawDir {
                size: dir.size,
                files: dir.files,
                subdirs: dir.subdirs.clone(),
                items: dir.items.iter().filter(|item| !item.synthetic).cloned().collect(),
                small_size: dir.small_size,
                small_count: dir.small_count,
                mtime_ms: dir.mtime_ms,
                direct_size: dir.direct_size,
                direct_files: dir.direct_files,
                stamps: dir.stamps.clone(),
                reused: false,
            },
        );
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
    }
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
    ScanIndex {
        root: cache.root.clone(),
        total,
        files,
        errors: 0,
        reused_dirs: 0,
        dirs,
        biggest_files,
        file_stamps,
    }
}

pub fn note_file(index: &mut ScanIndex, path: &str, name: &str, size: u64, kind: &str) {
    if path.is_empty() {
        return;
    }
    let _ = forget(index, path);
    if size == 0 {
        return;
    }
    let parent = parent_key(path).unwrap_or_else(|| index.root.clone());
    ensure(&mut index.dirs, &parent);
    if let Some(dir) = index.dirs.get_mut(&parent) {
        dir.direct_size = dir.direct_size.saturating_add(size);
        dir.direct_files = dir.direct_files.saturating_add(1);
        if size >= 4096 {
            dir.stamps.push(FileStamp {
                path: path.to_string(),
                size,
                modified_ms: 0,
            });
        }
        if size >= BIG_FILE {
            dir.items.push(Entry {
                name: name.to_string(),
                path: path.to_string(),
                size,
                kind: kind.to_string(),
                dir: false,
                synthetic: false,
            });
            dir.items.sort_by(|a, b| b.size.cmp(&a.size));
        } else {
            dir.small_size = dir.small_size.saturating_add(size);
            dir.small_count = dir.small_count.saturating_add(1);
        }
    }
    add_size(&mut index.dirs, &parent, size, 1);
    index.total = index.total.saturating_add(size);
    index.files = index.files.saturating_add(1);
    if size >= BIG_FILE {
        index.biggest_files.push(Entry {
            name: name.to_string(),
            path: path.to_string(),
            size,
            kind: kind.to_string(),
            dir: false,
            synthetic: false,
        });
        index.biggest_files.sort_by(|a, b| b.size.cmp(&a.size));
        index.biggest_files.truncate(TOP);
    }
    if size >= 4096 {
        index.file_stamps.push(FileStamp {
            path: path.to_string(),
            size,
            modified_ms: 0,
        });
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

pub fn snapshot_of(index: &ScanIndex) -> DiskSnapshot {
    DiskSnapshot {
        root: index.root.clone(),
        total: index.total,
        files: index.files,
        errors: index.errors,
        children: children_of(index, &index.root),
        biggest: index.biggest_files.clone(),
    }
}

pub fn save_cache(cache: &DirTreeCache) -> Vec<u8> {
    serde_json::to_vec(cache).unwrap_or_default()
}

pub fn load_cache(bytes: &[u8]) -> Option<DirTreeCache> {
    serde_json::from_slice(bytes).ok()
}

fn is_volume_root(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() == 3 && bytes[1] == b':' && (bytes[2] == b'\\' || bytes[2] == b'/')
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
            mtime_ms: 0,
            direct_size: 0,
            direct_files: 0,
            stamps: Vec::new(),
            reused: false,
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
    index.file_stamps.retain(|stamp| stamp.path != path && !stamp.path.starts_with(&prefix));
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
    index.file_stamps.retain(|stamp| stamp.path != path);
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
    fn builds_index_from_mft_nodes() {
        let nodes = vec![
            crate::ntfs::FlatNode {
                path: "C:\\docs".into(),
                name: "docs".into(),
                parent: "C:\\".into(),
                size: 0,
                is_dir: true,
                mtime_ms: 0,
                record: 10,
            },
            crate::ntfs::FlatNode {
                path: "C:\\docs\\a.bin".into(),
                name: "a.bin".into(),
                parent: "C:\\docs".into(),
                size: 300_000,
                is_dir: false,
                mtime_ms: 0,
                record: 11,
            },
        ];
        let index = index_from_nodes("C:\\", nodes);
        assert!(index.total >= 300_000);
        assert!(is_volume_root("C:\\"));
        assert!(!is_volume_root("C:\\docs"));
        assert!(children_of(&index, &index.root).iter().any(|entry| entry.name.ends_with("docs") || entry.path.ends_with("docs")));
    }

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
        let mut cache = cache_from_index(&index);
        let fotos = cache.dirs.keys().find(|path| path.ends_with("fotos")).cloned().expect("pasta fotos");
        if let Some(dir) = cache.dirs.get_mut(&fotos) {
            dir.mtime_ms = 1;
        }
        fs::write(root.join("fotos").join("b.jpg"), vec![2u8; 300_000]).unwrap();
        let again = scan_path_cached(&root, &AtomicBool::new(false), |_| {}, Some(&cache)).unwrap();
        assert!(again.total >= 600_000, "total {}", again.total);
        assert!(again.reused_dirs >= 1, "pastas intactas devem reaproveitar a lista");
        let bytes = save_cache(&cache);
        assert!(load_cache(&bytes).is_some());
        let _ = fs::remove_dir_all(&root);
    }
}
