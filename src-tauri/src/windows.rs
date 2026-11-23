use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, TitleBarStyle, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

use crate::{
    events,
    host::{self, Rect, Target},
    library::notes::NoteId,
    settings::appearance::Appearance,
};

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

pub const MAIN: &str = "main";
pub const SETTINGS: &str = "settings";
pub const QUICK: &str = "quick";

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

#[derive(Clone, Copy)]
pub enum QuickSize {
    Summary,
    Dock(usize),
    Edit,
    Search,
    Overview,
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

pub fn main(app: &AppHandle) -> tauri::Result<()> {
    let window = match app.get_webview_window(MAIN) {
        Some(window) => window,
        None => WebviewWindowBuilder::new(app, MAIN, WebviewUrl::default())
            .disable_drag_drop_handler()
            .title("Jotiva")
            .title_bar_style(TitleBarStyle::Overlay)
            .hidden_title(true)
            .traffic_light_position(LogicalPosition::new(15.0, 19.0))
            .inner_size(940.0, 640.0)
            .min_inner_size(580.0, 400.0)
            .visible(false)
            .build()?,
    };
    window.show()?;
    window.set_focus()
}

pub fn document_id(label: &str) -> Option<NoteId> {
    label.strip_prefix("document-")?.parse().ok()
}

pub fn document(app: &AppHandle, id: NoteId, title: &str) -> tauri::Result<()> {
    let label = format!("document-{id}");
    let window = match app.get_webview_window(&label) {
        Some(window) => window,
        None => WebviewWindowBuilder::new(app, label, WebviewUrl::default())
            .disable_drag_drop_handler()
            .title(title)
            .inner_size(700.0, 560.0)
            .min_inner_size(380.0, 280.0)
            .visible(false)
            .build()?,
    };
    window.unminimize()?;
    window.show()?;
    window.set_focus()
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SettingsSection {
    Appearance,
    Editor,
    Filing,
    Shortcuts,
    Windows,
}

pub fn settings(app: &AppHandle, section: Option<SettingsSection>) -> tauri::Result<()> {
    let window = match app.get_webview_window(SETTINGS) {
        Some(window) => {
            if let Some(section) = section {
                app.emit_to(SETTINGS, events::SETTINGS_SECTION, section)?;
            }
            window
        }
        None => WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::default())
            .title("Jotiva 设置")
            .inner_size(680.0, 510.0)
            .min_inner_size(475.0, 395.0)
            .initialization_script(format!(
                "window.__jotSettingsSection = {}",
                serde_json::to_string(&section.unwrap_or(SettingsSection::Appearance))
                    .expect("serialize settings section")
            ))
            .visible(false)
            .build()?,
    };
    window.show()?;
    window.set_focus()
}

pub fn note_label(id: NoteId) -> String {
    format!("quick-{id}")
}

pub fn note_id(label: &str) -> Option<NoteId> {
    label.strip_prefix("quick-")?.parse().ok()
}

pub fn is_quick(label: &str) -> bool {
    label == QUICK || note_id(label).is_some()
}

pub fn quick(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    quick_window(app, QUICK, 0)
}

pub fn quick_window(app: &AppHandle, label: &str, id: NoteId) -> tauri::Result<WebviewWindow> {
    match app.get_webview_window(label) {
        Some(window) => Ok(window),
        None => {
            let window = WebviewWindowBuilder::new(app, label, WebviewUrl::default())
                .disable_drag_drop_handler()
                .title("Jotiva 便签")
                .inner_size(384.0, 352.0)
                .title_bar_style(TitleBarStyle::Overlay)
                .hidden_title(true)
                .traffic_light_position(LogicalPosition::new(15.0, 19.0))
                .resizable(true)
                .minimizable(false)
                .always_on_top(true)
                .accept_first_mouse(true)
                .focused(false)
                .visible(false)
                .build()?;
            let panel = window.to_panel::<panel::JotNotePanel>()?;
            panel.set_released_when_closed(false);
            panel
                .add_style_mask(tauri_nspanel::objc2_app_kit::NSWindowStyleMask::NonactivatingPanel)
                .map_err(std::io::Error::other)?;
            unsafe {
                jot_configure_quick_titlebar(window.ns_window()?, id, crate::desktop::quick_resized)
            };
            Ok(window)
        }
    }
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

pub fn present(
    window: &WebviewWindow,
    size: QuickSize,
    target: Option<Target>,
    docked: bool,
    saved_origin: Option<LogicalPosition<f64>>,
    custom_size: Option<LogicalSize<f64>>,
    focus: bool,
) -> tauri::Result<()> {
    let (width, height): (f64, f64) = match size {
        QuickSize::Summary => (320.0, 76.0),
        QuickSize::Dock(0 | 1) => (216.0, 48.0),
        QuickSize::Dock(count) => (232.0, 44.0 + count.min(9) as f64 * 35.0),
        QuickSize::Edit => (384.0, 352.0),
        QuickSize::Search => (472.0, 416.0),
        QuickSize::Overview => (900.0, 585.0),
    };
    let saved_origin = match size {
        QuickSize::Edit | QuickSize::Summary => saved_origin,
        _ => None,
    };
    let screen = match saved_origin {
        Some(point) => host::screen_at(point),
        None => target.map_or_else(host::visible_screen, |target| target.screen),
    };
    let scale = window
        .state::<Mutex<Appearance>>()
        .lock()
        .unwrap()
        .scale()
        .max(1.0);
    let preferred = custom_size.unwrap_or(LogicalSize::new(width * scale, height * scale));
    let width = preferred.width.min((screen.width - 16.0).max(1.0));
    let height = preferred.height.min((screen.height - 16.0).max(1.0));
    let minimum = match size {
        QuickSize::Summary | QuickSize::Dock(_) => LogicalSize::new(164.0, 44.0),
        _ => LogicalSize::new(248.0, 145.0),
    };
    window.set_min_size(Some(minimum))?;

    let centered = matches!(size, QuickSize::Search | QuickSize::Overview)
        || (target.is_none() && saved_origin.is_none() && !docked);
    let origin = position(
        screen,
        width,
        height,
        centered,
        if docked { None } else { saved_origin },
    );
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
        host::show_passive(window)?;
    }
    Ok(())
}

pub fn default_note_position(screen: Rect, scale: f64) -> LogicalPosition<f64> {
    position(screen, 384. * scale, 352. * scale, false, None)
}

pub fn beside(
    parent: LogicalPosition<f64>,
    parent_size: LogicalSize<f64>,
    child_size: LogicalSize<f64>,
    screen: Rect,
    occupied: &[LogicalPosition<f64>],
) -> LogicalPosition<f64> {
    let (width, height) = (child_size.width, child_size.height);
    let offsets = [
        (parent_size.width + 8.0, 0.0),
        (-width - 8.0, 0.0),
        (0.0, parent_size.height + 8.0),
        (0.0, -height - 8.0),
    ];
    let at = |(x, y)| {
        position(
            screen,
            width,
            height,
            false,
            Some(LogicalPosition::new(parent.x + x, parent.y + y)),
        )
    };
    let distinct = |point: &LogicalPosition<f64>| {
        std::iter::once(&parent)
            .chain(occupied)
            .all(|other| (other.x - point.x).abs() >= 28.0 || (other.y - point.y).abs() >= 28.0)
    };
    let adjacent = offsets.into_iter().map(at).find(|p| {
        distinct(p)
            && (p.x >= parent.x + parent_size.width
                || p.x + width <= parent.x
                || p.y >= parent.y + parent_size.height
                || p.y + height <= parent.y)
    });
    adjacent
        .or_else(|| {
            let cascade = (1..=occupied.len() + 2).flat_map(|step| {
                let offset = 32.0 * step as f64;
                [
                    (-offset, offset),
                    (offset, -offset),
                    (-offset, -offset),
                    (offset, offset),
                ]
            });
            offsets.into_iter().chain(cascade).map(at).find(distinct)
        })
        .unwrap_or_else(|| at((0.0, 0.0)))
}

fn position(
    screen: Rect,
    width: f64,
    height: f64,
    centered: bool,
    saved_origin: Option<LogicalPosition<f64>>,
) -> LogicalPosition<f64> {
    if centered {
        return LogicalPosition::new(
            screen.x + (screen.width - width) / 2.0,
            screen.y + (screen.height - height) / 2.0,
        );
    }
    const GAP: f64 = 8.0;
    let right = screen.x + (screen.width - width - GAP).max(0.0);
    let left = (screen.x + GAP).min(right);
    let bottom = screen.y + (screen.height - height - GAP).max(0.0);
    let top = (screen.y + GAP).min(bottom);
    // Keep unsited notes out of the upper-right system notification banner area.
    let origin = saved_origin.unwrap_or(LogicalPosition::new(right, (top + bottom) / 2.0));
    LogicalPosition::new(origin.x.clamp(left, right), origin.y.clamp(top, bottom))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_note_fits_beside_a_folded_origin_even_at_the_right_screen_edge() {
        let screen = Rect {
            x: 0.,
            y: 0.,
            width: 1100.,
            height: 760.,
        };
        let folded = LogicalSize::new(320., 76.);
        let edit = LogicalSize::new(384., 352.);
        let parent = LogicalPosition::new(772., 120.);
        let first = beside(parent, folded, edit, screen, &[]);
        assert!(first.x + edit.width <= parent.x);
        let second = beside(parent, folded, edit, screen, &[first]);
        assert_ne!(first, second);
        assert_ne!(second, parent);
        assert!(second.x + edit.width <= screen.width && second.y + edit.height <= screen.height);
    }

    #[test]
    fn placement_uses_screen_coordinates_and_keeps_the_note_reachable() {
        let screen = Rect {
            x: -1440.0,
            y: 25.0,
            width: 1440.0,
            height: 850.0,
        };
        assert_eq!(
            position(screen, 320.0, 76.0, false, None),
            LogicalPosition::new(-328.0, 412.0)
        );
        let moved = LogicalPosition::new(-551.0, 238.0);
        assert_eq!(position(screen, 320.0, 76.0, false, Some(moved)), moved);
        let disconnected = LogicalPosition::new(1800.0, -400.0);
        assert_eq!(
            position(screen, 320.0, 76.0, false, Some(disconnected)),
            LogicalPosition::new(-328.0, 33.0)
        );
        let small = Rect {
            width: 50.0,
            height: 50.0,
            ..screen
        };
        assert_eq!(position(small, 320.0, 76.0, false, None).x, screen.x);
    }
}
