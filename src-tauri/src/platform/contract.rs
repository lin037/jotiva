use crate::host::{DropTarget, SourceChoice, Target, WindowIdentity};
use std::{collections::BTreeMap, path::Path};
use tauri::WebviewWindow;

pub(crate) trait HostBackend {
    fn supported() -> bool;
    fn watch(callback: extern "C" fn(u64, bool));
    fn focused(attach: bool) -> Target;
    fn permission(prompt: bool) -> bool;
    fn request_accessibility() -> Result<bool, String>;
    fn resigned() -> bool;
    fn raise(host: u64) -> Option<Target>;
    fn editor_target_after_denied_raise(_host: u64) -> Option<Target> {
        None
    }
    fn activate(pid: i32);
    fn unwatch(host: u64);
    fn watch_drag(_started: bool) {}
    fn focused_choice() -> Result<Option<u64>, String>;
    fn source_at_drop() -> Result<DropTarget, String>;
    async fn sources(owner: &str) -> Result<Option<Vec<SourceChoice>>, String>;
    fn clear_sources(owner: &str);
    fn register_source(choice: u64) -> Result<Target, String>;
    fn show_passive(window: &WebviewWindow) -> tauri::Result<()>;
    // Only restore when the host supplies an unambiguous window or document identity.
    fn identity(_host: u64) -> Option<WindowIdentity> {
        None
    }
    fn capture_identity(_host: u64, _done: fn(u64, WindowIdentity)) {}
    fn recover(
        _pending: &BTreeMap<u64, WindowIdentity>,
        _restored: fn(u64, u64, WindowIdentity) -> bool,
    ) {
    }
}

pub(crate) trait FileManager {
    type TrashRecord;
    fn reveal(path: &Path) -> Result<(), String>;
    fn move_to(from: &Path, to: &Path) -> Result<(), String>;
    fn copy(from: &Path, to: &Path) -> Result<(), String>;
    fn trash(from: &Path) -> Result<Self::TrashRecord, String>;
    fn restore(record: Self::TrashRecord, destination: &Path) -> Result<(), String>;
}
