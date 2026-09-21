//! Library, playlist and star endpoints.
//!
//! With per-client streams the client is the player, so the server owns no
//! playback state: it exposes the library, the user's playlists and stars, and
//! the audio stream itself. That keeps the API stateless per request and lets
//! any number of clients browse and listen independently.
//!
//! Browsing is confined to the configured music directory: every requested
//! path is canonicalised and checked to be inside it, so the API cannot be used
//! to read arbitrary files on the host.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use kog_audio::decoder::{PlaybackSource, StreamProperties};
use kog_audio::playlist::{Playlist, PlaylistEntry, PlaylistLocation};
use kog_core::db::{LibraryDb, StoredEntry};

use crate::routes::{AppState, bad_request};

/// Shared library access for the handlers.
pub struct Library {
    root: Mutex<Option<PathBuf>>,
    db: Mutex<LibraryDb>,
    /// Tag lookups are the expensive part of a playlist refresh, so the server
    /// remembers each answer (including "no metadata") across requests.
    metadata: Mutex<MetadataCache>,
    /// Mirrors `AppSettings::read_playlists_in_folders`: when a folder is
    /// browsed, playlist files are expanded into their entries instead of being
    /// offered as track files. The desktop applies the same preference while
    /// scanning a folder. Defaults to on, matching the setting's default.
    read_playlists_in_folders: Mutex<bool>,
}

impl Library {
    pub fn new(root: Option<PathBuf>, db: LibraryDb) -> Self {
        Self::with_read_playlists_in_folders(root, db, true)
    }

    pub fn with_read_playlists_in_folders(
        root: Option<PathBuf>,
        db: LibraryDb,
        read_playlists_in_folders: bool,
    ) -> Self {
        Self {
            root: Mutex::new(root),
            db: Mutex::new(db),
            metadata: Mutex::new(MetadataCache::default()),
            read_playlists_in_folders: Mutex::new(read_playlists_in_folders),
        }
    }

    /// Open the real library: the music directory from settings and Kog's
    /// database, falling back to an in-memory store when it cannot be opened
    /// (the server still serves streams and the codec list).
    pub fn open() -> Self {
        let settings = kog_audio::settings::AppSettings::load();
        let db = LibraryDb::open().or_else(|_| LibraryDb::open_in_memory());
        Self::with_read_playlists_in_folders(
            settings.music_directory.clone(),
            db.unwrap_or_else(|_| {
                LibraryDb::open_in_memory().expect("in-memory library always opens")
            }),
            settings.read_playlists_in_folders,
        )
    }

    pub fn root(&self) -> Option<PathBuf> {
        lock(&self.root).clone()
    }

    pub fn read_playlists_in_folders(&self) -> bool {
        *lock(&self.read_playlists_in_folders)
    }

    pub fn set_read_playlists_in_folders(&self, enabled: bool) {
        *lock(&self.read_playlists_in_folders) = enabled;
    }

    pub fn set_root(&self, root: Option<PathBuf>) {
        *lock(&self.root) = root;
    }

    pub fn db(&self) -> std::sync::MutexGuard<'_, LibraryDb> {
        lock(&self.db)
    }

    /// Resolve a client-supplied path inside the library root.
    ///
    /// The path is canonicalised first, so `..` and symlinks cannot escape:
    /// anything outside the root is refused.
    fn resolve(&self, requested: Option<&str>) -> Result<PathBuf, String> {
        let root = self
            .root()
            .ok_or_else(|| "no music directory is configured".to_owned())?;
        let root = root
            .canonicalize()
            .map_err(|error| format!("reading the music directory: {error}"))?;
        let candidate = match requested.filter(|value| !value.trim().is_empty()) {
            Some(value) => PathBuf::from(value),
            None => root.clone(),
        };
        let candidate = candidate
            .canonicalize()
            .map_err(|error| format!("reading {}: {error}", candidate.display()))?;
        if candidate != root && !candidate.starts_with(&root) {
            return Err("that path is outside the music directory".to_owned());
        }
        Ok(candidate)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[derive(Debug, Deserialize)]
pub struct BrowseQuery {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    pub kind: Option<String>,
    pub path: String,
    pub entry: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MoreQuery {
    pub g: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct StarRequest {
    pub kind: String,
    pub path: String,
    #[serde(default)]
    pub entry: String,
    #[serde(default)]
    pub fragment: String,
    pub starred: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreatePlaylistRequest {
    pub name: String,
}

/// One entry as a client sends it. Optional fields default, so a minimal
/// `{"kind": "local", "path": "..."}` is valid.
#[derive(Debug, Deserialize)]
pub struct EntryRequest {
    pub kind: String,
    pub path: String,
    #[serde(default)]
    pub entry: String,
    #[serde(default)]
    pub fragment: String,
}

impl EntryRequest {
    fn into_stored(self) -> StoredEntry {
        StoredEntry {
            kind: self.kind,
            path: self.path,
            entry: self.entry,
            fragment: (!self.fragment.trim().is_empty()).then_some(self.fragment),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct AppendEntriesRequest {
    pub entries: Vec<EntryRequest>,
}

/// One entry in a `POST /api/metadata` batch. Unlike [`EntryRequest`] the
/// locator fields tolerate JSON `null`, because the web UI sends
/// `"fragment": null` for ordinary tracks.
#[derive(Debug, Deserialize)]
pub struct MetadataEntry {
    pub kind: String,
    pub path: String,
    #[serde(default)]
    pub entry: Option<String>,
    #[serde(default)]
    pub fragment: Option<String>,
}

/// `GET /api/metadata` query.
#[derive(Debug, Deserialize)]
pub struct MetadataQuery {
    pub kind: String,
    pub path: String,
    #[serde(default)]
    pub entry: Option<String>,
    #[serde(default)]
    pub fragment: Option<String>,
}

/// Largest accepted batch. Bigger requests are a client bug and would pin a
/// blocking thread for a long time.
const METADATA_BATCH_LIMIT: usize = 1000;

/// How many metadata rows to remember. A 300-row playlist refresh re-asks for
/// exactly those 300 keys, so a few thousand covers several playlists while
/// keeping the map trivially small (a row is a handful of short strings).
const METADATA_CACHE_CAPACITY: usize = 4096;

/// One probed row, shaped for the playlist columns the web UI renders. Every
/// field is present; unknown tags are `null`.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetadataRow {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub composer: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub track_number: Option<u32>,
    /// Seconds, as a float.
    pub duration: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub bits_per_sample: Option<u8>,
    pub codec: Option<String>,
    pub bitrate: Option<u32>,
}

impl MetadataRow {
    fn from_properties(properties: StreamProperties) -> Self {
        Self {
            title: properties.title,
            artist: properties.artist,
            album: properties.album,
            album_artist: properties.album_artist,
            composer: properties.composer,
            genre: properties.genre,
            year: properties.year,
            track_number: properties.track_number,
            duration: properties.duration.map(|duration| duration.as_secs_f64()),
            sample_rate: properties.sample_rate,
            channels: properties.channels,
            bits_per_sample: properties.bits_per_sample,
            codec: properties.codec,
            bitrate: properties.bitrate,
        }
    }
}

/// Bounded, in-memory cache of metadata lookups (negatives included).
///
/// Eviction is FIFO by first insertion: a refreshed key keeps its original
/// slot, so a working set that fits the cap is never evicted by a burst of
/// one-off lookups until the queue itself reaches them. The whole map is
/// tiny, so exact LRU bookkeeping would not earn its complexity.
#[derive(Default)]
pub struct MetadataCache {
    rows: HashMap<String, Option<MetadataRow>>,
    order: VecDeque<String>,
}

impl MetadataCache {
    fn get(&self, key: &str) -> Option<Option<MetadataRow>> {
        self.rows.get(key).cloned()
    }

    fn insert(&mut self, key: String, row: Option<MetadataRow>) {
        if !self.rows.contains_key(&key) {
            self.order.push_back(key.clone());
        }
        self.rows.insert(key, row);
        while self.rows.len() > METADATA_CACHE_CAPACITY {
            match self.order.pop_front() {
                Some(oldest) => {
                    self.rows.remove(&oldest);
                }
                None => break,
            }
        }
    }
}

/// Stable identity for a lookup: the same fields the library locator uses,
/// so the POST batch and the GET lookup share one cache entry.
fn metadata_key(kind: &str, path: &str, entry: &str, fragment: &str) -> String {
    format!("{kind}|{path}|{entry}|{}", fragment.trim())
}

/// Turn a client entry into a resolved, probed row.
///
/// `Err` means the request itself is malformed (unknown kind, missing path),
/// which `GET` reports as 400; `Ok(None)` means it could not be probed, which
/// `GET` reports as 404 and the batch reports as `null`.
fn probe_metadata(
    streams: &crate::service::StreamService,
    kind: &str,
    path: &str,
    entry: &str,
    fragment: &str,
) -> Result<Option<MetadataRow>, String> {
    let location = match kind {
        "local" => {
            if path.trim().is_empty() {
                return Err("a track path is required".to_owned());
            }
            PlaylistLocation::Local(PathBuf::from(path))
        }
        "archive" => {
            if path.trim().is_empty() || entry.trim().is_empty() {
                return Err("an archive path and member name are required".to_owned());
            }
            PlaylistLocation::Archive {
                archive_path: PathBuf::from(path),
                entry_name: entry.to_owned(),
            }
        }
        "remote" => {
            if path.trim().is_empty() {
                return Err("a remote URL is required".to_owned());
            }
            PlaylistLocation::Remote(path.to_owned())
        }
        other => return Err(format!("unknown track kind: {other}")),
    };
    let source = PlaylistEntry {
        location,
        fragment: (!fragment.trim().is_empty()).then(|| fragment.trim().to_owned()),
    };
    Ok(streams
        .probe_entry(source)
        .ok()
        .map(MetadataRow::from_properties))
}

/// Cache-first lookup. Failures are cached too, so a broken entry is not
/// re-probed on every refresh.
fn lookup_metadata(
    library: &Arc<Library>,
    streams: &crate::service::StreamService,
    request: &MetadataEntry,
) -> Option<MetadataRow> {
    let entry = request.entry.clone().unwrap_or_default();
    let fragment = request.fragment.clone().unwrap_or_default();
    let key = metadata_key(&request.kind, &request.path, &entry, &fragment);
    if let Some(cached) = lock(&library.metadata).get(&key) {
        return cached;
    }
    let row = probe_metadata(streams, &request.kind, &request.path, &entry, &fragment)
        .unwrap_or(None);
    lock(&library.metadata).insert(key, row.clone());
    row
}

/// `POST /api/metadata` — tags for a batch of entries, same order and length.
pub async fn metadata_batch(
    State(state): State<AppState>,
    axum::Json(entries): axum::Json<Vec<MetadataEntry>>,
) -> Response {
    if entries.len() > METADATA_BATCH_LIMIT {
        return bad_request(&format!(
            "a metadata batch is limited to {METADATA_BATCH_LIMIT} entries"
        ));
    }
    let library = state.library.clone();
    let streams = state.streams.clone();
    let result = tokio::task::spawn_blocking(move || {
        entries
            .iter()
            .map(|entry| lookup_metadata(&library, &streams, entry))
            .collect::<Vec<_>>()
    })
    .await;
    match result {
        Ok(rows) => axum::Json(rows).into_response(),
        Err(error) => bad_request(&format!("looking up metadata failed: {error}")),
    }
}

/// `GET /api/metadata` — one entry; 400 on a bad request, 404 when unknown.
pub async fn metadata_one(
    State(state): State<AppState>,
    Query(query): Query<MetadataQuery>,
) -> Response {
    let request = MetadataEntry {
        kind: query.kind,
        path: query.path,
        entry: query.entry,
        fragment: query.fragment,
    };
    let key = metadata_key(
        &request.kind,
        &request.path,
        request.entry.as_deref().unwrap_or_default(),
        request.fragment.as_deref().unwrap_or_default(),
    );
    let library = state.library.clone();
    let streams = state.streams.clone();
    let result = tokio::task::spawn_blocking(move || {
        if let Some(cached) = lock(&library.metadata).get(&key) {
            return Ok(cached);
        }
        let entry = request.entry.as_deref().unwrap_or_default();
        let fragment = request.fragment.as_deref().unwrap_or_default();
        let row = probe_metadata(&streams, &request.kind, &request.path, entry, fragment)?;
        lock(&library.metadata).insert(key, row.clone());
        Ok(row)
    })
    .await
    .unwrap_or_else(|error| Err(format!("looking up metadata failed: {error}")));
    match result {
        Ok(Some(row)) => axum::Json(row).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({ "ok": false, "error": "no metadata for that entry" })),
        )
            .into_response(),
        Err(error) => bad_request(&error),
    }
}

/// `GET /api/library` — one directory level.
pub async fn browse(State(state): State<AppState>, Query(query): Query<BrowseQuery>) -> Response {
    let library = state.library.clone();
    let requested = query.path.clone();
    let result = tokio::task::spawn_blocking(move || browse_blocking(&library, requested.as_deref()))
        .await
        .unwrap_or_else(|error| Err(format!("library browse failed: {error}")));
    match result {
        Ok(value) => axum::Json(value).into_response(),
        Err(error) => bad_request(&error),
    }
}

/// One file line in a `/api/library` response.
///
/// `kind`/`entry`/`fragment` address the track the same way `/api/stream` and
/// `/api/metadata` do; a plain audio file carries `kind: "local"`, an empty
/// entry and no fragment. Playlist files in a folder are expanded into their
/// entries, so they never appear as a bare `local` file that cannot stream.
#[derive(Clone, Debug, serde::Serialize)]
struct BrowseFile {
    name: String,
    path: String,
    relative: String,
    kind: String,
    entry: String,
    fragment: Option<String>,
}

fn browse_file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn browse_file_relative(root: Option<&std::path::Path>, path: &std::path::Path) -> String {
    root.and_then(|root| path.strip_prefix(root).ok())
        .map(|relative| relative.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// The line for a file that is offered as-is (not expanded).
fn local_browse_file(path: &std::path::Path, root: Option<&std::path::Path>) -> BrowseFile {
    BrowseFile {
        name: browse_file_name(path),
        path: path.to_string_lossy().into_owned(),
        relative: browse_file_relative(root, path),
        kind: "local".to_owned(),
        entry: String::new(),
        fragment: None,
    }
}

/// Map one playlist entry to a browse line, or `None` when it cannot be
/// resolved to something streamable. A plain local path is only accepted when
/// it exists: GME companion playlists write `file.gbs::GBS,0,title` lines that
/// are metadata for the emulator, not paths, and must not be offered.
fn playlist_entry_browse_file(
    entry: &PlaylistEntry,
    root: Option<&std::path::Path>,
) -> Option<BrowseFile> {
    match &entry.location {
        PlaylistLocation::Local(path) => {
            if !path.exists() {
                return None;
            }
            Some(BrowseFile {
                name: browse_file_name(path),
                path: path.to_string_lossy().into_owned(),
                relative: browse_file_relative(root, path),
                kind: "local".to_owned(),
                entry: String::new(),
                fragment: entry.fragment.clone(),
            })
        }
        PlaylistLocation::Archive {
            archive_path,
            entry_name,
        } => Some(BrowseFile {
            name: PathBuf::from(entry_name)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| entry_name.clone()),
            path: archive_path.to_string_lossy().into_owned(),
            relative: browse_file_relative(root, archive_path),
            kind: "archive".to_owned(),
            entry: entry_name.clone(),
            fragment: entry.fragment.clone(),
        }),
        PlaylistLocation::Remote(url) => Some(BrowseFile {
            name: url.rsplit('/').next().unwrap_or(url).to_owned(),
            path: url.clone(),
            relative: url.clone(),
            kind: "remote".to_owned(),
            entry: String::new(),
            fragment: entry.fragment.clone(),
        }),
    }
}

/// The same letterbox a bare [`PlaybackSource`] gets, used for the HLS
/// fallback where the playlist parser refuses a stream it cannot expand.
fn playback_source_browse_file(
    source: &PlaybackSource,
    root: Option<&std::path::Path>,
) -> BrowseFile {
    if let Some(origin) = &source.archive_origin {
        return BrowseFile {
            name: PathBuf::from(&origin.entry_name)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| origin.entry_name.clone()),
            path: origin.archive_path.to_string_lossy().into_owned(),
            relative: browse_file_relative(root, &origin.archive_path),
            kind: "archive".to_owned(),
            entry: origin.entry_name.clone(),
            fragment: source.subsong.map(|subsong| subsong.to_string()),
        };
    }
    if let Some(url) = &source.remote_url {
        return BrowseFile {
            name: url.rsplit('/').next().unwrap_or(url).to_owned(),
            path: url.clone(),
            relative: url.clone(),
            kind: "remote".to_owned(),
            entry: String::new(),
            fragment: source.subsong.map(|subsong| subsong.to_string()),
        };
    }
    BrowseFile {
        name: browse_file_name(&source.path),
        path: source.path.to_string_lossy().into_owned(),
        relative: browse_file_relative(root, &source.path),
        kind: "local".to_owned(),
        entry: String::new(),
        fragment: source.subsong.map(|subsong| subsong.to_string()),
    }
}

/// Expand a playlist file found during a folder browse into the entries the
/// desktop's folder scanner would add. Empty when nothing resolves (a GME
/// companion, or a malformed playlist), so no unplayable line is ever offered.
fn playlist_browse_files(
    playlist_path: &std::path::Path,
    decoders: &kog_audio::decoder::DecoderRegistry,
    root: Option<&std::path::Path>,
) -> Vec<BrowseFile> {
    let expanded: Vec<BrowseFile> = match Playlist::open(playlist_path) {
        Ok(playlist) => playlist
            .entries()
            .iter()
            .filter_map(|entry| playlist_entry_browse_file(entry, root))
            .collect(),
        // HLS playlists (and anything else the M3U/PLS parser rejects) are
        // still playable through FFmpeg: the registry hands back the playlist
        // file itself as the source, exactly as the desktop does.
        Err(_) => decoders
            .expand_detailed(playlist_path.to_path_buf())
            .map(|expansion| {
                expansion
                    .sources
                    .iter()
                    .map(|source| playback_source_browse_file(source, root))
                    .collect()
            })
            .unwrap_or_default(),
    };
    if !expanded.is_empty() {
        return expanded;
    }
    // GME's own `.m3u` syntax (`file.gbs::GBS,0,title`) is not a path, so the
    // M3U parser cannot resolve it. A one-line playlist in this shape is a
    // track reference into a sibling music-emulator container; expand it into
    // one row per entry, each carrying its subsong as the fragment.
    gme_playlist_browse_files(playlist_path, decoders, root)
}

/// One GME `.m3u` track line: `file.gbs::GBS,0,Title,length,...`. Only
/// accepted when `file` resolves to an existing music-emulator container, so a
/// non-GME line such as `song.wav::WAV,1,title` stays unresolvable and
/// contributes nothing rather than a track that would 400 on stream.
fn gme_track_browse_file(
    line: &str,
    playlist_dir: &std::path::Path,
    decoders: &kog_audio::decoder::DecoderRegistry,
    root: Option<&std::path::Path>,
) -> Option<BrowseFile> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let (file, fields) = line.split_once("::")?;
    let (_, fields) = fields.split_once(',')?; // the GME system name
    let (subsong, title) = fields.split_once(',')?;
    let subsong = subsong.trim();
    subsong.parse::<u32>().ok()?;
    let file = file.trim().replace('\\', "/");
    if file.is_empty() {
        return None;
    }
    let path = std::path::Path::new(&file);
    let target = if path.is_absolute() {
        path.to_path_buf()
    } else {
        playlist_dir.join(path)
    };
    if !target.is_file() {
        return None;
    }
    let source = PlaybackSource::from_path(target.clone());
    if decoders.selected_backend_id(&source) != Some("game-music-emu") {
        return None;
    }
    let title = gme_title_field(title);
    Some(BrowseFile {
        name: if title.is_empty() {
            browse_file_name(&target)
        } else {
            title
        },
        path: target.to_string_lossy().into_owned(),
        relative: browse_file_relative(root, &target),
        kind: "local".to_owned(),
        entry: String::new(),
        fragment: Some(subsong.to_owned()),
    })
}

/// The title field of a GME track line, honouring backslash escapes and
/// stopping at the comma that begins the length/time field, exactly as GME's
/// own parser does.
fn gme_title_field(field: &str) -> String {
    let mut title = String::new();
    let mut chars = field.chars();
    while let Some(character) = chars.next() {
        match character {
            '\\' => {
                if let Some(escaped) = chars.next() {
                    title.push(escaped);
                }
            }
            ',' => {
                let rest = chars.as_str().trim_start();
                if rest.starts_with(',')
                    || rest.starts_with('-')
                    || rest.chars().next().is_some_and(|c| c.is_ascii_digit())
                {
                    break;
                }
                title.push(',');
            }
            _ => title.push(character),
        }
    }
    title.trim().to_owned()
}

fn gme_playlist_browse_files(
    playlist_path: &std::path::Path,
    decoders: &kog_audio::decoder::DecoderRegistry,
    root: Option<&std::path::Path>,
) -> Vec<BrowseFile> {
    let Ok(bytes) = std::fs::read(playlist_path) else {
        return Vec::new();
    };
    let text = kog_core::text_encoding::decode(&bytes).replace('\r', "\n");
    let directory = playlist_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    text.lines()
        .filter_map(|line| gme_track_browse_file(line, directory, decoders, root))
        .collect()
}

/// An `.m3u` whose stem matches a sibling playable file is GME's companion
/// playlist: metadata for the emulator, never a track of its own.
fn is_gme_companion(
    path: &std::path::Path,
    directory: &[PathBuf],
    decoders: &kog_audio::decoder::DecoderRegistry,
) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !extension.eq_ignore_ascii_case("m3u") && !extension.eq_ignore_ascii_case("m3u8") {
        return false;
    }
    let Some(stem) = path.file_stem() else {
        return false;
    };
    directory.iter().any(|sibling| {
        sibling != path
            && sibling.file_stem() == Some(stem)
            && !Playlist::is_path(sibling)
            && !kog_audio::archive::is_path(sibling)
            && decoders.accepts_path(sibling)
    })
}

fn browse_blocking(library: &Arc<Library>, requested: Option<&str>) -> Result<serde_json::Value, String> {
    // An archive file, or a path inside one, browses the archive's members;
    // the real-filesystem checks below would reject both.
    if let Some(requested) = requested {
        let path = std::path::Path::new(requested);
        if !path.is_dir() {
            if path.is_file() && kog_audio::archive::is_path(path) {
                return archive_listing(path, "");
            }
            if let Some((archive, subpath)) = archive_ancestor(path) {
                return archive_listing(&archive, &subpath);
            }
        }
    }
    let directory = library.resolve(requested)?;
    let decoders = kog_audio::decoder::DecoderRegistry::new(
        kog_audio::settings::AppSettings::load().decoder_settings(),
    );
    let read_playlists = library.read_playlists_in_folders();
    let root = library.root();
    let root = root.as_deref();

    let mut directories = Vec::new();
    let mut candidates = Vec::new();
    let entries = std::fs::read_dir(&directory)
        .map_err(|error| format!("reading {}: {error}", directory.display()))?;
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if crate::media_filter::is_hidden(&path) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            directories.push(path);
        } else if file_type.is_file() && decoders.accepts_path(&path) {
            // An archive browses as a folder of its members.
            if kog_audio::archive::is_path(&path) {
                directories.push(path);
            } else {
                candidates.push(path);
            }
        }
    }
    directories.sort();
    candidates.sort();

    let mut files: Vec<BrowseFile> = Vec::new();
    for path in &candidates {
        if Playlist::is_path(path) {
            // The companion check applies even when playlist reading is off:
            // it is emulator metadata, never a track to offer.
            if is_gme_companion(path, &candidates, &decoders) {
                continue;
            }
            if read_playlists {
                files.extend(playlist_browse_files(path, &decoders, root));
                continue;
            }
        }
        files.push(local_browse_file(path, root));
    }
    // A playlist entry can name a file that is also sitting in the folder, so
    // drop repeats exactly as the desktop's scanner does. The locator fields
    // are the same identity the stream and metadata endpoints use.
    let mut seen = HashSet::new();
    files.retain(|file| {
        seen.insert(format!(
            "{}|{}|{}|{}",
            file.kind,
            file.path,
            file.entry,
            file.fragment.clone().unwrap_or_default()
        ))
    });
    // A subsong playlist points at a sibling container, so the bare container
    // repeats the same file as a whole song. Keep the specific track rows and
    // drop the bare one, the way a repeated playlist entry is dropped above.
    let subsong_targets: HashSet<(String, String, String)> = files
        .iter()
        .filter(|file| {
            file.fragment
                .as_deref()
                .is_some_and(|fragment| !fragment.is_empty())
        })
        .map(|file| (file.kind.clone(), file.path.clone(), file.entry.clone()))
        .collect();
    files.retain(|file| {
        let bare = file.fragment.as_deref().map_or(true, str::is_empty);
        !bare
            || !subsong_targets.contains(&(file.kind.clone(), file.path.clone(), file.entry.clone()))
    });
    files.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.entry.cmp(&right.entry))
            .then_with(|| left.fragment.cmp(&right.fragment))
    });

    let list = |paths: Vec<PathBuf>| -> Vec<serde_json::Value> {
        paths
            .into_iter()
            .map(|path| {
                serde_json::json!({
                    "name": path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
                    "path": path.to_string_lossy(),
                    "relative": root
                        .and_then(|root| path.strip_prefix(root).ok())
                        .map(|relative| relative.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                })
            })
            .collect()
    };
    Ok(serde_json::json!({
        "path": directory.to_string_lossy(),
        "parent": directory.parent().map(|parent| parent.to_string_lossy().into_owned()),
        "directories": list(directories),
        "files": files,
    }))
}

/// `GET /api/library/search` — filename search under the music directory.
///
/// The desktop's tree search streams matches while its walk proceeds; a plain
/// request/response cannot, and on a library with a million files a whole
/// walk takes tens of seconds — long enough to look hung. So the walk runs in
/// time-boxed slices: this handler starts a session and returns the first
/// batch, the client pulls further batches from [`search_more`], and every
/// new query replaces the session (there is exactly one, so a stale walk is
/// simply dropped, like the desktop's superseded scan).
pub async fn search(State(state): State<AppState>, Query(query): Query<SearchQuery>) -> Response {
    let needle = query.q.trim().to_lowercase();
    let tokens: Vec<String> = needle.split_whitespace().map(str::to_owned).collect();
    let generation = state.search.generation.fetch_add(1, Ordering::Relaxed) + 1;
    let limit = query.limit.unwrap_or(500).min(1_000);
    if tokens.is_empty() {
        return finish_search(state, generation, Vec::new(), true);
    }
    let library = state.library.clone();
    let result = tokio::task::spawn_blocking(move || {
        let root = library.resolve(None)?;
        let mut session = SearchSession {
            tokens,
            pending: vec![(root, None, false)],
            found: Vec::new(),
            matched_dirs: Vec::new(),
        };
        run_search_slice(&mut session, limit);
        Ok::<_, String>(session)
    })
    .await
    .unwrap_or_else(|error| Err(format!("library search failed: {error}")));
    match result {
        Ok(session) => finish_session(state, generation, session),
        Err(error) => bad_request(&error),
    }
}

/// The original bytes of one library entry, served as an attachment: the file
/// itself for `kind=local` paths, the extracted member for `kind=archive`
/// entries. Remote URLs are not downloadable.
pub async fn media_download(
    State(state): State<AppState>,
    Query(query): Query<DownloadQuery>,
) -> Response {
    let kind = query.kind.clone().unwrap_or_else(|| "local".to_owned());
    let library = state.library.clone();
    let cache_dir = kog_audio::archive::nested_cache_dir();
    let job = tokio::task::spawn_blocking(move || -> Result<(std::path::PathBuf, String), String> {
        match kind.as_str() {
            "archive" => {
                let entry = query.entry.clone().unwrap_or_default();
                if entry.trim().is_empty() {
                    return Err("an archive member name is required".to_owned());
                }
                let archive = library.resolve(Some(&query.path))?;
                let member = kog_audio::archive::materialize_archive_member(
                    &archive,
                    &entry,
                    &cache_dir,
                )?;
                let name = std::path::Path::new(&entry)
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| entry.clone());
                Ok((member, name))
            }
            "local" => {
                let file = library.resolve(Some(&query.path))?;
                if !file.is_file() {
                    return Err(format!("{} is not a file", file.display()));
                }
                let name = file
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "media".to_owned());
                Ok((file, name))
            }
            other => Err(format!("unknown track kind: {other}")),
        }
    })
    .await
    .unwrap_or_else(|error| Err(error.to_string()));

    match job {
        Ok((file, name)) => match tokio::fs::read(&file).await {
            Ok(bytes) => {
                let mut response = Response::new(axum::body::Body::from(bytes));
                *response.status_mut() = StatusCode::OK;
                let ascii: String = name
                    .chars()
                    .map(|c| {
                        if c.is_ascii_graphic() && c != '"' && c != '\\' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                let encoded: String = name
                    .as_bytes()
                    .iter()
                    .map(|byte| match byte {
                        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                            (*byte as char).to_string()
                        }
                        other => format!("%{other:02X}"),
                    })
                    .collect();
                response.headers_mut().insert(
                    axum::http::header::CONTENT_DISPOSITION,
                    axum::http::HeaderValue::from_str(&format!(
                        "attachment; filename=\"{ascii}\"; filename*=UTF-8''{encoded}"
                    ))
                    .expect("a content disposition is a header value"),
                );
                response.headers_mut().insert(
                    axum::http::header::CONTENT_TYPE,
                    axum::http::HeaderValue::from_static("application/octet-stream"),
                );
                response
            }
            Err(error) => bad_request(&format!("reading {}: {error}", file.display())),
        },
        Err(error) => bad_request(&error),
    }
}

/// `GET /api/library/search/more` — the next slice of the active walk. A
/// generation older than the current one means the query was superseded and
/// the session is gone: done, with nothing.
pub async fn search_more(State(state): State<AppState>, Query(query): Query<MoreQuery>) -> Response {
    let asked = query.g.unwrap_or_default();
    let generation = state.search.generation.load(Ordering::Relaxed);
    if asked != generation {
        return finish_search(state, generation, Vec::new(), true);
    }
    let Some(mut session) = state.search.session.lock().unwrap().take() else {
        return finish_search(state, generation, Vec::new(), true);
    };
    let result = tokio::task::spawn_blocking(move || {
        run_search_slice(&mut session, 500);
        session
    })
    .await
    .map_err(|error| {
        // The walk died; drop the session rather than wedging on it forever.
        format!("library search failed: {error}")
    });
    match result {
        Ok(session) => finish_session(state, generation, session),
        Err(_) => finish_search(state, generation, Vec::new(), true),
    }
}

/// Publish a slice's results: done walks release the session, unfinished ones
/// go back so the next `more` can resume.
fn finish_session(state: AppState, generation: u64, mut session: SearchSession) -> Response {
    let done = session.pending.is_empty();
    let found = std::mem::take(&mut session.found);
    if done {
        *state.search.session.lock().unwrap() = None;
    } else {
        *state.search.session.lock().unwrap() = Some(session);
    }
    finish_search(state, generation, found, done)
}

fn finish_search(
    _state: AppState,
    generation: u64,
    found: Vec<(std::path::PathBuf, bool)>,
    done: bool,
) -> Response {
    axum::Json(serde_json::json!({
        "results": found
            .into_iter()
            .map(|(path, is_dir)| {
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                serde_json::json!({
                    "name": name,
                    "path": path.to_string_lossy(),
                    "is_dir": is_dir,
                })
            })
            .collect::<Vec<_>>(),
        "done": done,
        "generation": generation,
    }))
    .into_response()
}

/// The longest existing ancestor of `path` that is an archive file, with the
/// remaining components as the member subpath inside it.
fn archive_ancestor(path: &std::path::Path) -> Option<(std::path::PathBuf, String)> {
    let mut cursor = path.to_path_buf();
    let mut tail: Vec<String> = Vec::new();
    loop {
        if cursor.is_file() && kog_audio::archive::is_path(&cursor) {
            tail.reverse();
            return Some((cursor, tail.join("/")));
        }
        let name = cursor.file_name()?.to_string_lossy().into_owned();
        tail.push(name);
        cursor = cursor.parent()?.to_path_buf();
    }
}

/// List an archive's members as a browse response: member paths become one
/// level of virtual directories plus the playable files at that level, and
/// every file keeps its archive locator so /api/stream can play it.
fn archive_listing(
    archive_path: &std::path::Path,
    subpath: &str,
) -> Result<serde_json::Value, String> {
    let members = kog_audio::archive::list_archive_names(archive_path)?;
    let prefix = if subpath.is_empty() {
        String::new()
    } else {
        format!("{}/", subpath.trim_matches('/'))
    };
    let archive = archive_path.to_string_lossy().into_owned();
    let mut directory_names: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut files: Vec<serde_json::Value> = Vec::new();
    for member in members {
        let Some(relative) = member.strip_prefix(&prefix) else {
            continue;
        };
        if relative.is_empty() {
            continue;
        }
        // Members below this level are virtual directories; browsing one
        // lists its own members.
        if let Some(split) = relative.find('/') {
            let folder = relative[..split].to_owned();
            if seen.insert(folder.clone()) {
                directory_names.push(folder);
            }
            continue;
        }
        if crate::media_filter::is_hidden(std::path::Path::new(&relative)) {
            continue;
        }
        files.push(serde_json::json!({
            "name": relative,
            "path": archive,
            "relative": member,
            "kind": "archive",
            "entry": member,
            "fragment": serde_json::Value::Null,
        }));
    }
    directory_names.sort();
    let directories: Vec<serde_json::Value> = directory_names
        .into_iter()
        .map(|name| {
            serde_json::json!({
                "name": name,
                "path": format!("{}/{}", archive_path.display(), name),
            })
        })
        .collect();
    Ok(serde_json::json!({
        "path": if prefix.is_empty() {
            archive.clone()
        } else {
            format!("{archive}/{}", prefix.trim_end_matches('/'))
        },
        "directories": directories,
        "files": files,
    }))
}

/// One active search: the words to match, the directories still to visit
/// (with the file to resume from when a slice stopped mid-directory, and
/// whether the walk already knows the directory sits under a folder whose
/// own name matched), the matches gathered so far, and the folders that
/// matched by name — everything inside those is exposed as a match.
struct SearchSession {
    tokens: Vec<String>,
    pending: Vec<(std::path::PathBuf, Option<String>, bool)>,
    found: Vec<(std::path::PathBuf, bool)>,
    matched_dirs: Vec<std::path::PathBuf>,
}

/// The server's single active search walk. The newest query owns it: starting
/// a search bumps the generation, which makes every earlier continuation ask
/// for a superseded generation and end quietly, exactly like the desktop's
/// superseded scan.
#[derive(Default)]
pub struct SearchState {
    generation: AtomicU64,
    session: Mutex<Option<SearchSession>>,
}

/// Walk for at most [`SEARCH_SLICE`] before yielding, collecting at most
/// `cap` new matches. A directory interrupted by the cap or the budget is
/// re-queued with the name to resume after; sorting entries by name makes
/// that resume point stable, so nothing is reported twice.
///
/// A folder whose own name matches every word exposes all of it: the folder
/// lands in the results and every file below it counts as a match no matter
/// what its own name is, so "audiobooks" reaches the chapters inside.
fn run_search_slice(session: &mut SearchSession, cap: usize) {
    const SLICE: std::time::Duration = std::time::Duration::from_millis(400);
    let started = std::time::Instant::now();
    let decoders = kog_audio::decoder::DecoderRegistry::new(
        kog_audio::settings::AppSettings::load().decoder_settings(),
    );
    let extensions = decoders.audio_extensions();
    let mut limit = cap;
    while let Some((directory, resume_after, inherited)) = session.pending.pop() {
        if started.elapsed() >= SLICE {
            session.pending.push((directory, resume_after, inherited));
            return;
        }
        let under_matched_folder = inherited
            || session
                .matched_dirs
                .iter()
                .any(|matched| directory.starts_with(matched));
        // Metadata ancestors are constant for the directory; per entry only
        // the name needs a look, which keeps a million-file walk out of
        // per-entry path allocation.
        let dir_is_metadata = kog_core::media_path::is_metadata(&directory);
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        let mut rows: Vec<(String, bool)> = entries
            .filter_map(Result::ok)
            .filter(|entry| {
                !dir_is_metadata && !crate::media_filter::is_hidden_name(&entry.file_name())
            })
            .filter_map(|entry| {
                let file_type = entry.file_type().ok()?;
                Some((
                    entry.file_name().to_string_lossy().into_owned(),
                    file_type.is_dir(),
                ))
            })
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        // A resumed directory replays from the top but skips everything
        // through the entry the previous slice had last seen.
        let mut skipping = resume_after.clone();
        // Subdirectories queue behind a reversed push so the LIFO walk climbs
        // them in ascending name order; they must all be flushed before this
        // directory can be left unfinished, or some would never be scanned.
        let mut queued_subdirs: Vec<(std::path::PathBuf, bool)> = Vec::new();
        let flush_subdirs =
            |session: &mut SearchSession, queued: &mut Vec<(std::path::PathBuf, bool)>| {
                for entry in queued.drain(..).rev() {
                    let (path, inherit) = entry;
                    session.pending.push((path, None, inherit));
                }
            };
        for (name, is_dir) in rows {
            if let Some(after) = &skipping {
                if *after != name {
                    continue;
                }
                skipping = None; // The next entry is an unseen one.
                continue;
            }
            let path = directory.join(&name);
            let folded = name.to_lowercase();
            let name_match = session
                .tokens
                .iter()
                .all(|token| folded.contains(token.as_str()));
            if is_dir {
                if name_match {
                    session.matched_dirs.push(path.clone());
                    session.found.push((path.clone(), true));
                    limit -= 1;
                    if limit == 0 {
                        flush_subdirs(session, &mut queued_subdirs);
                        session.pending.push((directory, Some(name), inherited));
                        return;
                    }
                }
                queued_subdirs.push((path, under_matched_folder || name_match));
                continue;
            }
            // Extension test, never accepts_path: the cue backend's accepts
            // opens media files to look for embedded cuesheets, which priced
            // a whole-library walk out of reach. This is the desktop
            // search's supportedFile check — name only.
            let accepted = path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extensions.contains(extension));
            if !accepted {
                continue;
            }
            // An archive is a container: when it matches, it surfaces as a
            // folder whose members play through the archive locator.
            let container = kog_audio::archive::is_path(&path);
            if under_matched_folder || name_match {
                session.found.push((path, container));
                limit -= 1;
                if limit == 0 {
                    flush_subdirs(session, &mut queued_subdirs);
                    session.pending.push((directory, Some(name), inherited));
                    return;
                }
            }
        }
        flush_subdirs(session, &mut queued_subdirs);
    }
}

/// `GET /api/playlists`
pub async fn list_playlists(State(state): State<AppState>) -> Response {
    let library = state.library.clone();
    let result = tokio::task::spawn_blocking(move || {
        let db = library.db();
        let playlists = db.list_playlists()?;
        Ok::<_, String>(serde_json::json!({
            "playlists": playlists
                .iter()
                .map(|playlist| serde_json::json!({
                    "id": playlist.id,
                    "name": playlist.name,
                    "entryCount": playlist.entry_count,
                }))
                .collect::<Vec<_>>(),
        }))
    })
    .await
    .unwrap_or_else(|error| Err(format!("listing playlists failed: {error}")));
    match result {
        Ok(value) => axum::Json(value).into_response(),
        Err(error) => bad_request(&error),
    }
}

/// `GET /api/playlists/{id}` — Favorites (0) included.
pub async fn playlist_entries(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    let library = state.library.clone();
    let result = tokio::task::spawn_blocking(move || {
        let db = library.db();
        let entries = if id == 0 {
            db.starred_entries()?
        } else {
            db.playlist_entries(id)?
        };
        Ok::<_, String>(serde_json::json!({
            "id": id,
            "entries": entries.into_iter().map(entry_json).collect::<Vec<_>>(),
        }))
    })
    .await
    .unwrap_or_else(|error| Err(format!("reading the playlist failed: {error}")));
    match result {
        Ok(value) => axum::Json(value).into_response(),
        Err(error) => bad_request(&error),
    }
}

/// `POST /api/playlists` — create an empty playlist.
pub async fn create_playlist(
    State(state): State<AppState>,
    axum::Json(request): axum::Json<CreatePlaylistRequest>,
) -> Response {
    let library = state.library.clone();
    let name = request.name.clone();
    let result = tokio::task::spawn_blocking(move || {
        let db = library.db();
        db.create_playlist(&name).map(|id| {
            serde_json::json!({ "id": id, "name": name.trim() })
        })
    })
    .await
    .unwrap_or_else(|error| Err(format!("creating the playlist failed: {error}")));
    match result {
        Ok(value) => (StatusCode::CREATED, axum::Json(value)).into_response(),
        Err(error) => bad_request(&error),
    }
}

/// `POST /api/playlists/{id}/entries` — append entries to a playlist.
///
/// Favorites (id 0) are managed through `/api/stars`, so appending to them is
/// refused rather than silently doing something else.
pub async fn append_playlist_entries(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    axum::Json(request): axum::Json<AppendEntriesRequest>,
) -> Response {
    if id == 0 {
        return bad_request("Favorites are managed through /api/stars");
    }
    if request.entries.is_empty() {
        return bad_request("no entries were supplied");
    }
    let library = state.library.clone();
    let result = tokio::task::spawn_blocking(move || {
        let entries: Vec<StoredEntry> = request
            .entries
            .into_iter()
            .map(EntryRequest::into_stored)
            .collect();
        let added = entries.len();
        library.db().append_entries(id, &entries)?;
        Ok::<_, String>(serde_json::json!({ "ok": true, "id": id, "added": added }))
    })
    .await
    .unwrap_or_else(|error| Err(format!("appending to the playlist failed: {error}")));
    match result {
        Ok(value) => axum::Json(value).into_response(),
        Err(error) => bad_request(&error),
    }
}

/// `DELETE /api/playlists/{id}`
pub async fn delete_playlist(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    if id == 0 {
        return bad_request("Favorites cannot be deleted");
    }
    let library = state.library.clone();
    let result = tokio::task::spawn_blocking(move || {
        library.db().delete_playlist(id).map(|()| serde_json::json!({ "ok": true, "id": id }))
    })
    .await
    .unwrap_or_else(|error| Err(format!("deleting the playlist failed: {error}")));
    match result {
        Ok(value) => axum::Json(value).into_response(),
        Err(error) => bad_request(&error),
    }
}

/// `POST /api/playlists/{id}/rename` — rename a playlist (Favorites exempt).
pub async fn rename_playlist(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    if id == 0 {
        return bad_request("Favorites cannot be renamed");
    }
    let Some(name) = body["name"]
        .as_str()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
    else {
        return bad_request("a playlist name is required");
    };
    let library = state.library.clone();
    let result = tokio::task::spawn_blocking(move || {
        library
            .db()
            .rename_playlist(id, &name)
            .map(|()| serde_json::json!({ "ok": true, "id": id, "name": name }))
    })
    .await
    .unwrap_or_else(|error| Err(format!("renaming the playlist failed: {error}")));
    match result {
        Ok(value) => axum::Json(value).into_response(),
        Err(error) => bad_request(&error),
    }
}

/// `GET /api/columns` — the shared playlist column layout, in the desktop's
/// `id,width,visible;...` format. The web player roots its column set here.
pub async fn get_columns() -> Response {
    let layout = kog_audio::settings::AppSettings::load()
        .playlist_column_layout
        .unwrap_or_default();
    axum::Json(serde_json::json!({ "layout": layout })).into_response()
}

/// `POST /api/columns` — persist the playlist column layout.
pub async fn set_columns(axum::Json(body): axum::Json<serde_json::Value>) -> Response {
    let Some(layout) = body["layout"].as_str() else {
        return bad_request("a layout string is required");
    };
    if let Err(error) = kog_audio::settings::AppSettings::save_playlist_column_layout(layout) {
        return bad_request(&error);
    }
    axum::Json(serde_json::json!({ "ok": true })).into_response()
}

/// `POST /api/playlists/{id}/move` — reorder a playlist.
pub async fn move_playlist(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    let Some(to) = body["to"].as_u64() else {
        return bad_request("a target position is required");
    };
    let Ok(to) = usize::try_from(to) else {
        return bad_request("the position is out of range");
    };
    let library = state.library.clone();
    let result = tokio::task::spawn_blocking(move || {
        library
            .db()
            .move_playlist(id, to)
            .map(|()| serde_json::json!({ "ok": true, "id": id, "to": to }))
    })
    .await
    .unwrap_or_else(|error| Err(format!("moving the playlist failed: {error}")));
    match result {
        Ok(value) => axum::Json(value).into_response(),
        Err(error) => bad_request(&error),
    }
}

/// `POST /api/playlists/{id}/duplicate` — copy a playlist and its entries
/// under a new name, the desktop's Duplicate menu action.
pub async fn duplicate_playlist(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    if id == 0 {
        return bad_request("Favorites cannot be duplicated");
    }
    let Some(name) = body["name"]
        .as_str()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
    else {
        return bad_request("a playlist name is required");
    };
    let library = state.library.clone();
    let result = tokio::task::spawn_blocking(move || {
        library
            .db()
            .duplicate_playlist(id, &name)
            .map(|new_id| serde_json::json!({ "id": new_id, "name": name }))
    })
    .await
    .unwrap_or_else(|error| Err(format!("duplicating the playlist failed: {error}")));
    match result {
        Ok(value) => (StatusCode::CREATED, axum::Json(value)).into_response(),
        Err(error) => bad_request(&error),
    }
}

/// `POST /api/playlists/{id}/prune-missing` — drop entries whose file has
/// vanished, the desktop's Remove Missing Files menu action.
pub async fn prune_missing_playlist_entries(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<i64>,
) -> Response {
    if id == 0 {
        return bad_request("Favorites are cleaned from their starred files instead");
    }
    let library = state.library.clone();
    let result = tokio::task::spawn_blocking(move || {
        let rows = library.db().playlist_entry_rows(id)?;
        let doomed: Vec<i64> = rows
            .iter()
            .filter(|(_, entry)| {
                matches!(entry.kind.as_str(), kog_core::db::KIND_LOCAL | kog_core::db::KIND_ARCHIVE)
                    && !std::path::Path::new(&entry.path).exists()
            })
            .map(|(row_id, _)| *row_id)
            .collect();
        let removed = library.db().delete_entry_rows(id, &doomed)?;
        Ok::<_, String>(serde_json::json!({
            "ok": true,
            "id": id,
            "removed": removed,
            "checked": rows.len(),
        }))
    })
    .await
    .unwrap_or_else(|error| Err(format!("pruning the playlist failed: {error}")));
    match result {
        Ok(value) => axum::Json(value).into_response(),
        Err(error) => bad_request(&error),
    }
}

/// `GET /api/stars`
pub async fn list_stars(State(state): State<AppState>) -> Response {
    let library = state.library.clone();
    let result = tokio::task::spawn_blocking(move || {
        let db = library.db();
        db.starred_entries().map(|entries| {
            serde_json::json!({
                "entries": entries.into_iter().map(entry_json).collect::<Vec<_>>(),
            })
        })
    })
    .await
    .unwrap_or_else(|error| Err(format!("listing stars failed: {error}")));
    match result {
        Ok(value) => axum::Json(value).into_response(),
        Err(error) => bad_request(&error),
    }
}

/// `POST /api/stars` — set or clear a star on one entry.
pub async fn set_star(
    State(state): State<AppState>,
    axum::Json(request): axum::Json<StarRequest>,
) -> Response {
    let library = state.library.clone();
    let result = tokio::task::spawn_blocking(move || {
        let stored = StoredEntry {
            kind: request.kind.clone(),
            path: request.path.clone(),
            entry: request.entry.clone(),
            fragment: (!request.fragment.trim().is_empty()).then(|| request.fragment.clone()),
        };
        let locator = crate::media_filter::locator_for(&stored)?;
        let db = library.db();
        db.set_star(
            &locator,
            &stored.kind,
            &stored.path,
            &stored.entry,
            stored.fragment.as_deref(),
            request.starred,
        )
        .map(|()| serde_json::json!({ "ok": true, "starred": request.starred, "locator": locator }))
    })
    .await
    .unwrap_or_else(|error| Err(format!("updating the star failed: {error}")));
    match result {
        Ok(value) => axum::Json(value).into_response(),
        Err(error) => bad_request(&error),
    }
}

fn entry_json(entry: StoredEntry) -> serde_json::Value {
    serde_json::json!({
        "kind": entry.kind,
        "path": entry.path,
        "entry": entry.entry,
        "fragment": entry.fragment,
    })
}

/// Router fragment for the library endpoints, so `routes` stays readable.
/// The synthesizer choices, labeled like the desktop's Preferences combo.
fn midi_options() -> Vec<serde_json::Value> {
    use kog_audio::settings::MidiEngine;
    [MidiEngine::RustySynth, MidiEngine::Opl3Windows, MidiEngine::Sc55, MidiEngine::Mt32]
        .into_iter()
        .map(|engine| {
            serde_json::json!({
                "value": engine.setting_value(),
                "label": match engine {
                    MidiEngine::RustySynth => "RustySynth (SF2)",
                    MidiEngine::Opl3Windows => "OPL3Windows (Nuked OPL3)",
                    MidiEngine::Sc55 => "Nuked SC-55",
                    MidiEngine::Mt32 => "Munt (MT-32 / CM-32L)",
                },
            })
        })
        .collect()
}

/// `GET /api/settings/midi` — the active MIDI synthesizer and the choices.
pub async fn midi_settings(State(state): State<AppState>) -> Response {
    let engine = state.streams.midi_engine();
    axum::Json(serde_json::json!({
        "engine": engine.setting_value(),
        "options": midi_options(),
    }))
    .into_response()
}

/// `POST /api/settings/midi` — switch the synthesizer used for MIDI streams.
/// Persists the choice so the desktop's next start uses it too; the running
/// service is retuned so the client's next stream hears it.
pub async fn set_midi_setting(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    let Some(value) = body["engine"].as_str() else {
        return bad_request("an engine value is required");
    };
    let Some(engine) = kog_audio::settings::MidiEngine::from_setting(value) else {
        return bad_request(&format!("unknown MIDI engine: {value}"));
    };
    if let Err(error) = kog_audio::settings::AppSettings::save_midi_engine(engine) {
        return bad_request(&error);
    }
    state.streams.set_midi_engine(engine);
    axum::Json(serde_json::json!({ "engine": engine.setting_value() })).into_response()
}

pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/api/library", get(browse))
        .route("/api/library/search", get(search))
        .route("/api/library/search/more", get(search_more))
        .route("/api/media/download", get(media_download))
        .route("/api/metadata", get(metadata_one).post(metadata_batch))
        .route("/api/playlists", get(list_playlists).post(create_playlist))
        .route(
            "/api/playlists/{id}",
            get(playlist_entries).delete(delete_playlist),
        )
        .route("/api/playlists/{id}/rename", post(rename_playlist))
        .route("/api/playlists/{id}/move", post(move_playlist))
        .route("/api/playlists/{id}/entries", post(append_playlist_entries))
        .route("/api/playlists/{id}/duplicate", post(duplicate_playlist))
        .route(
            "/api/playlists/{id}/prune-missing",
            post(prune_missing_playlist_entries),
        )
        .route("/api/stars", get(list_stars).post(set_star))
        .route("/api/settings/midi", get(midi_settings).post(set_midi_setting))
        .route("/api/columns", get(get_columns).post(set_columns))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(title: &str) -> MetadataRow {
        MetadataRow {
            title: Some(title.to_owned()),
            artist: None,
            album: None,
            album_artist: None,
            composer: None,
            genre: None,
            year: None,
            track_number: None,
            duration: None,
            sample_rate: None,
            channels: None,
            bits_per_sample: None,
            codec: None,
            bitrate: None,
        }
    }

    #[test]
    fn metadata_cache_remembers_positives_and_negatives() {
        let mut cache = MetadataCache::default();
        assert_eq!(cache.get("k"), None, "a miss is distinct from a cached null");
        cache.insert("k".to_owned(), Some(row("a")));
        assert_eq!(
            cache.get("k").flatten().unwrap().title.as_deref(),
            Some("a")
        );
        cache.insert("missing".to_owned(), None);
        assert_eq!(cache.get("missing"), Some(None), "negatives are cached");
    }

    #[test]
    fn metadata_cache_evicts_the_oldest_past_its_cap() {
        let mut cache = MetadataCache::default();
        for index in 0..=METADATA_CACHE_CAPACITY {
            cache.insert(format!("key-{index}"), None);
        }
        assert_eq!(cache.rows.len(), METADATA_CACHE_CAPACITY);
        assert_eq!(cache.get("key-0"), None, "oldest evicted");
        assert!(cache.get(&format!("key-{METADATA_CACHE_CAPACITY}")).is_some());
    }
}
