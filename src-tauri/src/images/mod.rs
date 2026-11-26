mod config;
use config::{Config, Destination, FolderRule, existing_directory};
mod local;
mod upload;

use crate::{
    events,
    library::{
        folders::Folders,
        notes::{NoteId, Notes},
        workspace::Workspace,
    },
    platform,
    settings::filing::{KeyAction, http_endpoint},
    storage, windows,
};
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Mutex, RwLock},
};
use tauri::{
    AppHandle, Emitter, Manager, WebviewWindow, http,
    ipc::{InvokeBody, Request},
};

const LIMIT: usize = 32 * 1024 * 1024;

#[derive(Default, Deserialize, Serialize)]
struct Saved {
    config: Config,
    // Previously selected folders keep their existing Markdown image links readable.
    #[serde(default)]
    directories: Vec<PathBuf>,
}

pub struct Images {
    file: PathBuf,
    saved: Saved,
    revision: u64,
}

impl Images {
    pub fn load(file: PathBuf) -> io::Result<Self> {
        let saved = match fs::read(&file) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(e) if e.kind() == io::ErrorKind::NotFound => Saved::default(),
            Err(e) => return Err(e),
        };
        Ok(Self {
            file,
            saved,
            revision: 0,
        })
    }

    fn view(&self, folder: Option<PathBuf>) -> SettingsView {
        let config = &self.saved.config;
        SettingsView {
            revision: self.revision,
            folder: folder.map(|path| FolderContext {
                effective_rule: config.rule_at(&path).cloned(),
                inherited_rule: path
                    .parent()
                    .and_then(|parent| config.rule_at(parent))
                    .cloned(),
                path,
            }),
            config: config.clone(),
        }
    }

    fn check_revision(&self, revision: u64) -> Result<(), String> {
        if revision != self.revision {
            return Err("图片设置已在别处更新，请重新载入后再保存。".into());
        }
        Ok(())
    }

    fn save(&mut self, mut config: Config) -> Result<(), String> {
        config.validate(&self.saved.config)?;
        let mut directories = self.saved.directories.clone();
        if !config.directory.as_os_str().is_empty() && !directories.contains(&config.directory) {
            directories.push(config.directory.clone());
        }
        for rule in &config.rules {
            if let Some((base, parts)) = rule.fixed_path()? {
                let directory = base.join(parts);
                if !directories.contains(&directory) {
                    directories.push(directory);
                }
            }
        }
        let saved = Saved {
            config,
            directories,
        };
        storage::write_json(&self.file, &saved).map_err(|e| e.to_string())?;
        self.saved = saved;
        self.revision += 1;
        Ok(())
    }
}

fn settings_folder(
    window: &WebviewWindow,
    folder: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    let path = match (window.label(), folder) {
        (windows::SETTINGS, None) => return Ok(None),
        (windows::MAIN, Some(path)) => existing_directory(path)?,
        _ => return Err("请在设置或文件夹菜单中管理图片".into()),
    };
    let app = window.app_handle();
    if app
        .state::<Mutex<Workspace>>()
        .lock()
        .unwrap()
        .resolve(&path)
        .is_ok()
        || app
            .state::<Mutex<Folders>>()
            .lock()
            .unwrap()
            .list()
            .iter()
            .any(|folder| path == folder.path)
        || path
            == app
                .state::<RwLock<Notes>>()
                .read()
                .unwrap()
                .inbox_directory()
    {
        return Ok(Some(path));
    }
    Err("这个文件夹已移出工作区或归档目录。".into())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    revision: u64,
    config: Config,
    folder: Option<FolderContext>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FolderContext {
    path: PathBuf,
    effective_rule: Option<FolderRule>,
    inherited_rule: Option<FolderRule>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadSettings {
    endpoint: String,
    field: String,
    response_pointer: String,
    bearer: bool,
}

#[tauri::command]
pub fn read_image_settings(
    window: WebviewWindow,
    folder: Option<PathBuf>,
) -> Result<SettingsView, String> {
    let folder = settings_folder(&window, folder.as_deref())?;
    Ok(window.state::<Mutex<Images>>().lock().unwrap().view(folder))
}

#[tauri::command]
pub fn save_image_settings(
    window: WebviewWindow,
    config: Config,
    revision: u64,
) -> Result<SettingsView, String> {
    settings_folder(&window, None)?;
    let state = window.state::<Mutex<Images>>();
    let mut state = state.lock().unwrap();
    state.check_revision(revision)?;
    state.save(config)?;
    Ok(state.view(None))
}

#[tauri::command]
pub fn save_image_folder(
    window: WebviewWindow,
    folder: PathBuf,
    rule: Option<FolderRule>,
    upload: Option<UploadSettings>,
    revision: u64,
) -> Result<(), String> {
    let path = settings_folder(&window, Some(&folder))?.ok_or("请选择已有的目录")?;
    {
        let state = window.state::<Mutex<Images>>();
        let mut state = state.lock().unwrap();
        state.check_revision(revision)?;
        let mut config = state.saved.config.clone();
        let index = config.rules.iter().position(|rule| rule.path == path);
        if let Some(mut rule) = rule {
            rule.path = path;
            if let Some(upload) = upload {
                if rule.mode != Destination::Http {
                    return Err("仅 HTTP 上传可修改上传服务".into());
                }
                config.endpoint = upload.endpoint;
                config.field = upload.field;
                config.response_pointer = upload.response_pointer;
                config.bearer = upload.bearer;
            }
            if let Some(index) = index {
                config.rules[index] = rule;
            } else {
                config.rules.push(rule);
            }
        } else if let Some(index) = index {
            config.rules.remove(index);
        } else {
            return Ok(());
        }
        state.save(config)?;
    }
    let _ = window.emit_to(windows::SETTINGS, events::IMAGE_SETTINGS_CHANGED, ());
    Ok(())
}

#[tauri::command]
pub async fn image_key(
    window: WebviewWindow,
    folder: Option<PathBuf>,
    address: String,
    action: KeyAction,
) -> Result<bool, String> {
    settings_folder(&window, folder.as_deref())?;
    let url = http_endpoint(&address)?;
    let service = format!("{}.images", window.config().identifier);
    tauri::async_runtime::spawn_blocking(move || {
        platform::filing::credential(&service, url.as_str(), action)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[derive(Serialize)]
pub struct Options {
    compress: bool,
    revision: u64,
    mode: Destination,
}

#[tauri::command]
pub fn image_options(app: AppHandle, id: NoteId) -> Result<Options, String> {
    let path = document(&app, id)?;
    let state = app.state::<Mutex<Images>>();
    let state = state.lock().unwrap();
    Ok(Options {
        mode: state.saved.config.mode_for(&path),
        compress: state.saved.config.compress,
        revision: state.revision,
    })
}

fn document(app: &AppHandle, id: NoteId) -> Result<PathBuf, String> {
    let notes = app.state::<RwLock<Notes>>();
    let notes = notes.read().unwrap();
    if let Some(path) = notes.disk_path(id) {
        return Ok(path.to_owned());
    }
    if notes.title(id).is_none() {
        return Err("找不到图片所属的文档".into());
    }
    Ok(notes.inbox_directory().join(format!("{id}.md")))
}

fn raster(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("png", "image/png"))
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(("jpg", "image/jpeg"))
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(("gif", "image/gif"))
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some(("webp", "image/webp"))
    } else if bytes.get(4..8) == Some(b"ftyp")
        && matches!(bytes.get(8..12), Some(b"avif" | b"avis"))
    {
        Some(("avif", "image/avif"))
    } else {
        None
    }
}

#[tauri::command]
pub async fn store_note_image(app: AppHandle, request: Request<'_>) -> Result<String, String> {
    let number = |key: &str| {
        request
            .headers()
            .get(key)
            .and_then(|s| s.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or("无效的图片请求")
    };
    let id = number("x-jot-note")?;
    let revision = number("x-jot-revision")?;
    let path = document(&app, id)?;
    let (config, mode) = {
        let state = app.state::<Mutex<Images>>();
        let state = state.lock().unwrap();
        let mode = state.saved.config.mode_for(&path);
        let expected = request
            .headers()
            .get("x-jot-mode")
            .and_then(|h| h.to_str().ok());
        if revision != state.revision || expected != Some(mode.as_str()) {
            return Err("图片设置已经改变，请重新插入图片".into());
        }
        (state.saved.config.clone(), mode)
    };
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err("缺少图片数据".into());
    };
    if bytes.len() > LIMIT {
        return Err("图片超过 32 MiB".into());
    }
    let (ext, mime) = raster(bytes).ok_or("支持 PNG、JPEG、GIF、WebP 和 AVIF 图片")?;
    let name = request
        .headers()
        .get("x-jot-name")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("image");
    let decoded = percent_encoding::percent_decode_str(name).decode_utf8_lossy();
    let stem = local::safe_stem(
        Path::new(decoded.as_ref())
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("image"),
    );
    let bytes = bytes.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if mode == Destination::Http {
            let service = format!("{}.images", app.config().identifier);
            let key = if config.bearer {
                Some(upload::credential(&service, &config.endpoint)?)
            } else {
                None
            };
            upload::send(
                &config,
                &format!("{stem}-{}.{}", uuid::Uuid::new_v4(), ext),
                mime,
                bytes,
                key.as_deref(),
            )
        } else {
            local::save(&path, &config, mode, &stem, ext, &bytes)
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

pub fn serve(
    context: tauri::UriSchemeContext<'_, tauri::Wry>,
    request: http::Request<Vec<u8>>,
    responder: tauri::UriSchemeResponder,
) {
    let app = context.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let read = || {
            let route =
                percent_encoding::percent_decode_str(request.uri().path().strip_prefix('/')?)
                    .decode_utf8()
                    .ok()?;
            let (id, dest) = route.split_once('/')?;
            let doc = document(&app, id.parse().ok()?).ok()?;
            let roots = app
                .state::<Mutex<Images>>()
                .lock()
                .unwrap()
                .saved
                .directories
                .clone();
            local::read(&doc, dest, &roots)
        };
        let (code, mime, bytes) = match read() {
            Some((mime, bytes)) => (200, mime, bytes),
            None => (404, "text/plain", Vec::new()),
        };
        responder.respond(
            http::Response::builder()
                .status(code)
                .header("Content-Type", mime)
                .header("X-Content-Type-Options", "nosniff")
                .header("Cache-Control", "no-store")
                .body(bytes)
                .unwrap(),
        );
    });
}

#[cfg(test)]
mod tests;
