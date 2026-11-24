use std::{os::windows::ffi::OsStrExt, path::Path, ptr};
use windows_sys::Win32::{
    System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize},
    UI::Shell::{SHOpenFolderAndSelectItems, SHParseDisplayName},
};

/// Select the actual shell item in Explorer, including a directory selected in its parent.
pub(crate) fn reveal_in_file_manager(path: &Path) -> Result<(), String> {
    let path = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    let name: Vec<_> = dunce::simplified(&path)
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    std::thread::spawn(move || unsafe {
        let initialized = CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32);
        if initialized < 0 {
            return Err(format!("无法初始化资源管理器（{initialized:#x}）"));
        }
        let mut item = ptr::null_mut();
        let parsed = SHParseDisplayName(
            name.as_ptr(),
            ptr::null_mut(),
            &mut item,
            0,
            ptr::null_mut(),
        );
        let status = if parsed >= 0 {
            let status = SHOpenFolderAndSelectItems(item, 0, ptr::null(), 0);
            CoTaskMemFree(item.cast());
            status
        } else {
            parsed
        };
        CoUninitialize();
        if status < 0 {
            Err(format!("资源管理器无法定位此项（{status:#x}）"))
        } else {
            Ok(())
        }
    })
    .join()
    .map_err(|_| "资源管理器调用中断".to_owned())?
}
