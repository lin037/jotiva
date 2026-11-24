use crate::host::SourceChoice;
use std::collections::{HashMap, HashSet};

// IDs belong to this run. Remove destroyed windows before the OS can reuse a handle.
pub(crate) struct Registry<W> {
    next: u64,
    windows: HashMap<u64, W>,
    choices: HashMap<String, Vec<u64>>,
    watched: HashSet<u64>,
}

impl<W> Default for Registry<W> {
    fn default() -> Self {
        Self {
            next: 0,
            windows: HashMap::new(),
            choices: HashMap::new(),
            watched: HashSet::new(),
        }
    }
}

impl<W: PartialEq + Copy> Registry<W> {
    fn issue(&mut self, w: W) -> u64 {
        if let Some((&id, _)) = self.windows.iter().find(|(_, v)| **v == w) {
            return id;
        }
        self.next += 1;
        self.windows.insert(self.next, w);
        self.next
    }
    pub(crate) fn watch(&mut self, w: W) -> u64 {
        let id = self.issue(w);
        self.watched.insert(id);
        id
    }
    pub fn get(&self, id: u64) -> Option<W> {
        self.windows.get(&id).copied()
    }
    pub fn watched_id(&self, window: W) -> Option<u64> {
        self.watched
            .iter()
            .copied()
            .find(|id| self.get(*id) == Some(window))
    }
    pub fn unwatch(&mut self, id: u64) {
        self.watched.remove(&id);
        self.prune();
    }
    pub fn clear(&mut self, owner: &str) {
        self.choices.remove(owner);
        self.prune();
    }
    pub fn choices(&mut self, owner: &str, rows: Vec<(W, String, String)>) -> Vec<SourceChoice> {
        let rows: Vec<_> = rows
            .into_iter()
            .map(|(w, app, title)| SourceChoice {
                choice: self.issue(w),
                app,
                title,
            })
            .collect();
        self.choices
            .insert(owner.to_owned(), rows.iter().map(|r| r.choice).collect());
        self.prune();
        rows
    }
    pub fn transient(&mut self, w: W) -> u64 {
        let id = self.issue(w);
        self.choices.insert(String::new(), vec![id]);
        self.prune();
        id
    }
    pub fn destroy(&mut self, matches: impl Fn(W) -> bool) -> Option<u64> {
        let id = self
            .windows
            .iter()
            .find_map(|(&id, &window)| matches(window).then_some(id))?;
        self.windows.remove(&id);
        self.watched.remove(&id).then_some(id)
    }
    fn prune(&mut self) {
        self.windows.retain(|id, _| {
            self.watched.contains(id) || self.choices.values().any(|ids| ids.contains(id))
        });
    }
}
