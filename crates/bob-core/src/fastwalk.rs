//! Lista um diretório sem seguir links. No windows usa FindFirstFileEx com
//! LARGE_FETCH e informação básica. Nos outros sistemas, read_dir.

use std::path::Path;

#[derive(Clone, Debug)]
pub struct Listed {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified_ms: u64,
}

pub fn list_dir(dir: &Path) -> Result<Vec<Listed>, String> {
    #[cfg(windows)]
    {
        list_dir_windows(dir)
    }
    #[cfg(not(windows))]
    {
        list_dir_std(dir)
    }
}

#[cfg(not(windows))]
fn list_dir_std(dir: &Path) -> Result<Vec<Listed>, String> {
    let mut out = Vec::new();
    let read = std::fs::read_dir(dir).map_err(|err| err.to_string())?;
    for entry in read {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == "." || name == ".." {
            continue;
        }
        let path = crate::norm_path(&entry.path());
        let modified_ms = meta
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|time| time.as_millis() as u64)
            .unwrap_or(0);
        out.push(Listed {
            name,
            path,
            is_dir: meta.is_dir(),
            size: if meta.is_file() { meta.len() } else { 0 },
            modified_ms,
        });
    }
    Ok(out)
}

#[cfg(windows)]
fn list_dir_windows(dir: &Path) -> Result<Vec<Listed>, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        FindClose, FindExInfoBasic, FindExSearchNameMatch, FindFirstFileExW, FindNextFileW, FILE_ATTRIBUTE_DIRECTORY,
        FILE_ATTRIBUTE_REPARSE_POINT, FIND_FIRST_EX_LARGE_FETCH, WIN32_FIND_DATAW,
    };

    let display = dir.to_string_lossy().to_string();
    let mut pattern = display.trim_end_matches(['\\', '/']).to_string();
    pattern.push_str("\\*");
    let wide: Vec<u16> = std::ffi::OsStr::new(&pattern).encode_wide().chain(std::iter::once(0)).collect();
    let mut data: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
    let handle = unsafe {
        FindFirstFileExW(
            wide.as_ptr(),
            FindExInfoBasic,
            &mut data as *mut _ as *mut _,
            FindExSearchNameMatch,
            std::ptr::null(),
            FIND_FIRST_EX_LARGE_FETCH,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err("não deu para listar a pasta".into());
    }
    let mut out = Vec::new();
    loop {
        push_find(&mut out, dir, &data);
        let more = unsafe { FindNextFileW(handle, &mut data) };
        if more == 0 {
            break;
        }
    }
    unsafe {
        FindClose(handle);
        let _ = CloseHandle(handle);
    }
    let _ = FILE_ATTRIBUTE_DIRECTORY;
    let _ = FILE_ATTRIBUTE_REPARSE_POINT;
    Ok(out)
}

#[cfg(windows)]
fn push_find(out: &mut Vec<Listed>, dir: &Path, data: &windows_sys::Win32::Storage::FileSystem::WIN32_FIND_DATAW) {
    use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT};
    let attrs = data.dwFileAttributes;
    if attrs & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return;
    }
    let name = wide_name(&data.cFileName);
    if name.is_empty() || name == "." || name == ".." {
        return;
    }
    let path = crate::norm_path(&dir.join(&name));
    let is_dir = attrs & FILE_ATTRIBUTE_DIRECTORY != 0;
    let size = if is_dir {
        0
    } else {
        ((data.nFileSizeHigh as u64) << 32) | data.nFileSizeLow as u64
    };
    out.push(Listed {
        name,
        path,
        is_dir,
        size,
        modified_ms: filetime_ms(data.ftLastWriteTime.dwLowDateTime, data.ftLastWriteTime.dwHighDateTime),
    });
}

#[cfg(windows)]
fn wide_name(buf: &[u16]) -> String {
    let end = buf.iter().position(|ch| *ch == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

#[cfg(windows)]
fn filetime_ms(low: u32, high: u32) -> u64 {
    let ticks = ((high as u64) << 32) | low as u64;
    ticks.saturating_sub(116_444_736_000_000_000) / 10_000
}

pub fn dir_mtime_ms(path: &Path) -> u64 {
    std::fs::metadata(path)
        .ok()
        .and_then(|meta| meta.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|time| time.as_millis() as u64)
        .unwrap_or(0)
}
