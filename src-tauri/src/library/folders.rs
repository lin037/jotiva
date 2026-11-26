use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::storage::write_json;

pub type FolderId = u64;

#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub struct Folder {
    pub id: FolderId,
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    #[serde(default)]
    pub classify: bool,
}

#[derive(Deserialize)]
pub struct FolderInput {
    pub id: Option<FolderId>,
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub classify: bool,
}

pub struct Folders {
    pub(super) file: PathBuf,
    pub(super) items: Vec<Folder>,
    temporary_roots: Vec<PathBuf>,
}

pub use crate::platform::dialogs::{choose, choose_markdown, show_error};

impl Folders {
    pub fn load(file: PathBuf) -> io::Result<Self> {
        Self::load_with_roots(file, crate::storage::system_temporary_roots())
    }

    fn load_with_roots(file: PathBuf, temporary_roots: Vec<PathBuf>) -> io::Result<Self> {
        let items = match fs::read(&file) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(error),
        };
        Ok(Self {
            file,
            items,
            temporary_roots,
        })
    }

    #[cfg(test)]
    pub(crate) fn load_fixture(file: PathBuf) -> io::Result<Self> {
        let temporary = file.with_file_name("system-temporary");
        Self::load_with_roots(file, vec![temporary])
    }

    pub fn list(&self) -> Vec<Folder> {
        self.items.clone()
    }

    pub fn filing_candidates(&self) -> Vec<Folder> {
        self.items
            .iter()
            .filter(|folder| folder.classify && self.check_destination(&folder.path).is_ok())
            .cloned()
            .collect()
    }

    pub fn get(&self, id: FolderId) -> Result<&Folder, String> {
        self.items
            .iter()
            .find(|folder| folder.id == id)
            .ok_or_else(|| "找不到文件夹".into())
    }

    pub fn destination(&self, id: FolderId) -> Result<&Folder, String> {
        let folder = self.get(id)?;
        self.check_destination(&folder.path)?;
        Ok(folder)
    }

    fn check_destination(&self, path: &Path) -> Result<(), String> {
        if self
            .temporary_roots
            .iter()
            .any(|root| path.starts_with(root))
        {
            return Err("系统可能清理临时目录。请选择「文稿」或其他长期保留的文件夹。".into());
        }
        if !path.is_dir() || fs::canonicalize(path).ok().as_deref() != Some(path) {
            return Err("归档目录已移动或无法访问，请检查文件夹的位置。".into());
        }
        Ok(())
    }

    pub fn save(&mut self, input: FolderInput, has_documents: bool) -> Result<Folder, String> {
        let name = input.name.trim();
        if name.is_empty() || name == "其他" {
            return Err("请输入文件夹名称，「其他」由 Jotiva 管理".into());
        }
        if !input.path.is_absolute() {
            return Err("请选择一个已有的文件夹".into());
        }
        let path =
            fs::canonicalize(&input.path).map_err(|error| format!("无法打开文件夹：{error}"))?;
        self.check_destination(&path)?;
        let data = self
            .file
            .parent()
            .expect("folder configuration has a parent");
        let storage = data.join("notes");
        if path == data || path.starts_with(&storage) {
            return Err("此目录用于 Jotiva 的便签存储，请选择其他目录".into());
        }
        let mut items = self.items.clone();
        let id = match input.id {
            Some(id) => {
                let previous = self.get(id)?;
                if has_documents && previous.path != path {
                    return Err("请先将文档移出此文件夹，再更换路径".into());
                }
                id
            }
            None => items.iter().map(|folder| folder.id).max().unwrap_or(0) + 1,
        };
        if items
            .iter()
            .any(|folder| folder.id != id && (folder.name == name || folder.path == path))
        {
            return Err("已有同名文件夹或此路径已经添加".into());
        }
        let folder = Folder {
            id,
            name: name.into(),
            description: input.description.trim().into(),
            path,
            classify: input.classify,
        };
        if let Some(previous) = items.iter_mut().find(|entry| entry.id == id) {
            *previous = folder.clone();
        } else {
            items.push(folder.clone());
        }
        write_json(&self.file, &items).map_err(|error| error.to_string())?;
        self.items = items;
        Ok(folder)
    }
}

#[derive(Serialize)]
pub struct ArchiveEntry {
    path: PathBuf,
    id: Option<super::notes::NoteId>,
    managed: bool,
}

// An archive destination represents its direct Markdown files. Only opening a
// file reads its contents and registers a note; listing never adopts a file.
fn markdown_paths(directory: &std::path::Path) -> Result<Vec<PathBuf>, String> {
    let checked = fs::canonicalize(directory).map_err(|e| e.to_string())?;
    if checked != directory {
        return Err("归档目录的位置已改变，请重新选择目录".into());
    }
    let mut paths = Vec::new();
    for child in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let child = child.map_err(|e| e.to_string())?;
        if !child.file_name().to_string_lossy().starts_with('.')
            && super::workspace::is_document(&child.path())
            && child.file_type().map_err(|e| e.to_string())?.is_file()
        {
            paths.push(child.path());
        }
    }
    paths.sort_by_cached_key(|p| p.file_name().unwrap().to_string_lossy().to_lowercase());
    Ok(paths)
}

#[tauri::command]
pub async fn archive_entries(
    app: tauri::AppHandle,
    folder: Option<FolderId>,
) -> Result<Vec<ArchiveEntry>, String> {
    use std::sync::{Mutex, RwLock};
    use tauri::Manager;
    tauri::async_runtime::spawn_blocking(move || {
        let directory = match folder {
            Some(id) => app
                .state::<Mutex<Folders>>()
                .lock()
                .unwrap()
                .get(id)?
                .path
                .clone(),
            None => app
                .state::<RwLock<super::notes::Notes>>()
                .read()
                .unwrap()
                .inbox_directory(),
        };
        let paths = markdown_paths(&directory)?;
        let state = app.state::<RwLock<super::notes::Notes>>();
        let notes = state.read().unwrap();
        Ok(paths
            .into_iter()
            .map(|path| {
                let card = notes.card_at_path(&path);
                ArchiveEntry {
                    id: card.map(|c| c.id),
                    managed: folder.is_none()
                        || card.is_some_and(|c| c.file.is_none() && c.folder == folder),
                    path,
                }
            })
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_temporary_archives_remain_readable_but_cannot_receive_notes() {
        let base =
            std::env::temp_dir().join(format!("jot-old-temp-archive-{}", std::process::id()));
        fs::create_dir(&base).unwrap();
        let base = fs::canonicalize(base).unwrap();
        let old = base.join("archive");
        fs::create_dir(&old).unwrap();
        let note = old.join("already-here.md");
        fs::write(&note, "keep my note").unwrap();
        let alias = base.join("alias");
        crate::platform::test_links::directory(&old, &alias).unwrap();
        let config = base.join("folders.json");
        write_json(
            &config,
            &serde_json::json!([{
                "id": 1, "name": "Old", "description": "", "path": old, "classify": true,
            }]),
        )
        .unwrap();
        let mut folders = Folders::load(config).unwrap();
        assert_eq!(
            markdown_paths(&folders.get(1).unwrap().path).unwrap(),
            std::slice::from_ref(&note)
        );
        assert!(folders.destination(1).err().unwrap().contains("临时目录"));
        assert!(folders.filing_candidates().is_empty());
        let input = FolderInput {
            id: None,
            name: "Alias".into(),
            description: String::new(),
            path: alias,
            classify: true,
        };
        assert!(
            folders
                .save(input, false)
                .err()
                .unwrap()
                .contains("临时目录")
        );
        assert_eq!(folders.list().len(), 1);
        assert_eq!(fs::read_to_string(note).unwrap(), "keep my note");
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn directory_import_lists_direct_files_and_preserves_classification_choices() {
        let temp = std::env::temp_dir().join(format!("jot-archive-import-{}", std::process::id()));
        fs::create_dir(&temp).unwrap();
        let base = temp.canonicalize().unwrap();
        let shared = base.join("shared");
        let imported = base.join("import");
        let data = base.join("data");
        let inbox = data.join("notes/其他");
        fs::create_dir_all(&inbox).unwrap();
        fs::create_dir(&shared).unwrap();
        fs::create_dir_all(imported.join("nested")).unwrap();
        for file in [
            "a.md",
            "B.MARKDOWN",
            ".hidden.md",
            "x.txt",
            "nested/deep.md",
        ] {
            fs::write(imported.join(file), "# Existing file").unwrap();
        }
        crate::platform::test_links::file(imported.join("a.md"), imported.join("link.md")).unwrap();
        let config = data.join("folders.json");
        write_json(
            &config,
            &serde_json::json!([{"id":1, "name":"Old", "description":"", "path":shared}]),
        )
        .unwrap();
        let mut folders = Folders::load_fixture(config.clone()).unwrap();
        let input = |id, path: &std::path::Path, classify| FolderInput {
            id,
            name: "Imported".into(),
            description: String::new(),
            path: path.into(),
            classify,
        };
        let added = folders.save(input(None, &imported, false), false).unwrap();
        assert_eq!(
            markdown_paths(&added.path).unwrap(),
            [
                imported.join("a.md"),
                imported.join("B.MARKDOWN"),
                imported.join("x.txt")
            ]
        );
        assert_eq!(
            fs::read_to_string(imported.join("a.md")).unwrap(),
            "# Existing file"
        );
        assert_eq!(fs::read_dir(&inbox).unwrap().count(), 0);
        let enabled = |registry: &Folders| {
            registry
                .filing_candidates()
                .into_iter()
                .map(|folder| folder.id)
                .collect::<Vec<_>>()
        };
        assert!(enabled(&folders).is_empty());
        let renamed = FolderInput {
            name: "Meetings".into(),
            description: "Weekly discussions and action items".into(),
            ..input(Some(added.id), &imported, true)
        };
        folders.save(renamed, true).unwrap();
        let reloaded = Folders::load_fixture(config.clone())
            .unwrap()
            .filing_candidates();
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].id, added.id);
        assert_eq!(reloaded[0].name, "Meetings");
        assert_eq!(
            reloaded[0].description,
            "Weekly discussions and action items"
        );
        assert_eq!(reloaded[0].path, imported);
        folders
            .save(input(Some(added.id), &imported, false), false)
            .unwrap();
        assert!(enabled(&Folders::load_fixture(config).unwrap()).is_empty());
        for reserved in [&data, &inbox] {
            assert!(
                folders
                    .save(input(None, reserved, false), false)
                    .err()
                    .unwrap()
                    .contains("便签存储")
            );
        }
        fs::remove_dir_all(base).unwrap();
    }
}
