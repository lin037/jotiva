use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{LazyLock, RwLock},
};

#[derive(Clone, Copy, Default, Deserialize, Serialize, PartialEq)]
pub enum Language {
    #[default]
    #[serde(rename = "zh-CN")]
    Chinese,
    #[serde(rename = "en")]
    English,
}

static LANGUAGE: RwLock<Language> = RwLock::new(Language::Chinese);
static ENGLISH: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../../src/i18n/en.json")).expect("valid English catalog")
});

pub fn set_language(language: Language) {
    *LANGUAGE.write().unwrap() = language;
}

pub fn language() -> Language {
    *LANGUAGE.read().unwrap()
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
