use tauri_plugin_dialog::{DialogExt, FilePath};
async fn pick(filter: Option<(&str, &[&str])>) -> Result<Option<String>, String> {
    let (send, recv) = tokio::sync::oneshot::channel();
    let done = move |path: Option<FilePath>| {
        let _ = send.send(path);
    };
    let dialog = crate::desktop::app().dialog().file();
    if let Some((name, extensions)) = filter {
        dialog.add_filter(name, extensions).pick_file(done)
    } else {
        dialog.pick_folder(done)
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
    pick(None).await
}
pub async fn choose_markdown() -> Result<Option<String>, String> {
    pick(Some((
        "Markdown / TXT",
        crate::library::workspace::DOCUMENT_EXTENSIONS,
    )))
    .await
}
pub async fn choose_css() -> Result<Option<String>, String> {
    pick(Some(("CSS", &["css"]))).await
}
pub fn show_error(message: &str) {
    crate::desktop::app().dialog().message(message).show(|_| {});
}
