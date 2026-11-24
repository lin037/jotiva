use global_hotkey::hotkey::{HotKey as Shortcut, Modifiers};
use std::ffi::{CString, c_char};
const EVENT_HOTKEY_EXISTS: i32 = -9878;

unsafe extern "C" {
    fn jot_record_shortcut(window: *mut std::ffi::c_void, callback: extern "C" fn(*const c_char));
    fn jot_hotkeys_start(callback: extern "C" fn(u32)) -> i32;
    fn jot_hotkey_register(id: u32, key: *const c_char, modifiers: u32) -> i32;
    fn jot_hotkey_unregister(id: u32) -> i32;
}

pub fn record(window: Option<&tauri::WebviewWindow>) -> Result<(), String> {
    let window = window
        .map(|w| w.ns_window())
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or(std::ptr::null_mut());
    unsafe { jot_record_shortcut(window, crate::desktop::recorded_shortcut) };
    Ok(())
}

pub fn watch(callback: extern "C" fn(u32)) -> Result<(), String> {
    let status = unsafe { jot_hotkeys_start(callback) };
    if status == 0 {
        Ok(())
    } else {
        Err(format!("无法监听系统快捷键（macOS {status}）"))
    }
}

pub(crate) fn register(key: Shortcut) -> Result<(), String> {
    let code = CString::new(key.key.to_string()).expect("physical key names have no NUL");
    let mut modifiers = 0;
    for (mask, flag) in [
        (1, Modifiers::CONTROL),
        (2, Modifiers::SHIFT),
        (4, Modifiers::ALT),
        (8, Modifiers::SUPER),
    ] {
        if key.mods.contains(flag) {
            modifiers |= mask;
        }
    }
    let status = unsafe { jot_hotkey_register(key.id(), code.as_ptr(), modifiers) };
    match status {
        0 => Ok(()),
        EVENT_HOTKEY_EXISTS => Err(format!("{key} 已被其他应用独占登记")),
        _ => Err(format!("{key} 注册失败（macOS {status}）")),
    }
}

pub(crate) fn unregister(keys: impl IntoIterator<Item = Shortcut>) -> Result<(), String> {
    for key in keys {
        let status = unsafe { jot_hotkey_unregister(key.id()) };
        if status != 0 {
            return Err(format!("{key} 注销失败（macOS {status}）"));
        }
    }
    Ok(())
}
