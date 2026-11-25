#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
use crate::platform::{HostBackend, Native};

mod desktop;
mod events;
mod host;
mod i18n;
mod library;
mod menus;
mod platform;
mod settings;
mod storage;
mod terminal;
mod windows;

use std::sync::{Mutex, RwLock};

use library::{
    folders::{self, Folder, FolderId, FolderInput, Folders},
    notes::{Card, HostId, MoveResult, Note, NoteId, Notes, Page, Scope, SearchPage, SourceGroup},
    workspace,
};
use serde::Serialize;
use settings::{
    Preferences, Settings,
    appearance::{
        Appearance, AppearanceView, Preferences as AppearancePreferences, Stylesheet, ThemeMode,
        WorkspaceLayout,
    },
    filing::{self, Filing},
    shortcuts::Action,
};
use tauri::{
    AppHandle, Emitter, Manager, RunEvent, WebviewWindow, WindowEvent,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

#[tauri::command]
fn quick_view(window: WebviewWindow) -> events::QuickView {
    desktop::quick_view(window.app_handle(), window.label())
}

#[tauri::command]
fn quick_presented(window: WebviewWindow, epoch: u64) -> Result<(), String> {
    desktop::quick_presented(&window, epoch)
}

#[tauri::command]
fn initial_note(app: AppHandle) -> Option<NoteId> {
    desktop::main_note(&app)
}

#[tauri::command]
async fn show_main(app: AppHandle, id: Option<NoteId>) -> Result<(), String> {
    windows::on_main_thread(move || desktop::open_main(&app, id)).await
}

#[tauri::command]
async fn open_document_window(app: AppHandle, id: Option<NoteId>) -> Result<(), String> {
    if desktop::quitting(&app) {
        return Ok(());
    }
    match id {
        Some(id) => show_document(&app, id),
        None => {
            let id = library::file_actions::create_archive_note(
                app.clone(),
                None,
                i18n::t("未命名").into(),
            )
            .await?;
            show_document_with_name(&app, id, true)
        }
    }
}

fn show_document(app: &AppHandle, id: NoteId) -> Result<(), String> {
    show_document_with_name(app, id, false)
}

fn show_document_with_name(app: &AppHandle, id: NoteId, name_on_open: bool) -> Result<(), String> {
    let card = read_card(app.clone(), id)?;
    let label = format!("document-{id}");
    desktop::retain_editor(app, &label, id);
    if let Err(error) = windows::document(app, id, &card.title, name_on_open) {
        if app.get_webview_window(&label).is_none() {
            desktop::release_editor(app, &label, id);
        }
        return Err(error.to_string());
    }
    Ok(())
}

#[tauri::command]
fn return_to_host(window: WebviewWindow, id: Option<NoteId>) -> Result<(), String> {
    if windows::is_quick(window.label())
        && let Some(id) = id
    {
        let owner = window.state::<RwLock<Notes>>().read().unwrap().host(id);
        if let Some(owner) = owner {
            Native::raise(owner).ok_or("宿主窗口已关闭")?;
        }
    }
    Ok(())
}

#[tauri::command]
async fn tile_document(window: WebviewWindow, id: NoteId) -> Result<(), String> {
    windows::on_main_thread(move || tile_from(&window, id)).await
}

fn tile_from(window: &WebviewWindow, id: NoteId) -> Result<(), String> {
    let app = window.app_handle();
    if window.label() != windows::MAIN && windows::document_id(window.label()) != Some(id) {
        return Err("请从文档窗口并排编辑".into());
    }
    let id = if window.label() == windows::MAIN {
        desktop::recent_document(app).unwrap_or(id)
    } else {
        id
    };
    let title = app
        .state::<RwLock<Notes>>()
        .read()
        .unwrap()
        .card(id)
        .ok_or("文档已关闭")?
        .title;
    if app.get_webview_window(windows::MAIN).is_none() {
        desktop::open_main(app, Some(id))?;
    }
    windows::document(app, id, &title, false).map_err(|e| e.to_string())?;
    let main = app
        .get_webview_window(windows::MAIN)
        .ok_or("主窗口已关闭")?;
    let document = app
        .get_webview_window(&format!("document-{id}"))
        .ok_or("文档窗口已关闭")?;
    windows::tile(window, &main, &document).map_err(|e| e.to_string())
}

#[tauri::command]
async fn show_settings(
    app: AppHandle,
    section: Option<windows::SettingsSection>,
) -> Result<(), String> {
    open_settings(&app, section)
}

fn open_settings(app: &AppHandle, section: Option<windows::SettingsSection>) -> Result<(), String> {
    if desktop::quitting(app) {
        return Ok(());
    }
    windows::settings(app, section).map_err(|error| error.to_string())
}

#[tauri::command]
async fn shortcut(app: AppHandle, action: Action) -> Result<(), String> {
    windows::on_main_thread(move || desktop::perform(&app, action)).await
}

#[tauri::command]
async fn show_find(window: WebviewWindow, tiled: bool) -> Result<(), String> {
    windows::on_main_thread(move || desktop::find_from(&window, tiled)).await
}

#[tauri::command]
async fn new_quick(window: WebviewWindow, epoch: u64) -> Result<(), String> {
    desktop::create_quick(window.app_handle().clone(), window.label().into(), epoch).await
}

#[tauri::command]
fn dismiss_quick(window: WebviewWindow) -> Result<(), String> {
    desktop::dismiss_window(&window)
}

#[tauri::command]
fn close_quick(window: WebviewWindow, epoch: u64) -> Result<(), String> {
    if !windows::is_quick(window.label()) {
        return Err("只有速记窗口可以确认速记关闭".into());
    }
    desktop::close_quick(&window, epoch)
}

#[tauri::command]
fn drag_window(window: WebviewWindow) -> Result<(), String> {
    match window.label() {
        label if windows::is_quick(label) => desktop::drag_quick(&window),
        windows::MAIN => window.start_dragging().map_err(|error| error.to_string()),
        _ => Err("此窗口使用系统标题栏".into()),
    }
}

#[tauri::command]
async fn locate(window: WebviewWindow, id: NoteId) -> Result<(), String> {
    windows::on_main_thread(move || desktop::locate_from(&window, id)).await
}

#[tauri::command]
fn quick_inactive(window: WebviewWindow, epoch: u64, bookmark: Option<events::QuickBookmark>) {
    desktop::suspend(&window, epoch, bookmark);
}

#[tauri::command]
async fn list_notes(
    app: AppHandle,
    query: String,
    offset: usize,
    scope: Option<Scope>,
) -> Result<Page, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<RwLock<Notes>>()
            .read()
            .unwrap()
            .list(&query, offset, scope.as_ref())
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn search_notes(
    app: AppHandle,
    query: String,
    host: Option<HostId>,
    page: usize,
) -> Result<SearchPage, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<RwLock<Notes>>()
            .read()
            .unwrap()
            .search(&query, host, page)
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn list_sources(app: AppHandle) -> Result<Vec<SourceGroup>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<RwLock<Notes>>().read().unwrap().sources()
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
async fn source_choices(window: WebviewWindow) -> Result<Option<Vec<host::SourceChoice>>, String> {
    Native::sources(window.label()).await
}

#[tauri::command]
fn clear_source_choices(window: WebviewWindow) {
    Native::clear_sources(window.label());
}

#[tauri::command]
fn register_source(app: AppHandle, choice: u64) -> Result<String, String> {
    let target = Native::register_source(choice)?;
    let state = app.state::<RwLock<Notes>>();
    let mut notes = state.write().unwrap();
    let result = notes.register_source(target.id, target.source());
    if result.is_err() && notes.latest(target.id).is_none() {
        Native::unwatch(target.id);
    }
    drop(notes);
    if result.is_ok() {
        desktop::remember_host(&app, target.id);
    }
    result
}

#[tauri::command]
async fn attach_note(
    window: WebviewWindow,
    id: NoteId,
    choice: Option<u64>,
    source: Option<String>,
) -> Result<String, String> {
    windows::on_main_thread(move || desktop::attach_note(&window, id, choice, source.as_deref()))
        .await
}

#[tauri::command]
async fn create_from_text(
    app: AppHandle,
    text: String,
    choice: Option<u64>,
    source: Option<String>,
) -> Result<NoteId, String> {
    windows::on_main_thread(move || {
        desktop::create_from_text(&app, text, choice, source.as_deref())
    })
    .await
}

#[tauri::command]
async fn float_note(app: AppHandle, id: NoteId, standalone: bool) -> Result<(), String> {
    windows::on_main_thread(move || desktop::float_note(&app, id, standalone)).await
}

#[tauri::command]
async fn recall_note(app: AppHandle, id: NoteId) -> Result<(), String> {
    windows::on_main_thread(move || desktop::recall_note(&app, id)).await
}

#[tauri::command]
fn observe_document_drag(window: WebviewWindow, started: bool) {
    if windows::is_quick(window.label()) && !started {
        desktop::end_quick_drag(&window);
    } else {
        desktop::observe_document_drag(&window, started);
    }
}

#[tauri::command]
fn attach_on_activation(window: WebviewWindow, id: NoteId) -> Result<(), String> {
    desktop::attach_on_activation(&window, id)
}

#[tauri::command]
fn cancel_attachment(window: WebviewWindow) {
    desktop::cancel_attachment(&window);
}

#[tauri::command]
fn close_source(app: AppHandle, key: String) -> Result<(), String> {
    desktop::close_source(&app, &key)
}

#[tauri::command]
async fn locate_source(app: AppHandle, key: String) -> Result<(), String> {
    windows::on_main_thread(move || desktop::locate_source(&app, &key)).await
}

#[tauri::command]
fn detach_note(app: AppHandle, id: NoteId) -> Result<(), String> {
    desktop::detach_note(&app, id)
}

#[tauri::command]
fn pin_quick(window: WebviewWindow, id: NoteId, pinned: bool) -> Result<(), String> {
    if !windows::is_quick(window.label()) {
        return Err("请从便签切换置顶".into());
    }
    desktop::set_pinned(&window, id, pinned)
}

#[tauri::command]
fn source_at_drop(window: WebviewWindow) -> Result<host::DropTarget, String> {
    if window.label() != windows::MAIN && !windows::is_quick(window.label()) {
        return Err("请从文件树或便签浮窗拖出文档".into());
    }
    Native::source_at_drop()
}

#[tauri::command]
fn inbox_directory(app: AppHandle) -> std::path::PathBuf {
    app.state::<RwLock<Notes>>()
        .read()
        .unwrap()
        .inbox_directory()
}

#[tauri::command]
fn list_folders(app: AppHandle) -> Vec<Folder> {
    app.state::<Mutex<Folders>>().lock().unwrap().list()
}

#[tauri::command]
async fn choose_folder() -> Result<Option<String>, String> {
    folders::choose().await
}

#[tauri::command]
async fn save_folder(app: AppHandle, input: FolderInput) -> Result<Folder, String> {
    let handle = app.clone();
    let saved = tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<Mutex<Folders>>();
        let mut folders = state.lock().unwrap();
        let has_documents = input.id.is_some_and(|id| {
            handle
                .state::<RwLock<Notes>>()
                .read()
                .unwrap()
                .has_folder(id)
        });
        folders.save(input, has_documents)
    })
    .await
    .map_err(|error| error.to_string())??;
    let _ = app.emit(events::FOLDERS_CHANGED, ());
    Ok(saved)
}

#[tauri::command]
async fn move_note(
    app: AppHandle,
    id: NoteId,
    folder: Option<FolderId>,
) -> Result<MoveResult, String> {
    let handle = app.clone();
    let moved = tauri::async_runtime::spawn_blocking(move || {
        let filing_state = handle.state::<Mutex<Filing>>();
        let mut filing = filing_state.lock().unwrap();
        let state = handle.state::<Mutex<Folders>>();
        let folders = state.lock().unwrap();
        let target = folder.map(|id| folders.destination(id)).transpose()?;
        let notes_state = handle.state::<RwLock<Notes>>();
        let mut notes = notes_state.write().unwrap();
        let moved = notes.move_to(id, target)?;
        filing.manually_moved(id);
        Ok::<_, String>(moved)
    })
    .await
    .map_err(|error| error.to_string())??;
    let _ = app.emit("desktop:files-changed", ());
    let _ = app.emit(events::NOTES_CHANGED, id);
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || desktop::note_saved(&handle, id));
    Ok(moved)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OpenNote {
    #[serde(flatten)]
    note: Note,
    #[serde(skip_serializing_if = "Option::is_none")]
    filing_ticket: Option<u64>,
    file_unavailable: bool,
}

fn read_editor_revision<T>(
    app: &AppHandle,
    id: NoteId,
    select: impl FnOnce(&Note) -> T,
) -> Result<T, String> {
    let (output, changed) = {
        let state = app.state::<RwLock<Notes>>();
        let mut notes = state.write().unwrap();
        let (note, changed) = notes.refresh_from_disk(id)?;
        (select(note), changed)
    };
    if changed {
        let _ = app.emit(events::NOTES_CHANGED, id);
    }
    Ok(output)
}

#[tauri::command]
async fn read_note(app: AppHandle, id: NoteId) -> Result<OpenNote, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let note = read_editor_revision(&app, id, Note::clone)?;
        let filing_ticket = app
            .state::<Mutex<Filing>>()
            .lock()
            .unwrap()
            .pending_for_edit(id, note.version);
        Ok(OpenNote {
            note,
            filing_ticket,
            file_unavailable: false,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
async fn open_editor_note(window: WebviewWindow, id: NoteId) -> Result<OpenNote, String> {
    desktop::retain_editor(window.app_handle(), window.label(), id);
    let app = window.app_handle().clone();
    match read_note(app.clone(), id).await {
        Ok(open) => Ok(open),
        Err(error) => tauri::async_runtime::spawn_blocking(move || {
            let notes = app.state::<RwLock<Notes>>();
            let notes = notes.read().unwrap();
            let note = notes.missing_file_snapshot(id).ok_or(error)?;
            Ok(OpenNote {
                note,
                filing_ticket: None,
                file_unavailable: true,
            })
        })
        .await
        .map_err(|error| error.to_string())?,
    }
}

#[tauri::command]
async fn refresh_editor_note(
    app: AppHandle,
    id: NoteId,
    version: u64,
) -> Result<Option<Note>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        read_editor_revision(&app, id, |note| {
            (note.version > version).then(|| note.clone())
        })
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn read_card(app: AppHandle, id: NoteId) -> Result<Card, String> {
    app.state::<RwLock<Notes>>()
        .read()
        .unwrap()
        .card(id)
        .ok_or_else(|| "找不到便签".into())
}

#[tauri::command]
fn new_note(app: AppHandle) -> NoteId {
    app.state::<RwLock<Notes>>()
        .write()
        .unwrap()
        .new_note(None, "Jotiva".into())
}

#[tauri::command]
fn release_note(window: WebviewWindow, id: NoteId) {
    desktop::release_editor(window.app_handle(), window.label(), id);
}

#[tauri::command]
async fn save_note(
    app: AppHandle,
    id: NoteId,
    version: u64,
    markdown: String,
) -> Result<u64, String> {
    let handle = app.clone();
    let (saved, path_changed) = tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<RwLock<Notes>>();
        let mut notes = state.write().unwrap();
        let path = notes.disk_path(id).map(std::path::Path::to_path_buf);
        let saved = notes.save(id, version, markdown)?;
        Ok::<_, String>((saved, notes.disk_path(id) != path.as_deref()))
    })
    .await
    .map_err(|err| err.to_string())??;
    if saved != version {
        // The first save can add a source to the tree. Later edits only change a document.
        if path_changed {
            let _ = app.emit("desktop:files-changed", ());
        }
        let _ = app.emit(events::NOTES_CHANGED, (version != 0).then_some(id));
        let handle = app.clone();
        let _ = app.run_on_main_thread(move || desktop::note_saved(&handle, id));
    }
    Ok(saved)
}

#[tauri::command]
fn close_editor_window(window: WebviewWindow) -> Result<(), String> {
    if window.label() != windows::MAIN && windows::document_id(window.label()).is_none() {
        return Err("此窗口有独立的关闭流程".into());
    }
    window.destroy().map_err(|err| err.to_string())
}

#[tauri::command]
fn document_menu(
    window: WebviewWindow,
    storage: menus::DocumentStorage,
    mode: String,
    exists: bool,
    editable: bool,
    attached: bool,
) -> Result<(), String> {
    menus::document_changed(
        window.app_handle(),
        window.label(),
        menus::Document {
            storage,
            mode,
            exists,
            editable,
            attached,
        },
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn quit(app: AppHandle) -> Result<(), String> {
    desktop::request_quit(&app)
}

#[tauri::command]
fn cancel_quit(app: AppHandle) {
    desktop::cancel_quit(&app);
}

#[tauri::command]
fn read_appearance(app: AppHandle) -> AppearanceView {
    app.state::<Mutex<Appearance>>().lock().unwrap().view()
}

#[tauri::command]
fn save_workspace(window: WebviewWindow, layout: WorkspaceLayout) -> Result<(), String> {
    if window.label() != windows::MAIN {
        return Err("只有主窗口可以保存分栏宽度".into());
    }
    window
        .state::<Mutex<Appearance>>()
        .lock()
        .unwrap()
        .save_workspace(layout)
}

#[tauri::command]
fn save_appearance(app: AppHandle, preferences: AppearancePreferences) -> Result<(), String> {
    let (view, resized, mode_changed) = {
        let state = app.state::<Mutex<Appearance>>();
        let mut appearance = state.lock().unwrap();
        let previous = appearance.scale();
        let mode_changed = appearance.mode() != preferences.mode;
        let view = appearance.save(preferences)?;
        (view, previous != appearance.scale(), mode_changed)
    };
    if mode_changed {
        app.set_theme(view.mode().native());
    }
    if view.language() != i18n::language() {
        i18n::set_language(view.language());
        let keys = app
            .state::<Mutex<Settings>>()
            .lock()
            .unwrap()
            .preferences
            .clone();
        menus::apply_keys(&app, &keys).map_err(|e| e.to_string())?;
        for (item, source) in &app.state::<TrayLabels>().0 {
            item.set_text(i18n::t(source)).map_err(|e| e.to_string())?;
        }
        for (label, window) in app.webview_windows() {
            let title = if label == windows::SETTINGS {
                "Jotiva 设置"
            } else if windows::is_quick(&label) {
                "Jotiva 便签"
            } else {
                continue;
            };
            window
                .set_title(i18n::t(title))
                .map_err(|e| e.to_string())?;
        }
    }
    publish_appearance(&app, view)?;
    if resized {
        desktop::appearance_changed(&app)?;
    }
    Ok(())
}

fn set_theme_mode(app: &AppHandle, mode: ThemeMode) -> Result<(), String> {
    let view = app
        .state::<Mutex<Appearance>>()
        .lock()
        .unwrap()
        .set_mode(mode)?;
    app.set_theme(mode.native());
    publish_appearance(app, view)
}

#[tauri::command]
fn set_stylesheet(app: AppHandle, stylesheet: Option<Stylesheet>) -> Result<(), String> {
    let view = app
        .state::<Mutex<Appearance>>()
        .lock()
        .unwrap()
        .set_stylesheet(stylesheet)?;
    publish_appearance(&app, view)
}

fn publish_appearance(app: &AppHandle, view: AppearanceView) -> Result<(), String> {
    menus::sync_theme(app, &view).map_err(|error| error.to_string())?;
    app.emit(events::APPEARANCE_CHANGED, view)
        .map_err(|error| error.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    preferences: Preferences,
    defaults: Preferences,
    trusted: bool,
    shortcut_error: Option<String>,
    windows: desktop::WindowCounts,
}

#[tauri::command]
fn read_settings(app: AppHandle) -> SettingsView {
    let state = app.state::<Mutex<Settings>>();
    let settings = state.lock().unwrap();
    SettingsView {
        preferences: settings.preferences.clone(),
        defaults: Preferences::default(),
        trusted: Native::permission(false),
        shortcut_error: settings.shortcut_error.clone(),
        windows: desktop::counts(&app),
    }
}

#[tauri::command]
fn save_settings(window: WebviewWindow, preferences: Preferences) -> Result<(), String> {
    if window.label() != windows::SETTINGS {
        return Err("请在设置窗口保存快捷键".into());
    }
    let app = window.app_handle();
    settings::shortcuts::validate_local(&preferences)?;
    let state = app.state::<Mutex<Settings>>();
    let previous = state.lock().unwrap().preferences.clone();
    let local = preferences.local.clone();
    let changed = previous.local != local;
    let menu_result = if changed {
        menus::apply_keys(app, &preferences)
    } else {
        Ok(())
    };
    let result = menu_result
        .map_err(|error| error.to_string())
        .and_then(|_| state.lock().unwrap().update(preferences));
    if let Err(error) = result {
        if changed {
            menus::apply_keys(app, &previous)
                .map_err(|restore| format!("{error}。恢复菜单键位失败：{restore}"))?;
        }
        return Err(error);
    }
    app.emit("desktop:shortcuts-changed", local)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn record_shortcut(window: WebviewWindow, active: bool) -> Result<(), String> {
    if window.label() != windows::SETTINGS {
        return Err("在设置窗口录入快捷键".into());
    }
    settings::shortcuts::record(active.then_some(&window))?;
    window.state::<Mutex<Settings>>().lock().unwrap().recording = active;
    Ok(())
}

#[tauri::command]
fn accessibility_trusted() -> bool {
    Native::permission(false)
}

#[tauri::command]
fn running_app_path() -> Result<std::path::PathBuf, String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    Ok(executable
        .ancestors()
        .find(|p| p.extension().is_some_and(|ext| ext == "app"))
        .unwrap_or(&executable)
        .to_path_buf())
}

#[tauri::command]
fn request_accessibility(window: WebviewWindow) -> Result<bool, String> {
    desktop::hold_for_accessibility(&window, true);
    let result = Native::request_accessibility();
    if !matches!(result, Ok(false)) {
        desktop::hold_for_accessibility(&window, false);
    }
    result
}

#[tauri::command]
fn finish_accessibility_request(window: WebviewWindow) {
    desktop::hold_for_accessibility(&window, false);
}

struct TrayLabels(Vec<(MenuItem<tauri::Wry>, &'static str)>);

fn tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::new(app)?;
    let mut labels = Vec::new();
    for (id, source) in [
        ("open-main", "打开 Jotiva"),
        ("open-quick", "速记"),
        ("new-quick", "新建便签"),
        ("search", "查找便签和文档"),
        ("overview", "平铺便签"),
        ("dock", "靠右收拢 / 还原"),
        ("open-settings", "设置…"),
        ("quit", "退出 Jotiva"),
    ] {
        let item = MenuItem::with_id(app, id, i18n::t(source), true, None::<&str>)?;
        menu.append(&item)?;
        labels.push((item, source));
    }
    app.manage(TrayLabels(labels));
    TrayIconBuilder::new()
        .icon(tauri::include_image!("icons/tray.png"))
        .icon_as_template(true)
        .tooltip("Jotiva")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open-main" => desktop::report(desktop::open_main(app, None)),
            "open-quick" => desktop::report(desktop::perform(app, Action::Quick)),
            "new-quick" => desktop::report(desktop::start_quick(app, true)),
            "search" => desktop::report(desktop::perform(app, Action::Search)),
            "overview" => desktop::report(desktop::show_find(app, true)),
            "dock" => desktop::report(desktop::perform(app, Action::Dock)),
            "open-settings" => desktop::report(open_settings(app, None)),
            "quit" => desktop::report(desktop::request_quit(app)),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

fn main() {
    let mut startup_files = platform::startup_files();
    platform::builder()
        .menu(menus::build)
        .on_menu_event(menus::handle)
        .setup(|app| {
            let data = storage::library_path(&app.path().app_data_dir()?)?;
            app.manage(storage::lock_library(&data)?);
            app.manage(RwLock::new(Notes::load(data.join("notes"))?));
            app.manage(Mutex::new(Settings::load(data.join("settings.json"))?));
            app.manage(Mutex::new(Filing::load(data.join("filing.json"))?));
            app.manage(Mutex::new(Appearance::load(data.join("appearance.json"))?));
            app.manage(Mutex::new(workspace::Workspace::load(
                data.join("workspace.json"),
            )?));
            app.manage(terminal::Terminal::default());
            app.manage(workspace::DroppedTabs::default());
            app.manage(Mutex::new(Folders::load(data.join("folders.json"))?));
            app.manage(Mutex::new(desktop::Desktop::default()));
            app.manage(Mutex::new(menus::Context::default()));
            let appearance = app.state::<Mutex<Appearance>>().lock().unwrap().view();
            i18n::set_language(appearance.language());
            app.set_theme(appearance.mode().native());
            menus::sync_theme(app.handle(), &appearance)?;
            let keys = app
                .state::<Mutex<Settings>>()
                .lock()
                .unwrap()
                .preferences
                .clone();
            if !keys.local.is_empty() || appearance.language() == i18n::Language::English {
                menus::apply_keys(app.handle(), &keys)?;
            }
            desktop::start(app.handle())?;
            app.state::<Mutex<Settings>>().lock().unwrap().install();
            tray(app.handle())?;
            windows::main(app.handle())?;
            // Start one light shell during launch. Its normal idle timeout also
            // applies before the first capture and no editor Worker starts here.
            desktop::report(
                windows::quick(app.handle())
                    .map(|_| ())
                    .map_err(|error| error.to_string()),
            );
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event
                && window.label() != windows::SETTINGS
                && !platform::MACOS
            {
                workspace::open_drop(window.app_handle(), paths.clone());
            }
            if window.label() == windows::SETTINGS
                && matches!(event, WindowEvent::Focused(false) | WindowEvent::Destroyed)
            {
                window.state::<Mutex<Settings>>().lock().unwrap().recording = false;
                let _ = settings::shortcuts::record(None);
            }
            if matches!(event, WindowEvent::Destroyed) {
                if window.label() == windows::MAIN {
                    window.state::<terminal::Terminal>().shutdown();
                }
                desktop::window_destroyed(window.app_handle(), window.label());
                let _ = menus::window_destroyed(window.app_handle(), window.label());
                Native::clear_sources(window.label());
            }
            if matches!(event, WindowEvent::Focused(true))
                && !windows::is_quick(window.label())
                && window.is_focused().unwrap_or(false)
            {
                desktop::own_window_focused(window.app_handle(), window.label());
            }
            if matches!(event, WindowEvent::Focused(true)) {
                let _ = menus::sync(window.app_handle(), Some(window.label()));
            }
        })
        .invoke_handler(tauri::generate_handler![
            #[cfg(windows)]
            platform::system_window_menu,
            terminal::open_terminal,
            terminal::live_terminals,
            terminal::read_terminal,
            terminal::write_terminal,
            terminal::resize_terminal,
            terminal::close_terminal,
            quick_view,
            quick_presented,
            initial_note,
            show_main,
            show_settings,
            shortcut,
            show_find,
            new_quick,
            dismiss_quick,
            close_quick,
            drag_window,
            #[cfg(target_os = "macos")]
            platform::menu::context_menu,
            #[cfg(target_os = "macos")]
            platform::menu::copy_text,
            locate,
            quick_inactive,
            list_notes,
            search_notes,
            list_sources,
            source_choices,
            register_source,
            attach_note,
            source_at_drop,
            observe_document_drag,
            attach_on_activation,
            cancel_attachment,
            close_source,
            locate_source,
            detach_note,
            pin_quick,
            create_from_text,
            float_note,
            recall_note,
            clear_source_choices,
            workspace::workspace_roots,
            workspace::add_workspace_folder,
            workspace::remove_workspace_folder,
            library::file_actions::workspace_action,
            library::file_actions::file_selection_action,
            library::file_actions::note_action,
            library::file_actions::create_archive_note,
            library::file_actions::name_created_document,
            library::file_actions::reveal_file,
            workspace::workspace_children,
            workspace::search_document_files,
            workspace::open_library_file,
            workspace::choose_markdown_file,
            #[cfg(target_os = "macos")]
            platform::file_drop::open_dropped_markdown,
            workspace::take_dropped_tabs,
            workspace::read_tab_titles,
            inbox_directory,
            list_folders,
            folders::archive_entries,
            filing::confirm_filing,
            filing::file_note,
            filing::cancel_filing,
            filing::read_filing,
            filing::filing_key,
            filing::save_filing,
            choose_folder,
            save_folder,
            move_note,
            read_note,
            read_card,
            new_note,
            release_note,
            save_note,
            open_document_window,
            return_to_host,
            tile_document,
            open_editor_note,
            refresh_editor_note,
            close_editor_window,
            document_menu,
            menus::application_menu,
            menus::workspace_tab_menu,
            quit,
            cancel_quit,
            read_appearance,
            save_workspace,
            save_appearance,
            set_stylesheet,
            read_settings,
            save_settings,
            record_shortcut,
            accessibility_trusted,
            running_app_path,
            request_accessibility,
            finish_accessibility_request,
        ])
        .build(tauri::generate_context!())
        .expect("initialize Jotiva")
        .run(move |app, event| match event {
            RunEvent::Exit => app.state::<terminal::Terminal>().shutdown(),
            RunEvent::Ready if !startup_files.is_empty() => {
                workspace::open_paths(app, std::mem::take(&mut startup_files));
            }
            RunEvent::ExitRequested {
                code: None, api, ..
            } => api.prevent_exit(),
            #[cfg(target_os = "macos")]
            RunEvent::Opened { urls } => {
                let paths = urls
                    .into_iter()
                    .filter_map(|url| url.to_file_path().ok())
                    .collect::<Vec<_>>();
                if app.try_state::<RwLock<Notes>>().is_none() {
                    // AppKit may deliver the launch document before Tauri runs setup.
                    startup_files.extend(paths);
                } else if !paths.is_empty() {
                    workspace::open_paths(app, paths);
                }
            }
            #[cfg(target_os = "macos")]
            RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } => {
                desktop::report(desktop::open_main(app, None));
            }
            _ => {}
        });
}
