pub use crate::platform::unix_storage::{
    TEMP_ROOTS, invalid_name_char, open_options, reserved_name,
};
use std::os::fd::AsRawFd;
use std::{ffi::CString, fs::File, io, os::unix::ffi::OsStrExt, path::Path};
fn rename_with(from: &Path, to: &Path, flags: libc::c_uint) -> io::Result<()> {
    let from = CString::new(from.as_os_str().as_bytes())?;
    let to = CString::new(to.as_os_str().as_bytes())?;
    // Both C paths remain valid for this synchronous Darwin filesystem operation.
    if unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), flags) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub fn rename_exclusive(from: &Path, to: &Path) -> io::Result<()> {
    rename_with(from, to, libc::RENAME_EXCL)
}
pub fn swap(from: &Path, to: &Path) -> io::Result<()> {
    rename_with(from, to, libc::RENAME_SWAP)
}
pub fn copy_metadata(original: &File, file: &File) -> io::Result<()> {
    // Preserve Finder tags and access-control entries on the replacement.
    let flags = libc::COPYFILE_ACL | libc::COPYFILE_XATTR;
    if unsafe {
        libc::fcopyfile(
            original.as_raw_fd(),
            file.as_raw_fd(),
            std::ptr::null_mut(),
            flags,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
