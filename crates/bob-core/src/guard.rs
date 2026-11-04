use std::path::Path;

/// Pastas que o bob tudo se recusa a apagar, mesmo com confirmação.
pub fn protected_path(path: &Path) -> bool {
    if let Some(text) = path.to_str() {
        let unix = text.trim_end_matches('/');
        const UNIX: &[&str] = &[
            "", "/", "/usr", "/bin", "/sbin", "/etc", "/var", "/home", "/sys", "/proc", "/dev",
            "/boot", "/lib", "/lib64", "/opt", "/root",
        ];
        if UNIX.contains(&unix) {
            return true;
        }
    }
    let raw = path.to_string_lossy().replace('/', "\\");
    let mut lower = raw.trim().trim_end_matches('\\').to_lowercase();
    if lower.starts_with(r"\\?\") {
        lower = lower.trim_start_matches(r"\\?\").to_string();
    }
    let parts: Vec<&str> = lower.split('\\').filter(|part| !part.is_empty()).collect();
    if parts.len() <= 1 {
        return true;
    }
    let joined = parts.join("\\");
    const EXACT: &[&str] = &[
        "c:\\windows",
        "c:\\windows\\system32",
        "c:\\windows\\syswow64",
        "c:\\windows\\winsxs",
        "c:\\windows\\boot",
        "c:\\program files",
        "c:\\program files (x86)",
        "c:\\programdata",
        "c:\\users",
        "c:\\program files\\windowsapps",
    ];
    if EXACT.contains(&joined.as_str()) {
        return true;
    }
    const PREFIX: &[&str] = &[
        "c:\\windows\\system32\\",
        "c:\\windows\\syswow64\\",
        "c:\\windows\\winsxs\\",
        "c:\\windows\\boot\\",
        "c:\\windows\\fonts\\",
    ];
    if PREFIX.iter().any(|prefix| joined.starts_with(prefix)) {
        return true;
    }
    if joined.contains("\\system volume information") {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_system_and_allows_apps() {
        assert!(protected_path(Path::new(r"C:\Windows")));
        assert!(protected_path(Path::new(r"C:\Windows\System32\cmd.exe")));
        assert!(protected_path(Path::new(r"C:\Program Files")));
        assert!(protected_path(Path::new("/")));
        assert!(protected_path(Path::new("/usr")));
        assert!(!protected_path(Path::new(r"C:\Program Files\VLC")));
        assert!(!protected_path(Path::new(r"C:\Users\ana\AppData\Local\VLC")));
        assert!(!protected_path(Path::new("/tmp/vlc-leftover")));
    }
}
