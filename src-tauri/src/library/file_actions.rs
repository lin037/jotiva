use std::{
    collections::HashSet,
    fs, io,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, RwLock},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::{
    folders::Folders,
    notes::{NoteId, NoteLocation, Notes, file_stem},
    workspace::{Workspace, is_document},
};
use crate::platform::{FileManager, Native};
use crate::{events, storage::create_markdown};

// Serialize native tree mutations, independently of the locks needed to look up roots or edit notes.
static PATH_CHANGES: Mutex<()> = Mutex::new(());

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FileAction {
    CreateFile { name: Option<String> },
    CreateDirectory { name: String },
    Rename { name: String },
    Move { directory: PathBuf },
    Duplicate,
    Trash,
}

fn filename(name: &str) -> Result<&str, String> {
    let name = name.trim();
    if name.is_empty()
        || name.starts_with('.')
        || name.contains(['/', ':', '\\'])
        || name
            .chars()
            .any(crate::platform::storage::invalid_name_char)
        || crate::platform::storage::reserved_name(name)
        || name.chars().any(char::is_control)
    {
        return Err("请输入有效名称，名称不能使用路径分隔符、系统保留名或非法字符".into());
    }
    Ok(name)
}

fn markdown_name(name: &str) -> Result<String, String> {
    let name = filename(name)?;
    if is_document(Path::new(name)) {
        return Ok(name.into());
    }
    if Path::new(name).extension().is_some() {
        return Err("文件扩展名须为 .md、.markdown 或 .txt".into());
    }
    Ok(format!("{name}.md"))
}

fn archive_title(name: &str) -> Result<String, String> {
    let name = filename(name)?;
    if is_document(Path::new(name)) {
        return Ok(filename(
            Path::new(name)
                .file_stem()
                .and_then(|v| v.to_str())
                .unwrap_or(""),
        )?
        .to_owned());
    }
    Ok(name.to_owned())
}

fn map_path(path: &Path, from: &Path, to: &Path) -> PathBuf {
    // Joining an empty suffix creates a trailing slash, invalid for a regular file.
    if path == from {
        to.into()
    } else {
        to.join(path.strip_prefix(from).expect("path in changed subtree"))
    }
}

fn copy_sibling(source: &Path) -> Result<PathBuf, String> {
    let parent = source.parent().ok_or("无法复制此目录")?;
    let (stem, ext) = if source.is_dir() {
        (source.file_name(), String::new())
    } else {
        (
            source.file_stem(),
            source
                .extension()
                .map(|ext| format!(".{}", ext.to_string_lossy()))
                .unwrap_or_default(),
        )
    };
    let stem = stem.ok_or("无效名称")?.to_string_lossy();
    for index in 1u64.. {
        let suffix = if index == 1 {
            String::new()
        } else {
            format!(" {index}")
        };
        let target = parent.join(format!("{stem} 副本{suffix}{ext}"));
        match target.try_exists() {
            Ok(true) => continue,
            Err(error) => return Err(error.to_string()),
            Ok(false) => {}
        }
        Native::copy(source, &target)?;
        return Ok(target);
    }
    unreachable!()
}

struct MetadataEdit {
    file: PathBuf,
    before: Vec<u8>,
    after: Option<Vec<u8>>,
}

impl MetadataEdit {
    fn put(file: PathBuf, value: &impl Serialize) -> Result<Self, String> {
        Self::change(
            file,
            Some(serde_json::to_vec(value).map_err(|error| error.to_string())?),
        )
    }

    fn change(file: PathBuf, after: Option<Vec<u8>>) -> Result<Self, String> {
        let before = fs::read(&file).map_err(|e| e.to_string())?;
        Ok(Self {
            file,
            before,
            after,
        })
    }

    fn write(&self, bytes: Option<&[u8]>) -> io::Result<()> {
        if let Some(bytes) = bytes {
            crate::storage::write_atomic(&self.file, bytes)
        } else {
            fs::remove_file(&self.file)
        }
    }
}

fn commit(edits: &[MetadataEdit]) -> Result<(), String> {
    for (index, item) in edits.iter().enumerate() {
        if let Err(error) = item.write(item.after.as_deref()) {
            for done in edits[..index].iter().rev() {
                done.write(Some(&done.before)).map_err(|undo| {
                    format!(
                        "记录更新失败（{error}），恢复 {} 失败：{undo}",
                        done.file.display()
                    )
                })?;
            }
            return Err(error.to_string());
        }
    }
    Ok(())
}

#[derive(Default, Serialize)]
pub struct FileResult {
    path: Option<PathBuf>,
    removed: Vec<NoteId>,
    #[serde(skip)]
    folders_changed: bool,
}

fn relocate(
    workspace: &mut Workspace,
    folders: &mut Folders,
    notes: &mut Notes,
    source: &Path,
    target: Option<PathBuf>,
) -> Result<FileResult, String> {
    if let Some(to) = &target {
        if to == source {
            return Ok(FileResult {
                path: Some(to.clone()),
                ..Default::default()
            });
        }
        if to.starts_with(source) {
            return Err("不能将文件夹移动到自身的子目录".into());
        }
    }
    let trashed = match &target {
        Some(to) => {
            Native::move_to(source, to)?;
            None
        }
        None => Some(Native::trash(source)?),
    };
    finish_relocation(workspace, folders, notes, source, target, trashed)
}

fn finish_relocation(
    workspace: &mut Workspace,
    folders: &mut Folders,
    notes: &mut Notes,
    source: &Path,
    target: Option<PathBuf>,
    trashed: Option<<Native as FileManager>::TrashRecord>,
) -> Result<FileResult, String> {
    let mut result = FileResult::default();
    if let Err(error) = update_paths(
        workspace,
        folders,
        notes,
        source,
        target.clone(),
        &mut result,
    ) {
        let restored = match (target, trashed) {
            (Some(to), _) => Native::move_to(&to, source),
            (_, Some(saved)) => Native::restore(saved, source),
            _ => unreachable!(),
        };
        restored.map_err(|undo| format!("更新记录失败（{error}），还原文件失败（{undo}）"))?;
        return Err(error);
    }
    Ok(result)
}

// Update every reference as one batch. A failed write restores previous metadata before the native move is undone.
fn update_paths(
    workspace: &mut Workspace,
    folders: &mut Folders,
    notes: &mut Notes,
    source: &Path,
    target: Option<PathBuf>,
    result: &mut FileResult,
) -> Result<(), String> {
    let mut edits = Vec::new();
    let roots: Vec<_> = workspace
        .roots
        .iter()
        .filter_map(|root| {
            if root.starts_with(source) {
                target.as_ref().map(|to| map_path(root, source, to))
            } else {
                Some(root.clone())
            }
        })
        .collect();
    if roots != workspace.roots {
        edits.push(MetadataEdit::put(workspace.file.clone(), &roots)?);
    }
    let mapped_folders: Vec<_> = folders
        .items
        .iter()
        .filter_map(|folder| {
            let mut folder = folder.clone();
            if folder.path.starts_with(source) {
                folder.path = map_path(&folder.path, source, target.as_ref()?);
            }
            Some(folder)
        })
        .collect();
    if mapped_folders != folders.items {
        edits.push(MetadataEdit::put(folders.file.clone(), &mapped_folders)?);
    }

    let mut changed = Vec::new();
    let mut removed = Vec::new();
    for mut note in notes.under(source) {
        let metadata = notes.metadata_path(note.id);
        if let Some(to) = &target {
            let path = map_path(note.disk_path().unwrap(), source, to);
            if note.location.is_some() {
                note.name = file_stem(&path);
                let parent = mapped_folders
                    .iter()
                    .filter(|f| path.starts_with(&f.path))
                    .max_by_key(|f| f.path.components().count());
                note.location = if path.parent() == Some(notes.inbox_directory().as_path()) {
                    Some(NoteLocation {
                        folder: None,
                        path: path.clone(),
                    })
                } else {
                    parent.map(|f| NoteLocation {
                        folder: Some(f.id),
                        path: path.clone(),
                    })
                };
                note.file = note.location.is_none().then_some(path);
            } else {
                note.file = Some(path);
            }
            edits.push(MetadataEdit::put(metadata, &note)?);
            changed.push(note);
        } else {
            edits.push(MetadataEdit::change(metadata, None)?);
            removed.push(note.id);
        }
    }
    commit(&edits)?;
    result.folders_changed = mapped_folders != folders.items;
    workspace.roots = roots;
    folders.items = mapped_folders;
    for note in changed {
        notes.update_metadata(note);
    }
    for &id in &removed {
        notes.forget(id);
    }
    result.path = target;
    result.removed = removed;
    Ok(())
}

fn perform_workspace(
    mut workspace: MutexGuard<'_, Workspace>,
    folders: &Mutex<Folders>,
    notes: &RwLock<Notes>,
    source: &Path,
    action: FileAction,
) -> Result<FileResult, String> {
    let result = |path| {
        Ok(FileResult {
            path: Some(path),
            ..Default::default()
        })
    };
    let to = match action {
        FileAction::CreateFile { name } => {
            drop(workspace);
            let path = if let Some(name) = name {
                let path = source.join(markdown_name(&name)?);
                create_markdown(&path, "").map_err(|e| e.to_string())?;
                path
            } else {
                crate::storage::create_named_markdown(source, crate::i18n::t("未命名"), "")?
            };
            return result(path);
        }
        FileAction::CreateDirectory { name } => {
            drop(workspace);
            let path = source.join(filename(&name)?);
            fs::create_dir(&path).map_err(|e| e.to_string())?;
            return result(path);
        }
        FileAction::Duplicate => {
            drop(workspace);
            return result(copy_sibling(source)?);
        }
        FileAction::Rename { name } => {
            let name = if source.is_dir() {
                filename(&name)?.to_owned()
            } else {
                markdown_name(&name)?
            };
            Some(source.parent().ok_or("不能修改磁盘根目录")?.join(name))
        }
        FileAction::Move { directory } => {
            let directory = workspace.resolve(&directory)?;
            if !directory.is_dir() {
                return Err("请选择目标文件夹".into());
            }
            Some(directory.join(source.file_name().ok_or("不能移动磁盘根目录")?))
        }
        FileAction::Trash => None,
    };
    relocate(
        &mut workspace,
        &mut folders.lock().unwrap(),
        &mut notes.write().unwrap(),
        source,
        to,
    )
}

fn notify(app: &AppHandle, removed: Vec<NoteId>, folders_changed: bool) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if !removed.is_empty() {
            let _ = handle.emit("desktop:notes-removed", &removed);
            crate::desktop::notes_removed(&handle, &removed);
        }
        let _ = handle.emit("desktop:files-changed", ());
        if folders_changed {
            let _ = handle.emit(events::FOLDERS_CHANGED, ());
        }
        let _ = handle.emit(events::NOTES_CHANGED, ());
    });
}

#[tauri::command]
pub async fn workspace_action(
    app: AppHandle,
    path: PathBuf,
    action: FileAction,
) -> Result<FileResult, String> {
    let result = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || {
            let _mutation = PATH_CHANGES.lock().unwrap();
            let w = app.state::<Mutex<Workspace>>();
            let f = app.state::<Mutex<Folders>>();
            let n = app.state::<RwLock<Notes>>();
            let workspace = w.lock().unwrap();
            let source = workspace.resolve(&path)?;
            let data = app
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())?
                .canonicalize()
                .map_err(|e| e.to_string())?;
            let moving_parent = matches!(
                action,
                FileAction::Trash | FileAction::Move { .. } | FileAction::Rename { .. }
            ) && data.starts_with(&source);
            let moving_into_data = match &action {
                FileAction::Move { directory } => workspace.resolve(directory)?.starts_with(&data),
                _ => false,
            };
            if source.starts_with(&data) || moving_parent || moving_into_data {
                return Err("Jotiva 正在使用此应用数据目录".into());
            }
            perform_workspace(workspace, &f, &n, &source, action)
        }
    })
    .await
    .map_err(|e| e.to_string())??;
    notify(&app, result.removed.clone(), result.folders_changed);
    Ok(result)
}

// Validate the whole selection before touching disk. Each completed native move also commits
// its index changes; if a volume disconnects partway, publish the completed subset once.
fn change_selection(
    workspace: &mut Workspace,
    folders: &mut Folders,
    notes: &RwLock<Notes>,
    paths: Vec<PathBuf>,
    directory: Option<PathBuf>,
    changed: &mut FileResult,
) -> Result<(), String> {
    let inbox = notes.read().unwrap().inbox_directory();
    let data = inbox
        .parent()
        .and_then(Path::parent)
        .expect("notes reside in app data");
    let directory = directory.map(|path| workspace.resolve(&path)).transpose()?;
    if directory
        .as_ref()
        .is_some_and(|p| !p.is_dir() || p.starts_with(data))
    {
        return Err("请选择工作区内的目标文件夹".into());
    }
    let mut unique = HashSet::new();
    let mut targets = HashSet::new();
    let mut moves = Vec::new();
    for path in paths {
        if !unique.insert(path.clone()) {
            continue;
        }
        let in_workspace = workspace.roots.iter().any(|root| path.starts_with(root));
        let parent = path.parent().ok_or("无效文件路径")?;
        let in_archive = parent == inbox || folders.items.iter().any(|f| parent == f.path);
        if (!in_workspace && !in_archive)
            || (directory.is_some() && !in_workspace)
            || (path.starts_with(data) && parent != inbox)
            || !is_document(&path)
            || fs::canonicalize(&path).as_ref().ok() != Some(&path)
            || !path.is_file()
        {
            return Err(format!("文件已移出目录或不可用：{}", path.display()));
        }
        let to = directory
            .as_ref()
            .map(|d| d.join(path.file_name().unwrap()));
        if let Some(target) = &to {
            if target == &path {
                continue;
            }
            if target.try_exists().map_err(|e| e.to_string())? || !targets.insert(target.clone()) {
                return Err(format!("目标中有同名文件：{}", target.display()));
            }
        }
        moves.push((path, to));
    }
    let total = moves.len();
    for (completed, (path, target)) in moves.into_iter().enumerate() {
        let one = relocate(
            workspace,
            folders,
            &mut notes.write().unwrap(),
            &path,
            target,
        )
        .map_err(|error| {
            format!(
                "已处理 {completed}/{total} 个文件，{}：{error}",
                path.display()
            )
        })?;
        changed.removed.extend(one.removed);
        changed.folders_changed |= one.folders_changed;
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SelectionAction {
    Trash,
    Move { directory: PathBuf },
}

#[tauri::command]
pub async fn file_selection_action(
    app: AppHandle,
    paths: Vec<PathBuf>,
    action: SelectionAction,
) -> Result<(), String> {
    let directory = match action {
        SelectionAction::Trash => None,
        SelectionAction::Move { directory } => Some(directory),
    };
    let handle = app.clone();
    let (result, changed) = tauri::async_runtime::spawn_blocking(move || {
        let _mutation = PATH_CHANGES.lock().unwrap();
        let w = handle.state::<Mutex<Workspace>>();
        let f = handle.state::<Mutex<Folders>>();
        let n = handle.state::<RwLock<Notes>>();
        let mut changed = FileResult::default();
        let result = change_selection(
            &mut w.lock().unwrap(),
            &mut f.lock().unwrap(),
            &n,
            paths,
            directory,
            &mut changed,
        );
        (result, changed)
    })
    .await
    .map_err(|e| e.to_string())?;
    notify(&app, changed.removed, changed.folders_changed);
    result
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NoteAction {
    Rename { name: String },
    Duplicate,
    Trash,
}

#[tauri::command]
pub async fn create_archive_note(
    app: AppHandle,
    folder: Option<u64>,
    name: String,
) -> Result<NoteId, String> {
    let title = archive_title(&name)?;
    let id = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || {
            let _mutation = PATH_CHANGES.lock().unwrap();
            let f = app.state::<Mutex<Folders>>();
            let folders = f.lock().unwrap();
            let destination = folder.map(|id| folders.destination(id)).transpose()?;
            app.state::<RwLock<Notes>>().write().unwrap().create_named(
                title,
                String::new(),
                destination,
            )
        }
    })
    .await
    .map_err(|error| error.to_string())??;
    notify(&app, Vec::new(), false);
    Ok(id)
}

// Reserve each candidate with an exclusive filesystem rename. Other app windows and
// external writers cannot overwrite a document between choosing and using its name.
fn name_created_file(
    workspace: &mut Workspace,
    folders: &mut Folders,
    notes: &mut Notes,
    id: NoteId,
    name: &str,
) -> Result<Option<String>, String> {
    let note = notes.note(id).ok_or("找不到便签")?;
    let source = note.disk_path().ok_or("找不到文件")?;
    if !fs::symlink_metadata(source)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("原位置已不是文档".into());
    }
    let filename = if note.file.is_some() {
        markdown_name(name)?
    } else {
        format!("{}.md", archive_title(name)?)
    };
    let requested = source.with_file_name(&filename);
    let actual = crate::storage::numbered_path(
        requested.parent().ok_or("无效名称")?,
        requested
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("无效名称")?,
        requested
            .extension()
            .and_then(|s| s.to_str())
            .ok_or("无效名称")?,
        |candidate| {
            if candidate == source {
                Ok(())
            } else {
                crate::platform::storage::rename_exclusive(source, candidate)
            }
        },
    )
    .map_err(|e| e.to_string())?;
    let adjusted =
        (actual != requested).then(|| actual.file_name().unwrap().to_string_lossy().into_owned());
    if actual != source {
        finish_relocation(workspace, folders, notes, source, Some(actual), None)?;
    }
    Ok(adjusted)
}

#[tauri::command]
pub async fn name_created_document(
    app: AppHandle,
    id: NoteId,
    name: String,
) -> Result<Option<String>, String> {
    let handle = app.clone();
    let adjusted = tauri::async_runtime::spawn_blocking(move || {
        let _mutation = PATH_CHANGES.lock().unwrap();
        let w = handle.state::<Mutex<Workspace>>();
        let f = handle.state::<Mutex<Folders>>();
        let n = handle.state::<RwLock<Notes>>();
        name_created_file(
            &mut w.lock().unwrap(),
            &mut f.lock().unwrap(),
            &mut n.write().unwrap(),
            id,
            &name,
        )
    })
    .await
    .map_err(|e| e.to_string())??;
    let handle = app.clone();
    app.run_on_main_thread(move || crate::desktop::note_saved(&handle, id))
        .map_err(|e| e.to_string())?;
    notify(&app, Vec::new(), false);
    Ok(adjusted)
}

#[tauri::command]
pub async fn note_action(app: AppHandle, id: NoteId, action: NoteAction) -> Result<NoteId, String> {
    let renamed = matches!(action, NoteAction::Rename { .. });
    let (id, removed) = tauri::async_runtime::spawn_blocking({
        let app = app.clone();
        move || -> Result<_, String> {
            let _mutation = PATH_CHANGES.lock().unwrap();
            let w = app.state::<Mutex<Workspace>>();
            let f = app.state::<Mutex<Folders>>();
            let n = app.state::<RwLock<Notes>>();
            let mut workspace = w.lock().unwrap();
            let mut folders = f.lock().unwrap();
            let mut notes = n.write().unwrap();
            manage_note(&mut workspace, &mut folders, &mut notes, id, action)
        }
    })
    .await
    .map_err(|e| e.to_string())??;
    if renamed {
        let handle = app.clone();
        app.run_on_main_thread(move || crate::desktop::note_saved(&handle, id))
            .map_err(|e| e.to_string())?;
    }
    notify(&app, removed, false);
    Ok(id)
}

fn manage_note(
    workspace: &mut Workspace,
    folders: &mut Folders,
    notes: &mut Notes,
    id: NoteId,
    action: NoteAction,
) -> Result<(NoteId, Vec<NoteId>), String> {
    let note = notes.note(id).ok_or("找不到便签")?;
    if note.disk_path().is_none()
        && let NoteAction::Rename { name } = &action
    {
        notes.name_unfiled_draft(note, archive_title(name)?)?;
        return Ok((id, Vec::new()));
    }
    let path = note.disk_path().ok_or("请先输入并保存正文")?;
    {
        let metadata =
            fs::symlink_metadata(path).map_err(|e| format!("无法读取 {}：{e}", path.display()))?;
        if !metadata.is_file() || !is_document(path) {
            return Err("原位置已不是 Markdown 文件".into());
        }
    }
    let result = match action {
        NoteAction::Duplicate => {
            if let Some(path) = &note.file {
                let copy = copy_sibling(path)?;
                let copy_id = notes
                    .open_file(&copy)
                    .map_err(|e| format!("副本已保存到 {}，但未能打开：{e}", copy.display()))?;
                return Ok((copy_id, Vec::new()));
            }
            let folder = note
                .location
                .as_ref()
                .and_then(|loc| loc.folder)
                .map(|id| folders.destination(id))
                .transpose()?;
            return Ok((notes.duplicate(id, folder)?, Vec::new()));
        }
        NoteAction::Rename { name } => {
            let filename = if note.file.is_some() {
                markdown_name(&name)?
            } else {
                format!("{}.md", archive_title(&name)?)
            };
            relocate(
                workspace,
                folders,
                notes,
                path,
                Some(path.with_file_name(filename)),
            )?
        }
        NoteAction::Trash => relocate(workspace, folders, notes, path, None)?,
    };
    Ok((id, result.removed))
}

#[tauri::command]
pub fn reveal_file(path: PathBuf) -> Result<(), String> {
    if !path.exists() {
        return Err("此路径已不存在".into());
    }
    Native::reveal(&path)
}

#[cfg(test)]
mod tests;
