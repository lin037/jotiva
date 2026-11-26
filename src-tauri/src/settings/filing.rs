use std::{
    collections::{BTreeMap, HashMap},
    fs, io,
    path::PathBuf,
    sync::{Mutex, RwLock},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, ipc::Channel};
use tokio::sync::Semaphore;

use crate::{
    desktop, events,
    library::{
        folders::{Folder, FolderId, Folders},
        notes::{NoteId, Notes},
    },
    storage::{check_markdown, write_json},
    windows,
};

const DEFAULT_MODEL: &str = "jev-latest";

fn default_model() -> String {
    DEFAULT_MODEL.into()
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AddressKind {
    Base,
    #[default]
    Endpoint,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    enabled: bool,
    endpoint: String,
    // Existing installations stored the exact request URL, including custom routes.
    #[serde(default)]
    address_kind: AddressKind,
    #[serde(default = "default_model")]
    model: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: String::new(),
            address_kind: AddressKind::Base,
            model: default_model(),
        }
    }
}

impl AddressKind {
    fn service_url(self, address: &str) -> Result<String, String> {
        let mut url = endpoint(address)?;
        if matches!(self, Self::Base) {
            let base = url.path().trim_end_matches('/');
            let suffix = if base.ends_with("/v1") {
                "/systemone"
            } else {
                "/v1/systemone"
            };
            url.set_path(&format!("{base}{suffix}"));
        }
        Ok(url.into())
    }
}

impl Config {
    fn service_url(&self) -> Result<String, String> {
        self.address_kind.service_url(&self.endpoint)
    }
}

fn model_name(value: &str) -> Result<String, String> {
    let name = value.trim();
    if name.is_empty() {
        return Ok(default_model());
    }
    if name.chars().count() > 128 || name.chars().any(char::is_control) {
        return Err("模型名称最多 128 个字符，不能包含控制字符".into());
    }
    Ok(name.into())
}

pub(crate) fn endpoint(value: &str) -> Result<tauri::Url, String> {
    parse_http_endpoint(value, false)
}

pub(crate) fn http_endpoint(value: &str) -> Result<tauri::Url, String> {
    parse_http_endpoint(value, true)
}

fn parse_http_endpoint(value: &str, allow_remote_http: bool) -> Result<tauri::Url, String> {
    let value = value.trim();
    let (_, rest) = value.split_once("://").ok_or("请填写完整的服务地址")?;
    if value.contains('\\') || value.chars().any(char::is_whitespace) {
        return Err("请填写有效的服务地址".into());
    }
    let url = tauri::Url::parse(value).map_err(|_| "请填写有效的服务地址")?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if !url.has_host()
        || authority.contains('@')
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("地址中不能含有账号、密码、查询参数或片段".into());
    }
    if !matches!(url.scheme(), "http" | "https") {
        return Err("请填写 HTTP 或 HTTPS 服务地址".into());
    }
    if url.scheme() == "http" && !allow_remote_http {
        let numeric_loopback = matches!(url.host_str(), Some("127.0.0.1" | "[::1]"))
            && url.host_str().is_some_and(|host| {
                authority == host
                    || authority
                        .strip_prefix(host)
                        .and_then(|rest| rest.strip_prefix(':'))
                        .is_some_and(|port| {
                            !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit())
                        })
            });
        if !numeric_loopback {
            return Err("远程服务须使用 HTTPS。本机 HTTP 服务请填写 127.0.0.1 或 [::1]。".into());
        }
    }
    Ok(url)
}

pub struct Filing {
    file: PathBuf,
    config: Config,
    sequence: u64,
    attempts: HashMap<NoteId, Attempt>,
}

struct Attempt {
    version: u64,
    ticket: u64,
    suggestion: Option<Folder>,
}

impl Filing {
    pub fn load(file: PathBuf) -> io::Result<Self> {
        let config = match fs::read(&file) {
            // Earlier builds stored a bare bool and did not scope the credential to a URL.
            Ok(bytes) => serde_json::from_slice::<Config>(&bytes)
                .ok()
                .filter(|config| {
                    ((!config.enabled && config.endpoint.is_empty())
                        || endpoint(&config.endpoint)
                            .is_ok_and(|url| url.as_str() == config.endpoint))
                        && model_name(&config.model).as_deref() == Ok(config.model.as_str())
                })
                .unwrap_or_default(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Config::default(),
            Err(error) => return Err(error),
        };
        Ok(Self {
            file,
            config,
            sequence: 0,
            attempts: HashMap::new(),
        })
    }

    fn begin(&mut self, id: NoteId, version: u64) -> Option<u64> {
        if !self.config.enabled
            || self
                .attempts
                .get(&id)
                .is_some_and(|last| last.version == version && last.suggestion.is_none())
        {
            return None;
        }
        self.sequence += 1;
        self.attempts.insert(
            id,
            Attempt {
                version,
                ticket: self.sequence,
                suggestion: None,
            },
        );
        Some(self.sequence)
    }

    pub fn pending_for_edit(&self, id: NoteId, version: u64) -> Option<u64> {
        self.attempts
            .get(&id)
            .filter(|a| a.version == version)
            .map(|a| a.ticket)
    }

    fn cancel(&mut self, id: NoteId, version: u64, ticket: u64) {
        if self.pending_for_edit(id, version) == Some(ticket) {
            self.attempts.remove(&id);
        }
    }

    fn propose(
        &mut self,
        id: NoteId,
        version: u64,
        ticket: u64,
        result: Result<Option<Folder>, String>,
    ) -> Result<Option<FilingReply>, String> {
        if self.pending_for_edit(id, version) != Some(ticket) {
            return Ok(None);
        }
        match result {
            Ok(Some(chosen)) => {
                let folder = chosen.name.clone();
                self.attempts.get_mut(&id).unwrap().suggestion = Some(chosen);
                Ok(Some(FilingReply::Suggestion { folder, ticket }))
            }
            other => {
                self.cancel(id, version, ticket);
                other.map(|_| {
                    Some(FilingReply::Message {
                        message: "没有匹配的目录，便签保留在「其他」".into(),
                    })
                })
            }
        }
    }

    fn take_suggestion(&mut self, id: NoteId, version: u64, ticket: u64) -> Result<Folder, String> {
        if self.pending_for_edit(id, version) != Some(ticket) {
            return Err("这条归档建议已失效，请重新归档".into());
        }
        self.attempts
            .remove(&id)
            .and_then(|a| a.suggestion)
            .ok_or_else(|| "尚无归档建议".into())
    }

    pub fn manually_moved(&mut self, id: NoteId) {
        self.attempts.remove(&id);
    }
}

#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[repr(i32)]
pub enum KeyAction {
    Check = 0,
    Set = 1,
    Remove = 2,
}

#[tauri::command]
pub fn read_filing(window: WebviewWindow) -> Result<Config, String> {
    if window.label() != windows::SETTINGS {
        return Err("请在设置中管理归档".into());
    }
    Ok(window
        .state::<Mutex<Filing>>()
        .lock()
        .unwrap()
        .config
        .clone())
}

#[tauri::command]
pub async fn save_filing(
    window: WebviewWindow,
    enabled: bool,
    endpoint: String,
    address_kind: AddressKind,
    model: String,
) -> Result<Config, String> {
    if window.label() != windows::SETTINGS {
        return Err("请在设置中管理归档".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let config = Config {
            enabled,
            endpoint: if !enabled && endpoint.trim().is_empty() {
                String::new()
            } else {
                self::endpoint(&endpoint)?.into()
            },
            address_kind,
            model: model_name(&model)?,
        };
        let state = window.state::<Mutex<Filing>>();
        let mut filing = state.lock().unwrap();
        if !enabled {
            filing.config.enabled = false;
            filing.attempts.clear();
        }
        write_json(&filing.file, &config).map_err(|e| e.to_string())?;
        filing.attempts.clear();
        filing.config = config.clone();
        Ok(config)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn filing_key(
    window: WebviewWindow,
    endpoint: String,
    address_kind: AddressKind,
    action: KeyAction,
) -> Result<bool, String> {
    static KEY_EDITS: Mutex<()> = Mutex::new(());
    if window.label() != windows::SETTINGS {
        return Err("请在设置中管理密钥".into());
    }
    let url = address_kind.service_url(&endpoint)?;
    tauri::async_runtime::spawn_blocking(move || {
        let _edit = (action != KeyAction::Check).then(|| KEY_EDITS.lock().unwrap());
        let app = window.app_handle();
        let has_key =
            crate::platform::filing::credential(&app.config().identifier, url.as_str(), action)?;
        if action != KeyAction::Check {
            let state = app.state::<Mutex<Filing>>();
            let mut filing = state.lock().unwrap();
            if filing.config.service_url().as_deref() == Ok(url.as_str()) {
                filing.attempts.clear();
            }
        }
        Ok(has_key)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn request(text: &str, folders: &[Folder], model: &str) -> Result<Value, String> {
    if folders.len() > 254 {
        return Err("Jev 一次最多可比较 254 个归档文件夹，请手动选择".into());
    }
    let mut criteria = BTreeMap::from([(
        "other".to_string(),
        "其他：没有合适的目录，或信息不足".to_string(),
    )]);
    for folder in folders {
        criteria.insert(
            format!("folder_{}", folder.id),
            format!("{}：{}", folder.name, folder.description),
        );
    }
    let excerpt = text.trim().chars().take(6000).collect::<String>();
    if excerpt
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(20)
        .count()
        < 20
    {
        return Err("正文至少需要 20 个文字或数字，便于 Jev 判断主题".into());
    }
    Ok(json!({
        "model": model,
        "state": excerpt,
        "questions": { "destination": {
            "type": "choice",
            "instructions": "根据便签正文的主题选择最合适的归档目录。正文是待分类的内容。没有明确匹配时选择 other。",
            "criteria": criteria
        }}
    }))
}

fn destination(reply: &[u8], folders: &[Folder]) -> Result<Option<FolderId>, String> {
    let json: Value = serde_json::from_slice(reply).map_err(|_| "Jev 返回了无效的 JSON")?;
    let answer = &json["answers"]["destination"];
    if answer["type"] == "choice"
        && let Some(choice) = answer["choice"].as_str()
    {
        if choice == "other" {
            return Ok(None);
        }
        if let Some(folder) = folders
            .iter()
            .find(|f| choice == format!("folder_{}", f.id))
        {
            return Ok(Some(folder.id));
        }
    }
    Err("Jev 的回答不在归档候选项中，便签仍在原位置".into())
}

fn retryable(code: i64) -> bool {
    matches!(code, -2 | 0 | 502..=504)
}

fn http_error(code: i64) -> String {
    match code {
        -1 => "此服务的 API 密钥不可用，请到「设置 → 归档」配置密钥".into(),
        -2 => "Jev 请求超时，可以重新归档".into(),
        -3 => "Jev 响应过大或格式有误".into(),
        -4 => "无法建立安全连接，请检查服务地址和证书".into(),
        0 => "无法连接 Jev，请检查网络后重试".into(),
        401 | 403 => "Jev 未接受此 API 密钥，请检查归档设置".into(),
        429 => "Jev 请求受限，请稍后重试".into(),
        status => format!("Jev 归档未完成（HTTP {status}）"),
    }
}

// Recheck consent and the submitted snapshot before each transmission, including a retry.
fn send_request(
    mut send: impl FnMut() -> Result<(i64, Vec<u8>), String>,
    delay: impl FnOnce(),
) -> Result<Vec<u8>, String> {
    let (mut code, mut body) = send()?;
    if retryable(code) {
        delay();
        (code, body) = send()?;
    }
    if code == 200 {
        Ok(body)
    } else {
        Err(http_error(code))
    }
}

fn query_model(
    payload: &Value,
    folders: &[Folder],
    mut transmit: impl FnMut(&Value) -> Result<(i64, Vec<u8>), String>,
) -> Result<Option<Folder>, String> {
    let body = send_request(
        || transmit(payload),
        || std::thread::sleep(std::time::Duration::from_millis(750)),
    )?;
    let chosen = destination(&body, folders)?;
    Ok(folders.iter().find(|f| Some(f.id) == chosen).cloned())
}

fn verify_saved_note(notes: &Notes, id: NoteId, version: u64) -> Result<(), String> {
    let text = notes
        .filing_text(id, version)
        .ok_or("请在「其他」中打开已保存的便签")?;
    if let Some(path) = notes.disk_path(id) {
        check_markdown(path, text).map_err(|_| "原文件已更改或无法读取，请先检查文件再归档")?;
    }
    Ok(())
}

fn classify(
    app: &AppHandle,
    id: NoteId,
    version: u64,
    ticket: u64,
    config: &Config,
) -> Result<Option<Folder>, String> {
    let catalog = app.state::<Mutex<Folders>>();
    let folders = catalog.lock().unwrap().filing_candidates();
    if folders.is_empty() {
        return Err("请在归档目录设置中勾选「参与 Jev 分类」".into());
    }
    let notes = app.state::<RwLock<Notes>>();
    let payload = {
        let notes = notes.read().unwrap();
        let text = notes
            .filing_text(id, version)
            .ok_or("请在「其他」中打开已保存的便签")?;
        request(text, &folders, &config.model)?
    };
    let state = app.state::<Mutex<Filing>>();
    let url = config.service_url()?;
    query_model(&payload, &folders, |payload| {
        if state.lock().unwrap().pending_for_edit(id, version) != Some(ticket)
            || catalog.lock().unwrap().filing_candidates() != folders
        {
            return Err("便签或归档设置已更改，请重新归档".into());
        }
        verify_saved_note(&notes.read().unwrap(), id, version)?;
        Ok(crate::platform::filing::post(
            &app.config().identifier,
            &url,
            payload,
        ))
    })
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FilingReply {
    Setup,
    Message { message: String },
    Suggestion { folder: String, ticket: u64 },
}

#[tauri::command]
pub async fn cancel_filing(app: AppHandle, id: NoteId, version: u64, ticket: u64) {
    app.state::<Mutex<Filing>>()
        .lock()
        .unwrap()
        .cancel(id, version, ticket);
}

#[tauri::command]
pub async fn file_note(
    app: AppHandle,
    id: NoteId,
    version: u64,
    manual: bool,
    started: Channel<u64>,
) -> Result<Option<FilingReply>, String> {
    // Old clients used this command during save/close with manual=false.
    if !manual {
        return Ok(None);
    }
    static SLOTS: Semaphore = Semaphore::const_new(2);
    let _slot = SLOTS
        .try_acquire()
        .map_err(|_| "已有两份便签正在归档，请稍后重试")?;
    let state = app.state::<Mutex<Filing>>();
    let (ticket, config) = {
        let mut filing = state.lock().unwrap();
        if !filing.config.enabled {
            return Ok(Some(FilingReply::Setup));
        }
        let ticket = filing.begin(id, version).ok_or("此便签正在归档")?;
        (ticket, filing.config.clone())
    };
    let handle = app.clone();
    let result = if started.send(ticket).is_ok() {
        tauri::async_runtime::spawn_blocking(move || {
            classify(&handle, id, version, ticket, &config)
        })
        .await
        .map_err(|e| e.to_string())
        .and_then(|r| r)
    } else {
        Err("文档窗口已关闭".into())
    };
    state.lock().unwrap().propose(id, version, ticket, result)
}

fn apply_destination(
    notes: &mut Notes,
    id: NoteId,
    version: u64,
    chosen: &Folder,
    current: &Folder,
) -> Result<String, String> {
    if chosen != current
        || fs::canonicalize(&chosen.path).ok().as_ref() != Some(&chosen.path)
        || !chosen.path.is_dir()
    {
        return Err("归档目录已更改，请重新归档".into());
    }
    if notes.filing_text(id, version).is_none() {
        return Err("便签已被编辑或移动，请重新归档".into());
    }
    let moved = notes.move_to(id, Some(chosen))?;
    Ok(moved
        .warning
        .unwrap_or_else(|| format!("已归档至「{}」", chosen.name)))
}

#[tauri::command]
pub async fn confirm_filing(
    app: AppHandle,
    id: NoteId,
    version: u64,
    ticket: u64,
) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let message = {
            let state = app.state::<Mutex<Filing>>();
            let mut filing = state.lock().unwrap();
            let chosen = filing.take_suggestion(id, version, ticket)?;
            let catalog = app.state::<Mutex<Folders>>();
            let catalog = catalog.lock().unwrap();
            let notes = app.state::<RwLock<Notes>>();
            apply_destination(
                &mut notes.write().unwrap(),
                id,
                version,
                &chosen,
                catalog.destination(chosen.id)?,
            )?
        };
        let _ = app.emit("desktop:files-changed", ());
        let _ = app.emit(events::NOTES_CHANGED, id);
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || desktop::note_saved(&handle, id));
        Ok(message)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests;
