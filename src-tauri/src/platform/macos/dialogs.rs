use std::ffi::{CStr, c_char, c_void};
unsafe extern "C" {
    fn jot_choose_markdown() -> *mut c_char;
    fn jot_choose_css() -> *mut c_char;
    fn jot_file_error(message: *const c_char);
    fn jot_choose_folder() -> *mut c_char;
    fn free(pointer: *mut c_void);
}

fn pick_folder() -> Result<Option<String>, String> {
    chosen_text(unsafe { jot_choose_folder() })
}

fn pick_markdown() -> Result<Option<String>, String> {
    chosen_text(unsafe { jot_choose_markdown() })
}

pub fn show_error(message: &str) {
    let message = std::ffi::CString::new(message).expect("file error contains no NUL");
    unsafe { jot_file_error(message.as_ptr()) }
}

fn chosen_text(path: *mut c_char) -> Result<Option<String>, String> {
    if path.is_null() {
        return Ok(None);
    }
    let result = unsafe { CStr::from_ptr(path) }
        .to_str()
        .map(str::to_owned)
        .map_err(|_| "名称或路径不是有效的 UTF-8 文本".into());
    unsafe { free(path.cast()) };
    result.map(Some)
}

pub async fn choose() -> Result<Option<String>, String> {
    crate::windows::on_main_thread(pick_folder).await
}
pub async fn choose_markdown() -> Result<Option<String>, String> {
    crate::windows::on_main_thread(pick_markdown).await
}
pub async fn choose_css() -> Result<Option<String>, String> {
    crate::windows::on_main_thread(|| chosen_text(unsafe { jot_choose_css() })).await
}
