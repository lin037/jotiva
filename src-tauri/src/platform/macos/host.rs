use crate::host::{DropTarget, Point, Rect, SourceChoice, Target, WindowIdentity};
use crate::platform::{HostBackend, Native};
use std::ffi::{CStr, CString, c_char, c_void};
unsafe extern "C" {
    fn jot_host_watch(callback: extern "C" fn(u64, bool));
    fn jot_host_focused(attach: bool) -> Target;
    fn jot_host_permission(prompt: bool) -> bool;
    fn jot_open_accessibility_settings() -> bool;
    fn jot_host_resigned() -> bool;
    fn jot_host_raise(id: u64) -> Target;
    fn jot_host_activate(pid: i32);
    fn jot_host_unwatch(id: u64);
    fn jot_show_passive(window: *mut c_void);
    fn jot_visible_screen() -> Rect;
    fn jot_screen_at(x: f64, y: f64) -> Rect;
    fn jot_window_origin(window: *mut c_void) -> Point;
    fn jot_begin_source_choices(owner: *const c_char) -> u64;
    fn jot_source_choices(owner: *const c_char, generation: u64) -> *mut c_char;
    fn jot_clear_source_choices(owner: *const c_char);
    fn jot_register_source(choice: u64) -> Target;
    fn jot_drag_started();
    fn jot_drag_ended();
    fn jot_focused_choice() -> i64;
    fn jot_source_at_drop() -> i64;
    fn jot_pointer_position() -> Point;
    fn jot_window_identity(host: u64) -> *mut c_char;
    fn jot_capture_window_identity(host: u64, done: extern "C" fn(u64, *const c_char));
    fn jot_set_recovery(
        json: *const c_char,
        callback: extern "C" fn(u64, u64, *const c_char) -> bool,
    );
    fn free(pointer: *mut c_void);
}

fn decode_identity(raw: *const c_char) -> Option<WindowIdentity> {
    if raw.is_null() {
        return None;
    }
    serde_json::from_slice(unsafe { CStr::from_ptr(raw) }.to_bytes()).ok()
}

pub fn pointer() -> tauri::LogicalPosition<f64> {
    let point = unsafe { jot_pointer_position() };
    tauri::LogicalPosition::new(point.x, point.y)
}

pub fn screen_at(point: tauri::LogicalPosition<f64>) -> Rect {
    unsafe { jot_screen_at(point.x, point.y) }
}

pub fn window_origin(window: &tauri::WebviewWindow) -> tauri::Result<tauri::LogicalPosition<f64>> {
    let point = unsafe { jot_window_origin(window.ns_window()?) };
    Ok(tauri::LogicalPosition::new(point.x, point.y))
}

pub fn visible_screen() -> Rect {
    unsafe { jot_visible_screen() }
}

// The AppKit bridge and observer callbacks run on the main thread.

impl HostBackend for Native {
    fn supported() -> bool {
        true
    }
    fn watch_drag(started: bool) {
        unsafe {
            if started {
                jot_drag_started();
            } else {
                jot_drag_ended();
            }
        }
    }

    fn focused_choice() -> Result<Option<u64>, String> {
        match unsafe { jot_focused_choice() } {
            0 => Ok(None),
            -2 => Err("该应用没有可识别的活动窗口，可用窗口列表选择".into()),
            id => Ok(Some(id as u64)),
        }
    }

    fn source_at_drop() -> Result<DropTarget, String> {
        let hit = unsafe { jot_source_at_drop() };
        if hit == -2 {
            return Err("无法识别此窗口，请在来源列表中选择要关联的窗口".into());
        }
        Ok(DropTarget {
            outside: hit != 0,
            choice: (hit > 0).then_some(hit as u64),
        })
    }

    fn identity(host: u64) -> Option<WindowIdentity> {
        let raw = unsafe { jot_window_identity(host) };
        if raw.is_null() {
            return None;
        }
        let identity = decode_identity(raw);
        unsafe { free(raw.cast()) };
        identity
    }

    fn capture_identity(host: u64, callback: fn(u64, WindowIdentity)) {
        static CALLBACK: std::sync::OnceLock<fn(u64, WindowIdentity)> = std::sync::OnceLock::new();
        extern "C" fn captured(host: u64, data: *const c_char) {
            if let Some(identity) = decode_identity(data) {
                CALLBACK.get().unwrap()(host, identity);
            }
        }
        CALLBACK.get_or_init(|| callback);
        unsafe { jot_capture_window_identity(host, captured) };
    }

    fn recover(
        specs: &std::collections::BTreeMap<u64, WindowIdentity>,
        callback: fn(u64, u64, WindowIdentity) -> bool,
    ) {
        type Restored = fn(u64, u64, WindowIdentity) -> bool;
        static CALLBACK: std::sync::OnceLock<Restored> = std::sync::OnceLock::new();
        extern "C" fn restored(origin: u64, host: u64, data: *const c_char) -> bool {
            decode_identity(data)
                .is_some_and(|identity| CALLBACK.get().unwrap()(origin, host, identity))
        }
        CALLBACK.get_or_init(|| callback);
        let json = CString::new(serde_json::to_vec(specs).expect("serialize recovery keys"))
            .expect("JSON has no NUL");
        unsafe { jot_set_recovery(json.as_ptr(), restored) };
    }

    async fn sources(owner: &str) -> Result<Option<Vec<SourceChoice>>, String> {
        let owner = CString::new(owner).expect("Tauri window label has no NUL");
        let generation = unsafe { jot_begin_source_choices(owner.as_ptr()) };
        tauri::async_runtime::spawn_blocking(move || {
            let json = unsafe { jot_source_choices(owner.as_ptr(), generation) };
            if json.is_null() {
                return if Self::permission(false) {
                    Err("无法读取系统窗口列表，请重试".into())
                } else {
                    Ok(None)
                };
            }
            let decoded = serde_json::from_slice(unsafe { CStr::from_ptr(json) }.to_bytes())
                .map_err(|error| error.to_string());
            unsafe { free(json.cast()) };
            decoded.map(Some)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    fn clear_sources(owner: &str) {
        let owner = CString::new(owner).expect("Tauri window label has no NUL");
        unsafe { jot_clear_source_choices(owner.as_ptr()) };
    }

    fn register_source(choice: u64) -> Result<Target, String> {
        let target = unsafe { jot_register_source(choice) };
        if target.id == 0 {
            return Err("窗口已关闭或暂时无法关联，请刷新后重试".into());
        }
        Ok(target)
    }

    fn watch(callback: extern "C" fn(u64, bool)) {
        unsafe { jot_host_watch(callback) };
    }

    fn focused(attach: bool) -> Target {
        unsafe { jot_host_focused(attach) }
    }

    fn permission(prompt: bool) -> bool {
        unsafe { jot_host_permission(prompt) }
    }

    fn request_accessibility() -> Result<bool, String> {
        if Self::permission(true) {
            return Ok(true);
        }
        if unsafe { jot_open_accessibility_settings() } {
            Ok(false)
        } else {
            Err("请打开「系统设置 → 隐私与安全性 → 辅助功能」，开启 Jotiva。".into())
        }
    }

    fn resigned() -> bool {
        unsafe { jot_host_resigned() }
    }

    fn raise(id: u64) -> Option<Target> {
        let target = unsafe { jot_host_raise(id) };
        (target.id != 0).then_some(target)
    }

    fn activate(pid: i32) {
        unsafe { jot_host_activate(pid) };
    }

    fn unwatch(id: u64) {
        unsafe { jot_host_unwatch(id) };
    }

    fn show_passive(window: &tauri::WebviewWindow) -> tauri::Result<()> {
        // AppKit's orderFrontRegardless displays a summary without making it key.
        unsafe { jot_show_passive(window.ns_window()?) };
        Ok(())
    }
}
