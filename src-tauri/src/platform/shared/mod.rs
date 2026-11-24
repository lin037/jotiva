pub mod dialogs;
mod files;
pub mod filing;
pub mod geometry;
pub(crate) mod host_registry;
pub mod shortcuts;
pub mod window;

pub fn builder() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            let paths: Vec<_> = args
                .into_iter()
                .skip(1)
                .map(|arg| std::path::Path::new(&cwd).join(arg))
                .filter(|p| crate::library::workspace::is_document(p))
                .collect();
            if paths.is_empty() {
                crate::desktop::report(crate::desktop::open_main(app, None));
            } else {
                crate::library::workspace::open_paths(app, paths);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
}
pub fn startup_files() -> Vec<std::path::PathBuf> {
    std::env::args_os()
        .skip(1)
        .map(std::path::PathBuf::from)
        .filter(|p| crate::library::workspace::is_document(p))
        .collect()
}
pub fn system_font_size() -> f64 {
    13.0
}
