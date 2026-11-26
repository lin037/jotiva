use super::local;
use crate::settings::filing::http_endpoint;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct FolderRule {
    pub(super) path: PathBuf,
    pub(super) mode: Destination,
    #[serde(default)]
    pub(super) directory: PathBuf,
    #[serde(default)]
    pub(super) subfolder: String,
}

impl FolderRule {
    // Keep the part that already exists separate from the subdirectories created on first use.
    pub(super) fn fixed_path(&self) -> Result<Option<(PathBuf, PathBuf)>, String> {
        if self.mode != Destination::Fixed || self.directory.as_os_str().is_empty() {
            return Ok(None);
        }
        if self.directory.is_absolute() {
            return Ok(Some((self.directory.clone(), PathBuf::new())));
        }
        let suffix = self
            .directory
            .strip_prefix("${ruleDir}")
            .map_err(|_| "请填写绝对路径或以 ${ruleDir} 开头的路径")?;
        let mut base = self.path.clone();
        let mut parts = PathBuf::new();
        for part in suffix.components() {
            match part {
                std::path::Component::Normal(name) => parts.push(name),
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir if parts.pop() || base.pop() => {}
                _ => return Err("图片目录不能超出磁盘根目录".into()),
            }
        }
        Ok(Some((base, parts)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Destination {
    #[default]
    Relative,
    Fixed,
    Http,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    #[serde(alias = "mode")]
    pub(super) default_mode: Destination,
    pub(super) rules: Vec<FolderRule>,
    pub(super) subfolder: String,
    pub(super) directory: PathBuf,
    pub(super) group_by_document: bool,
    pub(super) compress: bool,
    pub(super) endpoint: String,
    pub(super) field: String,
    pub(super) response_pointer: String,
    pub(super) bearer: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_mode: Destination::Relative,
            rules: Vec::new(),
            subfolder: "_assets".into(),
            directory: PathBuf::new(),
            group_by_document: false,
            compress: false,
            endpoint: String::new(),
            field: "file".into(),
            response_pointer: "/url".into(),
            bearer: false,
        }
    }
}

impl Destination {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Relative => "relative",
            Self::Fixed => "fixed",
            Self::Http => "http",
        }
    }
}

impl Config {
    pub(super) fn rule_for(&self, document: &Path) -> Option<&FolderRule> {
        self.rule_at(document.parent()?)
    }

    pub(super) fn rule_at(&self, directory: &Path) -> Option<&FolderRule> {
        self.rules
            .iter()
            .filter(|rule| directory.starts_with(&rule.path))
            .max_by_key(|rule| rule.path.components().count())
    }

    pub(super) fn mode_for(&self, document: &Path) -> Destination {
        self.rule_for(document)
            .map_or(self.default_mode, |rule| rule.mode)
    }

    pub(super) fn validate(&mut self, previous: &Self) -> Result<(), String> {
        local::validate_folder(&self.subfolder)?;
        let mut paths = HashSet::new();
        for rule in &mut self.rules {
            if !previous.rules.iter().any(|old| old.path == rule.path) {
                rule.path = existing_directory(&rule.path)?;
            }
            if !paths.insert(rule.path.clone()) {
                return Err("每个目录只能指定一种图片保存方式".into());
            }
            if rule.mode == Destination::Relative && !rule.subfolder.is_empty() {
                local::validate_folder(&rule.subfolder)?;
            }
            if rule.mode == Destination::Fixed {
                if rule.directory.is_absolute()
                    && !previous
                        .rules
                        .iter()
                        .any(|old| old.path == rule.path && old.directory == rule.directory)
                {
                    rule.directory = existing_directory(&rule.directory)?;
                }
                rule.fixed_path()?;
            }
        }
        let uses = |mode| self.default_mode == mode || self.rules.iter().any(|r| r.mode == mode);
        if self.directory.as_os_str().is_empty() {
            if self.default_mode == Destination::Fixed
                || self
                    .rules
                    .iter()
                    .any(|r| r.mode == Destination::Fixed && r.directory.as_os_str().is_empty())
            {
                return Err("请为固定目录选择图片保存位置".into());
            }
        } else if self.directory != previous.directory {
            self.directory = existing_directory(&self.directory)?;
        }
        if self.endpoint.is_empty() {
            if uses(Destination::Http) {
                return Err("请为 HTTP 上传填写服务地址".into());
            }
        } else {
            self.endpoint = http_endpoint(&self.endpoint)?.into();
            if self.field.is_empty()
                || self.field.len() > 100
                || !self
                    .field
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
            {
                return Err("上传字段名仅支持字母、数字、下划线、点和连字符".into());
            }
            if !self.response_pointer.is_empty() && !self.response_pointer.starts_with('/') {
                return Err("请填写 JSON Pointer，例如 /data/url".into());
            }
        }
        Ok(())
    }
}

pub(super) fn existing_directory(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute() || !path.is_dir() {
        return Err("请选择已有的目录".into());
    }
    path.canonicalize().map_err(|e| e.to_string())
}
