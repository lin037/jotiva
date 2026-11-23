use crate::platform::{FileManager, Native};
use std::{
    ffi::{CStr, CString, c_char, c_void},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr,
};
unsafe extern "C" {
    fn jot_manage_path(
        action: i32,
        source: *const c_char,
        target: *const c_char,
        trashed: *mut *mut c_char,
    ) -> *mut c_char;
    fn jot_reveal_path(path: *const c_char);
    fn free(pointer: *mut c_void);
}

fn native_path(
    action: i32,
    source: &Path,
    target: Option<&Path>,
) -> Result<Option<PathBuf>, String> {
    let source = CString::new(source.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    let target = target
        .map(|path| CString::new(path.as_os_str().as_bytes()))
        .transpose()
        .map_err(|e| e.to_string())?;
    let mut trashed = ptr::null_mut();
    let error = unsafe {
        jot_manage_path(
            action,
            source.as_ptr(),
            target.as_ref().map_or(ptr::null(), |p| p.as_ptr()),
            &mut trashed,
        )
    };
    if !error.is_null() {
        let message = unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned();
        unsafe {
            free(error.cast());
        }
        return Err(message);
    }
    if trashed.is_null() {
        return Ok(None);
    }
    let path = PathBuf::from(std::ffi::OsStr::from_bytes(
        unsafe { CStr::from_ptr(trashed) }.to_bytes(),
    ));
    unsafe {
        free(trashed.cast());
    }
    Ok(Some(path))
}

impl FileManager for Native {
    type TrashRecord = PathBuf;
    fn reveal(path: &Path) -> Result<(), String> {
        let path = CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        unsafe {
            jot_reveal_path(path.as_ptr());
        }
        Ok(())
    }

    fn copy(from: &Path, to: &Path) -> Result<(), String> {
        native_path(1, from, Some(to)).map(drop)
    }

    fn move_to(from: &Path, to: &Path) -> Result<(), String> {
        native_path(0, from, Some(to)).map(drop)
    }

    fn trash(from: &Path) -> Result<PathBuf, String> {
        native_path(2, from, None)?.ok_or("系统未返回废纸篓位置".into())
    }

    fn restore(saved: PathBuf, to: &Path) -> Result<(), String> {
        Self::move_to(&saved, to).map_err(|e| format!("{}（{e}）", saved.display()))
    }
}
