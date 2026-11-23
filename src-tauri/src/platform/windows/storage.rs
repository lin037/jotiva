pub const TEMP_ROOTS: &[&str] = &[];
use std::{
    fs::{File, OpenOptions},
    io,
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt},
    path::Path,
    ptr,
};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_FLAG_OPEN_REPARSE_POINT, MoveFileW, ReplaceFileW,
};
fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
pub fn open_options(_mode: u32) -> OpenOptions {
    let mut options = OpenOptions::new();
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    options
}
pub fn rename_exclusive(from: &Path, to: &Path) -> io::Result<()> {
    if unsafe { MoveFileW(wide(from).as_ptr(), wide(to).as_ptr()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
pub fn swap(from: &Path, to: &Path) -> io::Result<()> {
    // Keep the displaced revision in the same directory, including on partial Win32 failures.
    let backup = tempfile::Builder::new()
        .prefix(".jot-backup-")
        .tempfile_in(to.parent().ok_or(io::ErrorKind::InvalidInput)?)?
        .into_temp_path()
        .keep()
        .map_err(io::Error::other)?;
    if unsafe {
        ReplaceFileW(
            wide(to).as_ptr(),
            wide(from).as_ptr(),
            wide(&backup).as_ptr(),
            0,
            ptr::null(),
            ptr::null(),
        )
    } == 0
    {
        let error = io::Error::last_os_error();
        // After a partial replacement the backup may be the only copy, even if empty.
        return Err(io::Error::other(format!(
            "替换失败（{error}），备份位置 {}",
            backup.display()
        )));
    }
    rename_exclusive(&backup, from)
        .map_err(|e| io::Error::other(format!("原文件保存在 {}（{e}）", backup.display())))
}
// ReplaceFileW preserves the original file's security descriptor and attributes.
pub fn copy_metadata(_original: &File, _file: &File) -> io::Result<()> {
    Ok(())
}

pub fn reserved_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
        .is_some_and(|n| {
            matches!(
                n,
                "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
            )
        })
}
pub fn invalid_name_char(c: char) -> bool {
    matches!(c, '<' | '>' | '"' | '|' | '?' | '*')
}
