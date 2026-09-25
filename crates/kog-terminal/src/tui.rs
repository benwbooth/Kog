//! A small terminal frontend. The terminal protocol, layout and hit testing
//! live here; browsing, persistence and playback use Kog's existing crates.
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::io::{self, Write};
use std::net::IpAddr;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use kog_audio::decoder::{DecoderRegistry, DecoderSettings, PlaybackSource, validate_soundfont};
use kog_audio::playback::{PlaybackEngine, PlaybackState, available_output_devices};
use kog_audio::playback_order::PlaybackOrder;
use kog_audio::playlist::{Playlist, PlaylistEntry, PlaylistLocation};
use kog_audio::settings::{
    AppSettings, MidiEngine, OpeningFilesBehavior, OutputDevicePreference, RepeatMode, ShuffleMode,
};
use kog_audio::track::Track as AudioTrack;
use kog_core::db::{BLACKLIST_FOLDER, BLACKLIST_SONG, BlacklistEntry, StoredEntry};
use kog_core::equalizer::{EqualizerSettings, presets};
use kog_server::api::{Library, LocalSearch, browse_local, expand_stored_entry};
use kog_server::radio::{Radio, RadioAdvance, RadioEntry, RadioStatus};
use kog_server::{AuthMode, StreamCodec, TlsMode};
use rand::Rng;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::columns::Columns;
use crate::cover_preview::{self, COVER_HEIGHT, COVER_WIDTH, CoverPreview};
use crate::remote::{RemoteFile, RemoteListing, RemoteSettings};
use crate::rom_import::{RomKind, import_rom_archive};
use crate::server_control::RunningServer;
use crate::tag_editor::{parse_edits, snapshot_json, write_tags};

#[derive(Clone)]
struct Track {
    name: String,
    entry: StoredEntry,
}

struct AutoFitCache {
    columns: Vec<(&'static str, bool)>,
    rows_by_key: HashMap<String, Vec<usize>>,
    row_widths: Vec<Vec<usize>>,
    width_counts: Vec<BTreeMap<usize, usize>>,
}

const SESSION_FILE: &str = "tui-session.json";
const SESSION_MAX_BYTES: usize = 64 * 1024 * 1024;
const SESSION_MAX_TRACKS: usize = 100_000;
const RADIO_READY_TARGET: usize = 10;
const MEDIA_PLAY: &str = "▶️";
const MEDIA_PAUSE: &str = "⏸️";
const MEDIA_STOP: &str = "⏹️";
const MEDIA_PREVIOUS: &str = "⏮️";
const MEDIA_NEXT: &str = "⏭️";
const MEDIA_SHUFFLE: &str = "🔀";
const MEDIA_REPEAT: &str = "🔁";
const MEDIA_REPEAT_ONE: &str = "🔂";
const MEDIA_RADIO: &str = "📻";

struct RestoredPlaylist {
    tracks: Vec<Track>,
    selected: usize,
}

fn load_playlist_session(path: &Path) -> Option<RestoredPlaylist> {
    let contents = std::fs::read(path).ok()?;
    if contents.len() > SESSION_MAX_BYTES {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&contents).ok()?;
    let saved = value.get("tracks")?.as_array()?;
    if saved.len() > SESSION_MAX_TRACKS {
        return None;
    }
    let mut selected = value
        .get("selectedIndex")
        .and_then(serde_json::Value::as_u64)
        .and_then(|index| usize::try_from(index).ok())
        .unwrap_or(0);
    let mut tracks = Vec::with_capacity(saved.len());
    for (index, row) in saved.iter().enumerate() {
        let kind = row.get("kind").and_then(serde_json::Value::as_str);
        let path = row.get("path").and_then(serde_json::Value::as_str);
        let entry = row
            .get("entry")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let valid = matches!(kind, Some("local" | "archive" | "remote"))
            && path.is_some_and(|path| !path.is_empty())
            && (kind != Some("archive") || !entry.is_empty());
        if !valid {
            if index < selected {
                selected -= 1;
            }
            continue;
        }
        let stored = StoredEntry {
            kind: kind.unwrap().to_owned(),
            path: path.unwrap().to_owned(),
            entry: entry.to_owned(),
            fragment: row
                .get("fragment")
                .and_then(serde_json::Value::as_str)
                .filter(|fragment| !fragment.is_empty())
                .map(str::to_owned),
        };
        let saved_name = row
            .get("name")
            .and_then(serde_json::Value::as_str)
            .filter(|name| !name.is_empty() && name.len() <= 8_192);
        let mut track = track_from_entry(stored);
        if let Some(name) = saved_name {
            track.name = name.to_owned();
        }
        tracks.push(track);
    }
    Some(RestoredPlaylist {
        selected: selected.min(tracks.len().saturating_sub(1)),
        tracks,
    })
}

fn save_playlist_session(path: &Path, tracks: &[Track], selected: usize) -> Result<(), String> {
    if tracks.len() > SESSION_MAX_TRACKS {
        return Err("the current playlist is too large to save as a session".to_owned());
    }
    let rows: Vec<_> = tracks
        .iter()
        .map(|track| {
            serde_json::json!({
                "name": track.name,
                "kind": track.entry.kind,
                "path": track.entry.path,
                "entry": track.entry.entry,
                "fragment": track.entry.fragment,
            })
        })
        .collect();
    let contents = serde_json::to_vec(&serde_json::json!({
        "tracks": rows,
        "selectedIndex": selected,
    }))
    .map_err(|error| format!("serializing terminal playlist session: {error}"))?;
    if contents.len() > SESSION_MAX_BYTES {
        return Err("the current playlist session is too large to save".to_owned());
    }
    let parent = path
        .parent()
        .ok_or("Invalid terminal playlist session path")?;
    std::fs::create_dir_all(parent)
        .map_err(|error| format!("creating {}: {error}", parent.display()))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("creating terminal playlist session: {error}"))?;
    temporary
        .write_all(&contents)
        .and_then(|()| temporary.flush())
        .map_err(|error| format!("writing terminal playlist session: {error}"))?;
    temporary
        .persist(path)
        .map_err(|error| format!("replacing {}: {}", path.display(), error.error))?;
    Ok(())
}

struct TrackMetadata {
    title: String,
    artist: String,
    album: String,
    album_artist: String,
    composer: String,
    year: Option<u32>,
    sample_rate: Option<u32>,
    channels: Option<u16>,
    bits_per_sample: Option<u8>,
    bitrate: Option<u32>,
    disc_number: Option<u32>,
    track_number: Option<u32>,
    duration: Option<Duration>,
    file_size_bytes: Option<u64>,
    lyrics: String,
    codec: String,
    genre: String,
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

fn tree_item_key(item: &Item) -> String {
    match item {
        Item::Directory(_, path) => format!("directory\0{}", path.display()),
        Item::Track(track) => format!(
            "track\0{}\0{}\0{}\0{}",
            track.entry.kind,
            track.entry.path,
            track.entry.entry,
            track.entry.fragment.as_deref().unwrap_or_default()
        ),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Playlists,
    Library,
    Tracks,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuPage {
    Main,
    View,
    Playback,
    Preferences,
    Tree,
    Tracks,
    Saved,
    Playlist,
    Columns,
    ColumnVisibility,
    TagEditor,
    Remote,
    Synthesis,
    Server,
}

#[derive(Clone, Copy)]
struct MenuLayer {
    page: MenuPage,
    selected: usize,
    offset: usize,
    x: usize,
    y: usize,
}

impl MenuPage {
    fn title(self) -> &'static str {
        match self {
            Self::Main => "Kog",
            Self::View => "View",
            Self::Playback => "Playback",
            Self::Preferences => "Preferences",
            Self::Tree => "File",
            Self::Tracks => "Playlist Row",
            Self::Saved => "Saved Playlist",
            Self::Playlist => "Playlist",
            Self::Columns => "Columns",
            Self::ColumnVisibility => "Visible Columns",
            Self::TagEditor => "Edit Tags",
            Self::Remote => "Remote Server",
            Self::Synthesis => "MIDI Synthesis",
            Self::Server => "API Server",
        }
    }
    fn labels(self) -> &'static [&'static str] {
        match self {
            Self::Main => &MAIN_MENU,
            Self::View => &VIEW_MENU,
            Self::Playback => &PLAYBACK_MENU,
            Self::Preferences => &PREFERENCES_MENU,
            Self::Tree => &TREE_MENU,
            Self::Tracks => &TRACKS_MENU,
            Self::Saved => &SAVED_MENU,
            Self::Playlist => &PLAYLIST_MENU,
            Self::Columns => &COLUMNS_MENU,
            Self::ColumnVisibility => &COLUMN_VISIBILITY_MENU,
            Self::TagEditor => &TAG_EDITOR_MENU,
            Self::Remote => &REMOTE_MENU,
            Self::Synthesis => &SYNTHESIS_MENU,
            Self::Server => &SERVER_MENU,
        }
    }
}

const MAIN_MENU_SHORTCUTS: [Option<char>; 16] = [
    Some('A'),
    Some('U'),
    Some('M'),
    None,
    Some('S'),
    Some('E'),
    Some('R'),
    Some('C'),
    None,
    Some('V'),
    Some('B'),
    Some('P'),
    None,
    Some('T'),
    Some('K'),
    Some('Q'),
];

fn menu_shortcuts(page: MenuPage) -> Vec<Option<char>> {
    if page == MenuPage::Main {
        return MAIN_MENU_SHORTCUTS.to_vec();
    }
    let mut used = HashSet::new();
    page.labels()
        .iter()
        .map(|label| {
            if label.is_empty() {
                return None;
            }
            let mut candidates = Vec::new();
            let mut word_start = true;
            for character in label.chars() {
                if character.is_ascii_alphabetic() {
                    if word_start {
                        candidates.push(character.to_ascii_uppercase());
                    }
                    word_start = false;
                } else {
                    word_start = true;
                }
            }
            candidates.extend(
                label
                    .chars()
                    .filter(|character| character.is_ascii_alphabetic())
                    .map(|character| character.to_ascii_uppercase()),
            );
            candidates.extend('A'..='Z');
            candidates.into_iter().find(|key| used.insert(*key))
        })
        .collect()
}

fn menu_shortcut_index(page: MenuPage, key: char) -> Option<usize> {
    menu_shortcuts(page)
        .iter()
        .position(|shortcut| shortcut.is_some_and(|shortcut| shortcut == key.to_ascii_uppercase()))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PromptKind {
    MusicFolder,
    NewPlaylist,
    RenamePlaylist,
    DeletePlaylist,
    Search,
    PlaylistSearch,
    AddFile,
    AddUrl,
    SavePlaylist,
    SaveSelection,
    DuplicatePlaylist,
    ExportPlaylist,
    Volume,
    SeekPosition,
    AddToPlaylist,
    EqualizerPreset,
    EqualizerBandNumber,
    EqualizerBandGain(usize),
    Preamp,
    OutputDevice,
    RemoveBlacklistId,
    TrashSelected,
    TagField(usize),
    TagArtwork,
    RemoteUrl,
    RemoteToken,
    RemoteUsername,
    RemotePassword,
    SoundFont,
    Sc55Roms,
    Mt32Roms,
    Sc55Archive,
    Mt32Archive,
    ServerAddress,
    ServerPort,
    ServerToken,
    ServerUsername,
    ServerPassword,
    ServerCertificate,
    ServerPrivateKey,
    ServerCache,
    ServerDevice,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FolderFocus {
    List,
    Location,
    Choose,
    Cancel,
}

struct FolderEntry {
    name: String,
    path: PathBuf,
    parent: bool,
}

struct FolderChooser {
    directory: PathBuf,
    entries: Vec<FolderEntry>,
    selected: usize,
    offset: usize,
    location: String,
    cursor: usize,
    select_all: bool,
    focus: FolderFocus,
    show_hidden: bool,
    error: String,
    last_click: Option<(Instant, usize)>,
}

#[derive(Clone, Copy)]
struct FolderGeometry {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl FolderGeometry {
    fn new(size: (usize, usize)) -> Self {
        let width = size.0.saturating_sub(4).min(82);
        let height = size.1.saturating_sub(4).min(26);
        Self {
            x: (size.0 - width) / 2,
            y: (size.1 - height) / 2,
            width,
            height,
        }
    }

    fn list_top(self) -> usize {
        self.y + 4
    }

    fn list_rows(self) -> usize {
        self.height.saturating_sub(7)
    }
}

impl FolderChooser {
    fn new(initial: &Path) -> Result<Self, String> {
        let fallback = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let start = if initial.is_dir() {
            initial.to_path_buf()
        } else {
            initial
                .parent()
                .filter(|path| path.is_dir())
                .map(Path::to_path_buf)
                .unwrap_or(fallback)
        };
        let mut chooser = Self {
            directory: PathBuf::new(),
            entries: Vec::new(),
            selected: 0,
            offset: 0,
            location: String::new(),
            cursor: 0,
            select_all: false,
            focus: FolderFocus::List,
            show_hidden: false,
            error: String::new(),
            last_click: None,
        };
        chooser.navigate(&start)?;
        Ok(chooser)
    }

    fn list_entries(directory: &Path, show_hidden: bool) -> Result<Vec<FolderEntry>, String> {
        let mut entries = Vec::new();
        if let Some(parent) = directory.parent() {
            entries.push(FolderEntry {
                name: "..".to_owned(),
                path: parent.to_path_buf(),
                parent: true,
            });
        }
        let listing = std::fs::read_dir(directory)
            .map_err(|error| format!("Reading {}: {error}", directory.display()))?;
        for result in listing {
            let Ok(entry) = result else { continue };
            let name = entry.file_name().to_string_lossy().into_owned();
            if !show_hidden && name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let is_dir = entry
                .file_type()
                .is_ok_and(|kind| kind.is_dir() || kind.is_symlink() && path.is_dir());
            if is_dir {
                entries.push(FolderEntry {
                    name,
                    path,
                    parent: false,
                });
            }
        }
        let first = usize::from(entries.first().is_some_and(|entry| entry.parent));
        entries[first..].sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(entries)
    }

    fn navigate(&mut self, path: &Path) -> Result<(), String> {
        let directory = path
            .canonicalize()
            .map_err(|error| format!("Opening {}: {error}", path.display()))?;
        if !directory.is_dir() {
            return Err(format!("{} is not a folder", directory.display()));
        }
        let entries = Self::list_entries(&directory, self.show_hidden)?;
        self.directory = directory;
        self.entries = entries;
        self.selected = usize::from(self.entries.len() > 1 && self.entries[0].parent);
        self.offset = 0;
        self.location = self.directory.to_string_lossy().into_owned();
        self.cursor = self.location.chars().count();
        self.select_all = false;
        self.error.clear();
        self.last_click = None;
        Ok(())
    }

    fn open_selected(&mut self) {
        if let Some(path) = self
            .entries
            .get(self.selected)
            .map(|entry| entry.path.clone())
        {
            if let Err(error) = self.navigate(&path) {
                self.error = error;
            }
        }
    }

    fn parent(&mut self) {
        if let Some(path) = self.directory.parent().map(Path::to_path_buf) {
            if let Err(error) = self.navigate(&path) {
                self.error = error;
            }
        }
    }

    fn navigate_location(&mut self) {
        let typed = self.location.trim();
        let path = if typed == "~" || typed.starts_with("~/") {
            let home = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default();
            home.join(typed.strip_prefix("~/").unwrap_or_default())
        } else {
            let path = PathBuf::from(typed);
            if path.is_absolute() {
                path
            } else {
                self.directory.join(path)
            }
        };
        match self.navigate(&path) {
            Ok(()) => self.focus = FolderFocus::List,
            Err(error) => self.error = error,
        }
    }

    fn toggle_hidden(&mut self) {
        let old = self
            .entries
            .get(self.selected)
            .map(|entry| entry.path.clone());
        self.show_hidden = !self.show_hidden;
        match Self::list_entries(&self.directory, self.show_hidden) {
            Ok(entries) => {
                self.entries = entries;
                self.selected = old
                    .and_then(|path| self.entries.iter().position(|entry| entry.path == path))
                    .unwrap_or(0);
                self.offset = 0;
                self.error.clear();
            }
            Err(error) => self.error = error,
        }
    }

    fn keep_selected_visible(&mut self, rows: usize) {
        let rows = rows.max(1);
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + rows {
            self.offset = self.selected + 1 - rows;
        }
    }

    fn edit_location(&mut self, key: Key) {
        self.cursor = self.cursor.min(self.location.chars().count());
        match key {
            Key::CtrlA => self.select_all = true,
            Key::CtrlU => {
                self.location.clear();
                self.cursor = 0;
                self.select_all = false;
            }
            Key::Left | Key::CtrlLeft => {
                self.select_all = false;
                self.cursor = if key == Key::CtrlLeft {
                    previous_path_boundary(&self.location, self.cursor)
                } else {
                    self.cursor.saturating_sub(1)
                };
            }
            Key::Right | Key::CtrlRight => {
                self.select_all = false;
                self.cursor = if key == Key::CtrlRight {
                    next_path_boundary(&self.location, self.cursor)
                } else {
                    (self.cursor + 1).min(self.location.chars().count())
                };
            }
            Key::Home => {
                self.cursor = 0;
                self.select_all = false;
            }
            Key::End => {
                self.cursor = self.location.chars().count();
                self.select_all = false;
            }
            Key::Backspace | Key::Delete | Key::CtrlW | Key::CtrlBackspace => {
                if self.select_all {
                    self.location.clear();
                    self.cursor = 0;
                    self.select_all = false;
                } else if matches!(key, Key::CtrlW | Key::CtrlBackspace) {
                    let start = if key == Key::CtrlW {
                        previous_path_word_boundary(&self.location, self.cursor)
                    } else {
                        previous_path_boundary(&self.location, self.cursor)
                    };
                    self.location.drain(
                        byte_offset(&self.location, start)
                            ..byte_offset(&self.location, self.cursor),
                    );
                    self.cursor = start;
                } else if key == Key::Backspace && self.cursor > 0 {
                    self.cursor -= 1;
                    self.location
                        .remove(byte_offset(&self.location, self.cursor));
                } else if key == Key::Delete && self.cursor < self.location.chars().count() {
                    self.location
                        .remove(byte_offset(&self.location, self.cursor));
                }
            }
            Key::Char(character) if !character.is_control() && self.location.len() < 4096 => {
                if self.select_all {
                    self.location.clear();
                    self.cursor = 0;
                    self.select_all = false;
                }
                self.location
                    .insert(byte_offset(&self.location, self.cursor), character);
                self.cursor += 1;
            }
            _ => {}
        }
        self.error.clear();
    }
}

enum RadioCommand {
    Enable(bool),
    Reshuffle,
    Advance,
}
enum RadioResponse {
    Status(RadioStatus),
    Advance(RadioAdvance),
}

struct TagSession {
    sources: Vec<PlaybackSource>,
    snapshot: serde_json::Value,
    fields: serde_json::Map<String, serde_json::Value>,
    artwork: Option<serde_json::Value>,
}

enum TagCommand {
    Open(Vec<PlaybackSource>),
    Save(Vec<PlaybackSource>, String),
}

enum TagResponse {
    Open(Vec<PlaybackSource>, Result<serde_json::Value, String>),
    Saved(Vec<PathBuf>, Option<String>),
}

struct RomImportResult {
    kind: RomKind,
    result: Result<(PathBuf, usize, Vec<String>), String>,
}

enum RemoteCommand {
    Root(u64, RemoteSettings, Option<String>),
    Expand(u64, RemoteSettings, PathBuf),
    Collect(u64, u64, RemoteSettings, PathBuf),
    Search(u64, u64, RemoteSettings, String),
    AddTracks(u64, u64, RemoteSettings, Vec<RemoteFile>, bool),
}

enum RemoteResponse {
    Root(u64, RemoteSettings, Result<RemoteListing, String>),
    Expand(u64, PathBuf, RemoteSettings, Result<RemoteListing, String>),
    Collect(
        u64,
        u64,
        PathBuf,
        RemoteSettings,
        Result<Vec<RemoteFile>, String>,
    ),
    Search(u64, u64, RemoteSettings, Result<Vec<RemoteFile>, String>),
    AddTracks(
        u64,
        u64,
        RemoteSettings,
        bool,
        Result<Vec<RemoteFile>, String>,
    ),
}

struct Ui {
    library: Arc<Library>,
    decoder_settings: DecoderSettings,
    decoders: DecoderRegistry,
    player: PlaybackEngine,
    browse_path: Option<PathBuf>,
    items: Vec<TreeRow>,
    root_items: Vec<Item>,
    expanded: HashSet<PathBuf>,
    children: HashMap<PathBuf, Vec<Item>>,
    selected_tree: HashSet<String>,
    tree_anchor: Option<usize>,
    lists: Vec<(i64, String)>,
    list_counts: HashMap<i64, usize>,
    selected_lists: HashSet<i64>,
    list_anchor: Option<usize>,
    tracks: Vec<Track>,
    queue: VecDeque<usize>,
    stop_after_rows: HashSet<usize>,
    selected_tracks: HashSet<usize>,
    selection_anchor: Option<usize>,
    range_click_pending: Option<Focus>,
    session_dirty: bool,
    selected: [usize; 3],
    offsets: [usize; 3],
    focus: Focus,
    playing: Option<usize>,
    status: String,
    marquee_started: Instant,
    volume: f32,
    volume_before_mute: f32,
    volume_drag: bool,
    repeat_mode: RepeatMode,
    shuffle_mode: ShuffleMode,
    order: PlaybackOrder,
    stop_after_current: bool,
    radio_enabled: bool,
    radio_pool: VecDeque<Track>,
    radio_pending_next: bool,
    radio_refill_pending: bool,
    radio_exhausted: bool,
    radio_generation: u64,
    radio_requests: Sender<(u64, RadioCommand, Option<PathBuf>)>,
    radio_results: Receiver<(u64, RadioResponse)>,
    equalizer: EqualizerSettings,
    last_click: Option<(Instant, usize, usize)>,
    prompt: Option<(PromptKind, String)>,
    folder_chooser: Option<FolderChooser>,
    input_cursor: usize,
    input_select_all: bool,
    search_due: Option<Instant>,
    search: Option<LocalSearch>,
    search_query: String,
    playlist_query: String,
    metadata: HashMap<String, Option<TrackMetadata>>,
    metadata_pending: HashSet<String>,
    metadata_generation: u64,
    metadata_requests: Sender<(u64, String, StoredEntry)>,
    metadata_results: Receiver<(u64, String, Option<TrackMetadata>)>,
    cover_requests: Sender<(u64, Option<PathBuf>, String, String, bool)>,
    cover_results: Receiver<(u64, Option<CoverPreview>)>,
    cover_generation: u64,
    cover_live_generation: Arc<AtomicU64>,
    cover_key: String,
    cover_preview: Option<CoverPreview>,
    download_cover_art: bool,
    folder_requests: Sender<(u64, PathBuf)>,
    folder_results: Receiver<(u64, PathBuf, Result<Vec<Track>, String>)>,
    folder_generation: u64,
    folder_play_pending: HashSet<PathBuf>,
    delete_requests: Sender<PathBuf>,
    delete_results: Receiver<(PathBuf, Result<(), String>)>,
    pending_delete_paths: Vec<PathBuf>,
    tag_requests: Sender<TagCommand>,
    tag_results: Receiver<TagResponse>,
    tag_session: Option<TagSession>,
    tag_busy: bool,
    tag_resume: Option<(usize, Duration, PlaybackState)>,
    rom_import_requests: Sender<(RomKind, PathBuf)>,
    rom_import_results: Receiver<RomImportResult>,
    api_server: Option<RunningServer>,
    pending_server_certificate: Option<PathBuf>,
    remote_settings: RemoteSettings,
    remote_connection: Option<RemoteSettings>,
    remote_active: bool,
    remote_generation: u64,
    remote_path: String,
    remote_pending: HashSet<PathBuf>,
    remote_requests: Sender<RemoteCommand>,
    remote_results: Receiver<RemoteResponse>,
    remote_search_generation: Arc<AtomicU64>,
    search_done: bool,
    exit_requested: bool,
    menu_open: bool,
    menu_selected: usize,
    menu_page: MenuPage,
    menu_offset: usize,
    menu_x: usize,
    menu_y: usize,
    menu_parents: Vec<MenuLayer>,
    menu_size: (usize, usize),
    modal: Option<String>,
    info_modal: bool,
    artwork_modal: bool,
    visualizer_open: bool,
    modal_scroll: usize,
    sidebar_visible: bool,
    compact_mode: bool,
    files_expanded: bool,
    playlists_expanded: bool,
    sidebar_width: Option<usize>,
    split_drag: bool,
    column_drag: Option<usize>,
    column_scroll_drag: Option<usize>,
    vertical_scroll_drag: Option<(usize, usize)>,
    manual_scroll_selection: [Option<usize>; 3],
    track_drag: Option<usize>,
    sort_column: Option<usize>,
    sort_ascending: bool,
    columns: Columns,
    auto_fit_cache: Option<AutoFitCache>,
    context_column: Option<usize>,
    keyboard_column: Option<usize>,
    column_viewport_width: usize,
    starred_keys: HashSet<String>,
}

impl Ui {
    fn new() -> Self {
        let settings = AppSettings::load();
        let decoder_settings = settings.decoder_settings();
        let (metadata_requests, worker_requests) = mpsc::channel::<(u64, String, StoredEntry)>();
        let (worker_results, metadata_results) = mpsc::channel();
        let metadata_settings = decoder_settings.clone();
        std::thread::spawn(move || {
            let decoders = DecoderRegistry::new(metadata_settings);
            while let Ok((generation, key, entry)) = worker_requests.recv() {
                let resolved = kog_audio::streaming::resolve_entry(
                    &playlist_entry(&entry),
                    &decoders,
                    &kog_server::service::scratch_root().join("tui-metadata"),
                );
                let metadata = resolved.ok().map(|source| {
                    let track = AudioTrack::from_source(source, &decoders);
                    let title = if (entry.kind == "archive" || entry.kind == "remote")
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
                        album_artist: track.album_artist,
                        composer: track.composer,
                        year: track.year,
                        sample_rate: track.sample_rate,
                        channels: track.channels,
                        bits_per_sample: track.bits_per_sample,
                        bitrate: track.bitrate,
                        disc_number: track.disc_number,
                        track_number: track.track_number,
                        duration: track.duration,
                        file_size_bytes: track.file_size_bytes,
                        lyrics: track.lyrics,
                        codec: track.codec,
                        genre: track.genre,
                    }
                });
                if worker_results.send((generation, key, metadata)).is_err() {
                    break;
                }
            }
        });
        let (cover_requests, cover_jobs) =
            mpsc::channel::<(u64, Option<PathBuf>, String, String, bool)>();
        let (cover_done, cover_results) = mpsc::channel();
        let cover_live_generation = Arc::new(AtomicU64::new(0));
        let cover_worker_generation = cover_live_generation.clone();
        std::thread::spawn(move || {
            while let Ok((generation, file, artist, album, allow_download)) = cover_jobs.recv() {
                if cover_worker_generation.load(Ordering::Relaxed) != generation {
                    continue;
                }
                let cover = cover_preview::resolve(
                    file.as_deref(),
                    &artist,
                    &album,
                    allow_download,
                    || cover_worker_generation.load(Ordering::Relaxed) != generation,
                );
                if cover_done.send((generation, cover)).is_err() {
                    break;
                }
            }
        });
        let library = Arc::new(Library::open());
        let (folder_requests, pending_folders) = mpsc::channel::<(u64, PathBuf)>();
        let (completed_folders, folder_results) = mpsc::channel();
        let folder_library = library.clone();
        std::thread::spawn(move || {
            let decoders = DecoderRegistry::new(AppSettings::load().decoder_settings());
            while let Ok((generation, path)) = pending_folders.recv() {
                let settings = AppSettings::load();
                let result = collect_folder(
                    &folder_library,
                    &decoders,
                    path.clone(),
                    settings.read_cue_sheets_in_folders,
                    settings.read_playlists_in_folders,
                );
                if completed_folders.send((generation, path, result)).is_err() {
                    break;
                }
            }
        });
        let (delete_requests, delete_jobs) = mpsc::channel::<PathBuf>();
        let (delete_done, delete_results) = mpsc::channel();
        std::thread::spawn(move || {
            while let Ok(path) = delete_jobs.recv() {
                let result = trash::delete(&path).map_err(|error| error.to_string());
                if delete_done.send((path, result)).is_err() {
                    break;
                }
            }
        });
        let (tag_requests, tag_jobs) = mpsc::channel::<TagCommand>();
        let (tag_done, tag_results) = mpsc::channel::<TagResponse>();
        std::thread::spawn(move || {
            while let Ok(command) = tag_jobs.recv() {
                let response = match command {
                    TagCommand::Open(sources) => {
                        TagResponse::Open(sources.clone(), snapshot_json(&sources))
                    }
                    TagCommand::Save(sources, request) => match parse_edits(&request) {
                        Ok(edits) => {
                            let outcome = write_tags(&sources, &edits);
                            TagResponse::Saved(outcome.updated_paths, outcome.error)
                        }
                        Err(error) => TagResponse::Saved(Vec::new(), Some(error)),
                    },
                };
                if tag_done.send(response).is_err() {
                    break;
                }
            }
        });
        let (rom_import_requests, rom_import_jobs) = mpsc::channel::<(RomKind, PathBuf)>();
        let (rom_import_done, rom_import_results) = mpsc::channel::<RomImportResult>();
        std::thread::spawn(move || {
            while let Ok((kind, archive)) = rom_import_jobs.recv() {
                let result = import_rom_archive(&archive, kind).and_then(|imported| {
                    let validation = match kind {
                        RomKind::Sc55 => {
                            kog_audio::sc55::validate_rom_directory(&imported.directory)
                                .map(|model| format!("{model:?}"))
                        }
                        RomKind::Mt32 => {
                            kog_audio::mt32::validate_rom_directory(&imported.directory)
                                .map(|model| format!("{model:?}"))
                        }
                    };
                    if let Err(error) = validation {
                        let _ = std::fs::remove_dir_all(&imported.directory);
                        return Err(format!("Incomplete ROM set: {error}"));
                    }
                    Ok((imported.directory, imported.file_count, imported.warnings))
                });
                if rom_import_done
                    .send(RomImportResult { kind, result })
                    .is_err()
                {
                    break;
                }
            }
        });
        let (remote_requests, remote_jobs) = mpsc::channel::<RemoteCommand>();
        let (remote_done, remote_results) = mpsc::channel::<RemoteResponse>();
        let remote_search_generation = Arc::new(AtomicU64::new(0));
        let worker_search_generation = remote_search_generation.clone();
        std::thread::spawn(move || {
            while let Ok(command) = remote_jobs.recv() {
                let response = match command {
                    RemoteCommand::Root(generation, settings, path) => {
                        let result = settings.browse(path.as_deref());
                        RemoteResponse::Root(generation, settings, result)
                    }
                    RemoteCommand::Expand(generation, settings, path) => {
                        let result = settings.browse(path.to_str());
                        RemoteResponse::Expand(generation, path, settings, result)
                    }
                    RemoteCommand::Collect(generation, folder_generation, settings, path) => {
                        let result = settings.collect_folder(&path);
                        RemoteResponse::Collect(
                            generation,
                            folder_generation,
                            path,
                            settings,
                            result,
                        )
                    }
                    RemoteCommand::Search(generation, search_generation, settings, query) => {
                        let result = settings.search(&query, || {
                            worker_search_generation.load(Ordering::Relaxed) != search_generation
                        });
                        RemoteResponse::Search(generation, search_generation, settings, result)
                    }
                    RemoteCommand::AddTracks(
                        generation,
                        folder_generation,
                        settings,
                        files,
                        play,
                    ) => {
                        let result = settings.expand_files(&files);
                        RemoteResponse::AddTracks(
                            generation,
                            folder_generation,
                            settings,
                            play,
                            result,
                        )
                    }
                };
                if remote_done.send(response).is_err() {
                    break;
                }
            }
        });
        let decoders = DecoderRegistry::new(decoder_settings.clone());
        let (radio_requests, radio_commands) =
            mpsc::channel::<(u64, RadioCommand, Option<PathBuf>)>();
        let (radio_responses, radio_results) = mpsc::channel();
        let radio_library = library.clone();
        std::thread::spawn(move || {
            let radio = Radio::from_settings();
            while let Ok((generation, command, scope)) = radio_commands.recv() {
                let root = radio_library.root();
                let response = match command {
                    RadioCommand::Enable(enabled) => RadioResponse::Status(
                        radio.set_enabled_incremental(
                            enabled,
                            root.as_deref(),
                            scope.as_deref(),
                        ),
                    ),
                    RadioCommand::Reshuffle => RadioResponse::Status(
                        radio.reshuffle_incremental(root.as_deref(), scope.as_deref()),
                    ),
                    RadioCommand::Advance => RadioResponse::Advance(
                        radio.advance_incremental(root.as_deref(), scope.as_deref()),
                    ),
                };
                if radio_responses.send((generation, response)).is_err() {
                    break;
                }
            }
        });
        let mut player = PlaybackEngine::with_equalizer_and_output(
            DecoderRegistry::new(decoder_settings.clone()),
            settings.equalizer.clone(),
            settings
                .output_device
                .as_ref()
                .map(|device| device.id.clone()),
        );
        let volume = settings.output_volume as f32;
        player.set_volume(volume);
        let columns = Columns::load(&settings);
        let starred_keys = library
            .db()
            .starred_entries()
            .unwrap_or_default()
            .iter()
            .map(metadata_key)
            .collect();
        let mut ui = Self {
            library,
            decoder_settings,
            decoders,
            player,
            browse_path: None,
            items: Vec::new(),
            root_items: Vec::new(),
            expanded: HashSet::new(),
            children: HashMap::new(),
            selected_tree: HashSet::new(),
            tree_anchor: None,
            lists: Vec::new(),
            list_counts: HashMap::new(),
            selected_lists: HashSet::new(),
            list_anchor: None,
            tracks: Vec::new(),
            queue: VecDeque::new(),
            stop_after_rows: HashSet::new(),
            selected_tracks: HashSet::new(),
            selection_anchor: None,
            range_click_pending: None,
            session_dirty: false,
            selected: [0; 3],
            offsets: [0; 3],
            focus: Focus::Library,
            playing: None,
            status: String::new(),
            marquee_started: Instant::now(),
            volume,
            volume_before_mute: if volume > 0.0 { volume } else { 0.75 },
            volume_drag: false,
            repeat_mode: settings.repeat_mode,
            shuffle_mode: settings.shuffle_mode,
            order: PlaybackOrder::new(
                settings.shuffle_mode,
                settings.repeat_mode,
                rand::rng().random(),
            ),
            stop_after_current: false,
            radio_enabled: settings.radio_enabled,
            radio_pool: VecDeque::new(),
            radio_pending_next: false,
            radio_refill_pending: false,
            radio_exhausted: false,
            radio_generation: 0,
            radio_requests,
            radio_results,
            equalizer: settings.equalizer,
            last_click: None,
            prompt: None,
            folder_chooser: None,
            input_cursor: 0,
            input_select_all: false,
            search_due: None,
            search: None,
            search_query: String::new(),
            playlist_query: String::new(),
            metadata: HashMap::new(),
            metadata_pending: HashSet::new(),
            metadata_generation: 0,
            metadata_requests,
            metadata_results,
            cover_requests,
            cover_results,
            cover_generation: 0,
            cover_live_generation,
            cover_key: String::new(),
            cover_preview: None,
            download_cover_art: settings.download_cover_art,
            folder_requests,
            folder_results,
            folder_generation: 0,
            folder_play_pending: HashSet::new(),
            delete_requests,
            delete_results,
            pending_delete_paths: Vec::new(),
            tag_requests,
            tag_results,
            tag_session: None,
            tag_busy: false,
            tag_resume: None,
            rom_import_requests,
            rom_import_results,
            api_server: None,
            pending_server_certificate: None,
            remote_settings: RemoteSettings::load(),
            remote_connection: None,
            remote_active: false,
            remote_generation: 0,
            remote_path: String::new(),
            remote_pending: HashSet::new(),
            remote_requests,
            remote_results,
            remote_search_generation,
            search_done: false,
            exit_requested: false,
            menu_open: false,
            menu_selected: 0,
            menu_page: MenuPage::Main,
            menu_offset: 0,
            menu_x: 4,
            menu_y: 1,
            menu_parents: Vec::new(),
            menu_size: (0, 0),
            modal: None,
            info_modal: false,
            artwork_modal: false,
            visualizer_open: false,
            modal_scroll: 0,
            sidebar_visible: true,
            compact_mode: false,
            files_expanded: true,
            playlists_expanded: true,
            sidebar_width: None,
            split_drag: false,
            column_drag: None,
            column_scroll_drag: None,
            vertical_scroll_drag: None,
            manual_scroll_selection: [None; 3],
            track_drag: None,
            sort_column: None,
            sort_ascending: true,
            columns,
            auto_fit_cache: None,
            context_column: None,
            keyboard_column: None,
            column_viewport_width: 40,
            starred_keys,
        };
        if let Some(session) = kog_audio::settings::setting_path(SESSION_FILE)
            .as_deref()
            .and_then(load_playlist_session)
        {
            ui.tracks = session.tracks;
            ui.selected[2] = session.selected;
            if !ui.tracks.is_empty() {
                ui.selected_tracks.insert(session.selected);
                ui.selection_anchor = Some(session.selected);
            }
            ui.order_tracks_changed();
            ui.session_dirty = false;
        }
        ui.reload_lists();
        ui.browse(None);
        if kog_server::config::load_config().enabled {
            ui.start_api_server();
        }
        if ui.radio_enabled {
            ui.request_radio_command(RadioCommand::Enable(true));
        }
        ui
    }

    fn reload_lists(&mut self) {
        self.lists = vec![(0, "Favorites".to_owned())];
        self.list_counts.clear();
        match self.library.db().list_playlists() {
            Ok(lists) => {
                for playlist in lists {
                    self.list_counts.insert(
                        playlist.id,
                        usize::try_from(playlist.entry_count).unwrap_or(0),
                    );
                    self.lists.push((playlist.id, playlist.name));
                }
            }
            Err(error) => self.status = error,
        }
        self.selected_lists
            .retain(|id| self.lists.iter().any(|(current, _)| current == id));
    }

    fn saved_list_label(&self, index: usize, width: usize, tick: usize) -> String {
        let Some((id, name)) = self.lists.get(index) else {
            return String::new();
        };
        let count = if *id == 0 {
            self.starred_keys.len()
        } else {
            self.list_counts.get(id).copied().unwrap_or(0)
        };
        saved_list_marquee_label(name, *id == 0, count, width, tick)
    }

    fn poll_metadata(&mut self) {
        let mut order_changed = false;
        let mut changed_keys = Vec::new();
        while let Ok((generation, key, metadata)) = self.metadata_results.try_recv() {
            if generation != self.metadata_generation {
                continue;
            }
            self.metadata_pending.remove(&key);
            order_changed |= metadata.as_ref().is_some_and(|meta| !meta.album.is_empty());
            self.metadata.insert(key.clone(), metadata);
            changed_keys.push(key);
        }
        self.refresh_auto_fit_metadata(&changed_keys);
        if order_changed && self.shuffle_mode == ShuffleMode::Albums {
            let tracks = self.order_tracks();
            self.order.album_metadata_changed(&tracks, self.playing);
        }
    }

    fn refresh_cover_request(&mut self) {
        let Some(track) = self.playing.and_then(|index| self.tracks.get(index)) else {
            if !self.cover_key.is_empty() {
                self.cover_generation = self.cover_generation.wrapping_add(1);
                self.cover_live_generation
                    .store(self.cover_generation, Ordering::Relaxed);
            }
            self.cover_key.clear();
            self.cover_preview = None;
            return;
        };
        let (artist, album) = self.metadata_for(track).map_or_else(
            || (String::new(), String::new()),
            |meta| (meta.artist.clone(), meta.album.clone()),
        );
        let key = format!(
            "{}\0{}\0{}\0{}",
            metadata_key(&track.entry),
            artist,
            album,
            self.download_cover_art
        );
        if key == self.cover_key {
            return;
        }
        self.cover_key = key;
        self.cover_preview = None;
        self.cover_generation = self.cover_generation.wrapping_add(1);
        self.cover_live_generation
            .store(self.cover_generation, Ordering::Relaxed);
        let file = (track.entry.kind == "local").then(|| PathBuf::from(&track.entry.path));
        let _ = self.cover_requests.send((
            self.cover_generation,
            file,
            artist,
            album,
            self.download_cover_art,
        ));
    }

    fn poll_cover_preview(&mut self) {
        while let Ok((generation, cover)) = self.cover_results.try_recv() {
            if generation == self.cover_generation {
                self.cover_preview = cover;
            }
        }
    }

    fn invalidate_metadata(&mut self) {
        self.metadata_generation = self.metadata_generation.wrapping_add(1);
        self.metadata.clear();
        self.metadata_pending.clear();
        self.auto_fit_cache = None;
    }

    fn order_tracks(&self) -> Vec<AudioTrack> {
        self.tracks
            .iter()
            .map(|track| {
                let mut ordered = AudioTrack::default();
                if let Some(meta) = self.metadata_for(track) {
                    ordered.album = meta.album.clone();
                    ordered.disc_number = meta.disc_number;
                    ordered.track_number = meta.track_number;
                }
                ordered
            })
            .collect()
    }

    fn order_tracks_changed(&mut self) {
        let tracks = self.order_tracks();
        self.order.tracks_changed(&tracks, self.playing);
        self.session_dirty = true;
        self.auto_fit_cache = None;
    }

    fn flush_session(&mut self) -> Result<(), String> {
        if !self.session_dirty {
            return Ok(());
        }
        let path = kog_audio::settings::setting_path(SESSION_FILE)
            .ok_or("Cannot find the terminal playlist settings directory")?;
        save_playlist_session(&path, &self.tracks, self.selected[2])?;
        self.session_dirty = false;
        Ok(())
    }

    fn finish_session(&mut self) -> Result<(), String> {
        // Save the final selection even if the playlist has not changed.
        self.session_dirty = true;
        self.flush_session()
    }

    fn poll_radio(&mut self) {
        loop {
            let (generation, response) = match self.radio_results.try_recv() {
                Ok(result) => result,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if self.radio_enabled && !self.radio_exhausted {
                        self.radio_exhausted = true;
                        self.status = "Random Radio worker stopped".to_owned();
                    }
                    break;
                }
            };
            if generation != self.radio_generation {
                continue;
            }
            self.radio_refill_pending = false;
            match response {
                RadioResponse::Status(status) => {
                    self.radio_enabled = status.enabled;
                    self.radio_pool = status.entries.into_iter().map(radio_track).collect();
                    if !status.enabled {
                        self.radio_pending_next = false;
                    }
                }
                RadioResponse::Advance(advance) => {
                    self.radio_exhausted = advance.exhausted;
                    self.radio_pool
                        .extend(advance.entries.into_iter().map(radio_track));
                }
            }
            if self.radio_pending_next && !self.radio_pool.is_empty() {
                self.append_next_radio();
            }
            if self.radio_pending_next && self.radio_exhausted && self.radio_pool.is_empty() {
                self.radio_pending_next = false;
                self.status = "Random Radio found no playable tracks".to_owned();
            }
            self.top_up_radio_pool();
        }
    }

    fn request_radio_command(&mut self, command: RadioCommand) {
        if self
            .radio_requests
            .send((self.radio_generation, command, self.browse_path.clone()))
            .is_ok()
        {
            self.radio_refill_pending = true;
        } else {
            self.radio_exhausted = true;
            self.status = "Random Radio worker stopped".to_owned();
        }
    }

    fn top_up_radio_pool(&mut self) {
        if self.radio_enabled
            && !self.radio_refill_pending
            && !self.radio_exhausted
            && self.radio_pool.len() < RADIO_READY_TARGET
        {
            self.request_radio_command(RadioCommand::Advance);
        }
    }

    fn toggle_radio(&mut self) {
        self.radio_generation = self.radio_generation.wrapping_add(1);
        self.radio_enabled = !self.radio_enabled;
        self.radio_pending_next = false;
        self.radio_refill_pending = false;
        self.radio_exhausted = false;
        if self.radio_enabled {
            self.repeat_mode = RepeatMode::Off;
            self.shuffle_mode = ShuffleMode::Off;
            self.order.set_repeat_mode(self.repeat_mode);
            self.order
                .set_shuffle_mode(self.shuffle_mode, &self.order_tracks(), self.playing);
            let _ = AppSettings::save_repeat_mode(self.repeat_mode);
            let _ = AppSettings::save_shuffle_mode(self.shuffle_mode);
        } else {
            self.radio_pool.clear();
        }
        self.status = format!(
            "Random Radio: {}",
            if self.radio_enabled { "on" } else { "off" }
        );
        self.request_radio_command(RadioCommand::Enable(self.radio_enabled));
    }

    fn activate_transport(&mut self, action: TransportAction) {
        match action {
            TransportAction::Shuffle => self.cycle_shuffle(),
            TransportAction::Previous => self.previous(),
            TransportAction::PlayPause => self.play_pause(),
            TransportAction::Stop => {
                self.player.stop();
                self.playing = None;
            }
            TransportAction::Next => self.next(false),
            TransportAction::Repeat => self.cycle_repeat(),
            TransportAction::Radio => self.toggle_radio(),
        }
    }

    fn reshuffle_radio(&mut self) {
        self.radio_generation = self.radio_generation.wrapping_add(1);
        self.radio_enabled = true;
        self.radio_pool.clear();
        self.radio_pending_next = false;
        self.radio_refill_pending = false;
        self.radio_exhausted = false;
        self.repeat_mode = RepeatMode::Off;
        self.shuffle_mode = ShuffleMode::Off;
        self.order.set_repeat_mode(self.repeat_mode);
        self.order
            .set_shuffle_mode(self.shuffle_mode, &self.order_tracks(), self.playing);
        let _ = AppSettings::save_repeat_mode(self.repeat_mode);
        let _ = AppSettings::save_shuffle_mode(self.shuffle_mode);
        self.status = "Reshuffling Random Radio…".to_owned();
        self.request_radio_command(RadioCommand::Reshuffle);
    }

    fn append_next_radio(&mut self) {
        self.radio_pending_next = false;
        if let Some(track) = self.radio_pool.pop_front() {
            self.tracks.push(track);
            self.order_tracks_changed();
            self.selected[2] = self.tracks.len() - 1;
            self.select_track(self.selected[2], false, false);
            self.play_selected();
            self.top_up_radio_pool();
            return;
        }
        if self.radio_exhausted {
            self.status = "Random Radio found no playable tracks".to_owned();
            return;
        }
        self.radio_pending_next = true;
        self.top_up_radio_pool();
    }

    fn request_metadata(&mut self, entry: &StoredEntry) {
        let key = metadata_key(entry);
        if self.metadata.contains_key(&key) || !self.metadata_pending.insert(key.clone()) {
            return;
        }
        let _ = self
            .metadata_requests
            .send((self.metadata_generation, key, entry.clone()));
    }

    fn metadata_for(&self, track: &Track) -> Option<&TrackMetadata> {
        self.metadata
            .get(&metadata_key(&track.entry))
            .and_then(Option::as_ref)
    }

    fn title_for(&self, track: &Track) -> String {
        let mut title = self
            .metadata_for(track)
            .map(|meta| meta.title.clone())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| display_title(track));
        if let Some((_, suffix)) = numbered_track_name(&track.name)
            && track.entry.fragment.is_some()
            && !title.ends_with(suffix)
        {
            title.push_str(suffix);
        }
        title
    }

    fn column_value(&self, index: usize, track: &Track, id: &str) -> String {
        let meta = self.metadata_for(track);
        match id {
            "index" => format!(" {:>3}", index + 1),
            "star" => if self.starred_keys.contains(&metadata_key(&track.entry)) {
                "★"
            } else {
                ""
            }
            .to_owned(),
            "status" => {
                if self.stop_after_rows.contains(&index) {
                    MEDIA_STOP.to_owned()
                } else if self.playing == Some(index) {
                    match self.player.state() {
                        PlaybackState::Playing => MEDIA_PLAY,
                        PlaybackState::Paused => MEDIA_PAUSE,
                        PlaybackState::Stopped => "",
                    }
                    .to_owned()
                } else if let Some(position) = self.queue.iter().position(|queued| *queued == index)
                {
                    format!("{MEDIA_NEXT}{}", position + 1)
                } else {
                    String::new()
                }
            }
            "rating" | "playcount" => String::new(),
            "title" => {
                let queue_badge = self
                    .queue
                    .iter()
                    .position(|queued| *queued == index)
                    .map(|position| format!("{MEDIA_NEXT}{} ", position + 1))
                    .unwrap_or_default();
                let stop_badge = if self.stop_after_rows.contains(&index) {
                    "⏹️ "
                } else {
                    ""
                };
                format!(
                    "{}  {queue_badge}{stop_badge}{}",
                    if self.playing == Some(index) {
                        if self.player.state() == PlaybackState::Paused {
                            MEDIA_PAUSE
                        } else {
                            MEDIA_PLAY
                        }
                    } else {
                        glyph(&track.entry)
                    },
                    self.title_for(track)
                )
            }
            "albumartist" => meta.map_or("", |m| m.album_artist.as_str()).to_owned(),
            "artist" => meta.map_or("", |m| m.artist.as_str()).to_owned(),
            "composer" => meta.map_or("", |m| m.composer.as_str()).to_owned(),
            "album" => meta.map_or("", |m| m.album.as_str()).to_owned(),
            "length" => meta
                .and_then(|m| m.duration)
                .map(|d| format!("{}:{:02}", d.as_secs() / 60, d.as_secs() % 60))
                .unwrap_or_default(),
            "filesizebytes" => meta
                .and_then(|m| m.file_size_bytes)
                .map(|bytes| bytes.to_string())
                .unwrap_or_default(),
            "filesize" => meta
                .and_then(|m| m.file_size_bytes)
                .map(kog_audio::track::file_size_label)
                .unwrap_or_default(),
            "date" => meta
                .and_then(|m| m.year)
                .map(|v| v.to_string())
                .unwrap_or_default(),
            "genre" => meta.map_or("", |m| m.genre.as_str()).to_owned(),
            "track" => meta
                .and_then(|m| m.track_number)
                .map(|v| v.to_string())
                .unwrap_or_default(),
            "path" => display_entry_path(&track.entry),
            "filename" if track.entry.kind == "remote" => track.name.clone(),
            "filename" => Path::new(if track.entry.entry.is_empty() {
                &track.entry.path
            } else {
                &track.entry.entry
            })
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default(),
            "codec" => meta.map_or("", |m| m.codec.as_str()).to_owned(),
            "samplerate" => meta
                .and_then(|m| m.sample_rate)
                .map(|v| format!("{} Hz", v))
                .unwrap_or_default(),
            "bitspersample" => meta
                .and_then(|m| m.bits_per_sample)
                .map(|v| v.to_string())
                .unwrap_or_default(),
            "bitrate" => meta
                .and_then(|m| m.bitrate)
                .map(|v| format!("{} kbps", v))
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    fn show_queue(&mut self) {
        self.focus = Focus::Tracks;
    }

    fn selected_track_indices(&self) -> Vec<usize> {
        let mut indices: Vec<_> = self
            .selected_tracks
            .iter()
            .copied()
            .filter(|&index| index < self.tracks.len())
            .collect();
        if indices.is_empty() && self.selected[2] < self.tracks.len() {
            indices.push(self.selected[2]);
        }
        indices.sort_unstable();
        indices
    }

    fn toggle_selected_queue(&mut self) {
        for index in self.selected_track_indices() {
            if let Some(position) = self.queue.iter().position(|&queued| queued == index) {
                self.queue.remove(position);
            } else {
                self.queue.push_back(index);
            }
        }
        self.auto_fit_cache = None;
        self.status = format!("{} track(s) queued", self.queue.len());
    }

    fn toggle_selected_stop_after(&mut self) {
        for index in self.selected_track_indices() {
            if !self.stop_after_rows.remove(&index) {
                self.stop_after_rows.insert(index);
            }
        }
        self.auto_fit_cache = None;
        self.status = format!("{} stop-after marker(s)", self.stop_after_rows.len());
    }

    fn clear_queue(&mut self) {
        self.queue.clear();
        self.auto_fit_cache = None;
        self.status = "Queue cleared".to_owned();
    }

    fn prune_missing_selected_list(&mut self) {
        let id = self.lists.get(self.selected[0]).map_or(0, |(id, _)| *id);
        if id <= 0 {
            self.status = "Favorites are cleaned from their starred files".to_owned();
            return;
        }
        let rows = self.library.db().playlist_entry_rows(id);
        let result = rows.and_then(|rows| {
            let doomed: Vec<_> = rows
                .iter()
                .filter(|(_, entry)| {
                    matches!(entry.kind.as_str(), "local" | "archive")
                        && !Path::new(&entry.path).exists()
                })
                .map(|(row_id, _)| *row_id)
                .collect();
            self.library.db().delete_entry_rows(id, &doomed)
        });
        self.status = match result {
            Ok(0) => "No missing files in the playlist".to_owned(),
            Ok(count) => {
                self.reload_lists();
                format!(
                    "Removed {count} missing file{} from the playlist",
                    if count == 1 { "" } else { "s" }
                )
            }
            Err(error) => error,
        };
    }

    fn show_track_in_tree(&mut self) {
        let Some(track) = self.tracks.get(self.selected[2]).cloned() else {
            return;
        };
        if track.entry.kind == "remote" {
            if self.remote_active
                && let Some(index) = self.items.iter().position(|row| {
                    matches!(&row.item, Item::Track(item) if item.entry.path == track.entry.path)
                })
            {
                self.selected[1] = index;
                self.focus = Focus::Library;
                self.status = format!("Located {}", track.name);
            } else {
                self.status = "Browse the source server to locate this track".to_owned();
            }
            return;
        }
        let path = PathBuf::from(&track.entry.path);
        let Some(parent) = path.parent() else {
            return;
        };
        self.browse(Some(parent.to_path_buf()));
        if let Some(index) = self.items.iter().position(|row| match &row.item {
            Item::Track(item) => item.entry.path == track.entry.path,
            Item::Directory(_, location) => location == &path,
        }) {
            self.selected[1] = index;
            self.focus = Focus::Library;
            self.status = format!("Located {}", track.name);
        }
    }

    fn toggle_mute(&mut self) {
        if self.volume <= 0.0 {
            self.volume = self.volume_before_mute.max(0.05);
        } else {
            self.volume_before_mute = self.volume;
            self.volume = 0.0;
        }
        self.player.set_volume(self.volume);
    }

    fn set_volume_from_bar(&mut self, x: usize, size: (usize, usize)) {
        let (_, _, bar_x, bar_width) = volume_geometry(size.0, size.1.saturating_sub(4));
        self.volume = ((x.saturating_sub(bar_x)) as f32 / (bar_width - 1) as f32).clamp(0.0, 1.0);
        if self.volume > 0.0 {
            self.volume_before_mute = self.volume;
        }
        self.player.set_volume(self.volume);
    }

    fn layout(&self, size: (usize, usize)) -> Layout {
        self.layout_for_track_count(size, self.visible_track_count())
    }

    fn visible_track_count(&self) -> usize {
        if self.playlist_query.is_empty() {
            self.tracks.len()
        } else {
            self.visible_tracks().len()
        }
    }

    fn layout_for_track_count(&self, size: (usize, usize), track_count: usize) -> Layout {
        let mut layout = Layout::new(
            size.0,
            size.1,
            self.lists.len(),
            self.files_expanded,
            self.playlists_expanded,
        );
        if !self.sidebar_visible {
            layout.show_sidebar = false;
            layout.first = 0;
        } else if layout.show_sidebar {
            if let Some(width) = self.sidebar_width {
                layout.first = width.clamp(18, size.0.saturating_sub(30).max(18));
            }
        }
        let track_width = size.0.saturating_sub(layout.first + 1);
        let vertical = track_count > layout.track_page;
        if self.columns.total_width() > track_width.saturating_sub(usize::from(vertical))
            && track_width >= 5
        {
            layout.track_page = layout.track_page.saturating_sub(1).max(1);
        }
        layout
    }

    fn scrollbar(&self, layout: &Layout, size: (usize, usize)) -> Option<HorizontalScrollbar> {
        self.scrollbar_for_track_count(layout, size, self.visible_track_count())
    }

    fn scrollbar_for_track_count(
        &self,
        layout: &Layout,
        size: (usize, usize),
        track_count: usize,
    ) -> Option<HorizontalScrollbar> {
        (!self.compact_mode && (layout.show_sidebar || self.focus == Focus::Tracks))
            .then(|| {
                HorizontalScrollbar::new(
                    layout.first + 1,
                    size.0.saturating_sub(layout.first + 1 + usize::from(
                        self.track_scrollbar_for_count(layout, size, track_count)
                            .is_some(),
                    )),
                    self.columns.total_width(),
                    self.columns.scroll,
                )
            })
            .flatten()
    }

    fn tree_scrollbar(&self, layout: &Layout, size: (usize, usize)) -> Option<VerticalScrollbar> {
        if self.compact_mode || !self.files_expanded {
            return None;
        }
        let (top, height, x) = if layout.show_sidebar {
            if layout.tree_bottom < layout.tree_top {
                return None;
            }
            (
                layout.tree_top,
                layout.tree_bottom - layout.tree_top + 1,
                layout.first.saturating_sub(1),
            )
        } else if self.focus == Focus::Library {
            (2, layout.footer_top.saturating_sub(2), size.0.saturating_sub(1))
        } else {
            return None;
        };
        VerticalScrollbar::new(x, top, height, self.items.len(), self.offsets[1])
    }

    fn tree_page(&self, layout: &Layout) -> usize {
        if !layout.show_sidebar && self.focus == Focus::Library {
            layout.footer_top.saturating_sub(2)
        } else if layout.tree_bottom >= layout.tree_top {
            layout.tree_bottom - layout.tree_top + 1
        } else {
            0
        }
    }

    fn track_scrollbar(&self, layout: &Layout, size: (usize, usize)) -> Option<VerticalScrollbar> {
        self.track_scrollbar_for_count(layout, size, self.visible_track_count())
    }

    fn track_scrollbar_for_count(
        &self,
        layout: &Layout,
        size: (usize, usize),
        count: usize,
    ) -> Option<VerticalScrollbar> {
        if self.compact_mode || !(layout.show_sidebar || self.focus == Focus::Tracks) {
            return None;
        }
        VerticalScrollbar::new(
            size.0.saturating_sub(1),
            2,
            layout.track_page,
            count,
            self.offsets[2],
        )
    }

    fn vertical_scrollbar_at(
        &self,
        layout: &Layout,
        size: (usize, usize),
        x: usize,
        y: usize,
    ) -> Option<(usize, VerticalScrollbar)> {
        let hit = |bar: VerticalScrollbar| {
            x == bar.x && (bar.top..bar.top + bar.height).contains(&y)
        };
        if let Some(bar) = self.tree_scrollbar(layout, size).filter(|bar| hit(*bar)) {
            return Some((1, bar));
        }
        if x != size.0.saturating_sub(1) {
            return None;
        }
        self.track_scrollbar(layout, size)
            .filter(|bar| hit(*bar))
            .map(|bar| (2, bar))
    }

    fn scroll_vertical_to_mouse(
        &mut self,
        pane: usize,
        bar: VerticalScrollbar,
        y: usize,
        grip: usize,
    ) {
        self.offsets[pane] = bar.offset_at(y, grip);
        self.manual_scroll_selection[pane] = Some(self.selected[pane]);
    }

    fn scroll_columns_to_mouse(&mut self, bar: HorizontalScrollbar, x: usize, grip: usize) {
        let position = x.saturating_sub(bar.x + 1).min(bar.track_width - 1);
        self.columns.scroll = position
            .saturating_sub(grip)
            .min(bar.travel)
            .saturating_mul(bar.max_scroll)
            / bar.travel.max(1);
    }

    fn auto_fit_columns(&mut self) {
        self.auto_fit_cache = None;
        self.ensure_auto_fit_columns();
        self.status = "Columns fitted to playlist content".to_owned();
    }

    fn ensure_auto_fit_columns(&mut self) {
        let signature: Vec<_> = self
            .columns
            .entries
            .iter()
            .map(|column| (column.id, column.visible))
            .collect();
        if self
            .auto_fit_cache
            .as_ref()
            .is_some_and(|cache| cache.columns == signature)
        {
            return;
        }
        let mut cache = AutoFitCache {
            columns: signature,
            rows_by_key: HashMap::new(),
            row_widths: self
                .columns
                .entries
                .iter()
                .map(|column| {
                    if column.visible {
                        vec![0; self.tracks.len()]
                    } else {
                        Vec::new()
                    }
                })
                .collect(),
            width_counts: vec![BTreeMap::new(); self.columns.entries.len()],
        };
        for (row, track) in self.tracks.iter().enumerate() {
            cache
                .rows_by_key
                .entry(metadata_key(&track.entry))
                .or_default()
                .push(row);
            for (column_index, column) in self.columns.entries.iter().enumerate() {
                if !column.visible {
                    continue;
                }
                let width = cell_width(&self.column_value(row, track, column.id));
                cache.row_widths[column_index][row] = width;
                *cache.width_counts[column_index].entry(width).or_default() += 1;
            }
        }
        self.apply_auto_fit_widths(&cache);
        self.auto_fit_cache = Some(cache);
    }

    fn refresh_auto_fit_metadata(&mut self, keys: &[String]) {
        if keys.is_empty() {
            return;
        }
        let Some(mut cache) = self.auto_fit_cache.take() else {
            return;
        };
        let signature_matches = self
            .columns
            .entries
            .iter()
            .map(|column| (column.id, column.visible))
            .eq(cache.columns.iter().copied());
        if !signature_matches {
            return;
        }
        for key in keys {
            let Some(rows) = cache.rows_by_key.get(key).cloned() else {
                continue;
            };
            for row in rows {
                let Some(track) = self.tracks.get(row) else {
                    continue;
                };
                for (column_index, column) in self.columns.entries.iter().enumerate() {
                    if !column.visible {
                        continue;
                    }
                    let width = cell_width(&self.column_value(row, track, column.id));
                    let previous = &mut cache.row_widths[column_index][row];
                    if *previous == width {
                        continue;
                    }
                    let old_width = *previous;
                    let counts = &mut cache.width_counts[column_index];
                    if counts.get(&old_width) == Some(&1) {
                        counts.remove(&old_width);
                    } else if let Some(count) = counts.get_mut(&old_width) {
                        *count -= 1;
                    }
                    *counts.entry(width).or_default() += 1;
                    *previous = width;
                }
            }
        }
        self.apply_auto_fit_widths(&cache);
        self.auto_fit_cache = Some(cache);
    }

    fn apply_auto_fit_widths(&mut self, cache: &AutoFitCache) {
        for (column, counts) in self.columns.entries.iter_mut().zip(&cache.width_counts) {
            if column.visible {
                column.width = counts
                    .last_key_value()
                    .map(|(width, _)| *width)
                    .unwrap_or(0)
                    .max(cell_width(column.label))
                    .saturating_add(1)
                    .max(3);
            }
        }
    }

    fn persist_columns(&mut self) {
        if let Err(error) = self.columns.save() {
            self.status = format!("Saving column layout: {error}");
        }
    }

    fn activate_menu(&mut self, index: usize, size: (usize, usize)) {
        if self.menu_page.labels().get(index) == Some(&"") {
            return;
        }
        let page = self.menu_page;
        self.menu_open = false;
        match (page, index) {
            (MenuPage::Main, 0) => self.begin_prompt(PromptKind::AddFile, String::new()),
            (MenuPage::Main, 1) => self.begin_prompt(PromptKind::AddUrl, String::new()),
            (MenuPage::Main, 2) | (MenuPage::Preferences, 0) => self.begin_prompt(
                PromptKind::MusicFolder,
                self.library
                    .root()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
            (MenuPage::Main, 4) => self.begin_prompt(PromptKind::SavePlaylist, String::new()),
            (MenuPage::Main, 5) => self.begin_prompt(PromptKind::SaveSelection, String::new()),
            (MenuPage::Main, 6) => self.remove_selected(),
            (MenuPage::Main, 7) => self.clear_playlist(),
            (MenuPage::Main, 9) => self.open_child_menu(MenuPage::View, size),
            (MenuPage::Main, 10) => self.open_child_menu(MenuPage::Playback, size),
            (MenuPage::Main, 11) => self.open_child_menu(MenuPage::Preferences, size),
            (MenuPage::Main, 13) => self.open_child_menu(MenuPage::Remote, size),
            (MenuPage::Main, 14) => {
                self.info_modal = false;
                self.artwork_modal = false;
                self.modal = Some(format!(
                    "Kog v{}\nTerminal player\nMusic folder: {}",
                    env!("CARGO_PKG_VERSION"),
                    self.library
                        .root()
                        .map_or_else(|| "None".to_owned(), |p| p.display().to_string())
                ))
            }
            (MenuPage::Main, 15) => self.exit_requested = true,
            (MenuPage::View, 0) => {
                self.sidebar_visible = !self.sidebar_visible;
                if !self.sidebar_visible {
                    self.focus = Focus::Tracks;
                }
            }
            (MenuPage::View, 1) => self.show_info(),
            (MenuPage::View, 2) => self.show_lyrics(),
            (MenuPage::View, 3) => self.show_equalizer(),
            (MenuPage::View, 4) => self.show_visualizer(),
            (MenuPage::View, 5) => self.show_supported_formats(),
            (MenuPage::View, 6) => {
                self.compact_mode = !self.compact_mode;
                self.status = if self.compact_mode {
                    "Compact player · press C to return".to_owned()
                } else {
                    "Playlist view".to_owned()
                };
            }
            (MenuPage::View, 7) => self.show_artwork(),
            (MenuPage::Playback, 0) => self.play_pause(),
            (MenuPage::Playback, 1) => {
                self.player.stop();
                self.playing = None;
            }
            (MenuPage::Playback, 2) => self.previous(),
            (MenuPage::Playback, 3) => self.next(false),
            (MenuPage::Playback, 4) => self.cycle_shuffle(),
            (MenuPage::Playback, 5) => self.cycle_repeat(),
            (MenuPage::Playback, 6) => {
                self.stop_after_current = !self.stop_after_current;
                self.status = format!(
                    "Stop after current: {}",
                    if self.stop_after_current { "on" } else { "off" }
                );
            }
            (MenuPage::Playback, 7) => self.toggle_mute(),
            (MenuPage::Playback, 8) => self.toggle_radio(),
            (MenuPage::Playback, 9) => self.reshuffle_radio(),
            (MenuPage::Playback, 10) => self.toggle_selected_queue(),
            (MenuPage::Playback, 11) => self.toggle_selected_stop_after(),
            (MenuPage::Playback, 12) => self.clear_queue(),
            (MenuPage::Playback, 13) => self.begin_seek_prompt(),
            (MenuPage::Preferences, 1) => self.begin_prompt(
                PromptKind::Volume,
                format!("{}", (self.volume * 100.0).round() as u8),
            ),
            (MenuPage::Preferences, 2) => self.cycle_repeat(),
            (MenuPage::Preferences, 3) => self.begin_prompt(
                PromptKind::EqualizerPreset,
                self.equalizer.preset_name.clone(),
            ),
            (MenuPage::Preferences, 4) => {
                self.equalizer.enabled = !self.equalizer.enabled;
                self.apply_equalizer();
            }
            (MenuPage::Preferences, 5) => {
                self.begin_prompt(PromptKind::Preamp, format!("{}", self.equalizer.preamp_db))
            }
            (MenuPage::Preferences, 6) => {
                let current = AppSettings::load()
                    .output_device
                    .map(|device| device.name)
                    .unwrap_or_else(|| "default".to_owned());
                self.begin_prompt(PromptKind::OutputDevice, current)
            }
            (MenuPage::Preferences, 7) => self.show_output_devices(),
            (MenuPage::Preferences, 8) => {
                self.begin_prompt(PromptKind::EqualizerBandNumber, String::new())
            }
            (MenuPage::Preferences, 9) => self.show_blacklist(),
            (MenuPage::Preferences, 10) => {
                self.begin_prompt(PromptKind::RemoveBlacklistId, String::new())
            }
            (MenuPage::Preferences, 11) => {
                let next = match AppSettings::load().opening_files_behavior {
                    OpeningFilesBehavior::ClearAndPlay => OpeningFilesBehavior::Enqueue,
                    OpeningFilesBehavior::Enqueue => OpeningFilesBehavior::EnqueueAndPlay,
                    OpeningFilesBehavior::EnqueueAndPlay => OpeningFilesBehavior::ClearAndPlay,
                };
                match AppSettings::save_opening_files_behavior(next) {
                    Ok(()) => self.status = format!("Opening files: {}", next.setting_value()),
                    Err(error) => self.status = error,
                }
            }
            (MenuPage::Preferences, 12) => self.open_child_menu(MenuPage::Synthesis, size),
            (MenuPage::Preferences, 13) => {
                let enabled = !AppSettings::load().read_cue_sheets_in_folders;
                match AppSettings::save_read_cue_sheets_in_folders(enabled) {
                    Ok(()) => {
                        self.status = format!(
                            "Read CUE sheets in folders: {}",
                            if enabled { "on" } else { "off" }
                        )
                    }
                    Err(error) => self.status = error,
                }
            }
            (MenuPage::Preferences, 14) => {
                let enabled = !AppSettings::load().read_playlists_in_folders;
                match AppSettings::save_read_playlists_in_folders(enabled) {
                    Ok(()) => {
                        self.library.set_read_playlists_in_folders(enabled);
                        self.status = format!(
                            "Read M3U/PLS in folders: {}",
                            if enabled { "on" } else { "off" }
                        );
                    }
                    Err(error) => self.status = error,
                }
            }
            (MenuPage::Preferences, 15) => self.open_child_menu(MenuPage::Server, size),
            (MenuPage::Preferences, 16) => {
                let enabled = !self.download_cover_art;
                match AppSettings::save_download_cover_art(enabled) {
                    Ok(()) => {
                        self.download_cover_art = enabled;
                        self.cover_key.clear();
                        self.status = format!(
                            "Automatic cover downloads: {}",
                            if enabled { "on" } else { "off" }
                        );
                    }
                    Err(error) => self.status = error,
                }
            }
            (MenuPage::Synthesis, 0) => {
                let next = match self.decoder_settings.midi_engine() {
                    MidiEngine::RustySynth => MidiEngine::Opl3Windows,
                    MidiEngine::Opl3Windows => MidiEngine::Sc55,
                    MidiEngine::Sc55 => MidiEngine::Mt32,
                    MidiEngine::Mt32 => MidiEngine::RustySynth,
                };
                match AppSettings::save_midi_engine(next) {
                    Ok(()) => {
                        self.decoder_settings.set_midi_engine(next);
                        self.invalidate_metadata();
                        self.status = format!("MIDI backend: {}", next.setting_value());
                    }
                    Err(error) => self.status = error,
                }
            }
            (MenuPage::Synthesis, 1) => self.begin_prompt(
                PromptKind::SoundFont,
                self.decoder_settings
                    .soundfont_path()
                    .map_or_else(String::new, |path| path.display().to_string()),
            ),
            (MenuPage::Synthesis, 2) => self.begin_prompt(
                PromptKind::Sc55Roms,
                self.decoder_settings
                    .sc55_rom_path()
                    .map_or_else(String::new, |path| path.display().to_string()),
            ),
            (MenuPage::Synthesis, 3) => self.begin_prompt(
                PromptKind::Mt32Roms,
                self.decoder_settings
                    .mt32_rom_path()
                    .map_or_else(String::new, |path| path.display().to_string()),
            ),
            (MenuPage::Synthesis, 4) => {
                let enabled = !self.decoder_settings.mt32_gm_program_mapping();
                match AppSettings::save_mt32_gm_program_mapping(enabled) {
                    Ok(()) => {
                        self.decoder_settings.set_mt32_gm_program_mapping(enabled);
                        self.invalidate_metadata();
                        self.status = format!(
                            "MT-32 GM program mapping: {}",
                            if enabled { "on" } else { "off" }
                        );
                    }
                    Err(error) => self.status = error,
                }
            }
            (MenuPage::Synthesis, 5) => {
                self.show_modal(format!(
                    "MIDI backend: {}\nSoundFont: {}\nSC-55 ROMs: {}\nMT-32 ROMs: {}\nMT-32 GM program mapping: {}",
                    self.decoder_settings.midi_engine().setting_value(),
                    self.decoder_settings.soundfont_path().map_or_else(|| "None".to_owned(), |path| path.display().to_string()),
                    self.decoder_settings.sc55_rom_path().map_or_else(|| "None".to_owned(), |path| path.display().to_string()),
                    self.decoder_settings.mt32_rom_path().map_or_else(|| "None".to_owned(), |path| path.display().to_string()),
                    if self.decoder_settings.mt32_gm_program_mapping() { "on" } else { "off" }
                ));
            }
            (MenuPage::Synthesis, 6) => self.begin_prompt(PromptKind::Sc55Archive, String::new()),
            (MenuPage::Synthesis, 7) => self.begin_prompt(PromptKind::Mt32Archive, String::new()),
            (MenuPage::Tree, 0) => self.add_selected(false),
            (MenuPage::Tree, 1) => self.add_selected(true),
            (MenuPage::Tree, 2) => self.open_selected(),
            (MenuPage::Tree, 3) => self.toggle_star(),
            (MenuPage::Tree, 4) => self.begin_prompt(PromptKind::AddToPlaylist, String::new()),
            (MenuPage::Tree, 5) => self.blacklist_selected_tree(false),
            (MenuPage::Tree, 6) => self.blacklist_selected_tree(true),
            (MenuPage::Tree, 7) => self.begin_trash_selected(),
            (MenuPage::Tree, 8) => {
                if self.selected_tree.len() > 1 {
                    self.status = "Select one folder to use as tree root".to_owned();
                } else if let Some(TreeRow {
                    item: Item::Directory(_, path),
                    ..
                }) = self.items.get(self.selected[1])
                {
                    if self.remote_active {
                        self.connect_remote(Some(path.to_string_lossy().into_owned()));
                    } else {
                        self.browse(Some(path.clone()));
                    }
                } else {
                    self.status = "Select a folder to use as tree root".to_owned();
                }
            }
            (MenuPage::Tree, 9) => {
                if self.remote_active {
                    self.connect_remote(None);
                } else {
                    self.browse(None);
                }
            }
            (MenuPage::Tracks, 0) => self.play_selected(),
            (MenuPage::Tracks, 1) => self.remove_selected(),
            (MenuPage::Tracks, 2) => self.begin_prompt(PromptKind::AddToPlaylist, String::new()),
            (MenuPage::Tracks, 3) => self.toggle_star(),
            (MenuPage::Tracks, 4) => self.show_info(),
            (MenuPage::Tracks, 5) => self.clear_playlist(),
            (MenuPage::Tracks, 6) => self.show_track_in_tree(),
            (MenuPage::Tracks, 7) => self.begin_prompt(PromptKind::SaveSelection, String::new()),
            (MenuPage::Tracks, 8) => {
                self.selected_tracks = self.visible_tracks().into_iter().collect();
                self.status = format!("Selected {} tracks", self.selected_tracks.len());
            }
            (MenuPage::Tracks, 9) => self.toggle_selected_queue(),
            (MenuPage::Tracks, 10) => self.toggle_selected_stop_after(),
            (MenuPage::Tracks, 11) => self.blacklist_selected_tracks(false),
            (MenuPage::Tracks, 12) => self.blacklist_selected_tracks(true),
            (MenuPage::Tracks, 13) => self.open_tag_editor(),
            (MenuPage::Saved, 0) => self.enqueue_selected_lists(false),
            (MenuPage::Saved, 1) => self.enqueue_selected_lists(true),
            (MenuPage::Saved, 2) => {
                let index = self.selected[0];
                self.clear_playlist();
                self.enqueue_list(index);
                if !self.tracks.is_empty() {
                    self.select_track(0, false, false);
                    self.play_selected();
                }
            }
            (MenuPage::Saved, 3) => {
                if let Some((id, name)) = self.lists.get(self.selected[0]) {
                    if *id > 0 {
                        self.begin_prompt(PromptKind::RenamePlaylist, name.clone());
                    }
                }
            }
            (MenuPage::Saved, 4) => {
                if let Some((id, name)) = self.lists.get(self.selected[0]) {
                    if *id > 0 {
                        self.begin_prompt(PromptKind::DuplicatePlaylist, format!("{name} copy"));
                    }
                }
            }
            (MenuPage::Saved, 5) => {
                if self
                    .selected_list_indices()
                    .iter()
                    .any(|&index| self.lists[index].0 > 0)
                {
                    self.begin_prompt(PromptKind::DeletePlaylist, String::new());
                }
            }
            (MenuPage::Saved, 6) => {
                if let Some((_, name)) = self.lists.get(self.selected[0]) {
                    let path = self
                        .library
                        .root()
                        .unwrap_or_else(|| PathBuf::from("."))
                        .join(format!("{name}.m3u"));
                    self.begin_prompt(PromptKind::ExportPlaylist, path.display().to_string());
                }
            }
            (MenuPage::Saved, 7) => self.prune_missing_selected_list(),
            (MenuPage::Playlist, 0) => self.begin_prompt(PromptKind::AddFile, String::new()),
            (MenuPage::Playlist, 1) => self.begin_prompt(PromptKind::AddUrl, String::new()),
            (MenuPage::Playlist, 2) => self.begin_prompt(PromptKind::SavePlaylist, String::new()),
            (MenuPage::Playlist, 3) => {
                self.selected_tracks = self.visible_tracks().into_iter().collect();
                self.status = format!("Selected {} tracks", self.selected_tracks.len());
            }
            (MenuPage::Playlist, 4) => self.clear_playlist(),
            (MenuPage::Playlist, 5) => self.open_child_menu(MenuPage::Columns, size),
            (MenuPage::Columns, 0..=2) => self.sort_tracks(index),
            (MenuPage::Columns, 3 | 4) => {
                let id = if index == 3 { "artist" } else { "album" };
                if let Some(column) = self.columns.index(id) {
                    self.toggle_column(column);
                }
            }
            (MenuPage::Columns, 5) => self.auto_fit_columns(),
            (MenuPage::Columns, 6) => self.open_child_menu(MenuPage::ColumnVisibility, size),
            (MenuPage::Columns, 7 | 8) => {
                if let Some(column) = self.context_column {
                    let delta = if index == 7 { -1 } else { 1 };
                    if let Some(target) = self.columns.move_by(column, delta) {
                        self.context_column = Some(target);
                        self.persist_columns();
                        self.status = "Column moved".to_owned();
                    }
                }
            }
            (MenuPage::Columns, 9 | 10) => {
                self.columns.scroll_by(
                    if index == 9 { -12 } else { 12 },
                    self.column_viewport_width,
                );
            }
            (MenuPage::Columns, 11) => {
                self.columns = Columns::default();
                self.auto_fit_cache = None;
                self.persist_columns();
                self.status = "Column layout reset".to_owned();
            }
            (MenuPage::ColumnVisibility, index) => {
                if let Some(id) = Columns::default()
                    .entries
                    .get(index)
                    .map(|column| column.id)
                    && let Some(column) = self.columns.index(id)
                {
                    self.toggle_column(column);
                }
            }
            (MenuPage::TagEditor, 0..=12) => {
                if let Some(session) = &self.tag_session {
                    let id = TAG_FIELDS[index].0;
                    let value = session
                        .fields
                        .get(id)
                        .and_then(|value| value.as_str())
                        .or_else(|| session.snapshot["fields"][id]["value"].as_str())
                        .unwrap_or_default()
                        .to_owned();
                    self.begin_prompt(PromptKind::TagField(index), value);
                }
            }
            (MenuPage::TagEditor, 13) => self.begin_prompt(PromptKind::TagArtwork, String::new()),
            (MenuPage::TagEditor, 14) => {
                if let Some(session) = self.tag_session.as_mut() {
                    session.artwork = Some(serde_json::json!({"action":"remove"}));
                    self.status = "Artwork removal staged".to_owned();
                    self.open_submenu(MenuPage::TagEditor);
                }
            }
            (MenuPage::TagEditor, 15) => self.save_tag_edits(),
            (MenuPage::TagEditor, 16) => {
                self.tag_session = None;
                self.status = "Tag edits cancelled".to_owned();
            }
            (MenuPage::Remote, 0) => self.begin_prompt(
                PromptKind::RemoteUrl,
                self.remote_settings.server_url.clone(),
            ),
            (MenuPage::Remote, 1) => self.begin_prompt(PromptKind::RemoteToken, String::new()),
            (MenuPage::Remote, 2) => self.begin_prompt(
                PromptKind::RemoteUsername,
                self.remote_settings.username.clone(),
            ),
            (MenuPage::Remote, 3) => self.begin_prompt(PromptKind::RemotePassword, String::new()),
            (MenuPage::Remote, 4) => {
                self.remote_settings.auth_mode = match self.remote_settings.auth_mode.as_str() {
                    "token" => "basic",
                    "basic" => "none",
                    _ => "token",
                }
                .to_owned();
                self.persist_remote_settings(false);
                self.status = format!("Server authentication: {}", self.remote_settings.auth_mode);
            }
            (MenuPage::Remote, 5) => {
                self.remote_settings.codec = match self.remote_settings.codec.as_str() {
                    "aac" => "opus",
                    "opus" => "flac",
                    _ => "aac",
                }
                .to_owned();
                self.persist_remote_settings(false);
                self.status = format!("Remote stream: {}", self.remote_settings.codec);
            }
            (MenuPage::Remote, 6) => self.connect_remote(None),
            (MenuPage::Remote, 7) => {
                if self.remote_active {
                    self.enqueue_folder(PathBuf::from(&self.remote_path));
                } else {
                    self.status = "Connect to a server first".to_owned();
                }
            }
            (MenuPage::Remote, 8) => {
                self.browse(None);
                self.status = "Showing local library".to_owned();
            }
            (MenuPage::Server, 0) => self.start_api_server(),
            (MenuPage::Server, 1) => self.stop_api_server(),
            (MenuPage::Server, 2) => self.show_api_server_status(),
            (MenuPage::Server, 3) => self.begin_prompt(
                PromptKind::ServerAddress,
                kog_server::config::load_config().address.to_string(),
            ),
            (MenuPage::Server, 4) => self.begin_prompt(
                PromptKind::ServerPort,
                kog_server::config::load_config().port.to_string(),
            ),
            (MenuPage::Server, 5) => self.update_server_config(|config| {
                config.auth = match config.auth {
                    AuthMode::None => AuthMode::Token,
                    AuthMode::Token => AuthMode::Basic,
                    AuthMode::Basic => AuthMode::None,
                };
                config.ensure_credentials();
                Ok(())
            }),
            (MenuPage::Server, 6) => self.begin_prompt(PromptKind::ServerToken, String::new()),
            (MenuPage::Server, 7) => self.update_server_config(|config| {
                config.token = kog_server::auth::generate_token()?;
                Ok(())
            }),
            (MenuPage::Server, 8) => self.begin_prompt(
                PromptKind::ServerUsername,
                kog_server::config::load_config().credentials.username,
            ),
            (MenuPage::Server, 9) => self.begin_prompt(PromptKind::ServerPassword, String::new()),
            (MenuPage::Server, 10) => self.update_server_config(|config| {
                config.tls.mode = match config.tls.mode {
                    TlsMode::Off => TlsMode::SelfSigned,
                    TlsMode::SelfSigned => TlsMode::Pem,
                    TlsMode::Pem => TlsMode::Off,
                };
                Ok(())
            }),
            (MenuPage::Server, 11) => {
                self.begin_prompt(PromptKind::ServerCertificate, String::new())
            }
            (MenuPage::Server, 12) => self.update_server_config(|config| {
                config.default_codec = match config.default_codec {
                    StreamCodec::Aac => StreamCodec::Opus,
                    StreamCodec::Opus => StreamCodec::Flac,
                    StreamCodec::Flac => StreamCodec::Aac,
                };
                Ok(())
            }),
            (MenuPage::Server, 13) => self.begin_prompt(
                PromptKind::ServerCache,
                (kog_server::config::load_config().cache_bytes / (1024 * 1024)).to_string(),
            ),
            (MenuPage::Server, 14) => {
                let devices = kog_server::devices::registry().list();
                let mut lines = vec![format!("Connected devices: {}", devices.len())];
                for device in devices {
                    lines.push(format!(
                        "{} · {} · {} · {} request(s){}",
                        device.id,
                        device.agent,
                        device.addr,
                        device.requests,
                        if device.blocked { " · blocked" } else { "" }
                    ));
                }
                self.show_modal(lines.join("\n"));
            }
            (MenuPage::Server, 15) => self.begin_prompt(PromptKind::ServerDevice, String::new()),
            (MenuPage::Server, 16) => {
                let config = kog_server::config::load_config();
                let problems = config.problems();
                self.show_modal(if problems.is_empty() {
                    "Server configuration is valid".to_owned()
                } else {
                    problems.join("\n")
                });
            }
            _ => {}
        }
        if !self.menu_open {
            self.menu_parents.clear();
        }
    }

    fn persist_remote_settings(&mut self, reconnect: bool) {
        match self.remote_settings.save() {
            Ok(()) if reconnect && !self.remote_settings.server_url.is_empty() => {
                self.connect_remote(None);
            }
            Ok(()) => self.status = "Remote settings saved".to_owned(),
            Err(error) => self.status = error,
        }
    }

    fn update_server_config(
        &mut self,
        change: impl FnOnce(&mut kog_server::config::ServerConfig) -> Result<(), String>,
    ) {
        let mut config = kog_server::config::load_config();
        if let Err(error) = change(&mut config) {
            self.status = error;
            return;
        }
        match kog_server::config::save_config(&config) {
            Ok(()) => {
                self.status = if self.api_server.is_some() {
                    "Server settings saved; restart to apply".to_owned()
                } else {
                    "Server settings saved".to_owned()
                }
            }
            Err(error) => self.status = error,
        }
    }

    fn start_api_server(&mut self) {
        if let Some(server) = &self.api_server {
            self.status = format!("Server already running at {}", server.url);
            return;
        }
        let mut config = kog_server::config::load_config();
        config.enabled = true;
        config.ensure_credentials();
        if let Err(error) = config.validate() {
            self.status = error;
            return;
        }
        match RunningServer::start(config.clone()) {
            Ok(server) => {
                let url = server.url.clone();
                self.api_server = Some(server);
                match kog_server::config::save_config(&config) {
                    Ok(()) => self.status = format!("Server running at {url}"),
                    Err(error) => self.status = format!("Server running at {url}; {error}"),
                }
            }
            Err(error) => self.status = error,
        }
    }

    fn stop_api_server(&mut self) {
        self.api_server = None;
        self.update_server_config(|config| {
            config.enabled = false;
            Ok(())
        });
        if self.status == "Server settings saved" {
            self.status = "Server stopped".to_owned();
        }
    }

    fn show_api_server_status(&mut self) {
        let config = kog_server::config::load_config();
        self.show_modal(format!(
            "API server: {}\nAddress: {}:{}\nAuthentication: {:?}\nHTTPS: {:?}\nCodec: {}\nCache: {} MB\nMusic folder: {}",
            self.api_server.as_ref().map_or("stopped".to_owned(), |server| format!("running at {}", server.url)),
            config.address,
            config.port,
            config.auth,
            config.tls.mode,
            config.default_codec.setting_value(),
            config.cache_bytes / (1024 * 1024),
            self.library.root().map_or_else(|| "None".to_owned(), |path| path.display().to_string()),
        ));
    }

    fn poll_api_server(&mut self) {
        if let Some(result) = self.api_server.as_ref().and_then(RunningServer::finished) {
            self.api_server = None;
            self.status = match result {
                Ok(()) => "API server stopped".to_owned(),
                Err(error) => format!("API server stopped: {error}"),
            };
        }
    }

    fn toggle_column(&mut self, index: usize) {
        if self.columns.toggle(index) {
            let column = &self.columns.entries[index];
            self.status = format!(
                "{} {}",
                column.label,
                if column.visible { "shown" } else { "hidden" }
            );
            self.persist_columns();
        }
    }

    fn open_submenu(&mut self, page: MenuPage) {
        self.range_click_pending = None;
        self.menu_parents.clear();
        self.menu_page = page;
        self.menu_open = true;
        self.menu_selected = 0;
        self.menu_offset = 0;
        self.menu_x = 4;
        self.menu_y = 1;
    }

    fn active_menu_layer(&self) -> MenuLayer {
        MenuLayer {
            page: self.menu_page,
            selected: self.menu_selected,
            offset: self.menu_offset,
            x: self.menu_x,
            y: self.menu_y,
        }
    }

    fn restore_menu_layer(&mut self, layer: MenuLayer) {
        self.menu_page = layer.page;
        self.menu_selected = layer.selected;
        self.menu_offset = layer.offset;
        self.menu_x = layer.x;
        self.menu_y = layer.y;
        self.menu_open = true;
    }

    fn open_child_menu(&mut self, page: MenuPage, size: (usize, usize)) {
        let mut parent = self.active_menu_layer();
        let width = menu_width(size.0);
        let parent_width = width.min(size.0.saturating_sub(parent.x));
        let right_x = parent.x + parent_width.saturating_sub(1);
        let child_x = if right_x + width <= size.0 {
            right_x
        } else if parent.x >= width.saturating_sub(1) {
            parent.x - width.saturating_sub(1)
        } else if size.0 >= width.saturating_mul(2).saturating_sub(1) {
            if parent.x + parent_width / 2 < size.0 / 2 {
                parent.x = 0;
                width.saturating_sub(1)
            } else {
                parent.x = size.0 - width;
                parent.x - width.saturating_sub(1)
            }
        } else {
            0
        };
        let selected_row = parent.y + 1 + parent.selected.saturating_sub(parent.offset);
        let page_height = page.labels().len().min(size.1.saturating_sub(4));
        self.menu_parents.push(parent);
        self.menu_page = page;
        self.menu_selected = 0;
        self.menu_offset = 0;
        self.menu_x = child_x;
        self.menu_y = selected_row
            .min(size.1.saturating_sub(page_height + 3))
            .max(1);
        self.menu_open = true;
    }

    fn reflow_menus(&mut self, size: (usize, usize)) {
        if self.menu_size == size {
            return;
        }
        self.menu_size = size;
        if !self.menu_open {
            return;
        }
        let mut layers = self.menu_parents.clone();
        layers.push(self.active_menu_layer());
        let mut root = layers[0];
        let fit_selection = |layer: &mut MenuLayer| {
            let page = layer
                .page
                .labels()
                .len()
                .min(size.1.saturating_sub(4))
                .max(1);
            layer.offset = layer
                .offset
                .min(layer.page.labels().len().saturating_sub(page));
            if layer.selected < layer.offset {
                layer.offset = layer.selected;
            } else if layer.selected >= layer.offset + page {
                layer.offset = layer.selected + 1 - page;
            }
        };
        fit_selection(&mut root);
        let root_page = root.page.labels().len().min(size.1.saturating_sub(4));
        let max_x = size.0.saturating_sub(menu_width(size.0));
        let max_y = size.1.saturating_sub(root_page + 3);
        root.x = if root.page == MenuPage::Main {
            4.min(max_x)
        } else {
            root.x.min(max_x)
        };
        root.y = if root.page == MenuPage::Main {
            1.min(max_y)
        } else {
            root.y.min(max_y)
        };
        self.menu_parents.clear();
        self.restore_menu_layer(root);
        for mut layer in layers.into_iter().skip(1) {
            self.open_child_menu(layer.page, size);
            fit_selection(&mut layer);
            self.menu_selected = layer.selected;
            self.menu_offset = layer.offset;
        }
    }

    fn open_context(&mut self, page: MenuPage, x: usize, y: usize, size: (usize, usize)) {
        self.open_submenu(page);
        self.menu_x = x.min(size.0.saturating_sub(34)).max(1);
        self.menu_y = y.min(
            size.1
                .saturating_sub(page.labels().len().min(size.1.saturating_sub(4)) + 3),
        );
    }

    fn sort_tracks(&mut self, column: usize) {
        let id = ["title", "artist", "album"]
            .get(column)
            .copied()
            .unwrap_or("title");
        if let Some(index) = self.columns.index(id) {
            self.sort_tracks_by_column(index);
        }
    }

    fn sort_tracks_by_column(&mut self, column: usize) {
        self.sort_ascending = if self.sort_column == Some(column) {
            !self.sort_ascending
        } else {
            true
        };
        self.sort_column = Some(column);
        let mut order: Vec<_> = (0..self.tracks.len()).collect();
        let id = self.columns.entries[column].id;
        order.sort_by_key(|&index| {
            if id == "index" {
                format!("{index:012}")
            } else if id == "filesizebytes" || id == "filesize" {
                format!(
                    "{:020}",
                    self.metadata_for(&self.tracks[index])
                        .and_then(|meta| meta.file_size_bytes)
                        .unwrap_or_default()
                )
            } else if id == "title" {
                self.title_for(&self.tracks[index]).to_lowercase()
            } else {
                self.column_value(index, &self.tracks[index], id)
                    .to_lowercase()
            }
        });
        if !self.sort_ascending {
            order.reverse();
        }
        let mut mapping = vec![0; order.len()];
        for (new, &old) in order.iter().enumerate() {
            mapping[old] = new;
        }
        let mut tracks: Vec<_> = std::mem::take(&mut self.tracks)
            .into_iter()
            .map(Some)
            .collect();
        self.tracks = order
            .into_iter()
            .map(|old| tracks[old].take().unwrap())
            .collect();
        self.playing = self.playing.and_then(|old| mapping.get(old).copied());
        self.queue = self
            .queue
            .iter()
            .filter_map(|old| mapping.get(*old).copied())
            .collect();
        self.stop_after_rows = self
            .stop_after_rows
            .iter()
            .filter_map(|old| mapping.get(*old).copied())
            .collect();
        self.selected_tracks = self
            .selected_tracks
            .iter()
            .filter_map(|old| mapping.get(*old).copied())
            .collect();
        self.selected[2] = mapping.get(self.selected[2]).copied().unwrap_or(0);
        self.selection_anchor = self
            .selection_anchor
            .and_then(|old| mapping.get(old).copied());
        self.order_tracks_changed();
    }

    fn move_menu_selection(&mut self, delta: isize, height: usize) {
        let labels = self.menu_page.labels();
        let mut next = self.menu_selected as isize;
        loop {
            next += delta;
            if next < 0 || next >= labels.len() as isize {
                break;
            }
            if !labels[next as usize].is_empty() {
                self.menu_selected = next as usize;
                break;
            }
        }
        let page = labels.len().min(height.saturating_sub(4)).max(1);
        if self.menu_selected < self.menu_offset {
            self.menu_offset = self.menu_selected;
        }
        if self.menu_selected >= self.menu_offset + page {
            self.menu_offset = self.menu_selected + 1 - page;
        }
    }

    fn activate_menu_shortcut(&mut self, key: char, size: (usize, usize)) -> bool {
        let Some(index) = menu_shortcut_index(self.menu_page, key) else {
            return false;
        };
        self.menu_selected = index;
        let page = self
            .menu_page
            .labels()
            .len()
            .min(size.1.saturating_sub(4))
            .max(1);
        if index < self.menu_offset {
            self.menu_offset = index;
        } else if index >= self.menu_offset + page {
            self.menu_offset = index + 1 - page;
        }
        self.activate_menu(index, size);
        true
    }

    fn clear_playlist(&mut self) {
        self.folder_generation = self.folder_generation.wrapping_add(1);
        self.folder_play_pending.clear();
        self.player.stop();
        self.playing = None;
        self.tracks.clear();
        self.auto_fit_cache = None;
        self.order.clear_tracks();
        self.queue.clear();
        self.stop_after_rows.clear();
        self.selected_tracks.clear();
        self.selection_anchor = None;
        self.selected[2] = 0;
        self.offsets[2] = 0;
        self.session_dirty = true;
        self.status = "Playlist cleared".to_owned();
    }

    fn show_info(&mut self) {
        let content = self.info_content();
        self.show_modal(content);
        self.info_modal = true;
    }

    fn show_artwork(&mut self) {
        self.show_modal("Album cover".to_owned());
        self.artwork_modal = true;
    }

    fn info_content(&self) -> String {
        let track = self
            .playing
            .and_then(|index| self.tracks.get(index))
            .or_else(|| self.tracks.get(self.selected[2]));
        if let Some(track) = track {
            let metadata = self.metadata_for(track);
            let length = metadata
                .and_then(|m| m.duration)
                .map(|duration| {
                    format!("{}:{:02}", duration.as_secs() / 60, duration.as_secs() % 60)
                })
                .unwrap_or_default();
            let position = self.player.position();
            let mut content = format!(
                "{}\n\nArtist: {}\nAlbum: {}\nTitle: {}\nAlbum Artist: {}\nComposer: {}\nTrack: {}\nDisc: {}\nLength: {}\nPosition: {}:{:02}\nDate: {}\nGenre: {}\nFilename: {}\nFormat: {}\nSample Rate: {}\nChannels: {}\nBitrate: {}\nBits Per Sample: {}\n\n♫\n{}",
                self.title_for(track),
                metadata.map_or("", |m| &m.artist),
                metadata.map_or("", |m| &m.album),
                self.title_for(track),
                metadata.map_or("", |m| &m.album_artist),
                metadata.map_or("", |m| &m.composer),
                metadata
                    .and_then(|m| m.track_number)
                    .map_or(String::new(), |n| n.to_string()),
                metadata
                    .and_then(|m| m.disc_number)
                    .map_or(String::new(), |n| n.to_string()),
                length,
                position.as_secs() / 60,
                position.as_secs() % 60,
                metadata
                    .and_then(|m| m.year)
                    .map_or(String::new(), |n| n.to_string()),
                metadata.map_or("", |m| &m.genre),
                track.name,
                metadata.map_or("", |m| &m.codec),
                metadata
                    .and_then(|m| m.sample_rate)
                    .map_or(String::new(), |n| format!("{n} Hz")),
                metadata
                    .and_then(|m| m.channels)
                    .map_or(String::new(), |n| n.to_string()),
                metadata
                    .and_then(|m| m.bitrate)
                    .map_or(String::new(), |n| format!("{n} kbps")),
                metadata
                    .and_then(|m| m.bits_per_sample)
                    .map_or(String::new(), |n| n.to_string()),
                display_entry_path(&track.entry)
            );
            if let Some(cover) = &self.cover_preview {
                content.push_str(&format!("\nArtwork: {}", cover.path.display()));
            }
            content
        } else {
            "No track selected".to_owned()
        }
    }

    fn show_modal(&mut self, content: String) {
        self.modal = Some(content);
        self.info_modal = false;
        self.artwork_modal = false;
        self.visualizer_open = false;
        self.modal_scroll = 0;
    }

    fn show_visualizer(&mut self) {
        self.info_modal = false;
        self.artwork_modal = false;
        self.visualizer_open = true;
        self.modal_scroll = 0;
    }

    fn visualizer_text(&self) -> String {
        let frame: serde_json::Value = serde_json::from_str(&self.player.visualizer_frame())
            .unwrap_or(serde_json::Value::Null);
        let bands: Vec<_> = frame["spectrum"]
            .as_array()
            .into_iter()
            .flatten()
            .take(40)
            .map(|value| value.as_f64().unwrap_or(0.0).clamp(0.0, 1.0))
            .collect();
        let mut rows = vec!["Visualizer · Spectrum".to_owned()];
        for height in (1..=8).rev() {
            rows.push(
                bands
                    .iter()
                    .map(|level| {
                        if level * 8.0 >= f64::from(height) {
                            '█'
                        } else {
                            ' '
                        }
                    })
                    .collect(),
            );
        }
        rows.push("▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁".to_owned());
        rows.push("Esc closes".to_owned());
        rows.join("\n")
    }

    fn show_lyrics(&mut self) {
        let content = self
            .playing
            .and_then(|index| self.tracks.get(index))
            .or_else(|| self.tracks.get(self.selected[2]))
            .and_then(|track| self.metadata_for(track))
            .map(|metadata| {
                if metadata.lyrics.is_empty() {
                    "No embedded lyrics".to_owned()
                } else {
                    metadata.lyrics.clone()
                }
            })
            .unwrap_or_else(|| "No lyrics for the selected track".to_owned());
        self.show_modal(content);
    }

    fn show_equalizer(&mut self) {
        let mut lines = vec![format!(
            "Equalizer: {} · {} · preamp {:+.1} dB",
            if self.equalizer.enabled { "On" } else { "Off" },
            self.equalizer.preset_name,
            self.equalizer.preamp_db
        )];
        for (frequency, gain) in kog_core::equalizer::EQUALIZER_FREQUENCIES
            .iter()
            .zip(self.equalizer.gains_db)
        {
            lines.push(format!("{:>7.0} Hz  {gain:+.1} dB", frequency));
        }
        self.show_modal(lines.join("\n"));
    }

    fn apply_equalizer(&mut self) {
        match AppSettings::save_equalizer(&self.equalizer) {
            Ok(()) => {
                self.player.set_equalizer(self.equalizer.clone());
                self.status = format!(
                    "Equalizer: {} · {}",
                    if self.equalizer.enabled { "On" } else { "Off" },
                    self.equalizer.preset_name
                );
            }
            Err(error) => self.status = error,
        }
    }

    fn show_output_devices(&mut self) {
        match available_output_devices() {
            Ok(devices) => {
                let mut lines = vec![
                    "Available output devices (enter name in Preferences):".to_owned(),
                    "default".to_owned(),
                ];
                lines.extend(devices.into_iter().map(|device| device.name));
                self.show_modal(lines.join("\n"));
            }
            Err(error) => self.show_modal(error),
        }
    }

    fn show_supported_formats(&mut self) {
        let value: serde_json::Value =
            match serde_json::from_str(&self.decoders.supported_formats_json()) {
                Ok(value) => value,
                Err(error) => {
                    self.status = format!("Reading decoder formats: {error}");
                    return;
                }
            };
        let mut lines = vec![format!(
            "Supported Formats · {} extensions",
            value["uniqueExtensionCount"].as_u64().unwrap_or_default()
        )];
        for group in value["groups"].as_array().into_iter().flatten() {
            let name = group["name"].as_str().unwrap_or("Other");
            let detail = group["detail"].as_str().unwrap_or_default();
            let extensions = group["extensions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .map(|extension| format!(".{extension}"))
                .collect::<Vec<_>>()
                .join("  ");
            lines.push(format!("\n{name} · {detail}\n{extensions}"));
        }
        self.show_modal(lines.join("\n"));
    }

    fn browse(&mut self, path: Option<PathBuf>) {
        self.search = None;
        self.search_query.clear();
        let result = self.read_directory(path.as_deref());
        match result {
            Ok((location, items)) => {
                self.remote_search_generation
                    .fetch_add(1, Ordering::Relaxed);
                self.remote_active = false;
                self.remote_connection = None;
                self.remote_generation = self.remote_generation.wrapping_add(1);
                self.remote_pending.clear();
                self.remote_path.clear();
                self.browse_path = location;
                self.root_items = items;
                self.expanded.clear();
                self.children.clear();
                self.selected_tree.clear();
                self.tree_anchor = None;
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

    fn connect_remote(&mut self, path: Option<String>) {
        if let Err(error) = self.remote_settings.validate() {
            self.status = error;
            return;
        }
        self.remote_generation = self.remote_generation.wrapping_add(1);
        self.remote_search_generation
            .fetch_add(1, Ordering::Relaxed);
        let generation = self.remote_generation;
        let settings = self.remote_settings.clone();
        if self
            .remote_requests
            .send(RemoteCommand::Root(generation, settings, path))
            .is_ok()
        {
            self.status = "Connecting to remote library…".to_owned();
        } else {
            self.status = "Remote browser worker is unavailable".to_owned();
        }
    }

    fn poll_remote(&mut self) {
        while let Ok(response) = self.remote_results.try_recv() {
            match response {
                RemoteResponse::Root(generation, settings, result) => {
                    if generation != self.remote_generation {
                        continue;
                    }
                    match result.and_then(|listing| {
                        let path = listing.path.clone();
                        remote_items(&settings, listing).map(|items| (path, items))
                    }) {
                        Ok((path, items)) => {
                            self.remote_active = true;
                            self.remote_connection = Some(settings.clone());
                            self.remote_path = path;
                            self.browse_path = None;
                            self.search = None;
                            self.search_query.clear();
                            self.root_items = items;
                            self.expanded.clear();
                            self.children.clear();
                            self.remote_pending.clear();
                            self.selected_tree.clear();
                            self.tree_anchor = None;
                            self.rebuild_tree();
                            self.selected[1] = 0;
                            self.offsets[1] = 0;
                            self.focus = Focus::Library;
                            self.status = format!("Connected to {}", settings.server_url);
                        }
                        Err(error) => self.status = error,
                    }
                }
                RemoteResponse::Expand(generation, path, settings, result) => {
                    if generation != self.remote_generation || !self.remote_active {
                        continue;
                    }
                    self.remote_pending.remove(&path);
                    match result.and_then(|listing| remote_items(&settings, listing)) {
                        Ok(items) => {
                            self.children.insert(path.clone(), items);
                            self.expanded.insert(path);
                            self.rebuild_tree();
                        }
                        Err(error) => self.status = error,
                    }
                }
                RemoteResponse::Collect(generation, folder_generation, path, settings, result) => {
                    if generation != self.remote_generation
                        || folder_generation != self.folder_generation
                    {
                        continue;
                    }
                    let tracks = result.and_then(|files| {
                        files
                            .into_iter()
                            .map(|file| remote_track(&settings, file))
                            .collect()
                    });
                    self.accept_folder_result(path, tracks);
                }
                RemoteResponse::Search(generation, search_generation, settings, result) => {
                    if generation != self.remote_generation
                        || search_generation
                            != self.remote_search_generation.load(Ordering::Relaxed)
                    {
                        continue;
                    }
                    match result.and_then(|files| {
                        files
                            .into_iter()
                            .map(|file| {
                                if file.kind == "dir" {
                                    Ok(Item::Directory(file.name, PathBuf::from(file.path)))
                                } else {
                                    remote_track(&settings, file).map(Item::Track)
                                }
                            })
                            .collect::<Result<Vec<_>, String>>()
                    }) {
                        Ok(items) => {
                            self.root_items = items;
                            self.expanded.clear();
                            self.children.clear();
                            self.selected_tree.clear();
                            self.tree_anchor = None;
                            self.rebuild_tree();
                            self.search_done = true;
                            self.status = format!(
                                "{} remote matches for {}",
                                self.items.len(),
                                self.search_query
                            );
                        }
                        Err(error) if error == "Search superseded" => {}
                        Err(error) => self.status = error,
                    }
                }
                RemoteResponse::AddTracks(
                    generation,
                    folder_generation,
                    settings,
                    play,
                    result,
                ) => {
                    if generation != self.remote_generation
                        || folder_generation != self.folder_generation
                    {
                        continue;
                    }
                    match result.and_then(|files| {
                        files
                            .into_iter()
                            .map(|file| remote_track(&settings, file))
                            .collect::<Result<Vec<_>, _>>()
                    }) {
                        Ok(tracks) => {
                            let count = tracks.len();
                            let first = self.tracks.len();
                            self.tracks.extend(tracks);
                            self.order_tracks_changed();
                            if count > 0 {
                                self.selected[2] = first;
                                self.select_track(first, false, false);
                                self.status = format!("Added {count} track(s)");
                                if play {
                                    self.play_selected();
                                }
                            }
                        }
                        Err(error) => self.status = error,
                    }
                }
            }
        }
    }

    fn rebuild_tree(&mut self) {
        let anchor_key = self
            .tree_anchor
            .and_then(|index| self.items.get(index))
            .map(|row| tree_item_key(&row.item));
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
        let visible: HashSet<_> = self
            .items
            .iter()
            .map(|row| tree_item_key(&row.item))
            .collect();
        self.selected_tree.retain(|key| visible.contains(key));
        self.tree_anchor = anchor_key.and_then(|key| {
            self.items
                .iter()
                .position(|row| tree_item_key(&row.item) == key)
        });
    }

    fn select_tree_with_modifiers(&mut self, index: usize, shift: bool, ctrl: bool) {
        let Some(row) = self.items.get(index) else {
            return;
        };
        let key = tree_item_key(&row.item);
        if shift {
            let anchor = self.tree_anchor.unwrap_or(self.selected[1]);
            if !ctrl {
                self.selected_tree.clear();
            }
            for row in anchor.min(index)..=anchor.max(index) {
                if let Some(item) = self.items.get(row) {
                    self.selected_tree.insert(tree_item_key(&item.item));
                }
            }
        } else if ctrl {
            if !self.selected_tree.insert(key.clone()) {
                self.selected_tree.remove(&key);
            }
            self.tree_anchor = Some(index);
        } else {
            self.selected_tree.clear();
            self.selected_tree.insert(key);
            self.tree_anchor = Some(index);
        }
        self.selected[1] = index;
    }

    fn selected_tree_items(&self) -> Vec<Item> {
        let selected: Vec<_> = self
            .items
            .iter()
            .filter(|row| self.selected_tree.contains(&tree_item_key(&row.item)))
            .map(|row| row.item.clone())
            .collect();
        if selected.is_empty() && self.tree_anchor.is_none() {
            self.items
                .get(self.selected[1])
                .map(|row| vec![row.item.clone()])
                .unwrap_or_default()
        } else {
            selected
        }
    }

    fn toggle_directory(&mut self, path: PathBuf) {
        if self.expanded.remove(&path) {
            self.rebuild_tree();
            return;
        }
        if !self.children.contains_key(&path) {
            if self.remote_active {
                if self.remote_pending.insert(path.clone()) {
                    let Some(settings) = self.remote_connection.clone() else {
                        self.status = "Connect to a server first".to_owned();
                        return;
                    };
                    if self
                        .remote_requests
                        .send(RemoteCommand::Expand(
                            self.remote_generation,
                            settings,
                            path.clone(),
                        ))
                        .is_ok()
                    {
                        self.status = format!("Browsing {}…", path.display());
                    } else {
                        self.remote_pending.remove(&path);
                        self.status = "Remote browser worker is unavailable".to_owned();
                    }
                }
                return;
            }
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
        if index < self.lists.len() {
            self.selected[0] = index;
            self.selected_lists.clear();
            self.selected_lists.insert(self.lists[index].0);
            self.list_anchor = Some(index);
        }
    }

    fn select_list_with_modifiers(&mut self, index: usize, shift: bool, ctrl: bool) {
        if index >= self.lists.len() {
            return;
        }
        if shift {
            let anchor = self.list_anchor.unwrap_or(self.selected[0]);
            if !ctrl {
                self.selected_lists.clear();
            }
            for row in anchor.min(index)..=anchor.max(index) {
                self.selected_lists.insert(self.lists[row].0);
            }
        } else if ctrl {
            let id = self.lists[index].0;
            if !self.selected_lists.insert(id) {
                self.selected_lists.remove(&id);
            }
        } else {
            self.selected_lists.clear();
            self.selected_lists.insert(self.lists[index].0);
        }
        self.selected[0] = index;
        if !shift || self.list_anchor.is_none() {
            self.list_anchor = Some(index);
        }
    }

    fn selected_list_indices(&self) -> Vec<usize> {
        let selected: Vec<_> = self
            .lists
            .iter()
            .enumerate()
            .filter_map(|(index, (id, _))| self.selected_lists.contains(id).then_some(index))
            .collect();
        if selected.is_empty() && self.selected[0] < self.lists.len() {
            vec![self.selected[0]]
        } else {
            selected
        }
    }

    fn enqueue_selected_lists(&mut self, play: bool) {
        let selected = self.selected_list_indices();
        let mut last_start = None;
        for index in selected {
            last_start = Some(self.tracks.len());
            self.enqueue_list(index);
        }
        if play && let Some(start) = last_start.filter(|start| *start < self.tracks.len()) {
            self.selected[2] = start;
            self.play_selected();
        }
    }

    fn enqueue_list(&mut self, index: usize) {
        let Some((id, _)) = self.lists.get(index) else {
            return;
        };
        let result = if *id == 0 {
            self.library.db().starred_entries()
        } else {
            self.library.db().playlist_entries(*id)
        };
        match result {
            Ok(entries) => {
                let count = entries.len();
                self.tracks
                    .extend(entries.into_iter().map(track_from_entry));
                self.order_tracks_changed();
                self.status = format!("Added {count} tracks to the playlist");
            }
            Err(error) => self.status = error,
        }
    }

    fn enqueue_folder(&mut self, path: PathBuf) {
        if self.remote_active {
            let Some(settings) = self.remote_connection.clone() else {
                self.status = "Connect to a server first".to_owned();
                return;
            };
            if self
                .remote_requests
                .send(RemoteCommand::Collect(
                    self.remote_generation,
                    self.folder_generation,
                    settings,
                    path.clone(),
                ))
                .is_ok()
            {
                self.status = format!("Adding tracks from {}…", path.display());
            } else {
                self.status = "Remote browser worker is unavailable".to_owned();
            }
            return;
        }
        if self
            .folder_requests
            .send((self.folder_generation, path.clone()))
            .is_ok()
        {
            self.status = format!("Adding tracks from {}…", path.display());
        } else {
            self.status = "Folder scanner is unavailable".to_owned();
        }
    }

    fn poll_folders(&mut self) {
        while let Ok((generation, path, result)) = self.folder_results.try_recv() {
            if generation != self.folder_generation {
                continue;
            }
            self.accept_folder_result(path, result);
        }
    }

    fn accept_folder_result(&mut self, path: PathBuf, result: Result<Vec<Track>, String>) {
        let play_when_loaded = self.folder_play_pending.remove(&path);
        match result {
            Ok(tracks) => {
                let count = tracks.len();
                let first = self.tracks.len();
                self.tracks.extend(tracks);
                self.order_tracks_changed();
                self.status = format!("Added {count} tracks from folder");
                if play_when_loaded && count > 0 {
                    self.folder_play_pending.clear();
                    self.selected[2] = first;
                    self.select_track(first, false, false);
                    self.play_selected();
                }
            }
            Err(error) => self.status = error,
        }
    }

    fn add_selected(&mut self, play: bool) {
        self.add_selected_with_remote_play(play, play);
    }

    fn add_selected_with_remote_play(&mut self, play: bool, play_remote: bool) {
        let items = self.selected_tree_items();
        if items.is_empty() {
            return;
        }
        if play {
            self.folder_generation = self.folder_generation.wrapping_add(1);
            self.folder_play_pending.clear();
            self.player.stop();
            self.playing = None;
            self.tracks.clear();
            self.auto_fit_cache = None;
            self.order.clear_tracks();
            self.queue.clear();
            self.stop_after_rows.clear();
            self.selected_tracks.clear();
            self.session_dirty = true;
        }
        let first = self.tracks.len();
        let mut folders = Vec::new();
        let mut remote_files = Vec::new();
        let root = self.library.root();
        for item in items {
            match item {
                Item::Track(track) => {
                    if track.entry.kind == "remote" {
                        if let Some(file) =
                            RemoteFile::from_stream_url(track.name, &track.entry.path)
                        {
                            remote_files.push(file);
                        }
                    } else {
                        self.tracks
                            .extend(expand_track(&self.decoders, root.as_deref(), track));
                    }
                }
                Item::Directory(_, path) => {
                    self.enqueue_folder(path.clone());
                    folders.push(path);
                }
            }
        }
        let added = self.tracks.len() - first;
        if added > 0 {
            self.order_tracks_changed();
            self.selected[2] = first;
            self.select_track(first, false, false);
            self.status = format!("Added {added} track(s)");
        }
        if play && added == 0 {
            if remote_files.is_empty() {
                self.folder_play_pending.extend(folders);
            }
        }
        if play && added > 0 {
            self.play_selected();
        }
        if !remote_files.is_empty() {
            let Some(settings) = self.remote_connection.clone() else {
                self.status = "Connect to a server first".to_owned();
                return;
            };
            if self
                .remote_requests
                .send(RemoteCommand::AddTracks(
                    self.remote_generation,
                    self.folder_generation,
                    settings,
                    remote_files,
                    play_remote && added == 0,
                ))
                .is_ok()
            {
                self.status = "Expanding remote tracks…".to_owned();
            } else {
                self.status = "Remote browser worker is unavailable".to_owned();
            }
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
            }) => self.activate_selected(),
            None => {}
        }
    }

    fn activate_selected(&mut self) {
        match AppSettings::load().opening_files_behavior {
            OpeningFilesBehavior::ClearAndPlay => self.add_selected(true),
            OpeningFilesBehavior::Enqueue => self.add_selected(false),
            OpeningFilesBehavior::EnqueueAndPlay => {
                let first = self.tracks.len();
                let folders: Vec<_> = self
                    .selected_tree_items()
                    .into_iter()
                    .filter_map(|item| match item {
                        Item::Directory(_, path) => Some(path),
                        Item::Track(_) => None,
                    })
                    .collect();
                self.add_selected_with_remote_play(false, true);
                if first < self.tracks.len() {
                    self.selected[2] = first;
                    self.play_selected();
                } else {
                    self.folder_play_pending.extend(folders);
                }
            }
        }
    }

    fn play_selected(&mut self) {
        self.try_play_selected();
    }

    fn try_play_selected(&mut self) -> bool {
        let index = self.selected[2];
        let Some(track) = self.tracks.get(index) else {
            return false;
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
                let starting = self.playing.is_none();
                if let Some(previous) = self.playing
                    && previous != index
                {
                    self.stop_after_rows.remove(&previous);
                }
                self.playing = Some(index);
                self.auto_fit_cache = None;
                if starting && self.shuffle_mode != ShuffleMode::Off {
                    self.order.set_shuffle_mode(
                        self.shuffle_mode,
                        &self.order_tracks(),
                        self.playing,
                    );
                }
                self.status = format!("Playing {}", track.name);
                true
            }
            Err(error) => {
                self.status = error;
                false
            }
        }
    }

    fn next(&mut self, honor_repeat_one: bool) {
        if self.tracks.is_empty() {
            if self.radio_enabled {
                self.append_next_radio();
            }
            return;
        }
        if honor_repeat_one
            && (self.stop_after_current
                || self
                    .playing
                    .is_some_and(|index| self.stop_after_rows.contains(&index)))
        {
            self.stop_after_current = false;
            if let Some(index) = self.playing {
                self.stop_after_rows.remove(&index);
            }
            self.player.stop();
            self.playing = None;
            self.auto_fit_cache = None;
            return;
        }
        if honor_repeat_one && self.repeat_mode == RepeatMode::One {
            if let Some(index) = self.playing {
                self.selected[2] = index;
                if self.try_play_selected() {
                    return;
                }
            }
        }
        if self.radio_enabled && self.playing.is_some_and(|i| i + 1 >= self.tracks.len()) {
            self.append_next_radio();
            return;
        }
        let tracks = self.order_tracks();
        let mut cursor = self.playing;
        let mut attempted = HashSet::new();
        for _ in 0..self.tracks.len().saturating_add(self.queue.len()) {
            let queued = loop {
                match self.queue.pop_front() {
                    Some(index) if index < self.tracks.len() => break Some(index),
                    Some(_) => continue,
                    None => break None,
                }
            };
            let next = queued.or_else(|| self.order.next(&tracks, cursor, false));
            let Some(next) = next else { break };
            if !attempted.insert(next) {
                continue;
            }
            self.selected[2] = next;
            if self.try_play_selected() {
                return;
            }
            cursor = Some(next);
        }
        self.player.stop();
        self.playing = None;
        self.auto_fit_cache = None;
    }

    fn play_pause(&mut self) {
        if self.player.state() == PlaybackState::Stopped {
            if self.tracks.is_empty() && self.radio_enabled {
                self.append_next_radio();
            } else {
                self.play_selected();
            }
        } else {
            self.player.play_pause();
        }
    }

    fn previous(&mut self) {
        if self.tracks.is_empty() {
            return;
        }
        let tracks = self.order_tracks();
        if let Some(previous) = self.order.previous(&tracks, self.playing) {
            self.selected[2] = previous;
            self.play_selected();
        }
    }

    fn cycle_repeat(&mut self) {
        if self.radio_enabled {
            self.toggle_radio();
        }
        self.repeat_mode = self.repeat_mode.next();
        self.order.set_repeat_mode(self.repeat_mode);
        let _ = AppSettings::save_repeat_mode(self.repeat_mode);
        self.status = format!("Repeat: {}", self.repeat_mode.setting_value());
    }

    fn cycle_shuffle(&mut self) {
        if self.radio_enabled {
            self.toggle_radio();
        }
        self.shuffle_mode = self.shuffle_mode.next();
        self.order
            .set_shuffle_mode(self.shuffle_mode, &self.order_tracks(), self.playing);
        let _ = AppSettings::save_shuffle_mode(self.shuffle_mode);
        self.status = format!("Shuffle: {}", self.shuffle_mode.setting_value());
    }

    fn remove_selected(&mut self) {
        if self.tracks.is_empty() {
            return;
        }
        let mut indices: Vec<_> = self
            .selected_tracks
            .iter()
            .copied()
            .filter(|&index| index < self.tracks.len())
            .collect();
        if indices.is_empty() {
            indices.push(self.selected[2].min(self.tracks.len() - 1));
        }
        indices.sort_unstable();
        let first = indices[0];
        let remap = |old: usize| {
            if indices.binary_search(&old).is_ok() {
                None
            } else {
                Some(old - indices.iter().filter(|&&index| index < old).count())
            }
        };
        self.queue = self.queue.iter().filter_map(|&old| remap(old)).collect();
        self.stop_after_rows = self
            .stop_after_rows
            .iter()
            .filter_map(|&old| remap(old))
            .collect();
        let was_playing = self
            .playing
            .is_some_and(|index| indices.binary_search(&index).is_ok());
        if was_playing {
            self.player.stop();
            self.playing = None;
        } else if let Some(playing) = self.playing {
            self.playing = Some(playing - indices.iter().filter(|&&index| index < playing).count());
        }
        for index in indices.iter().rev() {
            self.tracks.remove(*index);
        }
        self.selected[2] = first.min(self.tracks.len().saturating_sub(1));
        self.selected_tracks.clear();
        self.selection_anchor = None;
        if !self.tracks.is_empty() {
            self.select_track(self.selected[2], false, false);
        }
        self.order_tracks_changed();
        self.status = format!("Removed {} track(s)", indices.len());
    }

    fn select_track(&mut self, index: usize, shift: bool, ctrl: bool) {
        if index >= self.tracks.len() {
            return;
        }
        if shift {
            let anchor = self.selection_anchor.unwrap_or(self.selected[2]);
            if !ctrl {
                self.selected_tracks.clear();
            }
            let visible = self.visible_tracks();
            if let (Some(start), Some(end)) = (
                visible.iter().position(|&row| row == anchor),
                visible.iter().position(|&row| row == index),
            ) {
                for &row in &visible[start.min(end)..=start.max(end)] {
                    self.selected_tracks.insert(row);
                }
            } else {
                self.selected_tracks.insert(index);
                self.selection_anchor = Some(index);
            }
        } else if ctrl {
            if !self.selected_tracks.insert(index) {
                self.selected_tracks.remove(&index);
            }
            self.selection_anchor = Some(index);
        } else {
            self.selected_tracks.clear();
            self.selected_tracks.insert(index);
            self.selection_anchor = Some(index);
        }
        self.selected[2] = index;
    }

    fn toggle_range_click(&mut self) {
        if self.range_click_pending == Some(self.focus) {
            self.range_click_pending = None;
            self.status = "Range selection finished".to_owned();
            return;
        }
        let available = match self.focus {
            Focus::Playlists if !self.lists.is_empty() => {
                self.select_list(self.selected[0]);
                true
            }
            Focus::Library if !self.items.is_empty() => {
                self.select_tree_with_modifiers(self.selected[1], false, false);
                true
            }
            Focus::Tracks if !self.tracks.is_empty() => {
                self.select_track(self.selected[2], false, false);
                true
            }
            _ => false,
        };
        if available {
            self.range_click_pending = Some(self.focus);
            self.last_click = None;
            self.status = "Range selection: arrows or click the last row · v finishes".to_owned();
        }
    }

    fn extend_keyboard_range(&mut self, delta: isize, page: usize) {
        match self.focus {
            Focus::Playlists => {
                let anchor = self.list_anchor.unwrap_or(self.selected[0]);
                self.move_selection(delta, page);
                self.list_anchor = Some(anchor);
                self.select_list_with_modifiers(self.selected[0], true, false);
                self.list_anchor = Some(anchor);
            }
            Focus::Library => {
                let anchor = self.tree_anchor.unwrap_or(self.selected[1]);
                self.move_selection(delta, page);
                self.tree_anchor = Some(anchor);
                self.select_tree_with_modifiers(self.selected[1], true, false);
                self.tree_anchor = Some(anchor);
            }
            Focus::Tracks => {
                let anchor = self.selection_anchor.unwrap_or(self.selected[2]);
                self.move_selection(delta, page);
                self.selection_anchor = Some(anchor);
                self.select_track(self.selected[2], true, false);
            }
        }
    }

    fn move_track(&mut self, from: usize, to: usize) {
        if from == to || from >= self.tracks.len() || to >= self.tracks.len() {
            return;
        }
        let remap = |index: usize| -> usize {
            if index == from {
                to
            } else if from < to && index > from && index <= to {
                index - 1
            } else if to < from && index >= to && index < from {
                index + 1
            } else {
                index
            }
        };
        let track = self.tracks.remove(from);
        self.tracks.insert(to, track);
        self.playing = self.playing.map(remap);
        self.queue = self.queue.iter().copied().map(remap).collect();
        self.stop_after_rows = self.stop_after_rows.iter().copied().map(remap).collect();
        self.selected_tracks = self.selected_tracks.iter().copied().map(remap).collect();
        self.selected[2] = remap(self.selected[2]);
        self.selection_anchor = self.selection_anchor.map(remap);
        self.order_tracks_changed();
        self.status = format!("Moved track {} to {}", from + 1, to + 1);
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
                let key = metadata_key(&track.entry);
                if starred {
                    self.starred_keys.insert(key);
                } else {
                    self.starred_keys.remove(&key);
                }
                self.status = if starred {
                    format!("Starred {}", track.name)
                } else {
                    format!("Unstarred {}", track.name)
                };
            }
            Err(error) => self.status = error,
        }
    }

    fn blacklist_selected_tree(&mut self, folder: bool) {
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        for item in self.selected_tree_items() {
            let entry = match (&item, folder) {
                (Item::Directory(_, path), true) => {
                    if let Ok(Some(location)) = kog_audio::archive::tree_location(path) {
                        BlacklistEntry {
                            id: 0,
                            kind: BLACKLIST_FOLDER.to_owned(),
                            path: if location.entry.is_empty() {
                                canonical_blacklist_path(&location.archive.to_string_lossy())
                            } else {
                                format!(
                                    "{} :: {}",
                                    canonical_blacklist_path(&location.archive.to_string_lossy()),
                                    location.entry
                                )
                            },
                            entry: String::new(),
                        }
                    } else if path.is_dir() {
                        BlacklistEntry {
                            id: 0,
                            kind: BLACKLIST_FOLDER.to_owned(),
                            path: canonical_blacklist_path(&path.to_string_lossy()),
                            entry: String::new(),
                        }
                    } else {
                        continue;
                    }
                }
                (Item::Track(track), false) => blacklist_song(&track.entry),
                _ => continue,
            };
            let key = format!("{}:{}:{}", entry.kind, entry.path, entry.entry);
            if seen.insert(key) {
                entries.push(entry);
            }
        }
        self.commit_blacklist(entries);
    }

    fn blacklist_selected_tracks(&mut self, folder: bool) {
        let indices: Vec<_> = if self.selected_tracks.is_empty() {
            vec![self.selected[2]]
        } else {
            self.selected_tracks.iter().copied().collect()
        };
        let mut entries = Vec::new();
        let mut seen = HashSet::new();
        for index in indices {
            let Some(track) = self.tracks.get(index) else {
                continue;
            };
            let entry = if folder {
                if track.entry.kind != "local" {
                    continue;
                }
                let Some(parent) = Path::new(&track.entry.path).parent() else {
                    continue;
                };
                BlacklistEntry {
                    id: 0,
                    kind: BLACKLIST_FOLDER.to_owned(),
                    path: canonical_blacklist_path(&parent.to_string_lossy()),
                    entry: String::new(),
                }
            } else {
                blacklist_song(&track.entry)
            };
            let key = format!("{}:{}:{}", entry.kind, entry.path, entry.entry);
            if seen.insert(key) {
                entries.push(entry);
            }
        }
        self.commit_blacklist(entries);
    }

    fn commit_blacklist(&mut self, entries: Vec<BlacklistEntry>) {
        if entries.is_empty() {
            self.status = "Nothing selected can be blacklisted".to_owned();
            return;
        }
        let result = self.library.db().add_blacklist_entries(&entries);
        match result {
            Ok(added) => {
                self.status = if added == 0 {
                    "Already blacklisted".to_owned()
                } else {
                    format!("Blacklisted {added} item(s)")
                };
                self.refresh_radio_blacklist();
            }
            Err(error) => self.status = error,
        }
    }

    fn refresh_radio_blacklist(&mut self) {
        if self.radio_enabled {
            self.radio_generation = self.radio_generation.wrapping_add(1);
            self.radio_pool.clear();
            self.radio_refill_pending = false;
            self.radio_exhausted = false;
            self.request_radio_command(RadioCommand::Reshuffle);
        }
    }

    fn show_blacklist(&mut self) {
        self.info_modal = false;
        self.artwork_modal = false;
        match self.library.db().list_blacklist() {
            Ok(entries) => {
                self.modal = Some(if entries.is_empty() {
                    "Blacklist is empty".to_owned()
                } else {
                    format!(
                        "Blacklist · use Preferences to remove by ID\n{}",
                        entries
                            .iter()
                            .map(|entry| format!(
                                "{}  {}  {}{}",
                                entry.id,
                                entry.kind,
                                entry.path,
                                if entry.entry.is_empty() {
                                    String::new()
                                } else {
                                    format!(" :: {}", entry.entry)
                                }
                            ))
                            .collect::<Vec<_>>()
                            .join("\n")
                    )
                });
                self.modal_scroll = 0;
            }
            Err(error) => self.status = error,
        }
    }

    fn open_tag_editor(&mut self) {
        if self.tag_busy {
            return;
        }
        let mut indices: Vec<_> = if self.selected_tracks.is_empty() {
            vec![self.selected[2]]
        } else {
            self.selected_tracks.iter().copied().collect()
        };
        indices.sort_unstable();
        let mut sources = Vec::new();
        for index in indices {
            let Some(track) = self.tracks.get(index) else {
                continue;
            };
            if track.entry.kind != "local" || track.entry.fragment.is_some() {
                self.status = "Tags can only be edited on local, whole files".to_owned();
                return;
            }
            sources.push(PlaybackSource::from_path(PathBuf::from(&track.entry.path)));
        }
        if sources.is_empty() {
            self.status = "Select a track to edit tags".to_owned();
            return;
        }
        self.tag_busy = true;
        if self.tag_requests.send(TagCommand::Open(sources)).is_err() {
            self.tag_busy = false;
            self.status = "Tag worker is unavailable".to_owned();
        } else {
            self.status = "Reading tags…".to_owned();
        }
    }

    fn save_tag_edits(&mut self) {
        let Some(session) = self.tag_session.as_ref() else {
            return;
        };
        if session.fields.is_empty() && session.artwork.is_none() {
            self.status = "No tag changes staged".to_owned();
            self.open_submenu(MenuPage::TagEditor);
            return;
        }
        let mut request = serde_json::Map::new();
        if !session.fields.is_empty() {
            request.insert(
                "fields".to_owned(),
                serde_json::Value::Object(session.fields.clone()),
            );
        }
        if let Some(artwork) = &session.artwork {
            request.insert("artwork".to_owned(), artwork.clone());
        }
        let request = serde_json::Value::Object(request).to_string();
        if let Err(error) = parse_edits(&request) {
            self.status = error;
            self.open_submenu(MenuPage::TagEditor);
            return;
        }
        let sources = session.sources.clone();
        self.tag_resume = self
            .playing
            .and_then(|index| self.tracks.get(index).map(|track| (index, track)))
            .filter(|(_, track)| {
                sources
                    .iter()
                    .any(|source| source.path == PathBuf::from(&track.entry.path))
            })
            .and_then(|(index, _)| {
                let state = self.player.state();
                (state != PlaybackState::Stopped).then_some((index, self.player.position(), state))
            });
        if self.tag_resume.is_some() {
            self.player.stop();
        }
        self.tag_busy = true;
        if self
            .tag_requests
            .send(TagCommand::Save(sources, request))
            .is_err()
        {
            self.tag_busy = false;
            self.status = "Tag worker is unavailable".to_owned();
        } else {
            self.status = "Writing tags…".to_owned();
        }
    }

    fn poll_tags(&mut self) {
        while let Ok(response) = self.tag_results.try_recv() {
            self.tag_busy = false;
            match response {
                TagResponse::Open(sources, result) => match result {
                    Ok(mut snapshot) => {
                        if let Some(artwork) = snapshot
                            .get_mut("artwork")
                            .and_then(|value| value.as_object_mut())
                        {
                            artwork.remove("uri");
                        }
                        self.status = format!("Editing tags for {} file(s)", sources.len());
                        self.tag_session = Some(TagSession {
                            sources,
                            snapshot,
                            fields: serde_json::Map::new(),
                            artwork: None,
                        });
                        self.open_submenu(MenuPage::TagEditor);
                    }
                    Err(error) => self.status = error,
                },
                TagResponse::Saved(paths, error) => {
                    let succeeded = error.is_none();
                    if self
                        .playing
                        .and_then(|index| self.tracks.get(index))
                        .is_some_and(|track| {
                            paths
                                .iter()
                                .any(|path| path == Path::new(&track.entry.path))
                        })
                    {
                        if let Some(cover) = &self.cover_preview {
                            let _ = std::fs::remove_file(&cover.path);
                        }
                        self.cover_key.clear();
                        self.cover_preview = None;
                    }
                    for track in &self.tracks {
                        if paths
                            .iter()
                            .any(|path| path == Path::new(&track.entry.path))
                        {
                            let key = metadata_key(&track.entry);
                            self.metadata.remove(&key);
                            self.metadata_pending.remove(&key);
                        }
                    }
                    if !paths.is_empty() {
                        self.auto_fit_cache = None;
                    }
                    if !paths.is_empty() && self.shuffle_mode == ShuffleMode::Albums {
                        let tracks = self.order_tracks();
                        self.order.album_metadata_changed(&tracks, self.playing);
                    }
                    if let Some((index, position, state)) = self.tag_resume.take() {
                        let selected = self.selected[2];
                        self.selected[2] = index;
                        self.play_selected();
                        let _ = self.player.seek(position);
                        if state == PlaybackState::Paused {
                            self.player.play_pause();
                        }
                        self.selected[2] = selected;
                    }
                    self.status = error
                        .unwrap_or_else(|| format!("Updated tags for {} file(s)", paths.len()));
                    if succeeded {
                        self.tag_session = None;
                    } else {
                        self.open_submenu(MenuPage::TagEditor);
                    }
                }
            }
        }
    }

    fn begin_trash_selected(&mut self) {
        let mut paths: Vec<_> = self
            .selected_tree_items()
            .into_iter()
            .filter_map(|item| match item {
                Item::Directory(_, path)
                    if path.is_absolute() && !kog_audio::archive::is_tree_location(&path) =>
                {
                    Some(path)
                }
                Item::Track(track) if track.entry.kind == "local" => {
                    Some(PathBuf::from(&track.entry.path))
                }
                _ => None,
            })
            .filter(|path| path.is_absolute() && path.parent().is_some())
            .collect();
        paths.sort_by_key(|path| path.components().count());
        let mut unique = Vec::new();
        for path in paths {
            if !unique
                .iter()
                .any(|parent: &PathBuf| path.starts_with(parent))
            {
                unique.push(path);
            }
        }
        if unique.is_empty() {
            self.status = "Select a local file or folder to move to trash".to_owned();
            return;
        }
        self.pending_delete_paths = unique;
        self.begin_prompt(PromptKind::TrashSelected, String::new());
        self.status = if self.pending_delete_paths.len() == 1 {
            format!(
                "Move {} to trash? Type yes to confirm",
                self.pending_delete_paths[0].display()
            )
        } else {
            format!(
                "Move {} selected items to trash? Type yes to confirm",
                self.pending_delete_paths.len()
            )
        };
    }

    fn poll_deletes(&mut self) {
        while let Ok((path, result)) = self.delete_results.try_recv() {
            match result {
                Ok(()) => {
                    self.root_items.retain(|item| !item_under_path(item, &path));
                    for items in self.children.values_mut() {
                        items.retain(|item| !item_under_path(item, &path));
                    }
                    self.children.retain(|item, _| !item.starts_with(&path));
                    self.expanded.retain(|item| !item.starts_with(&path));
                    self.rebuild_tree();
                    let doomed: Vec<_> = self
                        .tracks
                        .iter()
                        .enumerate()
                        .filter_map(|(index, track)| {
                            (track.entry.kind != "remote"
                                && Path::new(&track.entry.path).starts_with(&path))
                            .then_some(index)
                        })
                        .collect();
                    if !doomed.is_empty() {
                        self.selected_tracks = doomed.into_iter().collect();
                        self.remove_selected();
                    }
                    self.status = format!("Moved {} to trash", path.display());
                }
                Err(error) => self.status = format!("Moving {} to trash: {error}", path.display()),
            }
        }
    }

    fn begin_prompt(&mut self, kind: PromptKind, value: String) {
        if kind == PromptKind::MusicFolder {
            match FolderChooser::new(Path::new(&value)) {
                Ok(chooser) => {
                    self.folder_chooser = Some(chooser);
                    self.prompt = None;
                    self.menu_open = false;
                    self.menu_parents.clear();
                }
                Err(error) => self.status = error,
            }
            return;
        }
        if kind == PromptKind::Search {
            self.files_expanded = true;
            self.focus = Focus::Library;
        }
        self.input_cursor = value.chars().count();
        self.input_select_all = false;
        self.prompt = Some((kind, value));
    }

    fn begin_seek_prompt(&mut self) {
        let position = self.player.position().as_secs();
        self.begin_prompt(
            PromptKind::SeekPosition,
            format!("{}:{:02}", position / 60, position % 60),
        );
        self.input_select_all = true;
    }

    fn apply_music_folder(&mut self, path: &Path) -> Result<(), String> {
        let directory = path
            .canonicalize()
            .map_err(|error| format!("Opening {}: {error}", path.display()))?;
        if !directory.is_dir() {
            return Err(format!("{} is not a directory", directory.display()));
        }
        AppSettings::save_music_directory(&directory)?;
        self.library.set_root(Some(directory.clone()));
        if let Some(server) = &self.api_server {
            server.library.set_root(Some(directory));
        }
        self.browse(None);
        Ok(())
    }

    fn accept_folder_chooser(&mut self) {
        let Some(chooser) = self.folder_chooser.as_mut() else {
            return;
        };
        if chooser.location.trim() != chooser.directory.to_string_lossy() {
            chooser.navigate_location();
            if !chooser.error.is_empty() {
                return;
            }
        }
        let path = chooser.directory.clone();
        match self.apply_music_folder(&path) {
            Ok(()) => self.folder_chooser = None,
            Err(error) => {
                if let Some(chooser) = self.folder_chooser.as_mut() {
                    chooser.error = error;
                }
            }
        }
    }

    fn update_search_draft(&mut self, kind: PromptKind, value: &str) {
        match kind {
            PromptKind::Search => {
                if value.trim().is_empty() {
                    self.search_due = None;
                    if self.remote_active {
                        self.connect_remote(Some(self.remote_path.clone()));
                    } else {
                        self.browse(None);
                    }
                } else {
                    self.search_due = Some(Instant::now() + Duration::from_millis(250));
                }
            }
            PromptKind::PlaylistSearch => {
                self.playlist_query = value.to_lowercase();
                self.offsets[2] = 0;
                if let Some(first) = self.visible_tracks().first() {
                    self.selected[2] = *first;
                }
            }
            _ => {}
        }
    }

    fn run_file_search(&mut self, value: &str) {
        if value.trim().is_empty() {
            if self.remote_active {
                self.connect_remote(Some(self.remote_path.clone()));
            } else {
                self.browse(None);
            }
            return;
        }
        if self.remote_active {
            let Some(settings) = self.remote_connection.clone() else {
                self.status = "Connect to a server first".to_owned();
                return;
            };
            self.search_query = value.to_owned();
            self.search_done = false;
            self.items.clear();
            self.selected_tree.clear();
            self.tree_anchor = None;
            self.selected[1] = 0;
            self.offsets[1] = 0;
            self.focus = Focus::Library;
            let search_generation = self
                .remote_search_generation
                .fetch_add(1, Ordering::Relaxed)
                .wrapping_add(1);
            if self
                .remote_requests
                .send(RemoteCommand::Search(
                    self.remote_generation,
                    search_generation,
                    settings,
                    value.to_owned(),
                ))
                .is_ok()
            {
                self.status = format!("Searching server for {value}…");
            } else {
                self.status = "Remote browser worker is unavailable".to_owned();
            }
            return;
        }
        if self.search_query == value && self.search.is_some() {
            return;
        }
        match LocalSearch::start(self.library.clone(), value) {
            Ok(search) => {
                self.search = Some(search);
                self.search_query = value.to_owned();
                self.search_done = false;
                self.items.clear();
                self.selected_tree.clear();
                self.tree_anchor = None;
                self.selected[1] = 0;
                self.offsets[1] = 0;
                self.focus = Focus::Library;
                self.status = format!("Searching for {value}…");
            }
            Err(error) => self.status = error,
        }
    }

    fn poll_search_due(&mut self) {
        if self.search_due.is_some_and(|at| Instant::now() >= at) {
            self.search_due = None;
            if let Some((PromptKind::Search, value)) = &self.prompt {
                let value = value.clone();
                self.run_file_search(&value);
            }
        }
    }

    fn finish_prompt(&mut self, kind: PromptKind, value: String) {
        let raw = value;
        let value = raw.trim();
        if value.is_empty()
            && !matches!(
                kind,
                PromptKind::PlaylistSearch
                    | PromptKind::Search
                    | PromptKind::TagField(_)
                    | PromptKind::RemoteToken
                    | PromptKind::RemoteUsername
                    | PromptKind::RemotePassword
                    | PromptKind::SoundFont
                    | PromptKind::Sc55Roms
                    | PromptKind::Mt32Roms
                    | PromptKind::ServerToken
                    | PromptKind::ServerUsername
            )
        {
            self.status = "A value is required".to_owned();
            return;
        }
        match kind {
            PromptKind::MusicFolder => match self.apply_music_folder(Path::new(value)) {
                Ok(()) => {}
                Err(error) => self.status = error,
            },
            PromptKind::NewPlaylist => {
                let result = self.library.db().create_playlist(value);
                match result {
                    Ok(id) => {
                        self.reload_lists();
                        if let Some(index) =
                            self.lists.iter().position(|(list_id, _)| *list_id == id)
                        {
                            self.select_list(index);
                            self.offsets[0] = index.saturating_sub(5);
                        }
                        self.status = format!("Created {value}");
                    }
                    Err(error) => self.status = error,
                }
            }
            PromptKind::RenamePlaylist => {
                let id = self.lists.get(self.selected[0]).map_or(0, |(id, _)| *id);
                if id <= 0 {
                    return;
                }
                let result = self.library.db().rename_playlist(id, value);
                match result {
                    Ok(()) => {
                        self.reload_lists();
                        self.status = format!("Renamed to {value}");
                    }
                    Err(error) => self.status = error,
                }
            }
            PromptKind::DeletePlaylist => {
                if value != "yes" {
                    self.status = "Playlist deletion cancelled".to_owned();
                    return;
                }
                let ids: Vec<_> = self
                    .selected_list_indices()
                    .into_iter()
                    .filter_map(|index| self.lists.get(index).map(|(id, _)| *id))
                    .filter(|id| *id > 0)
                    .collect();
                let mut deleted = 0;
                for id in ids {
                    match self.library.db().delete_playlist(id) {
                        Ok(()) => deleted += 1,
                        Err(error) => {
                            self.status = error;
                            break;
                        }
                    }
                }
                if deleted > 0 {
                    self.reload_lists();
                    self.select_list(0);
                    self.status = if deleted == 1 {
                        "Playlist deleted".to_owned()
                    } else {
                        format!("Deleted {deleted} playlists")
                    };
                }
            }
            PromptKind::Search => {
                self.search_due = None;
                self.run_file_search(value);
            }
            PromptKind::PlaylistSearch => {
                self.playlist_query = value.to_lowercase();
                self.offsets[2] = 0;
                if let Some(first) = self.visible_tracks().first() {
                    self.selected[2] = *first;
                }
            }
            PromptKind::AddFile => {
                let path = PathBuf::from(value);
                if path.is_dir() || (path.is_file() && kog_audio::archive::is_path(&path)) {
                    self.enqueue_folder(path);
                } else if path.is_file() && self.decoders.accepts_path(&path) {
                    let track = track_from_entry(StoredEntry {
                        kind: "local".to_owned(),
                        path: path.to_string_lossy().into_owned(),
                        entry: String::new(),
                        fragment: None,
                    });
                    self.status = format!("Added {}", track.name);
                    self.tracks.push(track);
                    self.order_tracks_changed();
                } else {
                    self.status = format!("Cannot add {}", path.display());
                }
            }
            PromptKind::AddUrl => {
                if !value.starts_with("http://") && !value.starts_with("https://") {
                    self.status = "Enter an http or https URL".to_owned();
                } else {
                    let track = track_from_entry(StoredEntry {
                        kind: "remote".to_owned(),
                        path: value.to_owned(),
                        entry: String::new(),
                        fragment: None,
                    });
                    self.status = format!("Added {}", track.name);
                    self.tracks.push(track);
                    self.order_tracks_changed();
                }
            }
            PromptKind::SavePlaylist | PromptKind::SaveSelection => {
                let entries: Vec<_> = if kind == PromptKind::SaveSelection {
                    self.tracks
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| self.selected_tracks.contains(index))
                        .map(|(_, track)| track.entry.clone())
                        .collect()
                } else {
                    self.tracks
                        .iter()
                        .map(|track| track.entry.clone())
                        .collect()
                };
                if entries.is_empty() {
                    self.status = "Select tracks to save".to_owned();
                    return;
                }
                let created = self.library.db().create_playlist(value);
                match created {
                    Ok(id) => {
                        let appended = self.library.db().append_entries(id, &entries);
                        match appended {
                            Ok(()) => {
                                self.reload_lists();
                                if let Some(index) =
                                    self.lists.iter().position(|(list_id, _)| *list_id == id)
                                {
                                    self.select_list(index);
                                    self.offsets[0] = index.saturating_sub(5);
                                }
                                self.status = format!("Saved {} tracks to {value}", entries.len());
                            }
                            Err(error) => self.status = error,
                        }
                    }
                    Err(error) => self.status = error,
                }
            }
            PromptKind::DuplicatePlaylist => {
                let id = self.lists.get(self.selected[0]).map_or(0, |(id, _)| *id);
                if id <= 0 {
                    return;
                }
                let result = self.library.db().duplicate_playlist(id, value);
                match result {
                    Ok(new_id) => {
                        self.reload_lists();
                        if let Some(index) = self.lists.iter().position(|(id, _)| *id == new_id) {
                            self.select_list(index);
                            self.offsets[0] = index.saturating_sub(5);
                        }
                        self.status = format!("Duplicated playlist as {value}");
                    }
                    Err(error) => self.status = error,
                }
            }
            PromptKind::ExportPlaylist => {
                let id = self.lists.get(self.selected[0]).map_or(0, |(id, _)| *id);
                let entries = if id == 0 {
                    self.library.db().starred_entries()
                } else {
                    self.library.db().playlist_entries(id)
                };
                match entries {
                    Ok(entries) if entries.is_empty() => {
                        self.status = "Playlist is empty".to_owned()
                    }
                    Ok(entries) => {
                        let mut path = PathBuf::from(value);
                        if !matches!(
                            path.extension().and_then(|ext| ext.to_str()),
                            Some("m3u" | "m3u8")
                        ) {
                            path.set_extension("m3u");
                        }
                        let count = entries.len();
                        let entries: Vec<_> = entries.iter().map(playlist_entry).collect();
                        match Playlist::save_portable(&path, &entries) {
                            Ok(()) => {
                                self.status =
                                    format!("Exported {count} tracks to {}", path.display())
                            }
                            Err(error) => self.status = error,
                        }
                    }
                    Err(error) => self.status = error,
                }
            }
            PromptKind::Volume => match value.parse::<u8>() {
                Ok(percent) if percent <= 100 => {
                    self.volume = f32::from(percent) / 100.0;
                    self.player.set_volume(self.volume);
                }
                _ => self.status = "Volume must be between 0 and 100".to_owned(),
            },
            PromptKind::SeekPosition => match parse_seek_position(value) {
                Some(seconds) => match self.player.seek(Duration::from_secs(seconds)) {
                    Ok(()) => {
                        self.status = format!("Seeked to {}:{:02}", seconds / 60, seconds % 60)
                    }
                    Err(error) => self.status = format!("Seeking: {error}"),
                },
                None => self.status = "Enter seconds, mm:ss, or hh:mm:ss".to_owned(),
            },
            PromptKind::AddToPlaylist => {
                let Some((id, _)) = self
                    .lists
                    .iter()
                    .find(|(id, name)| *id > 0 && name.eq_ignore_ascii_case(value))
                else {
                    self.status = format!("No saved playlist named {value}");
                    return;
                };
                let entries: Vec<_> = if self.focus == Focus::Library {
                    self.selected_tree_items()
                        .into_iter()
                        .filter_map(|item| match item {
                            Item::Track(track) => Some(track.entry),
                            Item::Directory(..) => None,
                        })
                        .collect()
                } else {
                    self.selected_tracks
                        .iter()
                        .copied()
                        .filter_map(|index| self.tracks.get(index).map(|track| track.entry.clone()))
                        .collect()
                };
                let entries = if entries.is_empty() && self.focus != Focus::Library {
                    self.tracks
                        .get(self.selected[2])
                        .map(|track| vec![track.entry.clone()])
                        .unwrap_or_default()
                } else {
                    entries
                };
                if entries.is_empty() {
                    self.status = "Select a track to add".to_owned();
                    return;
                }
                let id = *id;
                let result = self.library.db().append_entries(id, &entries);
                match result {
                    Ok(()) => {
                        self.reload_lists();
                        self.status = format!("Added {} track(s) to {value}", entries.len());
                    }
                    Err(error) => self.status = error,
                }
            }
            PromptKind::EqualizerPreset => {
                if let Some(preset) = presets()
                    .iter()
                    .find(|preset| preset.name.eq_ignore_ascii_case(value))
                {
                    self.equalizer.preset_name = preset.name.clone();
                    self.equalizer.preamp_db = preset.preamp_db;
                    self.equalizer.gains_db = preset.gains_db;
                    self.equalizer.enabled = true;
                    self.apply_equalizer();
                } else {
                    self.status = format!("Unknown equalizer preset: {value}");
                }
            }
            PromptKind::EqualizerBandNumber => match value.parse::<usize>() {
                Ok(number) if (1..=self.equalizer.gains_db.len()).contains(&number) => {
                    let index = number - 1;
                    self.begin_prompt(
                        PromptKind::EqualizerBandGain(index),
                        format!("{}", self.equalizer.gains_db[index]),
                    );
                }
                _ => self.status = "Equalizer band must be from 1 to 10".to_owned(),
            },
            PromptKind::EqualizerBandGain(index) => match value.parse::<f32>() {
                Ok(gain) if (-20.0..=20.0).contains(&gain) => {
                    self.equalizer.gains_db[index] = gain;
                    self.equalizer.preset_name = "Custom".to_owned();
                    self.equalizer.enabled = true;
                    self.apply_equalizer();
                }
                _ => self.status = "Equalizer gain must be between -20 and 20 dB".to_owned(),
            },
            PromptKind::Preamp => match value.parse::<f32>() {
                Ok(gain) if (-20.0..=20.0).contains(&gain) => {
                    self.equalizer.preamp_db = gain;
                    self.apply_equalizer();
                }
                _ => self.status = "Preamp must be between -20 and 20 dB".to_owned(),
            },
            PromptKind::OutputDevice => {
                if value.eq_ignore_ascii_case("default") {
                    match self.player.switch_output_device(None) {
                        Ok(()) => match AppSettings::save_output_device(None) {
                            Ok(()) => self.status = "Using default output".to_owned(),
                            Err(error) => self.status = error,
                        },
                        Err(error) => self.status = error,
                    }
                } else {
                    match available_output_devices() {
                        Ok(devices) => {
                            if let Some(device) = devices.into_iter().find(|device| {
                                device.name.eq_ignore_ascii_case(value) || device.id == value
                            }) {
                                match self.player.switch_output_device(Some(device.id.clone())) {
                                    Ok(()) => {
                                        let preference = OutputDevicePreference {
                                            id: device.id,
                                            name: device.name.clone(),
                                        };
                                        match AppSettings::save_output_device(Some(&preference)) {
                                            Ok(()) => {
                                                self.status =
                                                    format!("Using output {}", device.name)
                                            }
                                            Err(error) => self.status = error,
                                        }
                                    }
                                    Err(error) => self.status = error,
                                }
                            } else {
                                self.status = format!("No output device named {value}");
                            }
                        }
                        Err(error) => self.status = error,
                    }
                }
            }
            PromptKind::RemoveBlacklistId => match value.parse::<i64>() {
                Ok(id) => {
                    let result = self.library.db().remove_blacklist_entry(id);
                    match result {
                        Ok(()) => {
                            self.refresh_radio_blacklist();
                            self.status = "Removed from blacklist".to_owned();
                        }
                        Err(error) => self.status = error,
                    }
                }
                Err(_) => self.status = "Enter a blacklist ID".to_owned(),
            },
            PromptKind::TrashSelected => {
                let paths = std::mem::take(&mut self.pending_delete_paths);
                if value != "yes" {
                    self.status = "Move to trash cancelled".to_owned();
                } else {
                    let count = paths.len();
                    for path in &paths {
                        if self.delete_requests.send(path.clone()).is_err() {
                            self.status = "Trash worker is unavailable".to_owned();
                            return;
                        }
                    }
                    self.status = if count == 1 {
                        format!("Moving {} to trash…", paths[0].display())
                    } else {
                        format!("Moving {count} selected items to trash…")
                    };
                }
            }
            PromptKind::TagField(index) => {
                if let Some(session) = self.tag_session.as_mut() {
                    session.fields.insert(
                        TAG_FIELDS[index].0.to_owned(),
                        serde_json::Value::String(value.to_owned()),
                    );
                    self.status = format!("{} change staged", TAG_FIELDS[index].1);
                    self.open_submenu(MenuPage::TagEditor);
                }
            }
            PromptKind::TagArtwork => {
                if let Some(session) = self.tag_session.as_mut() {
                    session.artwork = Some(serde_json::json!({"action":"replace","path":value}));
                    self.status = "Artwork replacement staged".to_owned();
                    self.open_submenu(MenuPage::TagEditor);
                }
            }
            PromptKind::RemoteUrl => {
                self.remote_settings.server_url = value.to_owned();
                self.persist_remote_settings(true);
            }
            PromptKind::RemoteToken => {
                self.remote_settings.token = value.to_owned();
                self.persist_remote_settings(true);
            }
            PromptKind::RemoteUsername => {
                self.remote_settings.username = value.to_owned();
                self.persist_remote_settings(true);
            }
            PromptKind::RemotePassword => {
                self.remote_settings.password = raw;
                self.persist_remote_settings(true);
            }
            PromptKind::SoundFont => {
                let path = if value.is_empty() {
                    None
                } else {
                    let path = PathBuf::from(value);
                    if !path
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("sf2"))
                    {
                        self.status = "Choose an SF2 SoundFont".to_owned();
                        return;
                    }
                    let path = match path.canonicalize().and_then(|path| {
                        if path.is_file() {
                            Ok(path)
                        } else {
                            Err(io::Error::other("not a file"))
                        }
                    }) {
                        Ok(path) => path,
                        Err(error) => {
                            self.status = format!("Opening SoundFont: {error}");
                            return;
                        }
                    };
                    if let Err(error) = validate_soundfont(&path) {
                        self.status = error;
                        return;
                    }
                    Some(path)
                };
                match AppSettings::save_soundfont_path(path.as_deref()) {
                    Ok(()) => {
                        self.decoder_settings.set_soundfont_path(path);
                        self.invalidate_metadata();
                        self.status = "MIDI SoundFont updated".to_owned();
                    }
                    Err(error) => self.status = error,
                }
            }
            PromptKind::Sc55Roms | PromptKind::Mt32Roms => {
                let path = if value.is_empty() {
                    None
                } else {
                    let path = match PathBuf::from(value).canonicalize() {
                        Ok(path) if path.is_dir() => path,
                        Ok(_) => {
                            self.status = "ROM path is not a directory".to_owned();
                            return;
                        }
                        Err(error) => {
                            self.status = format!("Opening ROM directory: {error}");
                            return;
                        }
                    };
                    let validation = if kind == PromptKind::Sc55Roms {
                        kog_audio::sc55::validate_rom_directory(&path)
                    } else {
                        kog_audio::mt32::validate_rom_directory(&path)
                    };
                    if let Err(error) = validation {
                        self.status = error;
                        return;
                    }
                    Some(path)
                };
                let result = if kind == PromptKind::Sc55Roms {
                    AppSettings::save_sc55_rom_path(path.as_deref())
                } else {
                    AppSettings::save_mt32_rom_path(path.as_deref())
                };
                match result {
                    Ok(()) => {
                        if kind == PromptKind::Sc55Roms {
                            self.decoder_settings.set_sc55_rom_path(path);
                        } else {
                            self.decoder_settings.set_mt32_rom_path(path);
                        }
                        self.invalidate_metadata();
                        self.status = "ROM directory updated".to_owned();
                    }
                    Err(error) => self.status = error,
                }
            }
            PromptKind::Sc55Archive | PromptKind::Mt32Archive => {
                let kind = if kind == PromptKind::Sc55Archive {
                    RomKind::Sc55
                } else {
                    RomKind::Mt32
                };
                if self
                    .rom_import_requests
                    .send((kind, PathBuf::from(value)))
                    .is_err()
                {
                    self.status = "ROM import worker is unavailable".to_owned();
                } else {
                    self.status = "Importing ROM archive…".to_owned();
                }
            }
            PromptKind::ServerAddress => match value.parse::<IpAddr>() {
                Ok(address) => self.update_server_config(|config| {
                    config.address = address;
                    Ok(())
                }),
                Err(_) => self.status = "Enter a valid IPv4 or IPv6 bind address".to_owned(),
            },
            PromptKind::ServerPort => match value.parse::<u16>() {
                Ok(port) if port > 0 => self.update_server_config(|config| {
                    config.port = port;
                    Ok(())
                }),
                _ => self.status = "Choose a port between 1 and 65535".to_owned(),
            },
            PromptKind::ServerToken => self.update_server_config(|config| {
                config.token = value.to_owned();
                Ok(())
            }),
            PromptKind::ServerUsername => self.update_server_config(|config| {
                config.credentials.username = value.to_owned();
                Ok(())
            }),
            PromptKind::ServerPassword => {
                self.update_server_config(|config| config.credentials.set_password(value))
            }
            PromptKind::ServerCertificate => {
                let path = PathBuf::from(value);
                if !path.is_file() {
                    self.status = "Choose a certificate file".to_owned();
                } else {
                    self.pending_server_certificate = Some(path);
                    self.begin_prompt(PromptKind::ServerPrivateKey, String::new());
                }
            }
            PromptKind::ServerPrivateKey => {
                if let Some(certificate) = self.pending_server_certificate.take() {
                    match kog_server::import_pem_pair(&certificate, Path::new(value)) {
                        Ok(tls) => self.update_server_config(|config| {
                            config.tls = tls;
                            Ok(())
                        }),
                        Err(error) => self.status = error,
                    }
                }
            }
            PromptKind::ServerCache => match value.parse::<u64>() {
                Ok(megabytes) if megabytes <= 102_400 => self.update_server_config(|config| {
                    config.cache_bytes = megabytes * 1024 * 1024;
                    Ok(())
                }),
                _ => self.status = "Choose a cache size from 0 to 102400 MB".to_owned(),
            },
            PromptKind::ServerDevice => {
                let mut devices = kog_server::devices::registry();
                if let Some(device) = devices.list().into_iter().find(|device| device.id == value) {
                    let blocked = !device.blocked;
                    devices.set_blocked(value, blocked);
                    self.status = format!(
                        "Device {} {}",
                        value,
                        if blocked { "blocked" } else { "allowed" }
                    );
                } else {
                    self.status = "No device with that ID".to_owned();
                }
            }
        }
    }

    fn poll_rom_imports(&mut self) {
        while let Ok(response) = self.rom_import_results.try_recv() {
            let RomImportResult { kind, result } = response;
            let (directory, count, warnings) = match result {
                Ok(result) => result,
                Err(error) => {
                    self.status = error;
                    continue;
                }
            };
            let saved = match kind {
                RomKind::Sc55 => AppSettings::save_sc55_rom_path(Some(&directory)),
                RomKind::Mt32 => AppSettings::save_mt32_rom_path(Some(&directory)),
            };
            if let Err(error) = saved {
                let _ = std::fs::remove_dir_all(&directory);
                self.status = error;
                continue;
            }
            let label = match kind {
                RomKind::Sc55 => {
                    self.decoder_settings.set_sc55_rom_path(Some(directory));
                    "SC-55"
                }
                RomKind::Mt32 => {
                    self.decoder_settings.set_mt32_rom_path(Some(directory));
                    "MT-32"
                }
            };
            self.invalidate_metadata();
            self.status = format!("Imported {label} ROM set ({count} files)");
            if !warnings.is_empty() {
                self.status
                    .push_str(&format!("; {} warning(s)", warnings.len()));
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
            self.select_track(visible[next], false, false);
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
        if pane == 1 {
            self.selected_tree.clear();
            if let Some(row) = self.items.get(self.selected[1]) {
                self.selected_tree.insert(tree_item_key(&row.item));
            }
            self.tree_anchor = Some(self.selected[1]);
        } else if pane == 2 {
            self.select_track(self.selected[2], false, false);
        }
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
                let metadata = self.metadata_for(track);
                let matches = [
                    self.title_for(track),
                    metadata.map_or(String::new(), |meta| meta.artist.clone()),
                    metadata.map_or(String::new(), |meta| meta.album.clone()),
                    track.name.clone(),
                ]
                .iter()
                .any(|value| value.to_lowercase().contains(&self.playlist_query));
                matches.then_some(index)
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

    fn show_keyboard_help(&mut self) {
        self.show_modal(
            [
                "Keyboard controls",
                "",
                "Tab / Shift+Tab: change pane; arrows, Page Up/Down, Home/End: move",
                "Enter: open folder, add saved list, or play track",
                "m: application menu; M or Shift+F10: selected item actions",
                "Alt+letter: choose the labeled item in an open menu; main menu works directly",
                "Menus: arrows, Enter, Left/Esc; Page Up/Down and Home/End also work",
                "Shift+Up/Down or v then arrows: select a range; J/K: move cursor only",
                "x or Ctrl+Space: toggle the cursor row; Ctrl+A: select all",
                "z: collapse the focused Files or Playlists section",
                "Ctrl+Left/Right or { / }: resize sidebar; Ctrl+R or u: refresh",
                "H: focus columns; Left/Right choose; Enter sort; +/- resize",
                "In column mode: Ctrl+Left/Right or [ / ] reorder; a fit; v hide; M actions",
                "/: search files; F: search playlist; o: choose music folder",
                "a: add selected files; Delete: remove or move to trash; f: star",
                "Space: play/pause; s: stop; </>: previous/next; h/l: seek 10 seconds",
                "G: seek to a time; +/-: volume; R/S: repeat/shuffle; C: compact view",
                "In compact view, C returns to the playlist; playback keys still work",
                "Prompt editing: Ctrl+A select all; Ctrl+W delete word; Ctrl+U clear",
                "Esc closes dialogs; Esc or q quits from the main view.",
            ]
            .join("\n"),
        );
    }

    fn open_keyboard_context(&mut self, size: (usize, usize)) {
        let layout = self.layout(size);
        if let Some(column) = self.keyboard_column {
            self.context_column = Some(column);
            self.open_context(MenuPage::Columns, layout.first + 2, 1, size);
            return;
        }
        match self.focus {
            Focus::Library => {
                if let Some(row) = self.items.get(self.selected[1])
                    && !self.selected_tree.contains(&tree_item_key(&row.item))
                {
                    self.select_tree_with_modifiers(self.selected[1], false, false);
                }
            }
            Focus::Playlists => {
                if let Some((id, _)) = self.lists.get(self.selected[0])
                    && !self.selected_lists.contains(id)
                {
                    self.select_list(self.selected[0]);
                }
            }
            Focus::Tracks => {
                if !self.selected_tracks.contains(&self.selected[2]) {
                    self.select_track(self.selected[2], false, false);
                }
            }
        }
        let (page, x, y) = match self.focus {
            Focus::Library => (
                MenuPage::Tree,
                1,
                layout.tree_top + self.selected[1].saturating_sub(self.offsets[1]),
            ),
            Focus::Playlists => (
                MenuPage::Saved,
                1,
                layout.list_top + self.selected[0].saturating_sub(self.offsets[0]),
            ),
            Focus::Tracks if !self.tracks.is_empty() => (
                MenuPage::Tracks,
                layout.first + 2,
                2 + self.selected[2].saturating_sub(self.offsets[2]),
            ),
            Focus::Tracks => (MenuPage::Playlist, layout.first + 2, 2),
        };
        self.open_context(page, x, y, size);
    }

    fn move_cursor_only(&mut self, delta: isize, page: usize) {
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
            self.offsets[2] = self.offsets[2].min(next);
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
        self.selected[pane] = self.selected[pane]
            .saturating_add_signed(delta)
            .min(len - 1);
        if self.selected[pane] < self.offsets[pane] {
            self.offsets[pane] = self.selected[pane];
        }
        if self.selected[pane] >= self.offsets[pane] + page {
            self.offsets[pane] = self.selected[pane] + 1 - page;
        }
    }

    fn toggle_cursor_selection(&mut self) {
        match self.focus {
            Focus::Library => {
                self.select_tree_with_modifiers(self.selected[1], false, true);
                self.status = format!("Selected {} tree items", self.selected_tree.len());
            }
            Focus::Playlists => {
                self.select_list_with_modifiers(self.selected[0], false, true);
                self.status = format!("Selected {} playlists", self.selected_lists.len());
            }
            Focus::Tracks => {
                self.select_track(self.selected[2], false, true);
                self.status = format!("Selected {} tracks", self.selected_tracks.len());
            }
        }
    }

    fn focus_keyboard_column(&mut self, size: (usize, usize)) {
        self.compact_mode = false;
        self.focus = Focus::Tracks;
        self.keyboard_column = self
            .columns
            .index("title")
            .filter(|&index| self.columns.entries[index].visible)
            .or_else(|| self.columns.positions().next().map(|(index, _, _)| index));
        self.ensure_keyboard_column_visible(size);
    }

    fn ensure_keyboard_column_visible(&mut self, size: (usize, usize)) {
        let Some(index) = self.keyboard_column else {
            return;
        };
        let viewport = size.0.saturating_sub(self.layout(size).first + 1).max(1);
        let Some((_, start, width)) = self
            .columns
            .positions()
            .find(|(current, _, _)| *current == index)
        else {
            return;
        };
        if start < self.columns.scroll || width >= viewport {
            self.columns.scroll = start;
        } else if start + width > self.columns.scroll + viewport {
            self.columns.scroll = start + width - viewport;
        }
        self.columns.scroll_by(0, viewport);
    }

    fn move_keyboard_column(&mut self, delta: isize, size: (usize, usize)) {
        let visible: Vec<_> = self
            .columns
            .positions()
            .map(|(index, _, _)| index)
            .collect();
        let Some(current) = self
            .keyboard_column
            .and_then(|index| visible.iter().position(|&other| other == index))
        else {
            return;
        };
        let next = current.saturating_add_signed(delta).min(visible.len() - 1);
        self.keyboard_column = Some(visible[next]);
        self.ensure_keyboard_column_visible(size);
    }

    fn key(&mut self, key: Key, size: (usize, usize)) -> bool {
        if self.modal.is_some() {
            let max = self
                .modal
                .as_ref()
                .map(|text| {
                    modal_lines(text, size.0.saturating_sub(12))
                        .len()
                        .saturating_sub(size.1.saturating_sub(8))
                })
                .unwrap_or(0);
            match key {
                Key::Esc | Key::Enter | Key::Char(' ') => {
                    self.modal = None;
                    self.info_modal = false;
                    self.artwork_modal = false;
                    self.visualizer_open = false;
                }
                Key::Up | Key::Char('k') => self.modal_scroll = self.modal_scroll.saturating_sub(1),
                Key::Down | Key::Char('j') => self.modal_scroll = (self.modal_scroll + 1).min(max),
                Key::PageUp => {
                    self.modal_scroll = self.modal_scroll.saturating_sub(size.1.saturating_sub(8))
                }
                Key::PageDown => {
                    self.modal_scroll = (self.modal_scroll + size.1.saturating_sub(8)).min(max)
                }
                Key::Home => self.modal_scroll = 0,
                Key::End => self.modal_scroll = max,
                _ => {}
            }
            return true;
        }
        if let Some(mut chooser) = self.folder_chooser.take() {
            let rows = FolderGeometry::new(size).list_rows();
            let mut accept = false;
            let mut cancel = false;
            match key {
                Key::Esc => cancel = true,
                Key::CtrlO => accept = true,
                Key::CtrlL => {
                    chooser.focus = FolderFocus::Location;
                    chooser.select_all = true;
                }
                Key::Tab | Key::BackTab => {
                    chooser.select_all = false;
                    chooser.focus = match (chooser.focus, key) {
                        (FolderFocus::List, Key::Tab) | (FolderFocus::Choose, Key::BackTab) => {
                            FolderFocus::Location
                        }
                        (FolderFocus::Location, Key::Tab) | (FolderFocus::Cancel, Key::BackTab) => {
                            FolderFocus::Choose
                        }
                        (FolderFocus::Choose, Key::Tab) | (FolderFocus::List, Key::BackTab) => {
                            FolderFocus::Cancel
                        }
                        _ => FolderFocus::List,
                    };
                }
                Key::Enter => match chooser.focus {
                    FolderFocus::List => chooser.open_selected(),
                    FolderFocus::Location => chooser.navigate_location(),
                    FolderFocus::Choose => accept = true,
                    FolderFocus::Cancel => cancel = true,
                },
                Key::AltUp => chooser.parent(),
                Key::Char('.') if chooser.focus == FolderFocus::List => chooser.toggle_hidden(),
                Key::Char('/') if chooser.focus == FolderFocus::List => {
                    chooser.focus = FolderFocus::Location;
                    chooser.location = "/".to_owned();
                    chooser.cursor = 1;
                    chooser.select_all = false;
                }
                _ if chooser.focus == FolderFocus::Location => chooser.edit_location(key),
                Key::Backspace => chooser.parent(),
                Key::Up | Key::Down | Key::PageUp | Key::PageDown | Key::Home | Key::End => {
                    let last = chooser.entries.len().saturating_sub(1);
                    chooser.selected = match key {
                        Key::Up => chooser.selected.saturating_sub(1),
                        Key::Down => (chooser.selected + 1).min(last),
                        Key::PageUp => chooser.selected.saturating_sub(rows),
                        Key::PageDown => (chooser.selected + rows).min(last),
                        Key::Home => 0,
                        _ => last,
                    };
                    chooser.keep_selected_visible(rows);
                }
                _ => {}
            }
            if !cancel {
                self.folder_chooser = Some(chooser);
                if accept {
                    self.accept_folder_chooser();
                }
            }
            return true;
        }
        if let Some((kind, mut value)) = self.prompt.take() {
            let mut edited = false;
            self.input_cursor = self.input_cursor.min(value.chars().count());
            match key {
                Key::Esc => {
                    if kind == PromptKind::Search {
                        self.search_due = None;
                        if self.remote_active {
                            self.connect_remote(Some(self.remote_path.clone()));
                        } else {
                            self.browse(None);
                        }
                    } else if kind == PromptKind::PlaylistSearch {
                        self.playlist_query.clear();
                    } else if kind == PromptKind::TrashSelected {
                        self.pending_delete_paths.clear();
                    } else if matches!(kind, PromptKind::TagField(_) | PromptKind::TagArtwork) {
                        self.open_submenu(MenuPage::TagEditor);
                    }
                    self.status.clear();
                    return true;
                }
                Key::Enter => {
                    self.finish_prompt(kind, value);
                    return true;
                }
                Key::CtrlC => return false,
                Key::CtrlA => self.input_select_all = true,
                Key::CtrlU => {
                    value.clear();
                    self.input_cursor = 0;
                    self.input_select_all = false;
                    edited = true;
                }
                Key::CtrlW | Key::CtrlBackspace => {
                    if self.input_select_all {
                        value.clear();
                        self.input_cursor = 0;
                    } else {
                        let start = previous_path_word_boundary(&value, self.input_cursor);
                        value.drain(
                            byte_offset(&value, start)..byte_offset(&value, self.input_cursor),
                        );
                        self.input_cursor = start;
                    }
                    self.input_select_all = false;
                    edited = true;
                }
                Key::CtrlLeft => {
                    self.input_select_all = false;
                    self.input_cursor = previous_path_word_boundary(&value, self.input_cursor);
                }
                Key::CtrlRight => {
                    self.input_select_all = false;
                    self.input_cursor = next_path_word_boundary(&value, self.input_cursor);
                }
                Key::Left => {
                    self.input_select_all = false;
                    self.input_cursor = self.input_cursor.saturating_sub(1);
                }
                Key::Right => {
                    self.input_select_all = false;
                    self.input_cursor = (self.input_cursor + 1).min(value.chars().count())
                }
                Key::Home => {
                    self.input_select_all = false;
                    self.input_cursor = 0;
                }
                Key::End => {
                    self.input_select_all = false;
                    self.input_cursor = value.chars().count();
                }
                Key::Backspace => {
                    if self.input_select_all {
                        value.clear();
                        self.input_cursor = 0;
                        self.input_select_all = false;
                        edited = true;
                    } else if self.input_cursor > 0 {
                        self.input_cursor -= 1;
                        value.remove(byte_offset(&value, self.input_cursor));
                        edited = true;
                    }
                }
                Key::Delete => {
                    if self.input_select_all {
                        value.clear();
                        self.input_cursor = 0;
                        self.input_select_all = false;
                        edited = true;
                    } else if self.input_cursor < value.chars().count() {
                        value.remove(byte_offset(&value, self.input_cursor));
                        edited = true;
                    }
                }
                Key::Char(c)
                    if !c.is_control() && (value.len() < 1024 || self.input_select_all) =>
                {
                    if self.input_select_all {
                        value.clear();
                        self.input_cursor = 0;
                        self.input_select_all = false;
                    }
                    value.insert(byte_offset(&value, self.input_cursor), c);
                    self.input_cursor += 1;
                    edited = true;
                }
                _ => {}
            }
            if edited {
                self.update_search_draft(kind, &value);
            }
            self.prompt = Some((kind, value));
            return true;
        }
        if self.menu_open {
            match key {
                Key::Alt(character) => {
                    self.activate_menu_shortcut(character, size);
                }
                Key::Esc | Key::Left => {
                    if let Some(parent) = self.menu_parents.pop() {
                        self.restore_menu_layer(parent);
                    } else if self.menu_page == MenuPage::TagEditor {
                        self.tag_session = None;
                        self.menu_open = false;
                        self.status = "Tag edits cancelled".to_owned();
                    } else {
                        self.menu_open = false;
                    }
                }
                Key::Char('m') => {
                    self.menu_open = false;
                    self.menu_parents.clear();
                }
                Key::Up | Key::Char('k') => self.move_menu_selection(-1, size.1),
                Key::Down | Key::Char('j') => self.move_menu_selection(1, size.1),
                Key::PageUp | Key::PageDown => {
                    let page = self
                        .menu_page
                        .labels()
                        .len()
                        .min(size.1.saturating_sub(4))
                        .max(1);
                    let delta = if key == Key::PageUp { -1 } else { 1 };
                    for _ in 0..page {
                        self.move_menu_selection(delta, size.1);
                    }
                }
                Key::Home => {
                    self.menu_selected = self
                        .menu_page
                        .labels()
                        .iter()
                        .position(|label| !label.is_empty())
                        .unwrap_or(0);
                    self.menu_offset = 0;
                }
                Key::End => {
                    self.menu_selected = self
                        .menu_page
                        .labels()
                        .iter()
                        .rposition(|label| !label.is_empty())
                        .unwrap_or(0);
                    self.menu_offset = self.menu_selected.saturating_sub(size.1.saturating_sub(5));
                }
                Key::Enter | Key::Right => self.activate_menu(self.menu_selected, size),
                _ => {}
            }
            return true;
        }
        if let Some(column) = self.keyboard_column {
            match key {
                Key::Esc => return false,
                Key::Char('H') => self.keyboard_column = None,
                Key::Left | Key::Right => {
                    self.move_keyboard_column(if key == Key::Left { -1 } else { 1 }, size)
                }
                Key::Home | Key::End => {
                    let visible: Vec<_> = self
                        .columns
                        .positions()
                        .map(|(index, _, _)| index)
                        .collect();
                    self.keyboard_column = if key == Key::Home {
                        visible.first()
                    } else {
                        visible.last()
                    }
                    .copied();
                    self.ensure_keyboard_column_visible(size);
                }
                Key::CtrlLeft | Key::CtrlRight | Key::Char('[') | Key::Char(']') => {
                    let delta = if matches!(key, Key::CtrlLeft | Key::Char('[')) {
                        -1
                    } else {
                        1
                    };
                    if let Some(target) = self.columns.move_by(column, delta) {
                        self.keyboard_column = Some(target);
                        self.context_column = Some(target);
                        self.persist_columns();
                        self.ensure_keyboard_column_visible(size);
                        self.status =
                            format!("Moved {} column", self.columns.entries[target].label);
                    }
                }
                Key::Char('+') | Key::Char('=') | Key::Char('-') => {
                    let width = &mut self.columns.entries[column].width;
                    *width = width
                        .saturating_add_signed(if key == Key::Char('-') { -1 } else { 1 })
                        .clamp(3, 160);
                    self.persist_columns();
                    self.ensure_keyboard_column_visible(size);
                    self.status = format!(
                        "{} column: {} cells",
                        self.columns.entries[column].label, self.columns.entries[column].width
                    );
                }
                Key::Enter => self.sort_tracks_by_column(column),
                Key::Char('a') => self.auto_fit_columns(),
                Key::Char('v') => {
                    if self.columns.toggle(column) {
                        self.persist_columns();
                        self.keyboard_column =
                            self.columns.positions().next().map(|(index, _, _)| index);
                        self.ensure_keyboard_column_visible(size);
                    }
                }
                Key::Char('M') | Key::ContextMenu => self.open_keyboard_context(size),
                Key::Char('?') => self.show_keyboard_help(),
                Key::Tab | Key::BackTab => {
                    self.keyboard_column = None;
                    return self.key(key, size);
                }
                _ => {
                    self.keyboard_column = None;
                    return self.key(key, size);
                }
            }
            return true;
        }
        let layout = self.layout(size);
        let page = match self.focus {
            Focus::Playlists => layout.list_page,
            Focus::Library => self.tree_page(&layout),
            Focus::Tracks => layout.track_page,
        }
        .max(1);
        match key {
            Key::Esc | Key::Char('q') | Key::CtrlC => return false,
            Key::Alt(character) => {
                if menu_shortcut_index(MenuPage::Main, character).is_some() {
                    self.open_submenu(MenuPage::Main);
                    self.activate_menu_shortcut(character, size);
                }
            }
            Key::Char('?') => self.show_keyboard_help(),
            Key::Char('M') | Key::ContextMenu => self.open_keyboard_context(size),
            Key::F10 | Key::Char('m') => self.open_submenu(MenuPage::Main),
            Key::Char('H') => self.focus_keyboard_column(size),
            Key::Char('C') => {
                self.compact_mode = !self.compact_mode;
                self.status = if self.compact_mode {
                    "Compact player · C returns to playlist"
                } else {
                    "Playlist view"
                }
                .to_owned();
            }
            Key::Char('G') => self.begin_seek_prompt(),
            Key::CtrlR | Key::Char('u') if self.focus == Focus::Library => {
                if self.remote_active {
                    self.connect_remote(Some(self.remote_path.clone()));
                } else {
                    self.browse(None);
                }
            }
            Key::CtrlR | Key::Char('u') if self.focus == Focus::Playlists => self.reload_lists(),
            Key::CtrlLeft | Key::CtrlRight | Key::Char('{') | Key::Char('}')
                if layout.show_sidebar && !self.compact_mode =>
            {
                let current = self.sidebar_width.unwrap_or(layout.first);
                let next = current.saturating_add_signed(
                    if matches!(key, Key::CtrlLeft | Key::Char('{')) {
                        -1
                    } else {
                        1
                    },
                );
                let width = next.clamp(18, size.0.saturating_sub(30).max(18));
                self.sidebar_width = Some(width);
                self.status = format!("Sidebar width: {width} cells");
            }
            Key::Char('z') if self.focus == Focus::Library => {
                self.files_expanded = !self.files_expanded
            }
            Key::Char('z') if self.focus == Focus::Playlists => {
                self.playlists_expanded = !self.playlists_expanded
            }
            Key::Char('x') | Key::CtrlSpace => self.toggle_cursor_selection(),
            Key::AltUp | Key::Char('K') => self.move_cursor_only(-1, page),
            Key::AltDown | Key::Char('J') => self.move_cursor_only(1, page),
            Key::Char('/') => self.begin_prompt(PromptKind::Search, self.search_query.clone()),
            Key::Char('F') => {
                self.begin_prompt(PromptKind::PlaylistSearch, self.playlist_query.clone())
            }
            Key::Char('c') => self.show_queue(),
            Key::Char('v') => self.toggle_range_click(),
            Key::Char('t') => {
                self.sidebar_visible = !self.sidebar_visible;
                if !self.sidebar_visible {
                    self.focus = Focus::Tracks;
                }
            }
            Key::Tab => {
                self.range_click_pending = None;
                self.focus = match self.focus {
                    Focus::Playlists => Focus::Library,
                    Focus::Library => Focus::Tracks,
                    Focus::Tracks => Focus::Playlists,
                }
            }
            Key::BackTab => {
                self.range_click_pending = None;
                self.focus = match self.focus {
                    Focus::Playlists => Focus::Tracks,
                    Focus::Library => Focus::Playlists,
                    Focus::Tracks => Focus::Library,
                }
            }
            Key::Up | Key::Down | Key::Char('k') | Key::Char('j')
                if self.range_click_pending == Some(self.focus) =>
            {
                self.extend_keyboard_range(
                    if matches!(key, Key::Up | Key::Char('k')) {
                        -1
                    } else {
                        1
                    },
                    page,
                );
                self.status = match self.focus {
                    Focus::Library => format!("Selected {} tree items", self.selected_tree.len()),
                    Focus::Playlists => format!("Selected {} playlists", self.selected_lists.len()),
                    Focus::Tracks => format!("Selected {} tracks", self.selected_tracks.len()),
                };
            }
            Key::Up | Key::Char('k') if self.focus == Focus::Playlists => {
                self.move_selection(-1, page);
                self.select_list(self.selected[0]);
            }
            Key::Down | Key::Char('j') if self.focus == Focus::Playlists => {
                self.move_selection(1, page);
                self.select_list(self.selected[0]);
            }
            Key::Up | Key::Char('k') => self.move_selection(-1, page),
            Key::Down | Key::Char('j') => self.move_selection(1, page),
            Key::ShiftUp | Key::ShiftDown => {
                self.extend_keyboard_range(if key == Key::ShiftUp { -1 } else { 1 }, page);
            }
            Key::CtrlUp | Key::CtrlDown if self.focus == Focus::Tracks => {
                let from = self.selected[2];
                let to = from
                    .saturating_add_signed(if key == Key::CtrlUp { -1 } else { 1 })
                    .min(self.tracks.len().saturating_sub(1));
                self.move_track(from, to);
            }
            Key::CtrlA if self.focus == Focus::Tracks => {
                self.selected_tracks = self.visible_tracks().into_iter().collect();
                self.status = format!("Selected {} tracks", self.selected_tracks.len());
            }
            Key::CtrlA if self.focus == Focus::Playlists => {
                self.selected_lists = self.lists.iter().map(|(id, _)| *id).collect();
                self.status = format!("Selected {} playlists", self.selected_lists.len());
            }
            Key::CtrlA if self.focus == Focus::Library => {
                self.selected_tree = self
                    .items
                    .iter()
                    .map(|row| tree_item_key(&row.item))
                    .collect();
                self.tree_anchor = Some(self.selected[1]);
                self.status = format!("Selected {} tree items", self.selected_tree.len());
            }
            Key::PageUp => self.move_selection(-(page as isize), page),
            Key::PageDown => self.move_selection(page as isize, page),
            Key::Home => self.move_selection(-(isize::MAX / 2), page),
            Key::End => self.move_selection(isize::MAX / 2, page),
            Key::Left | Key::Backspace if self.focus == Focus::Library => self.up_directory(),
            Key::Right | Key::Enter if self.focus == Focus::Library => self.open_selected(),
            Key::Left | Key::Right if self.focus == Focus::Tracks && !self.compact_mode => {
                self.columns.scroll_by(
                    if key == Key::Left { -8 } else { 8 },
                    size.0.saturating_sub(layout.first + 1),
                );
            }
            Key::Enter if self.focus == Focus::Playlists => self.enqueue_list(self.selected[0]),
            Key::Enter if self.focus == Focus::Tracks => self.play_selected(),
            Key::Char('a') if self.focus == Focus::Library => self.add_selected(false),
            Key::Char('n') if self.focus == Focus::Playlists => {
                self.begin_prompt(PromptKind::NewPlaylist, String::new())
            }
            Key::Char('r')
                if self.focus == Focus::Playlists
                    && self
                        .lists
                        .get(self.selected[0])
                        .is_some_and(|(id, _)| *id > 0) =>
            {
                let name = self
                    .lists
                    .get(self.selected[0])
                    .map(|(_, name)| name.clone())
                    .unwrap_or_default();
                self.begin_prompt(PromptKind::RenamePlaylist, name);
            }
            Key::Delete
                if self.focus == Focus::Playlists
                    && self
                        .selected_list_indices()
                        .iter()
                        .any(|&index| self.lists[index].0 > 0) =>
            {
                self.begin_prompt(PromptKind::DeletePlaylist, String::new());
            }
            Key::Char('o') => self.begin_prompt(
                PromptKind::MusicFolder,
                self.library
                    .root()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            ),
            Key::Delete if self.focus == Focus::Tracks => self.remove_selected(),
            Key::Delete if self.focus == Focus::Library => self.begin_trash_selected(),
            Key::Char('f') => self.toggle_star(),
            Key::Char('e') if self.focus == Focus::Tracks => self.open_tag_editor(),
            Key::Char(' ') => self.play_pause(),
            Key::Char('s') => {
                self.player.stop();
                self.playing = None;
            }
            Key::Char('>') => self.next(false),
            Key::Char('<') => self.previous(),
            Key::Char('R') => self.cycle_repeat(),
            Key::Char('S') => self.cycle_shuffle(),
            Key::Char('[') if self.focus == Focus::Tracks => {
                self.columns
                    .scroll_by(-12, size.0.saturating_sub(layout.first + 1));
            }
            Key::Char(']') if self.focus == Focus::Tracks => {
                self.columns
                    .scroll_by(12, size.0.saturating_sub(layout.first + 1));
            }
            Key::Char('Q') if self.focus == Focus::Tracks => self.toggle_selected_queue(),
            Key::Char('X') if self.focus == Focus::Tracks => self.toggle_selected_stop_after(),
            Key::Char('+') | Key::Char('=') => {
                self.volume = (self.volume + 0.05).min(1.0);
                self.player.set_volume(self.volume);
            }
            Key::Char('-') => {
                self.volume = (self.volume - 0.05).max(0.0);
                self.player.set_volume(self.volume);
            }
            Key::Char('h') if self.focus == Focus::Tracks || self.compact_mode => {
                let _ = self.player.seek(
                    self.player
                        .position()
                        .saturating_sub(Duration::from_secs(10)),
                );
            }
            Key::Char('l') if self.focus == Focus::Tracks || self.compact_mode => {
                let _ = self
                    .player
                    .seek(self.player.position() + Duration::from_secs(10));
            }
            _ => {}
        }
        true
    }

    fn mouse(&mut self, button: u16, x: usize, y: usize, release: bool, size: (usize, usize)) {
        if self.folder_chooser.is_some() {
            if !release {
                self.folder_mouse(button, x, y, size);
            }
            return;
        }
        if release {
            if self.column_drag.is_some() {
                self.persist_columns();
            }
            self.split_drag = false;
            self.column_drag = None;
            self.column_scroll_drag = None;
            self.vertical_scroll_drag = None;
            self.volume_drag = false;
            self.track_drag = None;
            return;
        }
        let layout = self.layout(size);
        if self.modal.is_some() {
            if (button & 0b1100_0000) == 64 {
                let max = self
                    .modal
                    .as_ref()
                    .map(|text| {
                        modal_lines(text, size.0.saturating_sub(12))
                            .len()
                            .saturating_sub(size.1.saturating_sub(8))
                    })
                    .unwrap_or(0);
                self.modal_scroll = self
                    .modal_scroll
                    .saturating_add_signed(if button & 1 == 0 { -3 } else { 3 })
                    .min(max);
            } else if button & 32 == 0 {
                self.modal = None;
                self.info_modal = false;
                self.artwork_modal = false;
                self.visualizer_open = false;
            }
            return;
        }
        if self.split_drag && button & 32 != 0 {
            self.sidebar_width = Some(x.clamp(18, size.0.saturating_sub(30).max(18)));
            return;
        }
        if let Some(grip) = self.column_scroll_drag
            && button & 32 != 0
        {
            if let Some(bar) = self.scrollbar(&layout, size) {
                self.scroll_columns_to_mouse(bar, x, grip);
            }
            return;
        }
        if let Some((pane, grip)) = self.vertical_scroll_drag
            && button & 32 != 0
        {
            let bar = if pane == 1 {
                self.tree_scrollbar(&layout, size)
            } else {
                self.track_scrollbar(&layout, size)
            };
            if let Some(bar) = bar {
                self.scroll_vertical_to_mouse(pane, bar, y, grip);
            }
            return;
        }
        if let Some(column) = self.column_drag
            && button & 32 != 0
        {
            let start = self
                .columns
                .positions()
                .find(|(index, ..)| *index == column)
                .map(|(_, start, _)| start);
            if let Some(start) = start {
                let relative = x.saturating_sub(layout.first + 1) + self.columns.scroll;
                self.columns.entries[column].width = relative.saturating_sub(start).clamp(3, 160);
            }
            return;
        }
        if self.volume_drag && button & 32 != 0 {
            self.set_volume_from_bar(x, size);
            return;
        }
        if let Some(from) = self.track_drag
            && button & 32 != 0
        {
            let right_edge = size.0.saturating_sub(usize::from(
                self.track_scrollbar(&layout, size).is_some(),
            ));
            if x > layout.first && x < right_edge && y >= 2 && y < 2 + layout.track_page {
                let visible = self.visible_tracks();
                if let Some(&to) = visible.get(self.offsets[2] + y - 2) {
                    self.move_track(from, to);
                    self.track_drag = Some(to);
                }
            }
            return;
        }
        if (button & 0b1100_0000) == 64 {
            let wheel = button & 3;
            if x > layout.first
                && self.scrollbar(&layout, size).is_some()
                && (wheel >= 2 || button & (4 | 16) != 0)
            {
                self.columns.scroll_by(
                    if wheel == 0 || wheel == 2 { -8 } else { 8 },
                    size.0.saturating_sub(layout.first + 1),
                );
                return;
            }
            self.focus = if !layout.show_sidebar {
                self.focus
            } else if x < layout.first {
                if y >= layout.lists_header {
                    Focus::Playlists
                } else {
                    Focus::Library
                }
            } else {
                Focus::Tracks
            };
            let page = match self.focus {
                Focus::Library => self.tree_page(&layout),
                Focus::Playlists => layout.list_page,
                Focus::Tracks => layout.track_page,
            };
            self.move_selection(if button & 1 == 0 { -3 } else { 3 }, page);
            return;
        }
        if button & 32 != 0 {
            return;
        }
        let range_pending = if button & 3 == 0 {
            self.range_click_pending.take()
        } else {
            None
        };
        if range_pending.is_some() {
            self.status = "Range selection cancelled".to_owned();
        }
        if self.menu_open && y > 0 {
            let mut layers = self.menu_parents.clone();
            layers.push(self.active_menu_layer());
            for depth in (0..layers.len()).rev() {
                let layer = layers[depth];
                let width = menu_width(size.0).min(size.0.saturating_sub(layer.x));
                let page = layer.page.labels().len().min(size.1.saturating_sub(4));
                let inside = (layer.x..layer.x + width).contains(&x)
                    && (layer.y..=layer.y + page + 1).contains(&y);
                if !inside {
                    continue;
                }
                if depth < self.menu_parents.len() {
                    self.menu_parents.truncate(depth);
                    self.restore_menu_layer(layer);
                }
                if button & 3 == 0 && (layer.y + 1..=layer.y + page).contains(&y) {
                    let index = layer.offset + y - layer.y - 1;
                    if layer
                        .page
                        .labels()
                        .get(index)
                        .is_some_and(|label| !label.is_empty())
                    {
                        self.menu_selected = index;
                        self.activate_menu(index, size);
                    }
                }
                return;
            }
            self.menu_open = false;
            self.menu_parents.clear();
            if button & 3 != 2 {
                return;
            }
        }
        if self.menu_open && y == 0 && !(4..8).contains(&x) {
            self.menu_open = false;
            self.menu_parents.clear();
        }
        if let Some((kind, value)) = self.prompt.as_ref() {
            if matches!(kind, PromptKind::Search | PromptKind::PlaylistSearch) {
                let search_x = (size.0 / 2).saturating_sub(17).max(14);
                let search_width = size.0.saturating_sub(search_x + 7).min(36);
                let (row, col, width) = if *kind == PromptKind::Search && layout.show_sidebar {
                    (3, 0, layout.first)
                } else {
                    (0, search_x - 1, search_width)
                };
                if y == row && (col..col + width).contains(&x) {
                    self.input_cursor = (x.saturating_sub(col + 4)).min(value.chars().count());
                    self.input_select_all = false;
                    return;
                }
                let (kind, value) = self.prompt.take().unwrap();
                self.finish_prompt(kind, value);
            }
        }
        if button & 3 == 2 {
            if self.vertical_scrollbar_at(&layout, size, x, y).is_some() {
                return;
            }
            if layout.show_sidebar
                && x < layout.first
                && y >= layout.tree_top
                && y <= layout.tree_bottom
            {
                self.focus = Focus::Library;
                let index = self.offsets[1] + y - layout.tree_top;
                if index < self.items.len() {
                    if !self
                        .selected_tree
                        .contains(&tree_item_key(&self.items[index].item))
                    {
                        self.select_tree_with_modifiers(index, false, false);
                    } else {
                        self.selected[1] = index;
                    }
                    self.open_context(MenuPage::Tree, x, y, size);
                }
            } else if layout.show_sidebar
                && x < layout.first
                && y >= layout.list_top
                && y < layout.footer_top
            {
                self.focus = Focus::Playlists;
                let index = self.offsets[0] + y - layout.list_top;
                if index < self.lists.len() {
                    if !self.selected_lists.contains(&self.lists[index].0) {
                        self.select_list(index);
                    }
                    self.selected[0] = index;
                    self.open_context(MenuPage::Saved, x, y, size);
                }
            } else if !layout.show_sidebar
                && self.focus == Focus::Library
                && y >= 2
                && y < layout.footer_top
            {
                let index = self.offsets[1] + y - 2;
                if index < self.items.len() {
                    if !self
                        .selected_tree
                        .contains(&tree_item_key(&self.items[index].item))
                    {
                        self.select_tree_with_modifiers(index, false, false);
                    } else {
                        self.selected[1] = index;
                    }
                    self.open_context(MenuPage::Tree, x, y, size);
                }
            } else if !layout.show_sidebar
                && self.focus == Focus::Playlists
                && y >= 2
                && y < layout.footer_top
            {
                let index = self.offsets[0] + y - 2;
                if index < self.lists.len() {
                    if !self.selected_lists.contains(&self.lists[index].0) {
                        self.select_list(index);
                    }
                    self.selected[0] = index;
                    self.open_context(MenuPage::Saved, x, y, size);
                }
            } else if (layout.show_sidebar || self.focus == Focus::Tracks)
                && x > layout.first
                && y == 1
            {
                let relative = x.saturating_sub(layout.first + 1) + self.columns.scroll;
                self.context_column = self
                    .columns
                    .positions()
                    .find(|(_, start, width)| (*start..start + width).contains(&relative))
                    .map(|(index, _, _)| index);
                self.open_context(MenuPage::Columns, x, y, size);
            } else if (layout.show_sidebar || self.focus == Focus::Tracks)
                && x > layout.first
                && y >= 2
                && y < layout.footer_top
            {
                self.focus = Focus::Tracks;
                if self.scrollbar(&layout, size).is_some() && y == layout.footer_top - 1 {
                    self.open_context(MenuPage::Playlist, x, y, size);
                    return;
                }
                if let Some(&index) = self.visible_tracks().get(self.offsets[2] + y - 2) {
                    if !self.selected_tracks.contains(&index) {
                        self.select_track(index, false, false);
                    }
                    self.open_context(MenuPage::Tracks, x, y, size);
                } else {
                    self.open_context(MenuPage::Playlist, x, y, size);
                }
            }
            return;
        }
        if button & 3 != 0 {
            return;
        }
        if layout.show_sidebar && x == layout.first && y > 0 && y < layout.footer_top {
            self.split_drag = true;
            return;
        }
        if let Some((pane, bar)) = self.vertical_scrollbar_at(&layout, size, x, y) {
            self.focus = if pane == 1 { Focus::Library } else { Focus::Tracks };
            self.manual_scroll_selection[pane] = Some(self.selected[pane]);
            if y == bar.top {
                self.offsets[pane] = self.offsets[pane].saturating_sub(1);
            } else if y == bar.top + bar.height - 1 {
                self.offsets[pane] = (self.offsets[pane] + 1).min(bar.max_scroll);
            } else if bar.travel == 0 {
                return;
            } else {
                let position = y - bar.top - 1;
                let grip =
                    if (bar.thumb_start..bar.thumb_start + bar.thumb_height).contains(&position) {
                        position - bar.thumb_start
                    } else {
                        bar.thumb_height / 2
                    };
                self.vertical_scroll_drag = Some((pane, grip));
                self.scroll_vertical_to_mouse(pane, bar, y, grip);
            }
            return;
        }
        if let Some(bar) = self.scrollbar(&layout, size)
            && y == layout.footer_top - 1
            && (bar.x..bar.x + bar.width).contains(&x)
        {
            if x == bar.x {
                self.columns.scroll_by(-12, bar.width);
            } else if x == bar.x + bar.width - 1 {
                self.columns.scroll_by(12, bar.width);
            } else {
                let position = x - bar.x - 1;
                let grip =
                    if (bar.thumb_start..bar.thumb_start + bar.thumb_width).contains(&position) {
                        position - bar.thumb_start
                    } else {
                        bar.thumb_width / 2
                    };
                self.column_scroll_drag = Some(grip);
                self.scroll_columns_to_mouse(bar, x, grip);
            }
            return;
        }
        if self.scrollbar(&layout, size).is_some()
            && self.track_scrollbar(&layout, size).is_some()
            && y == layout.footer_top - 1
            && x == size.0.saturating_sub(1)
        {
            return;
        }
        if y == 0 {
            if x < 4 {
                self.begin_prompt(
                    PromptKind::MusicFolder,
                    self.library
                        .root()
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                );
            } else if x < 8 {
                if self.menu_open {
                    self.menu_open = false;
                } else {
                    self.open_submenu(MenuPage::Main);
                }
            } else if x < 12 {
                self.sidebar_visible = !self.sidebar_visible;
                if !self.sidebar_visible {
                    self.focus = Focus::Tracks;
                }
            } else if x >= size.0.saturating_sub(8) && x < size.0.saturating_sub(4) {
                self.show_keyboard_help();
            } else if x >= size.0.saturating_sub(4) {
                self.exit_requested = true;
            } else if self.compact_mode {
                self.compact_mode = false;
                self.status = "Playlist view".to_owned();
            } else {
                if !layout.show_sidebar && self.focus == Focus::Library {
                    self.begin_prompt(PromptKind::Search, self.search_query.clone());
                } else {
                    self.begin_prompt(PromptKind::PlaylistSearch, self.playlist_query.clone());
                }
            }
            return;
        }
        if y >= layout.footer_top {
            if (layout.footer_top..layout.footer_top + 4).contains(&y)
                && x < 10
                && size.0 >= 80
                && self.cover_preview.is_some()
            {
                self.show_artwork();
                return;
            }
            let (volume_row, icon_x, bar_x, bar_width) = volume_geometry(size.0, layout.footer_top);
            if y == volume_row && (icon_x..bar_x + bar_width).contains(&x) {
                if x < bar_x {
                    self.toggle_mute();
                } else {
                    self.set_volume_from_bar(x, size);
                    self.volume_drag = true;
                }
                return;
            }
            if y == layout.footer_top {
                if let Some(action) = footer_transport_at(size.0, x) {
                    self.activate_transport(action);
                }
            } else if y == layout.footer_top + 1 {
                let duration = self
                    .playing
                    .and_then(|index| self.tracks.get(index))
                    .and_then(|track| self.metadata_for(track))
                    .and_then(|meta| meta.duration);
                if let Some(duration) = duration.filter(|time| !time.is_zero()) {
                    let clock = self.player.position();
                    let clock_label =
                        format!("{}:{:02}", clock.as_secs() / 60, clock.as_secs() % 60);
                    let (_, bar_left, bar_width) = progress_bar_geometry(size.0, &clock_label);
                    if (bar_left..bar_left + bar_width).contains(&x) {
                        let fraction = (x - bar_left) as f64 / (bar_width - 1) as f64;
                        let _ = self.player.seek(duration.mul_f64(fraction));
                    }
                }
            }
            return;
        }
        if self.compact_mode {
            let card_width = size.0.saturating_sub(4).min(72);
            let card_x = size.0.saturating_sub(card_width) / 2;
            let rich = card_width >= 68 && layout.footer_top >= 18;
            let card_y = if rich {
                layout.footer_top.saturating_sub(11) / 2
            } else {
                layout.footer_top.saturating_div(2).saturating_sub(1).max(3)
            };
            if rich {
                if (card_y + 1..card_y + 8).contains(&y) && (card_x + 2..card_x + 15).contains(&x) {
                    self.show_artwork();
                    return;
                }
                let position = self.player.position();
                let clock_width =
                    format!("{}:{:02}", position.as_secs() / 60, position.as_secs() % 60)
                        .len()
                        .max(6);
                let seek_left = card_x + 17 + clock_width + 2;
                if y == card_y + 5 && (seek_left..seek_left + 26).contains(&x) {
                    let duration = self
                        .playing
                        .and_then(|index| self.tracks.get(index))
                        .and_then(|track| self.metadata_for(track))
                        .and_then(|meta| meta.duration);
                    if let Some(duration) = duration.filter(|time| !time.is_zero()) {
                        let fraction = (x - seek_left) as f64 / 25.0;
                        let _ = self.player.seek(duration.mul_f64(fraction));
                    }
                } else if y == card_y + 7 && x >= card_x + 17 {
                    let transport = compact_transport(
                        card_width - 21,
                        true,
                        self.player.state() == PlaybackState::Playing,
                    );
                    if let Some(action) = transport.action_at(x - card_x - 17) {
                        self.activate_transport(action);
                    }
                } else if y == card_y + 8 && (card_x + 19..card_x + 39).contains(&x) {
                    self.volume = (x - card_x - 19) as f32 / 19.0;
                    self.volume_before_mute = self.volume.max(0.05);
                    self.player.set_volume(self.volume);
                }
            } else if y == card_y + 3 && x >= card_x {
                let transport = compact_transport(
                    card_width,
                    false,
                    self.player.state() == PlaybackState::Playing,
                );
                if let Some(action) = transport.action_at(x - card_x) {
                    self.activate_transport(action);
                }
            }
            return;
        }
        if layout.show_sidebar && x < layout.first {
            if y == 1 {
                self.files_expanded = !self.files_expanded;
                return;
            }
            if y == 2 && self.files_expanded {
                if x < 4 {
                    if self.remote_active {
                        self.open_submenu(MenuPage::Remote);
                    } else {
                        self.begin_prompt(
                            PromptKind::MusicFolder,
                            self.library
                                .root()
                                .map(|p| p.to_string_lossy().into_owned())
                                .unwrap_or_default(),
                        );
                    }
                } else if x < 8 {
                    if self.remote_active {
                        self.connect_remote(Some(self.remote_path.clone()));
                    } else {
                        self.browse(None);
                    }
                }
                return;
            }
            if y == 3 && self.files_expanded {
                self.begin_prompt(PromptKind::Search, self.search_query.clone());
                return;
            }
            if y == layout.lists_header {
                if x >= layout.first.saturating_sub(4) {
                    self.begin_prompt(PromptKind::NewPlaylist, String::new());
                } else {
                    self.playlists_expanded = !self.playlists_expanded;
                    if self.playlists_expanded {
                        self.focus = Focus::Playlists;
                    }
                }
                return;
            }
            if y >= layout.list_top {
                self.focus = Focus::Playlists;
                let index = self.offsets[0] + y - layout.list_top;
                if index < self.lists.len() {
                    let ranged = button & (4 | 8) != 0 || range_pending == Some(Focus::Playlists);
                    let modified = ranged || button & 16 != 0;
                    let now = Instant::now();
                    let double = !modified
                        && self.last_click.is_some_and(|(when, pane, row)| {
                            pane == 0
                                && row == index
                                && now.duration_since(when) < Duration::from_millis(450)
                        });
                    self.select_list_with_modifiers(index, ranged, button & 16 != 0);
                    if ranged {
                        self.status = format!("Selected {} playlists", self.selected_lists.len());
                    }
                    self.last_click = if double || modified {
                        None
                    } else {
                        Some((now, 0, index))
                    };
                    if double {
                        self.enqueue_list(index);
                    }
                }
                return;
            }
            if y >= layout.tree_top && y <= layout.tree_bottom {
                self.focus = Focus::Library;
                let index = self.offsets[1] + y - layout.tree_top;
                if index >= self.items.len() {
                    return;
                }
                let ranged = button & (4 | 8) != 0 || range_pending == Some(Focus::Library);
                let modified = ranged || button & 16 != 0;
                self.select_tree_with_modifiers(index, ranged, button & 16 != 0);
                if ranged {
                    self.status = format!("Selected {} tree items", self.selected_tree.len());
                }
                if let Some(TreeRow {
                    item: Item::Directory(_, path),
                    depth,
                }) = self.items.get(index)
                {
                    if !modified && x <= depth * 2 + 2 {
                        self.toggle_directory(path.clone());
                        self.last_click = None;
                        return;
                    }
                }
                let now = Instant::now();
                let double = !modified
                    && self.last_click.is_some_and(|(when, pane, row)| {
                        pane == 1
                            && row == index
                            && now.duration_since(when) < Duration::from_millis(450)
                    });
                self.last_click = if modified {
                    None
                } else {
                    Some((now, 1, index))
                };
                if double {
                    match self.items.get(index).map(|row| row.item.clone()) {
                        Some(Item::Directory(..)) => self.add_selected(false),
                        Some(Item::Track(_)) => self.activate_selected(),
                        None => {}
                    }
                    self.last_click = None;
                } else if !modified
                    && let Some(TreeRow {
                        item: Item::Directory(_, path),
                        ..
                    }) = self.items.get(index)
                {
                    self.toggle_directory(path.clone());
                }
                return;
            }
        }
        if !layout.show_sidebar && self.focus == Focus::Library {
            if y == 1 {
                self.files_expanded = !self.files_expanded;
            } else if y >= 2 && self.files_expanded {
                let index = self.offsets[1] + y - 2;
                if index < self.items.len() {
                    let ranged = button & (4 | 8) != 0 || range_pending == Some(Focus::Library);
                    let modified = ranged || button & 16 != 0;
                    self.select_tree_with_modifiers(index, ranged, button & 16 != 0);
                    if ranged {
                        self.status = format!("Selected {} tree items", self.selected_tree.len());
                    }
                    if !modified && x < 4 {
                        if let Some(TreeRow {
                            item: Item::Directory(_, path),
                            ..
                        }) = self.items.get(index)
                        {
                            self.toggle_directory(path.clone());
                            self.last_click = None;
                            return;
                        }
                    }
                    let now = Instant::now();
                    let double = !modified
                        && self.last_click.is_some_and(|(when, pane, row)| {
                            pane == 1
                                && row == index
                                && now.duration_since(when) < Duration::from_millis(450)
                        });
                    self.last_click = if double || modified {
                        None
                    } else {
                        Some((now, 1, index))
                    };
                    if double {
                        match self.items.get(index).map(|row| row.item.clone()) {
                            Some(Item::Directory(..)) => self.add_selected(false),
                            Some(Item::Track(_)) => self.activate_selected(),
                            None => {}
                        }
                    } else if !modified
                        && let Some(TreeRow {
                            item: Item::Directory(_, path),
                            ..
                        }) = self.items.get(index)
                    {
                        self.toggle_directory(path.clone());
                    }
                }
            }
            return;
        }
        if !layout.show_sidebar && self.focus == Focus::Playlists {
            if y == 1 {
                if x >= size.0.saturating_sub(4) {
                    self.begin_prompt(PromptKind::NewPlaylist, String::new());
                } else {
                    self.playlists_expanded = !self.playlists_expanded;
                }
            } else if y >= 2 && self.playlists_expanded {
                let index = self.offsets[0] + y - 2;
                if index < self.lists.len() {
                    let ranged = button & (4 | 8) != 0 || range_pending == Some(Focus::Playlists);
                    let modified = ranged || button & 16 != 0;
                    let now = Instant::now();
                    let double = !modified
                        && self.last_click.is_some_and(|(when, pane, row)| {
                            pane == 0
                                && row == index
                                && now.duration_since(when) < Duration::from_millis(450)
                        });
                    self.select_list_with_modifiers(index, ranged, button & 16 != 0);
                    if ranged {
                        self.status = format!("Selected {} playlists", self.selected_lists.len());
                    }
                    self.last_click = if double || modified {
                        None
                    } else {
                        Some((now, 0, index))
                    };
                    if double {
                        self.enqueue_list(index);
                    }
                }
            }
            return;
        }
        if y == 1 && x >= layout.first {
            let relative = x.saturating_sub(layout.first + 1) + self.columns.scroll;
            let boundary_column = self
                .columns
                .positions()
                .find(|(_, start, width)| {
                    ((start + width).saturating_sub(1)..=start + width).contains(&relative)
                })
                .map(|(column, _, _)| column);
            if let Some(column) = boundary_column {
                let now = Instant::now();
                let double = self.last_click.is_some_and(|(when, pane, previous)| {
                    pane == 3
                        && previous == column
                        && now.duration_since(when) < Duration::from_millis(450)
                });
                self.last_click = if double { None } else { Some((now, 3, column)) };
                if double {
                    self.auto_fit_columns();
                } else {
                    self.column_drag = Some(column);
                }
                return;
            }
            let clicked_column = self
                .columns
                .positions()
                .find(|(_, start, width)| (*start..start + width).contains(&relative))
                .map(|(column, _, _)| column);
            if let Some(column) = clicked_column {
                self.sort_tracks_by_column(column);
            }
            return;
        }
        if y >= 2 && x >= layout.first {
            self.focus = Focus::Tracks;
            let visible = self.visible_tracks();
            let Some(&index) = visible.get(self.offsets[2] + y - 2) else {
                return;
            };
            let ranged = button & (4 | 8) != 0 || range_pending == Some(Focus::Tracks);
            let modified = ranged || button & 16 != 0;
            self.select_track(index, ranged, button & 16 != 0);
            if ranged {
                self.status = format!("Selected {} tracks", self.selected_tracks.len());
            }
            let relative = x.saturating_sub(layout.first + 1) + self.columns.scroll;
            let starred_cell = self
                .columns
                .positions()
                .find(|(_, start, width)| (*start..start + width).contains(&relative))
                .is_some_and(|(column, _, _)| self.columns.entries[column].id == "star");
            if starred_cell && !modified {
                self.toggle_star();
                return;
            }
            self.track_drag = (!modified).then_some(index);
            let now = Instant::now();
            let double = !modified
                && self.last_click.is_some_and(|(when, pane, row)| {
                    pane == 2
                        && row == index
                        && now.duration_since(when) < Duration::from_millis(450)
                });
            self.last_click = if double || modified {
                None
            } else {
                Some((now, 2, index))
            };
            if double {
                if self.playing == Some(index) && self.player.state() != PlaybackState::Stopped {
                    self.play_pause();
                } else {
                    self.play_selected();
                }
                self.last_click = None;
            }
        }
    }

    fn folder_mouse(&mut self, button: u16, x: usize, y: usize, size: (usize, usize)) {
        let geometry = FolderGeometry::new(size);
        let Some(mut chooser) = self.folder_chooser.take() else {
            return;
        };
        let mut accept = false;
        let mut cancel = false;
        if (button & 0b1100_0000) == 64 {
            if y >= geometry.list_top() && y < geometry.list_top() + geometry.list_rows() {
                let last = chooser.entries.len().saturating_sub(1);
                chooser.selected = chooser
                    .selected
                    .saturating_add_signed(if button & 1 == 0 { -3 } else { 3 })
                    .min(last);
                chooser.keep_selected_visible(geometry.list_rows());
            }
        } else if button & 32 == 0
            && x >= geometry.x
            && x < geometry.x + geometry.width
            && y >= geometry.y
            && y < geometry.y + geometry.height
        {
            let rel_x = x - geometry.x;
            let rel_y = y - geometry.y;
            match rel_y {
                2 => {
                    chooser.focus = FolderFocus::Location;
                    chooser.select_all = true;
                }
                3 => {
                    if rel_x < 9 {
                        if let Some(home) = std::env::var_os("HOME") {
                            if let Err(error) = chooser.navigate(Path::new(&home)) {
                                chooser.error = error;
                            }
                        }
                    } else if rel_x < 16 {
                        chooser.parent();
                    } else if rel_x < 24 {
                        if let Err(error) = chooser.navigate(Path::new("/")) {
                            chooser.error = error;
                        }
                    } else {
                        chooser.toggle_hidden();
                    }
                }
                row if row >= 4 && row < geometry.height.saturating_sub(3) => {
                    if x > geometry.x && x < geometry.x + geometry.width - 1 {
                        let index = chooser.offset + row - 4;
                        if index < chooser.entries.len() {
                            chooser.focus = FolderFocus::List;
                            let now = Instant::now();
                            let double = chooser.last_click.is_some_and(|(previous, selected)| {
                                selected == index
                                    && now.duration_since(previous) < Duration::from_millis(450)
                            });
                            chooser.selected = index;
                            chooser.last_click = Some((now, index));
                            if double {
                                chooser.open_selected();
                            }
                        }
                    }
                }
                row if row == geometry.height.saturating_sub(2) => {
                    let compact = geometry.width < 36;
                    let cancel_width = if compact { 5 } else { 10 };
                    let choose_width = if compact { 8 } else { 22 };
                    let choose_x = geometry
                        .width
                        .saturating_sub(choose_width + if compact { 1 } else { 2 });
                    let cancel_x =
                        choose_x.saturating_sub(cancel_width + if compact { 1 } else { 2 });
                    if rel_x >= cancel_x && rel_x < cancel_x + cancel_width {
                        cancel = true;
                    } else if rel_x >= choose_x && rel_x < choose_x + choose_width {
                        accept = true;
                    }
                }
                _ => {}
            }
        }
        if !cancel {
            self.folder_chooser = Some(chooser);
            if accept {
                self.accept_folder_chooser();
            }
        }
    }

    fn draw(&mut self, size: (usize, usize)) -> String {
        self.ensure_auto_fit_columns();
        self.reflow_menus(size);
        self.refresh_cover_request();
        if self.info_modal && self.modal.is_some() {
            self.modal = Some(self.info_content());
        }
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
        let marquee_tick = (self.marquee_started.elapsed().as_millis() / 180) as usize;
        let visible_tracks = self.visible_tracks();
        let layout = self.layout_for_track_count(size, visible_tracks.len());
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
        let tree_page = self.tree_page(&layout);
        let pages = [layout.list_page, tree_page, layout.track_page];
        for (pane, len) in [self.lists.len(), self.items.len(), visible_tracks.len()]
            .into_iter()
            .enumerate()
        {
            if len == 0 {
                self.selected[pane] = 0;
                self.offsets[pane] = 0;
                self.manual_scroll_selection[pane] = None;
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
            self.offsets[pane] = self.offsets[pane].min(len.saturating_sub(page));
            if self.manual_scroll_selection[pane] == Some(self.selected[pane]) {
                continue;
            }
            self.manual_scroll_selection[pane] = None;
            if selected_position < self.offsets[pane] {
                self.offsets[pane] = selected_position;
            } else if selected_position >= self.offsets[pane] + page {
                self.offsets[pane] = selected_position + 1 - page;
            }
        }

        let mut screen = String::from("\x1b[H\x1b[?25l");
        paint(&mut screen, 1, 1, "", width, Surface::Toolbar, false);
        paint(&mut screen, 1, 2, "⚙", 2, Surface::Accent, true);
        paint(&mut screen, 1, 6, "≡", 2, Surface::Toolbar, false);
        paint(&mut screen, 1, 10, "▤", 2, Surface::Toolbar, false);
        paint(
            &mut screen,
            1,
            width.saturating_sub(5),
            "?",
            1,
            Surface::Toolbar,
            true,
        );
        let search_x = (width / 2).saturating_sub(17).max(14);
        let search_width = width.saturating_sub(search_x + 7).min(36);
        let playlist_draft = self
            .prompt
            .as_ref()
            .filter(|(kind, _)| *kind == PromptKind::PlaylistSearch)
            .map(|(_, value)| value.as_str());
        let file_draft = self
            .prompt
            .as_ref()
            .filter(|(kind, _)| *kind == PromptKind::Search)
            .map(|(_, value)| value.as_str());
        let playlist_search = if let Some(draft) = playlist_draft {
            format!(
                " ⌕  {}",
                input_window(draft, self.input_cursor, search_width.saturating_sub(5)).0
            )
        } else if !layout.show_sidebar && file_draft.is_some() {
            format!(
                " ⌕  {}",
                input_window(
                    file_draft.unwrap_or_default(),
                    self.input_cursor,
                    search_width.saturating_sub(5)
                )
                .0
            )
        } else if !layout.show_sidebar && self.focus == Focus::Library {
            if self.search_query.is_empty() {
                " ⌕  Search files".to_owned()
            } else {
                format!(" ⌕  {}", self.search_query)
            }
        } else if self.playlist_query.is_empty() {
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
            if playlist_draft.is_some() || (!layout.show_sidebar && file_draft.is_some()) {
                Surface::Input
            } else {
                Surface::Main
            },
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
                if self.files_expanded {
                    " ▾ Files"
                } else {
                    " ▸ Files"
                },
                sidebar,
                Surface::Toolbar,
                true,
            );
            let location = if self.remote_active {
                let address = self
                    .remote_connection
                    .as_ref()
                    .map_or("", |settings| settings.server_url.as_str());
                format!("☁ {} · {}", address, self.remote_path)
            } else {
                self.browse_path
                    .clone()
                    .or_else(|| self.library.root())
                    .as_ref()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "Choose a music folder".to_owned())
            };
            if self.files_expanded {
                paint(
                    &mut screen,
                    3,
                    1,
                    &marquee_prefixed(" ▱  ↻  ", &location, sidebar, marquee_tick),
                    sidebar,
                    Surface::SidebarAlt,
                    true,
                );
            }
            let tree_search = if let Some(draft) = file_draft {
                format!(
                    " ⌕  {}",
                    input_window(draft, self.input_cursor, sidebar.saturating_sub(5)).0
                )
            } else if self.search.is_some() {
                format!(" ⌕  {}", self.search_query)
            } else {
                " ⌕  Search files and folders…".to_owned()
            };
            if self.files_expanded {
                paint(
                    &mut screen,
                    4,
                    1,
                    &tree_search,
                    sidebar,
                    if file_draft.is_some() {
                        Surface::Input
                    } else {
                        Surface::Main
                    },
                    false,
                );
            }
            let tree_scrollbar = self.tree_scrollbar(&layout, size);
            let tree_width = sidebar.saturating_sub(usize::from(tree_scrollbar.is_some()));
            for y in layout.tree_top..=layout.tree_bottom {
                let index = self.offsets[1] + y - layout.tree_top;
                let selected = self
                    .items
                    .get(index)
                    .is_some_and(|row| self.selected_tree.contains(&tree_item_key(&row.item)));
                let surface =
                    if self.focus == Focus::Library && (selected || index == self.selected[1]) {
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
                                marquee_prefixed(
                                    &format!("{indent}{arrow} ▱ "),
                                    name,
                                    tree_width,
                                    marquee_tick,
                                )
                            }
                            Item::Track(track) => marquee_prefixed(
                                &format!("{indent}  {} ", glyph(&track.entry)),
                                &track.name,
                                tree_width,
                                marquee_tick,
                            ),
                        }
                    })
                    .unwrap_or_default();
                paint(
                    &mut screen,
                    y + 1,
                    1,
                    &label,
                    tree_width,
                    surface,
                    false,
                );
            }
            if let Some(bar) = tree_scrollbar {
                paint_vertical_scrollbar(&mut screen, bar);
            }
            paint(
                &mut screen,
                layout.lists_header + 1,
                1,
                if self.playlists_expanded {
                    " ▾ Playlists"
                } else {
                    " ▸ Playlists"
                },
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
                let surface = if self
                    .lists
                    .get(index)
                    .is_some_and(|(id, _)| self.selected_lists.contains(id))
                    || index == self.selected[0] && self.focus == Focus::Playlists
                {
                    Surface::Selected
                } else {
                    Surface::Sidebar
                };
                let label = self.saved_list_label(index, sidebar, marquee_tick);
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
        let track_scrollbar = self.track_scrollbar_for_count(&layout, size, visible_tracks.len());
        let right_width = width.saturating_sub(layout.first + 1);
        let content_width = right_width.saturating_sub(usize::from(track_scrollbar.is_some()));
        self.column_viewport_width = content_width;
        self.columns.scroll_by(0, content_width);
        let scrollbar = self.scrollbar_for_track_count(&layout, size, visible_tracks.len());
        let show_tree = !layout.show_sidebar && self.focus == Focus::Library;
        let show_lists = !layout.show_sidebar && self.focus == Focus::Playlists;
        if show_lists {
            paint(
                &mut screen,
                2,
                1,
                if self.playlists_expanded {
                    " ▾ Playlists                                 +"
                } else {
                    " ▸ Playlists                                 +"
                },
                width,
                Surface::Header,
                true,
            );
            for y in 2..layout.footer_top {
                if !self.playlists_expanded {
                    break;
                }
                let index = self.offsets[0] + y - 2;
                let label = self.saved_list_label(index, width, marquee_tick);
                let surface = if self
                    .lists
                    .get(index)
                    .is_some_and(|(id, _)| self.selected_lists.contains(id))
                    || index == self.selected[0]
                {
                    Surface::Selected
                } else if index % 2 == 1 {
                    Surface::MainAlt
                } else {
                    Surface::Main
                };
                paint(&mut screen, y + 1, 1, &label, width, surface, false);
            }
        } else if show_tree {
            let tree_scrollbar = self.tree_scrollbar(&layout, size);
            let tree_width = width.saturating_sub(usize::from(tree_scrollbar.is_some()));
            paint(
                &mut screen,
                2,
                1,
                if self.files_expanded {
                    " ▾ Files"
                } else {
                    " ▸ Files"
                },
                width,
                Surface::Header,
                true,
            );
            for y in 2..layout.footer_top {
                if !self.files_expanded {
                    break;
                }
                let index = self.offsets[1] + y - 2;
                let label = self
                    .items
                    .get(index)
                    .map(|row| match &row.item {
                        Item::Directory(name, path) => marquee_prefixed(
                            if self.expanded.contains(path) {
                                " ▾ ▱ "
                            } else {
                                " ▸ ▱ "
                            },
                            name,
                            tree_width,
                            marquee_tick,
                        ),
                        Item::Track(track) => marquee_prefixed(
                            &format!("   {} ", glyph(&track.entry)),
                            &track.name,
                            tree_width,
                            marquee_tick,
                        ),
                    })
                    .unwrap_or_default();
                let selected = self
                    .items
                    .get(index)
                    .is_some_and(|row| self.selected_tree.contains(&tree_item_key(&row.item)));
                let surface = if selected || index == self.selected[1] {
                    Surface::Selected
                } else if index % 2 == 1 {
                    Surface::MainAlt
                } else {
                    Surface::Main
                };
                paint(
                    &mut screen,
                    y + 1,
                    1,
                    &label,
                    tree_width,
                    surface,
                    false,
                );
            }
            if let Some(bar) = tree_scrollbar {
                paint_vertical_scrollbar(&mut screen, bar);
            }
        } else {
            paint(
                &mut screen,
                2,
                right_x,
                "",
                content_width,
                Surface::Header,
                true,
            );
            if track_scrollbar.is_some() {
                paint(&mut screen, 2, width, "", 1, Surface::Header, false);
            }
            for (column_index, start, column_width) in self.columns.positions() {
                let selected = self.keyboard_column == Some(column_index);
                paint_playlist_cell(
                    &mut screen,
                    2,
                    right_x,
                    content_width,
                    start,
                    column_width,
                    self.columns.scroll,
                    self.columns.entries[column_index].label,
                    marquee_tick,
                    if selected {
                        Surface::Selected
                    } else {
                        Surface::Header
                    },
                    true,
                );
            }
            for y in 2..layout
                .footer_top
                .saturating_sub(usize::from(scrollbar.is_some()))
            {
                let index = visible_tracks.get(self.offsets[2] + y - 2).copied();
                let surface = if index.is_some_and(|index| self.selected_tracks.contains(&index))
                    || index == Some(self.selected[2]) && self.focus == Focus::Tracks
                {
                    Surface::Selected
                } else if y % 2 == 1 {
                    Surface::MainAlt
                } else {
                    Surface::Main
                };
                paint(&mut screen, y + 1, right_x, "", content_width, surface, false);
                if let Some((index, track)) =
                    index.and_then(|index| self.tracks.get(index).map(|track| (index, track)))
                {
                    for (column_index, start, column_width) in self.columns.positions() {
                        let value =
                            self.column_value(index, track, self.columns.entries[column_index].id);
                        paint_playlist_cell(
                            &mut screen,
                            y + 1,
                            right_x,
                            content_width,
                            start,
                            column_width,
                            self.columns.scroll,
                            &value,
                            marquee_tick,
                            surface,
                            false,
                        );
                    }
                }
            }
            if let Some(bar) = scrollbar {
                let row = layout.footer_top;
                paint(&mut screen, row, bar.x + 1, "‹", 1, Surface::Header, true);
                paint(
                    &mut screen,
                    row,
                    bar.x + 2,
                    &"─".repeat(bar.track_width),
                    bar.track_width,
                    Surface::Header,
                    false,
                );
                paint(
                    &mut screen,
                    row,
                    bar.x + 2 + bar.thumb_start,
                    &"━".repeat(bar.thumb_width),
                    bar.thumb_width,
                    Surface::Accent,
                    true,
                );
                paint(
                    &mut screen,
                    row,
                    bar.x + bar.width,
                    "›",
                    1,
                    Surface::Header,
                    true,
                );
                if track_scrollbar.is_some() {
                    paint(&mut screen, row, width, "┘", 1, Surface::Header, false);
                }
            }
            if let Some(bar) = track_scrollbar {
                paint_vertical_scrollbar(&mut screen, bar);
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
        let cover_space = if width >= 80 {
            if self.cover_preview.is_some() { 6 } else { 3 }
        } else {
            0
        };
        if cover_space > 0 {
            if let Some(cover) = &self.cover_preview {
                paint_cover_preview(&mut screen, layout.footer_top + 1, 2, cover);
            } else {
                paint(
                    &mut screen,
                    layout.footer_top + 1,
                    2,
                    "◈",
                    4,
                    Surface::Accent,
                    true,
                );
            }
        } else {
            paint(
                &mut screen,
                layout.footer_top + 1,
                2,
                "◈",
                2,
                Surface::Accent,
                true,
            );
        }
        let title_width = width.saturating_div(2).saturating_sub(17 + cover_space);
        paint(
            &mut screen,
            layout.footer_top + 1,
            5 + cover_space,
            &marquee_window(&current, title_width, marquee_tick),
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
            .unwrap_or_else(|| match self.player.state() {
                PlaybackState::Playing => "Playing".to_owned(),
                PlaybackState::Paused => "Paused".to_owned(),
                PlaybackState::Stopped => "Ready to play".to_owned(),
            });
        paint(
            &mut screen,
            layout.footer_top + 2,
            5 + cover_space,
            &marquee_window(&subtitle, title_width, marquee_tick),
            title_width,
            Surface::Muted,
            false,
        );
        if self.compact_mode {
            for y in 1..layout.footer_top {
                paint(&mut screen, y + 1, 1, "", width, Surface::Main, false);
            }
            paint(
                &mut screen,
                1,
                13,
                "▣  Compact Player · C returns to playlist",
                width.saturating_sub(16),
                Surface::Toolbar,
                true,
            );
            let card_width = width.saturating_sub(4).min(72);
            let card_x = width.saturating_sub(card_width) / 2 + 1;
            let rich = card_width >= 68 && layout.footer_top >= 18;
            let card_y = if rich {
                layout.footer_top.saturating_sub(11) / 2
            } else {
                layout.footer_top.saturating_div(2).saturating_sub(1).max(3)
            };
            if rich {
                for row in 0..11 {
                    paint(
                        &mut screen,
                        card_y + row,
                        card_x,
                        &" ".repeat(card_width),
                        card_width,
                        Surface::Sidebar,
                        false,
                    );
                }
                paint(
                    &mut screen,
                    card_y,
                    card_x,
                    &format!("╭{}╮", "─".repeat(card_width - 2)),
                    card_width,
                    Surface::Accent,
                    false,
                );
                paint(
                    &mut screen,
                    card_y + 10,
                    card_x,
                    &format!("╰{}╯", "─".repeat(card_width - 2)),
                    card_width,
                    Surface::Accent,
                    false,
                );
                for row in 1..10 {
                    paint(
                        &mut screen,
                        card_y + row,
                        card_x,
                        "│",
                        1,
                        Surface::Accent,
                        false,
                    );
                    paint(
                        &mut screen,
                        card_y + row,
                        card_x + card_width - 1,
                        "│",
                        1,
                        Surface::Accent,
                        false,
                    );
                }
                paint(
                    &mut screen,
                    card_y,
                    card_x + 3,
                    " Now Playing ",
                    13,
                    Surface::Accent,
                    true,
                );
                if let Some(cover) = &self.cover_preview {
                    paint_cover_at_size(&mut screen, card_y + 2, card_x + 3, cover, 12);
                } else {
                    for row in 2..8 {
                        paint(
                            &mut screen,
                            card_y + row,
                            card_x + 3,
                            "            ",
                            12,
                            Surface::MainAlt,
                            false,
                        );
                    }
                    paint(
                        &mut screen,
                        card_y + 4,
                        card_x + 6,
                        "◈ KOG",
                        5,
                        Surface::Accent,
                        true,
                    );
                }
                paint(
                    &mut screen,
                    card_y + 2,
                    card_x + 18,
                    &marquee_window(&current, card_width - 21, marquee_tick),
                    card_width - 21,
                    Surface::Sidebar,
                    true,
                );
                paint(
                    &mut screen,
                    card_y + 3,
                    card_x + 18,
                    &marquee_window(&subtitle, card_width - 21, marquee_tick),
                    card_width - 21,
                    Surface::Muted,
                    false,
                );
                let position = self.player.position();
                let duration = self
                    .playing
                    .and_then(|index| self.tracks.get(index))
                    .and_then(|track| self.metadata_for(track))
                    .and_then(|meta| meta.duration);
                let filled = duration
                    .filter(|value| !value.is_zero())
                    .map(|value| {
                        ((position.as_secs_f64() / value.as_secs_f64()).clamp(0.0, 1.0) * 26.0)
                            .round() as usize
                    })
                    .unwrap_or(0);
                let duration_label = duration
                    .map(|value| format!("{}:{:02}", value.as_secs() / 60, value.as_secs() % 60))
                    .unwrap_or_else(|| "--:--".to_owned());
                paint(
                    &mut screen,
                    card_y + 6,
                    card_x + 18,
                    &format!(
                        "{:>6}  {}{}  {}",
                        format!("{}:{:02}", position.as_secs() / 60, position.as_secs() % 60),
                        "━".repeat(filled),
                        "─".repeat(26 - filled),
                        duration_label
                    ),
                    card_width - 21,
                    Surface::Sidebar,
                    false,
                );
                paint(
                    &mut screen,
                    card_y + 8,
                    card_x + 18,
                    &compact_transport(
                        card_width - 21,
                        true,
                        self.player.state() == PlaybackState::Playing,
                    )
                    .text,
                    card_width - 21,
                    Surface::AccentSidebar,
                    false,
                );
                let volume_filled = (self.volume * 19.0).round() as usize;
                paint(
                    &mut screen,
                    card_y + 9,
                    card_x + 18,
                    &format!(
                        "♪ {}{} {:>3}%",
                        "━".repeat(volume_filled),
                        "─".repeat(20 - volume_filled),
                        (self.volume * 100.0).round() as u8
                    ),
                    card_width - 21,
                    Surface::Sidebar,
                    false,
                );
            } else {
                paint(
                    &mut screen,
                    card_y,
                    card_x,
                    "Now Playing",
                    card_width,
                    Surface::Accent,
                    true,
                );
                if let Some(cover) = &self.cover_preview {
                    paint_cover_at_size(&mut screen, card_y + 1, card_x, cover, 4);
                } else {
                    paint(
                        &mut screen,
                        card_y + 1,
                        card_x,
                        "◈",
                        4,
                        Surface::Accent,
                        true,
                    );
                }
                paint(
                    &mut screen,
                    card_y + 1,
                    card_x + 6,
                    &marquee_window(
                        &current,
                        card_width.saturating_sub(6),
                        marquee_tick,
                    ),
                    card_width.saturating_sub(6),
                    Surface::Main,
                    true,
                );
                paint(
                    &mut screen,
                    card_y + 2,
                    card_x + 6,
                    &marquee_window(
                        &subtitle,
                        card_width.saturating_sub(6),
                        marquee_tick,
                    ),
                    card_width.saturating_sub(6),
                    Surface::Muted,
                    false,
                );
                paint(
                    &mut screen,
                    card_y + 4,
                    card_x,
                    &compact_transport(
                        card_width,
                        false,
                        self.player.state() == PlaybackState::Playing,
                    )
                    .text,
                    card_width,
                    Surface::Accent,
                    false,
                );
            }
        }
        let playing = self.player.state() == PlaybackState::Playing;
        for action in FOOTER_TRANSPORT {
            let slot = footer_transport_slot(width, action);
            let show_mode = slot.end - slot.start >= 3;
            let (icon, surface) = match action {
                TransportAction::Shuffle => (
                    match self.shuffle_mode {
                        ShuffleMode::Off => MEDIA_SHUFFLE,
                        ShuffleMode::Albums if show_mode => "🔀A",
                        ShuffleMode::All if show_mode => "🔀•",
                        _ => MEDIA_SHUFFLE,
                    },
                    if self.shuffle_mode == ShuffleMode::Off {
                        Surface::Muted
                    } else {
                        Surface::Accent
                    },
                ),
                TransportAction::Previous => (MEDIA_PREVIOUS, Surface::Accent),
                TransportAction::PlayPause => (
                    if playing { MEDIA_PAUSE } else { MEDIA_PLAY },
                    Surface::Accent,
                ),
                TransportAction::Stop => (MEDIA_STOP, Surface::Accent),
                TransportAction::Next => (MEDIA_NEXT, Surface::Accent),
                TransportAction::Repeat => (
                    match self.repeat_mode {
                        RepeatMode::One => MEDIA_REPEAT_ONE,
                        RepeatMode::Album if show_mode => "🔁A",
                        _ => MEDIA_REPEAT,
                    },
                    if self.repeat_mode == RepeatMode::Off {
                        Surface::Muted
                    } else {
                        Surface::Accent
                    },
                ),
                TransportAction::Radio => (
                    MEDIA_RADIO,
                    if self.radio_enabled {
                        Surface::Accent
                    } else {
                        Surface::Muted
                    },
                ),
            };
            let icon_width = cell_width(icon);
            let icon_x = slot.start + (slot.end - slot.start).saturating_sub(icon_width) / 2;
            paint(
                &mut screen,
                layout.footer_top + 1,
                icon_x + 1,
                icon,
                icon_width.min(slot.end - slot.start),
                surface,
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
        let (progress_x, _, bar_width) = progress_bar_geometry(width, &clock);
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
            progress_x + 1,
            &format!("{clock:>5}  {progress_bar}  {duration_label}"),
            width.saturating_sub(progress_x),
            Surface::Toolbar,
            false,
        );
        let (volume_row, icon_x, bar_x, volume_width) = volume_geometry(width, layout.footer_top);
        paint(
            &mut screen,
            volume_row + 1,
            icon_x + 1,
            if self.volume <= 0.0 { "×" } else { "♪" },
            2,
            Surface::Toolbar,
            false,
        );
        paint(
            &mut screen,
            volume_row + 1,
            bar_x + 1,
            &"─".repeat(volume_width),
            volume_width,
            Surface::Muted,
            false,
        );
        let thumb = (self.volume * (volume_width - 1) as f32).round() as usize;
        if thumb > 0 {
            paint(
                &mut screen,
                volume_row + 1,
                bar_x + 1,
                &"━".repeat(thumb),
                thumb,
                Surface::Accent,
                false,
            );
        }
        paint(
            &mut screen,
            volume_row + 1,
            bar_x + thumb + 1,
            "●",
            1,
            Surface::Accent,
            false,
        );
        paint(
            &mut screen,
            volume_row + 1,
            bar_x + volume_width + 2,
            &format!("{:>3}%", (self.volume * 100.0).round() as u8),
            4,
            Surface::Toolbar,
            false,
        );
        let message = self.prompt.as_ref().map(|(kind, value)| {
            if matches!(kind, PromptKind::Search | PromptKind::PlaylistSearch) {
                if *kind == PromptKind::Search && !self.status.is_empty() {
                    return format!("{} · Enter finish · Esc clear", self.status);
                }
                return "Search updates as you type · Enter finish · Esc clear".to_owned();
            }
            let _ = value;
            "Enter confirm · Esc cancel · ←/→ move caret".to_owned()
        }).unwrap_or_else(|| {
            if self.status.is_empty() {
                "Tab pane · M actions · H columns · x mark · ? keys · Enter open/play · Space pause".to_owned()
            } else { self.status.clone() }
        });
        let message = if let Some(column) = self
            .keyboard_column
            .filter(|_| self.prompt.is_none() && !self.menu_open)
        {
            format!(
                "{} column · ←/→ choose · Enter sort · +/- width · Ctrl+←/→ reorder · M actions · Esc quit",
                self.columns.entries[column].label
            )
        } else {
            message
        };
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
            let mut layers = self.menu_parents.clone();
            layers.push(self.active_menu_layer());
            for layer in layers {
                let labels = layer.page.labels();
                let shortcuts = menu_shortcuts(layer.page);
                let page = labels.len().min(height.saturating_sub(4));
                let menu_width = width.saturating_sub(layer.x).min(menu_width(width));
                paint(
                    &mut screen,
                    layer.y + 1,
                    layer.x + 1,
                    &format!("╭─ {} ", layer.page.title()),
                    menu_width,
                    Surface::Header,
                    true,
                );
                paint(
                    &mut screen,
                    layer.y + 1,
                    layer.x + menu_width,
                    "╮",
                    1,
                    Surface::Header,
                    false,
                );
                for (row, index) in (layer.offset..labels.len()).take(page).enumerate() {
                    let label = labels[index];
                    let shown = menu_row(label, shortcuts[index], menu_width);
                    paint(
                        &mut screen,
                        layer.y + row + 2,
                        layer.x + 1,
                        &shown,
                        menu_width,
                        if index == layer.selected {
                            Surface::Selected
                        } else {
                            Surface::Toolbar
                        },
                        index == layer.selected,
                    );
                    paint(
                        &mut screen,
                        layer.y + row + 2,
                        layer.x + menu_width,
                        "│",
                        1,
                        if index == layer.selected {
                            Surface::Selected
                        } else {
                            Surface::Toolbar
                        },
                        false,
                    );
                }
                paint(
                    &mut screen,
                    layer.y + page + 2,
                    layer.x + 1,
                    &format!("╰{}╯", "─".repeat(menu_width.saturating_sub(2))),
                    menu_width,
                    Surface::Toolbar,
                    false,
                );
                if layer.offset > 0 {
                    paint(
                        &mut screen,
                        layer.y + 1,
                        layer.x + menu_width.saturating_sub(3),
                        "↑",
                        1,
                        Surface::Header,
                        false,
                    );
                }
                if layer.offset + page < labels.len() {
                    paint(
                        &mut screen,
                        layer.y + page + 1,
                        layer.x + menu_width.saturating_sub(3),
                        "↓",
                        1,
                        Surface::Toolbar,
                        false,
                    );
                }
            }
        }
        if self.visualizer_open {
            self.modal = Some(self.visualizer_text());
        }
        if self.artwork_modal && self.modal.is_some() && width >= 24 && height >= 18 {
            let labeled = width >= 56 && height >= 29;
            let extra_rows = if labeled { 5 } else { 2 };
            let art_size = COVER_WIDTH
                .min(width.saturating_sub(8))
                .min(height.saturating_sub(extra_rows).saturating_mul(2))
                & !1;
            let box_width = art_size + 8;
            let box_height = art_size / 2 + extra_rows;
            let x = width.saturating_sub(box_width) / 2 + 1;
            let y = height.saturating_sub(box_height) / 2 + 1;
            for row in 0..box_height {
                paint(
                    &mut screen,
                    y + row,
                    x,
                    &" ".repeat(box_width),
                    box_width,
                    Surface::Toolbar,
                    false,
                );
            }
            paint(
                &mut screen,
                y,
                x,
                &format!("╭{}╮", "─".repeat(box_width - 2)),
                box_width,
                Surface::Header,
                false,
            );
            paint(
                &mut screen,
                y + box_height - 1,
                x,
                &format!("╰{}╯", "─".repeat(box_width - 2)),
                box_width,
                Surface::Header,
                false,
            );
            for row in 1..box_height - 1 {
                paint(&mut screen, y + row, x, "│", 1, Surface::Header, false);
                paint(
                    &mut screen,
                    y + row,
                    x + box_width - 1,
                    "│",
                    1,
                    Surface::Header,
                    false,
                );
            }
            if labeled {
                paint(
                    &mut screen,
                    y + 1,
                    x + 2,
                    &current,
                    box_width - 4,
                    Surface::Toolbar,
                    true,
                );
            }
            let art_row = y + if labeled { 2 } else { 1 };
            if let Some(cover) = &self.cover_preview {
                paint_cover_at_size(&mut screen, art_row, x + 4, cover, art_size);
            } else {
                paint(
                    &mut screen,
                    art_row + art_size / 4,
                    x + 4,
                    "◈ KOG",
                    art_size,
                    Surface::Accent,
                    true,
                );
            }
            if labeled {
                paint(
                    &mut screen,
                    art_row + art_size / 2,
                    x + 2,
                    &subtitle,
                    box_width - 4,
                    Surface::Muted,
                    false,
                );
                paint(
                    &mut screen,
                    art_row + 1 + art_size / 2,
                    x + 2,
                    "Click or Esc to close",
                    box_width - 4,
                    Surface::Muted,
                    false,
                );
            }
        } else if let Some(modal) = &self.modal {
            let lines = modal_lines(modal, width.saturating_sub(12));
            let page = lines.len().min(height.saturating_sub(8)).max(1);
            self.modal_scroll = self.modal_scroll.min(lines.len().saturating_sub(page));
            let shown = &lines[self.modal_scroll..(self.modal_scroll + page).min(lines.len())];
            let box_width = shown
                .iter()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(0)
                .min(width.saturating_sub(8))
                + 4;
            let x = width.saturating_sub(box_width) / 2 + 1;
            let y = height.saturating_sub(shown.len() + 2) / 2 + 1;
            paint(
                &mut screen,
                y,
                x,
                &format!("╭{}╮", "─".repeat(box_width.saturating_sub(2))),
                box_width,
                Surface::Header,
                true,
            );
            for (index, line) in shown.iter().enumerate() {
                paint(
                    &mut screen,
                    y + index + 1,
                    x,
                    &format!("│ {line}"),
                    box_width,
                    Surface::Toolbar,
                    false,
                );
                paint(
                    &mut screen,
                    y + index + 1,
                    x + box_width - 1,
                    "│",
                    1,
                    Surface::Toolbar,
                    false,
                );
            }
            paint(
                &mut screen,
                y + shown.len() + 1,
                x,
                &format!("╰{}╯", "─".repeat(box_width.saturating_sub(2))),
                box_width,
                Surface::Toolbar,
                false,
            );
            if lines.len() > page {
                paint(
                    &mut screen,
                    y + shown.len() + 1,
                    x + 2,
                    &format!(" {}/{} ↑↓ ", self.modal_scroll + 1, lines.len() - page + 1),
                    box_width.saturating_sub(4),
                    Surface::Toolbar,
                    false,
                );
            }
        }
        if let Some((kind, value)) = &self.prompt {
            if matches!(kind, PromptKind::Search | PromptKind::PlaylistSearch) {
                let (row, col, field_width) = if *kind == PromptKind::Search && layout.show_sidebar
                {
                    (4, 1, layout.first)
                } else {
                    (1, search_x, search_width)
                };
                let cursor =
                    input_window(value, self.input_cursor, field_width.saturating_sub(5)).1;
                let column = (col + 5 + cursor).min(col + field_width.saturating_sub(1));
                screen.push_str(&format!("\x1b[{row};{column}H\x1b[?25h"));
            } else {
                let box_width = width.saturating_sub(8).min(72).max(12);
                let x = width.saturating_sub(box_width) / 2 + 1;
                let y = height / 2;
                let masked = if matches!(kind, PromptKind::RemoteToken | PromptKind::RemotePassword)
                {
                    "•".repeat(value.chars().count())
                } else {
                    value.clone()
                };
                let (shown, cursor) =
                    input_window(&masked, self.input_cursor, box_width.saturating_sub(4));
                paint(
                    &mut screen,
                    y.saturating_sub(1),
                    x,
                    &format!("╭{}╮", "─".repeat(box_width.saturating_sub(2))),
                    box_width,
                    Surface::Header,
                    false,
                );
                paint(
                    &mut screen,
                    y,
                    x,
                    &format!("│ {}", prompt_label(*kind)),
                    box_width,
                    Surface::Toolbar,
                    true,
                );
                paint(
                    &mut screen,
                    y,
                    x + box_width - 1,
                    "│",
                    1,
                    Surface::Toolbar,
                    false,
                );
                paint(
                    &mut screen,
                    y + 1,
                    x,
                    &format!("│ {shown}"),
                    box_width,
                    Surface::Input,
                    false,
                );
                if self.input_select_all {
                    paint(
                        &mut screen,
                        y + 1,
                        x + 2,
                        &shown,
                        box_width.saturating_sub(4),
                        Surface::Selected,
                        false,
                    );
                }
                paint(
                    &mut screen,
                    y + 1,
                    x + box_width - 1,
                    "│",
                    1,
                    Surface::Input,
                    false,
                );
                paint(
                    &mut screen,
                    y + 2,
                    x,
                    &format!("╰{}╯", "─".repeat(box_width.saturating_sub(2))),
                    box_width,
                    Surface::Toolbar,
                    false,
                );
                screen.push_str(&format!("\x1b[{};{}H\x1b[?25h", y + 1, x + 2 + cursor));
            }
        }
        if let Some(chooser) = &mut self.folder_chooser {
            draw_folder_chooser(&mut screen, chooser, size, marquee_tick);
        }
        screen
    }
}

impl Drop for Ui {
    fn drop(&mut self) {
        if self.session_dirty {
            let _ = self.flush_session();
        }
        let _ = AppSettings::save_output_volume(f64::from(self.volume));
    }
}

fn track_from_entry(entry: StoredEntry) -> Track {
    let name = if entry.kind == "remote" {
        url::Url::parse(&entry.path)
            .ok()
            .and_then(|url| {
                let pairs: HashMap<_, _> = url.query_pairs().into_owned().collect();
                let source = pairs
                    .get("entry")
                    .or_else(|| pairs.get("path"))
                    .map(String::as_str)
                    .unwrap_or_else(|| url.path());
                let mut name = Path::new(source)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                if let Some(number) = pairs
                    .get("fragment")
                    .and_then(|value| value.parse::<u32>().ok())
                {
                    name.push_str(&format!(" [{}]", number.saturating_add(1)));
                }
                (!name.is_empty()).then_some(name)
            })
            .unwrap_or_else(|| "Remote stream".to_owned())
    } else if entry.kind == "archive" && !entry.entry.is_empty() {
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

fn remote_track(settings: &RemoteSettings, file: RemoteFile) -> Result<Track, String> {
    let url = settings.stream_url(&file)?;
    Ok(Track {
        name: file.name,
        entry: StoredEntry {
            kind: "remote".to_owned(),
            path: url,
            entry: String::new(),
            fragment: None,
        },
    })
}

fn remote_items(settings: &RemoteSettings, listing: RemoteListing) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    items.extend(
        listing
            .directories
            .into_iter()
            .map(|directory| Item::Directory(directory.name, PathBuf::from(directory.path))),
    );
    for file in listing.files {
        items.push(Item::Track(remote_track(settings, file)?));
    }
    Ok(items)
}

fn radio_track(entry: RadioEntry) -> Track {
    track_from_entry(StoredEntry {
        kind: entry.kind,
        path: entry.path,
        entry: entry.entry,
        fragment: entry.fragment,
    })
}

fn collect_folder(
    library: &Arc<Library>,
    decoders: &DecoderRegistry,
    path: PathBuf,
    read_cue_sheets: bool,
    read_playlists: bool,
) -> Result<Vec<Track>, String> {
    let mut pending = vec![path];
    let mut tracks = Vec::new();
    while let Some(directory) = pending.pop() {
        let listing = browse_local(library, directory.to_str())?;
        if let Some(dirs) = listing["directories"].as_array() {
            for dir in dirs.iter().rev() {
                if let Some(path) = dir["path"].as_str() {
                    pending.push(PathBuf::from(path));
                }
            }
        }
        if let Some(files) = listing["files"].as_array() {
            for file in files {
                if let (Some(kind), Some(path)) = (file["kind"].as_str(), file["path"].as_str()) {
                    let track = track_from_entry(StoredEntry {
                        kind: kind.to_owned(),
                        path: path.to_owned(),
                        entry: file["entry"].as_str().unwrap_or_default().to_owned(),
                        fragment: file["fragment"].as_str().map(str::to_owned),
                    });
                    let extension_path =
                        if track.entry.kind == "archive" && !track.entry.entry.is_empty() {
                            &track.entry.entry
                        } else {
                            &track.entry.path
                        };
                    let extension = Path::new(extension_path)
                        .extension()
                        .and_then(|value| value.to_str())
                        .unwrap_or_default();
                    if extension.eq_ignore_ascii_case("cue") && !read_cue_sheets {
                        continue;
                    }
                    if matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "m3u" | "m3u8" | "pls"
                    ) && !read_playlists
                    {
                        continue;
                    }
                    tracks.extend(expand_track(decoders, library.root().as_deref(), track));
                }
            }
        }
    }
    let specific: HashSet<_> = tracks
        .iter()
        .filter(|track| track.entry.fragment.is_some())
        .map(|track| {
            (
                track.entry.kind.clone(),
                track.entry.path.clone(),
                track.entry.entry.clone(),
            )
        })
        .collect();
    tracks.retain(|track| {
        track.entry.fragment.is_some()
            || !specific.contains(&(
                track.entry.kind.clone(),
                track.entry.path.clone(),
                track.entry.entry.clone(),
            ))
    });
    Ok(tracks)
}

fn expand_track(decoders: &DecoderRegistry, root: Option<&Path>, track: Track) -> Vec<Track> {
    if track.entry.kind == "remote" || track.entry.fragment.is_some() {
        return vec![track];
    }
    let expanded = expand_stored_entry(decoders, root, &track.entry, &track.name);
    if expanded.is_empty() {
        vec![track]
    } else {
        expanded
            .into_iter()
            .map(|(name, entry)| Track { name, entry })
            .collect()
    }
}

fn display_title(track: &Track) -> String {
    let (name, suffix) = numbered_track_name(&track.name).unwrap_or((&track.name, ""));
    let mut title = Path::new(name)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(name)
        .to_owned();
    title.push_str(suffix);
    title
}

fn numbered_track_name(name: &str) -> Option<(&str, &str)> {
    let (base, number) = name.rsplit_once(" [")?;
    let number = number.strip_suffix(']')?;
    number.parse::<u32>().ok()?;
    Some((base, &name[base.len()..]))
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

fn display_entry_path(entry: &StoredEntry) -> String {
    if entry.kind == "remote" {
        if let Ok(url) = url::Url::parse(&entry.path) {
            let mut path = url
                .query_pairs()
                .find(|(key, _)| key == "path")
                .map(|(_, value)| value.into_owned())
                .unwrap_or_else(|| url.path().to_owned());
            if let Some((_, nested)) = url.query_pairs().find(|(key, _)| key == "entry") {
                path.push('/');
                path.push_str(&nested);
            }
            return format!("{} · {path}", url.origin().ascii_serialization());
        }
    }
    if entry.entry.is_empty() {
        entry.path.clone()
    } else {
        format!(
            "{}/{}",
            entry.path.trim_end_matches('/'),
            entry.entry.trim_start_matches('/')
        )
    }
}

fn canonical_blacklist_path(path: &str) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| PathBuf::from(path))
        .to_string_lossy()
        .into_owned()
}

fn blacklist_song(entry: &StoredEntry) -> BlacklistEntry {
    BlacklistEntry {
        id: 0,
        kind: BLACKLIST_SONG.to_owned(),
        path: if entry.kind == "remote" {
            entry.path.clone()
        } else {
            canonical_blacklist_path(&entry.path)
        },
        entry: if entry.kind == "archive" {
            entry.entry.clone()
        } else {
            String::new()
        },
    }
}

fn item_under_path(item: &Item, deleted: &Path) -> bool {
    match item {
        Item::Directory(_, path) => path.starts_with(deleted),
        Item::Track(track) => {
            track.entry.kind != "remote" && Path::new(&track.entry.path).starts_with(deleted)
        }
    }
}

fn byte_offset(text: &str, character: usize) -> usize {
    text.char_indices()
        .nth(character)
        .map_or(text.len(), |(offset, _)| offset)
}

fn parse_seek_position(text: &str) -> Option<u64> {
    let parts: Vec<_> = text.split(':').collect();
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|part| part.is_empty()) {
        return None;
    }
    let numbers: Vec<u64> = parts
        .iter()
        .map(|part| part.parse().ok())
        .collect::<Option<_>>()?;
    match numbers.as_slice() {
        [seconds] => Some(*seconds),
        [minutes, seconds] if *seconds < 60 => minutes.checked_mul(60)?.checked_add(*seconds),
        [hours, minutes, seconds] if *minutes < 60 && *seconds < 60 => hours
            .checked_mul(3600)?
            .checked_add(minutes.checked_mul(60)?)?
            .checked_add(*seconds),
        _ => None,
    }
}

fn previous_path_boundary(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut position = cursor.min(chars.len());
    while position > 0 && chars[position - 1] == '/' {
        position -= 1;
    }
    while position > 0 && chars[position - 1] != '/' {
        position -= 1;
    }
    position
}

fn previous_path_word_boundary(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut position = cursor.min(chars.len());
    while position > 0 && (chars[position - 1] == '/' || chars[position - 1].is_whitespace()) {
        position -= 1;
    }
    while position > 0 && chars[position - 1] != '/' && !chars[position - 1].is_whitespace() {
        position -= 1;
    }
    position
}

fn next_path_word_boundary(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut position = cursor.min(chars.len());
    while position < chars.len() && chars[position] != '/' && !chars[position].is_whitespace() {
        position += 1;
    }
    while position < chars.len() && (chars[position] == '/' || chars[position].is_whitespace()) {
        position += 1;
    }
    position
}

fn next_path_boundary(text: &str, cursor: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let mut position = cursor.min(chars.len());
    while position < chars.len() && chars[position] != '/' {
        position += 1;
    }
    while position < chars.len() && chars[position] == '/' {
        position += 1;
    }
    position
}

fn cell_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn modal_lines(content: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut result = Vec::new();
    for line in content.lines() {
        let chars: Vec<_> = line.chars().collect();
        if chars.is_empty() {
            result.push(String::new());
        } else {
            result.extend(chars.chunks(width).map(|part| part.iter().collect()));
        }
    }
    if result.is_empty() {
        result.push(String::new());
    }
    result
}

fn prompt_label(kind: PromptKind) -> &'static str {
    match kind {
        PromptKind::MusicFolder => "Music folder",
        PromptKind::NewPlaylist => "New playlist",
        PromptKind::RenamePlaylist => "Rename playlist",
        PromptKind::DeletePlaylist => "Type yes to delete selected playlists",
        PromptKind::Search => "Search files",
        PromptKind::PlaylistSearch => "Search playlist",
        PromptKind::AddFile => "Add file or folder",
        PromptKind::AddUrl => "Add URL",
        PromptKind::SavePlaylist => "Save playlist as",
        PromptKind::SaveSelection => "Save selection as",
        PromptKind::DuplicatePlaylist => "Duplicate playlist as",
        PromptKind::ExportPlaylist => "Export playlist to",
        PromptKind::Volume => "Volume 0-100",
        PromptKind::SeekPosition => "Seek to seconds, mm:ss, or hh:mm:ss",
        PromptKind::AddToPlaylist => "Add to saved playlist named",
        PromptKind::EqualizerPreset => "Equalizer preset",
        PromptKind::EqualizerBandNumber => "Equalizer band 1-10",
        PromptKind::EqualizerBandGain(_) => "Equalizer gain -20 to 20 dB",
        PromptKind::Preamp => "Preamp dB",
        PromptKind::OutputDevice => "Output device name or default",
        PromptKind::RemoveBlacklistId => "Remove blacklist entry ID",
        PromptKind::TrashSelected => "Type yes to move selected item to trash",
        PromptKind::TagField(index) => TAG_FIELDS[index].1,
        PromptKind::TagArtwork => "Replacement artwork file path",
        PromptKind::RemoteUrl => "Server address (http or https)",
        PromptKind::RemoteToken => "API token",
        PromptKind::RemoteUsername => "Server username",
        PromptKind::RemotePassword => "Server password",
        PromptKind::SoundFont => "SF2 SoundFont path (empty to clear)",
        PromptKind::Sc55Roms => "SC-55 ROM directory (empty to clear)",
        PromptKind::Mt32Roms => "MT-32 ROM directory (empty to clear)",
        PromptKind::Sc55Archive => "SC-55 ROM archive path",
        PromptKind::Mt32Archive => "MT-32 ROM archive path",
        PromptKind::ServerAddress => "Server bind IP address",
        PromptKind::ServerPort => "Server port 1-65535",
        PromptKind::ServerToken => "API token (empty to clear)",
        PromptKind::ServerUsername => "Server username",
        PromptKind::ServerPassword => "Server password",
        PromptKind::ServerCertificate => "PEM certificate file path",
        PromptKind::ServerPrivateKey => "PEM private key file path",
        PromptKind::ServerCache => "Stream cache size in MB",
        PromptKind::ServerDevice => "Device ID to block or allow",
    }
}

fn input_window(text: &str, cursor: usize, width: usize) -> (String, usize) {
    if width == 0 {
        return (String::new(), 0);
    }
    let chars: Vec<char> = text.chars().collect();
    let cursor = cursor.min(chars.len());
    let mut start = 0;
    let mut before = chars[..cursor]
        .iter()
        .map(|c| c.width().unwrap_or(0))
        .sum::<usize>();
    while before >= width && start < cursor {
        before = before.saturating_sub(chars[start].width().unwrap_or(0));
        start += 1;
    }
    let mut shown = String::new();
    let mut used = 0;
    for &character in &chars[start..] {
        let cells = character.width().unwrap_or(0);
        if used + cells > width {
            break;
        }
        shown.push(character);
        used += cells;
    }
    (shown, before.min(width.saturating_sub(1)))
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

const MAIN_MENU: [&str; 16] = [
    "Add Files or Folder…",
    "Add URL…",
    "Choose Music Folder…",
    "",
    "Save Current Playlist…",
    "Save Selection As…",
    "Remove Selected",
    "Clear Playlist",
    "",
    "View                     ›",
    "Playback                 ›",
    "Preferences              ›",
    "",
    "Connect to Server        ›",
    "About Kog",
    "Quit",
];
const REMOTE_MENU: [&str; 9] = [
    "Server Address…",
    "API Token…",
    "Username…",
    "Password…",
    "Cycle Authentication",
    "Cycle Stream Codec",
    "Connect and Browse",
    "Queue Current Folder",
    "Use Local Library",
];
const VIEW_MENU: [&str; 8] = [
    "Show/Hide Files and Playlists",
    "Track Info…",
    "Lyrics…",
    "Equalizer…",
    "Visualizer…",
    "Supported Formats…",
    "Compact Player On/Off",
    "Show Album Cover…",
];
const PLAYBACK_MENU: [&str; 14] = [
    "Play/Pause",
    "Stop",
    "Previous",
    "Next",
    "Cycle Shuffle",
    "Cycle Repeat",
    "Stop After Current",
    "Mute/Unmute",
    "Random Radio On/Off",
    "Reshuffle Radio",
    "Toggle Queue Selected",
    "Toggle Stop After Selected",
    "Clear Queue",
    "Seek to Time…",
];
const PREFERENCES_MENU: [&str; 17] = [
    "Music Folder…",
    "Volume…",
    "Cycle Repeat",
    "Equalizer Preset…",
    "Equalizer On/Off",
    "Preamp…",
    "Output Device…",
    "List Output Devices…",
    "Edit Equalizer Band…",
    "View Blacklist…",
    "Remove Blacklist Entry…",
    "Cycle Opening Files Behavior",
    "MIDI Synthesis          ›",
    "Read CUE Sheets On/Off",
    "Read M3U/PLS On/Off",
    "API Server               ›",
    "Auto Download Covers On/Off",
];
const SERVER_MENU: [&str; 17] = [
    "Start Server",
    "Stop Server",
    "Server Status…",
    "Bind Address…",
    "Port…",
    "Cycle Authentication",
    "API Token…",
    "Generate API Token",
    "Username…",
    "Password…",
    "Cycle HTTPS",
    "Import PEM Certificate…",
    "Cycle Stream Codec",
    "Stream Cache MB…",
    "Connected Devices…",
    "Block/Unblock Device ID…",
    "Server Problems…",
];
const SYNTHESIS_MENU: [&str; 8] = [
    "Cycle MIDI Backend",
    "SoundFont Path…",
    "SC-55 ROM Directory…",
    "MT-32 ROM Directory…",
    "MT-32 GM Mapping On/Off",
    "Show Synthesis Settings…",
    "Import SC-55 ROM Archive…",
    "Import MT-32 ROM Archive…",
];
const TREE_MENU: [&str; 10] = [
    "Add to Current Playlist",
    "Play Now",
    "Expand/Collapse",
    "Star/Unstar",
    "Add to Saved Playlist…",
    "Blacklist Song",
    "Blacklist Folder",
    "Move to Trash…",
    "Use as Tree Root",
    "Reset Tree Root",
];
const TRACKS_MENU: [&str; 14] = [
    "Play",
    "Remove Selected",
    "Add to Saved Playlist…",
    "Star/Unstar",
    "Track Info…",
    "Clear Playlist",
    "Show in File Tree",
    "Save Selection As…",
    "Select All",
    "Toggle Queue",
    "Toggle Stop After",
    "Blacklist Song",
    "Blacklist Folder",
    "Edit Tags…",
];
const TAG_FIELDS: [(&str, &str); 13] = [
    ("title", "Title"),
    ("artist", "Artist"),
    ("albumArtist", "Album Artist"),
    ("album", "Album"),
    ("composer", "Composer"),
    ("genre", "Genre"),
    ("year", "Year"),
    ("trackNumber", "Track Number"),
    ("trackTotal", "Track Total"),
    ("discNumber", "Disc Number"),
    ("discTotal", "Disc Total"),
    ("comment", "Comment"),
    ("lyrics", "Lyrics"),
];
const TAG_EDITOR_MENU: [&str; 17] = [
    "Title…",
    "Artist…",
    "Album Artist…",
    "Album…",
    "Composer…",
    "Genre…",
    "Year…",
    "Track Number…",
    "Track Total…",
    "Disc Number…",
    "Disc Total…",
    "Comment…",
    "Lyrics…",
    "Replace Artwork…",
    "Remove Artwork",
    "Save Changes",
    "Cancel",
];
const SAVED_MENU: [&str; 8] = [
    "Add to Current Playlist",
    "Play",
    "Replace Playlist Pane",
    "Rename…",
    "Duplicate…",
    "Delete…",
    "Export as M3U…",
    "Remove Missing Files",
];
const PLAYLIST_MENU: [&str; 6] = [
    "Add File…",
    "Add URL…",
    "Save Current Playlist…",
    "Select All",
    "Clear Playlist",
    "Columns ▶",
];
const COLUMNS_MENU: [&str; 12] = [
    "Sort by Title",
    "Sort by Artist",
    "Sort by Album",
    "Show/Hide Artist",
    "Show/Hide Album",
    "Auto Fit Columns",
    "Show/Hide Columns ▶",
    "Move This Column Left",
    "Move This Column Right",
    "Scroll Columns Left",
    "Scroll Columns Right",
    "Reset Column Layout",
];
const COLUMN_VISIBILITY_MENU: [&str; 22] = [
    "Toggle #",
    "Toggle Star",
    "Toggle Status",
    "Toggle Rating",
    "Toggle Title",
    "Toggle Album Artist",
    "Toggle Artist",
    "Toggle Composer",
    "Toggle Album",
    "Toggle Length",
    "Toggle Size (bytes)",
    "Toggle Size",
    "Toggle Year",
    "Toggle Genre",
    "Toggle Track Number",
    "Toggle Plays",
    "Toggle Path",
    "Toggle Filename",
    "Toggle Codec",
    "Toggle Sample Rate",
    "Toggle Bits",
    "Toggle Bitrate",
];

#[derive(Clone, Copy)]
struct HorizontalScrollbar {
    x: usize,
    width: usize,
    track_width: usize,
    thumb_start: usize,
    thumb_width: usize,
    travel: usize,
    max_scroll: usize,
}

impl HorizontalScrollbar {
    fn new(x: usize, width: usize, total: usize, scroll: usize) -> Option<Self> {
        if width < 5 || total <= width {
            return None;
        }
        let track_width = width - 2;
        let thumb_width = (track_width.saturating_mul(width) / total).clamp(1, track_width - 1);
        let travel = track_width - thumb_width;
        let max_scroll = total - width;
        let thumb_start = scroll.min(max_scroll).saturating_mul(travel) / max_scroll;
        Some(Self {
            x,
            width,
            track_width,
            thumb_start,
            thumb_width,
            travel,
            max_scroll,
        })
    }
}

#[derive(Clone, Copy)]
struct VerticalScrollbar {
    x: usize,
    top: usize,
    height: usize,
    track_height: usize,
    thumb_start: usize,
    thumb_height: usize,
    travel: usize,
    max_scroll: usize,
}

impl VerticalScrollbar {
    fn new(x: usize, top: usize, height: usize, total: usize, scroll: usize) -> Option<Self> {
        if height < 3 || total <= height {
            return None;
        }
        let track_height = height - 2;
        let thumb_height = if track_height == 1 {
            1
        } else {
            (track_height.saturating_mul(height) / total).clamp(1, track_height - 1)
        };
        let travel = track_height - thumb_height;
        let max_scroll = total - height;
        let thumb_start = scroll.min(max_scroll).saturating_mul(travel) / max_scroll;
        Some(Self {
            x,
            top,
            height,
            track_height,
            thumb_start,
            thumb_height,
            travel,
            max_scroll,
        })
    }

    fn offset_at(&self, y: usize, grip: usize) -> usize {
        let position = y.saturating_sub(self.top + 1).min(self.track_height - 1);
        position
            .saturating_sub(grip)
            .min(self.travel)
            .saturating_mul(self.max_scroll)
            / self.travel.max(1)
    }
}

fn paint_vertical_scrollbar(out: &mut String, bar: VerticalScrollbar) {
    paint(out, bar.top + 1, bar.x + 1, "▴", 1, Surface::Header, true);
    for row in 0..bar.track_height {
        let thumb = (bar.thumb_start..bar.thumb_start + bar.thumb_height).contains(&row);
        paint(
            out,
            bar.top + row + 2,
            bar.x + 1,
            if thumb { "┃" } else { "│" },
            1,
            if thumb { Surface::Accent } else { Surface::Header },
            thumb,
        );
    }
    paint(
        out,
        bar.top + bar.height,
        bar.x + 1,
        "▾",
        1,
        Surface::Header,
        true,
    );
}

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
    fn new(
        width: usize,
        height: usize,
        lists: usize,
        files_expanded: bool,
        playlists_expanded: bool,
    ) -> Self {
        let show_sidebar = width >= 60;
        let first = if show_sidebar {
            (width / 3).clamp(23, 46)
        } else {
            0
        };
        let footer_top = height.saturating_sub(4);
        let list_page = if playlists_expanded {
            lists.min(6).min(height.saturating_sub(13)).max(1)
        } else {
            0
        };
        let lists_header = if files_expanded {
            footer_top.saturating_sub(list_page + 1)
        } else {
            4
        };
        let list_top = if playlists_expanded {
            lists_header + 1
        } else {
            footer_top
        };
        let tree_top = 5;
        let tree_bottom = if files_expanded {
            lists_header.saturating_sub(1)
        } else {
            4
        };
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

fn volume_geometry(width: usize, footer_top: usize) -> (usize, usize, usize, usize) {
    let bar_width = if width >= 70 { 12 } else { 8 };
    let row = if width >= 80 {
        footer_top
    } else {
        footer_top + 2
    };
    let icon_x = width.saturating_sub(bar_width + 8);
    (row, icon_x, icon_x + 3, bar_width)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TransportAction {
    Shuffle,
    Previous,
    PlayPause,
    Stop,
    Next,
    Repeat,
    Radio,
}

const FOOTER_TRANSPORT: [TransportAction; 7] = [
    TransportAction::Shuffle,
    TransportAction::Previous,
    TransportAction::PlayPause,
    TransportAction::Stop,
    TransportAction::Next,
    TransportAction::Repeat,
    TransportAction::Radio,
];

fn footer_transport_layout(width: usize) -> (usize, usize) {
    let slot_width = (width / FOOTER_TRANSPORT.len()).clamp(1, 5);
    let total = slot_width * FOOTER_TRANSPORT.len();
    (width.saturating_sub(total) / 2, slot_width)
}

fn footer_transport_slot(width: usize, action: TransportAction) -> Range<usize> {
    let (left, slot_width) = footer_transport_layout(width);
    let index = FOOTER_TRANSPORT
        .iter()
        .position(|candidate| *candidate == action)
        .expect("footer transport action");
    let start = left + index * slot_width;
    start..start + slot_width
}

fn footer_transport_at(width: usize, x: usize) -> Option<TransportAction> {
    FOOTER_TRANSPORT
        .iter()
        .copied()
        .find(|action| footer_transport_slot(width, *action).contains(&x))
}

fn progress_bar_geometry(width: usize, clock: &str) -> (usize, usize, usize) {
    let (left, slot_width) = footer_transport_layout(width);
    let text_start = (left + slot_width).saturating_sub(3);
    let bar_start = text_start + cell_width(clock).max(5) + 2;
    let bar_width = (width / 2).clamp(4, 32);
    (text_start, bar_start, bar_width)
}

struct CompactTransport {
    text: String,
    buttons: Vec<(Range<usize>, TransportAction)>,
}

impl CompactTransport {
    fn action_at(&self, x: usize) -> Option<TransportAction> {
        self.buttons
            .iter()
            .find_map(|(range, action)| range.contains(&x).then_some(*action))
    }
}

fn compact_transport(width: usize, rich: bool, playing: bool) -> CompactTransport {
    let play_icon = if playing { MEDIA_PAUSE } else { MEDIA_PLAY };
    let play_label = if playing { "Pause" } else { "Play" };
    let labels = if rich {
        [
            format!("{MEDIA_PREVIOUS} Previous"),
            format!("{play_icon} {play_label}"),
            format!("{MEDIA_STOP} Stop"),
            format!("{MEDIA_NEXT} Next"),
        ]
    } else if width >= 36 {
        [
            format!("{MEDIA_PREVIOUS} Prev"),
            format!("{play_icon} {play_label}"),
            format!("{MEDIA_STOP} Stop"),
            format!("{MEDIA_NEXT} Next"),
        ]
    } else {
        [
            MEDIA_PREVIOUS.to_owned(),
            play_icon.to_owned(),
            MEDIA_STOP.to_owned(),
            MEDIA_NEXT.to_owned(),
        ]
    };
    let separator = if rich { "   " } else { "  " };
    let actions = [
        TransportAction::Previous,
        TransportAction::PlayPause,
        TransportAction::Stop,
        TransportAction::Next,
    ];
    let mut text = String::new();
    let mut buttons = Vec::with_capacity(actions.len());
    let mut x = 0;
    for (index, (label, action)) in labels.iter().zip(actions).enumerate() {
        if index > 0 {
            text.push_str(separator);
            x += cell_width(separator);
        }
        let end = x + cell_width(label);
        buttons.push((x..end, action));
        text.push_str(label);
        x = end;
    }
    CompactTransport { text, buttons }
}

fn menu_width(terminal_width: usize) -> usize {
    if terminal_width < 66 {
        terminal_width.div_ceil(2).max(18).min(terminal_width)
    } else {
        33
    }
}

fn menu_row(label: &str, shortcut: Option<char>, width: usize) -> String {
    if label.is_empty() {
        return format!("│ {}", "─".repeat(width.saturating_sub(3)));
    }
    let hint = shortcut.map_or_else(String::new, |key| format!("Alt+{key}"));
    let normalized = label.split_whitespace().collect::<Vec<_>>().join(" ");
    let (name, arrow) = if let Some(name) = normalized
        .strip_suffix('›')
        .or_else(|| normalized.strip_suffix('▶'))
    {
        (name.trim_end(), " ›")
    } else {
        (normalized.as_str(), "")
    };
    let label_width = width
        .saturating_sub(3)
        .saturating_sub(hint.len() + 1 + cell_width(arrow));
    format!("│ {}{arrow} {hint}", truncate(name, label_width))
}

fn truncate(text: &str, width: usize) -> String {
    let clipped = cell_width(text) > width;
    let limit = if clipped {
        width.saturating_sub(1)
    } else {
        width
    };
    let mut out = String::new();
    let mut used = 0;
    for grapheme in text.graphemes(true) {
        if grapheme.chars().any(char::is_control) {
            continue;
        }
        let cells = UnicodeWidthStr::width(grapheme);
        if used + cells > limit {
            break;
        }
        out.push_str(grapheme);
        used += cells;
    }
    if clipped && width > 0 {
        out.push('…');
        used += 1;
    }
    out.push_str(&" ".repeat(width.saturating_sub(used)));
    out
}

fn saved_list_label(name: &str, favorite: bool, count: usize, width: usize) -> String {
    let suffix = format!("{count} ");
    if suffix.len() >= width {
        return truncate(&suffix, width);
    }
    let prefix = if favorite { " ★  " } else { "    " };
    format!(
        "{}{}",
        truncate(&format!("{prefix}{name}"), width - suffix.len()),
        suffix
    )
}

fn marquee_window(value: &str, width: usize, tick: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let total = cell_width(value);
    if total <= width {
        return truncate(value, width);
    }
    let travel = total - width;
    const DWELL: usize = 6;
    let period = 2 * (travel + DWELL);
    let phase = tick % period;
    let offset = if phase < DWELL {
        0
    } else if phase < DWELL + travel {
        phase - DWELL + 1
    } else if phase < 2 * DWELL + travel {
        travel
    } else {
        travel - (phase - (2 * DWELL + travel) + 1)
    };
    let visible = cell_slice(value, offset, width);
    format!("{visible}{}", " ".repeat(width.saturating_sub(cell_width(&visible))))
}

fn marquee_prefixed(prefix: &str, value: &str, width: usize, tick: usize) -> String {
    format!(
        "{prefix}{}",
        marquee_window(value, width.saturating_sub(cell_width(prefix)), tick)
    )
}

fn saved_list_marquee_label(
    name: &str,
    favorite: bool,
    count: usize,
    width: usize,
    tick: usize,
) -> String {
    let prefix = if favorite { " ★  " } else { "    " };
    let suffix = format!("{count} ");
    let available = width.saturating_sub(cell_width(prefix) + cell_width(&suffix));
    if available == 0 {
        return saved_list_label(name, favorite, count, width);
    }
    format!("{prefix}{}{suffix}", marquee_window(name, available, tick))
}

fn cell_slice(text: &str, skip: usize, width: usize) -> String {
    let mut out = String::new();
    let mut position = 0;
    for grapheme in text.graphemes(true) {
        let cells = UnicodeWidthStr::width(grapheme);
        if position >= skip + width {
            break;
        }
        if position + cells > skip && position < skip + width {
            if position < skip || position + cells > skip + width {
                out.push_str(
                    &" ".repeat((position + cells).min(skip + width) - position.max(skip)),
                );
            } else {
                out.push_str(grapheme);
            }
        }
        position += cells;
    }
    out
}

fn paint_playlist_cell(
    out: &mut String,
    row: usize,
    viewport_x: usize,
    viewport_width: usize,
    column_start: usize,
    column_width: usize,
    scroll: usize,
    value: &str,
    marquee_tick: usize,
    surface: Surface,
    bold: bool,
) {
    let visible_left = column_start.saturating_sub(scroll);
    let clipped_left = scroll.saturating_sub(column_start);
    if visible_left >= viewport_width || clipped_left >= column_width {
        return;
    }
    let width = column_width
        .saturating_sub(clipped_left)
        .min(viewport_width - visible_left);
    let content_width = column_width.saturating_sub(1);
    let cell = if cell_width(value) > content_width {
        marquee_window(value, content_width, marquee_tick)
    } else {
        value.to_owned()
    };
    paint(
        out,
        row,
        viewport_x + visible_left,
        &cell_slice(&cell, clipped_left, width),
        width,
        surface,
        bold,
    );
}

#[derive(Clone, Copy)]
enum Surface {
    Toolbar,
    Sidebar,
    SidebarAlt,
    Main,
    Input,
    MainAlt,
    Header,
    Accent,
    AccentSidebar,
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
        Surface::Input => ("240;244;248", "39;44;48"),
        Surface::MainAlt => ("220;224;228", "32;35;37"),
        Surface::Header => ("225;230;235", "29;32;34"),
        Surface::Accent => ("103;179;233", "29;32;34"),
        Surface::AccentSidebar => ("103;179;233", "34;37;39"),
        Surface::Selected => ("245;248;251", "49;84;106"),
        Surface::Muted => ("151;160;168", "29;32;34"),
    };
    out.push_str(&format!(
        "\x1b[{row};{col}H\x1b[{};38;2;{foreground};48;2;{background}m{}\x1b[0m",
        if bold { "1" } else { "22" },
        truncate(text, width)
    ));
}

fn draw_folder_chooser(
    screen: &mut String,
    chooser: &mut FolderChooser,
    size: (usize, usize),
    marquee_tick: usize,
) {
    let geometry = FolderGeometry::new(size);
    let (x, y, width, height) = (geometry.x, geometry.y, geometry.width, geometry.height);
    let inner = width.saturating_sub(2);
    for row in 0..height {
        paint(
            screen,
            y + row + 1,
            x + 1,
            &" ".repeat(width),
            width,
            Surface::Toolbar,
            false,
        );
        if row > 0 && row + 1 < height {
            paint(screen, y + row + 1, x + 1, "│", 1, Surface::Header, false);
            paint(
                screen,
                y + row + 1,
                x + width,
                "│",
                1,
                Surface::Header,
                false,
            );
        }
    }
    paint(
        screen,
        y + 1,
        x + 1,
        &format!("╭{}╮", "─".repeat(inner)),
        width,
        Surface::Header,
        true,
    );
    paint(
        screen,
        y + 1,
        x + 3,
        " Select Music Folder ",
        inner.saturating_sub(2),
        Surface::Header,
        true,
    );
    paint(
        screen,
        y + height,
        x + 1,
        &format!("╰{}╯", "─".repeat(inner)),
        width,
        Surface::Header,
        false,
    );
    paint(
        screen,
        y + 2,
        x + 3,
        "Location:",
        inner.saturating_sub(2),
        Surface::Toolbar,
        true,
    );
    let field_width = width.saturating_sub(4);
    let (shown, cursor) = input_window(&chooser.location, chooser.cursor, field_width);
    paint(
        screen,
        y + 3,
        x + 3,
        &format!("{shown:<field_width$}"),
        field_width,
        if chooser.focus == FolderFocus::Location && chooser.select_all {
            Surface::Selected
        } else {
            Surface::Input
        },
        false,
    );
    let toolbar = if width >= 44 {
        format!(
            "[Home]  [Up]  [Root]  [{} Hidden]",
            if chooser.show_hidden { "✓" } else { " " }
        )
    } else {
        format!(
            "Home   Up   /   Hidden {}",
            if chooser.show_hidden { "✓" } else { " " }
        )
    };
    paint(
        screen,
        y + 4,
        x + 3,
        &toolbar,
        field_width,
        Surface::Toolbar,
        false,
    );
    let list_rows = geometry.list_rows();
    chooser.keep_selected_visible(list_rows);
    if chooser.entries.is_empty() {
        paint(
            screen,
            y + 5,
            x + 3,
            "No subfolders",
            field_width,
            Surface::Muted,
            false,
        );
    }
    for (row, entry) in chooser
        .entries
        .iter()
        .skip(chooser.offset)
        .take(list_rows)
        .enumerate()
    {
        let index = chooser.offset + row;
        let chosen = chooser.focus == FolderFocus::List && index == chooser.selected;
        let icon = if entry.parent { "↰" } else { "▱" };
        let prefix = format!(
            "{} {icon} ",
            if index == chooser.selected { "▸" } else { " " }
        );
        let label = marquee_prefixed(&prefix, &entry.name, field_width, marquee_tick);
        paint(
            screen,
            geometry.list_top() + row + 1,
            x + 3,
            &label,
            field_width,
            if chosen {
                Surface::Selected
            } else if row % 2 == 0 {
                Surface::Main
            } else {
                Surface::MainAlt
            },
            chosen,
        );
    }
    let hint = if chooser.error.is_empty() {
        "Enter open · Backspace parent · Ctrl+L path · Ctrl+W erase word"
    } else {
        &chooser.error
    };
    paint(
        screen,
        y + height - 2,
        x + 3,
        hint,
        field_width,
        if chooser.error.is_empty() {
            Surface::Muted
        } else {
            Surface::Accent
        },
        false,
    );
    let compact = width < 36;
    let cancel = if compact { "[Esc]" } else { "[ Cancel ]" };
    let choose = if compact {
        "[Choose]"
    } else {
        "[ Choose This Folder ]"
    };
    let spacing = if compact { 1 } else { 2 };
    let choose_x = x + width.saturating_sub(choose.len() + spacing);
    let cancel_x = choose_x.saturating_sub(cancel.len() + spacing);
    paint(
        screen,
        y + height - 1,
        cancel_x + 1,
        cancel,
        cancel.len(),
        if chooser.focus == FolderFocus::Cancel {
            Surface::Selected
        } else {
            Surface::Toolbar
        },
        chooser.focus == FolderFocus::Cancel,
    );
    paint(
        screen,
        y + height - 1,
        choose_x + 1,
        choose,
        choose.len(),
        if chooser.focus == FolderFocus::Choose {
            Surface::Selected
        } else {
            Surface::Accent
        },
        chooser.focus == FolderFocus::Choose,
    );
    if chooser.focus == FolderFocus::Location {
        screen.push_str(&format!("\x1b[{};{}H\x1b[?25h", y + 3, x + 3 + cursor));
    }
}

fn paint_cover_preview(out: &mut String, row: usize, col: usize, cover: &CoverPreview) {
    paint_cover_at_size(out, row, col, cover, 8);
}

fn paint_cover_at_size(
    out: &mut String,
    row: usize,
    col: usize,
    cover: &CoverPreview,
    width: usize,
) {
    let width = width.min(COVER_WIDTH);
    for y in 0..width / 2 {
        for x in 0..width {
            let upper = cover_pixel_at_size(cover, x, y * 2, width);
            let lower = cover_pixel_at_size(cover, x, y * 2 + 1, width);
            out.push_str(&format!(
                "\x1b[{};{}H\x1b[38;2;{};{};{};48;2;{};{};{}m▀\x1b[0m",
                row + y,
                col + x,
                upper[0],
                upper[1],
                upper[2],
                lower[0],
                lower[1],
                lower[2],
            ));
        }
    }
}

fn cover_pixel_at_size(cover: &CoverPreview, x: usize, y: usize, width: usize) -> [u8; 3] {
    let left = x * COVER_WIDTH / width;
    let right = ((x + 1) * COVER_WIDTH / width).max(left + 1);
    let top = y * COVER_HEIGHT / width;
    let bottom = ((y + 1) * COVER_HEIGHT / width).max(top + 1);
    let mut sum = [0_u32; 3];
    for source_y in top..bottom {
        for source_x in left..right {
            let pixel = cover.pixels[source_y * COVER_WIDTH + source_x];
            for channel in 0..3 {
                sum[channel] += u32::from(pixel[channel]);
            }
        }
    }
    let count = ((right - left) * (bottom - top)) as u32;
    sum.map(|channel| (channel / count) as u8)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    Char(char),
    Alt(char),
    CtrlA,
    CtrlC,
    CtrlL,
    CtrlO,
    CtrlR,
    CtrlSpace,
    CtrlU,
    CtrlW,
    CtrlBackspace,
    CtrlLeft,
    CtrlRight,
    AltUp,
    AltDown,
    ContextMenu,
    F10,
    Esc,
    Tab,
    BackTab,
    Enter,
    Backspace,
    Delete,
    Up,
    Down,
    ShiftUp,
    ShiftDown,
    CtrlUp,
    CtrlDown,
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
        if matches!(bytes[1], 8 | 127) {
            bytes.drain(..2);
            return Some(Event::Key(Key::CtrlBackspace));
        }
        if bytes[1] != b'[' {
            if bytes[1].is_ascii_graphic() || bytes[1] == b' ' {
                let key = bytes[1] as char;
                bytes.drain(..2);
                return Some(Event::Key(Key::Alt(key)));
            }
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
            b"1;2A" => Key::ShiftUp,
            b"1;2B" => Key::ShiftDown,
            b"1;5A" => Key::CtrlUp,
            b"1;5B" => Key::CtrlDown,
            b"1;3A" => Key::AltUp,
            b"1;3B" => Key::AltDown,
            b"1;5D" => Key::CtrlLeft,
            b"1;5C" => Key::CtrlRight,
            b"127;5u" | b"8;5u" => Key::CtrlBackspace,
            b"32;5u" => Key::CtrlSpace,
            b"21;2~" | b"29~" => Key::ContextMenu,
            b"21~" => Key::F10,
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
        1 => Key::CtrlA,
        3 => Key::CtrlC,
        12 => Key::CtrlL,
        15 => Key::CtrlO,
        18 => Key::CtrlR,
        0 => Key::CtrlSpace,
        21 => Key::CtrlU,
        23 => Key::CtrlW,
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
    output: std::fs::File,
    stderr_backup: std::fs::File,
}
#[cfg(unix)]
impl Terminal {
    fn open() -> Result<Self, String> {
        use std::os::fd::AsRawFd;

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
        let (output, stderr_backup) = Self::redirect_diagnostics()?;
        let mut raw = original;
        unsafe { libc::cfmakeraw(&mut raw) };
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 0;
        if unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw) } != 0 {
            let error = io::Error::last_os_error();
            unsafe {
                libc::dup2(output.as_raw_fd(), libc::STDOUT_FILENO);
                libc::dup2(stderr_backup.as_raw_fd(), libc::STDERR_FILENO);
            }
            return Err(format!("setting terminal mode: {error}"));
        }
        let mut terminal = Self {
            original,
            output,
            stderr_backup,
        };
        terminal.write("\x1b[?1049h\x1b[2J\x1b[?1000h\x1b[?1002h\x1b[?1006h\x1b[?25l")?;
        Ok(terminal)
    }

    fn redirect_diagnostics() -> Result<(std::fs::File, std::fs::File), String> {
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

        let path = kog_audio::settings::setting_path("tui-diagnostics.log")
            .ok_or("No settings directory for TUI diagnostics")?;
        std::fs::create_dir_all(path.parent().ok_or("Invalid TUI diagnostics path")?)
            .map_err(|error| format!("creating TUI diagnostics directory: {error}"))?;
        if path.exists() {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                .map_err(|error| format!("securing TUI diagnostics: {error}"))?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(path)
            .map_err(|error| format!("opening TUI diagnostics: {error}"))?;
        // Keep a private terminal descriptor for frames. Native decoders and
        // child processes may write to either standard stream at any time.
        let output_fd = unsafe { libc::dup(libc::STDOUT_FILENO) };
        if output_fd < 0 {
            return Err(format!(
                "duplicating terminal output: {}",
                io::Error::last_os_error()
            ));
        }
        let output = unsafe { std::fs::File::from_raw_fd(output_fd) };
        let stderr_fd = unsafe { libc::dup(libc::STDERR_FILENO) };
        if stderr_fd < 0 {
            return Err(format!(
                "duplicating terminal errors: {}",
                io::Error::last_os_error()
            ));
        }
        let stderr_backup = unsafe { std::fs::File::from_raw_fd(stderr_fd) };
        unsafe { libc::fflush(std::ptr::null_mut()) };
        if unsafe { libc::dup2(file.as_raw_fd(), libc::STDOUT_FILENO) } < 0 {
            return Err(format!(
                "redirecting TUI diagnostics: {}",
                io::Error::last_os_error()
            ));
        }
        if unsafe { libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO) } < 0 {
            let error = io::Error::last_os_error();
            unsafe { libc::dup2(output.as_raw_fd(), libc::STDOUT_FILENO) };
            return Err(format!("redirecting TUI diagnostics: {error}"));
        }
        Ok((output, stderr_backup))
    }

    fn write(&mut self, frame: &str) -> Result<(), String> {
        self.output
            .write_all(frame.as_bytes())
            .map_err(|error| error.to_string())?;
        self.output.flush().map_err(|error| error.to_string())
    }

    fn size(&self) -> (usize, usize) {
        let mut size = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        use std::os::fd::AsRawFd;
        if unsafe { libc::ioctl(self.output.as_raw_fd(), libc::TIOCGWINSZ, &mut size) } == 0 {
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
        use std::os::fd::AsRawFd;
        let _ = self.write("\x1b[?25h\x1b[?1006l\x1b[?1002l\x1b[?1000l\x1b[?1049l");
        unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.original) };
        unsafe {
            libc::fflush(std::ptr::null_mut());
            libc::dup2(self.output.as_raw_fd(), libc::STDOUT_FILENO);
            libc::dup2(self.stderr_backup.as_raw_fd(), libc::STDERR_FILENO);
        }
    }
}

#[cfg(unix)]
pub fn run() -> Result<(), String> {
    let mut terminal = Terminal::open()?;
    let mut ui = Ui::new();
    let mut input = Vec::new();
    let mut last_size = (0, 0);
    let mut last_draw = Instant::now() - Duration::from_secs(1);
    let mut last_frame = String::new();
    let mut escape_pending = None::<Instant>;
    let mut last_session_flush = Instant::now();
    loop {
        ui.poll_search_due();
        ui.poll_search();
        ui.poll_folders();
        ui.poll_remote();
        ui.poll_deletes();
        ui.poll_tags();
        ui.poll_rom_imports();
        ui.poll_api_server();
        ui.poll_metadata();
        ui.poll_cover_preview();
        ui.poll_radio();
        if last_session_flush.elapsed() >= Duration::from_secs(1) {
            if let Err(error) = ui.flush_session() {
                ui.status = format!("Could not save playlist: {error}");
            }
            last_session_flush = Instant::now();
        }
        let size = terminal.size();
        if size != last_size || last_draw.elapsed() >= Duration::from_millis(150) {
            let frame = ui.draw(size);
            if frame != last_frame {
                terminal.write(&frame)?;
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
                if !ui.key(Key::Esc, size) {
                    return ui.finish_session();
                }
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
                        return ui.finish_session();
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
                return ui.finish_session();
            }
            let frame = ui.draw(size);
            if frame != last_frame {
                terminal.write(&frame)?;
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
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn folder_chooser_browses_directories_and_edits_path_components() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("Album With Spaces")).unwrap();
        std::fs::create_dir(root.path().join(".private")).unwrap();
        std::fs::write(root.path().join("song.mp3"), b"audio").unwrap();
        let mut chooser = FolderChooser::new(root.path()).unwrap();
        assert!(
            chooser
                .entries
                .iter()
                .any(|entry| entry.name == "Album With Spaces")
        );
        assert!(
            !chooser
                .entries
                .iter()
                .any(|entry| entry.name == "song.mp3" || entry.name == ".private")
        );
        chooser.toggle_hidden();
        assert!(chooser.entries.iter().any(|entry| entry.name == ".private"));
        chooser.focus = FolderFocus::Location;
        chooser.location = format!("{}/Album With Spaces/other", root.path().display());
        chooser.cursor = chooser.location.chars().count();
        chooser.edit_location(Key::CtrlW);
        assert!(chooser.location.ends_with("/Album With Spaces/"));
        chooser.edit_location(Key::CtrlW);
        assert!(chooser.location.ends_with("/Album With "));
        chooser.edit_location(Key::CtrlBackspace);
        assert_eq!(chooser.location, format!("{}/", root.path().display()));
        chooser.navigate_location();
        assert_eq!(chooser.directory, root.path().canonicalize().unwrap());
        chooser.location = root.path().join("missing").to_string_lossy().into_owned();
        chooser.navigate_location();
        assert!(!chooser.error.is_empty());
        assert_eq!(chooser.directory, root.path().canonicalize().unwrap());
    }

    #[test]
    fn terminal_session_preserves_order_fragments_and_names() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("tui-session.json");
        let tracks = vec![
            Track {
                name: "Archive title [2]".to_owned(),
                entry: StoredEntry {
                    kind: "archive".to_owned(),
                    path: "/music/collection.zip".to_owned(),
                    entry: "nested/song.nsf".to_owned(),
                    fragment: Some("1".to_owned()),
                },
            },
            Track {
                name: "Remote title".to_owned(),
                entry: StoredEntry {
                    kind: "remote".to_owned(),
                    path: "https://example.invalid/audio?id=3".to_owned(),
                    entry: String::new(),
                    fragment: None,
                },
            },
        ];
        save_playlist_session(&path, &tracks, 1).unwrap();
        #[cfg(unix)]
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let restored = load_playlist_session(&path).unwrap();
        assert_eq!(restored.selected, 1);
        assert_eq!(restored.tracks.len(), 2);
        assert_eq!(restored.tracks[0].name, "Archive title [2]");
        assert_eq!(restored.tracks[0].entry.entry, "nested/song.nsf");
        assert_eq!(restored.tracks[0].entry.fragment.as_deref(), Some("1"));
        assert_eq!(restored.tracks[1].entry.path, tracks[1].entry.path);
        assert_eq!(
            std::fs::read_to_string(&path)
                .unwrap()
                .matches("Remote title")
                .count(),
            1
        );
    }

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
        let mut right_and_horizontal = b"\x1b[<2;60;5M\x1b[<67;61;6M".to_vec();
        assert!(matches!(
            parse_event(&mut right_and_horizontal),
            Some(Event::Mouse {
                button: 2,
                x: 59,
                y: 4,
                release: false
            })
        ));
        assert!(matches!(
            parse_event(&mut right_and_horizontal),
            Some(Event::Mouse {
                button: 67,
                x: 60,
                y: 5,
                release: false
            })
        ));
        let mut unicode = "é".as_bytes().to_vec();
        assert!(matches!(
            parse_event(&mut unicode),
            Some(Event::Key(Key::Char('é')))
        ));
        let mut folder_keys = b"\x17\x1b\x7f\x1b[1;3A\x1b[1;5D".to_vec();
        for expected in [Key::CtrlW, Key::CtrlBackspace, Key::AltUp, Key::CtrlLeft] {
            assert!(
                matches!(parse_event(&mut folder_keys), Some(Event::Key(key)) if key == expected)
            );
        }
        let mut keyboard_keys = b"\x00\x12\x1b[21;2~\x1b[21~\x1b[1;3B".to_vec();
        for expected in [
            Key::CtrlSpace,
            Key::CtrlR,
            Key::ContextMenu,
            Key::F10,
            Key::AltDown,
        ] {
            assert!(
                matches!(parse_event(&mut keyboard_keys), Some(Event::Key(key)) if key == expected)
            );
        }
        let mut alt_keys = b"\x1bv\x1bP".to_vec();
        assert!(matches!(
            parse_event(&mut alt_keys),
            Some(Event::Key(Key::Alt('v')))
        ));
        assert!(matches!(
            parse_event(&mut alt_keys),
            Some(Event::Key(Key::Alt('P')))
        ));
    }
    #[test]
    fn every_menu_item_has_a_unique_visible_alt_shortcut() {
        for page in [
            MenuPage::Main,
            MenuPage::View,
            MenuPage::Playback,
            MenuPage::Preferences,
            MenuPage::Tree,
            MenuPage::Tracks,
            MenuPage::Saved,
            MenuPage::Playlist,
            MenuPage::Columns,
            MenuPage::ColumnVisibility,
            MenuPage::TagEditor,
            MenuPage::Remote,
            MenuPage::Synthesis,
            MenuPage::Server,
        ] {
            let shortcuts = menu_shortcuts(page);
            assert_eq!(shortcuts.len(), page.labels().len());
            let mut seen = HashSet::new();
            for (index, (&label, shortcut)) in page.labels().iter().zip(shortcuts).enumerate() {
                if label.is_empty() {
                    assert_eq!(shortcut, None);
                    continue;
                }
                let shortcut = shortcut.expect("menu item must have an Alt shortcut");
                assert!(seen.insert(shortcut));
                assert_eq!(menu_shortcut_index(page, shortcut), Some(index));
                for width in [18, 33] {
                    let row = menu_row(label, Some(shortcut), width);
                    assert!(row.contains(&format!("Alt+{shortcut}")));
                    assert!(cell_width(&row) <= width - 1);
                }
            }
        }
        assert_eq!(menu_shortcut_index(MenuPage::Main, 'v'), Some(9));
        assert_eq!(menu_shortcut_index(MenuPage::Main, 'p'), Some(11));
    }
    #[test]
    fn seek_time_accepts_long_tracks_and_rejects_invalid_fields() {
        assert_eq!(parse_seek_position("8:01:03"), Some(28_863));
        assert_eq!(parse_seek_position("83:59"), Some(5_039));
        assert_eq!(parse_seek_position("125"), Some(125));
        assert_eq!(parse_seek_position("1:60"), None);
        assert_eq!(parse_seek_position("1:01:60"), None);
        assert_eq!(parse_seek_position("1::03"), None);
    }
    #[test]
    fn transport_hitboxes_follow_drawn_controls() {
        for width in [20, 30, 48, 70, 72, 80, 120] {
            for action in FOOTER_TRANSPORT {
                let slot = footer_transport_slot(width, action);
                assert!(slot.end - slot.start >= 2 && slot.end <= width);
                assert_eq!(footer_transport_at(width, slot.start), Some(action));
                assert_eq!(footer_transport_at(width, slot.end - 1), Some(action));
            }
            let (volume_row, icon_x, bar_x, bar_width) = volume_geometry(width, 36);
            if volume_row == 36 {
                assert!(footer_transport_slot(width, TransportAction::Radio).end <= icon_x);
            }
            assert!(bar_x + bar_width <= width);
            let (_, bar_start, bar_width) = progress_bar_geometry(width, "8:01:03");
            assert!(bar_start < bar_start + bar_width);
        }
        for (width, rich) in [(20, false), (26, false), (36, false), (44, false), (51, true)] {
            for playing in [false, true] {
                let transport = compact_transport(width, rich, playing);
                assert!(cell_width(&transport.text) <= width);
                for (range, action) in &transport.buttons {
                    assert!(range.end <= width);
                    assert_eq!(transport.action_at(range.start), Some(*action));
                    assert_eq!(transport.action_at(range.end - 1), Some(*action));
                }
                assert_eq!(transport.action_at(width + 10), None);
            }
        }
    }
    #[test]
    fn prompt_word_navigation_handles_names_and_paths() {
        assert_eq!(next_path_word_boundary("Some album/file.wav", 0), 5);
        assert_eq!(next_path_word_boundary("Some album/file.wav", 5), 11);
        assert_eq!(previous_path_word_boundary("Some album/file.wav", 11), 5);
    }
    #[test]
    fn truncate_handles_wide_unicode() {
        assert_eq!(truncate("♫ 漢字", 4), "♫ … ");
        for icon in [
            MEDIA_PLAY,
            MEDIA_PAUSE,
            MEDIA_STOP,
            MEDIA_PREVIOUS,
            MEDIA_NEXT,
            MEDIA_SHUFFLE,
            MEDIA_REPEAT,
            MEDIA_REPEAT_ONE,
            MEDIA_RADIO,
        ] {
            assert_eq!(cell_width(icon), 2, "{icon}");
            assert_eq!(truncate(icon, 2), icon);
        }
        assert_eq!(cell_slice("▶️ Song", 0, 2), MEDIA_PLAY);
        assert_eq!(cell_slice("▶️ Song", 1, 2), "  ");
        assert_eq!(truncate("▶️ Song", 3), "▶️…");
    }
    #[test]
    fn saved_playlist_count_stays_visible_when_name_is_clipped() {
        let label = saved_list_label("Long 漢字 Playlist", false, 42, 18);
        assert!(label.contains('…'));
        assert!(label.ends_with("42 "));
        assert_eq!(
            label.chars().map(|c| c.width().unwrap_or(0)).sum::<usize>(),
            18
        );

        let favorite = saved_list_label("Favorites", true, 1, 24);
        assert!(favorite.contains("★  Favorites"));
        assert!(favorite.ends_with("1 "));
    }
    #[test]
    fn layout_reflows_at_terminal_widths() {
        let wide = Layout::new(120, 40, 5, true, true);
        let narrow = Layout::new(48, 18, 5, true, true);
        assert!(wide.show_sidebar);
        assert!(!narrow.show_sidebar);
        assert_eq!(wide.first, 40);
        assert_eq!(narrow.first, 0);
        assert!(wide.tree_bottom < wide.lists_header);
        assert_eq!(wide.list_top + wide.list_page, wide.footer_top);
        assert_eq!(narrow.footer_top, 14);
    }

    #[test]
    fn horizontal_scrollbar_reaches_both_ends() {
        let left = HorizontalScrollbar::new(41, 79, 160, 0).unwrap();
        let right = HorizontalScrollbar::new(41, 79, 160, 81).unwrap();
        assert_eq!(left.thumb_start, 0);
        assert_eq!(right.thumb_start, right.travel);
        assert!(right.thumb_width < right.track_width);
        assert!(HorizontalScrollbar::new(41, 79, 79, 0).is_none());
    }

    #[test]
    fn vertical_scrollbar_reaches_both_ends_and_preserves_drag_grip() {
        let top = VerticalScrollbar::new(39, 5, 10, 50, 0).unwrap();
        let bottom = VerticalScrollbar::new(39, 5, 10, 50, 40).unwrap();
        assert_eq!(top.thumb_start, 0);
        assert_eq!(bottom.thumb_start, bottom.travel);
        assert_eq!(top.offset_at(top.top + 1, 0), 0);
        assert_eq!(bottom.offset_at(bottom.top + bottom.track_height, 0), 40);
        let gripped = VerticalScrollbar::new(39, 5, 10, 20, 10).unwrap();
        assert_eq!(
            gripped.offset_at(gripped.top + gripped.height - 2, gripped.thumb_height - 1),
            gripped.max_scroll
        );
        assert!(VerticalScrollbar::new(39, 5, 10, 10, 0).is_none());
        assert!(VerticalScrollbar::new(39, 5, 3, 50, 0).is_some());
        assert!(VerticalScrollbar::new(39, 5, 2, 50, 0).is_none());
        let mut painted = String::new();
        paint_vertical_scrollbar(&mut painted, top);
        for glyph in ["▴", "▾", "┃", "│"] {
            assert!(painted.contains(glyph));
        }
    }

    #[test]
    fn nsf_expansion_keeps_each_subsong_visible() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("game.nsf");
        let mut bytes = vec![0_u8; 128];
        bytes[0..5].copy_from_slice(b"NESM\x1a");
        bytes[5] = 1;
        bytes[6] = 3;
        bytes[7] = 1;
        bytes[8..10].copy_from_slice(&0x8000_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&0x8000_u16.to_le_bytes());
        bytes[12..14].copy_from_slice(&0x8001_u16.to_le_bytes());
        bytes.extend_from_slice(&[0x60, 0x60]);
        std::fs::write(&path, bytes).unwrap();
        let track = Track {
            name: "game.nsf".to_owned(),
            entry: StoredEntry {
                kind: "local".to_owned(),
                path: path.display().to_string(),
                entry: String::new(),
                fragment: None,
            },
        };
        let expanded = expand_track(
            &DecoderRegistry::new(AppSettings::load().decoder_settings()),
            Some(root.path()),
            track,
        );
        assert_eq!(expanded.len(), 3);
        for (index, track) in expanded.iter().enumerate() {
            assert_eq!(
                track.entry.fragment.as_deref(),
                Some(["0", "1", "2"][index])
            );
            assert_eq!(display_title(track), format!("game [{}]", index + 1));
        }
    }

    #[test]
    fn saved_remote_subsong_recovers_its_title() {
        let track = track_from_entry(StoredEntry {
            kind: "remote".to_owned(),
            path: "https://example.test/api/stream?kind=archive&path=%2Fmusic%2Fpack.zip&entry=inner%2Fgame.nsf&fragment=2&token=hidden".to_owned(),
            entry: String::new(),
            fragment: None,
        });
        assert_eq!(track.name, "game.nsf [3]");
        assert_eq!(display_title(&track), "game [3]");
    }

    #[test]
    fn folder_scan_respects_cue_and_playlist_preferences() {
        let directory = tempfile::tempdir().unwrap();
        let wav_path = directory.path().join("song.wav");
        let samples = vec![0_u8; 800];
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + samples.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8_000_u32.to_le_bytes());
        wav.extend_from_slice(&8_000_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(samples.len() as u32).to_le_bytes());
        wav.extend_from_slice(&samples);
        std::fs::write(&wav_path, wav).unwrap();
        std::fs::write(
            directory.path().join("album.cue"),
            "FILE \"song.wav\" WAVE\n  TRACK 01 AUDIO\n    INDEX 01 00:00:00\n",
        )
        .unwrap();
        std::fs::write(directory.path().join("album.m3u"), "song.wav\n").unwrap();
        let database = kog_core::db::LibraryDb::open_in_memory().unwrap();
        let library = Arc::new(Library::with_read_playlists_in_folders(
            Some(directory.path().to_path_buf()),
            database,
            false,
        ));
        let decoders = DecoderRegistry::new(AppSettings::load().decoder_settings());
        let without = collect_folder(
            &library,
            &decoders,
            directory.path().to_path_buf(),
            false,
            false,
        )
        .unwrap();
        assert_eq!(without.len(), 1);
        assert_eq!(without[0].entry.path, wav_path.display().to_string());
        library.set_read_playlists_in_folders(true);
        let with_cue = collect_folder(
            &library,
            &decoders,
            directory.path().to_path_buf(),
            true,
            true,
        )
        .unwrap();
        assert!(
            with_cue.iter().any(|track| track.entry.fragment.is_some()),
            "{with_cue_len} tracks",
            with_cue_len = with_cue.len()
        );
        assert_eq!(with_cue.len(), 2);
        assert!(
            with_cue
                .iter()
                .any(|track| track.entry.path.ends_with("album.cue")
                    && track.entry.fragment.as_deref() == Some("1"))
        );
    }
}
