pub(crate) struct Native;
pub mod dialogs;
pub mod file_drop;
mod files;
pub mod filing;
mod host;
pub mod menu;
pub mod shortcuts;
pub mod storage;
pub mod window;
pub fn builder() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default().plugin(tauri_nspanel::init())
}
pub fn startup_files() -> Vec<std::path::PathBuf> {
    Vec::new()
}
unsafe extern "C" {
    fn jot_system_font_size() -> f64;
}
pub fn system_font_size() -> f64 {
    unsafe { jot_system_font_size() }
}

pub(crate) use host::{pointer, screen_at, visible_screen, window_origin};

pub(crate) fn prepare_caption(_: &tauri::WebviewWindow) -> tauri::Result<()> {
    Ok(())
}
