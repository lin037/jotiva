pub(crate) struct Native;
pub(crate) mod credentials;
mod host;
pub mod storage;
mod x11;

pub(crate) fn can_position_windows() -> bool {
    // Read the backend GDK actually selected during startup (also handles XWayland).
    use gtk::glib::prelude::ObjectExt;
    static SUPPORTED: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        gtk::gdk::Display::default()
            .is_some_and(|display| display.type_().name() == "GdkX11Display")
    });
    *SUPPORTED
}

pub(crate) use crate::platform::geometry::{pointer, screen_at, visible_screen, window_origin};

pub(crate) fn reveal_in_file_manager(path: &std::path::Path) -> Result<(), String> {
    tauri_plugin_opener::reveal_item_in_dir(path).map_err(|e| e.to_string())
}

pub(crate) fn prepare_caption(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    use gtk::prelude::*;
    // GTK recursively calls show_all() whenever Tauri presents a window.
    // Keep the installed accelerators, and let the custom caption own the menu button.
    let target = window.clone();
    window.run_on_main_thread(move || {
        if let Ok(container) = target.default_vbox() {
            for widget in container.children() {
                if widget.is::<gtk::MenuBar>() {
                    widget.set_no_show_all(true);
                    widget.hide();
                }
            }
        }
    })
}
