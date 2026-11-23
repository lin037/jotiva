use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::storage::{
    check_directory, check_markdown, remove_matching, replace_markdown, write_json,
};

use super::folders::{Folder, FolderId};
use crate::host::WindowIdentity;

pub type NoteId = u64;
pub type HostId = u64;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: NoteId,
    pub source: String,
    pub markdown: String,
    pub version: u64,
    updated: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    // Older drafts have only the display source and belong to 「其他」.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    origin: Option<NoteId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<NoteLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
}

impl Note {
    pub fn disk_path(&self) -> Option<&Path> {
        self.file
            .as_deref()
            .or_else(|| self.location.as_ref().map(|value| value.path.as_path()))
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct NoteLocation {
    pub folder: Option<FolderId>,
    pub path: PathBuf,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: NoteId,
    pub version: u64,
    pub title: String,
    pub excerpt: String,
    pub source: String,
    pub attached: bool,
    pub folder: Option<FolderId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archive_path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
}

#[derive(Clone, Serialize)]
pub struct Page {
    pub items: Vec<Card>,
    pub more: bool,
}

#[derive(Default, Serialize)]
pub struct SearchPage {
    current: Vec<Card>,
    attached: Vec<Card>,
    documents: Vec<Card>,
    more: bool,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Scope {
    Folder { id: Option<FolderId> },
    Source { key: String },
    Host { id: HostId },
}

#[derive(Serialize)]
pub struct SourceGroup {
    app: String,
    windows: Vec<SourceWindow>,
}

#[derive(Serialize)]
pub struct SourceWindow {
    key: String,
    title: String,
    attached: bool,
}

#[derive(Serialize)]
pub struct MoveResult {
    pub path: Option<PathBuf>,
    pub warning: Option<String>,
}

struct Entry {
    note: Note,
    host: Option<HostId>,
    card: Card,
    searchable: String,
    source_key: String,
}

impl Entry {
    fn new(mut note: Note, host: Option<HostId>) -> Self {
        let mut lines = note
            .markdown
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty());
        let opening = lines.next().unwrap_or("");
        let first = opening.trim_start_matches('#').trim();
        if note.file.is_none() && note.name.is_none() {
            note.name = note
                .location
                .as_ref()
                .filter(|location| location.folder.is_some())
                .and_then(|value| value.path.file_stem())
                .and_then(|name| name.to_str())
                .map(str::to_owned)
                .or_else(|| note_name(first));
        }
        let title = note
            .file
            .as_ref()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .or(note.name.as_deref())
            .or_else(|| note.location.as_ref()?.path.file_stem()?.to_str())
            .unwrap_or("新便签");
        let stem = note
            .disk_path()
            .and_then(Path::file_stem)
            .and_then(|s| s.to_str())
            .unwrap_or(title);
        let mut preview = std::iter::once(opening)
            .filter(|_| first != stem)
            .chain(lines);
        let mut excerpt: String = preview.next().unwrap_or("").chars().take(120).collect();
        let remaining = 120 - excerpt.chars().count();
        if remaining > 1
            && let Some(second) = preview.next()
        {
            excerpt.push('\n');
            excerpt.extend(second.chars().take(remaining - 1));
        }
        let card = Card {
            id: note.id,
            version: note.version,
            title: title.chars().take(100).collect(),
            excerpt,
            source: note.source.clone(),
            attached: host.is_some(),
            file: note.file.clone(),
            archive_path: note.location.as_ref().map(|location| location.path.clone()),
            folder: note.location.as_ref().and_then(|location| location.folder),
        };
        let searchable = format!("{} {} {}", card.title, note.source, note.markdown).to_lowercase();
        let source_key = note.origin.map_or_else(
            || format!("label/{}", note.source),
            |id| format!("window/{id}"),
        );
        Self {
            note,
            host,
            card,
            searchable,
            source_key,
        }
    }

    fn matches(&self, scope: Option<&Scope>) -> bool {
        match scope {
            None => true,
            Some(Scope::Folder { id }) => self.note.file.is_none() && self.card.folder == *id,
            Some(Scope::Source { key }) => self.source_key == *key,
            Some(Scope::Host { id }) => self.host == Some(*id),
        }
    }
}

pub struct Notes {
    folder: PathBuf,
    sources_file: PathBuf,
    saved_sources: BTreeMap<NoteId, String>,
    bindings: BTreeMap<NoteId, WindowIdentity>,
    source_hosts: HashMap<HostId, NoteId>,
    records: HashMap<NoteId, Entry>,
    by_path: BTreeMap<PathBuf, NoteId>,
    recent: BTreeSet<(u64, NoteId)>,
    by_host: HashMap<HostId, BTreeSet<(u64, NoteId)>>,
    by_origin: HashMap<NoteId, BTreeSet<NoteId>>,
    next_id: NoteId,
}

fn same_regular_file(left: &Path, right: &Path) -> io::Result<bool> {
    for path in [left, right] {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => return Ok(false),
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e),
        }
    }
    same_file::is_same_file(left, right)
}

impl Notes {
    pub fn load(folder: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&folder)?;
        if !fs::symlink_metadata(&folder)?.is_dir() {
            return Err(io::Error::other("便签存储目录已被替换"));
        }
        let folder = fs::canonicalize(folder)?;
        fs::create_dir_all(folder.join("其他"))?;
        check_directory(&folder.join("其他"))?;
        let sources_file = folder.join("sources.json");
        let saved_sources: BTreeMap<NoteId, String> = match fs::read(&sources_file) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(error),
        };
        let next_id = saved_sources.last_key_value().map_or(1, |(id, _)| id + 1);
        let binding_file = folder.join("window-bindings.json");
        let bindings = match fs::read(&binding_file) {
            Ok(bytes) => serde_json::from_slice(&bytes)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(error),
        };
        let mut notes = Self {
            folder,
            sources_file,
            saved_sources,
            bindings,
            source_hosts: HashMap::new(),
            records: HashMap::new(),
            by_path: BTreeMap::new(),
            recent: BTreeSet::new(),
            by_host: HashMap::new(),
            by_origin: HashMap::new(),
            next_id,
        };
        let mut stages = Vec::new();
        for item in fs::read_dir(&notes.folder)? {
            let path = item?.path();
            let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            if let Some(id) = name
                .strip_prefix('.')
                .and_then(|s| s.strip_suffix(".inbox"))
                .and_then(|s| s.parse::<NoteId>().ok())
            {
                if name == format!(".{id}.inbox") {
                    stages.push((id, path));
                }
                continue;
            }
            let Some(id) = name
                .strip_suffix(".json")
                .and_then(|s| s.parse::<NoteId>().ok())
            else {
                continue;
            };
            let Some(next_id) = id.checked_add(1) else {
                continue;
            };
            notes.next_id = notes.next_id.max(next_id);
            let record = fs::symlink_metadata(&path).and_then(|entry| {
                if !entry.is_file() {
                    return Err(io::Error::other("记录不是普通文件"));
                }
                Ok(serde_json::from_slice::<Note>(&fs::read(&path)?)?)
            });
            let mut note = match record {
                Ok(note) if id == note.id && name == format!("{id}.json") => note,
                _ => {
                    // Reserve the id so a later capture cannot overwrite the damaged snapshot.
                    eprintln!("无法读取便签记录 {}，已保留原文件", path.display());
                    continue;
                }
            };
            if note.version > 0 && note.disk_path().is_none() {
                notes.publish_inbox(&mut note)?;
            }
            notes.next_id = notes.next_id.max(note.id.max(note.origin.unwrap_or(0)) + 1);
            notes.insert(note, None);
        }
        // A crash after committing metadata may leave our staging hard link.
        for (id, stage) in stages {
            if let Some(path) = notes.records.get(&id).and_then(|e| e.note.disk_path())
                && same_regular_file(path, &stage)?
            {
                fs::remove_file(stage)?;
            }
        }
        // A close can span several note records. Its marker is authoritative until
        // every index and note was written, including when Jotiva stopped mid-close.
        let closures = notes.folder.join("closing-sources");
        if closures.is_dir() {
            for item in fs::read_dir(&closures)? {
                let path = item?.path();
                if let Some(origin) = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.parse().ok())
                {
                    notes.complete_source_close(origin, &path);
                    notes.next_id = notes.next_id.max(origin + 1);
                }
            }
        }
        let origins = notes.origins();
        notes.bindings.retain(|origin, _| origins.contains(origin));
        Ok(notes)
    }

    fn insert(&mut self, note: Note, host: Option<HostId>) {
        let key = (note.updated, note.id);
        self.recent.insert(key);
        if let Some(host) = host {
            self.by_host.entry(host).or_default().insert(key);
            if let Some(origin) = note.origin {
                self.source_hosts.insert(host, origin);
            }
        }
        let old_origin = self
            .records
            .get(&note.id)
            .and_then(|entry| entry.note.origin);
        if old_origin != note.origin {
            if let Some(origin) = old_origin {
                self.remove_origin_member(origin, note.id);
            }
            if let Some(origin) = note.origin {
                self.by_origin.entry(origin).or_default().insert(note.id);
            }
        }
        let previous_path = self
            .records
            .get(&note.id)
            .and_then(|entry| entry.note.disk_path());
        if previous_path != note.disk_path() {
            if let Some(previous) = previous_path {
                self.by_path.remove(previous);
            }
            if let Some(path) = note.disk_path() {
                self.by_path.insert(path.to_owned(), note.id);
            }
        }
        self.records.insert(note.id, Entry::new(note, host));
    }

    pub fn new_note(&mut self, host: Option<HostId>, source: String) -> NoteId {
        let origin = host.map(|host| self.origin_for_host(host).unwrap_or(self.next_id));
        let source = origin
            .and_then(|origin| self.saved_sources.get(&origin))
            .cloned()
            .unwrap_or(source);
        self.insert_draft(host, origin, source)
    }

    fn insert_draft(
        &mut self,
        host: Option<HostId>,
        origin: Option<NoteId>,
        source: String,
    ) -> NoteId {
        let id = self.next_id;
        self.next_id += 1;
        self.insert(
            Note {
                id,
                source,
                markdown: String::new(),
                version: 0,
                updated: timestamp(),
                origin,
                name: None,
                location: None,
                file: None,
            },
            host,
        );
        id
    }

    fn origin_for_host(&self, host: HostId) -> Option<NoteId> {
        self.source_hosts.get(&host).copied()
    }

    fn remove_origin_member(&mut self, origin: NoteId, id: NoteId) {
        if let Some(members) = self.by_origin.get_mut(&origin) {
            members.remove(&id);
            if members.is_empty() {
                self.by_origin.remove(&origin);
            }
        }
    }

    fn origins(&self) -> BTreeSet<NoteId> {
        self.saved_sources
            .keys()
            .copied()
            .chain(self.by_origin.keys().copied())
            .collect()
    }

    pub fn needs_identity(&self, host: HostId) -> bool {
        self.source_hosts
            .get(&host)
            .is_some_and(|origin| !self.bindings.contains_key(origin))
    }

    pub fn hosts_needing_identity(&self) -> Vec<HostId> {
        self.source_hosts
            .iter()
            .filter(|(_, origin)| !self.bindings.contains_key(origin))
            .map(|(&host, _)| host)
            .collect()
    }

    pub fn remember(&mut self, host: HostId, identity: WindowIdentity) -> Result<(), String> {
        let Some(&origin) = self.source_hosts.get(&host) else {
            return Ok(());
        };
        if self.bindings.get(&origin) == Some(&identity) {
            return Ok(());
        }
        let mut bindings = self.bindings.clone();
        let origins = self.origins();
        bindings.retain(|key, _| origins.contains(key));
        bindings.insert(origin, identity);
        write_json(&self.folder.join("window-bindings.json"), &bindings)
            .map_err(|e| e.to_string())?;
        self.bindings = bindings;
        Ok(())
    }

    pub fn pending_bindings(&self) -> BTreeMap<NoteId, WindowIdentity> {
        let origins = self.origins();
        let live: BTreeSet<_> = self.source_hosts.values().copied().collect();
        let mut documents = HashMap::new();
        for (key, identity) in &self.bindings {
            if origins.contains(key)
                && let Some(doc) = &identity.document
            {
                *documents.entry((&identity.app, doc)).or_insert(0) += 1;
            }
        }
        self.bindings
            .iter()
            .filter(|(key, _)| origins.contains(key) && !live.contains(key))
            .filter_map(|(key, original)| {
                let mut identity = original.clone();
                if original.ambiguous
                    || original
                        .document
                        .as_ref()
                        .is_some_and(|doc| documents[&(&original.app, doc)] != 1)
                {
                    identity.document = None;
                }
                (identity.session.is_some() || identity.document.is_some())
                    .then_some((*key, identity))
            })
            .collect()
    }

    pub fn restore(&mut self, origin: NoteId, host: HostId, identity: &WindowIdentity) -> bool {
        let Some(wanted) = self.bindings.get(&origin) else {
            return false;
        };
        if (!self.by_origin.contains_key(&origin) && !self.saved_sources.contains_key(&origin))
            || self.source_hosts.contains_key(&host)
            || self.source_hosts.values().any(|&value| value == origin)
            || wanted.app != identity.app
        {
            return false;
        }
        let exact = wanted.session.is_some()
            && wanted.session == identity.session
            && (wanted.document.is_none() || wanted.document == identity.document);
        let same_document = !wanted.ambiguous
            && !identity.ambiguous
            && wanted.document.is_some()
            && wanted.document == identity.document
            && !self.bindings.iter().any(|(other, value)| {
                *other != origin
                    && value.app == wanted.app
                    && value.document == wanted.document
                    && (self.saved_sources.contains_key(other)
                        || self.by_origin.contains_key(other))
            });
        if !exact && !same_document {
            return false;
        }
        self.source_hosts.insert(host, origin);
        for id in self.by_origin.get(&origin).into_iter().flatten() {
            let entry = self.records.get_mut(id).expect("origin indexes its notes");
            entry.host = Some(host);
            entry.card.attached = true;
            self.by_host
                .entry(host)
                .or_default()
                .insert((entry.note.updated, entry.note.id));
        }
        true
    }

    pub fn live_source(&self, key: &str) -> Result<HostId, String> {
        let origin = key
            .strip_prefix("window/")
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or("找不到该来源窗口")?;
        self.source_hosts
            .iter()
            .find_map(|(&host, &value)| (value == origin).then_some(host))
            .ok_or_else(|| "窗口已丢失，请选择已打开的来源".into())
    }

    pub fn register_source(&mut self, host: HostId, source: String) -> Result<String, String> {
        if let Some(origin) = self.source_hosts.get(&host)
            && self.saved_sources.contains_key(origin)
        {
            return Ok(format!("window/{origin}"));
        }
        let origin = self.origin_for_host(host).unwrap_or(self.next_id);
        let mut sources = self.saved_sources.clone();
        let name = self
            .latest(host)
            .map_or(source, |id| self.records[&id].note.source.clone());
        sources.insert(origin, name);
        write_json(&self.sources_file, &sources).map_err(|error| error.to_string())?;
        self.saved_sources = sources;
        self.source_hosts.insert(host, origin);
        self.next_id = self.next_id.max(origin + 1);
        Ok(format!("window/{origin}"))
    }

    pub fn is_registered(&self, host: HostId) -> bool {
        self.source_hosts
            .get(&host)
            .is_some_and(|origin| self.saved_sources.contains_key(origin))
    }

    pub fn attach_to_host(
        &mut self,
        id: NoteId,
        host: HostId,
        source_name: String,
    ) -> Result<String, String> {
        let entry = self.records.get(&id).ok_or("找不到便签")?;
        if entry.host == Some(host) {
            return Ok(entry.source_key.clone());
        }

        let origin = self.origin_for_host(host).unwrap_or(self.next_id);
        let source = self.saved_sources.get(&origin).cloned().or_else(|| {
            self.latest(host)
                .map(|id| self.records[&id].note.source.clone())
        });
        let mut note = entry.note.clone();
        note.origin = Some(origin);
        note.source = source.unwrap_or(source_name);
        self.replace_association(note, Some(host))?;
        self.next_id = self.next_id.max(origin + 1);
        Ok(format!("window/{origin}"))
    }

    pub fn close_source(&mut self, key: &str) -> Result<(Option<HostId>, Vec<NoteId>), String> {
        let origin: NoteId = key
            .strip_prefix("window/")
            .and_then(|s| s.parse().ok())
            .ok_or("找不到来源窗口")?;
        if !self.saved_sources.contains_key(&origin) && !self.by_origin.contains_key(&origin) {
            return Err("找不到来源窗口".into());
        }
        let directory = self.folder.join("closing-sources");
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let marker = directory.join(origin.to_string());
        write_json(&marker, &origin).map_err(|e| e.to_string())?;
        Ok(self.complete_source_close(origin, &marker))
    }

    fn complete_source_close(
        &mut self,
        origin: NoteId,
        marker: &Path,
    ) -> (Option<HostId>, Vec<NoteId>) {
        let host = self
            .source_hosts
            .iter()
            .find_map(|(&host, &key)| (key == origin).then_some(host));
        if let Some(host) = host {
            self.return_to_main(host);
        }
        let ids: Vec<_> = self
            .by_origin
            .get(&origin)
            .into_iter()
            .flatten()
            .copied()
            .collect();
        self.saved_sources.remove(&origin);
        self.bindings.remove(&origin);
        let mut written = write_json(&self.sources_file, &self.saved_sources)
            .and_then(|_| write_json(&self.folder.join("window-bindings.json"), &self.bindings))
            .is_ok();
        for &id in &ids {
            let mut note = self.records[&id].note.clone();
            note.origin = None;
            note.source = "Jotiva".into();
            if note.version != 0 {
                written &= write_json(&self.folder.join(format!("{id}.json")), &note).is_ok();
            }
            self.insert(note, None);
        }
        if written {
            let _ = fs::remove_file(marker);
        }
        (host, ids)
    }

    pub fn detach(&mut self, id: NoteId) -> Result<(), String> {
        let mut note = self.note(id).ok_or("找不到便签")?;
        let Some(origin) = note.origin else {
            return Ok(());
        };
        if self
            .by_origin
            .get(&origin)
            .is_some_and(|ids| ids.len() == 1)
            && self.saved_sources.contains_key(&origin)
        {
            self.close_source(&format!("window/{origin}"))?;
            return Ok(());
        }
        note.origin = None;
        note.source = "Jotiva".into();
        self.replace_association(note, None)
    }

    fn replace_association(&mut self, note: Note, host: Option<HostId>) -> Result<(), String> {
        let id = note.id;
        if note.version != 0 {
            write_json(&self.folder.join(format!("{id}.json")), &note)
                .map_err(|error| format!("无法保存窗口关联：{error}"))?;
        }
        if let Some(previous) = self.host(id) {
            let keys = self
                .by_host
                .get_mut(&previous)
                .expect("host indexes its notes");
            keys.remove(&(note.updated, id));
            if keys.is_empty() {
                self.by_host.remove(&previous);
                if !self.is_registered(previous) {
                    self.source_hosts.remove(&previous);
                }
            }
        }
        self.insert(note, host);
        Ok(())
    }

    pub fn new_at_source(&mut self, key: &str) -> Result<NoteId, String> {
        if let Some(origin) = key
            .strip_prefix("window/")
            .and_then(|id| id.parse::<NoteId>().ok())
            && let Some(source) = self.saved_sources.get(&origin)
        {
            let host = self
                .source_hosts
                .iter()
                .find_map(|(&host, &id)| (id == origin).then_some(host));
            return Ok(self.insert_draft(host, Some(origin), source.clone()));
        }
        let entry = self
            .records
            .values()
            .find(|entry| entry.source_key == key)
            .ok_or("找不到来源")?;
        Ok(self.insert_draft(entry.host, entry.note.origin, entry.note.source.clone()))
    }

    pub fn note(&self, id: NoteId) -> Option<Note> {
        self.records.get(&id).map(|entry| entry.note.clone())
    }

    pub fn missing_file_snapshot(&self, id: NoteId) -> Option<Note> {
        let note = &self.records.get(&id)?.note;
        matches!(note.disk_path()?.try_exists(), Ok(false)).then(|| note.clone())
    }

    pub fn filing_text(&self, id: NoteId, version: u64) -> Option<&str> {
        let note = &self.records.get(&id)?.note;
        if version == 0
            || note.version != version
            || note.file.is_some()
            || note.location.as_ref().and_then(|loc| loc.folder).is_some()
        {
            return None;
        }
        (!note.markdown.trim().is_empty()).then_some(&note.markdown)
    }

    pub fn host(&self, id: NoteId) -> Option<HostId> {
        self.records.get(&id).and_then(|entry| entry.host)
    }

    pub fn card_at_path(&self, path: &Path) -> Option<&Card> {
        self.by_path
            .get(path)
            .and_then(|id| self.records.get(id))
            .map(|entry| &entry.card)
    }

    pub fn disk_path(&self, id: NoteId) -> Option<&Path> {
        self.records.get(&id)?.note.disk_path()
    }

    pub fn title(&self, id: NoteId) -> Option<&str> {
        self.records.get(&id).map(|entry| entry.card.title.as_str())
    }

    pub fn card(&self, id: NoteId) -> Option<Card> {
        self.records.get(&id).map(|entry| entry.card.clone())
    }

    pub fn latest(&self, host: HostId) -> Option<NoteId> {
        self.by_host.get(&host)?.last().map(|(_, id)| *id)
    }

    pub fn latest_except(&self, host: HostId, excluded: impl Fn(NoteId) -> bool) -> Option<NoteId> {
        self.by_host
            .get(&host)?
            .iter()
            .rev()
            .map(|(_, id)| *id)
            .find(|id| !excluded(*id))
    }

    pub fn saved_for_host(&self, host: HostId) -> usize {
        self.by_host
            .get(&host)
            .into_iter()
            .flatten()
            .rev()
            .filter(|(_, id)| self.records[id].note.version != 0)
            .take(9)
            .count()
    }

    pub fn return_to_main(&mut self, host: HostId) -> bool {
        let registered = self.source_hosts.remove(&host).is_some();
        let Some(keys) = self.by_host.remove(&host) else {
            return registered;
        };
        for (_, id) in keys {
            let entry = self
                .records
                .get_mut(&id)
                .expect("host index points to a note");
            entry.host = None;
            entry.card.attached = false;
        }
        true
    }

    pub fn discard_empty(&mut self, id: NoteId) -> bool {
        let Some(entry) = self.records.get(&id) else {
            return false;
        };
        if entry.note.version != 0 {
            return false;
        }
        self.forget(id);
        true
    }

    pub fn forget(&mut self, id: NoteId) {
        let entry = &self.records[&id];
        if let Some(path) = entry.note.disk_path() {
            self.by_path.remove(path);
        }
        let key = (entry.note.updated, id);
        self.recent.remove(&key);
        if let Some(host) = entry.host {
            let notes = self
                .by_host
                .get_mut(&host)
                .expect("note is indexed by its host");
            notes.remove(&key);
            if notes.is_empty() {
                self.by_host.remove(&host);
                if !self.is_registered(host) {
                    self.source_hosts.remove(&host);
                }
            }
        }
        if let Some(entry) = self.records.remove(&id)
            && let Some(origin) = entry.note.origin
        {
            self.remove_origin_member(origin, id);
        }
    }

    pub fn list(&self, query: &str, offset: usize, scope: Option<&Scope>) -> Page {
        const LIMIT: usize = 40;
        let query = query.trim().to_lowercase();
        let keys = match scope {
            Some(Scope::Host { id }) => self.by_host.get(id),
            _ => Some(&self.recent),
        };
        let mut matches = keys
            .into_iter()
            .flatten()
            .rev()
            .filter_map(|(_, id)| {
                let entry = &self.records[id];
                (entry.note.version != 0
                    && entry.matches(scope)
                    && entry.searchable.contains(&query))
                .then_some(&entry.card)
            })
            .skip(offset);
        let items = matches.by_ref().take(LIMIT).cloned().collect();
        Page {
            items,
            more: matches.next().is_some(),
        }
    }

    pub fn search(&self, query: &str, host: Option<HostId>, page: usize) -> SearchPage {
        const PER_GROUP: usize = 4;
        let query = query.trim().to_lowercase();
        let mut result = SearchPage::default();
        let mut skip = [page.saturating_mul(PER_GROUP); 3];
        let current = host
            .and_then(|id| self.by_host.get(&id))
            .map_or(0, BTreeSet::len);
        let attached: usize = self.by_host.values().map(BTreeSet::len).sum();
        let mut remaining = [current, attached - current, self.recent.len() - attached];
        for (_, id) in self.recent.iter().rev() {
            if remaining == [0; 3] {
                break;
            }
            let entry = &self.records[id];
            let (index, items) = match entry.host {
                Some(id) if Some(id) == host => (0, &mut result.current),
                Some(_) => (1, &mut result.attached),
                None => (2, &mut result.documents),
            };
            if remaining[index] == 0 {
                continue;
            }
            remaining[index] -= 1;
            if entry.note.version == 0 || !entry.searchable.contains(&query) {
                continue;
            }
            if skip[index] > 0 {
                skip[index] -= 1;
            } else if items.len() < PER_GROUP {
                items.push(entry.card.clone());
            } else {
                remaining[index] = 0;
                result.more = true;
            }
        }
        result
    }

    pub fn sources(&self) -> Vec<SourceGroup> {
        let mut groups = BTreeMap::<String, BTreeMap<String, SourceWindow>>::new();
        let live_saved: BTreeSet<_> = self.source_hosts.values().copied().collect();
        let saved = self
            .saved_sources
            .iter()
            .map(|(id, source)| (format!("window/{id}"), source, live_saved.contains(id)));
        let observed = self
            .records
            .values()
            .filter(|entry| entry.note.version != 0 && entry.note.origin.is_some())
            .map(|entry| {
                (
                    entry.source_key.clone(),
                    &entry.note.source,
                    entry.host.is_some(),
                )
            });
        for (key, source, attached) in observed.chain(saved) {
            let (app, title) = source.split_once(" · ").unwrap_or((source, ""));
            groups
                .entry(app.into())
                .or_default()
                .entry(key.clone())
                .and_modify(|window| window.attached |= attached)
                .or_insert_with(|| SourceWindow {
                    key,
                    title: title.into(),
                    attached,
                });
        }
        groups
            .into_iter()
            .map(|(app, entries)| {
                let mut windows: Vec<_> = entries.into_values().collect();
                windows.sort_unstable_by(|left, right| {
                    left.title.cmp(&right.title).then(left.key.cmp(&right.key))
                });
                SourceGroup { app, windows }
            })
            .collect()
    }

    pub fn inbox_directory(&self) -> PathBuf {
        self.folder.join("其他")
    }

    // The note snapshot is durable before publishing its Markdown. A hard link identifies
    // our file after interruption, even when another file has the same name and contents.
    fn publish_inbox(&self, note: &mut Note) -> io::Result<()> {
        let inbox = self.inbox_directory();
        check_directory(&inbox)?;
        let stage = self.folder.join(format!(".{}.inbox", note.id));
        let mut published = None;
        if stage.try_exists()? {
            for entry in fs::read_dir(&inbox)? {
                let entry = entry?;
                if same_regular_file(&entry.path(), &stage)? {
                    published = Some(entry.path());
                    break;
                }
            }
        }
        let first = note
            .markdown
            .lines()
            .find_map(|line| note_name(line.trim().trim_start_matches('#').trim()));
        let title = note.name.as_deref().or(first.as_deref());
        let path = if let Some(path) = published
            && check_markdown(&path, &note.markdown).is_ok()
        {
            path
        } else {
            // The old inode can have a new owner/content after interruption. Keep its visible file.
            if stage.try_exists()? {
                fs::remove_file(&stage)?;
            }
            crate::storage::create_markdown(&stage, &note.markdown)?;
            crate::storage::named_markdown_path(&inbox, title.unwrap_or("未命名"), |path| {
                fs::hard_link(&stage, path)
            })?
        };
        if title.is_some() {
            note.name = file_stem(&path);
        }
        note.location = Some(NoteLocation { folder: None, path });
        write_json(&self.metadata_path(note.id), note)?;
        // The persisted location is authoritative. Startup can remove this link if cleanup fails.
        let _ = fs::remove_file(stage);
        Ok(())
    }

    pub fn metadata_path(&self, id: NoteId) -> PathBuf {
        self.folder.join(format!("{id}.json"))
    }

    pub fn under(&self, path: &Path) -> Vec<Note> {
        self.by_path
            .range(path.to_owned()..)
            .take_while(|(file, _)| file.starts_with(path))
            .map(|(_, id)| self.records[id].note.clone())
            .collect()
    }

    pub fn update_metadata(&mut self, note: Note) {
        let host = self.host(note.id);
        self.insert(note, host);
    }

    pub fn duplicate(
        &mut self,
        id: NoteId,
        destination: Option<&Folder>,
    ) -> Result<NoteId, String> {
        let markdown = self.refresh_from_disk(id)?.0.markdown.clone();
        let title = format!("{} 副本", self.records[&id].card.title);
        self.create_named(title, markdown, destination)
    }

    fn create_archive_file(
        &self,
        folder: Option<&Folder>,
        title: &str,
        markdown: &str,
    ) -> Result<NoteLocation, String> {
        let inbox = self.inbox_directory();
        let directory = folder.map_or(inbox.as_path(), |f| f.path.as_path());
        check_directory(directory).map_err(|e| e.to_string())?;
        let path = crate::storage::create_named_markdown(directory, title, markdown)?;
        Ok(NoteLocation {
            folder: folder.map(|f| f.id),
            path,
        })
    }

    pub fn create_named(
        &mut self,
        title: String,
        markdown: String,
        destination: Option<&Folder>,
    ) -> Result<NoteId, String> {
        let location = self.create_archive_file(destination, &title, &markdown)?;
        let id = self.next_id;
        let copy = Note {
            id,
            name: file_stem(&location.path),
            source: "Jotiva".into(),
            origin: None,
            location: Some(location),
            file: None,
            version: 1,
            updated: timestamp(),
            markdown,
        };
        self.commit_archive(copy, None)?;
        self.next_id += 1;
        Ok(id)
    }

    // Explicit creation publishes the empty Markdown too. A plain capture
    // shortcut still creates an ephemeral draft until its first input.
    pub fn create_window_note(
        &mut self,
        host: Option<HostId>,
        source: String,
    ) -> Result<NoteId, String> {
        let location = self.create_archive_file(None, "新便签", "")?;
        let id = self.new_note(host, source);
        let mut note = self.records[&id].note.clone();
        note.version = 1;
        note.location = Some(location);
        if let Err(error) = self.commit_archive(note, host) {
            self.forget(id);
            return Err(error);
        }
        Ok(id)
    }

    pub fn name_unfiled_draft(&mut self, mut note: Note, name: String) -> Result<(), String> {
        let inbox = self.inbox_directory();
        check_directory(&inbox).map_err(|e| e.to_string())?;
        let path = inbox.join(format!("{name}.md"));
        crate::storage::create_markdown(&path, &note.markdown).map_err(|e| e.to_string())?;
        let host = self.host(note.id);
        note.location = Some(NoteLocation { folder: None, path });
        note.name = Some(name);
        note.version = 1;
        self.commit_archive(note, host)
    }

    fn commit_archive(&mut self, note: Note, host: Option<HostId>) -> Result<(), String> {
        if let Err(error) = write_json(&self.metadata_path(note.id), &note) {
            let location = note
                .location
                .as_ref()
                .expect("new archive has a Markdown path");
            let _ = remove_matching(&location.path, &note.markdown);
            return Err(error.to_string());
        }
        self.insert(note, host);
        Ok(())
    }

    pub fn has_folder(&self, id: FolderId) -> bool {
        self.records
            .values()
            .any(|entry| entry.card.folder == Some(id))
    }

    /// Paths returned by the native picker and workspace are canonicalized before indexing.
    pub fn open_file(&mut self, path: &Path) -> Result<NoteId, String> {
        let path = fs::canonicalize(path).map_err(|error| format!("无法打开文档：{error}"))?;
        if !super::workspace::is_document(&path) || !path.is_file() {
            return Err("请选择 .md、.markdown 或 .txt 文件".into());
        }
        let markdown = fs::read_to_string(&path)
            .map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
        if let Some(&id) = self.by_path.get(&path) {
            if self.records[&id].note.markdown != markdown {
                self.update_from_disk(id, markdown)?;
            }
            return Ok(id);
        }
        let id = self.next_id;
        let in_inbox = path.parent() == Some(self.inbox_directory().as_path());
        let note = Note {
            id,
            source: "Jotiva".into(),
            markdown,
            version: 1,
            updated: timestamp(),
            origin: None,
            location: in_inbox.then(|| NoteLocation {
                folder: None,
                path: path.clone(),
            }),
            name: in_inbox.then(|| file_stem(&path)).flatten(),
            file: (!in_inbox).then_some(path),
        };
        write_json(&self.folder.join(format!("{id}.json")), &note)
            .map_err(|error| error.to_string())?;
        self.next_id += 1;
        self.insert(note, None);
        Ok(id)
    }

    fn update_from_disk(&mut self, id: NoteId, markdown: String) -> Result<(), String> {
        let entry = &self.records[&id];
        let mut note = entry.note.clone();
        note.markdown = markdown;
        note.version += 1;
        // The note was edited outside Jotiva. Preserve its position in the Jotiva recency index.
        write_json(&self.folder.join(format!("{id}.json")), &note)
            .map_err(|error| error.to_string())?;
        self.insert(note, entry.host);
        Ok(())
    }

    pub fn refresh_from_disk(&mut self, id: NoteId) -> Result<(&Note, bool), String> {
        let entry = self.records.get(&id).ok_or("找不到便签")?;
        if let Some(path) = entry.note.disk_path() {
            let markdown = fs::read_to_string(path)
                .map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
            if entry.note.markdown != markdown {
                self.update_from_disk(id, markdown)?;
                return Ok((&self.records[&id].note, true));
            }
        }
        Ok((&self.records[&id].note, false))
    }

    pub fn move_to(&mut self, id: NoteId, folder: Option<&Folder>) -> Result<MoveResult, String> {
        check_directory(&self.folder).map_err(|e| e.to_string())?;
        let entry = self.records.get(&id).ok_or("找不到便签")?;
        if entry.note.file.is_some() {
            return Err("此文档在原路径编辑，请在 Finder 中管理文件位置".into());
        }
        if entry.note.version == 0 {
            return Err("请先输入并保存正文".into());
        }
        let previous = entry.note.location.clone();
        if previous.as_ref().and_then(|location| location.folder) == folder.map(|folder| folder.id)
        {
            return Ok(MoveResult {
                path: previous.map(|location| location.path),
                warning: None,
            });
        }
        let mut note = entry.note.clone();
        let host = entry.host;
        if let Some(location) = &previous {
            if let Some(parent) = location.path.parent() {
                check_directory(parent).map_err(|e| e.to_string())?;
            }
            check_markdown(&location.path, &note.markdown)
                .map_err(|error| format!("无法移动原文件：{error}"))?;
        }
        let location = self.create_archive_file(folder, &entry.card.title, &note.markdown)?;
        note.name = file_stem(&location.path);
        note.location = Some(location);
        if let Err(error) = write_json(&self.folder.join(format!("{id}.json")), &note) {
            if let Some(location) = &note.location {
                let _ = remove_matching(&location.path, &note.markdown);
            }
            return Err(format!("无法记录新位置：{error}"));
        }
        let warning = previous.and_then(|location| {
            remove_matching(&location.path, &note.markdown)
                .err()
                .map(|error| {
                    format!(
                        "文档已保存到新位置，未能清理原路径 {}（{error}）",
                        location.path.display()
                    )
                })
        });
        let path = note.location.as_ref().map(|location| location.path.clone());
        self.insert(note, host);
        Ok(MoveResult { path, warning })
    }

    pub fn save(&mut self, id: NoteId, version: u64, markdown: String) -> Result<u64, String> {
        check_directory(&self.folder).map_err(|e| e.to_string())?;
        let entry = self.records.get(&id).ok_or("便签已关闭")?;
        if entry.note.file.is_none() && entry.card.folder.is_none() {
            check_directory(&self.inbox_directory()).map_err(|e| e.to_string())?;
        }
        if version != entry.note.version {
            return Err("此便签已在另一个编辑窗口更新".into());
        }
        let previous = &entry.note.markdown;
        let markdown = if entry.note.disk_path().is_some()
            && previous.contains("\r\n")
            && previous
                .split_inclusive('\n')
                .all(|line| !line.ends_with('\n') || line.ends_with("\r\n"))
        {
            markdown.replace("\r\n", "\n").replace('\n', "\r\n")
        } else {
            markdown
        };
        if version != 0 && markdown == *previous {
            return Ok(version);
        }
        let old_key = (entry.note.updated, id);
        let host = entry.host;
        let mut note = entry.note.clone();
        note.markdown = markdown;
        if note.file.is_none() && note.name.is_none() {
            note.name = note
                .markdown
                .lines()
                .find_map(|line| note_name(line.trim().trim_start_matches('#').trim()));
        }
        note.version = version + 1;
        note.updated = timestamp();
        let previous_path = entry.note.disk_path();
        let first_title =
            entry.note.name.is_none() && note.name.is_some() && note.location.is_some();
        if let (Some(path), Some(title)) = (previous_path, note.name.as_deref())
            && first_title
        {
            check_markdown(path, previous).map_err(|e| e.to_string())?;
            let target = crate::storage::create_named_markdown(
                &self.inbox_directory(),
                title,
                &note.markdown,
            )?;
            note.name = file_stem(&target);
            note.location = Some(NoteLocation {
                folder: None,
                path: target,
            });
        } else if let Some(path) = previous_path {
            replace_markdown(path, previous, &note.markdown)
                .map_err(|error| format!("无法保存 Markdown：{error}"))?;
        }
        if let Err(error) = write_json(&self.metadata_path(id), &note) {
            if let Some(path) = note.disk_path() {
                let rollback = if first_title {
                    remove_matching(path, &note.markdown)
                } else {
                    replace_markdown(path, &note.markdown, previous)
                };
                if let Err(restore) = rollback {
                    return Err(format!(
                        "记录更新失败（{error}），当前正文已写入 {}（{restore}）",
                        path.display()
                    ));
                }
            }
            return Err(error.to_string());
        }
        if note.disk_path().is_none() {
            self.publish_inbox(&mut note).map_err(|e| e.to_string())?;
        } else if first_title && let Some(path) = previous_path {
            // The new name and the new content have already committed.
            // Preserve the old file when the filesystem refuses to remove it.
            let _ = remove_matching(path, previous);
        }
        self.recent.remove(&old_key);
        if let Some(host) = host {
            self.by_host
                .get_mut(&host)
                .expect("note is indexed by its host")
                .remove(&old_key);
        }
        let version = note.version;
        self.insert(note, host);
        Ok(version)
    }
}

pub fn file_stem(path: &Path) -> Option<String> {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
}

fn note_name(line: &str) -> Option<String> {
    let name: String = line
        .chars()
        .take(40)
        .map(|ch| {
            if ch.is_control()
                || matches!(ch, '/' | ':' | '\\')
                || crate::platform::storage::invalid_name_char(ch)
            {
                '-'
            } else {
                ch
            }
        })
        .collect();
    let name = name.trim_matches(['.', ' ']);
    if crate::platform::storage::reserved_name(name) {
        return Some(format!("_{name}"));
    }
    (!name.is_empty()).then(|| name.to_owned())
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock precedes Unix epoch")
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn markdown_headings_yield_usable_windows_filenames() {
        assert_eq!(note_name("LPT1"), Some("_LPT1".into()));
        assert_eq!(note_name("a?b<c"), Some("a-b-c".into()));
        assert_eq!(note_name("Console"), Some("Console".into()));
    }

    #[test]
    fn card_excerpt_shows_the_opening_without_repeating_the_filename() {
        let mut note = Note {
            id: 1,
            source: String::new(),
            markdown: "# 议程\n\n时间待定\n稍后确认\n- [ ] 之后的待办".into(),
            version: 1,
            updated: 0,
            name: None,
            origin: None,
            location: None,
            file: Some("议程.md".into()),
        };
        assert_eq!(
            Entry::new(note.clone(), None).card.excerpt,
            "时间待定\n稍后确认"
        );
        note.markdown = "独立的一行".into();
        assert_eq!(Entry::new(note.clone(), None).card.excerpt, note.markdown);
        note.markdown = "字".repeat(150);
        assert_eq!(Entry::new(note, None).card.excerpt, "字".repeat(120));
    }

    #[test]
    fn search_pages_each_destination_without_duplicate_notes() {
        let data = std::env::temp_dir().join(format!(
            "jot-search-groups-{}-{}",
            std::process::id(),
            timestamp()
        ));
        let mut notes = Notes::load(data.clone()).unwrap();
        for host in [Some(11), Some(12), None] {
            for _ in 0..7 {
                let id = notes.new_note(host, "记录".into());
                notes.save(id, 0, format!("WeekLy {id}")).unwrap();
            }
        }
        let first = notes.search(" weekly ", Some(11), 0);
        let second = notes.search("weekly", Some(11), 1);
        assert!(first.more);
        assert!(!second.more);
        let mut ids = BTreeSet::new();
        for (expected, first, second) in [
            (Some(11), first.current, second.current),
            (Some(12), first.attached, second.attached),
            (None, first.documents, second.documents),
        ] {
            assert_eq!((first.len(), second.len()), (4, 3));
            for card in first.into_iter().chain(second) {
                assert_eq!(notes.host(card.id), expected);
                assert!(ids.insert(card.id));
            }
        }
        assert_eq!(ids.len(), 21);
        assert!(notes.search("", None, 0).current.is_empty());
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn explicit_siblings_are_separate_archived_files_before_typing() {
        let directory =
            std::env::temp_dir().join(format!("jot-new-siblings-{}", std::process::id()));
        let mut notes = Notes::load(directory.clone()).unwrap();
        let create = |notes: &mut Notes| notes.create_window_note(Some(42), "会议".into()).unwrap();
        let first = create(&mut notes);
        let second = create(&mut notes);
        let first_path = notes.card(first).unwrap().archive_path.unwrap();
        let second_path = notes.card(second).unwrap().archive_path.unwrap();
        assert_ne!(first_path, second_path);
        for path in [&first_path, &second_path] {
            assert_eq!(path.parent(), Some(notes.inbox_directory().as_path()));
            assert_eq!(fs::read_to_string(path).unwrap(), "");
        }
        assert_eq!(
            notes.note(first).unwrap().origin,
            notes.note(second).unwrap().origin
        );
        assert_eq!(notes.host(second), Some(42));
        assert!(!notes.discard_empty(second));
        notes.save(first, 1, "# 会议日程".into()).unwrap();
        assert!(!first_path.exists());
        let renamed = notes.card(first).unwrap().archive_path.unwrap();
        assert_eq!(renamed.file_name().unwrap(), "会议日程.md");
        assert_eq!(fs::read_to_string(second_path).unwrap(), "");
        assert_eq!(notes.saved_for_host(42), 2);
        let reloaded = Notes::load(directory.clone()).unwrap();
        assert_eq!(reloaded.card(first).unwrap().archive_path.unwrap(), renamed);
        assert_eq!(reloaded.card(second).unwrap().version, 1);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn interrupted_source_close_finishes_on_restart_without_detaching_a_sibling_window() {
        let data = std::env::temp_dir().join(format!(
            "jot-close-interrupted-{}-{}",
            std::process::id(),
            timestamp()
        ));
        let mut notes = Notes::load(data.clone()).unwrap();
        notes.register_source(22, "会议".into()).unwrap();
        notes.register_source(23, "会议".into()).unwrap();
        let first = notes.new_note(Some(22), "会议".into());
        let second = notes.new_note(Some(22), "会议".into());
        let sibling = notes.new_note(Some(23), "会议".into());
        for id in [first, second, sibling] {
            notes.save(id, 0, format!("正文 {id}")).unwrap();
        }
        // Stop with one record committed and one still pointing at the closing source.
        let origin = notes.origin_for_host(22).unwrap();
        let directory = data.join("closing-sources");
        fs::create_dir_all(&directory).unwrap();
        let marker = directory.join(origin.to_string());
        write_json(&marker, &origin).unwrap();
        notes.detach(first).unwrap();
        drop(notes);

        for _ in 0..2 {
            let reopened = Notes::load(data.clone()).unwrap();
            assert!(!marker.exists());
            assert_eq!(reopened.sources()[0].windows.len(), 1);
            for id in [first, second] {
                let note = reopened.note(id).unwrap();
                assert!(note.origin.is_none());
                assert_eq!(note.markdown, format!("正文 {id}"));
            }
            assert!(reopened.note(sibling).unwrap().origin.is_some());
        }
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn closing_notes_and_sources_keeps_documents_and_removes_empty_sources() {
        let data = std::env::temp_dir().join(format!(
            "jot-close-source-{}-{}",
            std::process::id(),
            timestamp()
        ));
        let mut notes = Notes::load(data.clone()).unwrap();
        let key = notes.register_source(22, "会议".into()).unwrap();
        let a = notes.new_note(Some(22), "会议".into());
        let b = notes.new_note(Some(22), "会议".into());
        notes.save(a, 0, "A".into()).unwrap();
        notes.save(b, 0, "B".into()).unwrap();
        notes.detach(a).unwrap();
        assert_eq!(notes.host(b), Some(22));
        assert!(notes.card(a).unwrap().source == "Jotiva");
        notes.close_source(&key).unwrap();
        let notes = Notes::load(data.clone()).unwrap();
        assert!(notes.sources().is_empty());
        assert_eq!(notes.note(a).unwrap().markdown, "A");
        assert_eq!(notes.note(b).unwrap().markdown, "B");
        assert!(notes.pending_bindings().is_empty());
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn sources_restore_after_restart_without_crossing_same_title_windows() {
        let data = std::env::temp_dir().join(format!(
            "jot-recovery-{}-{}",
            std::process::id(),
            timestamp()
        ));
        let mut notes = Notes::load(data.clone()).unwrap();
        let first = notes.new_note(Some(11), "编辑器 · 同名".into());
        let second = notes.new_note(Some(12), "编辑器 · 同名".into());
        let ordinary = notes.new_note(None, "Jotiva".into());
        for id in [first, second, ordinary] {
            notes.save(id, 0, "正文".into()).unwrap();
        }
        let identity = |session: &str| WindowIdentity {
            app: "test.editor".into(),
            instance: Some("owner".into()),
            session: Some(session.into()),
            document: Some("same-document".into()),
            ambiguous: false,
        };
        notes.remember(11, identity("old-1")).unwrap();
        notes.remember(12, identity("old-2")).unwrap();
        let mut notes = Notes::load(data.clone()).unwrap();
        assert_eq!(
            notes.sources().len(),
            1,
            "ordinary drafts have no window origin"
        );
        assert!(!notes.restore(first, 90, &identity("replacement")));
        assert!(notes.restore(second, 91, &identity("old-2")));
        assert_eq!(notes.host(first), None);
        assert!(notes.restore(first, 92, &identity("old-1")));
        let key = notes.attach_to_host(ordinary, 92, String::new()).unwrap();
        assert_eq!(notes.live_source(&key).unwrap(), 92);
        notes.detach(ordinary).unwrap();
        assert!(notes.host(ordinary).is_none());
        assert_eq!(notes.note(ordinary).unwrap().markdown, "正文");
        assert_eq!(notes.note(ordinary).unwrap().source, "Jotiva");
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn unique_document_restores_all_its_notes_and_never_binds_another_application() {
        let data = std::env::temp_dir().join(format!(
            "jot-document-{}-{}",
            std::process::id(),
            timestamp()
        ));
        let mut notes = Notes::load(data.clone()).unwrap();
        let first = notes.new_note(Some(11), "会议 · 文件".into());
        let second = notes.new_note(Some(11), "会议 · 文件".into());
        for id in [first, second] {
            notes.save(id, 0, "记录".into()).unwrap();
        }
        let mut identity = WindowIdentity {
            app: "test.meeting".into(),
            instance: Some("first-launch".into()),
            session: Some("original".into()),
            document: Some("document".into()),
            ambiguous: false,
        };
        notes.remember(11, identity.clone()).unwrap();
        notes.return_to_main(11);
        identity.session = Some("new".into());
        identity.instance = Some("next-launch".into());
        identity.app = "other".into();
        assert!(!notes.restore(first, 22, &identity));
        identity.app = "test.meeting".into();
        identity.ambiguous = true;
        assert!(!notes.restore(first, 22, &identity));
        identity.ambiguous = false;
        assert!(notes.restore(first, 22, &identity));
        assert_eq!(notes.host(first), Some(22));
        assert_eq!(notes.host(second), Some(22));
        assert!(notes.pending_bindings().is_empty());
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn many_bindings_keep_a_direct_host_index_and_search_page() {
        let folder = PathBuf::new();
        let mut notes = Notes {
            folder,
            sources_file: PathBuf::new(),
            saved_sources: BTreeMap::new(),
            bindings: BTreeMap::new(),
            source_hosts: HashMap::new(),
            records: HashMap::new(),
            by_path: BTreeMap::new(),
            recent: BTreeSet::new(),
            by_host: HashMap::new(),
            by_origin: HashMap::new(),
            next_id: 1,
        };
        for id in 1..=10_000 {
            notes.next_id = id + 1;
            notes.insert(
                Note {
                    id,
                    source: "Test".into(),
                    markdown: format!("# Note {id}\n- [ ] Item"),
                    version: 1,
                    updated: id,
                    origin: None,
                    name: None,
                    location: None,
                    file: None,
                },
                Some(id),
            );
        }
        assert_eq!(notes.latest(7_432), Some(7_432));
        assert_eq!(notes.list("", 0, None).items.len(), 40);
        assert!(notes.list("", 0, None).more);
        assert_eq!(notes.list("note 7432", 0, None).items[0].id, 7_432);
        assert_eq!(notes.search("note 7432", None, 0).attached[0].id, 7_432);
        assert!(notes.return_to_main(7_432));
        let offline = notes.search("note 7432", None, 0);
        assert!(offline.attached.is_empty());
        assert_eq!(offline.documents[0].id, 7_432);
        assert_eq!(notes.list("note 7432", 0, None).items[0].id, 7_432);
        assert_eq!(notes.latest(7_432), None);
        assert!(!notes.card(7_432).unwrap().attached);
        assert_eq!(notes.latest(7_433), Some(7_433));
    }

    #[test]
    fn legacy_inbox_migration_recovers_published_files_and_keeps_external_collisions() {
        let base = std::env::temp_dir().join(format!(
            "jot-inbox-migration-{}-{}",
            std::process::id(),
            timestamp()
        ));
        let inbox = base.join("其他");
        fs::create_dir_all(&inbox).unwrap();
        let legacy = |id, text: &str| Note {
            id,
            source: "会议".into(),
            markdown: text.into(),
            version: 3,
            updated: 123,
            name: Some("纪要".into()),
            origin: Some(42),
            location: None,
            file: None,
        };
        write_json(&base.join("1.json"), &legacy(1, "原文")).unwrap();
        write_json(&base.join("2.json"), &legacy(2, "旧稿")).unwrap();
        fs::write(inbox.join("纪要.md"), "外部文件").unwrap();
        // Jotiva terminated after linking the first document but before committing its path.
        fs::write(base.join(".1.inbox"), "原文").unwrap();
        fs::hard_link(base.join(".1.inbox"), inbox.join("纪要 (2).md")).unwrap();
        // Another editor changed the second published file during the interruption.
        fs::write(base.join(".2.inbox"), "外部改稿").unwrap();
        fs::hard_link(base.join(".2.inbox"), inbox.join("纪要 (3).md")).unwrap();
        fs::write(base.join("我的.inbox"), "其他文件").unwrap();
        fs::write(base.join("索引.json"), r#"{"custom":true}"#).unwrap();
        for _ in 0..2 {
            let mut notes = Notes::load(base.clone()).unwrap();
            let first = notes.note(1).unwrap();
            assert_eq!(
                (first.origin, first.version, first.source.as_str()),
                (Some(42), 3, "会议")
            );
            assert_eq!(
                first.location.unwrap().path,
                notes.inbox_directory().join("纪要 (2).md")
            );
            assert_eq!(notes.open_file(&inbox.join("纪要 (2).md")).unwrap(), 1);
            assert_eq!(notes.filing_text(2, 3), Some("旧稿"));
            assert_eq!(
                fs::read_to_string(notes.note(2).unwrap().disk_path().unwrap()).unwrap(),
                "旧稿"
            );
            assert_eq!(
                fs::read_to_string(inbox.join("纪要 (3).md")).unwrap(),
                "外部改稿"
            );
            assert_eq!(
                fs::read_to_string(inbox.join("纪要.md")).unwrap(),
                "外部文件"
            );
            assert!(base.join("我的.inbox").is_file());
            assert!(!base.join(".1.inbox").exists() && !base.join(".2.inbox").exists());
            assert_eq!(fs::read_dir(&inbox).unwrap().count(), 4);
        }
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn inbox_symlink_cannot_redirect_new_notes_at_runtime_or_on_restart() {
        let root =
            std::env::temp_dir().join(format!("jot-inbox-redirection-{}", std::process::id()));
        let library = root.join("notes");
        let redirection = root.join("temporary");
        fs::create_dir_all(&redirection).unwrap();
        let mut notes = Notes::load(library.clone()).unwrap();
        let id = notes.new_note(None, "Jotiva".into());
        let inbox = notes.inbox_directory();
        fs::rename(&inbox, root.join("original")).unwrap();
        crate::platform::test_links::directory(&redirection, &inbox).unwrap();
        assert!(notes.save(id, 0, "仍在编辑器".into()).is_err());
        assert!(!notes.metadata_path(id).exists());
        assert!(Notes::load(library).is_err());
        assert_eq!(fs::read_dir(redirection).unwrap().count(), 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn damaged_note_record_preserves_its_id_and_other_documents_stay_available() {
        let base = std::env::temp_dir().join(format!("jot-damaged-record-{}", std::process::id()));
        let mut notes = Notes::load(base.clone()).unwrap();
        let good = notes.new_note(None, "Jotiva".into());
        notes.save(good, 0, "完好的文件".into()).unwrap();
        fs::write(base.join("20.json"), b"broken").unwrap();
        let mut reopened = Notes::load(base.clone()).unwrap();
        assert_eq!(
            reopened.refresh_from_disk(good).unwrap().0.markdown,
            "完好的文件"
        );
        let new_id = reopened.new_note(None, "Jotiva".into());
        assert!(new_id > 20);
        reopened.save(new_id, 0, "新文件".into()).unwrap();
        assert_eq!(fs::read(base.join("20.json")).unwrap(), b"broken");
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn saved_draft_survives_restart_and_stale_write() {
        let folder =
            std::env::temp_dir().join(format!("jot-notes-{}-{}", std::process::id(), timestamp()));
        let mut notes = Notes::load(folder.clone()).unwrap();
        let id = notes.new_note(Some(42), "会议".into());
        assert_eq!(notes.save(id, 0, "# 会后\n- [ ] 核对".into()).unwrap(), 1);
        assert!(notes.save(id, 0, "旧内容".into()).is_err());
        notes.discard_empty(id);
        assert_eq!(notes.latest(42), Some(id));
        let restarted = Notes::load(folder.clone()).unwrap();
        assert_eq!(restarted.note(id).unwrap().markdown, "# 会后\n- [ ] 核对");
        assert_eq!(restarted.host(id), None);
        assert_eq!(restarted.card(id).unwrap().source, "会议");
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn discarded_empty_draft_releases_its_host_entry() {
        let mut notes = Notes {
            folder: PathBuf::new(),
            sources_file: PathBuf::new(),
            saved_sources: BTreeMap::new(),
            bindings: BTreeMap::new(),
            source_hosts: HashMap::new(),
            records: HashMap::new(),
            by_path: BTreeMap::new(),
            recent: BTreeSet::new(),
            by_host: HashMap::new(),
            by_origin: HashMap::new(),
            next_id: 1,
        };
        let id = notes.new_note(Some(42), "会议".into());
        assert_eq!(notes.latest(42), Some(id));
        assert!(notes.discard_empty(id));
        assert!(notes.by_host.is_empty());
        assert!(notes.recent.is_empty());
        assert!(notes.note(id).is_none());
    }

    #[test]
    fn moving_an_empty_quick_note_binds_the_same_draft_until_its_first_save() {
        let directory = std::env::temp_dir().join(format!("jot-empty-drag-{}", std::process::id()));
        let mut notes = Notes::load(directory.clone()).unwrap();
        let id = notes.new_note(Some(12), "会议 A".into());
        notes.attach_to_host(id, 24, "会议 B".into()).unwrap();
        assert_eq!(notes.latest(12), None);
        assert_eq!(notes.latest(24), Some(id));
        assert!(!notes.metadata_path(id).exists());
        assert_eq!(notes.disk_path(id), None);
        notes.save(id, 0, "改到 B 开会".into()).unwrap();
        let saved = Notes::load(directory.clone()).unwrap().note(id).unwrap();
        assert_eq!(saved.source, "会议 B");
        assert_eq!(
            fs::read_to_string(saved.disk_path().unwrap()).unwrap(),
            "改到 B 开会"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn disk_document_keeps_its_identity_permissions_and_conflict_guard() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;
        let data = std::env::temp_dir().join(format!("jot-original-{}", std::process::id()));
        let library = data.join("notes");
        let mut notes = Notes::load(library.clone()).unwrap();
        let path = data.join("meeting.md");
        fs::write(&path, "# 标题\r\n正文\r\n").unwrap();
        #[cfg(unix)]
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let id = notes.open_file(&path).unwrap();
        assert_eq!(notes.card(id).unwrap().title, "meeting.md");
        assert!(
            notes
                .list("", 0, Some(&Scope::Folder { id: None }))
                .items
                .is_empty()
        );
        notes.attach_to_host(id, 22, "会议 · 讨论".into()).unwrap();
        assert_eq!(notes.save(id, 1, "# 标题\n改动\n".into()).unwrap(), 2);
        assert_eq!(fs::read_to_string(&path).unwrap(), "# 标题\r\n改动\r\n");
        #[cfg(unix)]
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::write(&path, "外部修改").unwrap();
        assert!(notes.save(id, 2, "未保存编辑".into()).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "外部修改");
        let mut notes = Notes::load(library).unwrap();
        assert_eq!(notes.open_file(&data.join("./meeting.md")).unwrap(), id);
        assert_eq!(notes.note(id).unwrap().version, 3);
        let (note, changed) = notes.refresh_from_disk(id).unwrap();
        assert_eq!(note.markdown, "外部修改");
        assert!(!changed);
        fs::write(&path, "切回之前在别处修改").unwrap();
        let (note, changed) = notes.refresh_from_disk(id).unwrap();
        assert!(changed);
        assert_eq!(note.version, 4);
        assert_eq!(note.markdown, "切回之前在别处修改");
        assert!(!notes.refresh_from_disk(id).unwrap().1);
        assert!(notes.move_to(id, None).is_err());
        assert!(path.is_file());
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn opening_an_archived_path_reuses_the_archived_note() {
        let data = std::env::temp_dir().join(format!("jot-open-archive-{}", std::process::id()));
        let mut notes = Notes::load(data.join("notes")).unwrap();
        let id = notes.new_note(None, "Jotiva".into());
        notes.save(id, 0, "# 归档记录".into()).unwrap();
        let folder = Folder {
            id: 1,
            name: "归档".into(),
            classify: true,
            description: String::new(),
            path: fs::canonicalize(&data).unwrap(),
        };
        let file = notes.move_to(id, Some(&folder)).unwrap().path.unwrap();
        assert_eq!(notes.open_file(&file).unwrap(), id);
        assert!(notes.note(id).unwrap().file.is_none());
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn moving_and_editing_writes_markdown_without_overwriting_other_files() {
        let data =
            std::env::temp_dir().join(format!("jot-move-{}-{}", std::process::id(), timestamp()));
        let destination = data.join("研究");
        fs::create_dir_all(&destination).unwrap();
        let untouched = destination.join("任务.md");
        fs::write(&untouched, "已有的文件").unwrap();
        let folder = Folder {
            id: 1,
            name: "研究".into(),
            classify: true,
            description: "研究任务".into(),
            path: fs::canonicalize(destination).unwrap(),
        };
        let mut notes = Notes::load(data.join("notes")).unwrap();
        let id = notes.new_note(Some(5), "会议 · 总结".into());
        assert_eq!(notes.save(id, 0, "# 任务".into()).unwrap(), 1);
        assert_eq!(
            notes
                .list("", 0, Some(&Scope::Folder { id: None }))
                .items
                .len(),
            1
        );
        let saved = notes.move_to(id, Some(&folder)).unwrap().path.unwrap();
        assert_eq!(fs::read_to_string(&untouched).unwrap(), "已有的文件");
        assert_eq!(fs::read_to_string(&saved).unwrap(), "# 任务");
        assert_eq!(notes.save(id, 1, "# 任务\n更新".into()).unwrap(), 2);
        assert_eq!(fs::read_to_string(&saved).unwrap(), "# 任务\n更新");
        let mut notes = Notes::load(data.join("notes")).unwrap();
        assert_eq!(
            notes
                .list("", 0, Some(&Scope::Folder { id: Some(1) }))
                .items[0]
                .id,
            id
        );
        let moved = notes.move_to(id, None).unwrap();
        assert!(moved.warning.is_none() && !saved.exists());
        assert_eq!(notes.note(id).unwrap().markdown, "# 任务\n更新");
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn distinct_windows_keep_separate_sources_after_detaching() {
        let data = std::env::temp_dir().join(format!(
            "jot-sources-{}-{}",
            std::process::id(),
            timestamp()
        ));
        let mut notes = Notes::load(data.clone()).unwrap();
        let a = notes.new_note(Some(1), "浏览器 · 文档".into());
        let b = notes.new_note(Some(1), "浏览器 · 文档".into());
        let c = notes.new_note(Some(2), "浏览器 · 文档".into());
        for id in [a, b, c] {
            notes.save(id, 0, format!("# 记录 {id}")).unwrap();
        }
        notes.return_to_main(1);
        let sources = notes.sources();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].windows.len(), 2);
        let key = sources[0].windows[0].key.clone();
        let page = notes.list("", 0, Some(&Scope::Source { key }));
        assert_eq!(page.items.len(), 2);
        assert!(page.items.iter().all(|card| !card.attached));
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn registered_source_stays_available_without_a_live_window() {
        let data = std::env::temp_dir().join(format!(
            "jot-register-{}-{}",
            std::process::id(),
            timestamp()
        ));
        let mut notes = Notes::load(data.clone()).unwrap();
        let key = notes.register_source(22, "会议 · 项目评审".into()).unwrap();
        assert_eq!(notes.sources()[0].windows[0].key, key);
        assert_eq!(
            notes
                .register_source(22, "会议 · 修改后的标题".into())
                .unwrap(),
            key
        );
        let id = notes.new_at_source(&key).unwrap();
        assert_eq!(notes.host(id), Some(22));
        assert!(notes.return_to_main(22));
        assert!(notes.discard_empty(id));
        let mut notes = Notes::load(data.clone()).unwrap();
        let offline = notes.new_at_source(&key).unwrap();
        notes.save(offline, 0, "会后补记".into()).unwrap();
        assert_eq!(notes.host(offline), None);
        assert_eq!(notes.sources()[0].windows[0].key, key);
        let page = notes.list("", 0, Some(&Scope::Source { key }));
        assert_eq!(page.items[0].id, offline);
        fs::remove_dir_all(data).unwrap();
    }

    #[test]
    fn an_archived_note_can_follow_a_new_window_without_rewriting_the_file() {
        let data =
            std::env::temp_dir().join(format!("jot-attach-{}-{}", std::process::id(), timestamp()));
        let library = data.join("notes");
        let mut notes = Notes::load(library.clone()).unwrap();
        let first = notes.new_note(Some(21), "会议 · 朝会".into());
        notes.save(first, 0, "# 会后跟进".into()).unwrap();
        let sibling = notes.new_note(Some(21), "会议 · 朝会".into());
        notes.save(sibling, 0, "# 会议议程".into()).unwrap();
        let folder = Folder {
            id: 1,
            name: "会议".into(),
            classify: true,
            description: String::new(),
            path: fs::canonicalize(&data).unwrap(),
        };
        let path = notes.move_to(first, Some(&folder)).unwrap().path.unwrap();
        let original = notes.records[&first].source_key.clone();
        let target = notes.register_source(22, "浏览器 · 方案".into()).unwrap();

        assert_eq!(
            notes
                .attach_to_host(first, 22, "浏览器 · 新标题".into())
                .unwrap(),
            target
        );
        assert_eq!(notes.latest(21), Some(sibling));
        assert_eq!(notes.latest(22), Some(first));
        assert!(
            notes
                .list("", 0, Some(&Scope::Source { key: original }))
                .items
                .iter()
                .all(|card| card.id == sibling)
        );
        assert_eq!(notes.note(first).unwrap().version, 1);
        assert_eq!(fs::read_to_string(&path).unwrap(), "# 会后跟进");
        assert!(
            notes
                .sources()
                .iter()
                .all(|group| group.windows[0].attached)
        );

        assert!(notes.return_to_main(22));
        assert!(!notes.card(first).unwrap().attached);
        let mut notes = Notes::load(library).unwrap();
        assert_eq!(notes.card(first).unwrap().source, "浏览器 · 方案");
        assert_eq!(notes.note(first).unwrap().location.unwrap().path, path);
        assert!(
            notes
                .sources()
                .iter()
                .all(|group| !group.windows[0].attached)
        );
        let new_source = notes
            .attach_to_host(first, 31, "浏览器 · 方案".into())
            .unwrap();
        assert_ne!(target, new_source);
        assert_eq!(notes.host(first), Some(31));
        assert_eq!(notes.note(first).unwrap().markdown, "# 会后跟进");
        fs::remove_dir_all(data).unwrap();
    }
}
