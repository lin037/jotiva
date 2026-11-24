use crate::{
    desktop,
    i18n::t,
    library::{
        folders::{FolderId, Folders},
        notes::{NoteId, Notes},
        workspace::Workspace,
    },
    windows,
};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::{
    collections::HashMap,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicU32, Ordering},
    },
};
use tauri::{Manager, WebviewWindow, ipc::Response};
use tokio::sync::{Mutex as AsyncMutex, mpsc};

type Master = Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>;
type Output = Arc<AsyncMutex<mpsc::Receiver<Vec<u8>>>>;

struct Session {
    id: u32,
    master: Master,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    output: Output,
    child: Arc<Mutex<Box<dyn Child + Send + Sync>>>,
}

#[derive(Default)]
pub struct Terminal(Mutex<HashMap<u32, Session>>);

impl Terminal {
    pub fn has_sessions(&self) -> bool {
        !self.0.lock().unwrap().is_empty()
    }

    pub fn shutdown(&self) {
        self.0.lock().unwrap().clear();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let mut child = self.child.lock().unwrap();
        if matches!(child.try_wait(), Ok(Some(_))) {
            return;
        }
        #[cfg(unix)]
        if let Some(group) = self
            .master
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|pty| pty.process_group_leader())
            .filter(|id| *id > 1)
        {
            // The foreground job may be in a separate process group from the shell.
            unsafe {
                libc::killpg(group, libc::SIGHUP);
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn shell() -> CommandBuilder {
    #[cfg(windows)]
    {
        let system = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| r"C:\Windows".into());
        let mut cmd =
            CommandBuilder::new(system.join(r"System32\WindowsPowerShell\v1.0\powershell.exe"));
        cmd.arg("-NoLogo");
        cmd
    }
    #[cfg(unix)]
    {
        CommandBuilder::new_default_prog()
    }
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        cols: cols.clamp(2, 1000),
        rows: rows.clamp(1, 500),
        ..Default::default()
    }
}

fn failure(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl Session {
    fn start(directory: &Path, inbox: &Path, size: PtySize) -> Result<Self, String> {
        #[cfg(windows)]
        let (directory, inbox) = (dunce::simplified(directory), dunce::simplified(inbox));
        let mut cmd = shell();
        cmd.cwd(directory);
        cmd.env("TERM", "xterm-256color");
        cmd.env("JOTIVA_INBOX_DIR", inbox);
        let pair = native_pty_system().openpty(size).map_err(failure)?;
        let mut reader = pair.master.try_clone_reader().map_err(failure)?;
        let writer = pair.master.take_writer().map_err(failure)?;
        let child = Arc::new(Mutex::new(pair.slave.spawn_command(cmd).map_err(failure)?));
        drop(pair.slave);
        let (send, recv) = mpsc::channel(8);
        tauri::async_runtime::spawn_blocking(move || {
            let mut bytes = [0; 8192];
            loop {
                match reader.read(&mut bytes) {
                    Ok(0) => break,
                    Ok(n) => {
                        if send.blocking_send(bytes[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        });
        let master = Arc::new(Mutex::new(Some(pair.master)));
        let release = Arc::clone(&master);
        let shell = Arc::clone(&child);
        tauri::async_runtime::spawn_blocking(move || {
            while matches!(shell.lock().unwrap().try_wait(), Ok(None)) {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            // Closing ConPTY after the shell exits lets the reader consume the final bytes and EOF.
            let handle = release.lock().unwrap().take();
            drop(handle);
        });
        static NEXT: AtomicU32 = AtomicU32::new(1);
        Ok(Self {
            id: NEXT.fetch_add(1, Ordering::Relaxed),
            master,
            writer: Arc::new(Mutex::new(writer)),
            output: Arc::new(AsyncMutex::new(recv)),
            child,
        })
    }
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Location {
    Document { id: Option<NoteId> },
    Workspace { path: PathBuf },
    Archive { folder: Option<FolderId> },
}

impl Location {
    fn directory(self, app: &tauri::AppHandle) -> Result<PathBuf, String> {
        let directory = match self {
            Self::Document { id } => {
                let state = app.state::<RwLock<Notes>>();
                let notes = state.read().unwrap();
                id.and_then(|id| notes.disk_path(id))
                    .and_then(Path::parent)
                    .map(Path::to_owned)
                    .unwrap_or_else(|| notes.inbox_directory())
            }
            Self::Archive { folder: Some(id) } => app
                .state::<Mutex<Folders>>()
                .lock()
                .unwrap()
                .get(id)?
                .path
                .clone(),
            Self::Archive { folder: None } => app
                .state::<RwLock<Notes>>()
                .read()
                .unwrap()
                .inbox_directory(),
            Self::Workspace { path } => {
                let item = app
                    .state::<Mutex<Workspace>>()
                    .lock()
                    .unwrap()
                    .resolve(&path)?;
                if item.is_dir() {
                    item
                } else {
                    item.parent().ok_or_else(|| failure(item.display()))?.into()
                }
            }
        };
        crate::storage::check_directory(&directory).map_err(failure)?;
        Ok(directory)
    }
}

#[derive(serde::Serialize)]
pub struct Started {
    id: u32,
    directory: PathBuf,
}

fn workspace_only(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == windows::MAIN {
        Ok(())
    } else {
        Err(t("请从主窗口打开终端").into())
    }
}

fn with_session<T>(
    window: &WebviewWindow,
    id: u32,
    f: impl FnOnce(&Session) -> T,
) -> Result<T, String> {
    workspace_only(window)?;
    let state = window.state::<Terminal>();
    let guard = state.0.lock().unwrap();
    guard.get(&id).map(f).ok_or_else(|| t("终端已结束").into())
}

#[tauri::command]
pub async fn open_terminal(
    window: WebviewWindow,
    location: Location,
    cols: u16,
    rows: u16,
) -> Result<Started, String> {
    workspace_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let directory = location.directory(window.app_handle())?;
        let state = window.state::<Terminal>();
        let mut sessions = state.0.lock().unwrap();
        if desktop::quitting(window.app_handle())
            || window.get_webview_window(windows::MAIN).is_none()
        {
            return Err(t("终端已结束").into());
        }
        let inbox = window
            .state::<RwLock<Notes>>()
            .read()
            .unwrap()
            .inbox_directory();
        let session = Session::start(&directory, &inbox, size(cols, rows))?;
        let id = session.id;
        sessions.insert(id, session);
        Ok(Started { id, directory })
    })
    .await
    .map_err(failure)?
}

#[tauri::command]
pub fn live_terminals(window: WebviewWindow) -> Result<usize, String> {
    workspace_only(&window)?;
    Ok(window
        .state::<Terminal>()
        .0
        .lock()
        .unwrap()
        .values()
        .filter(|s| matches!(s.child.lock().unwrap().try_wait(), Ok(None)))
        .count())
}

#[tauri::command]
pub async fn read_terminal(window: WebviewWindow, id: u32) -> Result<Response, String> {
    let output = with_session(&window, id, |s| s.output.clone())?;
    let mut queue = output.lock().await;
    let mut bytes = queue.recv().await.unwrap_or_default();
    while bytes.len() < 65536 {
        let Ok(next) = queue.try_recv() else { break };
        bytes.extend(next);
    }
    Ok(Response::new(bytes))
}

#[tauri::command]
pub async fn write_terminal(window: WebviewWindow, id: u32, data: String) -> Result<(), String> {
    let writer = with_session(&window, id, |s| s.writer.clone())?;
    tauri::async_runtime::spawn_blocking(move || {
        writer
            .lock()
            .unwrap()
            .write_all(data.as_bytes())
            .map_err(failure)
    })
    .await
    .map_err(failure)?
}

#[tauri::command]
pub fn resize_terminal(window: WebviewWindow, id: u32, cols: u16, rows: u16) -> Result<(), String> {
    with_session(&window, id, |s| {
        if let Some(master) = s.master.lock().unwrap().as_ref() {
            master.resize(size(cols, rows)).map_err(failure)?;
        }
        Ok(())
    })?
}

#[tauri::command]
pub fn close_terminal(window: WebviewWindow, id: u32) -> Result<(), String> {
    workspace_only(&window)?;
    let state = window.state::<Terminal>();
    let removed = state.0.lock().unwrap().remove(&id);
    drop(removed);
    Ok(())
}
