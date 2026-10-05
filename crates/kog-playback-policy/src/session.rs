//! The application session. Frontends own ports and views, never queue mutations
//! or asynchronous command ordering. Multiple instances are independent.
use crate::{
    NavigationEvent, OrderTrack, PlaybackDecision, PlaybackOrder, RepeatMode, ShuffleMode,
};
use crate::{radio::RadioBuffer, selection::Selection, sort::SortRow, workspace};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};

/// A platform may attach resolved decoder data to an entry. Only its durable
/// locator and metadata cross the persistence boundary.
pub trait Item: Clone {
    fn entry(&self) -> Value;
    fn metadata(&self) -> SortRow;
}

impl Item for Value {
    fn entry(&self) -> Value {
        self.clone()
    }
    fn metadata(&self) -> SortRow {
        metadata_from_json(self)
    }
}

/// Common projection for the HTTP, Swift and Kotlin track representations.
pub fn metadata_from_json(value: &Value) -> SortRow {
    let text = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .or_else(|| {
                value
                    .get("metadata")
                    .and_then(|m| m.get(key))
                    .and_then(Value::as_str)
            })
            .unwrap_or_default()
            .to_owned()
    };
    let number = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_f64)
            .or_else(|| {
                value
                    .get("metadata")
                    .and_then(|m| m.get(key))
                    .and_then(Value::as_f64)
            })
            .or_else(|| {
                text(key)
                    .split('/')
                    .next()
                    .and_then(|s| s.parse::<f64>().ok())
            })
    };
    let path = text("path");
    let entry = text("entry");
    let filename = if entry.is_empty() { &path } else { &entry }
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .to_owned();
    let title = [text("title"), text("name"), filename.clone()]
        .into_iter()
        .find(|s| !s.is_empty())
        .unwrap_or_default();
    SortRow {
        original: number("queueOrder"),
        title,
        artist: text("artist"),
        album: text("album"),
        album_artist: text("albumArtist"),
        composer: text("composer"),
        genre: text("genre"),
        year: number("year"),
        disc_number: number("discNumber").or_else(|| number("disc_number")),
        track_number: number("trackNumber").or_else(|| number("track_number")),
        duration: number("duration").filter(|v| *v > 0.0).map(|v| v / 1000.0),
        file_size_bytes: number("fileSizeBytes"),
        sample_rate: number("sampleRate"),
        bits_per_sample: number("bitsPerSample"),
        bitrate: number("bitrate"),
        channels: number("channels"),
        codec: text("codec"),
        path: if entry.is_empty() {
            path
        } else {
            format!("{path}/{entry}")
        },
        filename,
        star: value["star"].as_bool().unwrap_or(false),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Token {
    pub session: String,
    pub incarnation: u64,
    pub serial: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    #[default]
    Stopped,
    Starting,
    Playing,
    Paused,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Command<T> {
    Append {
        tracks: Vec<T>,
        #[serde(default)]
        action: workspace::QueueAction,
    },
    Replace {
        tracks: Vec<T>,
        current: Option<usize>,
    },
    Insert {
        index: usize,
        tracks: Vec<T>,
    },
    ReplaceRange {
        start: usize,
        end: usize,
        tracks: Vec<T>,
    },
    ReloadOutput,
    Remove {
        indices: Vec<usize>,
    },
    Move {
        indices: Vec<usize>,
        target: usize,
    },
    Reorder {
        indices: Vec<usize>,
    },
    Clear,
    UpdateItem {
        index: usize,
        track: T,
    },
    Metadata {
        rows: Vec<SortRow>,
    },
    Select {
        command: crate::selection::Command,
    },
    Filter {
        query: String,
    },
    Sort {
        column: String,
        descending: bool,
        #[serde(default)]
        physical: bool,
    },
    Workspace {
        command: workspace::Command,
    },
    WorkspaceRestore {
        value: Value,
    },
    AppendQueueToWorkspace {
        #[serde(default)]
        selected_only: bool,
    },
    Expand {
        scope: String,
        entries: Vec<Value>,
        #[serde(default)]
        action: workspace::QueueAction,
    },
    Collect {
        scope: String,
        path: String,
        query: String,
        root: String,
        #[serde(default)]
        action: workspace::QueueAction,
    },
    Complete {
        token: Token,
        result: IoResult<T>,
    },
    Play {
        index: usize,
    },
    Activate {
        index: usize,
    },
    Toggle,
    Pause,
    Resume,
    Stop,
    Seek {
        seconds: f64,
    },
    Volume {
        value: f64,
    },
    Navigate {
        event: NavigationEvent,
    },
    Output {
        token: Token,
        event: OutputEvent,
    },
    Shuffle {
        mode: ShuffleMode,
    },
    Repeat {
        mode: RepeatMode,
    },
    CycleShuffle,
    CycleRepeat,
    ToggleQueued {
        indices: Vec<usize>,
    },
    ToggleStopAfter {
        indices: Vec<usize>,
    },
    ClearQueued,
    Radio {
        enabled: bool,
        scope: String,
        root: String,
        #[serde(default)]
        reshuffle: bool,
    },
    RefillRadio,
    Scopes {
        scopes: Vec<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IoResult<T> {
    Loaded { entries: Vec<Value> },
    Saved,
    Expanded { tracks: Vec<T> },
    Radio { tracks: Vec<T>, exhausted: bool },
    Failed { error: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum OutputEvent {
    Started,
    Failed { error: String },
    Ended,
    Progress { seconds: f64, duration: f64 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Effect {
    Load {
        token: Token,
        scope: String,
        playlist_id: i64,
    },
    Save {
        token: Token,
        scope: String,
        playlist_id: i64,
        entries: Vec<Value>,
        expected_entries: Vec<Value>,
    },
    Expand {
        token: Token,
        scope: String,
        entries: Vec<Value>,
    },
    Collect {
        token: Token,
        scope: String,
        path: String,
        query: String,
        root: String,
    },
    Radio {
        token: Token,
        scope: String,
        root: String,
        enabled: bool,
        reshuffle: bool,
        reset: bool,
    },
    Play {
        token: Token,
        index: usize,
        seconds: f64,
        playing: bool,
    },
    Pause,
    Resume,
    Stop,
    Seek {
        seconds: f64,
    },
    Volume {
        value: f64,
    },
    QueueChanged {
        old_to_new: Vec<Option<usize>>,
    },
    Persist {
        value: Value,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum Pending<T> {
    Load {
        key: String,
        generation: u64,
        scope: String,
    },
    Save {
        key: String,
        revision: u64,
        scope: String,
    },
    Expand {
        action: workspace::QueueAction,
        transport_epoch: u64,
        scope: String,
        ready: Option<Result<Vec<T>, String>>,
    },
    Radio {
        generation: u64,
        scope: String,
    },
}

impl<T> Pending<T> {
    fn scope(&self) -> &str {
        match self {
            Self::Load { scope, .. }
            | Self::Save { scope, .. }
            | Self::Expand { scope, .. }
            | Self::Radio { scope, .. } => scope,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct OutputRequest {
    token: Token,
    row: u64,
    previous: Option<u64>,
    automatic: bool,
    #[serde(default)]
    started: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Session<T> {
    id: String,
    incarnation: u64,
    serial: u64,
    revision: u64,
    next_row: u64,
    rows: Vec<u64>,
    queue: Vec<T>,
    metadata: Vec<SortRow>,
    current: Option<usize>,
    transport: Transport,
    position: f64,
    duration: f64,
    volume: f64,
    selection: Selection,
    order: PlaybackOrder,
    workspace: workspace::Workspace,
    filter: String,
    sort_column: String,
    descending: bool,
    physical_sort: bool,
    visible: Vec<usize>,
    pending: BTreeMap<u64, Pending<T>>,
    expansion_order: VecDeque<u64>,
    output: Option<OutputRequest>,
    transport_epoch: u64,
    radio: RadioBuffer<T>,
    radio_scope: String,
    radio_root: String,
    scopes: Vec<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Snapshot<'a, T> {
    pub session_id: &'a str,
    pub revision: u64,
    pub queue: &'a [T],
    pub row_ids: &'a [u64],
    pub current: Option<usize>,
    pub transport: Transport,
    pub position: f64,
    pub duration: f64,
    pub volume: f64,
    pub selection: &'a Selection,
    pub shuffle: ShuffleMode,
    pub repeat: RepeatMode,
    pub queued: &'a [usize],
    pub stop_after: Vec<usize>,
    pub workspace: workspace::Snapshot,
    pub filter: &'a str,
    pub sort_column: &'a str,
    pub descending: bool,
    pub visible: &'a [usize],
    pub output_token: Option<&'a Token>,
    pub radio_enabled: bool,
    pub radio_waiting: bool,
    pub radio_pending: bool,
    pub radio_ready: usize,
    pub error: Option<&'a str>,
}

impl<T: Item> Session<T> {
    pub fn new(
        id: impl Into<String>,
        incarnation: u64,
        shuffle: ShuffleMode,
        repeat: RepeatMode,
    ) -> Self {
        Self {
            id: id.into(),
            incarnation,
            serial: 0,
            revision: 0,
            next_row: 1,
            rows: vec![],
            queue: vec![],
            metadata: vec![],
            current: None,
            transport: Transport::Stopped,
            position: 0.0,
            duration: 0.0,
            volume: 1.0,
            selection: Selection::default(),
            order: PlaybackOrder::new(shuffle, repeat, incarnation),
            workspace: workspace::Workspace::default(),
            filter: String::new(),
            sort_column: "index".into(),
            descending: false,
            physical_sort: true,
            visible: vec![],
            pending: BTreeMap::new(),
            expansion_order: VecDeque::new(),
            output: None,
            transport_epoch: 0,
            radio: RadioBuffer::default(),
            radio_scope: String::new(),
            radio_root: String::new(),
            scopes: vec![],
            error: None,
        }
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn queue(&self) -> &[T] {
        &self.queue
    }
    pub fn current(&self) -> Option<usize> {
        self.current
    }
    pub fn order(&self) -> &PlaybackOrder {
        &self.order
    }
    pub fn selection(&self) -> &Selection {
        &self.selection
    }
    pub fn visible(&self) -> &[usize] {
        &self.visible
    }
    pub fn workspace_model(&self) -> &workspace::Workspace {
        &self.workspace
    }
    pub fn workspace(&self) -> workspace::Snapshot {
        self.workspace
            .snapshot_for(self.queue.len(), self.selection.indices.len())
    }
    pub fn snapshot(&self) -> Snapshot<'_, T> {
        let mut stop_after: Vec<_> = self.order.stop_after_indices().collect();
        stop_after.sort_unstable();
        Snapshot {
            session_id: &self.id,
            revision: self.revision,
            queue: &self.queue,
            row_ids: &self.rows,
            current: self.current,
            transport: self.transport,
            position: self.position,
            duration: self.duration,
            volume: self.volume,
            selection: &self.selection,
            shuffle: self.order.shuffle_mode(),
            repeat: self.order.repeat_mode(),
            queued: self.order.queued_indices(),
            stop_after,
            workspace: self.workspace(),
            filter: &self.filter,
            sort_column: &self.sort_column,
            descending: self.descending,
            visible: &self.visible,
            output_token: self.output.as_ref().map(|o| &o.token),
            radio_enabled: self.radio.enabled(),
            radio_waiting: self.radio.waiting(),
            radio_pending: self.radio.pending(),
            radio_ready: self.radio.ready_len(),
            error: self.error.as_deref(),
        }
    }
    fn token(&mut self) -> Token {
        self.serial += 1;
        Token {
            session: self.id.clone(),
            incarnation: self.incarnation,
            serial: self.serial,
        }
    }
    fn order_tracks(&self) -> Vec<OrderTrack> {
        self.metadata
            .iter()
            .map(|m| OrderTrack {
                album: m.album.clone(),
                disc_number: m.disc_number.map(|n| n as u32),
                track_number: m.track_number.map(|n| n as u32),
            })
            .collect()
    }
    fn refresh(&mut self) {
        for (i, row) in self.metadata.iter_mut().enumerate() {
            row.original = Some(self.order.original_position(i) as f64);
        }
        let sorted = if self.physical_sort {
            (0..self.queue.len()).collect()
        } else {
            crate::sort::sorted_rows(&self.metadata, &self.sort_column, self.descending)
        };
        self.order.set_sequence(sorted.clone(), self.queue.len());
        self.visible = sorted
            .into_iter()
            .filter(|i| self.metadata[*i].matches(&self.filter))
            .collect();
    }
    fn changed(&mut self, old_to_new: Vec<Option<usize>>, effects: &mut Vec<Effect>) {
        self.order.remap_tracks(&old_to_new);
        self.selection.remap(&old_to_new);
        self.current = self
            .current
            .and_then(|i| old_to_new.get(i).copied().flatten());
        self.order
            .tracks_changed(&self.order_tracks(), self.current);
        self.refresh();
        effects.push(Effect::QueueChanged { old_to_new });
    }
    fn append(
        &mut self,
        tracks: Vec<T>,
        action: workspace::QueueAction,
        effects: &mut Vec<Effect>,
    ) {
        if tracks.is_empty() {
            return;
        }
        let start = self.queue.len();
        let count = tracks.len();
        for track in tracks {
            self.rows.push(self.next_row);
            self.next_row += 1;
            self.metadata.push(track.metadata());
            self.queue.push(track);
        }
        self.changed((0..start).map(Some).collect(), effects);
        if let Some(PlaybackDecision::Play(index)) =
            self.order.apply_queue_action(action, start, count)
        {
            self.play(index, false, effects);
        }
    }
    fn reorder(&mut self, order: Vec<usize>, effects: &mut Vec<Effect>) {
        if order.len() != self.queue.len() {
            self.error = Some("Invalid queue permutation".into());
            return;
        }
        let mut check = order.clone();
        check.sort_unstable();
        check.dedup();
        if check != (0..self.queue.len()).collect::<Vec<_>>() {
            self.error = Some("Invalid queue permutation".into());
            return;
        }
        let mapping = (0..order.len())
            .map(|i| order.iter().position(|old| *old == i))
            .collect();
        self.queue = order.iter().map(|i| self.queue[*i].clone()).collect();
        self.metadata = order.iter().map(|i| self.metadata[*i].clone()).collect();
        self.rows = order.iter().map(|i| self.rows[*i]).collect();
        self.changed(mapping, effects);
    }
    fn stop(&mut self, effects: &mut Vec<Effect>) {
        self.transport_epoch += 1;
        self.output = None;
        self.radio.cancel_waiting();
        self.order.cancel_navigation();
        self.transport = Transport::Stopped;
        self.position = 0.0;
        effects.push(Effect::Stop);
    }
    fn play(&mut self, index: usize, automatic: bool, effects: &mut Vec<Effect>) {
        if index >= self.queue.len() {
            return;
        }
        if !automatic {
            self.order.cancel_navigation();
            self.radio.cancel_waiting();
            self.transport_epoch += 1;
        }
        let token = self.token();
        self.output = Some(OutputRequest {
            token: token.clone(),
            row: self.rows[index],
            previous: self
                .output
                .as_ref()
                .filter(|o| !o.started)
                .map(|o| o.previous)
                .unwrap_or_else(|| self.current.map(|i| self.rows[i])),
            automatic,
            started: false,
        });
        self.current = Some(index);
        self.transport = Transport::Starting;
        self.position = 0.0;
        self.duration = 0.0;
        effects.push(Effect::Play {
            token,
            index,
            seconds: 0.0,
            playing: true,
        });
    }
    fn navigate(&mut self, event: NavigationEvent, effects: &mut Vec<Effect>) {
        match self
            .order
            .navigate(&self.order_tracks(), self.current, event)
        {
            PlaybackDecision::Play(index) => self.play(index, true, effects),
            PlaybackDecision::Stop => self.stop(effects),
            PlaybackDecision::Radio => {
                if let Some(track) = self.radio.request_next() {
                    self.radio_track(track, effects)
                } else {
                    self.transport = Transport::Stopped;
                    self.output = None;
                    effects.push(Effect::Stop);
                    self.refill(false, false, effects);
                }
            }
        }
    }
    fn radio_track(&mut self, track: T, effects: &mut Vec<Effect>) {
        let index = self.queue.len();
        self.append(vec![track], workspace::QueueAction::AddToQueue, effects);
        self.order.radio_candidate(index);
        self.play(index, true, effects);
        self.refill(false, false, effects);
    }
    fn refill(&mut self, reset: bool, reshuffle: bool, effects: &mut Vec<Effect>) {
        if !reset && !self.radio.needs_refill() {
            return;
        }
        let generation = self.radio.begin_request();
        let token = self.token();
        self.pending.insert(
            token.serial,
            Pending::Radio {
                generation,
                scope: self.radio_scope.clone(),
            },
        );
        effects.push(Effect::Radio {
            token,
            scope: self.radio_scope.clone(),
            root: self.radio_root.clone(),
            enabled: self.radio.enabled(),
            reshuffle,
            reset,
        });
    }
    fn expand(
        &mut self,
        scope: String,
        entries: Vec<Value>,
        action: workspace::QueueAction,
        effects: &mut Vec<Effect>,
    ) {
        if entries.is_empty() {
            return;
        }
        let token = self.token();
        self.expansion_order.push_back(token.serial);
        self.pending.insert(
            token.serial,
            Pending::Expand {
                action,
                transport_epoch: self.transport_epoch,
                scope: scope.clone(),
                ready: None,
            },
        );
        effects.push(Effect::Expand {
            token,
            scope,
            entries,
        });
    }
    fn workspace_command(&mut self, command: workspace::Command, effects: &mut Vec<Effect>) {
        if let workspace::Command::Append { entries } = &command {
            if let Some(tab) = self
                .workspace()
                .tabs
                .iter()
                .find(|t| t.key == self.workspace().active)
            {
                let device = tab.scope == "device";
                if entries
                    .iter()
                    .any(|e| e["kind"] != "remote" && (e["kind"] == "device") != device)
                {
                    self.error =
                        Some("Choose a playlist in the same library as these tracks.".into());
                    return;
                }
            }
        }
        match self
            .workspace
            .apply_ui(command, self.queue.len(), self.selection.indices.len())
        {
            Ok(workspace::Effect::Load {
                key,
                scope,
                playlist_id,
                generation,
            }) => {
                let token = self.token();
                self.pending.insert(
                    token.serial,
                    Pending::Load {
                        key,
                        generation,
                        scope: scope.clone(),
                    },
                );
                effects.push(Effect::Load {
                    token,
                    scope,
                    playlist_id,
                });
            }
            Ok(workspace::Effect::Save {
                key,
                scope,
                playlist_id,
                revision,
                entries,
                expected_entries,
            }) => {
                let token = self.token();
                self.pending.insert(
                    token.serial,
                    Pending::Save {
                        key,
                        revision,
                        scope: scope.clone(),
                    },
                );
                effects.push(Effect::Save {
                    token,
                    scope,
                    playlist_id,
                    entries,
                    expected_entries,
                });
            }
            Ok(workspace::Effect::Queue {
                mode,
                entries,
                scope,
            }) => self.expand(scope, entries, mode, effects),
            Ok(workspace::Effect::None) => (),
            Err(error) => self.error = Some(error),
        }
    }
    fn complete(&mut self, token: Token, result: IoResult<T>, effects: &mut Vec<Effect>) {
        if token.session != self.id || token.incarnation != self.incarnation {
            return;
        }
        let Some(pending) = self.pending.remove(&token.serial) else {
            return;
        };
        let result = if !self.scopes.is_empty() && !self.scopes.iter().any(|s| s == pending.scope())
        {
            IoResult::Failed {
                error: "The operation's library is no longer connected".into(),
            }
        } else {
            result
        };
        match pending {
            Pending::Load {
                key, generation, ..
            } => self.workspace_command(
                match result {
                    IoResult::Loaded { entries } => workspace::Command::Loaded {
                        key,
                        generation,
                        entries,
                    },
                    IoResult::Failed { error } => workspace::Command::LoadFailed {
                        key,
                        generation,
                        error,
                    },
                    _ => workspace::Command::LoadFailed {
                        key,
                        generation,
                        error: "Invalid load response".into(),
                    },
                },
                effects,
            ),
            Pending::Save { key, revision, .. } => self.workspace_command(
                match result {
                    IoResult::Saved => workspace::Command::Saved { key, revision },
                    IoResult::Failed { error } => workspace::Command::SaveFailed {
                        key,
                        revision,
                        error,
                    },
                    _ => workspace::Command::SaveFailed {
                        key,
                        revision,
                        error: "Invalid save response".into(),
                    },
                },
                effects,
            ),
            Pending::Expand {
                action,
                transport_epoch,
                scope,
                ..
            } => {
                let ready = Some(match result {
                    IoResult::Expanded { tracks } => Ok(tracks),
                    IoResult::Failed { error } => Err(error),
                    _ => Err("Invalid expansion response".into()),
                });
                self.pending.insert(
                    token.serial,
                    Pending::Expand {
                        action,
                        transport_epoch,
                        scope,
                        ready,
                    },
                );
                while let Some(serial) = self.expansion_order.front().copied() {
                    if !matches!(
                        self.pending.get(&serial),
                        Some(Pending::Expand { ready: Some(_), .. })
                    ) {
                        break;
                    }
                    self.expansion_order.pop_front();
                    if let Some(Pending::Expand {
                        action,
                        transport_epoch,
                        ready: Some(result),
                        ..
                    }) = self.pending.remove(&serial)
                    {
                        match result {
                            Ok(tracks) => {
                                let action = if transport_epoch != self.transport_epoch
                                    && action == workspace::QueueAction::PlayNow
                                {
                                    workspace::QueueAction::AddToQueue
                                } else {
                                    action
                                };
                                let epoch = self.transport_epoch;
                                self.append(tracks, action, effects);
                                self.transport_epoch = epoch;
                            }
                            Err(error) => self.error = Some(error),
                        }
                    }
                }
            }
            Pending::Radio { generation, .. } => {
                let accepted = match result {
                    IoResult::Radio { tracks, exhausted } => {
                        self.radio.accept(generation, tracks, exhausted)
                    }
                    IoResult::Failed { error } => {
                        let accepted = self.radio.fail(generation);
                        if accepted {
                            self.error = Some(error)
                        }
                        accepted
                    }
                    _ => self.radio.fail(generation),
                };
                if accepted {
                    if let Some(track) = self.radio.take_pending() {
                        self.radio_track(track, effects)
                    } else {
                        self.refill(false, false, effects)
                    }
                }
            }
        }
    }
    pub fn dispatch(&mut self, command: Command<T>) -> Vec<Effect> {
        if let Command::Complete { token, .. } = &command {
            if token.session != self.id
                || token.incarnation != self.incarnation
                || !self.pending.contains_key(&token.serial)
            {
                return vec![];
            }
        }
        if let Command::Output { token, .. } = &command {
            if !self
                .output
                .as_ref()
                .is_some_and(|request| &request.token == token)
            {
                return vec![];
            }
        }
        let persist = !matches!(
            &command,
            Command::Output {
                event: OutputEvent::Progress { .. },
                ..
            }
        );
        let mut effects = vec![];
        self.error = None;
        match command {
            Command::Append { tracks, action } => self.append(tracks, action, &mut effects),
            Command::Replace { tracks, current } => {
                self.clear(&mut effects);
                self.append(tracks, workspace::QueueAction::AddToQueue, &mut effects);
                self.current = current.filter(|i| *i < self.queue.len());
            }
            Command::Insert { index, tracks } => {
                self.replace_range(index, index, tracks, &mut effects)
            }
            Command::ReplaceRange { start, end, tracks } => {
                self.replace_range(start, end, tracks, &mut effects)
            }
            Command::ReloadOutput => {
                if let Some(index) = self.current {
                    if self.transport != Transport::Stopped {
                        let playing = self.transport != Transport::Paused;
                        let token = self.token();
                        self.output = Some(OutputRequest {
                            token: token.clone(),
                            row: self.rows[index],
                            previous: Some(self.rows[index]),
                            automatic: false,
                            started: false,
                        });
                        self.transport = if playing {
                            Transport::Starting
                        } else {
                            Transport::Paused
                        };
                        effects.push(Effect::Play {
                            token,
                            index,
                            seconds: self.position,
                            playing,
                        });
                    }
                }
            }
            Command::Clear => self.clear(&mut effects),
            Command::Remove { indices } => {
                let keep: Vec<_> = (0..self.queue.len())
                    .filter(|i| !indices.contains(i))
                    .collect();
                let mapping = (0..self.queue.len())
                    .map(|i| keep.iter().position(|old| *old == i))
                    .collect();
                if self.current.is_some_and(|i| indices.contains(&i)) {
                    self.stop(&mut effects)
                }
                self.queue = keep.iter().map(|i| self.queue[*i].clone()).collect();
                self.metadata = keep.iter().map(|i| self.metadata[*i].clone()).collect();
                self.rows = keep.iter().map(|i| self.rows[*i]).collect();
                self.changed(mapping, &mut effects);
            }
            Command::Move {
                mut indices,
                target,
            } => {
                indices.retain(|i| *i < self.queue.len());
                indices.sort_unstable();
                indices.dedup();
                let mut order: Vec<_> = (0..self.queue.len())
                    .filter(|i| !indices.contains(i))
                    .collect();
                let slot = target
                    .min(self.queue.len())
                    .saturating_sub(indices.iter().filter(|i| **i < target).count())
                    .min(order.len());
                order.splice(slot..slot, indices);
                self.reorder(order, &mut effects);
            }
            Command::Reorder { indices } => self.reorder(indices, &mut effects),
            Command::UpdateItem { index, track } => {
                if index < self.queue.len() {
                    self.metadata[index] = track.metadata();
                    self.queue[index] = track;
                    self.order
                        .album_metadata_changed(&self.order_tracks(), self.current);
                    self.refresh();
                }
            }
            Command::Metadata { rows } => {
                if rows.len() == self.queue.len() {
                    self.metadata = rows;
                    self.order
                        .album_metadata_changed(&self.order_tracks(), self.current);
                    self.refresh();
                }
            }
            Command::Select { command } => {
                let command = if matches!(command, crate::selection::Command::All) {
                    crate::selection::Command::Set {
                        indices: self.visible.clone(),
                        anchor: self.visible.first().copied(),
                    }
                } else {
                    command
                };
                self.selection
                    .apply(command, self.queue.len(), &self.visible);
            }
            Command::Filter { query } => {
                self.filter = query;
                self.refresh()
            }
            Command::Sort {
                column,
                descending,
                physical,
            } => {
                if physical {
                    let indices = crate::sort::sorted_rows(&self.metadata, &column, descending);
                    self.reorder(indices, &mut effects);
                }
                self.sort_column = column;
                self.descending = descending;
                self.physical_sort = physical;
                self.refresh();
            }
            Command::Workspace { command } => self.workspace_command(command, &mut effects),
            Command::WorkspaceRestore { value } => match workspace::Workspace::restore(value) {
                Ok(state) => self.workspace = state,
                Err(error) => self.error = Some(error),
            },
            Command::AppendQueueToWorkspace { selected_only } => {
                let entries = self
                    .queue
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !selected_only || self.selection.indices.contains(i))
                    .map(|(_, track)| track.entry())
                    .collect();
                self.workspace_command(workspace::Command::Append { entries }, &mut effects);
            }
            Command::Expand {
                scope,
                entries,
                action,
            } => self.expand(scope, entries, action, &mut effects),
            Command::Collect {
                scope,
                path,
                query,
                root,
                action,
            } => {
                let token = self.token();
                self.expansion_order.push_back(token.serial);
                self.pending.insert(
                    token.serial,
                    Pending::Expand {
                        action,
                        transport_epoch: self.transport_epoch,
                        scope: scope.clone(),
                        ready: None,
                    },
                );
                effects.push(Effect::Collect {
                    token,
                    scope,
                    path,
                    query,
                    root,
                });
            }
            Command::Complete { token, result } => self.complete(token, result, &mut effects),
            Command::Play { index } => self.play(index, false, &mut effects),
            Command::Activate { index } => {
                match crate::selection::activate(index, self.current, self.queue.len()) {
                    Some(crate::selection::Activation::Play { index }) => {
                        self.play(index, false, &mut effects)
                    }
                    Some(crate::selection::Activation::TogglePlayback) => self.toggle(&mut effects),
                    None => (),
                }
            }
            Command::Toggle => self.toggle(&mut effects),
            Command::Pause => {
                if self.radio.waiting() {
                    self.stop(&mut effects)
                } else if matches!(self.transport, Transport::Playing | Transport::Starting) {
                    self.transport = Transport::Paused;
                    effects.push(Effect::Pause)
                }
            }
            Command::Resume => self.resume(&mut effects),
            Command::Stop => self.stop(&mut effects),
            Command::Seek { seconds } => {
                if seconds.is_finite() {
                    self.position = seconds.max(0.0);
                    effects.push(Effect::Seek {
                        seconds: self.position,
                    })
                }
            }
            Command::Volume { value } => {
                if value.is_finite() {
                    self.volume = value.clamp(0.0, 1.0);
                    effects.push(Effect::Volume { value: self.volume })
                }
            }
            Command::Navigate { event } => self.navigate(event, &mut effects),
            Command::Output { token, event } => {
                if let Some(request) = self.output.clone().filter(|r| r.token == token) {
                    if let Some(index) = self.rows.iter().position(|id| *id == request.row) {
                        match event {
                            OutputEvent::Started => {
                                let previous = request
                                    .previous
                                    .and_then(|row| self.rows.iter().position(|id| *id == row));
                                if !request.started {
                                    self.order.started(previous, index);
                                }
                                if let Some(output) = self.output.as_mut() {
                                    output.started = true;
                                }
                                self.current = Some(index);
                                if self.transport != Transport::Paused {
                                    self.transport = Transport::Playing;
                                }
                            }
                            OutputEvent::Failed { error } => {
                                self.error = Some(error);
                                if request.automatic {
                                    self.navigate(NavigationEvent::Failed, &mut effects)
                                } else {
                                    self.stop(&mut effects)
                                }
                            }
                            OutputEvent::Ended => {
                                self.navigate(NavigationEvent::Ended, &mut effects)
                            }
                            OutputEvent::Progress { seconds, duration } => {
                                if seconds.is_finite() {
                                    self.position = seconds.max(0.0)
                                }
                                if duration.is_finite() {
                                    self.duration = duration.max(0.0)
                                }
                            }
                        }
                    }
                }
            }
            Command::Shuffle { mode } => {
                self.order
                    .set_shuffle_mode(mode, &self.order_tracks(), self.current)
            }
            Command::Repeat { mode } => self.order.set_repeat_mode(mode),
            Command::CycleShuffle => self.order.set_shuffle_mode(
                self.order.shuffle_mode().next(),
                &self.order_tracks(),
                self.current,
            ),
            Command::CycleRepeat => self.order.set_repeat_mode(self.order.repeat_mode().next()),
            Command::ToggleQueued { indices } => self.order.toggle_queue(&indices),
            Command::ToggleStopAfter { indices } => self.order.toggle_stop_after(&indices),
            Command::ClearQueued => self.order.clear_queue(),
            Command::Radio {
                enabled,
                scope,
                root,
                reshuffle,
            } => {
                self.order
                    .set_radio_enabled(enabled, &self.order_tracks(), self.current);
                self.radio.reset(enabled);
                self.radio_scope = scope;
                self.radio_root = root;
                self.refill(true, reshuffle, &mut effects);
            }
            Command::RefillRadio => self.refill(false, false, &mut effects),
            Command::Scopes { scopes } => self.scopes = scopes,
        }
        if self.radio.enabled() && !self.order.radio_enabled() {
            self.radio.reset(false);
            self.refill(true, false, &mut effects);
        }
        // One dispatch can drain several FIFO completions. Only the final play
        // request is still live, and views apply one mapping to the final queue.
        let live = self.output.as_ref().map(|o| &o.token);
        effects.retain(|effect| !matches!(effect,Effect::Play{token,..} if Some(token)!=live));
        let mut mapping: Option<Vec<Option<usize>>> = None;
        effects.retain(|effect| {
            if let Effect::QueueChanged { old_to_new } = effect {
                mapping = Some(match mapping.take() {
                    Some(old) => old
                        .into_iter()
                        .map(|i| i.and_then(|i| old_to_new.get(i).copied().flatten()))
                        .collect(),
                    None => old_to_new.clone(),
                });
                false
            } else {
                true
            }
        });
        if let Some(old_to_new) = mapping {
            effects.insert(0, Effect::QueueChanged { old_to_new });
        }
        self.revision += 1;
        if persist {
            effects.insert(
                0,
                Effect::Persist {
                    value: self.checkpoint(),
                },
            );
        }
        effects
    }
    fn replace_range(
        &mut self,
        start: usize,
        end: usize,
        tracks: Vec<T>,
        effects: &mut Vec<Effect>,
    ) {
        let start = start.min(self.queue.len());
        let end = end.max(start).min(self.queue.len());
        let count = tracks.len();
        if self.current.is_some_and(|i| i >= start && i < end) {
            self.stop(effects)
        }
        let mapping = (0..self.queue.len())
            .map(|i| {
                if i < start {
                    Some(i)
                } else if i < end {
                    None
                } else {
                    Some(i - (end - start) + count)
                }
            })
            .collect();
        let metadata = tracks.iter().map(Item::metadata).collect::<Vec<_>>();
        let rows = (0..count)
            .map(|_| {
                let id = self.next_row;
                self.next_row += 1;
                id
            })
            .collect::<Vec<_>>();
        self.queue.splice(start..end, tracks);
        self.metadata.splice(start..end, metadata);
        self.rows.splice(start..end, rows);
        self.changed(mapping, effects);
    }
    fn clear(&mut self, effects: &mut Vec<Effect>) {
        self.stop(effects);
        let count = self.queue.len();
        self.queue.clear();
        self.rows.clear();
        self.metadata.clear();
        self.current = None;
        self.selection = Selection::default();
        self.order.clear_tracks();
        self.pending
            .retain(|_, p| !matches!(p, Pending::Expand { .. }));
        self.expansion_order.clear();
        self.changed(vec![None; count], effects);
    }
    fn toggle(&mut self, effects: &mut Vec<Effect>) {
        if self.radio.waiting() {
            self.stop(effects)
        } else if matches!(self.transport, Transport::Playing | Transport::Starting) {
            self.transport = Transport::Paused;
            effects.push(Effect::Pause)
        } else {
            self.resume(effects)
        }
    }
    fn resume(&mut self, effects: &mut Vec<Effect>) {
        if self.transport == Transport::Paused && self.output.is_some() {
            self.transport = Transport::Playing;
            effects.push(Effect::Resume)
        } else if !self.queue.is_empty() {
            self.play(self.current.unwrap_or(0), false, effects)
        } else if self.radio.enabled() {
            self.navigate(NavigationEvent::Next, effects)
        }
    }
    /// A checkpoint contains no pending network requests or output handles.
    /// Restoring it cannot resume output or accept an old instance's callback.
    pub fn checkpoint(&self) -> Value {
        serde_json::json!({"version":1,"session_id":self.id,"queue":self.queue.iter().map(Item::entry).collect::<Vec<_>>(),
            "metadata":self.metadata,"rows":self.rows,"next_row":self.next_row,"current":self.current,
            "order":serde_json::to_string(&self.order).expect("serializable playback order"),"selection":self.selection,"workspace":self.workspace,"volume":self.volume,
            "filter":self.filter,"sort_column":self.sort_column,"descending":self.descending,"physical_sort":self.physical_sort,
            "radio_enabled":self.radio.enabled(),"radio_scope":self.radio_scope,"radio_root":self.radio_root})
    }
    pub fn restore(
        &mut self,
        value: Value,
        mut decode: impl FnMut(&Value) -> Result<T, String>,
    ) -> Result<(), String> {
        #[derive(Deserialize)]
        struct Saved {
            version: u32,
            session_id: String,
            queue: Vec<Value>,
            metadata: Vec<SortRow>,
            rows: Vec<u64>,
            next_row: u64,
            current: Option<usize>,
            order: String,
            selection: Selection,
            workspace: Value,
            volume: f64,
            filter: String,
            sort_column: String,
            descending: bool,
            physical_sort: bool,
            radio_enabled: bool,
            radio_scope: String,
            radio_root: String,
        }
        let saved: Saved = serde_json::from_value(value).map_err(|e| e.to_string())?;
        if saved.version != 1 || saved.session_id != self.id {
            return Err("Checkpoint belongs to another session or version".into());
        }
        if saved.queue.len() != saved.metadata.len() || saved.queue.len() != saved.rows.len() {
            return Err("Invalid checkpoint row data".into());
        }
        let mut next = Self::new(
            self.id.clone(),
            self.incarnation.wrapping_add(1),
            self.order.shuffle_mode(),
            self.order.repeat_mode(),
        );
        let mut mapping = vec![None; saved.queue.len()];
        for (i, entry) in saved.queue.iter().enumerate() {
            if let Ok(track) = decode(entry) {
                mapping[i] = Some(next.queue.len());
                next.queue.push(track);
                next.metadata.push(saved.metadata[i].clone());
                next.rows.push(saved.rows[i]);
            }
        }
        next.next_row = saved
            .next_row
            .max(next.rows.iter().copied().max().unwrap_or(0) + 1);
        next.current = saved
            .current
            .and_then(|i| mapping.get(i).copied().flatten());
        next.order = serde_json::from_str(&saved.order).map_err(|e| e.to_string())?;
        next.order.remap_tracks(&mapping);
        next.order.cancel_navigation();
        next.selection = saved.selection;
        next.selection.remap(&mapping);
        next.workspace = workspace::Workspace::restore(saved.workspace)?;
        next.volume = saved.volume.clamp(0.0, 1.0);
        next.filter = saved.filter;
        next.sort_column = saved.sort_column;
        next.descending = saved.descending;
        next.physical_sort = saved.physical_sort;
        next.radio_scope = saved.radio_scope;
        next.radio_root = saved.radio_root;
        next.radio.reset(saved.radio_enabled);
        next.order
            .tracks_changed(&next.order_tracks(), next.current);
        next.refresh();
        *self = next;
        Ok(())
    }
}

/// The same backend instance is serialized between FFI calls. Its checkpoint
/// is a separate durable format: in-flight effects survive calls, not restarts.
pub fn dispatch_json(input: &str) -> Result<String, String> {
    #[derive(Deserialize)]
    struct Request {
        #[serde(default)]
        state: Option<String>,
        session_id: String,
        #[serde(default)]
        incarnation: u64,
        #[serde(default)]
        command: Option<Command<Value>>,
        #[serde(default)]
        restore: Option<Value>,
    }
    let request: Request = serde_json::from_str(input).map_err(|e| e.to_string())?;
    let mut session = match request.state.filter(|s| !s.is_empty()) {
        Some(state) => serde_json::from_str::<Session<Value>>(&state).map_err(|e| e.to_string())?,
        None => Session::new(
            request.session_id.clone(),
            request.incarnation,
            ShuffleMode::Off,
            RepeatMode::Off,
        ),
    };
    if session.id() != request.session_id {
        return Err("Session identity mismatch".into());
    }
    if let Some(value) = request.restore {
        session.restore(value, |v| Ok(v.clone()))?;
    }
    let effects = request
        .command
        .map(|command| session.dispatch(command))
        .unwrap_or_default();
    serde_json::to_string(
        &serde_json::json!({"state":serde_json::to_string(&session).map_err(|e|e.to_string())?,
        "snapshot":session.snapshot(),"effects":effects,"checkpoint":session.checkpoint()}),
    )
    .map_err(|e| e.to_string())
}
