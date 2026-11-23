// Compiled only into the temporary QA application.
use std::ffi::{CString, c_char, c_void};

unsafe extern "C" {
    fn jot_qa_menu(window: *mut c_void, request: *const c_char) -> i32;
}

#[tauri::command]
pub async fn qa_frame(window: tauri::WebviewWindow, action: String) -> Result<bool, String> {
    let action = CString::new(action).map_err(|e| e.to_string())?;
    let (send, receive) = tokio::sync::oneshot::channel();
    let caller = window.clone();
    window
        .run_on_main_thread(move || {
            let result = caller
                .ns_window()
                .map(|w| unsafe { jot_qa_menu(w, action.as_ptr()) });
            let _ = send.send(result);
        })
        .map_err(|e| e.to_string())?;
    match receive
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?
    {
        0 => Ok(false),
        1 => Ok(true),
        code => Err(format!("Native shortcut probe failed ({code})")),
    }
}
