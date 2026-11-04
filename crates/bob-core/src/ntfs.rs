//! Leitura rápida do NTFS. O parser do registro MFT é puro e testável.
//! A leitura do volume só existe no windows e exige privilégio (backup/admin).
//! Se falhar, o mapa cai para a enumeração paralela de diretórios.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct FlatNode {
    pub path: String,
    pub name: String,
    pub parent: String,
    pub size: u64,
    pub is_dir: bool,
    pub mtime_ms: u64,
    pub record: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct UsnCursor {
    pub journal_id: u64,
    pub next_usn: i64,
    pub dirs: Vec<FrnPath>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FrnPath {
    pub record: u64,
    pub path: String,
}

#[derive(Clone, Debug)]
struct Parsed {
    record: u64,
    parent: u64,
    name: String,
    size: u64,
    is_dir: bool,
    in_use: bool,
}

pub fn parse_records(bytes: &[u8], record_size: usize) -> Vec<Parsed> {
    if record_size < 64 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut offset = 0usize;
    let mut record = 0u64;
    while offset + record_size <= bytes.len() {
        if let Some(parsed) = parse_record(&bytes[offset..offset + record_size], record) {
            out.push(parsed);
        }
        offset += record_size;
        record += 1;
    }
    out
}

fn parse_record(raw: &[u8], record: u64) -> Option<Parsed> {
    if raw.len() < 64 || &raw[0..4] != b"FILE" {
        return None;
    }
    let mut buf = raw.to_vec();
    apply_fixup(&mut buf)?;
    let flags = u16_at(&buf, 22)?;
    let in_use = flags & 0x01 != 0;
    if !in_use {
        return None;
    }
    let is_dir = flags & 0x02 != 0;
    let mut attr = u16_at(&buf, 20)? as usize;
    let mut size = 0u64;
    let mut best: Option<(u64, String, u8)> = None;
    while attr + 8 <= buf.len() {
        let kind = u32_at(&buf, attr)?;
        if kind == 0xffff_ffff {
            break;
        }
        let len = u32_at(&buf, attr + 4)? as usize;
        if len < 24 || attr + len > buf.len() {
            break;
        }
        let non_resident = buf[attr + 8] != 0;
        let name_len = buf[attr + 9] as usize;
        let name_off = u16_at(&buf, attr + 10)? as usize;
        if kind == 0x80 && name_len == 0 {
            if !non_resident {
                let value_len = u32_at(&buf, attr + 16)? as u64;
                size = size.saturating_add(value_len);
            } else if attr + 56 <= attr + len {
                size = size.saturating_add(u64_at(&buf, attr + 48).unwrap_or(0));
            }
        }
        if kind == 0x30 && !non_resident {
            let value_off = attr + u16_at(&buf, attr + 20)? as usize;
            let value_len = u32_at(&buf, attr + 16)? as usize;
            if value_off + 66 <= buf.len() && value_len >= 66 {
                let parent = u64_at(&buf, value_off)? & 0x0000_ffff_ffff_ffff;
                let name_len = buf[value_off + 64] as usize;
                let namespace = buf[value_off + 65];
                let name_pos = value_off + 66;
                let name_end = name_pos + name_len * 2;
                if name_end <= buf.len() {
                    let name = utf16_at(&buf[name_pos..name_end]);
                    if !name.is_empty() {
                        let rank = match namespace {
                            1 | 3 => 0,
                            0 => 1,
                            _ => 2,
                        };
                        let replace = best.as_ref().map(|(_, _, old)| rank < *old).unwrap_or(true);
                        if replace {
                            best = Some((parent, name, rank));
                        }
                    }
                }
            }
        }
        let _ = name_off;
        attr += len;
    }
    let (parent, name, _) = best?;
    Some(Parsed {
        record,
        parent,
        name,
        size,
        is_dir,
        in_use: true,
    })
}

fn apply_fixup(buf: &mut [u8]) -> Option<()> {
    let usa_off = u16_at(buf, 4)? as usize;
    let usa_count = u16_at(buf, 6)? as usize;
    if usa_count < 2 || usa_off + usa_count * 2 > buf.len() {
        return Some(());
    }
    let seq = u16_at(buf, usa_off)?;
    for index in 1..usa_count {
        let sector_end = index * 512;
        if sector_end > buf.len() || sector_end < 2 {
            return None;
        }
        let found = u16_at(buf, sector_end - 2)?;
        if found != seq {
            return None;
        }
        let fix = u16_at(buf, usa_off + index * 2)?;
        buf[sector_end - 2] = (fix & 0xff) as u8;
        buf[sector_end - 1] = (fix >> 8) as u8;
    }
    Some(())
}

pub fn nodes_from_records(records: &[Parsed], volume: &str) -> Vec<FlatNode> {
    use std::collections::HashMap;
    let mut by_id: HashMap<u64, &Parsed> = HashMap::new();
    for record in records {
        by_id.insert(record.record, record);
    }
    let root = volume_root(volume);
    let mut paths: HashMap<u64, String> = HashMap::new();
    paths.insert(5, root.clone());
    let mut out = Vec::new();
    for record in records {
        if record.record < 16 && record.name.starts_with('$') {
            continue;
        }
        let parent_path = resolve_parent(record.parent, &by_id, &mut paths, &root, 0);
        let path = join_path(&parent_path, &record.name);
        if record.is_dir {
            paths.insert(record.record, path.clone());
        }
        out.push(FlatNode {
            path,
            name: record.name.clone(),
            parent: parent_path,
            size: if record.is_dir { 0 } else { record.size },
            is_dir: record.is_dir,
            mtime_ms: 0,
            record: record.record,
        });
    }
    out
}

fn resolve_parent<'a>(
    mut id: u64,
    by_id: &std::collections::HashMap<u64, &'a Parsed>,
    paths: &mut std::collections::HashMap<u64, String>,
    root: &str,
    depth: u8,
) -> String {
    if let Some(found) = paths.get(&id) {
        return found.clone();
    }
    if depth > 40 {
        return root.to_string();
    }
    if id == 5 || id == 0 {
        paths.insert(id, root.to_string());
        return root.to_string();
    }
    let Some(node) = by_id.get(&id) else {
        return root.to_string();
    };
    let parent = resolve_parent(node.parent, by_id, paths, root, depth + 1);
    let path = join_path(&parent, &node.name);
    paths.insert(id, path.clone());
    path
}

fn volume_root(volume: &str) -> String {
    let trimmed = volume.trim().trim_end_matches(['\\', '/']);
    if trimmed.len() == 2 && trimmed.as_bytes()[1] == b':' {
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

pub fn decode_runs(data: &[u8]) -> Vec<(i64, u64)> {
    let mut out = Vec::new();
    let mut index = 0usize;
    let mut lcn: i64 = 0;
    while index < data.len() {
        let header = data[index];
        index += 1;
        if header == 0 {
            break;
        }
        let len_len = (header & 0x0f) as usize;
        let off_len = (header >> 4) as usize;
        if len_len == 0 || index + len_len + off_len > data.len() {
            break;
        }
        let mut length = 0u64;
        for (byte, value) in data[index..index + len_len].iter().enumerate() {
            length |= (*value as u64) << (8 * byte);
        }
        index += len_len;
        let mut offset = 0i64;
        if off_len > 0 {
            for (byte, value) in data[index..index + off_len].iter().enumerate() {
                offset |= (*value as i64) << (8 * byte);
            }
            let sign = 1i64 << (off_len * 8 - 1);
            if offset & sign != 0 {
                offset |= -1i64 << (off_len * 8);
            }
            lcn = lcn.saturating_add(offset);
        }
        index += off_len;
        if length > 0 && off_len > 0 {
            out.push((lcn, length));
        }
    }
    out
}

pub const REASON_DATA_OVERWRITE: u32 = 0x0000_0001;
pub const REASON_DATA_EXTEND: u32 = 0x0000_0002;
pub const REASON_DATA_TRUNCATION: u32 = 0x0000_0004;
pub const REASON_FILE_CREATE: u32 = 0x0000_0100;
pub const REASON_FILE_DELETE: u32 = 0x0000_0200;
pub const REASON_RENAME_OLD: u32 = 0x0000_1000;
pub const REASON_RENAME_NEW: u32 = 0x0000_2000;
pub const REASON_CLOSE: u32 = 0x8000_0000;
pub const ATTR_DIRECTORY: u32 = 0x10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UsnEvent {
    pub record: u64,
    pub parent: u64,
    pub usn: i64,
    pub reason: u32,
    pub attributes: u32,
    pub time_ms: u64,
    pub name: String,
}

/// O buffer do FSCTL_READ_USN_JOURNAL começa com o próximo USN (8 bytes)
/// e segue com registros V2.
pub fn parse_usn_payload(buf: &[u8]) -> (i64, Vec<UsnEvent>) {
    if buf.len() < 8 {
        return (0, Vec::new());
    }
    let next = i64::from_le_bytes(buf[0..8].try_into().unwrap_or([0; 8]));
    (next, parse_usn_records(&buf[8..]))
}

pub fn parse_usn_records(records: &[u8]) -> Vec<UsnEvent> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    while offset + 60 <= records.len() {
        let length = u32_at(records, offset).unwrap_or(0) as usize;
        if length < 60 || offset + length > records.len() {
            break;
        }
        let major = u16_at(records, offset + 4).unwrap_or(0);
        if major <= 2 {
            let name_len = u16_at(records, offset + 56).unwrap_or(0) as usize;
            let name_off = u16_at(records, offset + 58).unwrap_or(60) as usize;
            let name = if name_off + name_len <= length {
                utf16_at(&records[offset + name_off..offset + name_off + name_len])
            } else {
                String::new()
            };
            out.push(UsnEvent {
                record: u64_at(records, offset + 8).unwrap_or(0) & 0x0000_ffff_ffff_ffff,
                parent: u64_at(records, offset + 16).unwrap_or(0) & 0x0000_ffff_ffff_ffff,
                usn: i64::from_le_bytes(records.get(offset + 24..offset + 32).and_then(|b| b.try_into().ok()).unwrap_or([0; 8])),
                reason: u32_at(records, offset + 40).unwrap_or(0),
                attributes: u32_at(records, offset + 52).unwrap_or(0),
                time_ms: filetime_ms(i64::from_le_bytes(records.get(offset + 32..offset + 40).and_then(|b| b.try_into().ok()).unwrap_or([0; 8]))),
                name,
            });
        }
        let step = length.saturating_add(7) & !7;
        if step == 0 {
            break;
        }
        offset += step;
    }
    out
}

fn filetime_ms(raw: i64) -> u64 {
    if raw <= 0 {
        return 0;
    }
    let unix = raw / 10_000 - 116_444_736_00000;
    if unix <= 0 { 0 } else { unix as u64 }
}

/// Aplica um buffer de USN_RECORD_V2 (depois dos 8 bytes do próximo USN)
/// e devolve os FRNs de diretório pai que mudaram.
pub fn dirty_parents(records: &[u8]) -> Vec<u64> {
    let mut out: Vec<u64> = parse_usn_records(records).into_iter().map(|event| event.parent).filter(|parent| *parent > 0).collect();
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(windows)]
pub fn try_scan_volume(root: &std::path::Path) -> Option<(Vec<FlatNode>, UsnCursor)> {
    let letter = drive_letter(root)?;
    let nodes = read_mft(letter)?;
    let cursor = query_usn(letter).unwrap_or_default();
    let dirs = nodes
        .iter()
        .filter(|node| node.is_dir)
        .map(|node| FrnPath {
            record: node.record,
            path: node.path.clone(),
        })
        .collect();
    Some((
        nodes,
        UsnCursor {
            journal_id: cursor.0,
            next_usn: cursor.1,
            dirs,
        },
    ))
}

#[cfg(windows)]
fn drive_letter(root: &std::path::Path) -> Option<char> {
    let text = root.to_string_lossy();
    let mut chars = text.chars();
    let letter = chars.next()?;
    if chars.next() == Some(':') && letter.is_ascii_alphabetic() {
        Some(letter.to_ascii_uppercase())
    } else {
        None
    }
}

#[cfg(windows)]
fn read_mft(letter: char) -> Option<Vec<FlatNode>> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ, HANDLE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;

    const FSCTL_GET_NTFS_VOLUME_DATA: u32 = 0x0009_0064;
    let path = format!("\\\\.\\{letter}:");
    let wide: Vec<u16> = std::ffi::OsStr::new(&path).encode_wide().chain(std::iter::once(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        return None;
    }
    let result = unsafe { read_mft_handle(handle, letter, FSCTL_GET_NTFS_VOLUME_DATA) };
    unsafe {
        let _ = CloseHandle(handle);
    }
    result
}

#[cfg(windows)]
unsafe fn read_mft_handle(handle: windows_sys::Win32::Foundation::HANDLE, letter: char, code: u32) -> Option<Vec<FlatNode>> {
    use windows_sys::Win32::System::IO::DeviceIoControl;
    let mut volume = [0u8; 128];
    let mut returned = 0u32;
    let ok = DeviceIoControl(
        handle,
        code,
        std::ptr::null(),
        0,
        volume.as_mut_ptr() as *mut _,
        volume.len() as u32,
        &mut returned,
        std::ptr::null_mut(),
    );
    if ok == 0 || returned < 96 {
        return None;
    }
    let bytes_per_cluster = u32::from_le_bytes(volume[44..48].try_into().ok()?) as u64;
    let bytes_per_record = u32::from_le_bytes(volume[48..52].try_into().ok()?) as usize;
    let mft_len = u64::from_le_bytes(volume[56..64].try_into().ok()?);
    let mft_lcn = u64::from_le_bytes(volume[64..72].try_into().ok()?);
    if bytes_per_cluster == 0 || bytes_per_record < 512 || mft_len == 0 {
        return None;
    }
    let start = mft_lcn.saturating_mul(bytes_per_cluster);
    let to_read = mft_len.min(512 * 1024 * 1024) as usize;
    let mut buf = vec![0u8; to_read];
    if !read_at(handle, start, &mut buf) {
        return None;
    }
    let records = parse_records(&buf, bytes_per_record);
    if records.is_empty() {
        return None;
    }
    let _ = letter;
    Some(nodes_from_records(&records, &format!("{letter}:")))
}

#[cfg(windows)]
unsafe fn read_at(handle: windows_sys::Win32::Foundation::HANDLE, offset: u64, buf: &mut [u8]) -> bool {
    use windows_sys::Win32::Storage::FileSystem::{ReadFile, SetFilePointerEx};
    let mut moved = 0i64;
    let pos = offset as i64;
    if SetFilePointerEx(handle, pos, &mut moved, 0) == 0 {
        return false;
    }
    let mut done = 0usize;
    while done < buf.len() {
        let mut got = 0u32;
        let ok = ReadFile(
            handle,
            buf[done..].as_mut_ptr(),
            (buf.len() - done).min(8 * 1024 * 1024) as u32,
            &mut got,
            std::ptr::null_mut(),
        );
        if ok == 0 || got == 0 {
            return done > 0;
        }
        done += got as usize;
    }
    true
}

#[cfg(windows)]
fn query_usn(letter: char) -> Option<(u64, i64)> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;
    const FSCTL_QUERY_USN_JOURNAL: u32 = 0x0009_00f4;
    let path = format!("\\\\.\\{letter}:");
    let wide: Vec<u16> = std::ffi::OsStr::new(&path).encode_wide().chain(std::iter::once(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        return None;
    }
    let mut out = [0u8; 80];
    let mut returned = 0u32;
    let ok = unsafe {
        DeviceIoControl(
            handle,
            FSCTL_QUERY_USN_JOURNAL,
            std::ptr::null(),
            0,
            out.as_mut_ptr() as *mut _,
            out.len() as u32,
            &mut returned,
            std::ptr::null_mut(),
        )
    };
    unsafe {
        let _ = CloseHandle(handle);
    }
    if ok == 0 || returned < 16 {
        return None;
    }
    let journal_id = u64::from_le_bytes(out[0..8].try_into().ok()?);
    let next_usn = i64::from_le_bytes(out[8..16].try_into().ok()?);
    Some((journal_id, next_usn))
}

#[cfg(windows)]
pub fn read_usn_events(letter: char, journal_id: u64, start: i64) -> Option<(i64, Vec<UsnEvent>)> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::{CloseHandle, GENERIC_READ};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows_sys::Win32::System::IO::DeviceIoControl;
    const FSCTL_READ_USN_JOURNAL: u32 = 0x0009_00bb;
    let current = query_usn(letter)?;
    if current.0 != journal_id {
        return None;
    }
    let path = format!("\\\\.\\{letter}:");
    let wide: Vec<u16> = std::ffi::OsStr::new(&path).encode_wide().chain(std::iter::once(0)).collect();
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            std::ptr::null(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };
    if handle == windows_sys::Win32::Foundation::INVALID_HANDLE_VALUE {
        return None;
    }
    let mut cursor = start;
    let mut events = Vec::new();
    let mut guard = 0;
    while guard < 8 && cursor < current.1 {
        let mut input = [0u8; 40];
        input[0..8].copy_from_slice(&cursor.to_le_bytes());
        input[8..12].copy_from_slice(&0xffff_ffffu32.to_le_bytes());
        input[32..40].copy_from_slice(&journal_id.to_le_bytes());
        let mut output = vec![0u8; 256 * 1024];
        let mut returned = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                handle,
                FSCTL_READ_USN_JOURNAL,
                input.as_ptr() as *const _,
                input.len() as u32,
                output.as_mut_ptr() as *mut _,
                output.len() as u32,
                &mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 || returned < 8 {
            unsafe { let _ = CloseHandle(handle); }
            return None;
        }
        output.truncate(returned as usize);
        let (next, batch) = parse_usn_payload(&output);
        if next <= cursor && batch.is_empty() {
            break;
        }
        events.extend(batch);
        cursor = next;
        guard += 1;
        if events.len() > 200_000 {
            unsafe { let _ = CloseHandle(handle); }
            return None;
        }
    }
    unsafe { let _ = CloseHandle(handle); }
    Some((cursor.max(current.1.min(cursor)), events))
}

fn u16_at(buf: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(buf.get(offset..offset + 2)?.try_into().ok()?))
}

fn u32_at(buf: &[u8], offset: usize) -> Option<u32> {
    Some(u32::from_le_bytes(buf.get(offset..offset + 4)?.try_into().ok()?))
}

fn u64_at(buf: &[u8], offset: usize) -> Option<u64> {
    Some(u64::from_le_bytes(buf.get(offset..offset + 8)?.try_into().ok()?))
}

fn utf16_at(buf: &[u8]) -> String {
    let units: Vec<u16> = buf.chunks_exact(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect();
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_a_simple_runlist() {
        let runs = decode_runs(&[0x21, 0x20, 0x01, 0x00, 0x00]);
        assert_eq!(runs, vec![(1, 32)]);
    }

    #[test]
    fn parses_a_file_record_and_builds_a_path() {
        let record = sample_record();
        let parsed = parse_records(&record, 1024);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "notas.txt");
        assert_eq!(parsed[0].parent, 5);
        assert_eq!(parsed[0].size, 13);
        assert!(!parsed[0].is_dir);
        let nodes = nodes_from_records(&parsed, "C:");
        assert_eq!(nodes[0].path, "C:\\notas.txt");
        assert_eq!(nodes[0].parent, "C:\\");
    }

    #[test]
    fn usn_buffer_marks_parent_frn() {
        let mut buf = vec![0u8; 64];
        buf[0] = 64;
        buf[16] = 5;
        let parents = dirty_parents(&buf);
        assert_eq!(parents, vec![5]);
    }

    #[test]
    fn usn_payload_reads_name_reason_and_next() {
        let mut rec = vec![0u8; 80];
        rec[0..4].copy_from_slice(&80u32.to_le_bytes());
        rec[4] = 2;
        rec[8] = 20;
        rec[16] = 10;
        rec[24] = 100;
        rec[40..44].copy_from_slice(&REASON_FILE_CREATE.to_le_bytes());
        let name: Vec<u16> = "nota.txt".encode_utf16().collect();
        rec[56..58].copy_from_slice(&((name.len() * 2) as u16).to_le_bytes());
        rec[58..60].copy_from_slice(&60u16.to_le_bytes());
        for (index, unit) in name.iter().enumerate() {
            let pos = 60 + index * 2;
            rec[pos..pos + 2].copy_from_slice(&unit.to_le_bytes());
        }
        let mut payload = 500i64.to_le_bytes().to_vec();
        payload.extend(rec);
        let (next, events) = parse_usn_payload(&payload);
        assert_eq!(next, 500);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].name, "nota.txt");
        assert_eq!(events[0].record, 20);
        assert_eq!(events[0].parent, 10);
        assert_eq!(events[0].reason, REASON_FILE_CREATE);
    }

    fn sample_record() -> Vec<u8> {
        let mut rec = vec![0u8; 1024];
        rec[0..4].copy_from_slice(b"FILE");
        rec[4] = 0x30;
        rec[6] = 3;
        rec[20] = 56;
        rec[22] = 1;
        rec[48] = 0x11;
        rec[49] = 0x22;
        rec[510] = 0x11;
        rec[511] = 0x22;
        rec[1022] = 0x11;
        rec[1023] = 0x22;
        write_filename(&mut rec, 56, 5, "notas.txt");
        let data_at = 56 + attr_len(&rec, 56);
        write_data(&mut rec, data_at, b"ola, bob tudo");
        rec
    }

    fn write_filename(buf: &mut [u8], at: usize, parent: u64, name: &str) {
        let units: Vec<u16> = name.encode_utf16().collect();
        let value_len = 66 + units.len() * 2;
        let len = (24 + value_len + 7) & !7;
        buf[at..at + 4].copy_from_slice(&0x30u32.to_le_bytes());
        buf[at + 4..at + 8].copy_from_slice(&(len as u32).to_le_bytes());
        buf[at + 16..at + 20].copy_from_slice(&(value_len as u32).to_le_bytes());
        buf[at + 20] = 24;
        let value = at + 24;
        buf[value..value + 8].copy_from_slice(&parent.to_le_bytes());
        buf[value + 64] = units.len() as u8;
        buf[value + 65] = 1;
        for (index, unit) in units.iter().enumerate() {
            let pos = value + 66 + index * 2;
            buf[pos..pos + 2].copy_from_slice(&unit.to_le_bytes());
        }
    }

    fn write_data(buf: &mut [u8], at: usize, data: &[u8]) {
        let len = (24 + data.len() + 7) & !7;
        buf[at..at + 4].copy_from_slice(&0x80u32.to_le_bytes());
        buf[at + 4..at + 8].copy_from_slice(&(len as u32).to_le_bytes());
        buf[at + 16..at + 20].copy_from_slice(&(data.len() as u32).to_le_bytes());
        buf[at + 20] = 24;
        buf[at + 24..at + 24 + data.len()].copy_from_slice(data);
        let end = at + len;
        if end + 4 <= buf.len() {
            buf[end..end + 4].copy_from_slice(&0xffff_ffffu32.to_le_bytes());
        }
    }

    fn attr_len(buf: &[u8], at: usize) -> usize {
        u32::from_le_bytes(buf[at + 4..at + 8].try_into().unwrap()) as usize
    }
}
