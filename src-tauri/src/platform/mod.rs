//! Compile-time selection of the desktop adapter. All three targets implement the same contracts.
mod contract;
pub(crate) use contract::{FileManager, HostBackend};

#[cfg_attr(target_os = "macos", path = "macos/mod.rs")]
#[cfg_attr(windows, path = "windows/mod.rs")]
#[cfg_attr(target_os = "linux", path = "linux/mod.rs")]
mod native;
pub(crate) use native::*;

#[cfg(not(target_os = "macos"))]
mod shared;
#[cfg(not(target_os = "macos"))]
pub(crate) use shared::{
    builder, dialogs, filing, geometry, shortcuts, startup_files, system_font_size, window,
};

#[cfg(unix)]
mod unix_storage;

pub const MACOS: bool = cfg!(target_os = "macos");
pub(crate) fn host_windows() -> bool {
    Native::supported()
}

#[cfg(test)]
pub mod test_links;
