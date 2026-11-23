use global_hotkey::hotkey::HotKey;
use tauri_plugin_global_shortcut::ShortcutState;
pub fn record(_window: Option<&tauri::WebviewWindow>) -> Result<(), String> {
    Ok(())
}
pub fn watch(callback: extern "C" fn(u32)) -> Result<(), String> {
    // Window commands remain available in display sessions without global registration.
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(move |app, key, event| {
            if event.state == ShortcutState::Pressed {
                let id = key.id();
                let _ = app.run_on_main_thread(move || callback(id));
            }
        })
        .build();
    if let Err(error) = crate::desktop::app().plugin(plugin) {
        eprintln!("全局快捷键不可用：{error}");
    }
    Ok(())
}
type ShortcutManager = tauri_plugin_global_shortcut::GlobalShortcut<tauri::Wry>;

fn manager() -> Result<tauri::State<'static, ShortcutManager>, String> {
    use tauri::Manager;
    crate::desktop::app()
        .try_state()
        .ok_or_else(|| "当前桌面会话不支持全局快捷键".into())
}
pub(crate) fn register(key: HotKey) -> Result<(), String> {
    manager()?.register(key).map_err(|e| e.to_string())
}
pub(crate) fn unregister(keys: impl IntoIterator<Item = HotKey>) -> Result<(), String> {
    for key in keys {
        manager()?.unregister(key).map_err(|e| e.to_string())?;
    }
    Ok(())
}
