pub use crate::platform::unix_storage::{
    TEMP_ROOTS, invalid_name_char, open_options, reserved_name,
};
use std::{
    ffi::CString,
    fs::File,
    io,
    os::{
        fd::AsRawFd,
        unix::{ffi::OsStrExt, fs::MetadataExt},
    },
    path::Path,
};
fn rename(from: &Path, to: &Path, flags: u32) -> io::Result<()> {
    let from = CString::new(from.as_os_str().as_bytes())?;
    let to = CString::new(to.as_os_str().as_bytes())?;
    if unsafe {
        libc::renameat2(
            libc::AT_FDCWD,
            from.as_ptr(),
            libc::AT_FDCWD,
            to.as_ptr(),
            flags,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
pub fn rename_exclusive(from: &Path, to: &Path) -> io::Result<()> {
    rename(from, to, libc::RENAME_NOREPLACE)
}
pub fn swap(from: &Path, to: &Path) -> io::Result<()> {
    rename(from, to, libc::RENAME_EXCHANGE)
}
// A new file may inherit a different group and ACL from its parent directory.
pub fn copy_metadata(original: &File, staged: &File) -> io::Result<()> {
    let source = original.metadata()?;
    let dest = staged.metadata()?;
    if (source.uid(), source.gid()) != (dest.uid(), dest.gid()) {
        if unsafe { libc::fchown(staged.as_raw_fd(), source.uid(), source.gid()) } != 0 {
            return Err(io::Error::last_os_error());
        }
        staged.set_permissions(source.permissions())?;
    }
    let mut names = vec![0_u8; 65536]; // Linux bounds both name lists and values to 64 KiB.
    let count =
        unsafe { libc::flistxattr(original.as_raw_fd(), names.as_mut_ptr().cast(), names.len()) };
    if count < 0 {
        let error = io::Error::last_os_error();
        return if error.raw_os_error() == Some(libc::ENOTSUP) {
            Ok(())
        } else {
            Err(error)
        };
    }
    let acl = c"system.posix_acl_access";
    let mut has_acl = false;
    let mut value = vec![0_u8; 65536];
    for name in names[..count as usize].split_inclusive(|&byte| byte == 0) {
        let name = std::ffi::CStr::from_bytes_with_nul(name).map_err(io::Error::other)?;
        has_acl |= name == acl;
        if name != acl && !name.to_bytes().starts_with(b"user.") {
            continue;
        }
        let size = unsafe {
            libc::fgetxattr(
                original.as_raw_fd(),
                name.as_ptr(),
                value.as_mut_ptr().cast(),
                value.len(),
            )
        };
        if size < 0
            || unsafe {
                libc::fsetxattr(
                    staged.as_raw_fd(),
                    name.as_ptr(),
                    value.as_ptr().cast(),
                    size as usize,
                    0,
                )
            } != 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    if !has_acl && unsafe { libc::fremovexattr(staged.as_raw_fd(), acl.as_ptr()) } != 0 {
        let error = io::Error::last_os_error();
        if !matches!(error.raw_os_error(), Some(libc::ENODATA | libc::ENOTSUP)) {
            return Err(error);
        }
    }
    Ok(())
}
