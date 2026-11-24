use super::x11::desktop;
use crate::host::{DropTarget, SourceChoice, Target};
use crate::platform::{HostBackend, Native, geometry::visible_screen};
const UNAVAILABLE: &str = "当前桌面未提供可关联的外部窗口，可使用独立便签";

impl HostBackend for Native {
    fn supported() -> bool {
        desktop().is_some()
    }
    fn watch(callback: extern "C" fn(u64, bool)) {
        if let Some(x) = desktop() {
            x.watch(callback);
        }
    }
    fn focused(attach: bool) -> Target {
        use tauri::Manager;
        desktop()
            .and_then(|x| x.focused(attach))
            .unwrap_or_else(|| Target {
                own_app: crate::desktop::app()
                    .webview_windows()
                    .values()
                    .any(|w| w.is_focused().unwrap_or(false)),
                screen: visible_screen(),
                ..Default::default()
            })
    }
    fn permission(_: bool) -> bool {
        Self::supported()
    }
    fn request_accessibility() -> Result<bool, String> {
        Ok(Self::supported())
    }
    fn resigned() -> bool {
        !Self::focused(false).own_app
    }
    fn raise(id: u64) -> Option<Target> {
        desktop()?.raise(id)
    }
    fn activate(pid: i32) {
        if let Some(x) = desktop() {
            x.activate_pid(pid);
        }
    }
    fn unwatch(id: u64) {
        if let Some(x) = desktop() {
            x.unwatch(id);
        }
    }
    fn focused_choice() -> Result<Option<u64>, String> {
        Ok(desktop().and_then(|x| x.focused_choice()))
    }
    fn source_at_drop() -> Result<DropTarget, String> {
        desktop()
            .map(|x| x.drop_target())
            .ok_or_else(|| UNAVAILABLE.into())
    }
    async fn sources(owner: &str) -> Result<Option<Vec<SourceChoice>>, String> {
        let x = desktop().ok_or(UNAVAILABLE)?;
        let owner = owner.to_owned();
        tauri::async_runtime::spawn_blocking(move || Some(x.choices(&owner)))
            .await
            .map_err(|e| e.to_string())
    }
    fn clear_sources(owner: &str) {
        if let Some(x) = desktop() {
            x.clear(owner);
        }
    }
    fn register_source(id: u64) -> Result<Target, String> {
        desktop()
            .and_then(|x| x.register(id))
            .ok_or_else(|| "窗口已关闭，请刷新后重试".into())
    }
    fn show_passive(window: &tauri::WebviewWindow) -> tauri::Result<()> {
        use gtk::prelude::GtkWindowExt;
        window.gtk_window()?.set_focus_on_map(false);
        window.show()
    }
}
