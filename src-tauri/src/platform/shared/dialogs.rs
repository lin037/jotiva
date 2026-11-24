use tauri_plugin_dialog::{DialogExt, FilePath};
async fn pick(folder: bool) -> Result<Option<String>, String> {
    let (send, recv) = tokio::sync::oneshot::channel();
    let done = move |path: Option<FilePath>| {
        let _ = send.send(path);
    };
    let dialog = crate::desktop::app().dialog().file();
    if folder {
        dialog.pick_folder(done)
    } else {
        dialog
            .add_filter(
                "Markdown / TXT",
                crate::library::workspace::DOCUMENT_EXTENSIONS,
            )
            .pick_file(done)
    }
    recv.await
        .map_err(|e| e.to_string())?
        .map(|p| {
            p.into_path()
                .map(|p| p.to_string_lossy().into_owned())
                .map_err(|e| e.to_string())
        })
        .transpose()
}
pub async fn choose() -> Result<Option<String>, String> {
    pick(true).await
}
pub async fn choose_markdown() -> Result<Option<String>, String> {
    pick(false).await
}
pub fn show_error(message: &str) {
    crate::desktop::app().dialog().message(message).show(|_| {});
}
