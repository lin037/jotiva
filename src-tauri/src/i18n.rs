use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock},
};

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq)]
pub enum Language {
    #[default]
    #[serde(rename = "zh-CN")]
    Chinese,
    #[serde(rename = "en")]
    English,
}

// Freeze the interface language for this run, including windows created after a preference change.
static LANGUAGE: OnceLock<Language> = OnceLock::new();
static ENGLISH: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../src/i18n/en.json")).expect("valid English catalog")
});

pub fn start(language: Language) {
    let _ = LANGUAGE.set(language);
}

pub fn language() -> Language {
    LANGUAGE.get().copied().unwrap_or_default()
}

pub fn t(source: &str) -> &str {
    if language() == Language::English {
        ENGLISH.get(source).map_or(source, String::as_str)
    } else {
        source
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_menus_and_webviews_share_the_english_catalog() {
        assert_eq!(ENGLISH["文件"], "File");
        assert_eq!(ENGLISH["关闭其他标签"], "Close other tabs");
    }
}
