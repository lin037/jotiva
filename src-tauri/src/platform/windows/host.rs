use crate::host::{DropTarget, SourceChoice, Target};
use crate::platform::geometry::rect_at_pixel;
use crate::platform::geometry::window_origin;
use crate::platform::shared::host_registry::Registry;
use crate::platform::{HostBackend, Native};
use std::{
    path::Path,
    ptr,
    sync::{LazyLock, Mutex, OnceLock},
};
use tauri::WebviewWindow;
use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND, LPARAM, POINT, RECT},
    System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    },
    UI::{
        Accessibility::{HWINEVENTHOOK, SetWinEventHook},
        WindowsAndMessaging::*,
    },
};

// HWNDs may be reused. Only issue opaque IDs for windows observed in this session,
// discard them on destruction, and recheck the owning process before every operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowRef {
    handle: usize,
    pid: u32,
}
static REGISTRY: LazyLock<Mutex<Registry<WindowRef>>> = LazyLock::new(Default::default);
static NOTIFY: OnceLock<extern "C" fn(u64, bool)> = OnceLock::new();
fn inspect(hwnd: HWND) -> Option<WindowRef> {
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    (pid != 0 && !hwnd.is_null()).then_some(WindowRef {
        handle: hwnd as usize,
        pid,
    })
}
impl WindowRef {
    fn hwnd(self) -> HWND {
        self.handle as HWND
    }
    fn title(self) -> String {
        let mut text = [0u16; 512];
        let length = unsafe { GetWindowTextW(self.hwnd(), text.as_mut_ptr(), text.len() as i32) };
        String::from_utf16_lossy(&text[..length as usize])
    }
    fn app(self) -> String {
        let mut buffer = vec![0u16; 32768];
        let mut len = buffer.len() as u32;
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, self.pid);
            if !process.is_null() {
                let ok = QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut len);
                CloseHandle(process);
                if ok != 0 {
                    let path = String::from_utf16_lossy(&buffer[..len as usize]);
                    return Path::new(&path)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned();
                }
            }
        }
        format!("进程 {}", self.pid)
    }
    fn external(self) -> bool {
        self.pid != std::process::id()
            && unsafe {
                IsWindowVisible(self.hwnd()) != 0
                    && self.hwnd() != GetShellWindow()
                    && self.hwnd() != GetDesktopWindow()
            }
    }
}
fn target(w: WindowRef, id: u64) -> Target {
    let mut rect = RECT::default();
    unsafe { GetWindowRect(w.hwnd(), &mut rect) };
    let screen = rect_at_pixel(
        (rect.left as f64 + rect.right as f64) / 2.,
        (rect.top as f64 + rect.bottom as f64) / 2.,
    );
    let mut result = Target {
        id,
        pid: w.pid as i32,
        own_app: w.pid == std::process::id(),
        has_window: true,
        fullscreen: false,
        screen,
        source: [0; 256],
    };
    result.set_source(&format!("{} · {}", w.app(), w.title()));
    result
}
fn foreground() -> Option<WindowRef> {
    inspect(unsafe { GetForegroundWindow() })
}
fn enumerate() -> Vec<WindowRef> {
    unsafe extern "system" fn visit(hwnd: HWND, arg: LPARAM) -> i32 {
        if let Some(w) = inspect(hwnd).filter(|w| w.external() && !w.title().is_empty()) {
            unsafe { &mut *(arg as *mut Vec<WindowRef>) }.push(w);
        }
        1
    }
    let mut rows = Vec::new();
    unsafe { EnumWindows(Some(visit), &mut rows as *mut _ as LPARAM) };
    rows
}
fn transient(w: WindowRef) -> u64 {
    REGISTRY.lock().unwrap().transient(w)
}
// Win32 does not supply a durable document identity for arbitrary third-party windows.

fn save_quick_frame(handle: usize) {
    use tauri::Manager;
    for (label, window) in crate::desktop::app().webview_windows() {
        if !crate::windows::is_quick(&label)
            || !window.hwnd().is_ok_and(|hwnd| hwnd.0 as usize == handle)
        {
            continue;
        }
        if let (Ok(origin), Ok(size), Ok(scale)) = (
            window_origin(&window),
            window.inner_size(),
            window.scale_factor(),
        ) {
            let size = size.to_logical::<f64>(scale);
            crate::desktop::quick_resized(
                crate::windows::note_id(&label).unwrap_or(0),
                origin.x,
                origin.y,
                size.width,
                size.height,
            );
        }
    }
}

impl HostBackend for Native {
    fn supported() -> bool {
        true
    }
    fn register_source(choice: u64) -> Result<Target, String> {
        let mut reg = REGISTRY.lock().unwrap();
        let w = reg
            .get(choice)
            .filter(|w| inspect(w.hwnd()) == Some(*w) && w.external())
            .ok_or("窗口已关闭，请刷新后重试")?;
        reg.watch(w);
        drop(reg);
        Ok(target(w, choice))
    }

    fn focused(attach: bool) -> Target {
        let Some(w) = foreground() else {
            return Target::default();
        };
        let id = if w.external() {
            let mut reg = REGISTRY.lock().unwrap();
            if attach {
                reg.watch(w)
            } else {
                reg.watched_id(w).unwrap_or(0)
            }
        } else {
            0
        };
        target(w, id)
    }

    fn permission(_: bool) -> bool {
        true
    }

    fn request_accessibility() -> Result<bool, String> {
        Ok(true)
    }

    fn resigned() -> bool {
        foreground().is_none_or(|w| w.pid != std::process::id())
    }

    fn raise(id: u64) -> Option<Target> {
        let t = Self::register_source(id).ok()?;
        let w = REGISTRY.lock().unwrap().get(id)?;
        unsafe {
            if IsIconic(w.hwnd()) != 0 {
                ShowWindow(w.hwnd(), SW_RESTORE);
            }
            (SetForegroundWindow(w.hwnd()) != 0).then_some(t)
        }
    }

    fn editor_target_after_denied_raise(id: u64) -> Option<Target> {
        // Windows can deny foreground transfer after an OLE drop even while we own focus.
        // Present our editor for the verified host without activating another application.
        foreground().filter(|w| w.pid == std::process::id())?;
        Self::register_source(id).ok()
    }

    fn activate(pid: i32) {
        if let Some(w) = enumerate().into_iter().find(|w| w.pid == pid as u32) {
            unsafe {
                SetForegroundWindow(w.hwnd());
            }
        }
    }

    fn unwatch(id: u64) {
        REGISTRY.lock().unwrap().unwatch(id)
    }

    async fn sources(owner: &str) -> Result<Option<Vec<SourceChoice>>, String> {
        let owner = owner.to_owned();
        tauri::async_runtime::spawn_blocking(move || {
            let rows: Vec<_> = enumerate()
                .into_iter()
                .map(|w| (w, w.app(), w.title()))
                .collect();
            Some(REGISTRY.lock().unwrap().choices(&owner, rows))
        })
        .await
        .map_err(|e| e.to_string())
    }

    fn clear_sources(owner: &str) {
        REGISTRY.lock().unwrap().clear(owner)
    }

    fn focused_choice() -> Result<Option<u64>, String> {
        Ok(foreground().filter(|w| w.external()).map(transient))
    }

    fn source_at_drop() -> Result<DropTarget, String> {
        let mut point = POINT::default();
        unsafe { GetCursorPos(&mut point) };
        let w = inspect(unsafe { GetAncestor(WindowFromPoint(point), GA_ROOT) });
        Ok(DropTarget {
            outside: w.is_none_or(|w| w.pid != std::process::id()),
            choice: w.filter(|w| w.external()).map(transient),
        })
    }

    fn watch(callback: extern "C" fn(u64, bool)) {
        unsafe extern "system" fn event(
            _: HWINEVENTHOOK,
            kind: u32,
            hwnd: HWND,
            object: i32,
            child: i32,
            _: u32,
            _: u32,
        ) {
            if hwnd.is_null() || object != 0 || child != 0 {
                return;
            }
            if kind == EVENT_SYSTEM_MOVESIZEEND {
                if inspect(hwnd).is_some_and(|w| w.pid == std::process::id()) {
                    let handle = hwnd as usize;
                    tauri::async_runtime::spawn(async move {
                        let _ = crate::desktop::app()
                            .run_on_main_thread(move || save_quick_frame(handle));
                    });
                }
                return;
            }
            let mut closed = 0;
            if kind == EVENT_OBJECT_DESTROY {
                let Some(id) = REGISTRY
                    .lock()
                    .unwrap()
                    .destroy(|w| w.handle == hwnd as usize)
                else {
                    return;
                };
                closed = id;
            }
            if let Some(&notify) = NOTIFY.get() {
                tauri::async_runtime::spawn(async move {
                    let _ = crate::desktop::app().run_on_main_thread(move || notify(closed, false));
                });
            }
        }
        let _ = NOTIFY.set(callback);
        for kind in [
            EVENT_SYSTEM_FOREGROUND,
            EVENT_OBJECT_DESTROY,
            EVENT_SYSTEM_MOVESIZEEND,
        ] {
            unsafe {
                SetWinEventHook(
                    kind,
                    kind,
                    ptr::null_mut(),
                    Some(event),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT,
                );
            }
        }
    }

    fn show_passive(window: &WebviewWindow) -> tauri::Result<()> {
        unsafe {
            ShowWindow(window.hwnd()?.0 as HWND, SW_SHOWNOACTIVATE);
        }
        Ok(())
    }
}
