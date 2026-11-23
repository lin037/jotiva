use std::ffi::{CString, c_char};

use global_hotkey::hotkey::{Code, HotKey as Shortcut, Modifiers};
use serde::Deserialize;

use super::Preferences;

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Quick,
    Search,
    Dock,
}

pub struct Bindings(Vec<(Action, Shortcut)>);

#[derive(Deserialize)]
pub struct Command {
    pub id: String,
    pub label: String,
    pub key: Option<String>,
    pub menu: Option<String>,
}

pub fn commands() -> &'static [Command] {
    static COMMANDS: std::sync::OnceLock<Vec<Command>> = std::sync::OnceLock::new();
    COMMANDS.get_or_init(|| {
        [
            include_str!("../../../src/desktop/shortcut-commands.json"),
            include_str!("../../../src/desktop/shortcut-editor.json"),
        ]
        .into_iter()
        .flat_map(|json| {
            serde_json::from_str::<Vec<Command>>(json).expect("invalid shortcut catalog")
        })
        .collect()
    })
}

impl Command {
    pub fn key<'a>(&'a self, preferences: &'a Preferences) -> Option<&'a str> {
        preferences
            .local
            .get(&self.id)
            .unwrap_or(&self.key)
            .as_deref()
    }
}

pub fn validate_local(preferences: &Preferences) -> Result<(), String> {
    use std::collections::HashMap;
    let mut used = HashMap::new();
    for id in preferences.local.keys() {
        if !commands().iter().any(|command| command.id == *id) {
            return Err(format!("未知快捷键命令：{id}"));
        }
    }
    for (label, key) in [
        ("全局速记", Some(&preferences.quick)),
        ("全局查找", preferences.search.as_ref()),
        ("全局靠右", preferences.dock.as_ref()),
    ] {
        if let Some(raw) = key {
            let parsed = raw.parse::<Shortcut>().map_err(|error| error.to_string())?;
            if reserved(parsed) {
                return Err(format!("“{label}”的组合已用于系统或键盘导航，请换一组"));
            }
            used.insert(parsed.id(), ("global", label));
        }
    }
    for command in commands() {
        let Some(raw) = command.key(preferences) else {
            continue;
        };
        let key = raw
            .parse::<Shortcut>()
            .map_err(|_| format!("“{}”的组合键无效", command.label))?;
        if preferences.local.contains_key(&command.id) {
            let function = key
                .key
                .to_string()
                .strip_prefix('F')
                .and_then(|n| n.parse::<u8>().ok())
                .is_some_and(|n| (1..=20).contains(&n));
            if !function
                && !key
                    .mods
                    .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER)
            {
                return Err(format!("“{}”需要带修饰键的组合或功能键", command.label));
            }
            if reserved(key) {
                return Err(format!(
                    "“{}”的组合已用于系统或键盘导航，请换一组",
                    command.label
                ));
            }
        }
        if let Some((other_id, other)) =
            used.insert(key.id(), (command.id.as_str(), command.label.as_str()))
        {
            // When a note is attached, Return to Host takes priority over exiting
            // an editor block. In other windows the original editor key still works.
            if other_id == "returnHost"
                && command.id == "editor.exit-block"
                && key.mods == Modifiers::SUPER
                && key.key == Code::Enter
            {
                continue;
            }
            return Err(format!("“{}”与“{other}”使用了相同的快捷键", command.label));
        }
    }
    Ok(())
}

fn reserved(key: Shortcut) -> bool {
    let mods = key.mods;
    (key.key == Code::Space && (mods == Modifiers::CONTROL || mods == Modifiers::SUPER))
        || (mods.contains(Modifiers::CONTROL | Modifiers::ALT) && !mods.contains(Modifiers::SUPER))
        || (key.key == Code::Tab && mods == Modifiers::SUPER)
        || (key.key == Code::F6 && !mods.intersects(Modifiers::CONTROL | Modifiers::SUPER))
        || (key.key == Code::F2 && mods.is_empty())
        || (matches!(
            key.key,
            Code::KeyA | Code::KeyC | Code::KeyV | Code::KeyX | Code::KeyH | Code::KeyM
        ) && mods == Modifiers::SUPER)
        || (key.key == Code::KeyZ
            && (mods == Modifiers::SUPER || mods == (Modifiers::SUPER | Modifiers::SHIFT)))
        || (key.key == Code::KeyH && mods == (Modifiers::SUPER | Modifiers::ALT))
        || (key.key == Code::Backspace && mods == Modifiers::SUPER)
}

const EVENT_HOTKEY_EXISTS: i32 = -9878;

unsafe extern "C" {
    fn jot_record_shortcut(window: *mut std::ffi::c_void, callback: extern "C" fn(*const c_char));
    fn jot_hotkeys_start(callback: extern "C" fn(u32)) -> i32;
    fn jot_hotkey_register(id: u32, key: *const c_char, modifiers: u32) -> i32;
    fn jot_hotkey_unregister(id: u32) -> i32;
}

pub fn record(window: Option<&tauri::WebviewWindow>) -> Result<(), String> {
    let window = window
        .map(|w| w.ns_window())
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or(std::ptr::null_mut());
    unsafe { jot_record_shortcut(window, crate::desktop::recorded_shortcut) };
    Ok(())
}

pub fn watch(callback: extern "C" fn(u32)) -> Result<(), String> {
    let status = unsafe { jot_hotkeys_start(callback) };
    if status == 0 {
        Ok(())
    } else {
        Err(format!("无法监听系统快捷键（macOS {status}）"))
    }
}

fn register(key: Shortcut) -> Result<(), String> {
    let code = CString::new(key.key.to_string()).expect("physical key names have no NUL");
    let mut modifiers = 0;
    for (mask, flag) in [
        (1, Modifiers::CONTROL),
        (2, Modifiers::SHIFT),
        (4, Modifiers::ALT),
        (8, Modifiers::SUPER),
    ] {
        if key.mods.contains(flag) {
            modifiers |= mask;
        }
    }
    let status = unsafe { jot_hotkey_register(key.id(), code.as_ptr(), modifiers) };
    match status {
        0 => Ok(()),
        EVENT_HOTKEY_EXISTS => Err(format!("{key} 已被其他应用独占登记")),
        _ => Err(format!("{key} 注册失败（macOS {status}）")),
    }
}

fn unregister(keys: impl IntoIterator<Item = Shortcut>) -> Result<(), String> {
    for key in keys {
        let status = unsafe { jot_hotkey_unregister(key.id()) };
        if status != 0 {
            return Err(format!("{key} 注销失败（macOS {status}）"));
        }
    }
    Ok(())
}

impl Bindings {
    pub fn parse(preferences: &Preferences) -> Result<Self, String> {
        let mut bindings = Vec::with_capacity(3);
        for (action, value) in [
            (Action::Quick, Some(preferences.quick.as_str())),
            (Action::Search, preferences.search.as_deref()),
            (Action::Dock, preferences.dock.as_deref()),
        ] {
            let Some(value) = value else { continue };
            let key = value
                .parse::<Shortcut>()
                .map_err(|error| error.to_string())?;
            if !key
                .mods
                .intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER)
            {
                return Err("全局快捷键至少需要 Control、Option 或 Command 中的一项".into());
            }
            if (key.key == Code::Space
                && (key.mods == Modifiers::CONTROL || key.mods == Modifiers::SUPER))
                || (key.mods.contains(Modifiers::CONTROL | Modifiers::ALT)
                    && !key.mods.contains(Modifiers::SUPER))
            {
                return Err("请保留 macOS 的输入法、Spotlight 和 VoiceOver 按键".into());
            }
            if bindings.iter().any(|(_, other)| other == &key) {
                return Err("全局快捷键需要使用不同的组合".into());
            }
            bindings.push((action, key));
        }
        Ok(Self(bindings))
    }

    pub fn get(&self, id: u32) -> Option<(Action, Shortcut)> {
        self.0.iter().find(|(_, key)| key.id() == id).copied()
    }

    pub fn install(&self) -> Result<(), String> {
        self.apply(None)
    }

    pub fn replace(&self, previous: &Self) -> Result<(), String> {
        self.apply(Some(previous))
    }

    pub fn remove(&self) -> Result<(), String> {
        unregister(self.0.iter().map(|(_, key)| *key))
    }

    fn apply(&self, previous: Option<&Self>) -> Result<(), String> {
        let added = self
            .0
            .iter()
            .map(|(_, key)| *key)
            .filter(|key| previous.is_none_or(|old| old.get(key.id()).is_none()));
        let mut installed = Vec::new();
        for key in added {
            if let Err(error) = register(key) {
                unregister(installed)
                    .map_err(|restore| format!("{error}。撤回注册失败：{restore}"))?;
                return Err(error);
            }
            installed.push(key);
        }
        if let Some(old) = previous {
            let removed = old
                .0
                .iter()
                .map(|(_, key)| *key)
                .filter(|key| self.get(key.id()).is_none());
            if let Err(error) = unregister(removed) {
                unregister(installed)
                    .map_err(|restore| format!("{error}。撤回注册失败：{restore}"))?;
                return Err(error);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_preferences_and_local_defaults_are_compatible() {
        let mut preferences: Preferences =
            serde_json::from_str(r#"{"quick":"Control+Shift+Space","search":null,"dock":null}"#)
                .unwrap();
        validate_local(&preferences).unwrap();
        assert!(preferences.local.is_empty());
        let open = commands()
            .iter()
            .find(|command| command.menu.as_deref() == Some("jot-open-file"))
            .unwrap();
        assert_eq!(open.key(&preferences), Some("Super+KeyO"));
        preferences
            .local
            .insert(open.id.clone(), Some("Super+Shift+KeyG".into()));
        validate_local(&preferences).unwrap();
        let saved: Preferences =
            serde_json::from_slice(&serde_json::to_vec(&preferences).unwrap()).unwrap();
        assert_eq!(open.key(&saved), Some("Super+Shift+KeyG"));
        preferences.local.insert(open.id.clone(), None);
        assert_eq!(open.key(&preferences), None);
        assert!(validate_local(&preferences).is_ok());
    }

    #[test]
    fn an_older_global_binding_does_not_block_loading_the_library() {
        let path =
            std::env::temp_dir().join(format!("jot-shortcuts-legacy-{}.json", std::process::id()));
        use std::io::Write;
        std::fs::File::create_new(&path)
            .unwrap()
            .write_all(br#"{"quick":"Command+O","search":null,"dock":null}"#)
            .unwrap();
        let loaded = crate::settings::Settings::load(path.clone()).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(loaded.preferences.quick, "Command+O");
        assert!(loaded.preferences.local.is_empty());
        assert!(validate_local(&loaded.preferences).is_err());
    }

    #[test]
    fn local_conflicts_include_menu_commands_global_aliases_and_standard_editing() {
        let mut preferences = Preferences::default();
        for key in [
            "Super+KeyO",
            "Ctrl+Shift+Space",
            "Super+KeyC",
            "Super+KeyZ",
            "F6",
            "Ctrl+Space",
            "KeyA",
        ] {
            preferences.local.insert("save".into(), Some(key.into()));
            assert!(validate_local(&preferences).is_err(), "accepted {key}");
        }
        preferences.local.clear();
        preferences.search = Some("Command+C".into());
        assert!(validate_local(&preferences).is_err());
        preferences.search = None;
        preferences.local.insert("madeUpAction".into(), None);
        assert!(validate_local(&preferences).is_err());
    }

    #[test]
    fn default_registers_only_quick_capture() {
        let bindings = Bindings::parse(&Preferences::default()).unwrap();
        assert_eq!(bindings.0.len(), 1);
        assert!(matches!(
            bindings.get(bindings.0[0].1.id()),
            Some((Action::Quick, _))
        ));
    }

    #[test]
    fn duplicate_aliases_and_system_combinations_are_rejected() {
        let mut preferences = Preferences {
            search: Some("Ctrl+Shift+Space".into()),
            ..Preferences::default()
        };
        assert!(Bindings::parse(&preferences).is_err());
        for key in ["Ctrl+Space", "Super+Space", "Ctrl+Alt+J", "Shift+J", "J"] {
            preferences.search = Some(key.into());
            assert!(Bindings::parse(&preferences).is_err(), "accepted {key}");
        }
        preferences.search = Some("Ctrl+Shift+F8".into());
        assert_eq!(Bindings::parse(&preferences).unwrap().0.len(), 2);
    }
}
