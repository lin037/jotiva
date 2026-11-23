// Explicitly opened notes have stable native window identities. A hidden WebView
// may be reclaimed without losing the note's host, geometry or presentation.
use super::*;

pub(super) struct NoteWindow {
    pub host: Option<HostId>,
    pub expanded: bool,
    pub pinned: bool,
    pub visible: bool,
    pub dragged: bool,
    pub search: Option<bool>,
    pub position: LogicalPosition<f64>,
    pub edit_size: Option<LogicalSize<f64>>,
    pub summary_size: Option<LogicalSize<f64>>,
    pub epoch: u64,
    pub(super) focus: Option<bool>,
    painted: Option<bool>,
    cursor: Option<events::Cursor>,
    idle: Option<JoinHandle<()>>,
}

impl NoteWindow {
    pub fn needs_visibility(&self, active: Option<HostId>, docked: bool) -> bool {
        self.host.is_none() || self.pinned || (self.host == active && !docked)
    }

    pub fn view(&self, id: NoteId, notes: &Notes) -> QuickView {
        let view = if !self.visible {
            View::Hidden
        } else if let Some(tiled) = self.search {
            View::Search {
                tiled,
                host: self.host,
            }
        } else if self.expanded {
            View::Edit {
                id,
                cursor: self.cursor.clone(),
                pinned: self.pinned,
                standalone: self.host.is_none(),
                source: notes.card(id).filter(|c| c.attached).map(|c| c.source),
            }
        } else {
            notes.card(id).map_or(View::Hidden, |note| View::Summary {
                note,
                docked: false,
                rail: None,
            })
        };
        QuickView {
            epoch: self.epoch,
            view,
        }
    }
}

pub(super) fn open(
    app: &AppHandle,
    id: NoteId,
    host: Option<HostId>,
    parent: Option<&WebviewWindow>,
) -> Result<(), String> {
    let state = app.state::<Mutex<Desktop>>();
    if !state.lock().unwrap().note_windows.contains_key(&id) {
        let screen = state
            .lock()
            .unwrap()
            .target
            .filter(|t| Some(t.id) == host)
            .map(|t| t.screen);
        let position = if let Some(parent) = parent {
            let at = host::window_origin(parent).map_err(|e| e.to_string())?;
            let size = parent
                .outer_size()
                .map_err(|e| e.to_string())?
                .to_logical::<f64>(parent.scale_factor().map_err(|e| e.to_string())?);
            let mut occupied: Vec<_> = state
                .lock()
                .unwrap()
                .note_windows
                .values()
                .filter(|w| w.visible)
                .map(|w| w.position)
                .collect();
            if parent.label() != windows::QUICK
                && let Some(primary) = app.get_webview_window(windows::QUICK)
                && primary.is_visible().map_err(|e| e.to_string())?
            {
                occupied.push(host::window_origin(&primary).map_err(|e| e.to_string())?);
            }
            let scale = app
                .state::<Mutex<crate::settings::appearance::Appearance>>()
                .lock()
                .unwrap()
                .scale()
                .max(1.0);
            if let Some(target) = screen
                && host::screen_at(at) != target
            {
                windows::default_note_position(target, scale)
            } else {
                windows::beside(
                    at,
                    size,
                    LogicalSize::new(384. * scale, 352. * scale),
                    host::screen_at(at),
                    &occupied,
                )
            }
        } else if let Some(screen) = screen {
            let scale = app
                .state::<Mutex<crate::settings::appearance::Appearance>>()
                .lock()
                .unwrap()
                .scale()
                .max(1.0);
            windows::default_note_position(screen, scale)
        } else {
            host::pointer()
        };
        state.lock().unwrap().note_windows.insert(
            id,
            NoteWindow {
                host,
                expanded: true,
                pinned: false,
                visible: false,
                dragged: false,
                search: None,
                position,
                edit_size: None,
                summary_size: None,
                epoch: 0,
                focus: None,
                painted: None,
                cursor: None,
                idle: None,
            },
        );
    }
    if let Some(note) = state.lock().unwrap().note_windows.get_mut(&id) {
        note.expanded = true;
        note.search = None;
    }
    update(app, id, true, true)
}

pub(super) fn update(
    app: &AppHandle,
    id: NoteId,
    visible: bool,
    focus: bool,
) -> Result<(), String> {
    let label = windows::note_label(id);
    let state = app.state::<Mutex<Desktop>>();
    let (view, floating, warm) = {
        let mut desktop = state.lock().unwrap();
        let Some(note) = desktop.note_windows.get_mut(&id) else {
            return Ok(());
        };
        if note.dragged {
            if let Some(w) = app.get_webview_window(&label) {
                note.position = host::window_origin(&w).map_err(|e| e.to_string())?;
            }
            note.dragged = false;
        }
        if let Some(idle) = note.idle.take() {
            idle.abort();
        }
        let warm = visible && note.search.is_none() && note.painted == Some(note.expanded);
        if visible && !warm {
            note.painted = None;
        }
        note.epoch += 1;
        note.visible = visible;
        note.focus = visible.then_some(focus || note.focus == Some(true));
        let notes = app.state::<RwLock<Notes>>();
        (
            note.view(id, &notes.read().unwrap()),
            note.host.is_some() || note.pinned || note.search.is_some(),
            warm,
        )
    };
    if visible {
        let window = windows::quick_window(app, &label, id).map_err(|e| e.to_string())?;
        if window.is_always_on_top().map_err(|e| e.to_string())? != floating {
            window
                .set_always_on_top(floating)
                .map_err(|e| e.to_string())?;
        }
        if warm {
            presented(&window, id, view.epoch)?;
        }
    } else if let Some(window) = app.get_webview_window(&label) {
        window.hide().map_err(|e| e.to_string())?;
    }
    app.emit_to(label.as_str(), events::QUICK_VIEW, view)
        .map_err(|e| e.to_string())
}

pub(super) fn presented(window: &WebviewWindow, id: NoteId, epoch: u64) -> Result<(), String> {
    let state = window.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    let Some(note) = desktop.note_windows.get_mut(&id) else {
        return Ok(());
    };
    if note.epoch != epoch {
        return Ok(());
    }
    let Some(focus) = note.focus.take() else {
        return Ok(());
    };
    let searching = note.search.is_some();
    note.painted = (!searching).then_some(note.expanded);
    let (size, dimensions) = match note.search {
        Some(true) => (QuickSize::Overview, None),
        Some(false) => (QuickSize::Search, None),
        None if note.expanded => (QuickSize::Edit, note.edit_size),
        None => (QuickSize::Summary, note.summary_size),
    };
    let position = (!searching).then_some(note.position);
    let host = note.host;
    let target = desktop.target.filter(|t| host == Some(t.id));
    drop(desktop);
    windows::present(window, size, target, false, position, dimensions, focus)
        .map_err(|e| e.to_string())
}

pub(super) fn follow(app: &AppHandle, active: Option<HostId>, docked: bool) -> Result<(), String> {
    let state = app.state::<Mutex<Desktop>>();
    let changes: Vec<_> = {
        let desktop = state.lock().unwrap();
        desktop
            .note_windows
            .iter()
            .filter_map(|(&id, w)| {
                let show = w.needs_visibility(active, docked)
                    || desktop.accessibility_panels.contains(&Some(id));
                (show != w.visible).then_some((id, show))
            })
            .collect()
    };
    for (id, show) in changes {
        update(app, id, show, false)?;
    }
    Ok(())
}

pub(super) fn dismiss(app: &AppHandle, id: NoteId) -> Result<(), String> {
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    let Some(note) = desktop.note_windows.get_mut(&id) else {
        return Ok(());
    };
    let editing = note.search.take().is_some() && note.expanded;
    if !editing {
        note.expanded = false;
    }
    let host = note.host;
    drop(desktop);
    update(app, id, true, editing)?;
    if !editing && let Some(host) = host {
        host::raise(host);
    }
    Ok(())
}

pub(super) fn inactive(
    app: &AppHandle,
    id: NoteId,
    epoch: u64,
    bookmark: Option<events::QuickBookmark>,
) {
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    let Some(note) = desktop.note_windows.get_mut(&id) else {
        return;
    };
    if note.epoch != epoch || note.visible || note.idle.is_some() {
        return;
    }
    if let Some(bookmark) = bookmark.filter(|b| b.id == id) {
        note.cursor = Some(bookmark.state);
    }
    let app = app.clone();
    note.idle = Some(tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(15)).await;
        let main = app.clone();
        let _ = app.run_on_main_thread(move || {
            let state = main.state::<Mutex<Desktop>>();
            let mut desktop = state.lock().unwrap();
            let Some(note) = desktop.note_windows.get_mut(&id) else {
                return;
            };
            if note.epoch != epoch || note.visible {
                return;
            }
            note.idle = None;
            note.painted = None;
            drop(desktop);
            if let Some(window) = main.get_webview_window(&windows::note_label(id)) {
                report(windows::destroy_note(&window).map_err(|e| e.to_string()));
            }
        });
    }));
}

pub(super) fn close(window: &WebviewWindow, id: NoteId, epoch: u64) -> Result<(), String> {
    let app = window.app_handle();
    let state = app.state::<Mutex<Desktop>>();
    let desktop = state.lock().unwrap();
    let quitting = desktop.closing_for_quit.is_some();
    let Some(note) = desktop.note_windows.get(&id) else {
        drop(desktop);
        return windows::destroy_note(window).map_err(|e| e.to_string());
    };
    if !quitting && note.epoch != epoch {
        return Ok(());
    }
    if !quitting && note.search.is_some() {
        drop(desktop);
        return dismiss(app, id);
    }
    let host = note.host;
    drop(desktop);
    if !quitting
        && host.is_some()
        && app
            .state::<RwLock<Notes>>()
            .read()
            .unwrap()
            .host(id)
            .is_some()
    {
        super::detach(app, id)?;
    }
    let removed = state.lock().unwrap().note_windows.remove(&id);
    if let Some(idle) = removed.and_then(|note| note.idle) {
        idle.abort();
    }
    windows::destroy_note(window).map_err(|e| e.to_string())?;
    if !quitting && let Some(host) = host {
        host::raise(host);
    }
    Ok(())
}

// Closing a host or removing its association uses the usual save-before-close
// handshake. An already reclaimed window has no live editor to flush.
pub(super) fn retire(app: &AppHandle, ids: &[NoteId]) {
    for &id in ids {
        let state = app.state::<Mutex<Desktop>>();
        let mut desktop = state.lock().unwrap();
        let Some(note) = desktop.note_windows.get_mut(&id) else {
            continue;
        };
        note.host = None;
        note.search = None;
        if let Some(idle) = note.idle.take() {
            idle.abort();
        }
        let window = app.get_webview_window(&windows::note_label(id));
        if window.is_none() {
            desktop.note_windows.remove(&id);
        }
        drop(desktop);
        if let Some(window) = window {
            report(window.close().map_err(|e| e.to_string()));
        }
    }
}
