pub(crate) struct Native;
pub(crate) mod credentials;
mod host;
pub mod storage;

pub(crate) fn can_position_windows() -> bool {
    true
}

pub(crate) use crate::platform::geometry::{pointer, screen_at, visible_screen, window_origin};

mod explorer;
pub(crate) use explorer::reveal_in_file_manager;

pub(crate) fn prepare_caption(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    window.hide_menu()
}
