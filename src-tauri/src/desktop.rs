use crate::platform::{HostBackend, Native};
mod note_windows;
use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, OnceLock, RwLock},
    time::{Duration, Instant},
};

use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, WebviewWindow};

use crate::{
    events::{self, QuickView, View},
    host::{self, Target},
    library::notes::{HostId, NoteId, Notes, Scope},
    settings::{
        Settings,
        shortcuts::{self, Action},
    },
    windows::{self, QuickSize},
};

static APP: OnceLock<AppHandle> = OnceLock::new();

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Hidden,
    Summary(NoteId),
    Edit(NoteId),
    Search(bool),
}

#[derive(Clone, Copy)]
enum LayoutOwner {
    Host(HostId),
    Independent,
}

#[derive(Default)]
struct Attachment {
    screen: Option<host::Rect>,
    selected: Option<NoteId>,
    position: Option<LogicalPosition<f64>>,
    expanded: bool,
    edit_size: Option<LogicalSize<f64>>,
    summary_size: Option<LogicalSize<f64>>,
}

fn move_to_screen(point: &mut LogicalPosition<f64>, previous: host::Rect, next: host::Rect) {
    let x = ((point.x - previous.x) / previous.width).clamp(0.0, 1.0);
    let y = ((point.y - previous.y) / previous.height).clamp(0.0, 1.0);
    point.x = next.x + (x * next.width).min(next.width - 1.0);
    point.y = next.y + (y * next.height).min(next.height - 1.0);
}

pub struct Desktop {
    mode: Mode,
    painted: Option<Mode>,
    before_search: Mode,
    search_from: Option<Target>,
    pending_locate: Option<PendingLocate>,
    target: Option<Target>,
    standalone: Option<NoteId>,
    free_layout: Attachment,
    pinned: Option<NoteId>,
    dragging_note: bool,
    // None identifies the shared quick panel. Some(id) identifies a note's own window.
    accessibility_panels: HashSet<Option<NoteId>>,
    attach_next: Option<(NoteId, String)>,
    origin: i32,
    docked: bool,
    attachments: HashMap<HostId, Attachment>,
    note_windows: HashMap<NoteId, note_windows::NoteWindow>,
    // Layout to update on the next presentation after a native title-bar drag.
    dragged_layout: Option<LayoutOwner>,
    epoch: u64,
    // A visible view is ordered on screen only after its matching WebView render.
    pending_focus: Option<bool>,
    idle: Option<JoinHandle<()>>,
    bookmark: Option<events::QuickBookmark>,
    main_note: Option<NoteId>,
    document_order: Vec<NoteId>,
    editing: HashMap<String, NoteId>,
    closing_for_quit: Option<HashSet<String>>,
}

#[derive(Clone, Copy)]
struct PendingLocate {
    note: NoteId,
    host: HostId,
    pid: i32,
    from_pid: i32,
    requested: Instant,
}

impl Default for Desktop {
    fn default() -> Self {
        Self {
            mode: Mode::Hidden,
            painted: None,
            before_search: Mode::Hidden,
            search_from: None,
            pending_locate: None,
            target: None,
            standalone: None,
            free_layout: Attachment::default(),
            pinned: None,
            dragging_note: false,
            accessibility_panels: HashSet::new(),
            attach_next: None,
            origin: 0,
            docked: false,
            attachments: HashMap::new(),
            note_windows: HashMap::new(),
            dragged_layout: None,
            epoch: 0,
            pending_focus: None,
            idle: None,
            bookmark: None,
            main_note: None,
            document_order: Vec::new(),
            editing: HashMap::new(),
            closing_for_quit: None,
        }
    }
}

impl Desktop {
    fn current_note(&self) -> Option<NoteId> {
        match self.mode {
            Mode::Edit(id) | Mode::Summary(id) => Some(id),
            _ => None,
        }
    }

    fn keeps_visible(&self) -> bool {
        self.accessibility_panels.contains(&None)
            || self.standalone.is_some()
            || (self.pinned.is_some() && self.pinned == self.current_note())
    }

    fn floats_above(&self) -> bool {
        matches!(self.mode, Mode::Search(_)) || self.target.is_some() || self.pinned.is_some()
    }

    fn restored_mode(&self, summary: Mode, host: HostId) -> Mode {
        match summary {
            Mode::Summary(id)
                if !self.docked && self.attachments.get(&host).is_some_and(|a| a.expanded) =>
            {
                Mode::Edit(id)
            }
            mode => mode,
        }
    }

    fn summary(&self, app: &AppHandle) -> Mode {
        self.target
            .and_then(|target| {
                let attachment = self.attachments.get(&target.id);
                attachment
                    .and_then(|item| item.selected)
                    .filter(|id| self.docked || !self.note_windows.contains_key(id))
                    .or_else(|| {
                        app.state::<RwLock<Notes>>()
                            .read()
                            .unwrap()
                            .latest_except(target.id, |id| {
                                !self.docked && self.note_windows.contains_key(&id)
                            })
                    })
            })
            .map_or(Mode::Hidden, Mode::Summary)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowCounts {
    pub webviews: usize,
    pub main: bool,
    pub settings: bool,
    pub quick: bool,
}

pub(crate) fn app() -> &'static AppHandle {
    APP.get().expect("desktop initialized")
}

pub fn start(app: &AppHandle) -> Result<(), String> {
    assert!(APP.set(app.clone()).is_ok());
    Native::watch(host_event);
    sync_recovery(app);
    shortcuts::watch(shortcut_pressed)
}

#[cfg(target_os = "macos")]
pub extern "C" fn recorded_shortcut(chord: *const std::ffi::c_char) {
    if let Some(app) = APP.get() {
        // The native monitor calls synchronously with a NUL-terminated UTF-8 chord.
        let chord = unsafe { std::ffi::CStr::from_ptr(chord) }.to_string_lossy();
        report(
            app.emit_to(windows::SETTINGS, events::SHORTCUT_RECORDED, chord.as_ref())
                .map_err(|error| error.to_string()),
        );
    }
}

extern "C" fn shortcut_pressed(id: u32) {
    let app = APP.get().expect("native observers started");
    let (recording, binding) = {
        let state = app.state::<Mutex<Settings>>();
        let settings = state.lock().unwrap();
        (settings.recording, settings.bindings.get(id))
    };
    let Some((action, key)) = binding else { return };
    if recording
        && app
            .get_webview_window(windows::SETTINGS)
            .is_some_and(|window| window.is_focused().unwrap_or(false))
    {
        report(
            app.emit_to(
                windows::SETTINGS,
                events::SHORTCUT_RECORDED,
                key.to_string(),
            )
            .map_err(|error| error.to_string()),
        );
    } else {
        report(perform(app, action));
    }
}

extern "C" fn host_event(closed: u64, space_changed: bool) {
    let app = APP.get().expect("native observers started");
    report(on_host_change(app, closed, space_changed));
}

fn sync_recovery(app: &AppHandle) {
    let pending = app
        .state::<RwLock<Notes>>()
        .read()
        .unwrap()
        .pending_bindings();
    Native::recover(&pending, restored);
}

fn restored(origin: NoteId, host: HostId, identity: host::WindowIdentity) -> bool {
    let app = APP.get().expect("native observers started");
    let state = app.state::<RwLock<Notes>>();
    let mut notes = state.write().unwrap();
    if !notes.restore(origin, host, &identity) {
        return false;
    }
    report(notes.remember(host, identity));
    drop(notes);
    let _ = app.emit_to(windows::MAIN, events::NOTES_CHANGED, ());
    sync_recovery(app);
    report(on_host_change(app, 0, false));
    true
}

pub fn remember_host(app: &AppHandle, host: HostId) {
    let state = app.state::<RwLock<Notes>>();
    let missing = state.read().unwrap().needs_identity(host);
    if missing && let Some(identity) = Native::identity(host) {
        report(state.write().unwrap().remember(host, identity));
        sync_recovery(app);
    }
}

fn captured(host: HostId, identity: host::WindowIdentity) {
    let app = APP.get().expect("native observers started");
    report(
        app.state::<RwLock<Notes>>()
            .write()
            .unwrap()
            .remember(host, identity),
    );
    sync_recovery(app);
}

pub fn report(result: Result<(), String>) {
    if let Err(error) = result {
        eprintln!("Jotiva: {error}");
    }
}

pub fn quick_view(app: &AppHandle, label: &str) -> QuickView {
    let state = app.state::<Mutex<Desktop>>();
    let desktop = state.lock().unwrap();
    if let Some(id) = windows::note_id(label) {
        return desktop.note_windows.get(&id).map_or(
            QuickView {
                epoch: desktop.epoch,
                view: View::Hidden,
            },
            |w| w.view(id, &app.state::<RwLock<Notes>>().read().unwrap()),
        );
    }
    let view = match desktop.mode {
        Mode::Hidden => View::Hidden,
        Mode::Summary(id) => {
            let notes = app.state::<RwLock<Notes>>();
            let notes = notes.read().unwrap();
            notes.card(id).map_or(View::Hidden, |note| View::Summary {
                note,
                docked: desktop.docked,
                rail: desktop
                    .target
                    .filter(|_| desktop.docked)
                    .map(|target| events::Rail {
                        host: target.id,
                        page: notes.list("", 0, Some(&Scope::Host { id: target.id })),
                    }),
            })
        }
        Mode::Edit(id) => View::Edit {
            id,
            cursor: desktop
                .bookmark
                .as_ref()
                .filter(|bookmark| bookmark.id == id)
                .map(|bookmark| bookmark.state.clone()),
            pinned: desktop.pinned == Some(id),
            standalone: desktop.standalone == Some(id) || desktop.target.is_none(),
            source: app
                .state::<RwLock<Notes>>()
                .read()
                .unwrap()
                .card(id)
                .filter(|card| card.attached)
                .map(|card| card.source),
        },
        Mode::Search(tiled) => View::Search {
            tiled,
            host: desktop
                .search_from
                .or(desktop.target)
                .filter(|target| target.id != 0)
                .map(|target| target.id),
        },
    };
    QuickView {
        view,
        epoch: desktop.epoch,
    }
}

fn transition(app: &AppHandle, mode: Mode, focus: bool, refresh: bool) -> Result<(), String> {
    let (changed, replace_note, waiting, floating) = {
        let state = app.state::<Mutex<Desktop>>();
        let mut desktop = state.lock().unwrap();
        if desktop.closing_for_quit.is_some() {
            return Ok(());
        }
        let changed = desktop.mode != mode || refresh;
        let warm = (desktop.mode == Mode::Hidden || desktop.mode == mode)
            && desktop.painted == Some(mode)
            && !desktop.docked;
        if changed && mode != Mode::Hidden && !warm {
            desktop.painted = None;
        }
        desktop.pending_locate = None;
        if matches!(desktop.mode, Mode::Search(_)) && !matches!(mode, Mode::Search(_)) {
            desktop.before_search = Mode::Hidden;
            desktop.search_from = None;
        }
        if mode == Mode::Hidden {
            desktop.standalone = None;
        }
        if let Some(owner) = desktop.dragged_layout.take()
            && let Some(window) = app.get_webview_window(windows::QUICK)
        {
            let origin = host::window_origin(&window).map_err(|error| error.to_string())?;
            match owner {
                LayoutOwner::Independent => desktop.free_layout.position = Some(origin),
                LayoutOwner::Host(id) => {
                    if let Some(layout) = desktop.attachments.get_mut(&id) {
                        layout.position = Some(origin);
                    }
                }
            }
        }
        let previous_note = desktop.current_note();
        let previous_mode = desktop.mode;
        desktop.mode = mode;
        if mode == Mode::Hidden
            || (desktop.current_note().is_some() && desktop.pinned != desktop.current_note())
        {
            desktop.pinned = None;
        }
        if changed {
            desktop.epoch += 1;
        }
        if mode != Mode::Hidden
            && let Some(idle) = desktop.idle.take()
        {
            idle.abort();
        }
        let replace_note = previous_note.is_some()
            && desktop.current_note().is_some()
            && previous_note != desktop.current_note();
        if mode == Mode::Hidden {
            desktop.pending_focus = None;
        } else if changed && !warm {
            desktop.pending_focus =
                Some(focus || (previous_mode == mode && desktop.pending_focus == Some(true)));
        } else if focus && desktop.pending_focus.is_some() {
            desktop.pending_focus = Some(true);
        }
        (
            changed,
            replace_note,
            desktop.pending_focus.is_some(),
            desktop.floats_above(),
        )
    };
    if mode == Mode::Hidden {
        if changed && let Some(window) = app.get_webview_window(windows::QUICK) {
            app.emit_to(
                windows::QUICK,
                events::QUICK_VIEW,
                quick_view(app, windows::QUICK),
            )
            .map_err(|err| err.to_string())?;
            window.hide().map_err(|err| err.to_string())?;
        }
    } else {
        let window = windows::quick(app).map_err(|err| err.to_string())?;
        if replace_note && window.is_visible().map_err(|e| e.to_string())? {
            window.hide().map_err(|e| e.to_string())?;
        }
        if window.is_always_on_top().map_err(|e| e.to_string())? != floating {
            window
                .set_always_on_top(floating)
                .map_err(|e| e.to_string())?;
        }
        if changed {
            app.emit_to(
                windows::QUICK,
                events::QUICK_VIEW,
                quick_view(app, windows::QUICK),
            )
            .map_err(|err| err.to_string())?;
        }
        if !waiting {
            present(app, &window, focus)?;
        }
    }
    Ok(())
}

pub fn quick_presented(window: &WebviewWindow, epoch: u64) -> Result<(), String> {
    if let Some(id) = windows::note_id(window.label()) {
        return note_windows::presented(window, id, epoch);
    }
    if window.label() != windows::QUICK {
        return Err("只有便签窗口可以确认显示".into());
    }
    let app = window.app_handle();
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    if desktop.epoch != epoch || desktop.closing_for_quit.is_some() {
        return Ok(());
    }
    desktop.painted = Some(desktop.mode);
    let Some(focus) = desktop.pending_focus.take() else {
        return Ok(());
    };
    drop(desktop);
    present(app, window, focus)
}

fn present(app: &AppHandle, window: &WebviewWindow, focus: bool) -> Result<(), String> {
    let state = app.state::<Mutex<Desktop>>();
    let desktop = state.lock().unwrap();
    let (mode, target, docked) = (desktop.mode, desktop.target, desktop.docked);
    let layout = if desktop.standalone.is_some() {
        Some(&desktop.free_layout)
    } else {
        target.and_then(|t| desktop.attachments.get(&t.id))
    };
    let offset = layout.and_then(|v| v.position);
    let dimensions = match mode {
        Mode::Edit(_) => layout.and_then(|v| v.edit_size),
        Mode::Summary(_) if !docked => layout.and_then(|v| v.summary_size),
        _ => None,
    };
    drop(desktop);
    let size = match mode {
        Mode::Summary(_) if docked => {
            let count = target.map_or(0, |target| {
                app.state::<RwLock<Notes>>()
                    .read()
                    .unwrap()
                    .saved_for_host(target.id)
            });
            QuickSize::Dock(count)
        }
        Mode::Summary(_) => QuickSize::Summary,
        Mode::Edit(_) => QuickSize::Edit,
        Mode::Search(false) => QuickSize::Search,
        Mode::Search(true) => QuickSize::Overview,
        Mode::Hidden => return Ok(()),
    };
    windows::present(window, size, target, docked, offset, dimensions, focus)
        .map_err(|err| err.to_string())
}

pub fn suspend(window: &WebviewWindow, epoch: u64, bookmark: Option<events::QuickBookmark>) {
    let app = window.app_handle();
    if let Some(id) = windows::note_id(window.label()) {
        return note_windows::inactive(app, id, epoch, bookmark);
    }
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    if desktop.mode != Mode::Hidden || desktop.epoch != epoch || desktop.idle.is_some() {
        return;
    }
    // A summary-only WebView never opened the editor and has no newer position.
    if let Some(position) = bookmark {
        desktop.bookmark = Some(position);
    }
    let app = app.clone();
    let keep = if matches!(desktop.painted, Some(Mode::Edit(_))) {
        90
    } else {
        15
    };
    desktop.idle = Some(tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(keep)).await;
        let main = app.clone();
        let _ = app.run_on_main_thread(move || {
            let state = main.state::<Mutex<Desktop>>();
            {
                let mut desktop = state.lock().unwrap();
                if desktop.mode != Mode::Hidden || desktop.epoch != epoch {
                    return;
                }
                desktop.idle = None;
            }
            if let Some(window) = main.get_webview_window(windows::QUICK) {
                report(windows::destroy_note(&window).map_err(|error| error.to_string()));
            }
        });
    }));
}

pub fn main_note(app: &AppHandle) -> Option<NoteId> {
    app.state::<Mutex<Desktop>>().lock().unwrap().main_note
}

pub fn open_main(app: &AppHandle, note: Option<NoteId>) -> Result<(), String> {
    if quitting(app) {
        return Ok(());
    }
    if let Some(id) = note {
        app.state::<Mutex<Desktop>>().lock().unwrap().main_note = Some(id);
    }
    let keep_note = app
        .state::<Mutex<Desktop>>()
        .lock()
        .unwrap()
        .keeps_visible();
    if !keep_note {
        transition(app, Mode::Hidden, false, false)?;
    }
    windows::main(app).map_err(|err| err.to_string())?;
    if let Some(id) = note {
        app.emit_to(windows::MAIN, events::MAIN_NOTE, id)
            .map_err(|err| err.to_string())?;
    }
    Ok(())
}

pub fn recent_document(app: &AppHandle) -> Option<NoteId> {
    app.state::<Mutex<Desktop>>()
        .lock()
        .unwrap()
        .document_order
        .last()
        .copied()
}

pub fn own_window_focused(app: &AppHandle, label: &str) {
    if windows::activating_note() || !Native::focused(false).own_app {
        return;
    }
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    if let Some(id) = windows::document_id(label)
        && desktop.document_order.last() != Some(&id)
    {
        desktop.document_order.retain(|open| *open != id);
        desktop.document_order.push(id);
    }
    // Activating Jotiva can temporarily make its old main window key on another
    // display while WebKit prepares the explicitly requested note.
    if desktop.pending_focus == Some(true)
        || desktop.note_windows.values().any(|w| w.focus == Some(true))
    {
        return;
    }
    desktop.pending_locate = None;
    drop(desktop);
    report(note_windows::follow(app, None, false));
    let mut desktop = state.lock().unwrap();
    if desktop.keeps_visible() || desktop.dragging_note {
        return;
    }
    desktop.target = None;
    desktop.origin = 0;
    drop(desktop);
    report(transition(app, Mode::Hidden, false, false));
}

fn on_host_change(app: &AppHandle, closed: u64, space_changed: bool) -> Result<(), String> {
    if closed != 0 {
        let siblings: Vec<_> = app
            .state::<Mutex<Desktop>>()
            .lock()
            .unwrap()
            .note_windows
            .iter()
            .filter_map(|(&id, w)| (w.host == Some(closed)).then_some(id))
            .collect();
        note_windows::retire(app, &siblings);
        let selected = app
            .state::<Mutex<Desktop>>()
            .lock()
            .unwrap()
            .attachments
            .remove(&closed)
            .and_then(|layout| layout.selected);
        let returned = app
            .state::<RwLock<Notes>>()
            .write()
            .unwrap()
            .return_to_main(closed);
        if let Some(id) = selected {
            release_note(app, id, true);
        }
        if returned {
            sync_recovery(app);
            app.emit_to(windows::MAIN, events::NOTES_CHANGED, ())
                .map_err(|err| err.to_string())?;
        }
    }

    let state = app.state::<Mutex<Desktop>>();
    let pending = state.lock().unwrap().pending_locate;
    if let Some(pending) = pending {
        let target = Native::focused(false);
        let mut desktop = state.lock().unwrap();
        let on_target = target.pid == pending.pid;
        let cancelled = closed == pending.host
            || pending.requested.elapsed() > Duration::from_secs(2)
            || (!on_target && !target.own_app && target.pid != pending.from_pid);
        if !cancelled {
            if !on_target || !Native::resigned() || !target.has_window {
                return Ok(());
            }
            let owner = app
                .state::<RwLock<Notes>>()
                .read()
                .unwrap()
                .host(pending.note);
            if target.id == pending.host && owner == Some(target.id) {
                desktop.pending_locate = None;
                desktop.origin = target.pid;
                desktop.target = Some(target);
                drop(desktop);
                note_windows::follow(app, Some(target.id), false)?;
                return edit(app, pending.note);
            }
        }
        desktop.pending_locate = None;
    }
    let waiting = state.lock().unwrap().attach_next.is_some();
    if waiting {
        match Native::focused_choice() {
            Ok(None) => {}
            result => {
                let pending = state.lock().unwrap().attach_next.take();
                if let Some((id, label)) = pending {
                    let outcome = result.and_then(|choice| {
                        let window = app
                            .get_webview_window(&label)
                            .ok_or("发起关联的窗口已关闭")?;
                        attach_note(&window, id, choice, None)
                    });
                    let completed = outcome.is_ok();
                    let _ = app.emit_to(label.as_str(), "desktop:attachment-finished", outcome);
                    if completed {
                        return Ok(());
                    }
                }
            }
        }
    }
    let target = Native::focused(false);
    let mut moved_screen = false;
    if target.id != 0 {
        let mut desktop = state.lock().unwrap();
        let layout = desktop.attachments.entry(target.id).or_default();
        if let Some(old) = layout.screen.replace(target.screen)
            && old != target.screen
        {
            moved_screen = true;
            if let Some(p) = layout.position.as_mut() {
                move_to_screen(p, old, target.screen);
            }
            for note in desktop
                .note_windows
                .values_mut()
                .filter(|w| w.host == Some(target.id))
            {
                move_to_screen(&mut note.position, old, target.screen);
            }
        }
    }
    if !target.own_app {
        let (docked, dragging) = {
            let d = state.lock().unwrap();
            (d.docked, d.dragging_note)
        };
        if !dragging {
            note_windows::follow(app, Some(target.id), docked)?;
            if moved_screen || space_changed {
                let ids: Vec<_> = state
                    .lock()
                    .unwrap()
                    .note_windows
                    .iter()
                    .filter_map(|(&id, w)| (w.visible && w.host == Some(target.id)).then_some(id))
                    .collect();
                for id in ids {
                    note_windows::update(app, id, true, false)?;
                }
            }
        }
    }
    {
        let desktop = state.lock().unwrap();
        let current_host_closed =
            closed != 0 && desktop.target.is_some_and(|target| target.id == closed);
        if !current_host_closed && (desktop.keeps_visible() || desktop.dragging_note) {
            return Ok(());
        }
    }
    if target.own_app {
        let state = app.state::<Mutex<Desktop>>();
        let mut desktop = state.lock().unwrap();
        if desktop.target.is_some_and(|target| target.id == closed) {
            desktop.target = None;
            desktop.origin = 0;
            desktop.before_search = Mode::Hidden;
            let mode = match desktop.mode {
                Mode::Search(_) => desktop.mode,
                _ => Mode::Hidden,
            };
            drop(desktop);
            return transition(app, mode, false, true);
        }
        return Ok(());
    }
    let (current_mode, same_host, mode) = {
        let state = app.state::<Mutex<Desktop>>();
        let mut desktop = state.lock().unwrap();
        let same_host = desktop.target.is_some_and(|old| old.id == target.id);
        desktop.origin = target.pid;
        desktop.target = (target.id != 0).then_some(target);
        (
            desktop.mode,
            same_host,
            desktop.restored_mode(desktop.summary(app), target.id),
        )
    };
    if same_host && current_mode == mode {
        if (moved_screen || space_changed)
            && mode != Mode::Hidden
            && let Some(window) = app.get_webview_window(windows::QUICK)
            && state.lock().unwrap().painted == Some(mode)
        {
            present(app, &window, false)?;
        }
        return Ok(());
    }
    transition(app, mode, false, moved_screen)
}

pub fn perform(app: &AppHandle, action: Action) -> Result<(), String> {
    if quitting(app) {
        return Ok(());
    }
    match action {
        Action::Quick => start_quick(app, false),
        Action::Search => show_find(app, false),
        Action::Dock => {
            let state = app.state::<Mutex<Desktop>>();
            let mut desktop = state.lock().unwrap();
            desktop.docked = !desktop.docked;
            let docked = desktop.docked;
            let host = desktop.target.map(|t| t.id);
            let mode = host.map(|id| desktop.restored_mode(desktop.summary(app), id));
            drop(desktop);
            note_windows::follow(app, host, docked)?;
            if let Some(mode) = mode {
                transition(app, mode, false, true)?;
            }
            Ok(())
        }
    }
}

pub fn find_from(window: &WebviewWindow, tiled: bool) -> Result<(), String> {
    let Some(id) = windows::note_id(window.label()) else {
        return show_find(window.app_handle(), tiled);
    };
    let state = window.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    let note = desktop.note_windows.get_mut(&id).ok_or("便签已关闭")?;
    if note.search == Some(tiled) {
        drop(desktop);
        return note_windows::dismiss(window.app_handle(), id);
    }
    note.search = Some(tiled);
    drop(desktop);
    note_windows::update(window.app_handle(), id, true, true)
}

pub fn show_find(app: &AppHandle, tiled: bool) -> Result<(), String> {
    let target = Native::focused(false);
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    if desktop.mode == Mode::Search(tiled) {
        drop(desktop);
        return dismiss(app);
    }
    if !matches!(desktop.mode, Mode::Search(_)) {
        desktop.search_from = (!target.own_app).then_some(target);
        if !target.own_app && !desktop.keeps_visible() {
            desktop.origin = target.pid;
            desktop.target = (target.id != 0).then_some(target);
        }
        desktop.before_search = desktop.mode;
    }
    drop(desktop);
    transition(app, Mode::Search(tiled), true, false)
}

pub fn appearance_changed(app: &AppHandle) -> Result<(), String> {
    let mode = app.state::<Mutex<Desktop>>().lock().unwrap().mode;
    if mode != Mode::Hidden {
        transition(app, mode, false, true)?;
    }
    Ok(())
}

pub fn start_quick(app: &AppHandle, create: bool) -> Result<(), String> {
    if quitting(app) {
        return Ok(());
    }
    let target = Native::focused(true);
    let (target, selected, epoch) = {
        let state = app.state::<Mutex<Desktop>>();
        let mut desktop = state.lock().unwrap();
        if !target.own_app {
            desktop.standalone = None;
            desktop.origin = target.pid;
            desktop.target = (target.id != 0).then_some(target);
        }
        let selected = match (target.own_app, desktop.mode) {
            (true, Mode::Edit(id)) => Some(id),
            _ => desktop
                .target
                .and_then(|target| desktop.attachments.get(&target.id)?.selected),
        };
        (desktop.target, selected, desktop.epoch)
    };
    if create {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            report(create_quick(app, windows::QUICK.to_string(), epoch).await)
        });
        return Ok(());
    }
    let id = {
        let state = app.state::<RwLock<Notes>>();
        let mut notes = state.write().unwrap();
        let host = target.map(|target| target.id);
        let existing = selected.or_else(|| host.and_then(|host| notes.latest(host)));
        if let Some(id) = existing {
            id
        } else {
            let source = target.map_or_else(|| "Jotiva".into(), |target| target.source());
            notes.new_note(host, source)
        }
    };
    edit(app, id)
}

pub async fn create_quick(app: AppHandle, label: String, epoch: u64) -> Result<(), String> {
    let caller = windows::note_id(&label);
    let (host, parent, source) = {
        let state = app.state::<Mutex<Desktop>>();
        let desktop = state.lock().unwrap();
        if desktop.closing_for_quit.is_some() {
            return Ok(());
        }
        if let Some(id) = caller {
            let w = desktop.note_windows.get(&id).ok_or("便签已关闭")?;
            if w.epoch != epoch {
                return Ok(());
            }
            (w.host, Some(id), String::new())
        } else {
            if desktop.epoch != epoch {
                return Ok(());
            }
            let target = desktop.target.filter(|_| desktop.standalone.is_none());
            let parent = target
                .and_then(|t| desktop.attachments.get(&t.id)?.selected)
                .or(desktop.standalone);
            (
                target.map(|t| t.id),
                parent,
                target.map_or_else(|| "Jotiva".into(), |t| t.source()),
            )
        }
    };
    let handle = app.clone();
    let id = tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<RwLock<Notes>>();
        let mut notes = state.write().unwrap();
        let source = if let Some(parent) = parent {
            if host.is_some() && notes.host(parent) != host {
                return Err("原便签的挂载窗口已改变，请重新新建".into());
            }
            if host.is_some() {
                notes.card(parent).ok_or("原便签已关闭")?.source
            } else {
                "Jotiva".into()
            }
        } else {
            source
        };
        notes.create_window_note(host, source)
    })
    .await
    .map_err(|e| e.to_string())??;
    let (sent, result) = tokio::sync::oneshot::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        note_saved(&handle, id);
        let _ = handle.emit("desktop:files-changed", ());
        let _ = handle.emit(events::NOTES_CHANGED, ());
        let current = {
            let state = handle.state::<Mutex<Desktop>>();
            let desktop = state.lock().unwrap();
            desktop.closing_for_quit.is_none()
                && match caller {
                    Some(id) => desktop
                        .note_windows
                        .get(&id)
                        .is_some_and(|w| w.epoch == epoch && w.host == host),
                    None => {
                        desktop.epoch == epoch
                            && desktop
                                .target
                                .filter(|_| desktop.standalone.is_none())
                                .map(|t| t.id)
                                == host
                    }
                }
        };
        let result = if !current {
            Ok(())
        } else if parent.is_some() {
            note_windows::open(
                &handle,
                id,
                host,
                handle.get_webview_window(&label).as_ref(),
            )
        } else {
            edit(&handle, id)
        };
        let _ = sent.send(result);
    })
    .map_err(|e| e.to_string())?;
    result.await.map_err(|e| e.to_string())?
}

fn edit(app: &AppHandle, id: NoteId) -> Result<(), String> {
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    let target = desktop.target;
    let existing = desktop.note_windows.contains_key(&id);
    let primary = target.and_then(|t| desktop.attachments.get(&t.id)?.selected);
    let another_host_open = primary.is_none()
        && target.is_some_and(|t| {
            desktop.attachments.iter().any(|(&host, a)| {
                host != t.id
                    && a.expanded
                    && a.selected
                        .is_some_and(|id| !desktop.note_windows.contains_key(&id))
            })
        });
    if existing
        || another_host_open
        || primary.is_some_and(|other| other != id && !desktop.note_windows.contains_key(&other))
    {
        let previous_mode = desktop.mode;
        let restore = if another_host_open {
            Mode::Hidden
        } else {
            let summary = desktop.summary(app);
            target.map_or(summary, |t| desktop.restored_mode(summary, t.id))
        };
        let desktop_can_restore = desktop.pinned.is_none() && desktop.standalone.is_none()
            || matches!(previous_mode, Mode::Search(_));
        desktop.docked = false;
        let host = if existing {
            desktop.note_windows[&id].host
        } else {
            target.map(|t| t.id)
        };
        drop(desktop);
        if previous_mode != restore && desktop_can_restore {
            transition(app, restore, false, false)?;
        }
        if host.is_some() {
            note_windows::follow(app, host, false)?;
        }
        return note_windows::open(
            app,
            id,
            host,
            app.get_webview_window(windows::QUICK).as_ref(),
        );
    }
    desktop.docked = false;
    if let Some(target) = desktop.target {
        desktop.standalone = None;
        let attachment = desktop.attachments.entry(target.id).or_default();
        attachment.screen = Some(target.screen);
        attachment.selected = Some(id);
        attachment.expanded = true;
    } else {
        desktop.standalone = Some(id);
    }
    drop(desktop);
    transition(app, Mode::Edit(id), true, true)
}

pub fn close_quick(window: &WebviewWindow, epoch: u64) -> Result<(), String> {
    if let Some(id) = windows::note_id(window.label()) {
        return note_windows::close(window, id, epoch);
    }
    let app = window.app_handle();
    let state = app.state::<Mutex<Desktop>>();
    let desktop = state.lock().unwrap();
    let quitting = desktop.closing_for_quit.is_some();
    if desktop.epoch != epoch && !quitting {
        return Ok(());
    }
    // The search overlay does not own the document underneath it.
    if !quitting && matches!(desktop.mode, Mode::Search(_)) {
        drop(desktop);
        return dismiss(app);
    }
    let note = desktop.current_note();
    let return_focus = !quitting && desktop.mode != Mode::Hidden;
    let (target, origin) = (desktop.target, desktop.origin);
    let independent = desktop.standalone.is_some();
    drop(desktop);
    if let Some(id) = note
        && !quitting
        && !independent
    {
        detach(app, id)?;
    }
    windows::destroy_note(window).map_err(|error| error.to_string())?;
    let mut desktop = state.lock().unwrap();

    desktop.mode = Mode::Hidden;
    desktop.before_search = Mode::Hidden;
    desktop.bookmark = None;
    drop(desktop);
    if let Some(id) = note {
        release_note(app, id, true);
    }
    if return_focus {
        restore_host(target, origin);
    }
    Ok(())
}

fn restore_host(target: Option<Target>, origin: i32) {
    let raised = target.and_then(|target| Native::raise(target.id)).is_some();
    if !raised && origin != 0 {
        Native::activate(origin);
    }
}

pub fn drag_quick(window: &WebviewWindow) -> Result<(), String> {
    if let Some(id) = windows::note_id(window.label()) {
        if let Some(w) = window
            .state::<Mutex<Desktop>>()
            .lock()
            .unwrap()
            .note_windows
            .get_mut(&id)
        {
            w.dragged = w.search.is_none();
        }
        return window.start_dragging().map_err(|e| e.to_string());
    }
    let state = window.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    if desktop.mode == Mode::Hidden || (matches!(desktop.mode, Mode::Summary(_)) && desktop.docked)
    {
        return Ok(());
    }
    if desktop.current_note().is_some() && !desktop.docked {
        desktop.dragged_layout = Some(
            desktop
                .target
                .map_or(LayoutOwner::Independent, |t| LayoutOwner::Host(t.id)),
        );
    }
    drop(desktop);
    window.start_dragging().map_err(|error| error.to_string())
}

#[cfg(any(windows, target_os = "macos"))]
pub extern "C" fn quick_resized(id: NoteId, x: f64, y: f64, width: f64, height: f64) {
    let app = APP.get().expect("native observers started");
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    if id != 0 {
        if let Some(note) = desktop.note_windows.get_mut(&id)
            && note.search.is_none()
        {
            note.position = LogicalPosition::new(x, y);
            let size = Some(LogicalSize::new(width, height));
            if note.expanded {
                note.edit_size = size;
            } else {
                note.summary_size = size;
            }
        }
        return;
    }
    let edit = matches!(desktop.mode, Mode::Edit(_));
    if !edit && !matches!(desktop.mode, Mode::Summary(_)) || desktop.docked {
        return;
    }
    let layout = if let Some(target) = desktop.target {
        desktop.attachments.entry(target.id).or_default()
    } else {
        &mut desktop.free_layout
    };
    layout.position = Some(LogicalPosition::new(x, y));
    let size = Some(LogicalSize::new(width, height));
    if edit {
        layout.edit_size = size;
    } else {
        layout.summary_size = size;
    }
}

pub fn set_pinned(window: &WebviewWindow, id: NoteId, pinned: bool) -> Result<(), String> {
    let app = window.app_handle();
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    if windows::note_id(window.label()) == Some(id) {
        let note = desktop.note_windows.get_mut(&id).ok_or("便签已关闭")?;
        window
            .set_always_on_top(pinned || note.host.is_some())
            .map_err(|e| e.to_string())?;
        windows::note_spaces(window, pinned || note.host.is_some()).map_err(|e| e.to_string())?;
        note.pinned = pinned;
        return Ok(());
    }
    if desktop.mode != Mode::Edit(id) {
        return Err("便签已切换".into());
    }
    let floating = desktop.target.is_some() || pinned;
    window
        .set_always_on_top(floating)
        .map_err(|error| error.to_string())?;
    windows::note_spaces(window, floating).map_err(|e| e.to_string())?;
    desktop.pinned = pinned.then_some(id);
    Ok(())
}

pub fn observe_document_drag(window: &WebviewWindow, started: bool) {
    if windows::is_quick(window.label()) {
        window
            .state::<Mutex<Desktop>>()
            .lock()
            .unwrap()
            .dragging_note = started;
    }
    Native::watch_drag(started);
}

pub fn end_quick_drag(window: &WebviewWindow) {
    observe_document_drag(window, false);
    report(on_host_change(window.app_handle(), 0, false));
}

pub fn dismiss_window(window: &WebviewWindow) -> Result<(), String> {
    if let Some(id) = windows::note_id(window.label()) {
        return note_windows::dismiss(window.app_handle(), id);
    }
    dismiss(window.app_handle())
}

pub fn dismiss(app: &AppHandle) -> Result<(), String> {
    let (mode, target, origin, focus_editor) = {
        let state = app.state::<Mutex<Desktop>>();
        let mut desktop = state.lock().unwrap();
        if matches!(desktop.mode, Mode::Search(_)) {
            let mode = std::mem::replace(&mut desktop.before_search, Mode::Hidden);
            match desktop.search_from.take() {
                Some(from) => (mode, (from.id != 0).then_some(from), from.pid, false),
                None => (
                    mode,
                    desktop.target,
                    desktop.origin,
                    matches!(mode, Mode::Edit(_)),
                ),
            }
        } else {
            if let Some(target) = desktop.target {
                desktop.attachments.entry(target.id).or_default().expanded = false;
            }
            let summary = desktop
                .standalone
                .map_or_else(|| desktop.summary(app), Mode::Summary);
            (summary, desktop.target, desktop.origin, false)
        }
    };
    transition(app, mode, focus_editor, false)?;
    if !focus_editor {
        restore_host(target, origin);
    }
    Ok(())
}

pub fn locate_from(window: &WebviewWindow, id: NoteId) -> Result<(), String> {
    if let Some(own) = windows::note_id(window.label()) {
        let state = window.state::<Mutex<Desktop>>();
        let mut desktop = state.lock().unwrap();
        let w = desktop.note_windows.get_mut(&own).ok_or("便签已关闭")?;
        let searching = w.search.take().is_some();
        let host = w.host;
        if own == id {
            w.expanded = true;
        }
        drop(desktop);
        if own == id && !searching {
            return note_windows::update(window.app_handle(), id, true, true);
        }
        if searching {
            note_windows::update(window.app_handle(), own, true, false)?;
        }
        if own == id && host.is_none() {
            return note_windows::update(window.app_handle(), id, true, true);
        }
    }
    locate(window.app_handle(), id)
}

pub fn locate(app: &AppHandle, id: NoteId) -> Result<(), String> {
    {
        let state = app.state::<Mutex<Desktop>>();
        let desktop = state.lock().unwrap();
        if desktop
            .note_windows
            .get(&id)
            .is_some_and(|w| w.host.is_none())
        {
            drop(desktop);
            return note_windows::open(app, id, None, None);
        }
        if desktop.standalone == Some(id) && !matches!(desktop.mode, Mode::Search(_)) {
            drop(desktop);
            return edit(app, id);
        }
    }
    let owner = app.state::<RwLock<Notes>>().read().unwrap().host(id);
    let Some(owner) = owner else {
        return open_main(app, Some(id));
    };
    let current = Native::focused(false);
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    // Searching inside a note on this host already has the right window behind it.
    let searching_here = current.own_app
        && matches!(desktop.mode, Mode::Search(_))
        && matches!(desktop.before_search, Mode::Edit(_) | Mode::Summary(_))
        && desktop.pinned.is_none()
        && desktop.target.is_some_and(|target| target.id == owner);
    if current.id == owner {
        desktop.target = Some(current);
    } else if !searching_here {
        drop(desktop);
        let Some(target) = Native::raise(owner) else {
            let target = Native::editor_target_after_denied_raise(owner)
                .ok_or("无法激活挂载窗口，请重试")?;
            {
                let mut desktop = state.lock().unwrap();
                desktop.pending_locate = None;
                desktop.origin = target.pid;
                desktop.target = Some(target);
            }
            return edit(app, id);
        };
        desktop = state.lock().unwrap();
        // AXRaise acknowledges a request before Workspace transfers keyboard ownership.
        // Resume when the selected document is frontmost and Jotiva has resigned activation.
        desktop.pending_locate = Some(PendingLocate {
            note: id,
            host: target.id,
            pid: target.pid,
            from_pid: current.pid,
            requested: Instant::now(),
        });
        drop(desktop);
        return on_host_change(app, 0, false);
    }
    drop(desktop);
    edit(app, id)
}

pub fn attach_on_activation(window: &WebviewWindow, id: NoteId) -> Result<(), String> {
    let app = window.app_handle();
    if !Native::permission(false) {
        return Err("辅助功能权限已关闭，请重新开始挂载。".into());
    }
    if !app
        .state::<RwLock<Notes>>()
        .read()
        .unwrap()
        .note(id)
        .is_some_and(|note| note.version > 0)
    {
        return Err("请先输入并保存正文".into());
    }
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    if let Some((_, old)) = desktop.attach_next.replace((id, window.label().to_owned())) {
        let _ = app.emit_to(
            old.as_str(),
            "desktop:attachment-finished",
            Err::<String, _>("已开始另一次关联"),
        );
    }
    Ok(())
}

pub fn cancel_attachment(window: &WebviewWindow) {
    let state = window.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    if desktop
        .attach_next
        .as_ref()
        .is_some_and(|(_, label)| label == window.label())
    {
        desktop.attach_next = None;
    }
}

pub fn locate_source(app: &AppHandle, key: &str) -> Result<(), String> {
    let host = app
        .state::<RwLock<Notes>>()
        .read()
        .unwrap()
        .live_source(key)?;
    if Native::raise(host).is_some() {
        Ok(())
    } else {
        Err("窗口已关闭或无法激活".into())
    }
}

pub fn close_source(app: &AppHandle, key: &str) -> Result<(), String> {
    let (host, ids) = app
        .state::<RwLock<Notes>>()
        .write()
        .unwrap()
        .close_source(key)?;
    if let Some(host) = host {
        Native::unwatch(host);
        app.state::<Mutex<Desktop>>()
            .lock()
            .unwrap()
            .attachments
            .remove(&host);
    }
    association_removed(app, &ids)?;
    sync_recovery(app);
    app.emit(events::NOTES_CHANGED, ())
        .map_err(|e| e.to_string())
}

fn association_removed(app: &AppHandle, ids: &[NoteId]) -> Result<(), String> {
    note_windows::retire(app, ids);
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    if matches!(desktop.before_search, Mode::Edit(id) | Mode::Summary(id) if ids.contains(&id)) {
        desktop.before_search = Mode::Hidden;
    }
    let affected = matches!(desktop.mode, Mode::Edit(id) | Mode::Summary(id) if ids.contains(&id));
    let next = desktop.summary(app);
    drop(desktop);
    if affected {
        transition(app, next, false, false)?;
    }
    Ok(())
}

pub fn notes_removed(app: &AppHandle, ids: &[NoteId]) {
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    let notes = app.state::<RwLock<Notes>>();
    let notes = notes.read().unwrap();
    desktop.attachments.retain(|host_id, item| {
        if item.selected.is_some_and(|id| ids.contains(&id)) {
            item.selected = notes.latest(*host_id);
        }
        if item.selected.is_none() && !notes.is_registered(*host_id) {
            Native::unwatch(*host_id);
            false
        } else {
            true
        }
    });
    if desktop.main_note.is_some_and(|id| ids.contains(&id)) {
        desktop.main_note = None;
    }
    drop(notes);
    drop(desktop);
    report(association_removed(app, ids));
    sync_recovery(app);
}

pub fn attach_note(
    window: &WebviewWindow,
    id: NoteId,
    choice: Option<u64>,
    source: Option<&str>,
) -> Result<String, String> {
    let app = window.app_handle();
    let quick = window.label() == windows::QUICK;
    let sibling = windows::note_id(window.label()) == Some(id);
    let own_document = windows::document_id(window.label()) == Some(id);
    if window.label() != windows::MAIN
        && !own_document
        && !sibling
        && !(quick && app.state::<Mutex<Desktop>>().lock().unwrap().mode == Mode::Edit(id))
    {
        return Err("请从当前文档修改窗口关联".into());
    }
    let (host, name) = resolve_target(app, choice, source)?.ok_or("请选择目标窗口")?;
    let state = app.state::<RwLock<Notes>>();
    let mut notes = state.write().unwrap();
    let previous = notes.host(id).filter(|old| *old != host);
    let key = match notes.attach_to_host(id, host, name) {
        Ok(key) => key,
        Err(error) => {
            let unused = notes.latest(host).is_none() && !notes.is_registered(host);
            drop(notes);
            if unused {
                Native::unwatch(host);
            }
            return Err(error);
        }
    };
    let saved = notes.disk_path(id).is_some();
    let previous = previous.map(|host| (host, notes.latest(host), notes.is_registered(host)));
    drop(notes);

    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    let has_window = desktop.note_windows.contains_key(&id);
    let next = desktop.attachments.entry(host).or_default();
    // Re-selecting the same binding must keep its explicit presentation choice.
    if !has_window {
        next.selected = Some(id);
    }
    if quick {
        next.expanded = true;
    }
    let mut mode = None;
    if let Some((host, latest, registered)) = previous {
        let unused = latest.is_none() && !registered;
        if unused {
            desktop.attachments.remove(&host);
        } else if let Some(attachment) = desktop.attachments.get_mut(&host)
            && attachment.selected == Some(id)
        {
            attachment.selected = latest;
        }
        let refresh_summary = desktop.target.is_some_and(|target| target.id == host)
            && desktop.mode == Mode::Summary(id);
        mode = refresh_summary.then(|| desktop.summary(app));
    }
    if let Some(w) = desktop.note_windows.get_mut(&id) {
        w.host = Some(host);
    }
    drop(desktop);
    if let Some((host, None, false)) = previous {
        Native::unwatch(host);
    }
    if let Some(mode) = mode {
        report(transition(app, mode, false, false));
    }
    if saved {
        remember_host(app, host);
        sync_recovery(app);
    }
    let _ = app.emit_to(windows::MAIN, events::NOTES_CHANGED, ());
    if sibling {
        locate(app, id)?;
    }
    if quick {
        app.state::<Mutex<Desktop>>().lock().unwrap().standalone = None;
        locate(app, id)?;
    }
    Ok(key)
}

fn resolve_target(
    app: &AppHandle,
    choice: Option<u64>,
    source: Option<&str>,
) -> Result<Option<(HostId, String)>, String> {
    match (choice, source) {
        (Some(choice), None) => {
            let host = Native::register_source(choice)?;
            Ok(Some((host.id, host.source())))
        }
        (None, Some(source)) => {
            let host = app
                .state::<RwLock<Notes>>()
                .read()
                .unwrap()
                .live_source(source)?;
            Ok(Some((host, String::new())))
        }
        (None, None) => Ok(None),
        (Some(_), Some(_)) => Err("一次只能选择一个来源窗口".into()),
    }
}

pub fn create_from_text(
    app: &AppHandle,
    text: String,
    choice: Option<u64>,
    source: Option<&str>,
) -> Result<NoteId, String> {
    if text.trim().is_empty() {
        return Err("文本为空".into());
    }
    let target = resolve_target(app, choice, source)?;
    let state = app.state::<RwLock<Notes>>();
    let mut notes = state.write().unwrap();
    let id = if let Some(source) = source {
        notes.new_at_source(source)?
    } else if let Some((host, name)) = &target {
        notes.new_note(Some(*host), name.clone())
    } else {
        notes.new_note(None, "Jotiva".into())
    };
    if let Err(error) = notes.save(id, 0, text) {
        notes.discard_empty(id);
        if let Some((host, _)) = target
            && notes.latest(host).is_none()
            && !notes.is_registered(host)
        {
            Native::unwatch(host);
        }
        return Err(error);
    }
    drop(notes);
    if let Some((host, _)) = target {
        remember_host(app, host);
    }
    let _ = app.emit_to(windows::MAIN, "desktop:files-changed", ());
    let _ = app.emit_to(windows::MAIN, events::NOTES_CHANGED, ());
    Ok(id)
}

pub fn float_note(app: &AppHandle, id: NoteId, standalone: bool) -> Result<(), String> {
    let card = app
        .state::<RwLock<Notes>>()
        .read()
        .unwrap()
        .card(id)
        .ok_or("找不到便签")?;
    if !standalone && card.attached {
        return locate(app, id);
    }
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    if let Some(window) = desktop.note_windows.get_mut(&id) {
        window.host = None;
        drop(desktop);
        return note_windows::open(app, id, None, None);
    }
    if desktop.current_note().is_some_and(|current| current != id) {
        drop(desktop);
        return note_windows::open(
            app,
            id,
            None,
            app.get_webview_window(windows::QUICK).as_ref(),
        );
    }
    desktop.target = None;
    desktop.origin = 0;
    desktop.standalone = Some(id);
    desktop.free_layout = Attachment {
        position: Some(host::pointer()),
        ..Attachment::default()
    };
    drop(desktop);
    transition(app, Mode::Edit(id), true, true)
}

pub fn detach(app: &AppHandle, id: NoteId) -> Result<(), String> {
    let state = app.state::<RwLock<Notes>>();
    let mut notes = state.write().unwrap();
    let previous = notes.host(id);
    notes.detach(id)?;
    let previous = previous.map(|host| (host, notes.latest(host), notes.is_registered(host)));
    drop(notes);
    if let Some((host, latest, registered)) = previous {
        let state = app.state::<Mutex<Desktop>>();
        let mut desktop = state.lock().unwrap();
        if let Some(attachment) = desktop.attachments.get_mut(&host)
            && attachment.selected == Some(id)
        {
            attachment.selected = latest;
        }
        if latest.is_none() && !registered {
            desktop.attachments.remove(&host);
            drop(desktop);
            Native::unwatch(host);
        }
    }
    sync_recovery(app);
    let _ = app.emit_to(windows::MAIN, events::NOTES_CHANGED, ());
    Ok(())
}

pub fn detach_note(app: &AppHandle, id: NoteId) -> Result<(), String> {
    detach(app, id)?;
    association_removed(app, &[id])
}

pub fn recall_note(app: &AppHandle, id: NoteId) -> Result<(), String> {
    detach_note(app, id)?;
    open_main(app, Some(id))
}

pub fn counts(app: &AppHandle) -> WindowCounts {
    let windows = app.webview_windows();
    WindowCounts {
        webviews: windows.len(),
        main: app.get_webview_window(windows::MAIN).is_some(),
        settings: app.get_webview_window(windows::SETTINGS).is_some(),
        quick: windows.keys().any(|label| windows::is_quick(label)),
    }
}

pub fn retain_editor(app: &AppHandle, label: &str, id: NoteId) {
    app.state::<Mutex<Desktop>>()
        .lock()
        .unwrap()
        .editing
        .insert(label.into(), id);
}

pub fn release_editor(app: &AppHandle, label: &str, id: NoteId) {
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    if desktop.editing.get(label) == Some(&id) {
        desktop.editing.remove(label);
    }
    drop(desktop);
    release_note(app, id, label == windows::QUICK);
}

pub fn release_note(app: &AppHandle, id: NoteId, from_quick: bool) {
    let state = app.state::<Mutex<Desktop>>();
    let desktop = state.lock().unwrap();
    if desktop.editing.values().any(|open| *open == id)
        || desktop.mode == Mode::Edit(id)
        || (from_quick && desktop.main_note == Some(id))
    {
        return;
    }
    // Returning to an expanded host also restores an untouched, in-memory draft.
    let state_notes = app.state::<RwLock<Notes>>();
    if from_quick
        && let Some(host) = state_notes.read().unwrap().host(id)
        && desktop
            .attachments
            .get(&host)
            .is_some_and(|layout| layout.expanded && layout.selected == Some(id))
    {
        return;
    }
    let summary = desktop.mode == Mode::Summary(id);
    drop(desktop);
    let mut notes = state_notes.write().unwrap();
    let owner = notes.host(id);
    if !notes.discard_empty(id) {
        return;
    }
    let next = owner.and_then(|owner| notes.latest(owner));
    let explicitly_registered = owner.is_some_and(|owner| notes.is_registered(owner));
    drop(notes);
    let empty_host = owner.filter(|_| next.is_none() && !explicitly_registered);
    if let Some(owner) = empty_host {
        Native::unwatch(owner);
    }
    let mut desktop = state.lock().unwrap();
    if let Some(owner) = empty_host {
        desktop.attachments.remove(&owner);
        if desktop.target.is_some_and(|target| target.id == owner) {
            desktop.target = None;
        }
    } else if let Some(attachment) = owner.and_then(|owner| desktop.attachments.get_mut(&owner))
        && attachment.selected == Some(id)
    {
        attachment.selected = next;
    }
    if desktop.main_note == Some(id) {
        desktop.main_note = None;
    }
    drop(desktop);
    if summary {
        report(transition(
            app,
            next.map_or(Mode::Hidden, Mode::Summary),
            false,
            false,
        ));
    }
}

pub fn note_saved(app: &AppHandle, id: NoteId) {
    let host = {
        let notes = app.state::<RwLock<Notes>>();
        let notes = notes.read().unwrap();
        notes.host(id).filter(|host| notes.needs_identity(*host))
    };
    if let Some(host) = host {
        Native::capture_identity(host, captured);
    }
    let state = app.state::<Mutex<Desktop>>();
    let desktop = state.lock().unwrap();
    let summary = desktop
        .note_windows
        .get(&id)
        .is_some_and(|w| w.visible && !w.expanded && w.search.is_none());
    let current = desktop.mode;
    drop(desktop);
    if summary {
        report(note_windows::update(app, id, true, false));
    }
    if current == Mode::Summary(id) {
        report(transition(app, current, false, true));
    }
}

pub fn quitting(app: &AppHandle) -> bool {
    app.state::<Mutex<Desktop>>()
        .lock()
        .unwrap()
        .closing_for_quit
        .is_some()
}

pub fn request_quit(app: &AppHandle) -> Result<(), String> {
    let windows = app.webview_windows();
    let state = app.state::<Mutex<Desktop>>();
    let mut desktop = state.lock().unwrap();
    if desktop.closing_for_quit.is_some() {
        return Ok(());
    }
    desktop.closing_for_quit = Some(windows.keys().cloned().collect());
    drop(desktop);
    if windows.is_empty() {
        app.exit(0);
    }
    // The workspace owns terminal sessions and can veto quitting. Close it first.
    if let Some(main) = windows.get(crate::windows::MAIN) {
        if let Err(error) = main
            .unminimize()
            .and_then(|_| main.show())
            .and_then(|_| main.set_focus())
            .and_then(|_| main.close())
        {
            cancel_quit(app);
            return Err(error.to_string());
        }
        return Ok(());
    }
    for window in windows.values() {
        if let Err(error) = window.close() {
            cancel_quit(app);
            return Err(error.to_string());
        }
    }
    Ok(())
}

pub fn cancel_quit(app: &AppHandle) {
    app.state::<Mutex<Desktop>>()
        .lock()
        .unwrap()
        .closing_for_quit = None;
}

pub fn hold_for_accessibility(window: &WebviewWindow, hold: bool) {
    if windows::is_quick(window.label()) {
        let state = window.state::<Mutex<Desktop>>();
        let mut desktop = state.lock().unwrap();
        let panel = windows::note_id(window.label());
        if hold {
            desktop.accessibility_panels.insert(panel);
        } else {
            desktop.accessibility_panels.remove(&panel);
        }
    }
}

pub fn window_destroyed(app: &AppHandle, label: &str) {
    let (exit, opened) = {
        let state = app.state::<Mutex<Desktop>>();
        let mut desktop = state.lock().unwrap();
        if windows::is_quick(label) {
            desktop
                .accessibility_panels
                .remove(&windows::note_id(label));
        }
        if let Some(id) = windows::document_id(label) {
            desktop.document_order.retain(|open| *open != id);
        }
        if desktop
            .attach_next
            .as_ref()
            .is_some_and(|(_, owner)| owner == label)
        {
            desktop.attach_next = None;
        }
        if label == windows::MAIN || windows::is_quick(label) && desktop.dragging_note {
            Native::watch_drag(false);
        }
        if label == windows::QUICK {
            desktop.painted = None;
            desktop.dragging_note = false;
            desktop.dragged_layout = None;
            desktop.pinned = None;
            desktop.mode = Mode::Hidden;
            desktop.pending_focus = None;
            desktop.before_search = Mode::Hidden;
            desktop.search_from = None;
            desktop.standalone = None;
            desktop.epoch += 1;
            if let Some(idle) = desktop.idle.take() {
                idle.abort();
            }
        }
        let exit = desktop.closing_for_quit.as_mut().is_some_and(|closing| {
            closing.remove(label);
            closing.is_empty()
        });
        (exit, desktop.editing.remove(label))
    };
    if let Some(id) = opened {
        release_note(app, id, label == windows::QUICK);
    }
    if !exit && label == windows::MAIN && quitting(app) {
        for (label, window) in app.webview_windows() {
            if label != windows::MAIN
                && let Err(e) = window.close()
            {
                cancel_quit(app);
                report(Err(e.to_string()));
                break;
            }
        }
    }
    if exit {
        // A first save can finish just before the last window closes, while its
        // asynchronous AX identity capture is still waiting on a host application.
        let state = app.state::<RwLock<Notes>>();
        let missing = state.read().unwrap().hosts_needing_identity();
        for host in missing {
            if let Some(identity) = Native::identity(host) {
                report(state.write().unwrap().remember(host, identity));
            }
        }
        app.exit(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_quick_panel_requesting_access_stays_visible() {
        let mut desktop = Desktop::default();
        desktop.accessibility_panels.insert(Some(7));
        assert!(!desktop.keeps_visible());
        desktop.accessibility_panels.insert(None);
        assert!(desktop.keeps_visible());
        desktop.accessibility_panels.remove(&None);
        assert!(!desktop.keeps_visible());
    }

    #[test]
    fn remembered_origin_stays_on_the_smaller_destination_display() {
        let large = host::Rect {
            x: 0.0,
            y: 31.0,
            width: 3008.0,
            height: 1597.0,
        };
        let small = host::Rect {
            x: -1470.0,
            y: 1726.0,
            width: 1470.0,
            height: 922.0,
        };
        let mut point = LogicalPosition::new(2616.0, 654.0);
        move_to_screen(&mut point, large, small);
        assert!(point.x > small.x && point.x < 0.0 && point.y > small.y);
        move_to_screen(&mut point, small, large);
        assert!((point.x - 2616.0).abs() < 0.01 && (point.y - 654.0).abs() < 0.01);
    }

    #[test]
    fn switching_hosts_restores_the_explicit_shape_and_docking_is_reversible() {
        let mut desktop = Desktop::default();
        desktop.attachments.insert(
            4,
            Attachment {
                selected: Some(7),
                expanded: true,
                ..Attachment::default()
            },
        );
        let summary = Mode::Summary(7);
        assert!(desktop.restored_mode(summary, 4) == Mode::Edit(7));
        assert!(desktop.restored_mode(Mode::Hidden, 5) == Mode::Hidden);
        desktop.docked = true;
        assert!(desktop.restored_mode(summary, 4) == summary);
        desktop.docked = false;
        assert!(desktop.restored_mode(summary, 4) == Mode::Edit(7));
        desktop.attachments.get_mut(&4).unwrap().expanded = false;
        assert!(desktop.restored_mode(summary, 4) == summary);
    }

    #[test]
    fn pin_only_keeps_the_selected_note_visible_across_app_switches() {
        let mut desktop = Desktop {
            mode: Mode::Edit(1),
            ..Desktop::default()
        };
        assert!(!desktop.keeps_visible());
        desktop.pinned = Some(1);
        assert!(desktop.keeps_visible() && desktop.floats_above());
        desktop.mode = Mode::Summary(1);
        assert!(desktop.keeps_visible());
        desktop.mode = Mode::Edit(2);
        assert!(!desktop.keeps_visible());
        desktop.pinned = None;
        desktop.standalone = Some(2);
        assert!(desktop.keeps_visible() && !desktop.floats_above());
    }
}
