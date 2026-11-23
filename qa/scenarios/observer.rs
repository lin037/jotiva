// Compiled only into the temporary QA application.
#[tauri::command]
pub fn qa_frame(window: tauri::WebviewWindow, frame: String) -> Result<(), String> {
    let path = std::path::PathBuf::from(std::env::var_os("JOT_QA_STATE").unwrap())
        .join(window.label())
        .with_extension("json");
    let tmp = path.with_extension("writing");
    std::fs::write(&tmp, frame)
        .and_then(|_| std::fs::rename(tmp, path))
        .map_err(|e| e.to_string())
}
