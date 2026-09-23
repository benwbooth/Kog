//! A small terminal frontend. The terminal protocol, layout and hit testing
//! live here; browsing, persistence and playback use Kog's existing crates.
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use kog_audio::decoder::DecoderRegistry;
use kog_audio::playback::{PlaybackEngine, PlaybackState};
use kog_audio::playlist::{PlaylistEntry, PlaylistLocation};
use kog_audio::settings::{AppSettings, RepeatMode};
use kog_audio::track::Track as AudioTrack;
use kog_core::db::StoredEntry;
use kog_server::api::{Library, LocalSearch, browse_local};
use unicode_width::UnicodeWidthChar;

#[derive(Clone)]
struct Track {
    name: String,
    entry: StoredEntry,
}

struct TrackMetadata {
    title: String,
    artist: String,
    album: String,
    duration: Option<Duration>,
}

#[derive(Clone)]
enum Item {
    Directory(String, PathBuf),
    Track(Track),
}

#[derive(Clone)]
struct TreeRow {
    item: Item,
    depth: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Playlists,
    Library,
    Tracks,
}

enum PromptKind {
    MusicFolder,
    NewPlaylist,
    RenamePlaylist,
    DeletePlaylist,
    Search,
    PlaylistSearch,
}

struct Ui {
    library: Arc<Library>,
    decoders: DecoderRegistry,
    player: PlaybackEngine,
    browse_path: Option<PathBuf>,
    items: Vec<TreeRow>,
    root_items: Vec<Item>,
    expanded: HashSet<PathBuf>,
    children: HashMap<PathBuf, Vec<Item>>,
    lists: Vec<(i64, String)>,
    active_list: i64, // -1 is the ephemeral queue; 0 is Favorites.
    tracks: Vec<Track>,
    queue: Vec<Track>,
    selected: [usize; 3],
    offsets: [usize; 3],
    focus: Focus,
    playing: Option<usize>,
    status: String,
    volume: f32,
    repeat_mode: RepeatMode,
    last_click: Option<(Instant, usize, usize)>,
    prompt: Option<(PromptKind, String)>,
    search: Option<LocalSearch>,
    search_query: String,
    playlist_query: String,
    metadata: HashMap<String, Option<TrackMetadata>>,
    metadata_pending: HashSet<String>,
    metadata_requests: Sender<(String, StoredEntry)>,
    metadata_results: Receiver<(String, Option<TrackMetadata>)>,
    search_done: bool,
    exit_requested: bool,
    menu_open: bool,
    menu_selected: usize,
    sidebar_visible: bool,
}

impl Ui {
    fn new() -> Self {
        let settings = AppSettings::load();
        let (metadata_requests, worker_requests) = mpsc::channel::<(String, StoredEntry)>();
        let (worker_results, metadata_results) = mpsc::channel();
        let metadata_settings = settings.decoder_settings();
        std::thread::spawn(move || {
            let decoders = DecoderRegistry::new(metadata_settings);
            while let Ok((key, entry)) = worker_requests.recv() {
                let resolved = kog_audio::streaming::resolve_entry(
                    &playlist_entry(&entry),
                    &decoders,
                    &kog_server::service::scratch_root().join("tui-metadata"),
                );
                let metadata = resolved.ok().map(|source| {
                    let track = AudioTrack::from_source(source, &decoders);
                    let title = if entry.kind == "archive"
                        && track.title
                            == track
                                .source
                                .path
                                .file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                    {
                        String::new()
                    } else {
                        track.title
                    };
                    TrackMetadata {
                        title,
                        artist: track.artist,
                        album: track.album,
                        duration: track.duration,
                    }
                });
                if worker_results.send((key, metadata)).is_err() {
                    break;
                }
            }
        });
        let library = Arc::new(Library::open());
        let decoders = DecoderRegistry::new(settings.decoder_settings());
        let mut player = PlaybackEngine::new(DecoderRegistry::new(settings.decoder_settings()));
        let volume = settings.output_volume as f32;
        player.set_volume(volume);
        let mut ui = Self {
            library,
            decoders,
            player,
            browse_path: None,
            items: Vec::new(),
            root_items: Vec::new(),
            expanded: HashSet::new(),
            children: HashMap::new(),
            lists: Vec::new(),
            active_list: -1,
            tracks: Vec::new(),
            queue: Vec::new(),
            selected: [0; 3],
            offsets: [0; 3],
            focus: Focus::Library,
            playing: None,
            status: String::new(),
            volume,
            repeat_mode: settings.repeat_mode,
            last_click: None,
            prompt: None,
            search: None,
            search_query: String::new(),
            playlist_query: String::new(),
            metadata: HashMap::new(),
            metadata_pending: HashSet::new(),
            metadata_requests,
            metadata_results,
            search_done: false,
            exit_requested: false,
            menu_open: false,
            menu_selected: 0,
            sidebar_visible: true,
        };
        ui.reload_lists();
        ui.browse(None);
        ui
    }

    fn reload_lists(&mut self) {
        self.lists = vec![(0, "Favorites".to_owned())];
        match self.library.db().list_playlists() {
            Ok(lists) => self.lists.extend(lists.into_iter().map(|p| (p.id, p.name))),
            Err(error) => self.status = error,
        }
    }

    fn poll_metadata(&mut self) {
        while let Ok((key, metadata)) = self.metadata_results.try_recv() {
            self.metadata_pending.remove(&key);
            self.metadata.insert(key, metadata);
        }
    }

    fn request_metadata(&mut self, entry: &StoredEntry) {
        let key = metadata_key(entry);
        if self.metadata.contains_key(&key) || !self.metadata_pending.insert(key.clone()) {
            return;
        }
        let _ = self.metadata_requests.send((key, entry.clone()));
    }

    fn metadata_for(&self, track: &Track) -> Option<&TrackMetadata> {
        self.metadata
            .get(&metadata_key(&track.entry))
            .and_then(Option::as_ref)
    }

    fn title_for(&self, track: &Track) -> String {
        self.metadata_for(track)
            .map(|meta| meta.title.clone())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| display_title(track))
    }

    fn show_queue(&mut self) {
        self.player.stop();
        self.playing = None;
        self.active_list = -1;
        self.tracks = self.queue.clone();
        self.focus = Focus::Tracks;
        self.selected[2] = 0;
        self.offsets[2] = 0;
    }

    fn layout(&self, size: (usize, usize)) -> Layout {
        let mut layout = Layout::new(size.0, size.1, self.lists.len());
        if !self.sidebar_visible {
            layout.show_sidebar = false;
            layout.first = 0;
        }
        layout
    }

    fn activate_menu(&mut self, index: usize) {
        self.menu_open = false;
        match index {
            0 => self.prompt = Some((PromptKind::Search, self.search_query.clone())),
            1 => {
                self.prompt = Some((
                    PromptKind::MusicFolder,
                    self.library
                        .root()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                ))
            }
            2 => self.prompt = Some((PromptKind::NewPlaylist, String::new())),
            3 => self.show_queue(),
            4 => self.cycle_repeat(),
            5 => self.exit_requested = true,
            _ => {}
        }
    }

    fn browse(&mut self, path: Option<PathBuf>) {
        self.search = None;
        self.search_query.clear();
        let result = self.read_directory(path.as_deref());
        match result {
            Ok((location, items)) => {
                self.browse_path = location;
                self.root_items = items;
                self.expanded.clear();
                self.children.clear();
                self.rebuild_tree();
                self.selected[1] = 0;
                self.offsets[1] = 0;
                self.status.clear();
            }
            Err(error) => self.status = error,
        }
    }

    fn read_directory(&self, path: Option<&Path>) -> Result<(Option<PathBuf>, Vec<Item>), String> {
        let value = browse_local(&self.library, path.and_then(Path::to_str))?;
        let mut items = Vec::new();
        for dir in value["directories"].as_array().into_iter().flatten() {
            if let (Some(name), Some(path)) = (dir["name"].as_str(), dir["path"].as_str()) {
                items.push(Item::Directory(name.to_owned(), PathBuf::from(path)));
            }
        }
        for file in value["files"].as_array().into_iter().flatten() {
            if let (Some(name), Some(path), Some(kind)) = (
                file["name"].as_str(),
                file["path"].as_str(),
                file["kind"].as_str(),
            ) {
                items.push(Item::Track(Track {
                    name: name.to_owned(),
                    entry: StoredEntry {
                        kind: kind.to_owned(),
                        path: path.to_owned(),
                        entry: file["entry"].as_str().unwrap_or_default().to_owned(),
                        fragment: file["fragment"].as_str().map(str::to_owned),
                    },
                }));
            }
        }
        Ok((value["path"].as_str().map(PathBuf::from), items))
    }

    fn rebuild_tree(&mut self) {
        fn append(
            rows: &mut Vec<TreeRow>,
            entries: &[Item],
            depth: usize,
            expanded: &HashSet<PathBuf>,
            children: &HashMap<PathBuf, Vec<Item>>,
        ) {
            for item in entries {
                rows.push(TreeRow {
                    item: item.clone(),
                    depth,
                });
                if let Item::Directory(_, path) = item {
                    if expanded.contains(path) {
                        if let Some(child) = children.get(path) {
                            append(rows, child, depth + 1, expanded, children);
                        }
                    }
                }
            }
        }
        self.items.clear();
        append(
            &mut self.items,
            &self.root_items,
            0,
            &self.expanded,
            &self.children,
        );
    }

    fn toggle_directory(&mut self, path: PathBuf) {
        if self.expanded.remove(&path) {
            self.rebuild_tree();
            return;
        }
        if !self.children.contains_key(&path) {
            match self.read_directory(Some(&path)) {
                Ok((_, children)) => {
                    self.children.insert(path.clone(), children);
                }
                Err(error) => {
                    self.status = error;
                    return;
                }
            }
        }
        self.expanded.insert(path);
        self.rebuild_tree();
    }

    fn up_directory(&mut self) {
        if self.search.is_some() {
            self.browse(None);
            return;
        }
        let selected = self.selected[1];
        if let Some(TreeRow {
            item: Item::Directory(_, path),
            ..
        }) = self.items.get(selected)
        {
            if self.expanded.contains(path) {
                self.toggle_directory(path.clone());
                return;
            }
        }
        let depth = self.items.get(selected).map_or(0, |row| row.depth);
        if depth > 0 {
            if let Some(parent) = (0..selected).rev().find(|&i| self.items[i].depth < depth) {
                self.selected[1] = parent;
            }
        }
    }

    fn select_list(&mut self, index: usize) {
        if index >= self.lists.len() {
            return;
        }
        if self.active_list == self.lists[index].0 {
            return;
        }
        self.player.stop();
        self.playing = None;
        self.selected[0] = index;
        self.active_list = self.lists[index].0;
        let rows = if self.active_list == -1 {
            self.queue.clone()
        } else {
            let db = self.library.db();
            let result = if self.active_list == 0 {
                db.starred_entries()
            } else {
                db.playlist_entries(self.active_list)
            };
            match result {
                Ok(entries) => entries.into_iter().map(track_from_entry).collect(),
                Err(error) => {
                    self.status = error;
                    Vec::new()
                }
            }
        };
        self.tracks = rows;
        self.selected[2] = 0;
        self.offsets[2] = 0;
        self.playing = None;
    }

    fn add_selected(&mut self, play: bool) {
        let Some(TreeRow {
            item: Item::Track(track),
            ..
        }) = self.items.get(self.selected[1]).cloned()
        else {
            self.open_selected();
            return;
        };
        if self.active_list == 0 {
            self.status = "Choose Queue or a playlist before adding".to_owned();
            return;
        }
        if self.active_list == -1 {
            self.queue.push(track.clone());
            self.tracks = self.queue.clone();
        } else if let Err(error) = self
            .library
            .db()
            .append_entries(self.active_list, &[track.entry.clone()])
        {
            self.status = error;
            return;
        } else {
            self.tracks.push(track.clone());
        }
        self.selected[2] = self.tracks.len() - 1;
        self.status = format!("Added {}", track.name);
        if play {
            self.play_selected();
        }
    }

    fn open_selected(&mut self) {
        match self.items.get(self.selected[1]).cloned() {
            Some(TreeRow {
                item: Item::Directory(_, path),
                ..
            }) => self.toggle_directory(path),
            Some(TreeRow {
                item: Item::Track(_),
                ..
            }) => self.add_selected(true),
            None => {}
        }
    }

    fn play_selected(&mut self) {
        let index = self.selected[2];
        let Some(track) = self.tracks.get(index) else {
            return;
        };
        let entry = playlist_entry(&track.entry);
        // The decoder registry holds any extracted archive files alive while
        // this playback engine plays them.
        match kog_audio::streaming::resolve_entry(
            &entry,
            &self.decoders,
            &kog_server::service::scratch_root().join("tui"),
        )
        .and_then(|source| self.player.play_source(&source).map(|_| ()))
        {
            Ok(()) => {
                self.playing = Some(index);
                self.status = format!("Playing {}", track.name);
            }
            Err(error) => self.status = error,
        }
    }

    fn next(&mut self, honor_repeat_one: bool) {
        if self.tracks.is_empty() {
            return;
        }
        let next = match self.playing {
            Some(i) if honor_repeat_one && self.repeat_mode == RepeatMode::One => i,
            Some(i) if i + 1 < self.tracks.len() => i + 1,
            Some(_) if self.repeat_mode == RepeatMode::All => 0,
            Some(_) => {
                self.player.stop();
                self.playing = None;
                return;
            }
            None => 0,
        };
        self.selected[2] = next;
        self.play_selected();
    }

    fn play_pause(&mut self) {
        if self.player.state() == PlaybackState::Stopped {
            self.play_selected();
        } else {
            self.player.play_pause();
        }
    }

    fn previous(&mut self) {
        if self.tracks.is_empty() {
            return;
        }
        let previous = self.playing.map_or(0, |i| {
            i.checked_sub(1)
                .unwrap_or(if self.repeat_mode == RepeatMode::All {
                    self.tracks.len() - 1
                } else {
                    0
                })
        });
        self.selected[2] = previous;
        self.play_selected();
    }

    fn cycle_repeat(&mut self) {
        self.repeat_mode = self.repeat_mode.next();
        let _ = AppSettings::save_repeat_mode(self.repeat_mode);
        self.status = format!("Repeat: {}", self.repeat_mode.setting_value());
    }

    fn remove_selected(&mut self) {
        let index = self.selected[2];
        if index >= self.tracks.len() || self.active_list == 0 {
            return;
        }
        if self.active_list == -1 {
            self.queue.remove(index);
            self.tracks = self.queue.clone();
        } else {
            let db = self.library.db();
            let result = db
                .playlist_entry_rows(self.active_list)
                .and_then(|rows| {
                    rows.get(index)
                        .map(|(id, _)| *id)
                        .ok_or_else(|| "row changed".to_owned())
                })
                .and_then(|id| db.delete_entry_rows(self.active_list, &[id]).map(|_| ()));
            if let Err(error) = result {
                self.status = error;
                return;
            }
            self.tracks.remove(index);
        }
        self.selected[2] = index.min(self.tracks.len().saturating_sub(1));
        if self.playing == Some(index) {
            self.player.stop();
            self.playing = None;
        } else if self.playing.is_some_and(|i| i > index) {
            self.playing = self.playing.map(|i| i - 1);
        }
        self.status = "Removed selected track".to_owned();
    }

    fn toggle_star(&mut self) {
        let track = match self.focus {
            Focus::Library => match self.items.get(self.selected[1]) {
                Some(TreeRow {
                    item: Item::Track(track),
                    ..
                }) => Some(track.clone()),
                _ => None,
            },
            Focus::Tracks => self.tracks.get(self.selected[2]).cloned(),
            Focus::Playlists => None,
        };
        let Some(track) = track else {
            return;
        };
        let locator = match kog_server::media_filter::locator_for(&track.entry) {
            Ok(locator) => locator,
            Err(error) => {
                self.status = error;
                return;
            }
        };
        let db = self.library.db();
        let starred = !db.is_starred(&locator);
        let result = db.set_star(
            &locator,
            &track.entry.kind,
            &track.entry.path,
            &track.entry.entry,
            track.entry.fragment.as_deref(),
            starred,
        );
        drop(db);
        match result {
            Ok(()) => {
                self.status = if starred {
                    format!("Starred {}", track.name)
                } else {
                    format!("Unstarred {}", track.name)
                };
                if self.active_list == 0 {
                    self.active_list = i64::MIN;
                    self.select_list(self.selected[0]);
                }
            }
            Err(error) => self.status = error,
        }
    }

    fn finish_prompt(&mut self, kind: PromptKind, value: String) {
        let value = value.trim();
        if value.is_empty() && !matches!(kind, PromptKind::PlaylistSearch) {
            self.status = "A value is required".to_owned();
            return;
        }
        match kind {
            PromptKind::MusicFolder => {
                let path = PathBuf::from(value)
                    .canonicalize()
                    .unwrap_or_else(|_| PathBuf::from(value));
                if !path.is_dir() {
                    self.status = format!("{} is not a directory", path.display());
                    return;
                }
                match AppSettings::save_music_directory(&path) {
                    Ok(()) => {
                        self.library.set_root(Some(path));
                        self.browse(None);
                    }
                    Err(error) => self.status = error,
                }
            }
            PromptKind::NewPlaylist => {
                let result = self.library.db().create_playlist(value);
                match result {
                    Ok(id) => {
                        self.reload_lists();
                        if let Some(index) =
                            self.lists.iter().position(|(list_id, _)| *list_id == id)
                        {
                            self.select_list(index);
                        }
                        self.status = format!("Created {value}");
                    }
                    Err(error) => self.status = error,
                }
            }
            PromptKind::RenamePlaylist => {
                if self.active_list <= 0 {
                    return;
                }
                let result = self.library.db().rename_playlist(self.active_list, value);
                match result {
                    Ok(()) => {
                        self.reload_lists();
                        self.status = format!("Renamed to {value}");
                    }
                    Err(error) => self.status = error,
                }
            }
            PromptKind::DeletePlaylist => {
                if self.active_list <= 0 {
                    return;
                }
                if value != "yes" {
                    self.status = "Playlist deletion cancelled".to_owned();
                    return;
                }
                let result = self.library.db().delete_playlist(self.active_list);
                match result {
                    Ok(()) => {
                        self.reload_lists();
                        self.select_list(0);
                        self.status = "Playlist deleted".to_owned();
                    }
                    Err(error) => self.status = error,
                }
            }
            PromptKind::Search => match LocalSearch::start(self.library.clone(), value) {
                Ok(search) => {
                    self.search = Some(search);
                    self.search_query = value.to_owned();
                    self.search_done = false;
                    self.items.clear();
                    self.selected[1] = 0;
                    self.offsets[1] = 0;
                    self.focus = Focus::Library;
                    self.status = format!("Searching for {value}…");
                }
                Err(error) => self.status = error,
            },
            PromptKind::PlaylistSearch => {
                self.playlist_query = value.to_lowercase();
                self.offsets[2] = 0;
                if let Some(first) = self.visible_tracks().first() {
                    self.selected[2] = *first;
                }
            }
        }
    }

    fn poll_search(&mut self) {
        let Some(search) = self.search.as_ref() else {
            return;
        };
        if self.search_done {
            return;
        }
        let (hits, done) = search.results_since(self.items.len());
        for hit in hits {
            if hit.kind == "dir" {
                self.items.push(TreeRow {
                    item: Item::Directory(hit.name, PathBuf::from(hit.path)),
                    depth: 0,
                });
            } else {
                self.items.push(TreeRow {
                    item: Item::Track(Track {
                        name: hit.name,
                        entry: StoredEntry {
                            kind: hit.kind.to_owned(),
                            path: hit.path,
                            entry: hit.entry,
                            fragment: None,
                        },
                    }),
                    depth: 0,
                });
            }
        }
        if done {
            self.search_done = true;
            self.status = format!("{} matches for {}", self.items.len(), self.search_query);
        } else if !self.items.is_empty() {
            self.status = format!(
                "{} matches so far for {}…",
                self.items.len(),
                self.search_query
            );
        }
    }

    fn move_selection(&mut self, delta: isize, page: usize) {
        let pane = self.pane_index();
        if pane == 2 && !self.playlist_query.is_empty() {
            let visible = self.visible_tracks();
            if visible.is_empty() {
                return;
            }
            let current = visible
                .iter()
                .position(|&row| row == self.selected[2])
                .unwrap_or(0);
            let next = current.saturating_add_signed(delta).min(visible.len() - 1);
            self.selected[2] = visible[next];
            if next < self.offsets[2] {
                self.offsets[2] = next;
            }
            if next >= self.offsets[2] + page {
                self.offsets[2] = next + 1 - page;
            }
            return;
        }
        let len = match pane {
            0 => self.lists.len(),
            1 => self.items.len(),
            _ => self.tracks.len(),
        };
        if len == 0 {
            return;
        }
        let current = self.selected[pane];
        self.selected[pane] = current.saturating_add_signed(delta).min(len - 1);
        if self.selected[pane] < self.offsets[pane] {
            self.offsets[pane] = self.selected[pane];
        }
        if self.selected[pane] >= self.offsets[pane] + page {
            self.offsets[pane] = self.selected[pane] + 1 - page;
        }
    }

    fn visible_tracks(&self) -> Vec<usize> {
        if self.playlist_query.is_empty() {
            return (0..self.tracks.len()).collect();
        }
        self.tracks
            .iter()
            .enumerate()
            .filter_map(|(index, track)| {
                self.title_for(track)
                    .to_lowercase()
                    .contains(&self.playlist_query)
                    .then_some(index)
            })
            .collect()
    }

    fn pane_index(&self) -> usize {
        match self.focus {
            Focus::Playlists => 0,
            Focus::Library => 1,
            Focus::Tracks => 2,
        }
    }

    fn key(&mut self, key: Key, size: (usize, usize)) -> bool {
        if let Some((kind, mut value)) = self.prompt.take() {
            match key {
                Key::Esc => self.status.clear(),
                Key::Enter => self.finish_prompt(kind, value),
                Key::Backspace => {
                    value.pop();
                    self.prompt = Some((kind, value));
                }
                Key::Char(c) if !c.is_control() && value.len() < 1024 => {
                    value.push(c);
                    self.prompt = Some((kind, value));
                }
                _ => self.prompt = Some((kind, value)),
            }
            return true;
        }
        if self.menu_open {
            match key {
                Key::Esc | Key::Char('m') => self.menu_open = false,
                Key::Up | Key::Char('k') => {
                    self.menu_selected = self.menu_selected.saturating_sub(1)
                }
                Key::Down | Key::Char('j') => self.menu_selected = (self.menu_selected + 1).min(5),
                Key::Enter => self.activate_menu(self.menu_selected),
                _ => {}
            }
            return true;
        }
        let layout = self.layout(size);
        let page = match self.focus {
            Focus::Playlists => layout.list_page,
            Focus::Library => layout.tree_bottom.saturating_sub(layout.tree_top) + 1,
            Focus::Tracks => layout.track_page,
        }
        .max(1);
        match key {
            Key::Char('q') | Key::CtrlC => return false,
            Key::Esc if self.search.is_some() => self.browse(None),
            Key::Esc if !self.playlist_query.is_empty() => self.playlist_query.clear(),
            Key::Char('/') => self.prompt = Some((PromptKind::Search, self.search_query.clone())),
            Key::Char('F') => {
                self.prompt = Some((PromptKind::PlaylistSearch, self.playlist_query.clone()))
            }
            Key::Char('c') => self.show_queue(),
            Key::Char('m') => self.menu_open = true,
            Key::Char('t') => {
                self.sidebar_visible = !self.sidebar_visible;
                if !self.sidebar_visible {
                    self.focus = Focus::Tracks;
                }
            }
            Key::Tab => {
                self.focus = match self.focus {
                    Focus::Playlists => Focus::Library,
                    Focus::Library => Focus::Tracks,
                    Focus::Tracks => Focus::Playlists,
                }
            }
            Key::BackTab => {
                self.focus = match self.focus {
                    Focus::Playlists => Focus::Tracks,
                    Focus::Library => Focus::Playlists,
                    Focus::Tracks => Focus::Library,
                }
            }
            Key::Up | Key::Char('k') => self.move_selection(-1, page),
            Key::Down | Key::Char('j') => self.move_selection(1, page),
            Key::PageUp => self.move_selection(-(page as isize), page),
            Key::PageDown => self.move_selection(page as isize, page),
            Key::Home => self.move_selection(-(isize::MAX / 2), page),
            Key::End => self.move_selection(isize::MAX / 2, page),
            Key::Left | Key::Backspace if self.focus == Focus::Library => self.up_directory(),
            Key::Right | Key::Enter if self.focus == Focus::Library => self.open_selected(),
            Key::Enter if self.focus == Focus::Playlists => self.select_list(self.selected[0]),
            Key::Enter if self.focus == Focus::Tracks => self.play_selected(),
            Key::Char('a') if self.focus == Focus::Library => self.add_selected(false),
            Key::Char('n') if self.focus == Focus::Playlists => {
                self.prompt = Some((PromptKind::NewPlaylist, String::new()))
            }
            Key::Char('r') if self.focus == Focus::Playlists && self.active_list > 0 => {
                let name = self
                    .lists
                    .get(self.selected[0])
                    .map(|(_, name)| name.clone())
                    .unwrap_or_default();
                self.prompt = Some((PromptKind::RenamePlaylist, name));
            }
            Key::Delete if self.focus == Focus::Playlists && self.active_list > 0 => {
                self.prompt = Some((PromptKind::DeletePlaylist, String::new()));
            }
            Key::Char('o') => {
                self.prompt = Some((
                    PromptKind::MusicFolder,
                    self.library
                        .root()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                ))
            }
            Key::Delete if self.focus == Focus::Tracks => self.remove_selected(),
            Key::Char('f') => self.toggle_star(),
            Key::Char(' ') => self.play_pause(),
            Key::Char('s') => {
                self.player.stop();
                self.playing = None;
            }
            Key::Char('>') => self.next(false),
            Key::Char('<') => self.previous(),
            Key::Char('R') => self.cycle_repeat(),
            Key::Char('+') | Key::Char('=') => {
                self.volume = (self.volume + 0.05).min(1.0);
                self.player.set_volume(self.volume);
            }
            Key::Char('-') => {
                self.volume = (self.volume - 0.05).max(0.0);
                self.player.set_volume(self.volume);
            }
            Key::Char('h') if self.focus == Focus::Tracks => {
                let _ = self.player.seek(
                    self.player
                        .position()
                        .saturating_sub(Duration::from_secs(10)),
                );
            }
            Key::Char('l') if self.focus == Focus::Tracks => {
                let _ = self
                    .player
                    .seek(self.player.position() + Duration::from_secs(10));
            }
            _ => {}
        }
        true
    }

    fn mouse(&mut self, button: u16, x: usize, y: usize, release: bool, size: (usize, usize)) {
        if release {
            return;
        }
        let layout = self.layout(size);
        if (button & 0b1100_0000) == 64 {
            self.focus = if layout.show_sidebar && x < layout.first {
                if y >= layout.lists_header {
                    Focus::Playlists
                } else {
                    Focus::Library
                }
            } else {
                Focus::Tracks
            };
            let page = match self.focus {
                Focus::Library => layout.tree_bottom.saturating_sub(layout.tree_top) + 1,
                Focus::Playlists => layout.list_page,
                Focus::Tracks => layout.track_page,
            };
            self.move_selection(if button & 1 == 0 { -3 } else { 3 }, page);
            return;
        }
        if button & 32 != 0 {
            return;
        }
        if button & 3 != 0 {
            return;
        }
        if self.menu_open && y > 0 {
            if (4..28).contains(&x) && (1..=MENU_ITEMS.len()).contains(&y) {
                self.menu_selected = y - 1;
                self.activate_menu(self.menu_selected);
            } else {
                self.menu_open = false;
            }
            return;
        }
        if y == 0 {
            if x < 4 {
                self.prompt = Some((
                    PromptKind::MusicFolder,
                    self.library
                        .root()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                ));
            } else if x < 8 {
                self.menu_open = !self.menu_open;
            } else if x < 12 {
                self.sidebar_visible = !self.sidebar_visible;
                if !self.sidebar_visible {
                    self.focus = Focus::Tracks;
                }
            } else if x >= size.0.saturating_sub(4) {
                self.exit_requested = true;
            } else {
                self.prompt = Some((PromptKind::PlaylistSearch, self.playlist_query.clone()));
            }
            return;
        }
        if y >= layout.footer_top {
            let start = size.0.saturating_div(2).saturating_sub(11);
            if y == layout.footer_top {
                if x >= size.0.saturating_sub(11) {
                    self.volume = ((x - size.0.saturating_sub(11)) as f32 / 10.0).clamp(0.0, 1.0);
                    self.player.set_volume(self.volume);
                    return;
                }
                match x.saturating_sub(start) / 5 {
                    0 if x >= start => self.previous(),
                    1 if x >= start => self.play_pause(),
                    2 if x >= start => {
                        self.player.stop();
                        self.playing = None;
                    }
                    3 if x >= start => self.next(false),
                    4 if x >= start => self.cycle_repeat(),
                    _ => {}
                }
            } else if y == layout.footer_top + 1 {
                let duration = self
                    .playing
                    .and_then(|index| self.tracks.get(index))
                    .and_then(|track| self.metadata_for(track))
                    .and_then(|meta| meta.duration);
                if let Some(duration) = duration.filter(|time| !time.is_zero()) {
                    let bar_left = start.saturating_sub(3) + 6;
                    let bar_width = size.0.saturating_div(2).min(32).max(4);
                    if (bar_left..bar_left + bar_width).contains(&x) {
                        let fraction = (x - bar_left) as f64 / (bar_width - 1) as f64;
                        let _ = self.player.seek(duration.mul_f64(fraction));
                    }
                }
            }
            return;
        }
        if layout.show_sidebar && x < layout.first {
            if y == 2 {
                if x < 4 {
                    self.prompt = Some((
                        PromptKind::MusicFolder,
                        self.library
                            .root()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    ));
                } else if x < 8 {
                    self.browse(None);
                }
                return;
            }
            if y == 3 {
                self.prompt = Some((PromptKind::Search, self.search_query.clone()));
                return;
            }
            if y == layout.lists_header {
                if x >= layout.first.saturating_sub(4) {
                    self.prompt = Some((PromptKind::NewPlaylist, String::new()));
                } else {
                    self.focus = Focus::Playlists;
                }
                return;
            }
            if y >= layout.list_top {
                self.focus = Focus::Playlists;
                let index = self.offsets[0] + y - layout.list_top;
                if index < self.lists.len() {
                    self.selected[0] = index;
                    self.select_list(index);
                }
                return;
            }
            if y >= layout.tree_top && y <= layout.tree_bottom {
                self.focus = Focus::Library;
                let index = self.offsets[1] + y - layout.tree_top;
                if index >= self.items.len() {
                    return;
                }
                self.selected[1] = index;
                if let Some(TreeRow {
                    item: Item::Directory(_, path),
                    depth,
                }) = self.items.get(index)
                {
                    if x <= depth * 2 + 2 {
                        self.toggle_directory(path.clone());
                        self.last_click = None;
                        return;
                    }
                }
                let now = Instant::now();
                let double = self.last_click.is_some_and(|(when, pane, row)| {
                    pane == 1
                        && row == index
                        && now.duration_since(when) < Duration::from_millis(450)
                });
                self.last_click = Some((now, 1, index));
                if double {
                    self.open_selected();
                    self.last_click = None;
                }
                return;
            }
        }
        if !layout.show_sidebar && self.focus == Focus::Library {
            let index = self.offsets[1] + y.saturating_sub(2);
            if index < self.items.len() {
                self.selected[1] = index;
                self.open_selected();
            }
            return;
        }
        if !layout.show_sidebar && self.focus == Focus::Playlists {
            if y == 1 {
                self.prompt = Some((PromptKind::NewPlaylist, String::new()));
            } else if y >= 2 {
                let index = self.offsets[0] + y - 2;
                if index < self.lists.len() {
                    self.selected[0] = index;
                    self.select_list(index);
                }
            }
            return;
        }
        if y >= 2 && x >= layout.first {
            self.focus = Focus::Tracks;
            let visible = self.visible_tracks();
            let Some(&index) = visible.get(self.offsets[2] + y - 2) else {
                return;
            };
            self.selected[2] = index;
            let now = Instant::now();
            let double = self.last_click.is_some_and(|(when, pane, row)| {
                pane == 2 && row == index && now.duration_since(when) < Duration::from_millis(450)
            });
            self.last_click = Some((now, 2, index));
            if double {
                self.play_selected();
                self.last_click = None;
            }
        }
    }

    fn draw(&mut self, size: (usize, usize)) -> String {
        let (width, height) = size;
        if width < 20 || height < 14 {
            let mut screen = String::from("\x1b[H\x1b[2J\x1b[?25l");
            paint(
                &mut screen,
                1,
                1,
                "Kog · Enlarge terminal",
                width,
                Surface::Toolbar,
                true,
            );
            return screen;
        }
        let layout = self.layout(size);
        let probe_range = if self.playlist_query.is_empty() {
            self.offsets[2].min(self.tracks.len())
                ..self
                    .tracks
                    .len()
                    .min(self.offsets[2] + layout.track_page + 1)
        } else {
            0..self.tracks.len()
        };
        let to_probe: Vec<_> = self.tracks[probe_range]
            .iter()
            .map(|track| track.entry.clone())
            .collect();
        for entry in &to_probe {
            self.request_metadata(entry);
        }
        let tree_page = layout.tree_bottom.saturating_sub(layout.tree_top) + 1;
        let pages = [layout.list_page, tree_page, layout.track_page];
        let visible_tracks = self.visible_tracks();
        for (pane, len) in [self.lists.len(), self.items.len(), visible_tracks.len()]
            .into_iter()
            .enumerate()
        {
            if len == 0 {
                self.selected[pane] = 0;
                self.offsets[pane] = 0;
                continue;
            }
            let selected_position = if pane == 2 {
                visible_tracks
                    .iter()
                    .position(|&row| row == self.selected[2])
                    .unwrap_or(0)
            } else {
                self.selected[pane] = self.selected[pane].min(len - 1);
                self.selected[pane]
            };
            let page = pages[pane].max(1);
            if selected_position < self.offsets[pane] {
                self.offsets[pane] = selected_position;
            } else if selected_position >= self.offsets[pane] + page {
                self.offsets[pane] = selected_position + 1 - page;
            }
        }

        let mut screen = String::from("\x1b[H\x1b[?25l");
        paint(&mut screen, 1, 1, "", width, Surface::Toolbar, false);
        paint(&mut screen, 1, 2, "⚙", 2, Surface::Accent, true);
        paint(&mut screen, 1, 6, "☰", 2, Surface::Toolbar, false);
        paint(&mut screen, 1, 10, "▤", 2, Surface::Toolbar, false);
        let search_x = (width / 2).saturating_sub(17).max(14);
        let search_width = width.saturating_sub(search_x + 7).min(36);
        let playlist_search = if self.playlist_query.is_empty() {
            " ⌕  Search playlist".to_owned()
        } else {
            format!(" ⌕  {}", self.playlist_query)
        };
        paint(
            &mut screen,
            1,
            search_x,
            &playlist_search,
            search_width,
            Surface::Main,
            false,
        );
        paint(
            &mut screen,
            1,
            width.saturating_sub(2),
            "×",
            1,
            Surface::Toolbar,
            false,
        );

        if layout.show_sidebar {
            let sidebar = layout.first;
            for y in 1..layout.footer_top {
                paint(&mut screen, y + 1, 1, "", sidebar, Surface::Sidebar, false);
            }
            paint(
                &mut screen,
                2,
                1,
                " ▾ Files",
                sidebar,
                Surface::Toolbar,
                true,
            );
            let root_location = self.browse_path.clone().or_else(|| self.library.root());
            let location = root_location
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Choose a music folder".to_owned());
            paint(
                &mut screen,
                3,
                1,
                &format!(" ▱  ↻  {location}"),
                sidebar,
                Surface::SidebarAlt,
                true,
            );
            let tree_search = if self.search.is_some() {
                format!(" ⌕  {}", self.search_query)
            } else {
                " ⌕  Search files and folders…".to_owned()
            };
            paint(
                &mut screen,
                4,
                1,
                &tree_search,
                sidebar,
                Surface::Main,
                false,
            );
            for y in layout.tree_top..=layout.tree_bottom {
                let index = self.offsets[1] + y - layout.tree_top;
                let surface = if index == self.selected[1] && self.focus == Focus::Library {
                    Surface::Selected
                } else if index % 2 == 1 {
                    Surface::SidebarAlt
                } else {
                    Surface::Sidebar
                };
                let label = self
                    .items
                    .get(index)
                    .map(|row| {
                        let indent = "  ".repeat(row.depth.min(12));
                        match &row.item {
                            Item::Directory(name, path) => {
                                let arrow = if self.expanded.contains(path) {
                                    "▾"
                                } else {
                                    "▸"
                                };
                                format!("{indent}{arrow} ▱ {name}")
                            }
                            Item::Track(track) => {
                                format!("{indent}  {} {}", glyph(&track.entry), track.name)
                            }
                        }
                    })
                    .unwrap_or_default();
                paint(&mut screen, y + 1, 1, &label, sidebar, surface, false);
            }
            paint(
                &mut screen,
                layout.lists_header + 1,
                1,
                " ▾ Playlists",
                sidebar,
                Surface::Toolbar,
                true,
            );
            paint(
                &mut screen,
                layout.lists_header + 1,
                sidebar.saturating_sub(1),
                "+",
                1,
                Surface::Toolbar,
                true,
            );
            for y in layout.list_top..layout.footer_top {
                let index = self.offsets[0] + y - layout.list_top;
                let surface = if index == self.selected[0] && self.focus == Focus::Playlists {
                    Surface::Selected
                } else {
                    Surface::Sidebar
                };
                let label = self
                    .lists
                    .get(index)
                    .map(|(id, name)| {
                        let icon = if *id == 0 { "★" } else { " " };
                        format!(" {icon}  {name}")
                    })
                    .unwrap_or_default();
                paint(&mut screen, y + 1, 1, &label, sidebar, surface, false);
            }
            for y in 1..layout.footer_top {
                paint(
                    &mut screen,
                    y + 1,
                    sidebar + 1,
                    "│",
                    1,
                    Surface::Toolbar,
                    false,
                );
            }
        }

        let right_x = layout.first + 2;
        let right_width = width.saturating_sub(layout.first + 1);
        let show_tree = !layout.show_sidebar && self.focus == Focus::Library;
        let show_lists = !layout.show_sidebar && self.focus == Focus::Playlists;
        if show_lists {
            paint(
                &mut screen,
                2,
                1,
                " ▾ Playlists                                 +",
                width,
                Surface::Header,
                true,
            );
            for y in 2..layout.footer_top {
                let index = self.offsets[0] + y - 2;
                let label = self
                    .lists
                    .get(index)
                    .map(|(id, name)| format!(" {}  {}", if *id == 0 { "★" } else { " " }, name))
                    .unwrap_or_default();
                let surface = if index == self.selected[0] {
                    Surface::Selected
                } else if index % 2 == 1 {
                    Surface::MainAlt
                } else {
                    Surface::Main
                };
                paint(&mut screen, y + 1, 1, &label, width, surface, false);
            }
        } else if show_tree {
            paint(&mut screen, 2, 1, " Files", width, Surface::Header, true);
            for y in 2..layout.footer_top {
                let index = self.offsets[1] + y - 2;
                let label = self
                    .items
                    .get(index)
                    .map(|row| match &row.item {
                        Item::Directory(name, path) => format!(
                            " {} ▱ {}",
                            if self.expanded.contains(path) {
                                "▾"
                            } else {
                                "▸"
                            },
                            name
                        ),
                        Item::Track(track) => format!("   {} {}", glyph(&track.entry), track.name),
                    })
                    .unwrap_or_default();
                let surface = if index == self.selected[1] {
                    Surface::Selected
                } else if index % 2 == 1 {
                    Surface::MainAlt
                } else {
                    Surface::Main
                };
                paint(&mut screen, y + 1, 1, &label, width, surface, false);
            }
        } else {
            let number_width = 5.min(right_width / 5);
            let title_width = (right_width * 45 / 100)
                .max(8)
                .min(right_width.saturating_sub(number_width));
            let artist_width = (right_width.saturating_sub(number_width + title_width)) / 2;
            let album_width = right_width.saturating_sub(number_width + title_width + artist_width);
            paint(
                &mut screen,
                2,
                right_x,
                "",
                right_width,
                Surface::Header,
                true,
            );
            paint(
                &mut screen,
                2,
                right_x,
                "#",
                number_width.saturating_sub(1),
                Surface::Header,
                true,
            );
            paint(
                &mut screen,
                2,
                right_x + number_width,
                "Title",
                title_width.saturating_sub(1),
                Surface::Header,
                true,
            );
            if artist_width > 0 {
                paint(
                    &mut screen,
                    2,
                    right_x + number_width + title_width,
                    "Artist",
                    artist_width.saturating_sub(1),
                    Surface::Header,
                    true,
                );
            }
            if album_width > 0 {
                paint(
                    &mut screen,
                    2,
                    right_x + number_width + title_width + artist_width,
                    "Album",
                    album_width,
                    Surface::Header,
                    true,
                );
            }
            for y in 2..layout.footer_top {
                let index = visible_tracks.get(self.offsets[2] + y - 2).copied();
                let surface = if index == Some(self.selected[2]) && self.focus == Focus::Tracks {
                    Surface::Selected
                } else if y % 2 == 1 {
                    Surface::MainAlt
                } else {
                    Surface::Main
                };
                paint(&mut screen, y + 1, right_x, "", right_width, surface, false);
                if let Some((index, track)) =
                    index.and_then(|index| self.tracks.get(index).map(|track| (index, track)))
                {
                    let number = format!(" {:>3}", index + 1);
                    let title = format!(
                        "{} {}",
                        if self.playing == Some(index) {
                            "▶"
                        } else {
                            glyph(&track.entry)
                        },
                        self.title_for(track)
                    );
                    paint(
                        &mut screen,
                        y + 1,
                        right_x,
                        &number,
                        number_width.saturating_sub(1),
                        surface,
                        false,
                    );
                    paint(
                        &mut screen,
                        y + 1,
                        right_x + number_width,
                        &title,
                        title_width.saturating_sub(1),
                        surface,
                        false,
                    );
                    if let Some(metadata) = self.metadata_for(track) {
                        if artist_width > 0 {
                            paint(
                                &mut screen,
                                y + 1,
                                right_x + number_width + title_width,
                                &metadata.artist,
                                artist_width.saturating_sub(1),
                                surface,
                                false,
                            );
                        }
                        if album_width > 0 {
                            paint(
                                &mut screen,
                                y + 1,
                                right_x + number_width + title_width + artist_width,
                                &metadata.album,
                                album_width,
                                surface,
                                false,
                            );
                        }
                    }
                }
            }
        }

        for y in layout.footer_top..height {
            paint(&mut screen, y + 1, 1, "", width, Surface::Toolbar, false);
        }
        let current = self
            .playing
            .and_then(|index| self.tracks.get(index))
            .map(|track| self.title_for(track))
            .unwrap_or_else(|| "Kog".to_owned());
        paint(
            &mut screen,
            layout.footer_top + 1,
            2,
            "⚙",
            2,
            Surface::Accent,
            true,
        );
        let title_width = width.saturating_div(2).saturating_sub(17);
        paint(
            &mut screen,
            layout.footer_top + 1,
            5,
            &current,
            title_width,
            Surface::Toolbar,
            true,
        );
        let subtitle = self
            .playing
            .and_then(|index| self.tracks.get(index))
            .and_then(|track| self.metadata_for(track))
            .map(|meta| {
                [meta.artist.as_str(), meta.album.as_str()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" • ")
            })
            .filter(|text| !text.is_empty())
            .unwrap_or_else(|| "Ready to play".to_owned());
        paint(
            &mut screen,
            layout.footer_top + 2,
            5,
            &subtitle,
            title_width,
            Surface::Muted,
            false,
        );
        let control_start = width.saturating_div(2).saturating_sub(11);
        let play = if self.player.state() == PlaybackState::Playing {
            "Ⅱ"
        } else {
            "▶"
        };
        for (index, icon) in ["◀", play, "■", "▶", "↻"].into_iter().enumerate() {
            paint(
                &mut screen,
                layout.footer_top + 1,
                control_start + index * 5,
                icon,
                2,
                Surface::Toolbar,
                true,
            );
        }
        let progress = self.player.position();
        let clock = format!(
            "{:01}:{:02}",
            progress.as_secs() / 60,
            progress.as_secs() % 60
        );
        let duration = self
            .playing
            .and_then(|index| self.tracks.get(index))
            .and_then(|track| self.metadata_for(track))
            .and_then(|meta| meta.duration);
        let duration_label = duration
            .map(|time| format!("{}:{:02}", time.as_secs() / 60, time.as_secs() % 60))
            .unwrap_or_else(|| "--:--".to_owned());
        let bar_width = width.saturating_div(2).min(32).max(4);
        let filled = duration
            .filter(|time| !time.is_zero())
            .map(|time| {
                ((progress.as_secs_f64() / time.as_secs_f64()).clamp(0.0, 1.0) * bar_width as f64)
                    .round() as usize
            })
            .unwrap_or(0);
        let progress_bar = format!("{}{}", "━".repeat(filled), "─".repeat(bar_width - filled));
        paint(
            &mut screen,
            layout.footer_top + 2,
            control_start.saturating_sub(3),
            &format!("{clock:>5}  {progress_bar}  {duration_label}"),
            width.saturating_sub(control_start),
            Surface::Toolbar,
            false,
        );
        paint(
            &mut screen,
            layout.footer_top + 1,
            width.saturating_sub(10),
            &format!("♪ {:>3}%", (self.volume * 100.0) as u8),
            10,
            Surface::Toolbar,
            false,
        );
        let message = self.prompt.as_ref().map(|(kind, value)| {
            format!("{}: {value}▏  Enter save · Esc cancel", match kind {
                PromptKind::MusicFolder => "Music folder",
                PromptKind::NewPlaylist => "New playlist",
                PromptKind::RenamePlaylist => "Rename playlist",
                PromptKind::DeletePlaylist => "Type yes to delete playlist",
                PromptKind::Search => "Search files",
                PromptKind::PlaylistSearch => "Search playlist",
            })
        }).unwrap_or_else(|| {
            if self.status.is_empty() {
                "Tab pane · Enter open/play · Space pause · m menu · t tree · / files · F playlist · Del remove".to_owned()
            } else { self.status.clone() }
        });
        paint(
            &mut screen,
            height,
            1,
            &format!(" {message}"),
            width,
            Surface::Toolbar,
            false,
        );
        if self.menu_open {
            for (index, label) in MENU_ITEMS.iter().enumerate() {
                paint(
                    &mut screen,
                    index + 2,
                    5,
                    label,
                    24,
                    if index == self.menu_selected {
                        Surface::Selected
                    } else {
                        Surface::Toolbar
                    },
                    index == self.menu_selected,
                );
            }
        }
        screen
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        let _ = AppSettings::save_output_volume(f64::from(self.volume));
    }
}

fn track_from_entry(entry: StoredEntry) -> Track {
    let name = if entry.kind == "archive" && !entry.entry.is_empty() {
        Path::new(&entry.entry)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    } else {
        Path::new(&entry.path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    };
    Track { name, entry }
}

fn display_title(track: &Track) -> String {
    Path::new(&track.name)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(&track.name)
        .to_owned()
}

fn metadata_key(entry: &StoredEntry) -> String {
    format!(
        "{}\0{}\0{}\0{}",
        entry.kind,
        entry.path,
        entry.entry,
        entry.fragment.as_deref().unwrap_or_default()
    )
}

fn playlist_entry(entry: &StoredEntry) -> PlaylistEntry {
    let location = match entry.kind.as_str() {
        "archive" => PlaylistLocation::Archive {
            archive_path: PathBuf::from(&entry.path),
            entry_name: entry.entry.clone(),
        },
        "remote" => PlaylistLocation::Remote(entry.path.clone()),
        _ => PlaylistLocation::Local(PathBuf::from(&entry.path)),
    };
    PlaylistEntry {
        location,
        fragment: entry.fragment.clone(),
    }
}

fn glyph(entry: &StoredEntry) -> &'static str {
    if entry.kind == "archive" {
        return "▣";
    }
    if entry.kind == "remote" {
        return "☁";
    }
    match Path::new(&entry.path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "mid" | "midi" | "kar" => "♪",
        "mp3" | "m4a" | "aac" => "♫",
        "flac" | "wav" | "aiff" => "◈",
        "vgm" | "vgz" | "spc" | "nsf" | "gbs" => "♬",
        "mod" | "xm" | "it" | "s3m" => "▦",
        _ => "♫",
    }
}

const MENU_ITEMS: [&str; 6] = [
    " ⌕  Search files",
    " ▱  Music folder",
    " +  New playlist",
    " ▤  Current playlist",
    " ↻  Repeat",
    " ×  Quit",
];

struct Layout {
    first: usize,
    show_sidebar: bool,
    tree_top: usize,
    tree_bottom: usize,
    lists_header: usize,
    list_top: usize,
    footer_top: usize,
    list_page: usize,
    track_page: usize,
}
impl Layout {
    fn new(width: usize, height: usize, lists: usize) -> Self {
        let show_sidebar = width >= 60;
        let first = if show_sidebar {
            (width / 3).clamp(23, 46)
        } else {
            0
        };
        let footer_top = height.saturating_sub(4);
        let list_page = lists.min(6).min(height.saturating_sub(13)).max(1);
        let lists_header = footer_top.saturating_sub(list_page + 1);
        let list_top = lists_header + 1;
        let tree_top = 5;
        let tree_bottom = lists_header.saturating_sub(1);
        Self {
            first,
            show_sidebar,
            tree_top,
            tree_bottom,
            lists_header,
            list_top,
            footer_top,
            list_page,
            track_page: footer_top.saturating_sub(2).max(1),
        }
    }
}

fn truncate(text: &str, width: usize) -> String {
    let clipped = text
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| c.width().unwrap_or(0))
        .sum::<usize>()
        > width;
    let limit = if clipped {
        width.saturating_sub(1)
    } else {
        width
    };
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        if c.is_control() {
            continue;
        }
        let cells = c.width().unwrap_or(0);
        if used + cells > limit {
            break;
        }
        out.push(c);
        used += cells;
    }
    if clipped && width > 0 {
        out.push('…');
        used += 1;
    }
    out.push_str(&" ".repeat(width.saturating_sub(used)));
    out
}

#[derive(Clone, Copy)]
enum Surface {
    Toolbar,
    Sidebar,
    SidebarAlt,
    Main,
    MainAlt,
    Header,
    Accent,
    Selected,
    Muted,
}

fn paint(
    out: &mut String,
    row: usize,
    col: usize,
    text: &str,
    width: usize,
    surface: Surface,
    bold: bool,
) {
    if width == 0 {
        return;
    }
    let (foreground, background) = match surface {
        Surface::Toolbar => ("226;231;235", "29;32;34"),
        Surface::Sidebar => ("222;226;230", "34;37;39"),
        Surface::SidebarAlt => ("222;226;230", "30;33;35"),
        Surface::Main => ("220;224;228", "25;27;29"),
        Surface::MainAlt => ("220;224;228", "32;35;37"),
        Surface::Header => ("225;230;235", "29;32;34"),
        Surface::Accent => ("103;179;233", "29;32;34"),
        Surface::Selected => ("245;248;251", "49;84;106"),
        Surface::Muted => ("151;160;168", "29;32;34"),
    };
    out.push_str(&format!(
        "\x1b[{row};{col}H\x1b[{};38;2;{foreground};48;2;{background}m{}\x1b[0m",
        if bold { "1" } else { "22" },
        truncate(text, width)
    ));
}

#[derive(Debug, PartialEq, Eq)]
enum Key {
    Char(char),
    CtrlC,
    Esc,
    Tab,
    BackTab,
    Enter,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
}
enum Event {
    Key(Key),
    Mouse {
        button: u16,
        x: usize,
        y: usize,
        release: bool,
    },
}

fn parse_event(bytes: &mut Vec<u8>) -> Option<Event> {
    if bytes.is_empty() {
        return None;
    }
    if bytes[0] >= 0x80 {
        let width = match bytes[0] {
            0xC2..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF4 => 4,
            _ => {
                bytes.drain(..1);
                return None;
            }
        };
        if bytes.len() < width {
            return None;
        }
        let key = std::str::from_utf8(&bytes[..width])
            .ok()
            .and_then(|value| value.chars().next());
        bytes.drain(..width);
        return key.map(|c| Event::Key(Key::Char(c)));
    }
    if bytes[0] == 0x1b {
        if bytes.len() < 2 {
            return None;
        }
        if bytes[1] != b'[' {
            bytes.drain(..1);
            return Some(Event::Key(Key::Esc));
        }
        if bytes.len() < 3 {
            return None;
        }
        if bytes[2] == b'<' {
            let end = bytes.iter().position(|b| *b == b'M' || *b == b'm')?;
            let release = bytes[end] == b'm';
            let text = String::from_utf8_lossy(&bytes[3..end]);
            let parts: Vec<_> = text
                .split(';')
                .filter_map(|p| p.parse::<usize>().ok())
                .collect();
            bytes.drain(..=end);
            return (parts.len() == 3).then(|| Event::Mouse {
                button: parts[0] as u16,
                x: parts[1].saturating_sub(1),
                y: parts[2].saturating_sub(1),
                release,
            });
        }
        let end = bytes
            .iter()
            .enumerate()
            .skip(2)
            .find(|(_, b)| (0x40..=0x7e).contains(*b))
            .map(|(i, _)| i)?;
        if end < 2 {
            bytes.drain(..=end);
            return None;
        }
        let seq = bytes[2..=end].to_vec();
        bytes.drain(..=end);
        let key = match seq.as_slice() {
            b"A" => Key::Up,
            b"B" => Key::Down,
            b"C" => Key::Right,
            b"D" => Key::Left,
            b"Z" => Key::BackTab,
            b"H" | b"1~" => Key::Home,
            b"F" | b"4~" => Key::End,
            b"3~" => Key::Delete,
            b"5~" => Key::PageUp,
            b"6~" => Key::PageDown,
            _ => return None,
        };
        return Some(Event::Key(key));
    }
    let byte = bytes.remove(0);
    let key = match byte {
        3 => Key::CtrlC,
        9 => Key::Tab,
        10 | 13 => Key::Enter,
        127 | 8 => Key::Backspace,
        b if b.is_ascii_graphic() || b == b' ' => Key::Char(b as char),
        _ => return None,
    };
    Some(Event::Key(key))
}

#[cfg(unix)]
struct Terminal {
    original: libc::termios,
}
#[cfg(unix)]
impl Terminal {
    fn open() -> Result<Self, String> {
        let mut original = std::mem::MaybeUninit::uninit();
        // SAFETY: stdin is a terminal and these pointers address initialized
        // termios storage owned by this call.
        if unsafe { libc::tcgetattr(libc::STDIN_FILENO, original.as_mut_ptr()) } != 0 {
            return Err(format!(
                "reading terminal mode: {}",
                io::Error::last_os_error()
            ));
        }
        let original = unsafe { original.assume_init() };
        let mut raw = original;
        unsafe { libc::cfmakeraw(&mut raw) };
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 0;
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw) } != 0 {
            return Err(format!(
                "setting terminal mode: {}",
                io::Error::last_os_error()
            ));
        }
        print!("\x1b[?1049h\x1b[2J\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?25l");
        io::stdout().flush().map_err(|e| e.to_string())?;
        Ok(Self { original })
    }

    fn size(&self) -> (usize, usize) {
        let mut size = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        if unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) } == 0 {
            (
                usize::from(size.ws_col).max(1),
                usize::from(size.ws_row).max(1),
            )
        } else {
            (80, 24)
        }
    }

    fn read(&self, buffer: &mut Vec<u8>) {
        let mut poll = libc::pollfd {
            fd: libc::STDIN_FILENO,
            events: libc::POLLIN,
            revents: 0,
        };
        if unsafe { libc::poll(&mut poll, 1, 100) } > 0 {
            let mut bytes = [0_u8; 4096];
            let count =
                unsafe { libc::read(libc::STDIN_FILENO, bytes.as_mut_ptr().cast(), bytes.len()) };
            if count > 0 {
                buffer.extend_from_slice(&bytes[..count as usize]);
            }
        }
    }
}
#[cfg(unix)]
impl Drop for Terminal {
    fn drop(&mut self) {
        print!("\x1b[?25h\x1b[?1006l\x1b[?1002l\x1b[?1000l\x1b[?1049l");
        let _ = io::stdout().flush();
        unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.original) };
    }
}

#[cfg(unix)]
pub fn run() -> Result<(), String> {
    let mut ui = Ui::new();
    let terminal = Terminal::open()?;
    let mut input = Vec::new();
    let mut last_size = (0, 0);
    let mut last_draw = Instant::now() - Duration::from_secs(1);
    let mut last_frame = String::new();
    let mut escape_pending = None::<Instant>;
    loop {
        ui.poll_search();
        ui.poll_metadata();
        let size = terminal.size();
        if size != last_size || last_draw.elapsed() >= Duration::from_millis(150) {
            let frame = ui.draw(size);
            if frame != last_frame {
                print!("{frame}");
                io::stdout().flush().map_err(|e| e.to_string())?;
                last_frame = frame;
            }
            last_size = size;
            last_draw = Instant::now();
        }
        if ui.player.finished() {
            ui.next(true);
        }
        terminal.read(&mut input);
        if input.as_slice() == [0x1b] {
            if escape_pending.is_some_and(|since| since.elapsed() >= Duration::from_millis(150)) {
                input.clear();
                ui.key(Key::Esc, size);
                escape_pending = None;
            } else if escape_pending.is_none() {
                escape_pending = Some(Instant::now());
            }
        } else {
            escape_pending = None;
        }
        while let Some(event) = parse_event(&mut input) {
            match event {
                Event::Key(key) => {
                    if !ui.key(key, size) {
                        return Ok(());
                    }
                }
                Event::Mouse {
                    button,
                    x,
                    y,
                    release,
                } => ui.mouse(button, x, y, release, size),
            }
            if ui.exit_requested {
                return Ok(());
            }
            let frame = ui.draw(size);
            if frame != last_frame {
                print!("{frame}");
                io::stdout().flush().map_err(|e| e.to_string())?;
                last_frame = frame;
            }
            last_draw = Instant::now();
        }
    }
}

#[cfg(not(unix))]
pub fn run() -> Result<(), String> {
    Err("the terminal UI is currently supported on Unix terminals".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parser_handles_mouse_and_keys() {
        let mut input = b"\x1b[<0;12;5M\x1b[3~".to_vec();
        assert!(matches!(
            parse_event(&mut input),
            Some(Event::Mouse {
                x: 11,
                y: 4,
                release: false,
                ..
            })
        ));
        assert!(matches!(
            parse_event(&mut input),
            Some(Event::Key(Key::Delete))
        ));
        let mut wheel = b"\x1b[<65;2;9M\x1b[<0;2;9m".to_vec();
        assert!(matches!(
            parse_event(&mut wheel),
            Some(Event::Mouse {
                button: 65,
                y: 8,
                ..
            })
        ));
        assert!(matches!(
            parse_event(&mut wheel),
            Some(Event::Mouse { release: true, .. })
        ));
        let mut unicode = "é".as_bytes().to_vec();
        assert!(matches!(
            parse_event(&mut unicode),
            Some(Event::Key(Key::Char('é')))
        ));
    }
    #[test]
    fn truncate_handles_wide_unicode() {
        assert_eq!(truncate("♫ 漢字", 4), "♫ … ");
    }
    #[test]
    fn layout_reflows_at_terminal_widths() {
        let wide = Layout::new(120, 40, 5);
        let narrow = Layout::new(48, 18, 5);
        assert!(wide.show_sidebar);
        assert!(!narrow.show_sidebar);
        assert_eq!(wide.first, 40);
        assert_eq!(narrow.first, 0);
        assert!(wide.tree_bottom < wide.lists_header);
        assert_eq!(wide.list_top + wide.list_page, wide.footer_top);
        assert_eq!(narrow.footer_top, 14);
    }
}
