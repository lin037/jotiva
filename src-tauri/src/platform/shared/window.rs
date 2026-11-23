use crate::platform::{HostBackend, Native};
use crate::{host::Target, library::notes::NoteId, windows::Builder};
use tauri::{LogicalPosition, LogicalSize, WebviewWindow};
pub fn builder<'a>(builder: Builder<'a>, _label: &str) -> Builder<'a> {
    builder.decorations(false).shadow(true)
}
pub fn configure(window: &WebviewWindow, _id: NoteId) -> tauri::Result<()> {
    crate::platform::prepare_caption(window)
}
pub fn destroy_note(window: &WebviewWindow) -> tauri::Result<()> {
    window.destroy()
}
pub fn activating_note() -> bool {
    false
}
pub fn note_spaces(_window: &WebviewWindow, _follows_spaces: bool) -> tauri::Result<()> {
    Ok(())
}
pub fn tile(
    caller: &WebviewWindow,
    left: &WebviewWindow,
    right: &WebviewWindow,
) -> tauri::Result<()> {
    if !crate::platform::can_position_windows() {
        return Err(tauri::Error::Io(std::io::Error::other(
            "请使用当前桌面的窗口平铺命令",
        )));
    }
    if let Some(m) = caller.current_monitor()? {
        let work = m.work_area();
        let half = work.size.width / 2;
        for (window, x, width) in [
            (left, work.position.x, half),
            (right, work.position.x + half as i32, work.size.width - half),
        ] {
            window.unmaximize()?;
            let outer = window.outer_size()?;
            let inner = window.inner_size()?;
            window.set_position(tauri::PhysicalPosition::new(x, work.position.y))?;
            window.set_size(tauri::PhysicalSize::new(
                width.saturating_sub(outer.width.saturating_sub(inner.width)),
                work.size
                    .height
                    .saturating_sub(outer.height.saturating_sub(inner.height)),
            ))?;
        }
    }
    Ok(())
}
pub fn place(
    window: &WebviewWindow,
    origin: LogicalPosition<f64>,
    width: f64,
    height: f64,
    _target: Option<Target>,
    focus: bool,
) -> tauri::Result<()> {
    if crate::platform::can_position_windows() {
        window.set_position(super::geometry::physical(origin))?;
    }
    window.set_size(LogicalSize::new(width, height))?;
    if focus {
        window.show()?;
        window.set_focus()
    } else {
        Native::show_passive(window)
    }
}

pub fn close_active(app: &tauri::AppHandle, multiple: bool) -> Result<(), String> {
    use tauri::{Emitter, Manager};
    if let Some((label, window)) = app
        .webview_windows()
        .into_iter()
        .find(|(_, w)| w.is_focused().unwrap_or(false))
    {
        if label == crate::windows::MAIN && multiple {
            app.emit_to(label, crate::events::EDITOR_ACTION, "close-tab")
                .map_err(|e| e.to_string())?
        } else {
            window.close().map_err(|e| e.to_string())?
        }
    }
    Ok(())
}
