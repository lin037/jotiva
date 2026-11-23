pub const TEMP_ROOTS: &[&str] = &["/tmp", "/var/tmp"];
use std::{fs::OpenOptions, os::unix::fs::OpenOptionsExt};
pub fn open_options(mode: u32) -> OpenOptions {
    let mut options = OpenOptions::new();
    options
        .mode(mode)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    options
}

pub fn reserved_name(_: &str) -> bool {
    false
}
pub fn invalid_name_char(_: char) -> bool {
    false
}
