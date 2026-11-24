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

pub const DOCUMENT_EXTENSIONS: &[&str] = &["md", "markdown", "txt"];

pub fn is_document(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            DOCUMENT_EXTENSIONS
                .iter()
                .any(|suffix| ext.eq_ignore_ascii_case(suffix))
        })
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

fn document_entry(child: fs::DirEntry) -> io::Result<Option<FileEntry>> {
    let name = child.file_name().to_string_lossy().into_owned();
    if name.starts_with('.') {
        return Ok(None);
    }
    let kind = child.file_type()?;
    let path = child.path();
    // Directory links can leave a selected root or introduce cycles, including Windows junctions.
    let directory = kind.is_dir() && !is_directory_link(&child);
    Ok(
        (directory || (kind.is_file() && is_document(&path))).then_some(FileEntry {
            name,
            path,
            directory,
        }),
    )
}

fn entries(directory: &Path) -> io::Result<Vec<FileEntry>> {
    let mut children = Vec::new();
    for child in fs::read_dir(directory)? {
        if let Some(entry) = document_entry(child?)? {
            children.push(entry);
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
pub async fn add_workspace_folder(app: AppHandle) -> Result<bool, String> {
    let Some(path) = folders::choose().await? else {
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

/// Search filenames without opening documents or making them recently used.
fn document_paths(roots: Vec<PathBuf>, archives: Vec<PathBuf>, query: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut visited = HashSet::new();
    let mut dirs: Vec<_> = roots.into_iter().map(|path| (path, true)).collect();
    dirs.extend(archives.into_iter().map(|path| (path, false)));
    let query = query.trim().to_lowercase();
    while let Some((directory, recursive)) = dirs.pop() {
        if !visited.insert((directory.clone(), recursive)) {
            continue;
        }
        let Ok(children) = fs::read_dir(directory) else {
            continue;
        };
        for entry in children
            .flatten()
            .filter_map(|child| document_entry(child).ok().flatten())
        {
            if recursive && entry.directory {
                dirs.push((entry.path, true));
            } else if !entry.directory && entry.name.to_lowercase().contains(&query) {
                files.push(entry.path);
            }
        }
    }
    files.sort_unstable();
    files.dedup();
    files
}

#[cfg(windows)]
fn is_directory_link(entry: &fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
    entry.metadata().map_or(true, |meta| {
        meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    })
}
#[cfg(not(windows))]
fn is_directory_link(_: &fs::DirEntry) -> bool {
    false
}

#[tauri::command]
pub async fn search_document_files(app: AppHandle, query: String) -> Result<Vec<PathBuf>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let roots = workspace_roots(app.clone());
        let mut archives: Vec<_> = app
            .state::<Mutex<folders::Folders>>()
            .lock()
            .unwrap()
            .items
            .iter()
            .map(|f| f.path.clone())
            .collect();
        let notes = app.state::<RwLock<Notes>>();
        archives.push(notes.read().unwrap().inbox_directory());
        let mut files = document_paths(roots, archives, &query);
        let notes = notes.read().unwrap();
        files.retain(|path| notes.card_at_path(path).is_none());
        files
    })
    .await
    .map_err(|e| e.to_string())
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
    let Some(path) = folders::choose_markdown().await? else {
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

pub fn open_drop(app: &AppHandle, paths: Vec<PathBuf>) {
    let paths: Vec<_> = paths.into_iter().filter(|p| is_document(p)).collect();
    if paths.is_empty() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = drop_paths(app, paths).await {
            folders::show_error(&e);
        }
    });
}

pub(crate) async fn drop_paths(app: AppHandle, paths: Vec<PathBuf>) -> Result<(), String> {
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

pub async fn menu_add(app: &AppHandle) -> Result<(), String> {
    if add_workspace_folder(app.clone()).await? {
        desktop::open_main(app, None)?;
        app.emit_to(windows::MAIN, "desktop:workspace-added", ())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::test_links::directory as symlink;

    #[test]
    fn discovers_unopened_files_across_overlapping_roots_and_shallow_archives() {
        let base = std::env::temp_dir().join(format!("jot-search-files-{}", std::process::id()));
        let root = base.join("project");
        let nested = root.join("sub");
        let archive = base.join("archive");
        fs::create_dir_all(&nested).unwrap();
        fs::create_dir_all(archive.join("private")).unwrap();
        for i in 0..28 {
            fs::write(nested.join(format!("Match-{i}.txt")), "").unwrap();
        }
        fs::write(archive.join("match.md"), "").unwrap();
        fs::write(archive.join("private/match.md"), "").unwrap();
        fs::write(root.join(".match.md"), "").unwrap();
        fs::write(root.join("match.bin"), "").unwrap();
        let files = document_paths(vec![root, nested], vec![archive], " mAtCh ");
        assert_eq!(files.len(), 29);
        assert!(files.windows(2).all(|pair| pair[0] < pair[1]));
        fs::remove_dir_all(base).unwrap();
    }

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
        fs::write(root.join("ignored.bin"), "").unwrap();
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
    fn roots_persist_and_only_direct_visible_document_children_are_listed() {
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
            ["sub", "a.markdown", "B.MD", "other.txt"]
        );
        assert!(workspace.resolve(&root.join("loop")).is_err());
        workspace.remove(&fs::canonicalize(&root).unwrap()).unwrap();
        assert!(workspace.resolve(&root).is_err());
        assert!(root.join("sub/nested.md").is_file());
        fs::remove_dir_all(data).unwrap();
    }
}
