use serde::{Deserialize, Serialize};

use crate::library::notes::{Card, HostId, NoteId, Page};

pub const QUICK_VIEW: &str = "desktop:quick-view";
pub const MAIN_NOTE: &str = "desktop:main-note";
pub const NOTES_CHANGED: &str = "desktop:notes-changed";
pub const SHORTCUT_RECORDED: &str = "desktop:shortcut-recorded";
pub const FOLDERS_CHANGED: &str = "desktop:folders-changed";
pub const APPEARANCE_CHANGED: &str = "desktop:appearance-changed";
pub const IMAGE_SETTINGS_CHANGED: &str = "desktop:image-settings-changed";
pub const SETTINGS_SECTION: &str = "desktop:settings-section";
pub const SIDEBAR_TOGGLE: &str = "desktop:sidebar-toggle";
pub const EDITOR_ACTION: &str = "desktop:editor-action";

#[derive(Clone, Serialize)]
pub struct Rail {
    pub host: HostId,
    #[serde(flatten)]
    pub page: Page,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Cursor {
    pub anchor: usize,
    pub head: usize,
    pub top: f64,
    pub mode: String,
}

#[derive(Deserialize)]
pub struct QuickBookmark {
    pub id: NoteId,
    pub state: Cursor,
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum View {
    Hidden,
    Summary {
        note: Card,
        docked: bool,
        rail: Option<Rail>,
    },
    Edit {
        id: NoteId,
        cursor: Option<Cursor>,
        pinned: bool,
        standalone: bool,
        source: Option<String>,
    },
    Search {
        tiled: bool,
        host: Option<HostId>,
    },
}

#[derive(Clone, Serialize)]
pub struct QuickView {
    pub epoch: u64,
    #[serde(flatten)]
    pub view: View,
}
