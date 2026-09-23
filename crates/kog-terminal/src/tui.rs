//! A small terminal frontend. The terminal protocol, layout and hit testing
//! live here; browsing, persistence and playback use Kog's existing crates.
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use kog_audio::decoder::DecoderRegistry;
use kog_audio::playback::{PlaybackEngine, PlaybackState};
use kog_audio::playlist::{PlaylistEntry, PlaylistLocation};
use kog_audio::settings::{AppSettings, RepeatMode};
use kog_core::db::StoredEntry;
use kog_server::api::{Library, LocalSearch, browse_local};
use unicode_width::UnicodeWidthChar;

#[derive(Clone)]
struct Track {
    name: String,
    entry: StoredEntry,
}

#[derive(Clone)]
enum Item {
    Directory(String, PathBuf),
    Track(Track),
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
}

struct Ui {
    library: Arc<Library>,
    decoders: DecoderRegistry,
    player: PlaybackEngine,
    browse_path: Option<PathBuf>,
    items: Vec<Item>,
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
    search_done: bool,
    exit_requested: bool,
    action_page: usize,
}

impl Ui {
    fn new() -> Self {
        let settings = AppSettings::load();
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
            search_done: false,
            exit_requested: false,
            action_page: 0,
        };
        ui.reload_lists();
        ui.browse(None);
        ui
    }

    fn reload_lists(&mut self) {
        self.lists = vec![(-1, "Queue".to_owned()), (0, "Favorites".to_owned())];
        match self.library.db().list_playlists() {
            Ok(lists) => self.lists.extend(lists.into_iter().map(|p| (p.id, p.name))),
            Err(error) => self.status = error,
        }
    }

    fn browse(&mut self, path: Option<PathBuf>) {
        self.search = None;
        self.search_query.clear();
        let result = browse_local(&self.library, path.as_ref().and_then(|p| p.to_str()));
        match result {
            Ok(value) => {
                self.browse_path = value["path"].as_str().map(PathBuf::from);
                self.items.clear();
                for dir in value["directories"].as_array().into_iter().flatten() {
                    if let (Some(name), Some(path)) = (dir["name"].as_str(), dir["path"].as_str()) {
                        self.items
                            .push(Item::Directory(name.to_owned(), PathBuf::from(path)));
                    }
                }
                for file in value["files"].as_array().into_iter().flatten() {
                    if let (Some(name), Some(path), Some(kind)) = (
                        file["name"].as_str(),
                        file["path"].as_str(),
                        file["kind"].as_str(),
                    ) {
                        self.items.push(Item::Track(Track {
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
                self.selected[1] = 0;
                self.offsets[1] = 0;
                self.status.clear();
            }
            Err(error) => self.status = error,
        }
    }

    fn up_directory(&mut self) {
        if self.search.is_some() {
            self.browse(None);
            return;
        }
        let Some(current) = self.browse_path.as_ref() else {
            return;
        };
        let Some(root) = self.library.root() else {
            return;
        };
        if current == &root {
            return;
        }
        let parent = current
            .parent()
            .filter(|p| p.starts_with(&root))
            .map(Path::to_path_buf);
        self.browse(parent);
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
        let Some(Item::Track(track)) = self.items.get(self.selected[1]).cloned() else {
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
            Some(Item::Directory(_, path)) => self.browse(Some(path)),
            Some(Item::Track(_)) => self.add_selected(true),
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
                Some(Item::Track(track)) => Some(track.clone()),
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
        if value.is_empty() {
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
                self.items
                    .push(Item::Directory(hit.name, PathBuf::from(hit.path)));
            } else {
                self.items.push(Item::Track(Track {
                    name: hit.name,
                    entry: StoredEntry {
                        kind: hit.kind.to_owned(),
                        path: hit.path,
                        entry: hit.entry,
                        fragment: None,
                    },
                }));
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

    fn pane_index(&self) -> usize {
        match self.focus {
            Focus::Playlists => 0,
            Focus::Library => 1,
            Focus::Tracks => 2,
        }
    }

    fn key(&mut self, key: Key, height: usize) -> bool {
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
        let page = height.saturating_sub(5).max(1);
        match key {
            Key::Char('q') | Key::CtrlC => return false,
            Key::Esc if self.search.is_some() => self.browse(None),
            Key::Char('/') => self.prompt = Some((PromptKind::Search, self.search_query.clone())),
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
        let layout = Layout::new(size.0, size.1);
        if y == layout.controls_y + 1 {
            match action_at(size.0, self.action_page, x) {
                Some("Open") => match self.focus {
                    Focus::Library => self.open_selected(),
                    Focus::Tracks => self.play_selected(),
                    Focus::Playlists => self.select_list(self.selected[0]),
                },
                Some("Add") if self.focus == Focus::Library => self.add_selected(false),
                Some("Star") => self.toggle_star(),
                Some("Remove") if self.focus == Focus::Tracks => self.remove_selected(),
                Some("Remove") if self.focus == Focus::Playlists && self.active_list > 0 => {
                    self.prompt = Some((PromptKind::DeletePlaylist, String::new()));
                }
                Some("Search") => {
                    self.prompt = Some((PromptKind::Search, self.search_query.clone()))
                }
                Some("Folder") => {
                    self.prompt = Some((
                        PromptKind::MusicFolder,
                        self.library
                            .root()
                            .map(|p| p.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    ))
                }
                Some("New") => self.prompt = Some((PromptKind::NewPlaylist, String::new())),
                Some("Repeat") => self.cycle_repeat(),
                Some("More") => self.action_page += 1,
                Some("Back") => self.action_page = 0,
                Some("Quit") => self.exit_requested = true,
                _ => {}
            }
            return;
        }
        if y == layout.controls_y {
            if x < 9 {
                self.previous();
            } else if x < 20 {
                self.play_pause();
            } else if x < 30 {
                self.player.stop();
                self.playing = None;
            } else if x < 40 {
                self.next(false);
            } else if x < 43 {
                self.volume = (self.volume - 0.05).max(0.0);
                self.player.set_volume(self.volume);
            } else if x < 46 {
                self.volume = (self.volume + 0.05).min(1.0);
                self.player.set_volume(self.volume);
            }
            return;
        }
        let pane = if x < layout.first {
            0
        } else if x < layout.second {
            1
        } else {
            2
        };
        if pane == 0 && !layout.show_sidebar {
            return;
        }
        let pane = if !layout.show_sidebar && x < layout.second {
            if self.focus == Focus::Playlists { 0 } else { 1 }
        } else {
            pane
        };
        if (button & 0b1100_0000) == 64 {
            self.focus = [Focus::Playlists, Focus::Library, Focus::Tracks][pane];
            self.move_selection(if button & 1 == 0 { -3 } else { 3 }, layout.page);
            return;
        }
        if button & 3 != 0 || y <= 1 || y >= layout.controls_y {
            return;
        }
        self.focus = [Focus::Playlists, Focus::Library, Focus::Tracks][pane];
        let index = self.offsets[pane] + y - 2;
        let len = match pane {
            0 => self.lists.len(),
            1 => self.items.len(),
            _ => self.tracks.len(),
        };
        if index >= len {
            return;
        }
        self.selected[pane] = index;
        if button & 32 != 0 {
            self.last_click = None;
            return;
        }
        let now = Instant::now();
        let double = self
            .last_click
            .is_some_and(|(when, last_pane, last_index)| {
                last_pane == pane
                    && last_index == index
                    && now.duration_since(when) < Duration::from_millis(450)
            });
        self.last_click = Some((now, pane, index));
        if double {
            match pane {
                0 => self.select_list(index),
                1 => self.open_selected(),
                _ => self.play_selected(),
            }
            self.last_click = None;
        } else if pane == 0 {
            self.select_list(index);
        }
    }

    fn draw(&mut self, size: (usize, usize)) -> String {
        if size.0 < 20 || size.1 < 6 {
            let mut screen = String::from("\x1b[H\x1b[2J");
            line(&mut screen, 1, 1, "Kog: enlarge terminal", size.0, true);
            return screen;
        }
        let layout = Layout::new(size.0, size.1);
        for (pane, len) in [self.lists.len(), self.items.len(), self.tracks.len()]
            .into_iter()
            .enumerate()
        {
            if len == 0 {
                self.selected[pane] = 0;
                self.offsets[pane] = 0;
                continue;
            }
            self.selected[pane] = self.selected[pane].min(len - 1);
            let selected = self.selected[pane];
            if selected < self.offsets[pane] {
                self.offsets[pane] = selected;
            } else if selected >= self.offsets[pane] + layout.page {
                self.offsets[pane] = selected + 1 - layout.page;
            }
        }
        let mut screen = String::from("\x1b[H\x1b[2J\x1b[?25l");
        let location = if self.search.is_some() {
            format!("Search: {}", self.search_query)
        } else {
            self.browse_path
                .as_ref()
                .map_or("Choose a music folder in Settings", |p| {
                    p.to_str().unwrap_or("Library")
                })
                .to_owned()
        };
        line(
            &mut screen,
            1,
            1,
            &format!(" Kog  │  {location}"),
            size.0,
            true,
        );
        if layout.show_sidebar {
            column(
                &mut screen,
                1,
                layout.first.saturating_sub(1),
                2,
                layout.controls_y,
                "☰ Playlists",
                self.focus == Focus::Playlists,
                self.offsets[0],
                self.selected[0],
                self.lists.iter().map(|(_, name)| format!("♫ {name}")),
            );
        }
        if !layout.show_sidebar && self.focus == Focus::Playlists {
            column(
                &mut screen,
                1,
                layout.second,
                2,
                layout.controls_y,
                "☰ Playlists",
                true,
                self.offsets[0],
                self.selected[0],
                self.lists.iter().map(|(_, name)| format!("♫ {name}")),
            );
        } else {
            column(
                &mut screen,
                layout.first + 1,
                layout.second.saturating_sub(layout.first + 1),
                2,
                layout.controls_y,
                "▤ Library",
                self.focus == Focus::Library,
                self.offsets[1],
                self.selected[1],
                self.items.iter().map(|item| match item {
                    Item::Directory(name, _) => format!("▸ {name}"),
                    Item::Track(track) => format!("{} {}", glyph(&track.entry), track.name),
                }),
            );
        }
        column(
            &mut screen,
            layout.second + 1,
            size.0.saturating_sub(layout.second + 1),
            2,
            layout.controls_y,
            "♫ Playlist",
            self.focus == Focus::Tracks,
            self.offsets[2],
            self.selected[2],
            self.tracks.iter().enumerate().map(|(i, track)| {
                format!(
                    "{} {}",
                    if self.playing == Some(i) {
                        "▶"
                    } else {
                        glyph(&track.entry)
                    },
                    track.name
                )
            }),
        );
        let state = match self.player.state() {
            PlaybackState::Playing => "▶ Play",
            PlaybackState::Paused => "⏸ Pause",
            PlaybackState::Stopped => "▶ Play",
        };
        line(
            &mut screen,
            layout.controls_y + 1,
            1,
            &format!(
                " ⏮ Prev   {state}   ⏹ Stop   ⏭ Next   −  +   Vol {:>3}%   {:>02}:{:>02}   ↻ {}",
                (self.volume * 100.0) as u8,
                self.player.position().as_secs() / 60,
                self.player.position().as_secs() % 60,
                self.repeat_mode.setting_value()
            ),
            size.0,
            true,
        );
        line(
            &mut screen,
            layout.controls_y + 2,
            1,
            &action_bar(size.0, self.action_page),
            size.0,
            false,
        );
        let prompt = self.prompt.as_ref().map(|(kind, value)| {
            format!(
                "{}: {value}▏  (Enter save, Esc cancel)",
                match kind {
                    PromptKind::MusicFolder => "Music folder",
                    PromptKind::NewPlaylist => "New playlist",
                    PromptKind::RenamePlaylist => "Rename playlist",
                    PromptKind::DeletePlaylist => "Type yes to delete playlist",
                    PromptKind::Search => "Search library",
                }
            )
        });
        line(&mut screen, layout.controls_y + 3, 1, prompt.as_deref().unwrap_or(if self.status.is_empty() { " Tab pane · Arrows move · Enter open/play · Space pause · </> track · h/l seek · Esc clear search" } else { &self.status }), size.0, false);
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

const ACTIONS: [&str; 9] = [
    "Open", "Add", "Star", "Remove", "Search", "Folder", "New", "Repeat", "Quit",
];
const MEDIUM_ACTIONS: [&[&str]; 2] = [
    &["Open", "Add", "Star", "Remove", "More", "Quit"],
    &["Search", "Folder", "New", "Repeat", "Back", "Quit"],
];
const SMALL_ACTIONS: [&[&str]; 4] = [
    &["Open", "Add", "More", "Quit"],
    &["Star", "Remove", "More", "Quit"],
    &["Search", "Folder", "More", "Quit"],
    &["New", "Repeat", "Back", "Quit"],
];

fn action_labels(width: usize, page: usize) -> &'static [&'static str] {
    if width >= 75 {
        &ACTIONS
    } else if width >= 42 {
        MEDIUM_ACTIONS[page % MEDIUM_ACTIONS.len()]
    } else {
        SMALL_ACTIONS[page % SMALL_ACTIONS.len()]
    }
}

fn action_bar(width: usize, page: usize) -> String {
    format!(
        " {}",
        action_labels(width, page)
            .iter()
            .map(|name| format!("[{name}]"))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn action_at(width: usize, page: usize, x: usize) -> Option<&'static str> {
    let mut left = 1;
    for &name in action_labels(width, page) {
        let right = left + name.len() + 2;
        if (left..right).contains(&x) {
            return Some(name);
        }
        left = right + 1;
    }
    None
}

struct Layout {
    first: usize,
    second: usize,
    controls_y: usize,
    page: usize,
    show_sidebar: bool,
}
impl Layout {
    fn new(width: usize, height: usize) -> Self {
        let show_sidebar = width >= 50;
        let first = if show_sidebar { (width / 5).max(16) } else { 0 };
        let second = first + (width - first) / 2;
        let controls_y = height.saturating_sub(3).max(3);
        Self {
            first,
            second,
            controls_y,
            page: controls_y.saturating_sub(2).max(1),
            show_sidebar,
        }
    }
}

fn truncate(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        if c.is_control() {
            continue;
        }
        let cells = c.width().unwrap_or(0);
        if used + cells > width {
            break;
        }
        out.push(c);
        used += cells;
    }
    out.push_str(&" ".repeat(width.saturating_sub(used)));
    out
}

fn line(out: &mut String, row: usize, col: usize, text: &str, width: usize, strong: bool) {
    if width == 0 {
        return;
    }
    out.push_str(&format!(
        "\x1b[{row};{col}H{}{}\x1b[0m",
        if strong {
            "\x1b[1;37;44m"
        } else {
            "\x1b[37;40m"
        },
        truncate(text, width)
    ));
}

fn column(
    out: &mut String,
    x: usize,
    width: usize,
    top: usize,
    bottom: usize,
    title: &str,
    focused: bool,
    offset: usize,
    selected: usize,
    rows: impl Iterator<Item = String>,
) {
    if width == 0 {
        return;
    }
    line(out, top, x, title, width, focused);
    let mut visible = rows.skip(offset);
    for y in top + 1..=bottom {
        let index = offset + y - (top + 1);
        let text = visible.next().unwrap_or_default();
        out.push_str(&format!(
            "\x1b[{y};{x}H{}{}\x1b[0m",
            if index == selected && focused {
                "\x1b[30;46m"
            } else if index == selected {
                "\x1b[30;47m"
            } else {
                "\x1b[37;40m"
            },
            truncate(&text, width)
        ));
    }
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
        print!("\x1b[?1049h\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?25l");
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
                ui.key(Key::Esc, size.1);
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
                    if !ui.key(key, size.1) {
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
        assert_eq!(truncate("♫ 漢字", 4), "♫ 漢");
    }
    #[test]
    fn layout_reflows_at_terminal_widths() {
        let wide = Layout::new(120, 40);
        let narrow = Layout::new(48, 18);
        assert!(wide.show_sidebar);
        assert!(!narrow.show_sidebar);
        assert!(wide.first < wide.second && wide.second < 120);
        assert_eq!(narrow.controls_y, 15);
        assert_eq!(action_at(100, 0, 2), Some("Open"));
        assert!(action_bar(48, 1).contains("[Quit]"));
    }
}
