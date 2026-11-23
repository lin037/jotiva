//! Window-manager operations defined by ICCCM and EWMH. Query the client list,
//! since a window manager may reparent clients below its own frame windows.
use crate::{
    host::{DropTarget, SourceChoice, Target},
    platform::{geometry, shared::host_registry::Registry},
};
use std::sync::{LazyLock, Mutex};
use x11rb::{
    atom_manager,
    connection::Connection,
    protocol::{Event, xproto::*},
    rust_connection::RustConnection,
};

atom_manager! {
    pub Atoms: AtomCookies {
        _NET_SUPPORTING_WM_CHECK, _NET_ACTIVE_WINDOW, _NET_CURRENT_DESKTOP,
        _NET_CLIENT_LIST_STACKING, _NET_WM_PID, _NET_WM_NAME,
        _NET_WM_STATE, _NET_WM_STATE_FULLSCREEN,
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
struct Client {
    window: Window,
    pid: u32,
}

pub(super) struct Desktop {
    conn: RustConnection,
    atoms: Atoms,
    root: Window,
    clients: Mutex<Registry<Client>>,
}

pub(super) fn desktop() -> Option<&'static Desktop> {
    static INSTANCE: LazyLock<Option<Desktop>> = LazyLock::new(|| {
        if !super::can_position_windows() {
            return None;
        }
        let (conn, screen) = x11rb::connect(None).ok()?;
        let root = conn.setup().roots[screen].root;
        let atoms = Atoms::new(&conn).ok()?.reply().ok()?;
        let desktop = Desktop {
            conn,
            atoms,
            root,
            clients: Mutex::default(),
        };
        desktop
            .property(root, atoms._NET_SUPPORTING_WM_CHECK)?
            .value32()?
            .next()?;
        desktop.listen(root, EventMask::PROPERTY_CHANGE)?;
        Some(desktop)
    });
    INSTANCE.as_ref()
}

impl Desktop {
    fn property(&self, w: Window, key: Atom) -> Option<GetPropertyReply> {
        self.conn
            .get_property(false, w, key, AtomEnum::ANY, 0, 65536)
            .ok()?
            .reply()
            .ok()
    }
    fn first(&self, w: Window, key: Atom) -> Option<u32> {
        self.property(w, key)?.value32()?.next()
    }
    fn list(&self) -> Vec<Window> {
        self.property(self.root, self.atoms._NET_CLIENT_LIST_STACKING)
            .and_then(|r| Some(r.value32()?.collect()))
            .unwrap_or_default()
    }
    fn active(&self) -> Option<Client> {
        self.client(self.first(self.root, self.atoms._NET_ACTIVE_WINDOW)?)
    }
    fn client(&self, w: Window) -> Option<Client> {
        let pid = self.first(w, self.atoms._NET_WM_PID)?;
        (pid != 0).then_some(Client { window: w, pid })
    }
    fn text(&self, w: Window, key: Atom) -> String {
        self.property(w, key)
            .map(|p| {
                String::from_utf8_lossy(&p.value)
                    .trim_end_matches('\0')
                    .to_owned()
            })
            .unwrap_or_default()
    }
    fn labels(&self, w: Window) -> (String, String) {
        let class = self.text(w, AtomEnum::WM_CLASS.into());
        let app = class.rsplit('\0').next().unwrap_or_default().to_owned();
        let mut title = self.text(w, self.atoms._NET_WM_NAME);
        if title.is_empty() {
            title = self.text(w, AtomEnum::WM_NAME.into());
        }
        (app, title)
    }
    fn listen(&self, w: Window, mask: EventMask) -> Option<()> {
        self.conn
            .change_window_attributes(w, &ChangeWindowAttributesAux::new().event_mask(mask))
            .ok()?
            .check()
            .ok()
    }
    fn observe(&self, w: Window) -> Option<Client> {
        self.listen(w, EventMask::STRUCTURE_NOTIFY)?;
        self.client(w).filter(|c| c.pid != std::process::id())
    }
    fn target(&self, c: Client, id: u64) -> Target {
        let (app, title) = self.labels(c.window);
        let screen = self
            .conn
            .translate_coordinates(c.window, self.root, 0, 0)
            .ok()
            .and_then(|r| r.reply().ok())
            .map(|r| geometry::rect_at_pixel(r.dst_x.into(), r.dst_y.into()))
            .unwrap_or_else(geometry::visible_screen);
        let fullscreen = self
            .property(c.window, self.atoms._NET_WM_STATE)
            .is_some_and(|r| {
                r.value32()
                    .is_some_and(|mut a| a.any(|a| a == self.atoms._NET_WM_STATE_FULLSCREEN))
            });
        let mut target = Target {
            id,
            pid: c.pid as i32,
            own_app: c.pid == std::process::id(),
            has_window: true,
            screen,
            fullscreen,
            ..Default::default()
        };
        target.set_source(&format!("{app} · {title}"));
        target
    }
    pub fn focused(&self, attach: bool) -> Option<Target> {
        let c = self.active()?;
        let observe = attach && self.observe(c.window) == Some(c);
        let mut clients = self.clients.lock().unwrap();
        let id = if observe {
            clients.watch(c)
        } else {
            clients.watched_id(c).unwrap_or(0)
        };
        drop(clients);
        Some(self.target(c, id))
    }
    pub fn choices(&self, owner: &str) -> Vec<SourceChoice> {
        let rows: Vec<_> = self
            .list()
            .into_iter()
            .rev()
            .filter_map(|w| {
                let c = self.observe(w)?;
                let (app, title) = self.labels(w);
                (!title.is_empty()).then_some((c, app, title))
            })
            .collect();
        self.clients.lock().unwrap().choices(owner, rows)
    }
    pub fn clear(&self, owner: &str) {
        self.clients.lock().unwrap().clear(owner);
    }
    pub fn register(&self, id: u64) -> Option<Target> {
        let mut clients = self.clients.lock().unwrap();
        let c = clients.get(id)?;
        if self.observe(c.window) != Some(c) {
            return None;
        }
        clients.watch(c);
        drop(clients);
        Some(self.target(c, id))
    }
    pub fn unwatch(&self, id: u64) {
        self.clients.lock().unwrap().unwatch(id);
    }
    fn activate(&self, w: Window) -> Option<()> {
        let event = ClientMessageEvent::new(32, w, self.atoms._NET_ACTIVE_WINDOW, [2, 0, 0, 0, 0]);
        self.conn
            .send_event(
                false,
                self.root,
                EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                event,
            )
            .ok()?
            .check()
            .ok()
    }
    pub fn raise(&self, id: u64) -> Option<Target> {
        let target = self.register(id)?;
        let c = self.clients.lock().unwrap().get(id)?;
        self.activate(c.window)?;
        Some(target)
    }
    pub fn activate_pid(&self, pid: i32) {
        if let Some(c) = self
            .list()
            .into_iter()
            .rev()
            .filter_map(|w| self.client(w))
            .find(|c| c.pid == pid as u32)
        {
            self.activate(c.window);
        }
    }
    fn transient(&self, c: Client) -> Option<u64> {
        (self.observe(c.window) == Some(c)).then(|| self.clients.lock().unwrap().transient(c))
    }
    pub fn focused_choice(&self) -> Option<u64> {
        self.transient(self.active()?)
    }
    pub fn drop_target(&self) -> DropTarget {
        let windows = self.list();
        let mut hit = self.root;
        let mut client = None;
        while let Some(p) = self
            .conn
            .query_pointer(hit)
            .ok()
            .and_then(|q| q.reply().ok())
        {
            if p.child == 0 || p.child == hit {
                break;
            }
            hit = p.child;
            if windows.contains(&hit) {
                client = self.client(hit);
                break;
            }
        }
        DropTarget {
            outside: client.is_none_or(|c| c.pid != std::process::id()),
            choice: client.and_then(|c| self.transient(c)),
        }
    }
    pub fn watch(&'static self, notify: extern "C" fn(u64, bool)) {
        std::thread::spawn(move || {
            while let Ok(event) = self.conn.wait_for_event() {
                let (closed, space) = match event {
                    Event::PropertyNotify(e)
                        if e.window == self.root && e.atom == self.atoms._NET_ACTIVE_WINDOW =>
                    {
                        (0, false)
                    }
                    Event::PropertyNotify(e)
                        if e.window == self.root && e.atom == self.atoms._NET_CURRENT_DESKTOP =>
                    {
                        (0, true)
                    }
                    Event::DestroyNotify(e) => {
                        let Some(id) = self
                            .clients
                            .lock()
                            .unwrap()
                            .destroy(|c| c.window == e.window)
                        else {
                            continue;
                        };
                        (id, false)
                    }
                    _ => continue,
                };
                let _ = crate::desktop::app().run_on_main_thread(move || notify(closed, space));
            }
        });
    }
}
