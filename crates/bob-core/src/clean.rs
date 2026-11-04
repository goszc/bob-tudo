use std::fs;
use std::path::Path;

use jwalk::WalkDir;
use serde::Serialize;

use crate::guard::protected_path;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeReport {
    pub bytes: u64,
    pub files: u64,
    pub skipped: u64,
}

pub fn measure_paths(paths: &[impl AsRef<Path>]) -> (u64, u64) {
    let mut bytes = 0u64;
    let mut files = 0u64;
    for path in paths {
        let path = path.as_ref();
        if is_link(path) {
            continue;
        }
        if path.is_file() {
            bytes += path.metadata().map(|meta| meta.len()).unwrap_or(0);
            files += 1;
            continue;
        }
        if !path.is_dir() {
            continue;
        }
        for entry in WalkDir::new(path).skip_hidden(false).follow_links(false) {
            let Ok(entry) = entry else { continue };
            if entry.file_type().is_file() {
                bytes += entry.metadata().map(|meta| meta.len()).unwrap_or(0);
                files += 1;
            }
        }
    }
    (bytes, files)
}

pub fn purge_paths(paths: &[impl AsRef<Path>]) -> PurgeReport {
    let mut report = PurgeReport {
        bytes: 0,
        files: 0,
        skipped: 0,
    };
    for path in paths {
        let path = path.as_ref();
        if protected_path(path) || is_link(path) {
            report.skipped += 1;
            continue;
        }
        if path.is_file() {
            let len = path.metadata().map(|meta| meta.len()).unwrap_or(0);
            if fs::remove_file(path).is_ok() {
                report.bytes += len;
                report.files += 1;
            } else {
                report.skipped += 1;
            }
            continue;
        }
        if path.is_dir() {
            purge_dir(path, &mut report);
        }
    }
    report
}

fn purge_dir(path: &Path, report: &mut PurgeReport) {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => {
            report.skipped += 1;
            return;
        }
    };
    for entry in entries.flatten() {
        let child = entry.path();
        if protected_path(&child) || is_link(&child) {
            report.skipped += 1;
            continue;
        }
        if child.is_dir() {
            purge_dir(&child, report);
            let _ = fs::remove_dir(&child);
        } else if child.is_file() {
            let len = child.metadata().map(|meta| meta.len()).unwrap_or(0);
            if fs::remove_file(&child).is_ok() {
                report.bytes += len;
                report.files += 1;
            } else {
                report.skipped += 1;
            }
        }
    }
}

fn is_link(path: &Path) -> bool {
    fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn measures_and_purges() {
        let root = std::env::temp_dir().join(format!("bob-tudo-clean-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.tmp"), vec![3u8; 1000]).unwrap();
        let (bytes, files) = measure_paths(&[root.clone()]);
        assert_eq!(files, 1);
        assert_eq!(bytes, 1000);
        let report = purge_paths(&[root.join("a.tmp")]);
        assert_eq!(report.files, 1);
        assert!(!root.join("a.tmp").exists());
        let _ = fs::remove_dir_all(&root);
    }
}
