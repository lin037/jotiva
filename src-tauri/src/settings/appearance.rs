use std::{fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::storage::write_json;

#[derive(Clone, Copy, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemeMode {
    pub fn native(self) -> Option<tauri::Theme> {
        match self {
            Self::System => None,
            Self::Light => Some(tauri::Theme::Light),
            Self::Dark => Some(tauri::Theme::Dark),
        }
    }
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WritingFont {
    #[default]
    System,
    Serif,
    Mono,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Editing {
    pub font: WritingFont,
    pub wrap_code: bool,
    pub fold_headings: bool,
    pub toolbar: bool,
    pub heading_scale: Option<[f64; 6]>,
    pub ruled_paper: bool,
}

impl Default for Editing {
    fn default() -> Self {
        Self {
            font: WritingFont::System,
            wrap_code: true,
            fold_headings: true,
            toolbar: true,
            heading_scale: None,
            ruled_paper: false,
        }
    }
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLayout {
    pub sidebar: Option<u16>,
    pub outline: Option<u16>,
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub language: crate::i18n::Language,
    pub base_size: Option<f64>,
    pub mode: ThemeMode,
    pub accent: Option<String>,
    pub editing: Editing,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Stylesheet {
    name: String,
    css: String,
}

#[derive(Clone, Default, Deserialize, Serialize)]
struct SavedAppearance {
    #[serde(flatten)]
    preferences: Preferences,
    stylesheet: Option<Stylesheet>,
    #[serde(default)]
    workspace: WorkspaceLayout,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceView {
    #[serde(flatten)]
    saved: SavedAppearance,
    pub system_size: f64,
}

impl AppearanceView {
    pub fn language(&self) -> crate::i18n::Language {
        self.saved.preferences.language
    }

    pub fn mode(&self) -> ThemeMode {
        self.saved.preferences.mode
    }

    pub fn has_stylesheet(&self) -> bool {
        self.saved.stylesheet.is_some()
    }
}

pub struct Appearance {
    file: PathBuf,
    saved: SavedAppearance,
    system_size: f64,
}

impl Appearance {
    pub fn load(file: PathBuf) -> io::Result<Self> {
        let saved = match fs::read(&file) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => SavedAppearance::default(),
            Err(error) => return Err(error),
        };
        Ok(Self {
            file,
            saved,
            system_size: crate::platform::system_font_size(),
        })
    }

    pub fn view(&self) -> AppearanceView {
        AppearanceView {
            saved: self.saved.clone(),
            system_size: self.system_size,
        }
    }

    pub fn scale(&self) -> f64 {
        self.saved.preferences.base_size.unwrap_or(self.system_size) / 13.0
    }

    pub fn mode(&self) -> ThemeMode {
        self.saved.preferences.mode
    }

    fn commit(&mut self, saved: SavedAppearance) -> Result<AppearanceView, String> {
        write_json(&self.file, &saved).map_err(|error| error.to_string())?;
        self.saved = saved;
        Ok(self.view())
    }

    pub fn save(&mut self, preferences: Preferences) -> Result<AppearanceView, String> {
        if preferences
            .base_size
            .is_some_and(|size| !(11.0..=18.0).contains(&size))
        {
            return Err("基础字号可设为 11–18 pt".into());
        }
        if preferences
            .editing
            .heading_scale
            .is_some_and(|ratios| ratios.iter().any(|ratio| !(0.8..=2.4).contains(ratio)))
        {
            return Err("标题字号可设为正文的 80%–240%".into());
        }
        if preferences.accent.as_ref().is_some_and(|color| {
            !color.strip_prefix('#').is_some_and(|hex| {
                hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
        }) {
            return Err("请选择六位十六进制强调色".into());
        }
        self.commit(SavedAppearance {
            preferences,
            ..self.saved.clone()
        })
    }

    pub fn save_workspace(&mut self, workspace: WorkspaceLayout) -> Result<(), String> {
        if [workspace.sidebar, workspace.outline]
            .into_iter()
            .flatten()
            .any(|width| !(144..=440).contains(&width))
        {
            return Err("分栏宽度可设为 144–440 pt".into());
        }
        self.commit(SavedAppearance {
            workspace,
            ..self.saved.clone()
        })?;
        Ok(())
    }

    pub fn set_mode(&mut self, mode: ThemeMode) -> Result<AppearanceView, String> {
        let mut preferences = self.saved.preferences.clone();
        preferences.mode = mode;
        self.save(preferences)
    }

    pub fn set_stylesheet(
        &mut self,
        stylesheet: Option<Stylesheet>,
    ) -> Result<AppearanceView, String> {
        if let Some(stylesheet) = &stylesheet
            && (!stylesheet.name.to_ascii_lowercase().ends_with(".css")
                || stylesheet.css.len() > 128 * 1024
                || stylesheet.css.trim().is_empty())
        {
            return Err("请选择不超过 128 KB 的 CSS 文件".into());
        }
        self.commit(SavedAppearance {
            stylesheet,
            ..self.saved.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_stylesheet_persists_and_invalid_imports_leave_it_in_place() {
        let file = std::env::temp_dir().join(format!("jot-stylesheet-{}.json", std::process::id()));
        let mut appearance = Appearance::load(file.clone()).unwrap();
        appearance
            .set_stylesheet(Some(Stylesheet {
                name: "ink-and-steel.css".into(),
                css: ":root { --jot-color-bg: #23262b; }".into(),
            }))
            .unwrap();
        for (name, css) in [
            ("theme.txt", "body {}".into()),
            ("theme.css", "  ".into()),
            ("theme.css", "a".repeat(128 * 1024 + 1)),
        ] {
            assert!(
                appearance
                    .set_stylesheet(Some(Stylesheet {
                        name: name.into(),
                        css,
                    }))
                    .is_err()
            );
        }
        let mut reopened = Appearance::load(file.clone()).unwrap();
        let theme = reopened.view().saved.stylesheet.unwrap();
        assert_eq!(theme.name, "ink-and-steel.css");
        assert_eq!(theme.css, ":root { --jot-color-bg: #23262b; }");
        reopened.set_stylesheet(None).unwrap();
        let cleared = Appearance::load(file.clone()).unwrap().view();
        assert!(!cleared.has_stylesheet());
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn existing_editor_preferences_can_add_heading_scales_and_paper() {
        let file = std::env::temp_dir().join(format!("jot-appearance-{}.json", std::process::id()));
        let saved =
            serde_json::from_str(r#"{"editing":{"toolbar":false},"stylesheet":null}"#).unwrap();
        let mut appearance = Appearance {
            file: file.clone(),
            saved,
            system_size: 13.0,
        };
        let mut preferences = appearance.saved.preferences.clone();
        assert!(matches!(
            preferences.language,
            crate::i18n::Language::Chinese
        ));
        preferences.language = crate::i18n::Language::English;
        assert!(preferences.editing.heading_scale.is_none());
        assert!(!preferences.editing.ruled_paper);
        assert!(!preferences.editing.toolbar);
        assert!(appearance.saved.workspace.sidebar.is_none());
        appearance
            .save_workspace(WorkspaceLayout {
                sidebar: Some(253),
                outline: Some(202),
            })
            .unwrap();
        preferences.editing.ruled_paper = true;
        preferences.editing.heading_scale = Some([1.9, 1.5, 1.3, 1.2, 1.05, 1.0]);
        appearance.save(preferences.clone()).unwrap();
        preferences.editing.heading_scale.as_mut().unwrap()[0] = 2.41;
        assert!(appearance.save(preferences).is_err());
        let persisted: SavedAppearance = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        assert_eq!(persisted.preferences.editing.heading_scale.unwrap()[0], 1.9);
        assert!(matches!(
            persisted.preferences.language,
            crate::i18n::Language::English
        ));
        assert!(persisted.preferences.editing.ruled_paper);
        assert!(!persisted.preferences.editing.toolbar);
        assert_eq!(persisted.workspace.sidebar, Some(253));
        assert_eq!(persisted.workspace.outline, Some(202));
        fs::remove_file(file).unwrap();
    }
}
