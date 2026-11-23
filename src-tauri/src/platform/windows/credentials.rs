use std::{io, mem, ptr, slice};
use windows_sys::Win32::{
    Foundation::{ERROR_CANCELLED, ERROR_NOT_FOUND},
    Security::Credentials::*,
};
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
// Credential Manager compares target names case-insensitively. Encode the full URL bytes
// so endpoints differing only in the case of a path cannot share a bearer token.
fn target(service: &str, url: &str) -> Vec<u16> {
    let bytes = url
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    wide(&format!("{service}/Jev/{bytes}"))
}
pub fn read(service: &str, url: &str) -> Result<Option<String>, String> {
    let mut raw = ptr::null_mut();
    if unsafe {
        CredReadW(
            target(service, url).as_ptr(),
            CRED_TYPE_GENERIC,
            0,
            &mut raw,
        )
    } == 0
    {
        let e = io::Error::last_os_error();
        return if e.raw_os_error() == Some(ERROR_NOT_FOUND as i32) {
            Ok(None)
        } else {
            Err(e.to_string())
        };
    }
    unsafe {
        let cred = &*raw;
        let key = if cred.CredentialBlob.is_null() {
            None
        } else {
            String::from_utf8(
                slice::from_raw_parts(cred.CredentialBlob, cred.CredentialBlobSize as usize)
                    .to_vec(),
            )
            .ok()
        };
        CredFree(raw.cast());
        Ok(key)
    }
}
pub fn remove(service: &str, url: &str) -> Result<(), String> {
    if unsafe { CredDeleteW(target(service, url).as_ptr(), CRED_TYPE_GENERIC, 0) } == 0 {
        let e = io::Error::last_os_error();
        if e.raw_os_error() != Some(ERROR_NOT_FOUND as i32) {
            return Err(e.to_string());
        }
    }
    Ok(())
}
pub fn prompt(service: &str, url: &str) -> Result<(), String> {
    let caption = wide("Jotiva · Jev");
    let message = wide(&format!("输入用于 {url} 的 API 密钥"));
    let info = CREDUI_INFOW {
        cbSize: mem::size_of::<CREDUI_INFOW>() as u32,
        pszCaptionText: caption.as_ptr(),
        pszMessageText: message.as_ptr(),
        ..Default::default()
    };
    let mut user = [0u16; 514];
    user[..3].copy_from_slice(&[74, 101, 118]);
    let mut secret = [0u16; 514];
    let mut save = 0;
    let status = unsafe {
        CredUIPromptForCredentialsW(
            &info,
            caption.as_ptr(),
            ptr::null(),
            0,
            user.as_mut_ptr(),
            514,
            secret.as_mut_ptr(),
            514,
            &mut save,
            CREDUI_FLAGS_GENERIC_CREDENTIALS
                | CREDUI_FLAGS_ALWAYS_SHOW_UI
                | CREDUI_FLAGS_DO_NOT_PERSIST
                | CREDUI_FLAGS_KEEP_USERNAME,
        )
    };
    if status == ERROR_CANCELLED {
        return Err("已取消配置密钥".into());
    }
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32).to_string());
    }
    let mut key = String::from_utf16_lossy(
        &secret[..secret.iter().position(|&c| c == 0).unwrap_or(secret.len())],
    )
    .into_bytes();
    secret.fill(0);
    if key.is_empty() || !key.iter().all(u8::is_ascii_graphic) {
        return Err("请输入有效的 API 密钥".into());
    }
    let mut target = target(service, url);
    let cred = CREDENTIALW {
        Type: CRED_TYPE_GENERIC,
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        TargetName: target.as_mut_ptr(),
        CredentialBlob: key.as_mut_ptr(),
        CredentialBlobSize: key.len() as u32,
        ..Default::default()
    };
    let ok = unsafe { CredWriteW(&cred, 0) };
    key.fill(0);
    if ok == 0 {
        Err(io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}
