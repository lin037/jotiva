mod themes;

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tauri::Manager;

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
pub enum ThemePreset {
    #[default]
    Default,
    Ink,
    Material,
    Blueprint,
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
    pub preview_tabs: bool,
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
            preview_tabs: true,
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
    pub theme_preset: ThemePreset,
    pub theme_file: Option<String>,
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
    theme_css: Option<String>,
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
    theme_css: Option<String>,
}

impl Appearance {
    pub fn load(file: PathBuf) -> io::Result<Self> {
        let mut saved: SavedAppearance = match fs::read(&file) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => SavedAppearance::default(),
            Err(error) => return Err(error),
        };
        let theme_css = saved
            .preferences
            .theme_file
            .as_deref()
            .and_then(|name| themes::read(&file.with_file_name("themes"), name).ok());
        if theme_css.is_none() {
            saved.preferences.theme_file = None;
        }
        Ok(Self {
            file,
            saved,
            system_size: crate::platform::system_font_size(),
            theme_css,
        })
    }

    pub fn view(&self) -> AppearanceView {
        // New WebViews need only the selected theme and must not touch the filesystem.
        AppearanceView {
            saved: self.saved.clone(),
            system_size: self.system_size,
            theme_css: self.theme_css.clone(),
        }
    }

    pub fn scale(&self) -> f64 {
        self.saved.preferences.base_size.unwrap_or(self.system_size) / 13.0
    }

    pub fn mode(&self) -> ThemeMode {
        self.saved.preferences.mode
    }

    pub fn language(&self) -> crate::i18n::Language {
        self.saved.preferences.language
    }

    pub fn has_stylesheet(&self) -> bool {
        self.saved.stylesheet.is_some()
    }

    fn commit(&mut self, saved: SavedAppearance) -> Result<(), String> {
        write_json(&self.file, &saved).map_err(|error| error.to_string())?;
        self.saved = saved;
        Ok(())
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
        let changed = preferences.theme_file != self.saved.preferences.theme_file;
        let css = if changed {
            preferences
                .theme_file
                .as_deref()
                .map(|file| themes::read(&self.theme_directory(), file))
                .transpose()?
        } else {
            None
        };
        self.commit(SavedAppearance {
            preferences,
            ..self.saved.clone()
        })?;
        if changed {
            self.theme_css = css;
        }
        Ok(self.view())
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
        })
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
        if let Some(stylesheet) = &stylesheet {
            themes::validate(stylesheet)?;
        }
        self.commit(SavedAppearance {
            stylesheet,
            ..self.saved.clone()
        })?;
        Ok(self.view())
    }

    fn theme_directory(&self) -> PathBuf {
        self.file.with_file_name("themes")
    }

    pub fn read_theme_source(path: &Path) -> Result<Stylesheet, String> {
        themes::read_source(path)
    }

    pub fn install_theme(
        &mut self,
        stylesheet: Stylesheet,
        title: Option<&str>,
        replace: Option<&str>,
    ) -> Result<(Option<String>, Option<AppearanceView>), String> {
        match themes::install(&self.theme_directory(), &stylesheet, title, replace)? {
            themes::Installation::Conflict(file) => Ok((Some(file), None)),
            themes::Installation::Installed(file) => {
                let view = (self.saved.preferences.theme_file.as_ref() == Some(&file)).then(|| {
                    self.theme_css = Some(stylesheet.css);
                    self.view()
                });
                Ok((None, view))
            }
        }
    }

    pub fn rename_theme(
        &mut self,
        file: &str,
        title: &str,
    ) -> Result<Option<AppearanceView>, String> {
        let directory = self.theme_directory();
        let name = themes::rename(&directory, file, title)?;
        if self.saved.preferences.theme_file.as_deref() != Some(file) || file == name {
            return Ok(None);
        }
        let mut saved = self.saved.clone();
        saved.preferences.theme_file = Some(name.clone());
        self.commit(saved)
            .map_err(|error| match themes::move_file(&directory, &name, file) {
                Ok(()) => error,
                Err(restore) => {
                    format!("主题已改名为 {name}，无法保存选中项：{error}（{restore}）")
                }
            })?;
        Ok(Some(self.view()))
    }
}

#[tauri::command]
pub async fn list_theme_files(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let directory = app
        .state::<std::sync::Mutex<Appearance>>()
        .lock()
        .unwrap()
        .theme_directory();
    tauri::async_runtime::spawn_blocking(move || themes::files(&directory))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn theme_test_directory() -> PathBuf {
        let directory = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("jot-themes-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        directory
    }

    #[test]
    fn theme_presets_round_trip_from_settings() {
        let file = std::env::temp_dir().join(format!("jot-presets-{}.json", std::process::id()));
        let mut appearance = Appearance::load(file.clone()).unwrap();
        for id in ["ink", "material", "blueprint", "default"] {
            let preferences =
                serde_json::from_value(serde_json::json!({ "themePreset": id })).unwrap();
            appearance.save(preferences).unwrap();
            appearance = Appearance::load(file.clone()).unwrap();
            assert_eq!(
                serde_json::to_value(appearance.view()).unwrap()["themePreset"],
                id
            );
        }
        fs::remove_file(file).unwrap();
    }

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
    fn installed_theme_is_independent_of_the_global_stylesheet() {
        let directory = theme_test_directory();
        let source = directory.join("Paper.css");
        let original = "\u{feff}:root { color: red; }";
        fs::write(&source, original).unwrap();
        let config = directory.join("appearance.json");
        let mut appearance = Appearance::load(config.clone()).unwrap();
        let override_css = Stylesheet {
            name: "my-css.css".into(),
            css: ":root { color: blue; }".into(),
        };
        appearance.set_stylesheet(Some(override_css)).unwrap();

        let stylesheet = Appearance::read_theme_source(&source).unwrap();
        assert_eq!(
            appearance.install_theme(stylesheet, None, None).unwrap().0,
            None
        );
        let installed = directory.join("themes/Paper.css");
        assert_eq!(fs::read_to_string(&installed).unwrap(), original);
        let mut preferences = appearance.saved.preferences.clone();
        preferences.theme_preset = ThemePreset::Ink;
        preferences.theme_file = Some("Paper.css".into());
        assert_eq!(
            appearance.save(preferences).unwrap().theme_css.as_deref(),
            Some(original)
        );

        let updated = Stylesheet {
            name: "paper.css".into(),
            css: ":root { color: green; }".into(),
        };
        assert_eq!(
            appearance
                .install_theme(updated.clone(), None, None)
                .unwrap()
                .0
                .as_deref(),
            Some("Paper.css")
        );
        assert_eq!(fs::read_to_string(&installed).unwrap(), original);
        assert_eq!(
            appearance
                .install_theme(updated.clone(), Some("Alternative"), None)
                .unwrap()
                .0,
            None
        );
        let (conflict, refreshed) = appearance
            .install_theme(updated.clone(), None, Some("Paper.css"))
            .unwrap();
        assert!(conflict.is_none());
        assert_eq!(
            refreshed.unwrap().theme_css.as_deref(),
            Some(updated.css.as_str())
        );
        assert_eq!(fs::read_to_string(&source).unwrap(), original);

        let reopened = Appearance::load(config.clone()).unwrap();
        let view = reopened.view();
        assert_eq!(
            themes::files(&reopened.theme_directory()).unwrap(),
            ["Alternative.css", "Paper.css"]
        );
        assert_eq!(view.theme_css.as_deref(), Some(updated.css.as_str()));
        assert_eq!(view.saved.stylesheet.unwrap().name, "my-css.css");

        fs::remove_file(installed).unwrap();
        // Window opens keep using the in-memory CSS even if files are removed.
        assert_eq!(
            reopened.view().theme_css.as_deref(),
            Some(updated.css.as_str())
        );
        let view = Appearance::load(config).unwrap().view();
        assert!(view.saved.preferences.theme_file.is_none());
        assert!(matches!(
            view.saved.preferences.theme_preset,
            ThemePreset::Ink
        ));
        assert!(view.saved.stylesheet.is_some());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn renaming_a_theme_moves_the_file_and_preserves_its_selection() {
        let data = theme_test_directory();
        let config = data.join("appearance.json");
        let mut appearance = Appearance::load(config.clone()).unwrap();
        let css = "main { color: green; }";
        for name in ["Notes.css", "Paper.css"] {
            let stylesheet = Stylesheet {
                name: name.into(),
                css: css.into(),
            };
            appearance.install_theme(stylesheet, None, None).unwrap();
        }
        let mut preferences = appearance.saved.preferences.clone();
        preferences.theme_file = Some("Paper.css".into());
        appearance.save(preferences).unwrap();
        assert!(appearance.rename_theme("Paper.css", "notes").is_err());
        assert!(
            appearance
                .rename_theme("Paper.css", "../Elsewhere")
                .is_err()
        );

        let view = appearance
            .rename_theme("Paper.css", "paper")
            .unwrap()
            .unwrap();
        assert_eq!(
            view.saved.preferences.theme_file.as_deref(),
            Some("paper.css")
        );
        appearance.rename_theme("paper.css", "新的主题").unwrap();
        let directory = appearance.theme_directory();
        assert!(!directory.join("Paper.css").exists());
        assert_eq!(
            fs::read_to_string(directory.join("新的主题.css")).unwrap(),
            css
        );
        let reopened = Appearance::load(config.clone()).unwrap().view();
        assert_eq!(
            reopened.saved.preferences.theme_file.as_deref(),
            Some("新的主题.css")
        );
        assert_eq!(reopened.theme_css.as_deref(), Some(css));

        fs::remove_file(&config).unwrap();
        fs::create_dir(&config).unwrap();
        assert!(appearance.rename_theme("新的主题.css", "Failed").is_err());
        assert!(!directory.join("Failed.css").exists());
        assert_eq!(
            appearance.view().saved.preferences.theme_file.as_deref(),
            Some("新的主题.css")
        );
        fs::remove_dir_all(data).unwrap();
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
            theme_css: None,
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
        assert!(preferences.editing.preview_tabs);
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
