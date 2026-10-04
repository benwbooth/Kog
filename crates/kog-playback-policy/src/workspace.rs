//! Saved-playlist tabs are drafts. Only an explicit queue action can affect playback.
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const QUEUE_TAB: &str = "queue";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueAction {
    PlayNow,
    PlayNext,
    AddToQueue,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Row {
    id: u64,
    entry: Value,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Draft {
    rows: Vec<Row>,
    selected: Vec<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PendingSave {
    revision: u64,
    rows: Vec<Row>,
    close: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Tab {
    key: String,
    scope: String,
    playlist_id: i64,
    name: String,
    readonly: bool,
    draft: Draft,
    saved: Vec<Row>,
    undo: Vec<Draft>,
    redo: Vec<Draft>,
    revision: u64,
    loading: Option<u64>,
    saving: Option<PendingSave>,
    error: Option<String>,
}
impl Tab {
    fn dirty(&self) -> bool {
        self.draft.rows != self.saved
    }
    fn edit(&mut self) -> Result<(), String> {
        if self.readonly {
            return Err("Favorites is read-only; use the star controls to change it".into());
        }
        if self.loading.is_some() {
            return Err("The playlist is still loading".into());
        }
        self.undo.push(self.draft.clone());
        if self.undo.len() > 50 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.revision += 1;
        self.error = None;
        Ok(())
    }
    fn selected_indices(&self) -> Vec<usize> {
        self.draft
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, row)| self.draft.selected.contains(&row.id).then_some(i))
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
    tabs: Vec<Tab>,
    active: String,
    serial: u64,
    pending_close: Option<String>,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            tabs: vec![],
            active: QUEUE_TAB.into(),
            serial: 0,
            pending_close: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command {
    Open {
        key: String,
        scope: String,
        playlist_id: i64,
        name: String,
        #[serde(default)]
        readonly: bool,
    },
    Loaded {
        key: String,
        generation: u64,
        entries: Vec<Value>,
    },
    LoadFailed {
        key: String,
        generation: u64,
        error: String,
    },
    Focus {
        key: String,
    },
    Select {
        indices: Vec<usize>,
    },
    Append {
        entries: Vec<Value>,
    },
    Remove,
    Nudge {
        delta: i32,
    },
    Sort {
        rows: Vec<crate::sort::SortRow>,
        column: String,
        descending: bool,
    },
    Undo,
    Redo,
    Reload,
    Save,
    Saved {
        key: String,
        revision: u64,
    },
    SaveFailed {
        key: String,
        revision: u64,
        error: String,
    },
    Close {
        key: String,
    },
    ResolveClose {
        choice: CloseChoice,
    },
    Queue {
        action: QueueAction,
    },
    Renamed {
        key: String,
        name: String,
    },
    Deleted {
        key: String,
    },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseChoice {
    Save,
    Discard,
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Effect {
    None,
    Load {
        key: String,
        scope: String,
        playlist_id: i64,
        generation: u64,
    },
    Save {
        key: String,
        scope: String,
        playlist_id: i64,
        revision: u64,
        entries: Vec<Value>,
        expected_entries: Vec<Value>,
    },
    Queue {
        mode: QueueAction,
        entries: Vec<Value>,
        scope: String,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct TabSnapshot {
    pub key: String,
    pub scope: String,
    pub playlist_id: i64,
    pub name: String,
    pub dirty: bool,
    pub readonly: bool,
    pub loading: bool,
    pub saving: bool,
    pub count: usize,
}
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub active: String,
    pub tabs: Vec<TabSnapshot>,
    pub entries: Vec<Value>,
    pub selected: Vec<usize>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub pending_close: Option<String>,
    pub error: Option<String>,
}

impl Workspace {
    pub fn snapshot(&self) -> Snapshot {
        let active = self.tabs.iter().find(|t| t.key == self.active);
        let mut tabs = vec![TabSnapshot {
            key: QUEUE_TAB.into(),
            scope: String::new(),
            playlist_id: -1,
            name: "Play Queue".into(),
            dirty: false,
            readonly: false,
            loading: false,
            saving: false,
            count: 0,
        }];
        tabs.extend(self.tabs.iter().map(|t| TabSnapshot {
            key: t.key.clone(),
            scope: t.scope.clone(),
            playlist_id: t.playlist_id,
            name: t.name.clone(),
            dirty: t.dirty(),
            readonly: t.readonly,
            loading: t.loading.is_some(),
            saving: t.saving.is_some(),
            count: t.draft.rows.len(),
        }));
        Snapshot {
            active: self.active.clone(),
            tabs,
            entries: active
                .map(|t| t.draft.rows.iter().map(|r| r.entry.clone()).collect())
                .unwrap_or_default(),
            selected: active.map(Tab::selected_indices).unwrap_or_default(),
            can_undo: active.is_some_and(|t| !t.undo.is_empty()),
            can_redo: active.is_some_and(|t| !t.redo.is_empty()),
            pending_close: self.pending_close.clone(),
            error: active.and_then(|t| t.error.clone()),
        }
    }
    pub fn restore(value: Value) -> Result<Self, String> {
        let mut workspace: Self = serde_json::from_value(value).map_err(|e| e.to_string())?;
        workspace.pending_close = None;
        for tab in &mut workspace.tabs {
            if tab.loading.take().is_some() {
                tab.error = Some("Loading was interrupted. Reload this playlist.".into());
            }
            tab.saving = None;
        }
        if !workspace.tabs.iter().any(|t| t.key == workspace.active) {
            workspace.active = QUEUE_TAB.into();
        }
        Ok(workspace)
    }
    fn active_mut(&mut self) -> Result<&mut Tab, String> {
        self.tabs
            .iter_mut()
            .find(|t| t.key == self.active)
            .ok_or_else(|| "Choose a saved playlist tab".into())
    }
    fn rows(&mut self, entries: Vec<Value>) -> Vec<Row> {
        entries
            .into_iter()
            .map(|entry| {
                self.serial += 1;
                Row {
                    id: self.serial,
                    entry,
                }
            })
            .collect()
    }
    fn close(&mut self, key: &str) {
        if let Some(index) = self.tabs.iter().position(|t| t.key == key) {
            self.tabs.remove(index);
            if self.active == key {
                self.active = if index > 0 {
                    self.tabs[index - 1].key.clone()
                } else {
                    QUEUE_TAB.into()
                };
            }
        }
        if self.pending_close.as_deref() == Some(key) {
            self.pending_close = None;
        }
    }
    fn save(&mut self, key: &str, close: bool) -> Result<Effect, String> {
        self.serial += 1;
        let revision = self.serial;
        let tab = self
            .tabs
            .iter_mut()
            .find(|t| t.key == key)
            .ok_or("Playlist tab no longer exists")?;
        if tab.readonly {
            return Err("This playlist is read-only".into());
        }
        if tab.loading.is_some() {
            return Err("The playlist is still loading".into());
        }
        if tab.saving.is_some() {
            return Err("A save is already in progress".into());
        }
        let entries = tab.draft.rows.iter().map(|r| r.entry.clone()).collect();
        tab.saving = Some(PendingSave {
            revision,
            rows: tab.draft.rows.clone(),
            close,
        });
        tab.error = None;
        let expected_entries = tab.saved.iter().map(|r| r.entry.clone()).collect();
        Ok(Effect::Save {
            key: tab.key.clone(),
            scope: tab.scope.clone(),
            playlist_id: tab.playlist_id,
            revision,
            entries,
            expected_entries,
        })
    }
    pub fn apply(&mut self, command: Command) -> Result<Effect, String> {
        use Command::*;
        match command {
            Open {
                key,
                scope,
                playlist_id,
                name,
                readonly,
            } => {
                if key == QUEUE_TAB || key.is_empty() {
                    return Err("Invalid playlist tab key".into());
                }
                self.active = key.clone();
                if self.tabs.iter().any(|t| t.key == key) {
                    return Ok(Effect::None);
                }
                self.serial += 1;
                let generation = self.serial;
                self.tabs.push(Tab {
                    key: key.clone(),
                    scope: scope.clone(),
                    playlist_id,
                    name,
                    readonly: readonly || playlist_id == 0,
                    draft: Draft::default(),
                    saved: vec![],
                    undo: vec![],
                    redo: vec![],
                    revision: 0,
                    loading: Some(generation),
                    saving: None,
                    error: None,
                });
                return Ok(Effect::Load {
                    key,
                    scope,
                    playlist_id,
                    generation,
                });
            }
            Loaded {
                key,
                generation,
                entries,
            } => {
                if self
                    .tabs
                    .iter()
                    .any(|t| t.key == key && t.loading == Some(generation))
                {
                    let rows = self.rows(entries);
                    let tab = self.tabs.iter_mut().find(|t| t.key == key).unwrap();
                    tab.saved = rows.clone();
                    tab.draft = Draft {
                        rows,
                        selected: vec![],
                    };
                    tab.undo.clear();
                    tab.redo.clear();
                    tab.loading = None;
                    tab.error = None;
                }
            }
            LoadFailed {
                key,
                generation,
                error,
            } => {
                if let Some(tab) = self
                    .tabs
                    .iter_mut()
                    .find(|t| t.key == key && t.loading == Some(generation))
                {
                    tab.loading = None;
                    tab.error = Some(error);
                }
            }
            Focus { key } => {
                if key != QUEUE_TAB && !self.tabs.iter().any(|t| t.key == key) {
                    return Err("Playlist tab no longer exists".into());
                }
                self.active = key;
            }
            Select { indices } => {
                let tab = self.active_mut()?;
                tab.draft.selected = tab
                    .draft
                    .rows
                    .iter()
                    .enumerate()
                    .filter_map(|(i, r)| indices.contains(&i).then_some(r.id))
                    .collect();
            }
            Append { entries } => {
                if entries.is_empty() {
                    return Ok(Effect::None);
                }
                self.active_mut()?.edit()?;
                let rows = self.rows(entries);
                let tab = self.active_mut()?;
                tab.draft.selected = rows.iter().map(|r| r.id).collect();
                tab.draft.rows.extend(rows);
            }
            Remove => {
                let tab = self.active_mut()?;
                if tab.draft.selected.is_empty() {
                    return Ok(Effect::None);
                }
                tab.edit()?;
                tab.draft
                    .rows
                    .retain(|r| !tab.draft.selected.contains(&r.id));
                tab.draft.selected.clear();
            }
            Nudge { delta } => {
                let tab = self.active_mut()?;
                let indices = tab.selected_indices();
                if indices.is_empty() || delta == 0 {
                    return Ok(Effect::None);
                }
                let mut next = tab.draft.rows.clone();
                if delta < 0 {
                    for i in indices {
                        if i > 0 && !tab.draft.selected.contains(&next[i - 1].id) {
                            next.swap(i, i - 1);
                        }
                    }
                } else {
                    for i in indices.into_iter().rev() {
                        if i + 1 < next.len() && !tab.draft.selected.contains(&next[i + 1].id) {
                            next.swap(i, i + 1);
                        }
                    }
                }
                if next != tab.draft.rows {
                    tab.edit()?;
                    tab.draft.rows = next;
                }
            }
            Sort {
                rows,
                column,
                descending,
            } => {
                let tab = self.active_mut()?;
                if rows.len() != tab.draft.rows.len() {
                    return Err("Playlist changed before sorting".into());
                }
                let order = crate::sort::sorted_rows(&rows, &column, descending);
                let next: Vec<_> = order
                    .into_iter()
                    .map(|i| tab.draft.rows[i].clone())
                    .collect();
                if next != tab.draft.rows {
                    tab.edit()?;
                    tab.draft.rows = next;
                }
            }
            Undo | Redo => {
                let redo = matches!(command, Redo);
                let tab = self.active_mut()?;
                let snapshot = if redo { tab.redo.pop() } else { tab.undo.pop() };
                if let Some(snapshot) = snapshot {
                    let old = std::mem::replace(&mut tab.draft, snapshot);
                    if redo {
                        tab.undo.push(old);
                    } else {
                        tab.redo.push(old);
                    }
                    tab.revision += 1;
                    tab.error = None;
                }
            }
            Reload => {
                if self.active_mut()?.dirty() {
                    return Err("Save or undo the draft before reloading".into());
                }
                self.serial += 1;
                let generation = self.serial;
                let tab = self.active_mut()?;
                tab.loading = Some(generation);
                tab.error = None;
                return Ok(Effect::Load {
                    key: tab.key.clone(),
                    scope: tab.scope.clone(),
                    playlist_id: tab.playlist_id,
                    generation,
                });
            }
            Save => return self.save(&self.active.clone(), false),
            Saved { key, revision } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.key == key) {
                    if tab.saving.as_ref().is_some_and(|s| s.revision == revision) {
                        let save = tab.saving.take().unwrap();
                        tab.saved = save.rows;
                        if save.close && !tab.dirty() {
                            self.close(&key);
                        }
                    }
                }
            }
            SaveFailed {
                key,
                revision,
                error,
            } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| {
                    t.key == key && t.saving.as_ref().is_some_and(|s| s.revision == revision)
                }) {
                    tab.saving = None;
                    tab.error = Some(error);
                }
            }
            Close { key } => {
                if key == QUEUE_TAB {
                    return Ok(Effect::None);
                }
                if self.tabs.iter().any(|t| t.key == key && t.dirty()) {
                    self.pending_close = Some(key);
                } else {
                    self.close(&key);
                }
            }
            ResolveClose { choice } => {
                if let Some(key) = self.pending_close.take() {
                    match choice {
                        CloseChoice::Cancel => {}
                        CloseChoice::Discard => self.close(&key),
                        CloseChoice::Save => return self.save(&key, true),
                    }
                }
            }
            Queue { action } => {
                let tab = self.active_mut()?;
                if tab.loading.is_some() {
                    return Err("The playlist is still loading".into());
                }
                let entries = tab
                    .draft
                    .rows
                    .iter()
                    .filter(|r| tab.draft.selected.is_empty() || tab.draft.selected.contains(&r.id))
                    .map(|r| r.entry.clone())
                    .collect();
                return Ok(Effect::Queue {
                    mode: action,
                    entries,
                    scope: tab.scope.clone(),
                });
            }
            Renamed { key, name } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.key == key) {
                    tab.name = name;
                }
            }
            Deleted { key } => {
                if let Some(tab) = self.tabs.iter_mut().find(|t| t.key == key) {
                    tab.error = Some(
                        "This playlist was deleted. Its draft is still available to queue or copy."
                            .into(),
                    );
                }
            }
        }
        Ok(Effect::None)
    }
}
