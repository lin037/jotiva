use crate::settings::filing::KeyAction;
use serde_json::Value;
use std::ffi::{CStr, CString, c_char, c_void};
unsafe extern "C" {
    fn jot_filing_credential(service: *const c_char, endpoint: *const c_char, action: i32) -> i32;
    fn jot_read_endpoint_key(
        service: *const c_char,
        endpoint: *const c_char,
        out: *mut *mut c_char,
    ) -> i32;
    fn jot_filing_post(
        service: *const c_char,
        endpoint: *const c_char,
        payload: *const c_char,
        reply: *mut *mut c_char,
    ) -> i64;
    fn free(pointer: *mut c_void);
}

// The C client allocates a NUL-terminated response only on success.
fn native_response(
    payload: &Value,
    post: impl FnOnce(&CStr, *mut *mut c_char) -> i64,
) -> (i64, Vec<u8>) {
    let json = CString::new(payload.to_string()).unwrap();
    let mut reply = std::ptr::null_mut();
    let code = post(&json, &mut reply);
    if reply.is_null() {
        return (code, Vec::new());
    }
    let bytes = unsafe { CStr::from_ptr(reply) }.to_bytes().to_vec();
    unsafe { free(reply.cast()) };
    (code, bytes)
}

pub fn credential(service: &str, url: &str, action: KeyAction) -> Result<bool, String> {
    let service = CString::new(service).unwrap();
    let address = CString::new(url).unwrap();
    let has_key =
        match unsafe { jot_filing_credential(service.as_ptr(), address.as_ptr(), action as i32) } {
            0 => false,
            1 => true,
            -2 => return Err("已取消配置密钥".into()),
            -3 => return Err("密钥须由可打印的 ASCII 字符组成，不能包含空格和换行".into()),
            _ => return Err("无法访问 macOS 钥匙串，请检查 Jotiva 的访问权限".into()),
        };
    Ok(has_key)
}
pub fn post(service: &str, url: &str, payload: &Value) -> (i64, Vec<u8>) {
    let service = CString::new(service).unwrap();
    let url = CString::new(url).unwrap();
    native_response(payload, |json, reply| unsafe {
        jot_filing_post(service.as_ptr(), url.as_ptr(), json.as_ptr(), reply)
    })
}
#[cfg(test)]
pub fn post_fixture(endpoint: &str, key: &str, payload: &Value) -> (i64, Vec<u8>) {
    unsafe extern "C" {
        fn jot_filing_http(
            endpoint: *const c_char,
            key: *const c_char,
            payload: *const c_char,
            timeout: std::ffi::c_long,
            reply: *mut *mut c_char,
        ) -> i64;
    }
    let url = CString::new(endpoint).unwrap();
    let key = CString::new(key).unwrap();
    native_response(payload, |json, reply| unsafe {
        jot_filing_http(url.as_ptr(), key.as_ptr(), json.as_ptr(), 25, reply)
    })
}

pub fn read_token(service: &str, url: &str) -> Result<Option<String>, String> {
    let service = CString::new(service).map_err(|e| e.to_string())?;
    let url = CString::new(url).map_err(|e| e.to_string())?;
    let mut out = std::ptr::null_mut();
    match unsafe { jot_read_endpoint_key(service.as_ptr(), url.as_ptr(), &mut out) } {
        0 => Ok(None),
        1 => {
            let token = unsafe { CStr::from_ptr(out) }
                .to_string_lossy()
                .into_owned();
            unsafe { free(out.cast()) };
            Ok(Some(token))
        }
        _ => Err("无法读取图片上传密钥".into()),
    }
}
