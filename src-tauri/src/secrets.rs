//! Token do GitHub. No Windows fica no cofre de credenciais (DPAPI por baixo).
//! Se o cofre recusar, o mesmo segredo é gravado com CryptProtectData.

use std::path::Path;

#[cfg(windows)]
const TARGET: &str = "bob-tudo/github";

pub fn store_token(data_dir: &Path, token: &str) -> Result<(), String> {
    if token.trim().is_empty() {
        return Err("token vazio".into());
    }
    #[cfg(windows)]
    {
        if cred_write(token).is_ok() {
            let _ = std::fs::remove_file(blob_path(data_dir));
            return Ok(());
        }
        return dpapi_write(data_dir, token);
    }
    #[cfg(not(windows))]
    {
        let path = blob_path(data_dir);
        std::fs::write(&path, token).map_err(|err| err.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

pub fn read_token(data_dir: &Path) -> Option<String> {
    #[cfg(windows)]
    {
        if let Some(token) = cred_read() {
            return Some(token);
        }
        return dpapi_read(data_dir);
    }
    #[cfg(not(windows))]
    {
        std::fs::read_to_string(blob_path(data_dir)).ok().filter(|text| !text.trim().is_empty())
    }
}

pub fn delete_token(data_dir: &Path) {
    #[cfg(windows)]
    {
        cred_delete();
    }
    let _ = std::fs::remove_file(blob_path(data_dir));
}

fn blob_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("github.token")
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn cred_write(token: &str) -> Result<(), String> {
    use windows_sys::Win32::Security::Credentials::{CredWriteW, CREDENTIALW, CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC};
    let mut target = wide(TARGET);
    let mut user = wide("bob-tudo");
    let mut blob = token.as_bytes().to_vec();
    let cred = CREDENTIALW {
        Flags: 0,
        Type: CRED_TYPE_GENERIC,
        TargetName: target.as_mut_ptr(),
        Comment: std::ptr::null_mut(),
        LastWritten: windows_sys::Win32::Foundation::FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 },
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        AttributeCount: 0,
        Attributes: std::ptr::null_mut(),
        TargetAlias: std::ptr::null_mut(),
        UserName: user.as_mut_ptr(),
    };
    let ok = unsafe { CredWriteW(&cred, 0) };
    if ok == 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn cred_read() -> Option<String> {
    use windows_sys::Win32::Security::Credentials::{CredFree, CredReadW, CRED_TYPE_GENERIC};
    let mut target = wide(TARGET);
    let mut cred = std::ptr::null_mut();
    let ok = unsafe { CredReadW(target.as_mut_ptr(), CRED_TYPE_GENERIC, 0, &mut cred) };
    if ok == 0 || cred.is_null() {
        return None;
    }
    let token = unsafe {
        let slice = std::slice::from_raw_parts((*cred).CredentialBlob, (*cred).CredentialBlobSize as usize);
        let text = String::from_utf8_lossy(slice).to_string();
        CredFree(cred.cast::<std::ffi::c_void>());
        text
    };
    if token.trim().is_empty() { None } else { Some(token) }
}

#[cfg(windows)]
fn cred_delete() {
    use windows_sys::Win32::Security::Credentials::{CredDeleteW, CRED_TYPE_GENERIC};
    let mut target = wide(TARGET);
    unsafe {
        CredDeleteW(target.as_mut_ptr(), CRED_TYPE_GENERIC, 0);
    }
}

#[cfg(windows)]
fn dpapi_write(data_dir: &Path, token: &str) -> Result<(), String> {
    let protected = dpapi(token.as_bytes(), true)?;
    std::fs::write(blob_path(data_dir), protected).map_err(|err| err.to_string())
}

#[cfg(windows)]
fn dpapi_read(data_dir: &Path) -> Option<String> {
    let bytes = std::fs::read(blob_path(data_dir)).ok()?;
    let plain = dpapi(&bytes, false).ok()?;
    String::from_utf8(plain).ok().filter(|text| !text.trim().is_empty())
}

#[cfg(windows)]
fn dpapi(bytes: &[u8], protect: bool) -> Result<Vec<u8>, String> {
    use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CryptUnprotectData, CRYPT_INTEGER_BLOB};
    let mut input = bytes.to_vec();
    let mut in_blob = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_mut_ptr(),
    };
    let mut out_blob = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    let ok = if protect {
        unsafe { CryptProtectData(&mut in_blob, std::ptr::null(), std::ptr::null(), std::ptr::null_mut(), std::ptr::null(), 1, &mut out_blob) }
    } else {
        unsafe { CryptUnprotectData(&mut in_blob, std::ptr::null_mut(), std::ptr::null(), std::ptr::null_mut(), std::ptr::null(), 1, &mut out_blob) }
    };
    if ok == 0 || out_blob.pbData.is_null() {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let slice = unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize) };
    let owned = slice.to_vec();
    unsafe { windows_sys::Win32::Foundation::LocalFree(out_blob.pbData as *mut _) };
    Ok(owned)
}
