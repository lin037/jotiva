use std::ffi::{CStr, c_char};

use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
#[repr(C)]
#[cfg(target_os = "macos")]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Target {
    pub id: u64,
    pub pid: i32,
    pub own_app: bool,
    pub has_window: bool,
    pub fullscreen: bool,
    pub screen: Rect,
    pub(crate) source: [c_char; 256],
}

impl Target {
    #[cfg(not(target_os = "macos"))]
    pub(crate) fn set_source(&mut self, label: &str) {
        self.source.fill(0);
        let end = label.floor_char_boundary(label.len().min(self.source.len() - 1));
        for (dest, byte) in self.source.iter_mut().zip(label.bytes().take(end)) {
            *dest = byte as c_char;
        }
    }

    pub fn source(&self) -> String {
        // The native bridge returns a zero-initialized, NUL-terminated buffer.
        unsafe { CStr::from_ptr(self.source.as_ptr()) }
            .to_string_lossy()
            .into_owned()
    }
}

#[derive(Serialize, Deserialize)]
pub struct SourceChoice {
    pub choice: u64,
    pub app: String,
    pub title: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WindowIdentity {
    pub app: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ambiguous: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<String>,
}

#[derive(Serialize)]
pub struct DropTarget {
    pub(crate) outside: bool,
    pub(crate) choice: Option<u64>,
}

pub(crate) use crate::platform::{pointer, screen_at, visible_screen, window_origin};

#[cfg(not(target_os = "macos"))]
impl Default for Target {
    fn default() -> Self {
        Self {
            id: 0,
            pid: 0,
            own_app: false,
            has_window: false,
            fullscreen: false,
            screen: Rect::default(),
            source: [0; 256],
        }
    }
}
