use crate::library::workspace::{drop_paths, is_document};
use std::path::PathBuf;
use tauri::AppHandle;

unsafe extern "C" {
    fn jot_dropped_file_paths() -> *mut std::ffi::c_char;
    fn free(pointer: *mut std::ffi::c_void);
}

#[tauri::command]
pub async fn open_dropped_markdown(app: AppHandle, names: Vec<String>) -> Result<(), String> {
    let paths = crate::windows::on_main_thread(|| {
        let raw = unsafe { jot_dropped_file_paths() };
        if raw.is_null() {
            return Err("无法读取本次拖放".into());
        }
        let paths = serde_json::from_slice::<Vec<PathBuf>>(
            unsafe { std::ffi::CStr::from_ptr(raw) }.to_bytes(),
        );
        unsafe { free(raw.cast()) };
        paths.map_err(|e| e.to_string())
    })
    .await?;
    let paths: Vec<_> = paths.into_iter().filter(|p| is_document(p)).collect();
    let mut received: Vec<_> = paths
        .iter()
        .filter_map(|p| p.file_name()?.to_str())
        .collect();
    received.sort_unstable();
    let mut names = names;
    names.sort_unstable();
    if paths.is_empty() || received != names {
        return Err("无法确认这次拖入的文件，请重新拖放".into());
    }
    drop_paths(app, paths).await
}
