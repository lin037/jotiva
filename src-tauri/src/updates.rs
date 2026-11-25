use serde::Serialize;
use std::{sync::Mutex, time::Duration};
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::{desktop, windows};

#[derive(Clone, Default, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Status {
    #[default]
    Idle,
    Manual,
    Checking,
    Current,
    Available {
        version: String,
    },
    Downloading,
    Ready,
    Failed {
        message: String,
    },
}

#[derive(Default)]
struct State {
    status: Status,
    available: Option<Update>,
    pending: Option<(Update, Vec<u8>)>,
}

#[derive(Default)]
pub struct Updates {
    state: Mutex<State>,
    operation: tokio::sync::Mutex<()>,
}

pub fn enabled(app: &AppHandle) -> bool {
    #[cfg(target_os = "linux")]
    if app.env().appimage.is_none() {
        return false;
    }
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|config| config.get("pubkey"))
        .and_then(|key| key.as_str())
        .is_some_and(|key| !key.trim().is_empty())
}

fn publish(app: &AppHandle, status: Status) {
    app.state::<Updates>().state.lock().unwrap().status = status.clone();
    let _ = app.emit("desktop:update-status", status);
}

#[tauri::command]
pub fn read_update(app: AppHandle) -> Status {
    if !enabled(&app) {
        return Status::Manual;
    }
    app.state::<Updates>().state.lock().unwrap().status.clone()
}

pub fn start(app: &AppHandle) {
    if !enabled(app) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            check(&app).await;
            tokio::time::sleep(Duration::from_secs(6 * 60 * 60)).await;
        }
    });
}

async fn check(app: &AppHandle) {
    let updates = app.state::<Updates>();
    let Ok(_operation) = updates.operation.try_lock() else {
        return;
    };
    if updates.state.lock().unwrap().pending.is_some() || desktop::quitting(app) {
        return;
    }
    publish(app, Status::Checking);
    let result = match app
        .updater_builder()
        .timeout(Duration::from_secs(20))
        .build()
    {
        Ok(updater) => updater.check().await,
        Err(error) => Err(error),
    };
    match result {
        Ok(mut update) => {
            if let Some(update) = &mut update {
                update.timeout = Some(Duration::from_secs(15 * 60));
            }
            let status = update
                .as_ref()
                .map_or(Status::Current, |u| Status::Available {
                    version: u.version.clone(),
                });
            updates.state.lock().unwrap().available = update;
            publish(app, status);
        }
        Err(error) => publish(
            app,
            Status::Failed {
                message: error.to_string(),
            },
        ),
    }
}

#[tauri::command]
pub async fn check_update(window: WebviewWindow) {
    if window.label() == windows::SETTINGS && enabled(window.app_handle()) {
        check(window.app_handle()).await;
    }
}

#[tauri::command]
pub async fn install_update(window: WebviewWindow) -> Result<(), String> {
    let app = window.app_handle();
    if window.label() != windows::SETTINGS || !enabled(app) {
        return Ok(());
    }
    let updates = app.state::<Updates>();
    let Ok(_operation) = updates.operation.try_lock() else {
        return Ok(());
    };
    if desktop::quitting(app) {
        return Ok(());
    }
    let Some(update) = updates.state.lock().unwrap().available.clone() else {
        return Ok(());
    };
    publish(app, Status::Downloading);
    // download() verifies the signature before any window is asked to close.
    match update.download(|_, _| {}, || {}).await {
        Ok(bytes) => {
            let handle = app.clone();
            windows::on_main_thread(move || {
                if desktop::quitting(&handle) {
                    publish(
                        &handle,
                        Status::Available {
                            version: update.version,
                        },
                    );
                    return Ok(());
                }
                handle.state::<Updates>().state.lock().unwrap().pending = Some((update, bytes));
                publish(&handle, Status::Ready);
                desktop::request_quit(&handle)
            })
            .await?;
        }
        Err(error) => publish(
            app,
            Status::Failed {
                message: error.to_string(),
            },
        ),
    }
    Ok(())
}

pub fn cancel(app: &AppHandle) {
    let updates = app.state::<Updates>();
    let pending = updates.state.lock().unwrap().pending.take();
    if let Some((update, _)) = pending {
        publish(
            app,
            Status::Available {
                version: update.version,
            },
        );
    }
}

pub fn finish_quit(app: &AppHandle) {
    let updates = app.state::<Updates>();
    let pending = updates.state.lock().unwrap().pending.take();
    if let Some((update, bytes)) = pending {
        match update.install(bytes) {
            Ok(()) => app.request_restart(),
            Err(error) => {
                desktop::cancel_quit(app);
                publish(
                    app,
                    Status::Failed {
                        message: error.to_string(),
                    },
                );
                desktop::report(desktop::open_main(app, None));
                desktop::report(
                    windows::settings(app, Some(windows::SettingsSection::Updates))
                        .map_err(|e| e.to_string()),
                );
            }
        }
    } else {
        app.exit(0);
    }
}
