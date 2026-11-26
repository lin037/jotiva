use super::{Config, Destination, LIMIT, raster};
use crate::storage;
use std::{
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

pub(super) fn validate_folder(folder: &str) -> Result<(), String> {
    if folder.is_empty()
        || folder.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.contains(['\\', ':'])
                || !matches!(
                    Path::new(part).components().next(),
                    Some(Component::Normal(_))
                )
        })
    {
        return Err("请填写文档目录下的子目录，例如 _assets 或 media/images".into());
    }
    Ok(())
}

pub(super) fn safe_stem(name: &str) -> String {
    let s: String = name
        .chars()
        .take(80)
        .map(|c| {
            if c.is_alphanumeric() || "-_ ".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let s = s.trim();
    if s.is_empty() {
        return "image".into();
    }
    let upper = s.to_ascii_uppercase();
    if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|p| {
            upper
                .strip_prefix(p)
                .is_some_and(|s| s.len() == 1 && s.as_bytes()[0].is_ascii_digit())
        })
    {
        return format!("_{s}");
    }
    s.into()
}

fn subdirectory(root: &Path, parts: &Path) -> io::Result<PathBuf> {
    let mut path = root.to_owned();
    for part in parts.components() {
        let Component::Normal(name) = part else {
            return Err(io::Error::other("无效的图片目录"));
        };
        path.push(name);
        match fs::create_dir(&path) {
            Err(e) if e.kind() != io::ErrorKind::AlreadyExists => return Err(e),
            _ => {}
        }
        if !fs::symlink_metadata(&path)?.is_dir() || !path.canonicalize()?.starts_with(root) {
            return Err(io::Error::other("图片目录包含符号链接"));
        }
    }
    Ok(path)
}

pub(super) fn save(
    doc: &Path,
    config: &Config,
    mode: Destination,
    stem: &str,
    ext: &str,
    bytes: &[u8],
) -> Result<String, String> {
    let doc_dir = doc.parent().ok_or("文档路径无效")?;
    let rule = config.rule_for(doc);
    let template = mode == Destination::Fixed
        && rule.is_some_and(|r| !r.directory.as_os_str().is_empty() && r.directory.is_relative());
    let (base, mut parts) = if mode == Destination::Fixed {
        rule.map(|r| r.fixed_path())
            .transpose()?
            .flatten()
            .unwrap_or_else(|| (config.directory.clone(), PathBuf::new()))
    } else {
        let folder = rule
            .map(|r| r.subfolder.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&config.subfolder);
        validate_folder(folder)?;
        (doc_dir.to_owned(), PathBuf::from(folder))
    };
    if config.group_by_document {
        parts.push(safe_stem(
            doc.file_stem().and_then(|s| s.to_str()).unwrap_or("note"),
        ));
    }
    let root = base.canonicalize().map_err(|e| e.to_string())?;
    if mode == Destination::Fixed && root != base {
        return Err("图片目录已移动或被替换，请重新选择".into());
    }
    let folder = subdirectory(&root, &parts).map_err(|e| e.to_string())?;
    let path = storage::numbered_path(&folder, stem, ext, |p| storage::create_file(p, bytes))
        .map_err(|e| e.to_string())?;
    if mode == Destination::Fixed {
        let url = tauri::Url::from_file_path(path).map_err(|_| "无法生成图片地址")?;
        if template {
            return tauri::Url::from_directory_path(doc_dir)
                .ok()
                .and_then(|doc| doc.make_relative(&url))
                .ok_or_else(|| "无法生成图片地址".into());
        }
        return Ok(url.into());
    }
    // Encode individual segments, keeping directory separators portable in Markdown.
    let relative = path.strip_prefix(&root).map_err(|e| e.to_string())?;
    Ok(relative
        .iter()
        .map(|s| percent_encoding::utf8_percent_encode(&s.to_string_lossy(), SEGMENT).to_string())
        .collect::<Vec<_>>()
        .join("/"))
}

const SEGMENT: &percent_encoding::AsciiSet = &percent_encoding::CONTROLS
    .add(b' ')
    .add(b'%')
    .add(b'#')
    .add(b'?')
    .add(b'(')
    .add(b')')
    .add(b'<')
    .add(b'>')
    .add(b'[')
    .add(b']')
    .add(b'\\')
    .add(b'`')
    .add(b'"');

pub(super) fn read(doc: &Path, dest: &str, roots: &[PathBuf]) -> Option<(&'static str, Vec<u8>)> {
    let parent = doc.parent()?.canonicalize().ok()?;
    let allowed = |path: &Path| {
        #[cfg(windows)]
        let simplify = dunce::simplified;
        #[cfg(not(windows))]
        let simplify = std::convert::identity::<&Path>;
        simplify(path).starts_with(simplify(&parent))
            || roots
                .iter()
                .any(|r| simplify(path).starts_with(simplify(r)))
    };
    let path = if dest.starts_with("file:") {
        tauri::Url::parse(dest).ok()?.to_file_path().ok()?
    } else {
        let decoded = percent_encoding::percent_decode_str(dest)
            .decode_utf8()
            .ok()?;
        let relative = Path::new(decoded.as_ref());
        if relative.components().any(|p| {
            !matches!(
                p,
                Component::Normal(_) | Component::CurDir | Component::ParentDir
            )
        }) {
            return None;
        }
        // The canonical target must still be inside the document or a configured asset directory.
        parent.join(relative)
    };
    if !allowed(&path) {
        return None;
    }
    let path = path.canonicalize().ok()?;
    if !allowed(&path) {
        return None;
    }
    let metadata = fs::metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > LIMIT as u64 {
        return None;
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .ok()?
        .take(LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > LIMIT {
        return None;
    }
    Some((raster(&bytes)?.1, bytes))
}
