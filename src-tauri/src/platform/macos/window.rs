use crate::platform::{HostBackend, Native};
use crate::{host::Target, library::notes::NoteId, windows::Builder};
use tauri::{LogicalPosition, Manager, TitleBarStyle, WebviewWindow};
use tauri_nspanel::{ManagerExt, WebviewWindowExt};

mod panel {
    tauri_nspanel::tauri_panel!(JotNotePanel {
        config: {
            is_floating_panel: true,
            can_become_key_window: true,
            can_become_main_window: true,
            becomes_key_only_if_needed: false,
            hides_on_deactivate: false
        }
    });
}

unsafe extern "C" {
    fn jot_tile_documents(
        caller: *mut std::ffi::c_void,
        left: *mut std::ffi::c_void,
        right: *mut std::ffi::c_void,
    );
    fn jot_activating_note() -> bool;
    fn jot_note_spaces(window: *mut std::ffi::c_void, follows_spaces: bool);
    fn jot_focus_note(window: *mut std::ffi::c_void, move_to_host_space: bool);
    fn jot_place_note(
        window: *mut std::ffi::c_void,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        follows_spaces: bool,
    );
    fn jot_configure_quick_titlebar(
        window: *mut std::ffi::c_void,
        id: NoteId,
        resized: extern "C" fn(NoteId, f64, f64, f64, f64),
    );
}

pub fn note_spaces(window: &WebviewWindow, follows_spaces: bool) -> tauri::Result<()> {
    unsafe { jot_note_spaces(window.ns_window()?, follows_spaces) };
    Ok(())
}

pub fn activating_note() -> bool {
    unsafe { jot_activating_note() }
}

pub fn tile(
    caller: &WebviewWindow,
    left: &WebviewWindow,
    right: &WebviewWindow,
) -> tauri::Result<()> {
    unsafe {
        jot_tile_documents(caller.ns_window()?, left.ns_window()?, right.ns_window()?);
    }
    Ok(())
}

pub fn builder<'a>(mut builder: Builder<'a>, label: &str) -> Builder<'a> {
    if label != crate::windows::SETTINGS {
        builder = builder.disable_drag_drop_handler();
    }
    builder
        .title_bar_style(TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(LogicalPosition::new(15.0, 19.0))
}
pub fn configure(window: &WebviewWindow, id: NoteId) -> tauri::Result<()> {
    let panel = window.to_panel::<panel::JotNotePanel>()?;
    panel.set_released_when_closed(false);
    panel
        .add_style_mask(tauri_nspanel::objc2_app_kit::NSWindowStyleMask::NonactivatingPanel)
        .map_err(std::io::Error::other)?;
    unsafe { jot_configure_quick_titlebar(window.ns_window()?, id, crate::desktop::quick_resized) };
    Ok(())
}
pub fn destroy_note(window: &WebviewWindow) -> tauri::Result<()> {
    if let Ok(panel) = window.app_handle().get_webview_panel(window.label()) {
        // Release a nonactivating panel's native keyboard ownership while it
        // is still an NSPanel. to_window restores the original NSWindow class.
        panel.hide();
        panel.to_window();
    }
    window.destroy()
}

pub fn place(
    window: &WebviewWindow,
    origin: LogicalPosition<f64>,
    width: f64,
    height: f64,
    target: Option<Target>,
    focus: bool,
) -> tauri::Result<()> {
    unsafe {
        jot_place_note(
            window.ns_window()?,
            origin.x,
            origin.y,
            width,
            height,
            window.is_always_on_top()?,
        )
    };
    if focus {
        // Select this window before activating Jotiva across displays or Spaces.
        unsafe { jot_focus_note(window.ns_window()?, target.is_some_and(|t| t.fullscreen)) };
        tauri::Webview::set_focus(window.as_ref())?;
    } else {
        Native::show_passive(window)?;
    }
    Ok(())
}

unsafe extern "C" {
    fn jot_close_key_window(tabbed_workspace: *mut std::ffi::c_void) -> bool;
}
pub fn close_active(app: &tauri::AppHandle, multiple: bool) -> Result<(), String> {
    use crate::{events, windows};
    use tauri::Emitter;
    let workspace = app.get_webview_window(windows::MAIN).filter(|_| multiple);
    let pointer = workspace
        .as_ref()
        .map(|window| window.ns_window())
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or(std::ptr::null_mut());
    if unsafe { jot_close_key_window(pointer) } {
        app.emit_to(windows::MAIN, events::EDITOR_ACTION, "close-tab")
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
