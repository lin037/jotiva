use std::{
    fs,
    io::{self, Read},
    path::Path,
};

use super::Stylesheet;
use crate::{
    platform::storage::{invalid_name_char, open_options, rename_exclusive, reserved_name},
    storage::{check_directory, create_file, write_atomic},
};

const CSS_LIMIT: usize = 128 * 1024;
const INVALID_CSS: &str = "请选择不超过 128 KB 的 CSS 文件";
const INVALID_NAME: &str = "请输入有效的主题文件名";
const MISSING_THEME: &str = "找不到已安装的主题";
const NAME_TAKEN: &str = "已安装同名主题，请换个名字";

fn valid_stem(stem: &str) -> bool {
    !stem.is_empty()
        && stem == stem.trim()
        && !stem.starts_with('.')
        && !stem.ends_with('.')
        && !stem.contains(['/', '\\', ':'])
        && !stem.chars().any(|c| c.is_control() || invalid_name_char(c))
        && !reserved_name(stem)
}

fn stem(name: &str) -> Option<&str> {
    let (stem, extension) = name.rsplit_once('.')?;
    (extension.eq_ignore_ascii_case("css") && valid_stem(stem)).then_some(stem)
}

fn filename(title: &str) -> Result<String, String> {
    let title = title.trim();
    let stem = title
        .rsplit_once('.')
        .filter(|(_, extension)| extension.eq_ignore_ascii_case("css"))
        .map_or(title, |(stem, _)| stem);
    if !valid_stem(stem) {
        return Err(INVALID_NAME.into());
    }
    Ok(format!("{stem}.css"))
}

pub(super) fn validate(stylesheet: &Stylesheet) -> Result<(), String> {
    if !stylesheet.name.to_ascii_lowercase().ends_with(".css")
        || stylesheet.css.len() > CSS_LIMIT
        || stylesheet
            .css
            .trim_start_matches('\u{feff}')
            .trim()
            .is_empty()
    {
        return Err(INVALID_CSS.into());
    }
    Ok(())
}

fn read_stylesheet(path: &Path, name: String) -> Result<Stylesheet, String> {
    let file = open_options(0o600)
        .read(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err(INVALID_CSS.into());
    }
    let mut bytes = Vec::new();
    file.take(CSS_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > CSS_LIMIT {
        return Err(INVALID_CSS.into());
    }
    let css = String::from_utf8(bytes).map_err(|_| "主题 CSS 必须使用 UTF-8 编码")?;
    let stylesheet = Stylesheet { name, css };
    validate(&stylesheet)?;
    Ok(stylesheet)
}

pub(super) fn read_source(path: &Path) -> Result<Stylesheet, String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| stem(name).is_some())
        .ok_or(INVALID_NAME)?;
    let resolved = fs::canonicalize(path).map_err(|error| error.to_string())?;
    read_stylesheet(&resolved, name.into())
}

pub(super) fn files(directory: &Path) -> io::Result<Vec<String>> {
    match fs::symlink_metadata(directory) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        result => {
            if !result?.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "无效的主题目录",
                ));
            }
        }
    }
    check_directory(directory)?;
    let mut names = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && let Ok(name) = entry.file_name().into_string()
            && stem(&name).is_some()
        {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

pub(super) fn read(directory: &Path, name: &str) -> Result<String, String> {
    if stem(name).is_none() {
        return Err(MISSING_THEME.into());
    }
    check_directory(directory).map_err(|error| error.to_string())?;
    read_stylesheet(&directory.join(name), name.into()).map(|stylesheet| stylesheet.css)
}

fn conflicting_file<'a>(
    directory: &Path,
    names: &'a [String],
    name: &str,
    except: Option<&str>,
) -> Option<&'a str> {
    let lower = name.to_lowercase();
    let target = directory.join(name);
    let target_exists = target.is_file();
    names
        .iter()
        .map(String::as_str)
        .filter(|file| Some(*file) != except)
        .find(|file| *file == name)
        .or_else(|| {
            names.iter().map(String::as_str).find(|file| {
                Some(*file) != except
                    && (file.to_lowercase() == lower
                        || target_exists
                            && same_file::is_same_file(directory.join(file), &target)
                                .unwrap_or(false))
            })
        })
}

pub(super) enum Installation {
    Installed(String),
    Conflict(String),
}

pub(super) fn install(
    directory: &Path,
    stylesheet: &Stylesheet,
    title: Option<&str>,
    replace: Option<&str>,
) -> Result<Installation, String> {
    validate(stylesheet)?;
    let name = match title {
        Some(title) => filename(title)?,
        None if stem(&stylesheet.name).is_some() => stylesheet.name.clone(),
        None => return Err(INVALID_NAME.into()),
    };
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let names = files(directory).map_err(|error| error.to_string())?;
    let css = stylesheet.css.as_bytes();
    if let Some(existing) = conflicting_file(directory, &names, &name, None) {
        if replace != Some(existing) {
            return Ok(Installation::Conflict(existing.into()));
        }
        write_atomic(&directory.join(existing), css).map_err(|error| error.to_string())?;
        Ok(Installation::Installed(existing.into()))
    } else {
        create_file(&directory.join(&name), css).map_err(|error| error.to_string())?;
        Ok(Installation::Installed(name))
    }
}

pub(super) fn rename(directory: &Path, file: &str, title: &str) -> Result<String, String> {
    let entries = files(directory).map_err(|error| error.to_string())?;
    if !entries.iter().any(|name| name == file) {
        return Err(MISSING_THEME.into());
    }
    let name = if title == file {
        file.into()
    } else {
        filename(title)?
    };
    if file == name {
        return Ok(name);
    }
    if conflicting_file(directory, &entries, &name, Some(file)).is_some() {
        return Err(NAME_TAKEN.into());
    }
    move_file(directory, file, &name).map_err(|error| error.to_string())?;
    // Some filesystems normalize the name when creating the directory entry.
    let entries = files(directory).unwrap_or_default();
    Ok(conflicting_file(directory, &entries, &name, None)
        .unwrap_or(&name)
        .into())
}

pub(super) fn move_file(directory: &Path, from: &str, to: &str) -> io::Result<()> {
    let from = directory.join(from);
    let to = directory.join(to);
    if same_file::is_same_file(&from, &to).unwrap_or(false) {
        // An exclusive rename may reject a casing change on a case-insensitive volume.
        fs::rename(from, to)
    } else {
        rename_exclusive(&from, &to)
    }
}
