use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::UNIX_EPOCH;

use jwalk::WalkDir;
use rayon::prelude::*;
use serde::Serialize;

use crate::kind::is_image_ext;
use crate::nid;
use crate::phash::{dhash, hamming};

const MIN_BYTES: u64 = 4096;
const PREFIX: usize = 64 * 1024;
const IMAGE_MIN: u64 = 8 * 1024;
const HAMMING_MAX: u32 = 8;
const MAX_GROUPS: usize = 500;

#[derive(Clone, Debug)]
pub struct DupeProgress {
    pub phase: String,
    pub done: u64,
    pub total: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DupeFile {
    pub path: String,
    pub size: u64,
    pub modified_ms: u64,
    pub keep: bool,
    pub distance: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DupeGroup {
    pub id: String,
    pub kind: String,
    pub bytes: u64,
    pub files: Vec<DupeFile>,
}

struct Meta {
    path: PathBuf,
    size: u64,
    modified_ms: u64,
}

pub fn find_duplicates(
    roots: &[PathBuf],
    images: bool,
    cancel: &AtomicBool,
    mut progress: impl FnMut(DupeProgress),
) -> Result<Vec<DupeGroup>, String> {
    if roots.is_empty() {
        return Err("escolha uma pasta".into());
    }
    let mut by_size: HashMap<u64, Vec<Meta>> = HashMap::new();
    let mut seen = 0u64;
    for root in roots {
        if !root.exists() {
            return Err(format!("pasta não encontrada: {}", root.display()));
        }
        for entry in WalkDir::new(root).skip_hidden(false).follow_links(false) {
            if cancel.load(Ordering::Relaxed) {
                return Err("busca cancelada".into());
            }
            let Ok(entry) = entry else { continue };
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() || meta.file_type().is_symlink() {
                continue;
            }
            let size = meta.len();
            if size < MIN_BYTES {
                continue;
            }
            let modified_ms = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|time| time.as_millis() as u64)
                .unwrap_or(0);
            by_size.entry(size).or_default().push(Meta {
                path: entry.path(),
                size,
                modified_ms,
            });
            seen += 1;
            if seen % 4000 == 0 {
                progress(DupeProgress {
                    phase: "lendo tamanhos".into(),
                    done: seen,
                    total: seen,
                });
            }
        }
    }
    let mut groups: Vec<Vec<Meta>> = by_size.into_values().filter(|group| group.len() > 1).collect();
    progress(DupeProgress {
        phase: "comparando trechos".into(),
        done: 0,
        total: groups.len() as u64,
    });
    groups = regroup(groups, cancel, true, &mut progress)?;
    progress(DupeProgress {
        phase: "conferindo arquivos".into(),
        done: 0,
        total: groups.len() as u64,
    });
    groups = regroup(groups, cancel, false, &mut progress)?;

    let mut exact_paths = std::collections::HashSet::new();
    let mut out = Vec::new();
    for group in groups {
        if group.len() < 2 {
            continue;
        }
        let bytes = group[0].size;
        let mut files: Vec<DupeFile> = group
            .iter()
            .map(|meta| DupeFile {
                path: meta.path.display().to_string(),
                size: meta.size,
                modified_ms: meta.modified_ms,
                keep: false,
                distance: None,
            })
            .collect();
        mark_keep(&mut files, false);
        for file in &files {
            exact_paths.insert(file.path.clone());
        }
        out.push(DupeGroup {
            id: nid("d"),
            kind: "igual".into(),
            bytes,
            files,
        });
    }

    if images {
        progress(DupeProgress {
            phase: "imagens parecidas".into(),
            done: 0,
            total: 0,
        });
        out.extend(similar_images(roots, &exact_paths, cancel, &mut progress)?);
    }

    out.sort_by(|a, b| {
        let wa = a.bytes.saturating_mul(a.files.len().saturating_sub(1) as u64);
        let wb = b.bytes.saturating_mul(b.files.len().saturating_sub(1) as u64);
        wb.cmp(&wa)
    });
    out.truncate(MAX_GROUPS);
    Ok(out)
}

fn regroup(
    groups: Vec<Vec<Meta>>,
    cancel: &AtomicBool,
    prefix: bool,
    progress: &mut impl FnMut(DupeProgress),
) -> Result<Vec<Vec<Meta>>, String> {
    let total = groups.len() as u64;
    let phase = if prefix {
        "comparando trechos"
    } else {
        "conferindo arquivos"
    };
    progress(DupeProgress {
        phase: phase.into(),
        done: 0,
        total,
    });
    let hashed: Vec<Vec<(String, Meta)>> = groups
        .into_par_iter()
        .map(|group| {
            if cancel.load(Ordering::Relaxed) {
                return Vec::new();
            }
            group
                .into_iter()
                .filter_map(|meta| {
                    let hash = if prefix && meta.size > PREFIX as u64 {
                        hash_prefix(&meta.path, PREFIX)
                    } else {
                        hash_file(&meta.path)
                    }?;
                    Some((hash, meta))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    progress(DupeProgress {
        phase: phase.into(),
        done: total,
        total,
    });
    if cancel.load(Ordering::Relaxed) {
        return Err("busca cancelada".into());
    }
    let mut out = Vec::new();
    for group in hashed {
        let mut map: HashMap<String, Vec<Meta>> = HashMap::new();
        for (hash, meta) in group {
            map.entry(hash).or_default().push(meta);
        }
        out.extend(map.into_values().filter(|group| group.len() > 1));
    }
    Ok(out)
}

fn hash_prefix(path: &Path, len: usize) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut buf = vec![0u8; len];
    let read = file.read(&mut buf).ok()?;
    if read == 0 {
        return None;
    }
    Some(blake3::hash(&buf[..read]).to_hex().to_string())
}

fn hash_file(path: &Path) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(&mut file).ok()?;
    Some(hasher.finalize().to_hex().to_string())
}

fn mark_keep(files: &mut [DupeFile], prefer_largest: bool) {
    if files.is_empty() {
        return;
    }
    let mut best = 0usize;
    for (index, file) in files.iter().enumerate() {
        let better = if prefer_largest {
            file.size > files[best].size
                || (file.size == files[best].size && keep_score(file) < keep_score(&files[best]))
        } else {
            keep_score(file) < keep_score(&files[best])
        };
        if better {
            best = index;
        }
    }
    for (index, file) in files.iter_mut().enumerate() {
        file.keep = index == best;
    }
}

fn keep_score(file: &DupeFile) -> (i32, u64, usize) {
    let lower = file.path.to_lowercase();
    let mut penalty = 0;
    for bad in ["\\temp\\", "/temp/", "\\cache\\", "/cache/", "\\tmp\\", "/tmp/", "download", "recycle"] {
        if lower.contains(bad) {
            penalty += 10;
        }
    }
    (penalty, file.modified_ms, file.path.len())
}

fn similar_images(
    roots: &[PathBuf],
    exact: &std::collections::HashSet<String>,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(DupeProgress),
) -> Result<Vec<DupeGroup>, String> {
    let mut by_dir: HashMap<PathBuf, Vec<Meta>> = HashMap::new();
    for root in roots {
        for entry in WalkDir::new(root).skip_hidden(false).follow_links(false) {
            if cancel.load(Ordering::Relaxed) {
                return Err("busca cancelada".into());
            }
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if !is_image_ext(&path) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            if !meta.is_file() || meta.len() < IMAGE_MIN || meta.len() > 40 * 1024 * 1024 {
                continue;
            }
            let parent = path.parent().unwrap_or(Path::new("")).to_path_buf();
            let modified_ms = meta
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|time| time.as_millis() as u64)
                .unwrap_or(0);
            by_dir.entry(parent).or_default().push(Meta {
                path,
                size: meta.len(),
                modified_ms,
            });
        }
    }
    let mut groups = Vec::new();
    for files in by_dir.into_values() {
        if files.len() < 2 {
            continue;
        }
        let mut sample = files;
        if sample.len() > 800 {
            sample.sort_by(|a, b| b.size.cmp(&a.size));
            sample.truncate(800);
        }
            let hashed: Vec<(Meta, u64)> = sample
            .into_iter()
            .filter_map(|meta| {
                let _ = exact;
                let img = image::ImageReader::open(&meta.path).ok()?.decode().ok()?;
                Some((meta, dhash(&img)))
            })
            .collect();
        let n = hashed.len();
        let mut parent: Vec<usize> = (0..n).collect();
        let mut dist = vec![vec![0u32; n]; n];
        for i in 0..n {
            for j in (i + 1)..n {
                let d = hamming(hashed[i].1, hashed[j].1);
                dist[i][j] = d;
                dist[j][i] = d;
                if d > 0 && d <= HAMMING_MAX {
                    union(&mut parent, i, j);
                }
            }
        }
        let mut clusters: HashMap<usize, Vec<usize>> = HashMap::new();
        for i in 0..n {
            clusters.entry(find(&mut parent, i)).or_default().push(i);
        }
        for members in clusters.into_values() {
            if members.len() < 2 {
                continue;
            }
            let mut files = Vec::new();
            for index in &members {
                let (meta, _) = &hashed[*index];
                let distance = members
                    .iter()
                    .filter(|other| *other != index)
                    .map(|other| dist[*index][*other])
                    .min();
                files.push(DupeFile {
                    path: meta.path.display().to_string(),
                    size: meta.size,
                    modified_ms: meta.modified_ms,
                    keep: false,
                    distance,
                });
            }
            let only_identical = files.iter().all(|file| file.distance.unwrap_or(1) == 0);
            if only_identical {
                continue;
            }
            mark_keep(&mut files, true);
            let bytes = files.iter().map(|file| file.size).max().unwrap_or(0);
            groups.push(DupeGroup {
                id: nid("i"),
                kind: "imagem".into(),
                bytes,
                files,
            });
        }
        progress(DupeProgress {
            phase: "imagens parecidas".into(),
            done: groups.len() as u64,
            total: groups.len() as u64,
        });
    }
    Ok(groups)
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let pa = find(parent, a);
    let pb = find(parent, b);
    if pa != pb {
        parent[pb] = pa;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_identical_files() {
        let root = std::env::temp_dir().join(format!("bob-tudo-dupe-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("a")).unwrap();
        fs::create_dir_all(root.join("b")).unwrap();
        let body = vec![7u8; 20_000];
        fs::write(root.join("a").join("mesmo.bin"), &body).unwrap();
        fs::write(root.join("b").join("copia.bin"), &body).unwrap();
        fs::write(root.join("unico.bin"), vec![1u8; 20_000]).unwrap();
        let groups = find_duplicates(&[root.clone()], false, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
        assert_eq!(groups[0].files.iter().filter(|file| file.keep).count(), 1);
        let _ = fs::remove_dir_all(&root);
    }
}
