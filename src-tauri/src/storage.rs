use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::platform::storage::{copy_metadata, open_options, rename_exclusive, swap};
use serde::Serialize;

pub fn system_temporary_roots() -> Vec<PathBuf> {
    std::iter::once(std::env::temp_dir())
        .chain(
            crate::platform::storage::TEMP_ROOTS
                .iter()
                .map(PathBuf::from),
        )
        .filter_map(|root| fs::canonicalize(root).ok())
        .collect()
}

pub fn library_path(path: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(path)?;
    let resolved = fs::canonicalize(path)?;
    if !fs::symlink_metadata(path)?.is_dir()
        || system_temporary_roots()
            .iter()
            .any(|root| resolved.starts_with(root))
    {
        return Err(io::Error::other(format!(
            "文档库不能位于临时目录或符号链接：{}",
            path.display()
        )));
    }
    Ok(resolved)
}

pub fn check_directory(directory: &Path) -> io::Result<()> {
    if !directory.is_dir() || fs::canonicalize(directory)? != directory {
        return Err(io::Error::other(format!(
            "目录已移动或被替换：{}",
            directory.display()
        )));
    }
    Ok(())
}

pub fn lock_library(data: &Path) -> io::Result<File> {
    fs::create_dir_all(data)?;
    let lock = open_options(0o600)
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(data.join(".jot.lock"))?;
    lock.try_lock().map_err(|error| {
        io::Error::other(format!(
            "无法独占文档库，请确认其他 Jotiva 已退出（{error}）"
        ))
    })?;
    Ok(lock)
}

pub fn write_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    write_atomic(path, &serde_json::to_vec(value)?)
}

fn sibling(path: &Path, visible: bool) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let pid = std::process::id();
    let number = NEXT.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(if visible {
        format!("Jotiva 保留的副本-{pid}-{number}.md")
    } else {
        format!(".jot-{pid}-{number}.tmp")
    })
}

fn staged_file(path: &Path, mode: u32) -> io::Result<(PathBuf, File)> {
    loop {
        let temporary = sibling(path, false);
        match open_options(mode)
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}

pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (temporary, mut file) = staged_file(path, 0o600)?;
    if let Err(e) = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| fs::rename(&temporary, path))
    {
        let _ = fs::remove_file(temporary);
        return Err(e);
    }
    Ok(())
}

pub fn create_markdown(path: &Path, contents: &str) -> io::Result<()> {
    if fs::symlink_metadata(path).is_ok() {
        return Err(io::ErrorKind::AlreadyExists.into());
    }
    let (temporary, mut file) = staged_file(path, 0o666)?;
    if let Err(error) = file
        .write_all(contents.as_bytes())
        .and_then(|()| file.sync_all())
        .and_then(|()| rename_exclusive(&temporary, path))
    {
        let _ = fs::remove_file(temporary);
        return Err(error);
    }
    Ok(())
}

pub fn create_named_markdown(
    folder: &Path,
    title: &str,
    contents: &str,
) -> Result<PathBuf, String> {
    named_markdown_path(folder, title, |path| create_markdown(path, contents))
        .map_err(|e| format!("无法创建 Markdown：{e}"))
}

pub fn named_markdown_path(
    folder: &Path,
    title: &str,
    mut create: impl FnMut(&Path) -> io::Result<()>,
) -> io::Result<PathBuf> {
    if Path::new(title).file_name() != Some(title.as_ref()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "无效的 Markdown 文件名",
        ));
    }
    for suffix in 1u64.. {
        let name = if suffix == 1 {
            format!("{title}.md")
        } else {
            format!("{title} ({suffix}).md")
        };
        let path = folder.join(name);
        match create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    unreachable!()
}

pub fn check_markdown(path: &Path, expected: &str) -> io::Result<()> {
    if !fs::symlink_metadata(path)?.is_file() || fs::read_to_string(path)? != expected {
        return Err(io::Error::other("磁盘上的 Markdown 已更改"));
    }
    Ok(())
}

fn move_aside(path: &Path, visible: bool) -> io::Result<PathBuf> {
    loop {
        let reserved = sibling(path, visible);
        match rename_exclusive(path, &reserved) {
            Ok(()) => return Ok(reserved),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
}

/// An external editor may replace the pathname after our initial comparison.
/// Inspect the inode actually removed from the user's pathname before discarding it.
pub fn remove_matching(path: &Path, expected: &str) -> io::Result<()> {
    check_markdown(path, expected)?;
    let held = move_aside(path, false)?;
    finish_removal(path, held, expected)
}

fn finish_removal(path: &Path, held: PathBuf, expected: &str) -> io::Result<()> {
    if check_markdown(&held, expected).is_ok() && fs::remove_file(&held).is_ok() {
        return Ok(());
    }
    if rename_exclusive(&held, path).is_ok() {
        return Err(io::Error::other(format!(
            "原文件已保留在 {}",
            path.display()
        )));
    }
    let copy = move_aside(&held, true).unwrap_or(held);
    Err(io::Error::other(format!(
        "原文件已保留在 {}",
        copy.display()
    )))
}

fn exchange_markdown(temporary: &Path, path: &Path, previous: &str) -> io::Result<()> {
    // A failed replacement can leave the displaced revision at either staging path.
    swap(temporary, path).map_err(|e| {
        io::Error::other(format!(
            "无法替换 Markdown（{e}），暂存路径 {}",
            temporary.display()
        ))
    })?;
    if check_markdown(temporary, previous).is_ok() {
        // A crash here may leave an old revision beside the document.
        let _ = fs::remove_file(temporary);
        return Ok(());
    }
    // Preserve both versions even if a third editor changes the document during recovery.
    let copy = move_aside(temporary, true).unwrap_or_else(|_| temporary.to_owned());
    if swap(&copy, path).is_ok() {
        return Err(io::Error::other(format!(
            "磁盘上的 Markdown 已更改，本次编辑保留在 {}",
            copy.display()
        )));
    }
    Err(io::Error::other(format!(
        "磁盘上的 Markdown 已更改，原文件保留在 {}",
        copy.display()
    )))
}

pub fn replace_markdown(path: &Path, previous: &str, contents: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        check_directory(parent)?;
    }
    let original = open_options(0).read(true).write(true).open(path)?;
    let metadata = original.metadata()?;
    if !metadata.is_file() || metadata.permissions().readonly() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "文件为只读或已被替换",
        ));
    }
    check_markdown(path, previous)?;
    let (temporary, mut file) = staged_file(path, 0o600)?;
    let prepare = (|| -> io::Result<()> {
        file.write_all(contents.as_bytes())?;
        file.set_permissions(metadata.permissions())?;
        copy_metadata(&original, &file)?;
        file.sync_all()?;
        check_markdown(path, previous)
    })();
    if let Err(error) = prepare {
        let _ = fs::remove_file(temporary);
        return Err(error);
    }
    drop(original);
    drop(file);
    exchange_markdown(&temporary, path, previous)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_external_replacement_is_restored_and_local_edit_stays_readable() {
        let root = std::env::temp_dir().join(format!("jot-late-swap-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let target = root.join("笔记.md");
        let staged = root.join(".pending.tmp");
        fs::write(&staged, "本次编辑").unwrap();
        fs::write(&target, "旧版").unwrap();
        check_markdown(&target, "旧版").unwrap();
        // A third-party writer replaces the document after the final preflight.
        fs::write(root.join("外部暂存"), "外部编辑").unwrap();
        fs::rename(root.join("外部暂存"), &target).unwrap();
        assert!(exchange_markdown(&staged, &target, "旧版").is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "外部编辑");
        let copies: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p != &target)
            .collect();
        assert_eq!(copies.len(), 1);
        assert_eq!(copies[0].extension().unwrap(), "md");
        assert_eq!(fs::read_to_string(&copies[0]).unwrap(), "本次编辑");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_redirected_data_root_in_system_temp_is_not_a_library() {
        let root = std::env::temp_dir().join(format!("jot-temporary-data-{}", std::process::id()));
        fs::create_dir_all(root.join("target")).unwrap();
        crate::platform::test_links::directory(root.join("target"), root.join("redirect")).unwrap();
        assert!(library_path(&root.join("target")).is_err());
        assert!(library_path(&root.join("redirect")).is_err());
        assert_eq!(fs::read_dir(root.join("target")).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn two_instances_cannot_share_the_writable_library() {
        let root = std::env::temp_dir().join(format!("jot-writer-lock-{}", std::process::id()));
        let first = lock_library(&root).unwrap();
        assert!(lock_library(&root).is_err());
        drop(first);
        drop(lock_library(&root).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_edit_during_archive_cleanup_survives_even_if_the_source_is_recreated() {
        let root = std::env::temp_dir().join(format!("jot-late-cleanup-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let source = root.join("来源.md");
        fs::write(&source, "已归档").unwrap();
        check_markdown(&source, "已归档").unwrap();
        fs::write(&source, "归档期间被外部修改").unwrap();
        let held = move_aside(&source, false).unwrap();
        fs::write(&source, "同一路径的新文档").unwrap();
        assert!(finish_removal(&source, held, "已归档").is_err());
        assert_eq!(fs::read_to_string(&source).unwrap(), "同一路径的新文档");
        let kept = fs::read_dir(&root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p != &source)
            .unwrap();
        assert!(
            kept.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("Jotiva 保留的副本")
        );
        assert_eq!(fs::read_to_string(kept).unwrap(), "归档期间被外部修改");
        fs::remove_dir_all(root).unwrap();
    }
}
