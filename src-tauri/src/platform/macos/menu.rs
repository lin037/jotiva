use std::ffi::{CString, c_char, c_void};

unsafe extern "C" {
    fn jot_popup_menu(
        window: *mut c_void,
        labels: *const *const c_char,
        count: usize,
        x: f64,
        y: f64,
    ) -> isize;
    fn jot_copy_text(text: *const c_char) -> bool;
}

#[tauri::command]
pub fn context_menu(
    window: tauri::WebviewWindow,
    labels: Vec<Option<String>>,
    x: f64,
    y: f64,
) -> Result<Option<usize>, String> {
    let strings: Vec<_> = labels
        .into_iter()
        .map(|label| label.map(CString::new).transpose())
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    let labels: Vec<_> = strings
        .iter()
        .map(|label| label.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()))
        .collect();
    let index = unsafe {
        jot_popup_menu(
            window.ns_window().map_err(|e| e.to_string())?,
            labels.as_ptr(),
            labels.len(),
            x,
            y,
        )
    };
    Ok(usize::try_from(index).ok())
}

#[tauri::command]
pub fn copy_text(text: String) -> Result<(), String> {
    let text = CString::new(text).map_err(|e| e.to_string())?;
    if unsafe { jot_copy_text(text.as_ptr()) } {
        Ok(())
    } else {
        Err("无法写入剪贴板".into())
    }
}
