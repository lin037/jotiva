use crate::platform::{FileManager, Native};
use std::{collections::HashSet, ffi::OsString, fs, io, path::Path};
use trash::os_limited;
fn copy_entry(from: &Path, to: &Path) -> io::Result<()> {
    let meta = fs::symlink_metadata(from)?;
    if meta.is_dir() {
        fs::create_dir(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_entry(&entry.path(), &to.join(entry.file_name()))?;
        }
    } else if meta.is_file() {
        let mut dest = fs::File::create_new(to)?;
        io::copy(&mut fs::File::open(from)?, &mut dest)?;
        dest.sync_all()?;
    } else {
        return Err(io::Error::other("只能复制普通文件和目录"));
    }
    fs::set_permissions(to, meta.permissions())
}
// Remember existing recycle-bin identities so rollback cannot restore an older namesake.

impl FileManager for Native {
    type TrashRecord = HashSet<OsString>;
    fn reveal(path: &Path) -> Result<(), String> {
        crate::platform::reveal_in_file_manager(path)
    }
    fn move_to(from: &Path, to: &Path) -> Result<(), String> {
        crate::platform::storage::rename_exclusive(from, to).map_err(|e| e.to_string())
    }

    fn copy(from: &Path, to: &Path) -> Result<(), String> {
        let parent = to.parent().ok_or("目标没有上级目录")?;
        let temp = tempfile::Builder::new()
            .prefix(".jot-copy-")
            .tempdir_in(parent)
            .map_err(|e| e.to_string())?;
        let stage = temp.path().join("copy");
        copy_entry(from, &stage).map_err(|e| e.to_string())?;
        Self::move_to(&stage, to)
    }

    fn trash(from: &Path) -> Result<HashSet<OsString>, String> {
        let existing = os_limited::list()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|item| item.id)
            .collect();
        trash::delete(from).map_err(|e| e.to_string())?;
        Ok(existing)
    }

    fn restore(existing: HashSet<OsString>, to: &Path) -> Result<(), String> {
        let items = os_limited::list().map_err(|e| e.to_string())?;
        let mut matches = items.into_iter().filter(|item| {
            dunce::simplified(&item.original_path()) == dunce::simplified(to)
                && !existing.contains(&item.id)
        });
        let item = matches.next().ok_or("请在系统回收站中恢复文件")?;
        if matches.next().is_some() || to.exists() {
            return Err("原路径或回收站中存在同名文件，请手动还原".into());
        }
        os_limited::restore_all([item]).map_err(|e| e.to_string())
    }
}
