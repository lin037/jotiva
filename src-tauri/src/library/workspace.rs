use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    sync::{Mutex, RwLock},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use super::{
    folders,
    notes::{NoteId, Notes},
};
use crate::{desktop, storage::write_json, windows};

pub struct Workspace {
    pub(super) file: PathBuf,
    pub(super) roots: Vec<PathBuf>,
}

#[derive(Serialize)]
pub struct FileEntry {
    name: String,
    path: PathBuf,
    directory: bool,
}

pub fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown"))
}

impl Workspace {
    pub fn load(file: PathBuf) -> io::Result<Self> {
        let roots = match fs::read(&file) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };
        Ok(Self { file, roots })
    }

    fn add(&mut self, path: &Path) -> Result<(), String> {
        let root = fs::canonicalize(path).map_err(|error| error.to_string())?;
        if !root.is_dir() {
            return Err("请选择文件夹".into());
        }
        if self.roots.contains(&root) {
            return Ok(());
        }
        let mut roots = self.roots.clone();
        roots.push(root);
        write_json(&self.file, &roots).map_err(|error| error.to_string())?;
        self.roots = roots;
        Ok(())
    }

    fn remove(&mut self, root: &Path) -> Result<(), String> {
        let roots: Vec<_> = self
            .roots
            .iter()
            .filter(|path| *path != root)
            .cloned()
            .collect();
        write_json(&self.file, &roots).map_err(|error| error.to_string())?;
        self.roots = roots;
        Ok(())
    }

    pub(super) fn resolve(&self, path: &Path) -> Result<PathBuf, String> {
        let path = fs::canonicalize(path).map_err(|error| error.to_string())?;
        if !self.roots.iter().any(|root| path.starts_with(root)) {
            return Err("文件已移出工作区".into());
        }
        Ok(path)
    }
}

fn entries(directory: &Path) -> io::Result<Vec<FileEntry>> {
    let mut children = Vec::new();
    for child in fs::read_dir(directory)? {
        let child = child?;
        let name = child.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let kind = child.file_type()?;
        let path = child.path();
        // Traversing links can leave a chosen root or introduce directory cycles.
        if kind.is_dir() || (kind.is_file() && is_markdown(&path)) {
            children.push(FileEntry {
                name,
                path,
                directory: kind.is_dir(),
            });
        }
    }
    children.sort_by_cached_key(|child| (!child.directory, child.name.to_lowercase()));
    Ok(children)
}

#[tauri::command]
pub fn workspace_roots(app: AppHandle) -> Vec<PathBuf> {
    app.state::<Mutex<Workspace>>()
        .lock()
        .unwrap()
        .roots
        .clone()
}

#[tauri::command]
pub fn add_workspace_folder(app: AppHandle) -> Result<bool, String> {
    let Some(path) = folders::choose()? else {
        return Ok(false);
    };
    app.state::<Mutex<Workspace>>()
        .lock()
        .unwrap()
        .add(Path::new(&path))?;
    Ok(true)
}

#[tauri::command]
pub fn remove_workspace_folder(app: AppHandle, path: PathBuf) -> Result<(), String> {
    app.state::<Mutex<Workspace>>()
        .lock()
        .unwrap()
        .remove(&path)
}

#[tauri::command]
pub async fn workspace_children(app: AppHandle, path: PathBuf) -> Result<Vec<FileEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let directory = app
            .state::<Mutex<Workspace>>()
            .lock()
            .unwrap()
            .resolve(&path)?;
        entries(&directory).map_err(|error| format!("无法读取目录：{error}"))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn open_library_file(app: AppHandle, path: PathBuf) -> Result<NoteId, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let file = fs::canonicalize(path).map_err(|error| error.to_string())?;
        let workspace_file = app
            .state::<Mutex<Workspace>>()
            .lock()
            .unwrap()
            .roots
            .iter()
            .any(|root| file.starts_with(root));
        let inbox = app
            .state::<RwLock<Notes>>()
            .read()
            .unwrap()
            .inbox_directory();
        if file.parent() != Some(inbox.as_path())
            && !workspace_file
            && !app
                .state::<Mutex<folders::Folders>>()
                .lock()
                .unwrap()
                .items
                .iter()
                .any(|folder| Some(folder.path.as_path()) == file.parent())
        {
            return Err("文件已移出工作区或归档目录".into());
        }
        app.state::<RwLock<Notes>>()
            .write()
            .unwrap()
            .open_file(&file)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn choose_markdown_file(app: AppHandle) -> Result<Option<NoteId>, String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let _ = sender.send(folders::choose_markdown());
    })
    .map_err(|error| error.to_string())?;
    let Some(path) = receiver.await.map_err(|error| error.to_string())?? else {
        return Ok(None);
    };
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<RwLock<Notes>>()
            .write()
            .unwrap()
            .open_file(Path::new(&path))
            .map(Some)
    })
    .await
    .map_err(|error| error.to_string())?
}

// A drop can open the main window from a floating note. Keep the delivery until
// that window has mounted its listener; an event alone can arrive too early.
#[derive(Default)]
pub struct DroppedTabs(Mutex<Vec<DocumentTab>>);

#[derive(Serialize)]
pub struct DocumentTab {
    id: NoteId,
    title: String,
}

#[tauri::command]
pub fn take_dropped_tabs(queue: tauri::State<'_, DroppedTabs>) -> Vec<DocumentTab> {
    std::mem::take(&mut *queue.0.lock().unwrap())
}

#[tauri::command]
pub fn read_tab_titles(
    notes: tauri::State<'_, RwLock<Notes>>,
    ids: Vec<NoteId>,
) -> Vec<DocumentTab> {
    let notes = notes.read().unwrap();
    ids.into_iter()
        .filter_map(|id| {
            Some(DocumentTab {
                id,
                title: notes.title(id)?.to_owned(),
            })
        })
        .collect()
}

unsafe extern "C" {
    fn jot_dropped_file_paths() -> *mut std::ffi::c_char;
    fn free(pointer: *mut std::ffi::c_void);
}

#[tauri::command]
pub async fn open_dropped_markdown(app: AppHandle, names: Vec<String>) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let raw = unsafe { jot_dropped_file_paths() };
        let paths = unsafe {
            serde_json::from_slice::<Vec<PathBuf>>(std::ffi::CStr::from_ptr(raw).to_bytes())
        };
        unsafe { free(raw.cast()) };
        let _ = tx.send(paths);
    })
    .map_err(|e| e.to_string())?;
    let paths = rx
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let paths: Vec<_> = paths.into_iter().filter(|p| is_markdown(p)).collect();
    let mut received: Vec<_> = paths
        .iter()
        .filter_map(|p| p.file_name()?.to_str())
        .collect();
    received.sort_unstable();
    let mut names = names;
    names.sort_unstable();
    if paths.is_empty() || received != names {
        return Err("无法确认这次拖入的文件，请重新拖放".into());
    }
    let handle = app.clone();
    let (ids, errors) = tauri::async_runtime::spawn_blocking(move || read_paths(&handle, paths))
        .await
        .map_err(|e| e.to_string())?;
    if !ids.is_empty() {
        let tabs = read_tab_titles(app.state::<RwLock<Notes>>(), ids);
        app.state::<DroppedTabs>().0.lock().unwrap().extend(tabs);
        let handle = app.clone();
        app.run_on_main_thread(move || {
            desktop::report(desktop::open_main(&handle, None));
            let _ = handle.emit_to(windows::MAIN, "desktop:dropped-tabs", ());
        })
        .map_err(|e| e.to_string())?;
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

fn read_paths(app: &AppHandle, paths: Vec<PathBuf>) -> (Vec<NoteId>, Vec<String>) {
    let mut selected = Vec::new();
    let mut unique = HashSet::new();
    let mut errors = Vec::new();
    for path in paths {
        match app
            .state::<RwLock<Notes>>()
            .write()
            .unwrap()
            .open_file(&path)
        {
            Ok(id) if unique.insert(id) => selected.push(id),
            Ok(_) => {}
            Err(error) => errors.push(error),
        }
    }
    (selected, errors)
}

/// Native menu and Finder requests share the same canonical document identity.
pub fn open_paths(app: &AppHandle, paths: Vec<PathBuf>) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let (selected, errors) = read_paths(&app, paths);
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some((&first, others)) = selected.split_first() {
                desktop::report(desktop::open_main(&handle, Some(first)));
                for &id in others {
                    desktop::report(crate::show_document(&handle, id));
                }
            }
            if !errors.is_empty() {
                folders::show_error(&errors.join("\n"));
            }
            let _ = handle.emit_to(windows::MAIN, crate::events::NOTES_CHANGED, ());
        });
    });
}

pub fn menu_add(app: &AppHandle) -> Result<(), String> {
    if add_workspace_folder(app.clone())? {
        desktop::open_main(app, None)?;
        app.emit_to(windows::MAIN, "desktop:workspace-added", ())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    #[ignore = "capacity fixture with thousands of files"]
    fn lists_a_large_workspace_directory() {
        let root = std::env::temp_dir().join(format!("jot-capacity-{}", std::process::id()));
        fs::create_dir_all(root.join("sub")).unwrap();
        for index in (0..2_000).rev() {
            fs::write(root.join(format!("笔记-{index:04}.md")), "# 内容").unwrap();
        }
        fs::write(root.join(".hidden.md"), "").unwrap();
        fs::write(root.join("sub/nested.md"), "").unwrap();
        fs::write(root.join("ignored.txt"), "").unwrap();
        symlink(&root, root.join("loop")).unwrap();
        let files = entries(&root).unwrap();
        assert_eq!(files.len(), 2_001);
        assert!(files[0].directory);
        assert_eq!(files[0].name, "sub");
        for (index, file) in files[1..].iter().enumerate() {
            assert!(!file.directory);
            assert_eq!(file.name, format!("笔记-{index:04}.md"));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn roots_persist_and_only_direct_visible_markdown_children_are_listed() {
        let data = std::env::temp_dir().join(format!("jot-workspace-{}", std::process::id()));
        let root = data.join("project");
        fs::create_dir_all(root.join("sub")).unwrap();
        fs::write(root.join("B.MD"), "# Title").unwrap();
        fs::write(root.join("a.markdown"), "# Content").unwrap();
        fs::write(root.join(".draft.md"), "").unwrap();
        fs::write(root.join("other.txt"), "").unwrap();
        fs::write(root.join("sub/nested.md"), "").unwrap();
        symlink(&data, root.join("loop")).unwrap();

        let config = data.join("workspace.json");
        let mut workspace = Workspace::load(config.clone()).unwrap();
        workspace.add(&root).unwrap();
        workspace.add(&root.join(".")).unwrap();
        let mut workspace = Workspace::load(config).unwrap();
        assert_eq!(workspace.roots.len(), 1);
        let files = entries(&workspace.resolve(&root).unwrap()).unwrap();
        assert_eq!(
            files.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["sub", "a.markdown", "B.MD"]
        );
        assert!(workspace.resolve(&root.join("loop")).is_err());
        workspace.remove(&fs::canonicalize(&root).unwrap()).unwrap();
        assert!(workspace.resolve(&root).is_err());
        assert!(root.join("sub/nested.md").is_file());
        fs::remove_dir_all(data).unwrap();
    }
}
