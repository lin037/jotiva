use crate::i18n::t;
use std::{collections::HashMap, sync::Mutex};

use tauri::{
    AppHandle, Emitter, Manager,
    menu::{
        CheckMenuItem, HELP_SUBMENU_ID, Menu, MenuEvent, MenuItem, PredefinedMenuItem as Native,
        Submenu, WINDOW_SUBMENU_ID,
    },
};

use crate::{
    desktop, events, open_settings, set_stylesheet, set_theme_mode,
    settings::{
        Preferences,
        appearance::{Appearance, ThemeMode},
        shortcuts::Action,
    },
    windows::{self, SettingsSection},
};

#[derive(Clone, Copy, Default, PartialEq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentStorage {
    #[default]
    Draft,
    Archive,
    File,
}

#[derive(Clone, Default, PartialEq)]
pub struct Document {
    pub mode: String,
    pub storage: DocumentStorage,
    pub exists: bool,
    pub editable: bool,
    pub attached: bool,
}

#[derive(Default)]
pub struct Context {
    windows: HashMap<String, Document>,
    multiple_tabs: bool,
}

#[tauri::command]
pub async fn application_menu(window: tauri::WebviewWindow, x: f64, y: f64) -> Result<(), String> {
    // Release the WebView2 IPC callback before the native menu runs its modal loop.
    windows::on_main_thread(move || {
        let menu = window.app_handle().menu().ok_or("应用菜单尚未就绪")?;
        window
            .popup_menu_at(&menu, tauri::LogicalPosition::new(x, y))
            .map_err(|e| e.to_string())
    })
    .await
}

pub fn build(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    menu_with_keys(app, &Preferences::default())
}

fn menu_with_keys(app: &AppHandle, preferences: &Preferences) -> tauri::Result<Menu<tauri::Wry>> {
    let key = |id: &str| {
        match id {
            "document:history.undo" => return Some("Ctrl+Z"),
            "document:history.redo" => return Some("Ctrl+Y"),
            _ => {}
        }
        crate::settings::shortcuts::commands()
            .iter()
            .find(|command| command.menu.as_deref() == Some(id))
            .and_then(|command| command.key(preferences))
    };
    let item = |id: &str, label: &str| {
        let enabled = !id.starts_with("document:")
            && !id.starts_with("format:")
            && !matches!(
                id,
                "jot-move"
                    | "jot-file-note"
                    | "jot-attach"
                    | "jot-locate"
                    | "jot-save"
                    | "jot-remove-css"
                    | "jot-outline"
                    | "jot-previous"
            );
        MenuItem::with_id(app, id, t(label), enabled, key(id))
    };

    let file = Submenu::with_items(
        app,
        t("文件"),
        true,
        &[
            &item("jot-new", "新建")?,
            &item("jot-new-window", "新建文档窗口")?,
            &item("document:open-window", "在新窗口打开当前文档")?,
            &item("jot-open-file", "打开文档…")?,
            &item("jot-add-workspace", "添加文件夹到工作区…")?,
            &item("jot-save", "保存")?,
            &item("jot-move", "移动便签…")?,
            &item("jot-file-note", "用 Jev 归档")?,
        ],
    )?;
    file.append_items(&[&Native::separator(app)?, &item("jot-close", "关闭")?])?;
    let edit = Submenu::with_items(app, t("编辑"), true, &[])?;
    if crate::platform::MACOS {
        edit.append_items(&[
            &Native::undo(app, Some(t("撤销")))?,
            &Native::redo(app, Some(t("重做")))?,
        ])?;
    } else {
        edit.append_items(&[
            &item("document:history.undo", "撤销")?,
            &item("document:history.redo", "重做")?,
        ])?;
    }
    edit.append_items(&[
        &Native::separator(app)?,
        &Native::cut(app, Some(t("剪切")))?,
        &Native::copy(app, Some(t("复制")))?,
        &Native::paste(app, Some(t("粘贴")))?,
        &Native::select_all(app, Some(t("全选")))?,
        &Native::separator(app)?,
        &item("document:search.open", "查找当前文档…")?,
    ])?;
    let paragraph = Submenu::with_items(
        app,
        t("段落"),
        true,
        &[
            &item("format:format.paragraph", "正文")?,
            &item("format:format.heading-1", "一级标题")?,
            &item("format:format.heading-2", "二级标题")?,
            &item("format:format.heading-3", "三级标题")?,
            &item("format:format.heading-4", "四级标题")?,
            &item("format:format.heading-5", "五级标题")?,
            &item("format:format.heading-6", "六级标题")?,
            &Native::separator(app)?,
            &item("format:format.bullet-list", "无序列表")?,
            &item("format:format.ordered-list", "有序列表")?,
            &item("format:format.task-list", "任务列表")?,
            &item("format:task.toggle-checked", "切换任务完成状态")?,
            &item("format:list.renumber", "重排有序列表编号")?,
            &item("format:format.quote", "引用")?,
        ],
    )?;
    let format = Submenu::with_items(
        app,
        t("格式"),
        true,
        &[
            &item("format:format.bold", "粗体")?,
            &item("format:format.italic", "斜体")?,
            &item("format:format.code", "行内代码")?,
            &item("format:format.link", "插入或编辑链接…")?,
        ],
    )?;
    let view = Submenu::with_id_and_items(
        app,
        "jot-view",
        t("显示"),
        true,
        &[
            &item("jot-sidebar", "显示或隐藏侧栏")?,
            &item("jot-outline", "显示或隐藏文档大纲")?,
            &item("jot-find", "查找便签和文档…")?,
            &Native::separator(app)?,
            &item("document:mode.cycle", "切换编辑模式")?,
            &CheckMenuItem::with_id(
                app,
                "document:mode.live-preview",
                t("即时预览"),
                false,
                false,
                key("document:mode.live-preview"),
            )?,
            &CheckMenuItem::with_id(
                app,
                "document:mode.source",
                t("源码模式"),
                false,
                false,
                key("document:mode.source"),
            )?,
            &item("document:command.palette", "编辑器命令…")?,
        ],
    )?;
    let theme = Submenu::with_id_and_items(
        app,
        "jot-theme",
        t("主题"),
        true,
        &[
            &CheckMenuItem::with_id(
                app,
                "jot-system",
                t("跟随系统"),
                true,
                true,
                key("jot-system"),
            )?,
            &CheckMenuItem::with_id(app, "jot-light", t("浅色"), true, false, key("jot-light"))?,
            &CheckMenuItem::with_id(app, "jot-dark", t("深色"), true, false, key("jot-dark"))?,
            &Native::separator(app)?,
            &item("jot-appearance", "外观设置…")?,
            &item("jot-remove-css", "关闭自定义 CSS")?,
        ],
    )?;
    let window = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        t("窗口"),
        true,
        &[
            &Native::minimize(app, Some(t("最小化")))?,
            &Native::maximize(app, Some(t("缩放")))?,
            &Native::separator(app)?,
            &item("jot-main", "返回主窗口")?,
            &item("jot-return-host", "返回宿主")?,
            &item("jot-pin", "切换置顶")?,
            &item("document:tile", "与主窗口并排")?,
            &item("jot-previous", "上一份文档")?,
            &item("jot-quick", "新建便签")?,
            &item("jot-attach", "挂载便签到窗口…")?,
            &item("jot-locate", "定位到宿主窗口")?,
            &item("jot-overview", "平铺便签")?,
            &item("jot-dock", "靠右收拢 / 还原")?,
        ],
    )?;
    let help = Submenu::with_id_and_items(
        app,
        HELP_SUBMENU_ID,
        t("帮助"),
        true,
        &[&item("jot-shortcuts", "快捷键速查")?],
    )?;
    if crate::platform::MACOS {
        let jot = Submenu::with_items(
            app,
            "Jotiva",
            true,
            &[
                &Native::about(app, Some(t("关于 Jotiva")), None)?,
                &item("jot-settings", "设置…")?,
                &Native::separator(app)?,
                &Native::hide(app, Some(t("隐藏 Jotiva")))?,
                &Native::hide_others(app, Some(t("隐藏其他应用")))?,
                &Native::show_all(app, Some(t("全部显示")))?,
                &Native::separator(app)?,
                &item("jot-quit", "退出 Jotiva")?,
            ],
        )?;
        Menu::with_items(
            app,
            &[
                &jot, &file, &edit, &paragraph, &format, &view, &theme, &window, &help,
            ],
        )
    } else {
        file.append(&item("jot-quit", "退出 Jotiva")?)?;
        let tools = Submenu::with_items(app, t("工具"), true, &[&item("jot-settings", "设置…")?])?;
        help.append(&Native::about(app, Some(t("关于 Jotiva")), None)?)?;
        Menu::with_items(
            app,
            &[
                &file, &edit, &paragraph, &format, &view, &theme, &tools, &window, &help,
            ],
        )
    }
}

/// macOS keeps an NSMenuItem's old key equivalent when muda receives None.
/// Install freshly built menu items so clearing and swapping bindings also work.
pub fn apply_keys(app: &AppHandle, preferences: &Preferences) -> tauri::Result<()> {
    app.set_menu(menu_with_keys(app, preferences)?)?;
    for w in app.webview_windows().into_values() {
        crate::platform::prepare_caption(&w)?;
    }
    let (mode, has_stylesheet) = {
        let state = app.state::<Mutex<Appearance>>();
        let appearance = state.lock().unwrap();
        (appearance.mode(), appearance.has_stylesheet())
    };
    sync_theme(app, mode, has_stylesheet)?;
    sync(app, focused(app).as_deref())
}

fn focused(app: &AppHandle) -> Option<String> {
    app.webview_windows()
        .into_iter()
        .find(|(_, window)| window.is_focused().unwrap_or(false))
        .map(|(label, _)| label)
}

pub fn sync_theme(app: &AppHandle, current: ThemeMode, has_stylesheet: bool) -> tauri::Result<()> {
    let Some(menu) = app.menu().and_then(|menu| menu.get("jot-theme")) else {
        return Ok(());
    };
    let Some(theme) = menu.as_submenu() else {
        return Ok(());
    };
    for (id, mode) in [
        ("jot-system", ThemeMode::System),
        ("jot-light", ThemeMode::Light),
        ("jot-dark", ThemeMode::Dark),
    ] {
        if let Some(item) = theme
            .get(id)
            .and_then(|entry| entry.as_check_menuitem().cloned())
        {
            item.set_checked(current == mode)?;
        }
    }
    if let Some(item) = theme
        .get("jot-remove-css")
        .and_then(|entry| entry.as_menuitem().cloned())
    {
        item.set_enabled(has_stylesheet)?;
    }
    Ok(())
}

pub fn document_changed(app: &AppHandle, label: &str, document: Document) -> tauri::Result<()> {
    let state = app.state::<Mutex<Context>>();
    let mut context = state.lock().unwrap();
    if document.exists {
        if context.windows.get(label) == Some(&document) {
            return Ok(());
        }
        context.windows.insert(label.into(), document);
    } else if context.windows.remove(label).is_none() {
        return Ok(());
    }
    drop(context);
    if focused(app).as_deref() == Some(label) {
        sync(app, Some(label))?;
    }
    Ok(())
}

pub fn window_destroyed(app: &AppHandle, label: &str) -> tauri::Result<()> {
    if label == windows::MAIN {
        app.state::<Mutex<Context>>().lock().unwrap().multiple_tabs = false;
    }
    document_changed(app, label, Document::default())?;
    if label == windows::MAIN {
        sync(app, focused(app).as_deref())?;
    }
    Ok(())
}

#[tauri::command]
pub fn workspace_tab_menu(window: tauri::WebviewWindow, multiple: bool) -> Result<(), String> {
    if window.label() != windows::MAIN {
        return Err("只有主窗口使用文档标签".into());
    }
    let app = window.app_handle();
    app.state::<Mutex<Context>>().lock().unwrap().multiple_tabs = multiple;
    if focused(app).as_deref() == Some(windows::MAIN) {
        sync(app, Some(windows::MAIN)).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn close_active(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<Mutex<Context>>();
    let context = state.lock().unwrap();
    let document = context.windows.contains_key(windows::MAIN);
    let multiple = context.multiple_tabs;
    drop(context);
    let close_tab = multiple
        || document && (cfg!(windows) || app.state::<crate::terminal::Terminal>().has_sessions());
    crate::platform::window::close_active(app, close_tab)
}

pub fn sync(app: &AppHandle, label: Option<&str>) -> tauri::Result<()> {
    let state = app.state::<Mutex<Context>>();
    let context = state.lock().unwrap();
    let document = label
        .and_then(|label| context.windows.get(label))
        .cloned()
        .unwrap_or_default();
    let multiple_tabs = label == Some(windows::MAIN) && context.multiple_tabs;
    drop(context);
    let Some(menu) = app.menu() else {
        return Ok(());
    };
    for entry in menu.items()? {
        let Some(submenu) = entry.as_submenu() else {
            continue;
        };
        for entry in submenu.items()? {
            if let Some(item) = entry.as_check_menuitem()
                && let Some(mode) = item.id().as_ref().strip_prefix("document:mode.")
            {
                item.set_enabled(document.exists)?;
                item.set_checked(document.exists && document.mode == mode)?;
                continue;
            }
            let Some(item) = entry.as_menuitem() else {
                continue;
            };
            let id = item.id().as_ref();
            let enabled = match id {
                "jot-new" => {
                    label.is_some_and(|s| s == windows::MAIN || windows::document_id(s).is_some())
                }
                "jot-return-host" => label.is_some_and(windows::is_quick) && document.attached,
                "jot-pin" => {
                    document.exists
                        && label.is_some_and(|s| {
                            windows::is_quick(s) || windows::document_id(s).is_some()
                        })
                }
                "document:tile" => {
                    document.exists
                        && label.is_some_and(|s| {
                            s == windows::MAIN || windows::document_id(s).is_some()
                        })
                }
                "jot-save" => document.exists,
                "document:open-window" => {
                    document.exists && label.is_some_and(|l| windows::document_id(l).is_none())
                }
                "jot-file-note" => document.exists && document.storage == DocumentStorage::Draft,
                "jot-move" => {
                    document.exists
                        && document.storage != DocumentStorage::File
                        && label == Some(windows::MAIN)
                }
                "jot-previous" => multiple_tabs,
                "jot-attach" => {
                    crate::platform::host_windows()
                        && document.exists
                        && label == Some(windows::MAIN)
                }
                "jot-outline" => document.exists && label == Some(windows::MAIN),
                "jot-locate" => document.attached && label == Some(windows::MAIN),
                "jot-sidebar" => label == Some(windows::MAIN),
                _ if id.starts_with("document:") => document.exists,
                _ if id.starts_with("format:") => document.editable,
                _ => continue,
            };
            item.set_enabled(enabled)?;
        }
    }
    Ok(())
}

fn editor_action(app: &AppHandle, action: &str) -> Result<(), String> {
    match focused(app).as_deref() {
        Some(label)
            if windows::is_quick(label)
                || label == windows::MAIN
                || windows::document_id(label).is_some() =>
        {
            app.emit_to(label, events::EDITOR_ACTION, action)
                .map_err(|error| error.to_string())
        }
        _ => match action {
            "find" => desktop::show_find(app, false),
            "overview" => desktop::show_find(app, true),
            _ => Ok(()),
        },
    }
}

pub fn handle(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    if let Some(command) = id
        .strip_prefix("document:")
        .or_else(|| id.strip_prefix("format:"))
    {
        desktop::report(editor_action(app, command));
        if command.starts_with("mode.") {
            let _ = sync(app, focused(app).as_deref());
        }
        return;
    }
    desktop::report(match id {
        "jot-settings" => open_settings(app, None),
        "jot-appearance" => open_settings(app, Some(SettingsSection::Appearance)),
        "jot-shortcuts" => open_settings(app, Some(SettingsSection::Shortcuts)),
        "jot-system" => set_theme_mode(app, ThemeMode::System),
        "jot-light" => set_theme_mode(app, ThemeMode::Light),
        "jot-dark" => set_theme_mode(app, ThemeMode::Dark),
        "jot-remove-css" => set_stylesheet(app.clone(), None),
        "jot-quit" => desktop::request_quit(app),
        "jot-quick" => desktop::perform(app, Action::Quick),
        "jot-open-file" | "jot-add-workspace" => {
            let app = app.clone();
            let folder = id == "jot-add-workspace";
            tauri::async_runtime::spawn(async move {
                if folder {
                    desktop::report(crate::library::workspace::menu_add(&app).await)
                } else {
                    desktop::report(
                        crate::library::folders::choose_markdown()
                            .await
                            .map(|path| {
                                if let Some(p) = path {
                                    crate::library::workspace::open_paths(&app, vec![p.into()]);
                                }
                            }),
                    )
                }
            });
            Ok(())
        }
        "jot-main" if focused(app).as_deref().is_some_and(windows::is_quick) => {
            editor_action(app, "main")
        }
        "jot-main" => desktop::open_main(app, None),
        "jot-pin" => editor_action(app, "pin"),
        "jot-return-host" => editor_action(app, "return-host"),
        "jot-find" => editor_action(app, "find"),
        "jot-close" => close_active(app),
        "jot-previous" => editor_action(app, "previous-document"),
        "jot-overview" => editor_action(app, "overview"),
        "jot-dock" => desktop::perform(app, Action::Dock),
        "jot-sidebar" => app
            .emit_to(windows::MAIN, events::SIDEBAR_TOGGLE, ())
            .map_err(|error| error.to_string()),
        "jot-new" => editor_action(app, "new"),
        "jot-new-window" => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = crate::open_document_window(app.clone(), None).await {
                    let _ =
                        app.run_on_main_thread(move || crate::library::folders::show_error(&error));
                }
            });
            Ok(())
        }
        "jot-save" => editor_action(app, "save"),
        "jot-move" => editor_action(app, "move"),
        "jot-file-note" => editor_action(app, "file"),
        "jot-attach" => editor_action(app, "attach"),
        "jot-outline" => editor_action(app, "outline"),
        "jot-locate" => editor_action(app, "locate"),
        _ => Ok(()),
    });
}
