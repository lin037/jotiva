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

#[tauri::command]
pub(crate) async fn system_window_menu(window: tauri::WebviewWindow) -> Result<(), String> {
    use windows_sys::Win32::{Foundation::POINT, UI::WindowsAndMessaging::*};
    crate::windows::on_main_thread(move || {
        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as _;
        // Ask USER32 for the localized system menu of this exact window.
        unsafe {
            let menu = GetSystemMenu(hwnd, 0);
            let mut point = POINT::default();
            if menu.is_null() || GetCursorPos(&mut point) == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
            let maximized = IsZoomed(hwnd) != 0;
            for (id, enabled) in [
                (SC_RESTORE, maximized),
                (SC_MAXIMIZE, !maximized),
                (SC_MOVE, !maximized),
                (SC_SIZE, !maximized),
            ] {
                EnableMenuItem(menu, id, if enabled { MF_ENABLED } else { MF_GRAYED });
            }
            let command = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                point.x,
                point.y,
                0,
                hwnd,
                std::ptr::null(),
            );
            if command != 0 && PostMessageW(hwnd, WM_SYSCOMMAND, command as usize, 0) == 0 {
                return Err(std::io::Error::last_os_error().to_string());
            }
        }
        Ok(())
    })
    .await
}
