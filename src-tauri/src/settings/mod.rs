pub mod appearance;
pub mod filing;
pub mod shortcuts;

use std::{collections::BTreeMap, fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::storage::write_json;
use shortcuts::Bindings;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preferences {
    pub quick: String,
    pub search: Option<String>,
    pub dock: Option<String>,
    #[serde(default)]
    pub local: BTreeMap<String, Option<String>>,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            quick: "Control+Shift+Space".into(),
            search: None,
            dock: None,
            local: BTreeMap::new(),
        }
    }
}

pub struct Settings {
    file: PathBuf,
    pub preferences: Preferences,
    pub bindings: Bindings,
    pub shortcut_error: Option<String>,
    pub recording: bool,
}

impl Settings {
    pub fn load(file: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let preferences = match fs::read(&file) {
            Ok(content) => serde_json::from_slice(&content)?,
            Err(err) if err.kind() == io::ErrorKind::NotFound => Preferences::default(),
            Err(err) => return Err(err.into()),
        };
        let bindings = Bindings::parse(&preferences)?;
        Ok(Self {
            file,
            preferences,
            bindings,
            shortcut_error: None,
            recording: false,
        })
    }

    pub fn install(&mut self) {
        self.shortcut_error = self.bindings.install().err();
    }

    pub fn update(&mut self, preferences: Preferences) -> Result<(), String> {
        let next = Bindings::parse(&preferences)?;
        let previous_installed = self.shortcut_error.is_none();
        if previous_installed {
            next.replace(&self.bindings)?;
        } else {
            next.install()?;
        }
        if let Err(error) = write_json(&self.file, &preferences) {
            let restore = if previous_installed {
                self.bindings.replace(&next)
            } else {
                next.remove()
            };
            restore.map_err(|restore| format!("{error}。恢复快捷键失败：{restore}"))?;
            return Err(error.to_string());
        }
        self.preferences = preferences;
        self.bindings = next;
        self.shortcut_error = None;
        Ok(())
    }
}
