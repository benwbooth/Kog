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
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
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
}

#[derive(Debug, Deserialize)]
pub struct ArtQuery {
    pub kind: Option<String>,
    pub path: Option<String>,
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
    /// Where the client wants matches from in the shared buffer: the walk
    /// runs on its own thread, so a poll only reads what has accumulated.
    pub offset: Option<usize>,
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
    pub file_size_bytes: Option<u64>,
}

impl MetadataRow {
    fn from_properties(properties: StreamProperties, file_size_bytes: Option<u64>) -> Self {
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
            file_size_bytes,
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
    let source = PlaylistEntry::from_locator(
        kind,
        path,
        entry,
        (!fragment.trim().is_empty()).then(|| fragment.trim().to_owned()),
    )?;
    Ok(streams.probe_entry_with_size(source).ok().map(
        |(properties, file_size_bytes)| MetadataRow::from_properties(properties, file_size_bytes),
    ))
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

/// How many locators one `/api/expand` call expands. Expansion is structural
/// (header parses and archive listings, never probes), but each entry still
/// costs filesystem I/O, so large folder adds chunk client-side.
const EXPAND_BATCH_LIMIT: usize = 200;

/// One locator to expand, shaped like every other API entry. `name` is only
/// used when the locator already addresses a single track.
#[derive(Debug, Deserialize)]
struct ExpandEntry {
    kind: String,
    path: String,
    #[serde(default)]
    entry: Option<String>,
    #[serde(default)]
    fragment: Option<String>,
    #[serde(default)]
    name: Option<String>,
}

/// `POST /api/expand` — expand locators into their playable tracks, the way
/// the desktop's add path expands every added path. A multi-song file (NSF,
/// cue sheet) yields one track per song, each carrying its fragment; an
/// already-specific locator passes through as one track. Same order as the
/// request; one list per entry, empty when nothing resolves. Structural
/// expansion only — nothing is probed, so this cannot hang on a wedged
/// decoder the way proving can.
pub async fn expand_entries(
    State(state): State<AppState>,
    axum::Json(request): axum::Json<Vec<ExpandEntry>>,
) -> Response {
    if request.len() > EXPAND_BATCH_LIMIT {
        return bad_request(&format!(
            "an expand batch is limited to {EXPAND_BATCH_LIMIT} entries"
        ));
    }
    let root = state.library.root();
    let result = tokio::task::spawn_blocking(move || {
        let decoders = kog_audio::decoder::DecoderRegistry::new(
            kog_audio::settings::AppSettings::load().decoder_settings(),
        );
        request
            .iter()
            .map(|entry| expand_locator(&decoders, root.as_deref(), entry))
            .collect::<Vec<_>>()
    })
    .await;
    match result {
        Ok(tracks) => axum::Json(serde_json::json!({ "tracks": tracks })).into_response(),
        Err(error) => bad_request(&format!("expanding entries failed: {error}")),
    }
}

/// Expand one locator into playable browse rows. A locator that already
/// carries a fragment addresses a single track and passes through untouched.
fn expand_locator(
    decoders: &kog_audio::decoder::DecoderRegistry,
    root: Option<&std::path::Path>,
    request: &ExpandEntry,
) -> Vec<BrowseFile> {
    let entry = request.entry.clone().unwrap_or_default();
    let fragment = request.fragment.clone().unwrap_or_default();
    if !fragment.trim().is_empty() {
        let name = request
            .name
            .clone()
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| expand_single_name(&request.kind, &request.path, &entry));
        return vec![BrowseFile {
            name,
            path: request.path.clone(),
            relative: expand_single_name(&request.kind, &request.path, &entry),
            kind: request.kind.clone(),
            entry,
            fragment: Some(fragment.trim().to_owned()),
        }];
    }
    let expanded = match request.kind.as_str() {
        "local" => {
            if request.path.trim().is_empty() {
                return Vec::new();
            }
            let path = std::path::PathBuf::from(&request.path);
            if path.is_dir() {
                return Vec::new();
            }
            decoders.expand_detailed(path)
        }
        "archive" => {
            if request.path.trim().is_empty() || entry.trim().is_empty() {
                return Vec::new();
            }
            let url = kog_audio::archive::member_url(
                std::path::Path::new(&request.path),
                &entry,
                false,
            );
            decoders.expand_detailed(url)
        }
        "remote" => {
            if request.path.trim().is_empty() {
                return Vec::new();
            }
            decoders.expand_remote_url(&request.path)
        }
        _ => return Vec::new(),
    };
    match expanded {
        Ok(expansion) => expansion
            .sources
            .iter()
            .map(|source| expand_source_browse_file(decoders, source, root))
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Reuse the HTTP expansion rules in native frontends. A multi-song locator
/// produces one stored entry per subsong while an explicit fragment stays
/// intact. Callers decide how to handle an empty result.
pub fn expand_stored_entry(
    decoders: &kog_audio::decoder::DecoderRegistry,
    root: Option<&std::path::Path>,
    stored: &StoredEntry,
    name: &str,
) -> Vec<(String, StoredEntry)> {
    let request = ExpandEntry {
        kind: stored.kind.clone(),
        path: stored.path.clone(),
        entry: Some(stored.entry.clone()),
        fragment: stored.fragment.clone(),
        name: Some(name.to_owned()),
    };
    expand_locator(decoders, root, &request)
        .into_iter()
        .map(|file| {
            (
                file.name,
                StoredEntry {
                    kind: file.kind,
                    path: file.path,
                    entry: file.entry,
                    fragment: file.fragment,
                },
            )
        })
        .collect()
}

/// Display name for a locator that already addresses one track: the file (or
/// member) name the pane would show.
fn expand_single_name(kind: &str, path: &str, entry: &str) -> String {
    if kind == "archive" && !entry.is_empty() {
        return std::path::Path::new(entry)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| entry.to_owned());
    }
    if kind == "remote" {
        return path.rsplit('/').next().unwrap_or(path).to_owned();
    }
    browse_file_name(std::path::Path::new(path))
}

/// One expanded source as a browse row: the shared locator mapping, plus the
/// desktop's ` [N]` disambiguation for subsong tracks. Cue sheets address by
/// declared track number (not position), mapped through the sheet itself.
fn expand_source_browse_file(
    decoders: &kog_audio::decoder::DecoderRegistry,
    source: &PlaybackSource,
    root: Option<&std::path::Path>,
) -> BrowseFile {
    let mut file = playback_source_browse_file(source, root);
    let Some(subsong) = source.subsong else {
        return file;
    };
    if decoders.selected_backend_id(source) == Some("cuesheet") {
        if let Some(number) = kog_audio::cuesheet_decoder::cue_track_number(&source.path, subsong)
        {
            file.fragment = Some(number.to_string());
        }
    }
    file.name.push_str(&format!(" [{}]", subsong + 1));
    file
}

/// `GET /api/library` — one directory level.
pub async fn browse(State(state): State<AppState>, Query(query): Query<BrowseQuery>) -> Response {
    let library = state.library.clone();
    let requested = query.path.clone();
    let result = tokio::task::spawn_blocking(move || browse_blocking(&library, requested.as_deref(), false))
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

/// Browse synchronously for native frontends. This is the same listing used
/// by the HTTP endpoint, including archives, cue sheets and playlist files.
pub fn browse_local(
    library: &Arc<Library>,
    requested: Option<&str>,
) -> Result<serde_json::Value, String> {
    browse_blocking(library, requested, false)
}

/// Local terminal browsing may leave the configured music root. HTTP callers
/// continue to use the restricted path above.
pub fn browse_local_unrestricted(
    library: &Arc<Library>,
    requested: Option<&str>,
) -> Result<serde_json::Value, String> {
    browse_blocking(library, requested, true)
}

/// Collect a folder through the same browse and expansion rules used by the
/// web API. Native callers may opt out of the configured-root restriction;
/// HTTP callers must leave it enabled.
pub fn collect_local_folder(
    library: &Arc<Library>,
    decoders: &kog_audio::decoder::DecoderRegistry,
    path: &std::path::Path,
    unrestricted: bool,
    read_cue_sheets: bool,
    read_playlists: bool,
) -> Result<Vec<(String, StoredEntry)>, String> {
    let mut pending = vec![path.to_path_buf()];
    let mut tracks = Vec::new();
    let root = library.root();
    while let Some(directory) = pending.pop() {
        let listing = browse_blocking(library, directory.to_str(), unrestricted)?;
        if let Some(dirs) = listing["directories"].as_array() {
            for dir in dirs.iter().rev() {
                if let Some(path) = dir["path"].as_str() {
                    pending.push(PathBuf::from(path));
                }
            }
        }
        if let Some(files) = listing["files"].as_array() {
            for file in files {
                let (Some(kind), Some(path)) = (file["kind"].as_str(), file["path"].as_str()) else {
                    continue;
                };
                let entry = StoredEntry {
                    kind: kind.to_owned(),
                    path: path.to_owned(),
                    entry: file["entry"].as_str().unwrap_or_default().to_owned(),
                    fragment: file["fragment"].as_str().map(str::to_owned),
                };
                let extension_path = if entry.kind == "archive" && !entry.entry.is_empty() {
                    &entry.entry
                } else {
                    &entry.path
                };
                if !kog_audio::library_policy::include_discovered_file(
                    std::path::Path::new(extension_path),
                    read_cue_sheets,
                    read_playlists,
                    false,
                ) {
                    continue;
                }
                let name = file["name"].as_str().unwrap_or_default();
                let expanded = expand_stored_entry(decoders, root.as_deref(), &entry, name);
                if expanded.is_empty() {
                    tracks.push((name.to_owned(), entry));
                } else {
                    tracks.extend(expanded);
                }
            }
        }
    }
    let specific: HashSet<_> = tracks
        .iter()
        .filter(|(_, entry)| entry.fragment.is_some())
        .map(|(_, entry)| (entry.kind.clone(), entry.path.clone(), entry.entry.clone()))
        .collect();
    tracks.retain(|(_, entry)| {
        entry.fragment.is_some()
            || !specific.contains(&(entry.kind.clone(), entry.path.clone(), entry.entry.clone()))
    });
    Ok(tracks)
}

/// HTTP and native frontends use the same recursive folder collector. The
/// HTTP route keeps the configured music-root boundary enforced by browse.
pub async fn collect_folder_http(
    State(state): State<AppState>,
    Query(query): Query<BrowseQuery>,
) -> Response {
    let library = Arc::clone(&state.library);
    let result = tokio::task::spawn_blocking(move || {
        let settings = kog_audio::settings::AppSettings::load();
        let decoders = kog_audio::decoder::DecoderRegistry::new(settings.decoder_settings());
        let path = query.path.or_else(|| library.root().map(|path| path.to_string_lossy().into_owned()))
            .ok_or_else(|| "no music directory is configured".to_owned())?;
        let root = library.root();
        let entries = collect_local_folder(
            &library,
            &decoders,
            std::path::Path::new(&path),
            false,
            settings.read_cue_sheets_in_folders,
            library.read_playlists_in_folders(),
        )?;
        let files: Vec<_> = entries.into_iter().map(|(name, entry)| {
            let relative = root
                .as_deref()
                .and_then(|root| std::path::Path::new(&entry.path).strip_prefix(root).ok())
                .unwrap_or_else(|| std::path::Path::new(&entry.path))
                .to_string_lossy()
                .into_owned();
            let relative = if entry.entry.is_empty() { relative } else { format!("{relative}/{}", entry.entry) };
            serde_json::json!({
                "name": name,
                "relative": relative,
                "kind": entry.kind,
                "path": entry.path,
                "entry": entry.entry,
                "fragment": entry.fragment,
            })
        }).collect();
        Ok::<_, String>(serde_json::json!({ "tracks": files }))
    }).await;
    match result {
        Ok(Ok(value)) => axum::Json(value).into_response(),
        Ok(Err(error)) => bad_request(&error),
        Err(error) => bad_request(&format!("collecting folder failed: {error}")),
    }
}

fn browse_blocking(
    library: &Arc<Library>,
    requested: Option<&str>,
    unrestricted: bool,
) -> Result<serde_json::Value, String> {
    let resolve = |path: Option<&str>| -> Result<PathBuf, String> {
        if !unrestricted {
            return library.resolve(path);
        }
        let candidate = path
            .map(PathBuf::from)
            .or_else(|| library.root())
            .ok_or_else(|| "no music directory is configured".to_owned())?;
        candidate
            .canonicalize()
            .map_err(|error| format!("reading {}: {error}", candidate.display()))
    };
    // An archive file, or a path inside one, browses the archive's members;
    // the real-filesystem checks below would reject both.
    if let Some(requested) = requested {
        let path = std::path::Path::new(requested);
        if !path.is_dir() {
            if path.is_file() && kog_audio::archive::is_path(path) {
                return archive_listing(&resolve(Some(requested))?, "");
            }
            if let Some((archive, subpath)) = archive_ancestor(path) {
                return archive_listing(&resolve(archive.to_str())?, &subpath);
            }
        }
    }
    let directory = resolve(requested)?;
    let decoders = kog_audio::decoder::DecoderRegistry::new(
        kog_audio::settings::AppSettings::load().decoder_settings(),
    );
    let read_playlists = library.read_playlists_in_folders();
    let root = library.root();
    let root = root.as_deref().filter(|root| directory.starts_with(root));

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
/// The desktop's tree search runs its scanner on a background thread at full
/// native speed, streaming matches as they are found; this endpoint does the
/// same on the server. Starting a query spawns a worker that walks the whole
/// music folder — files and folders first, then each archive's member list,
/// so ordinary files never wait behind an archive — appending matches to a
/// shared buffer. The client polls: the first response arrives after a short
/// beat, further matches come from [`search_more`], and a newer query
/// cancels the older walk, exactly like the desktop's superseded scan.
pub async fn search(State(state): State<AppState>, Query(query): Query<SearchQuery>) -> Response {
    let needle = query.q.trim().to_lowercase();
    let tokens: Vec<String> = needle.split_whitespace().map(str::to_owned).collect();
    let generation = state.search.generation.fetch_add(1, Ordering::Relaxed) + 1;
    // Any previous walk is abandoned, empty query or not.
    if let Some(old) = state.search.job.lock().unwrap().as_ref() {
        old.shared.cancel.store(true, Ordering::Relaxed);
    }
    if tokens.is_empty() {
        // An empty query clears the results, like the desktop's reset.
        *state.search.job.lock().unwrap() = None;
        return search_snapshot(&state, generation, 0);
    }
    let shared = Arc::new(SearchShared::default());
    let library = state.library.clone();
    let worker_shared = shared.clone();
    let worker = std::thread::Builder::new()
        .name(format!("library-search-{generation}"))
        .spawn(move || walk_library_for_search(library, tokens, worker_shared));
    if let Err(error) = worker {
        return bad_request(&format!("starting the search failed: {error}"));
    }
    *state.search.job.lock().unwrap() = Some(SearchJob { shared });
    // Give the walk a beat so the first response already carries matches.
    std::thread::sleep(std::time::Duration::from_millis(120));
    search_snapshot(&state, generation, 0)
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

/// `POST /api/library/search/pause` — pause or resume the active walk.
/// Pausing idles the worker where it is; results already gathered stay
/// readable, and a new query always starts unpaused.
pub async fn pause_search(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> Response {
    let paused = body["paused"].as_bool().unwrap_or(false);
    if let Some(job) = state.search.job.lock().unwrap().as_ref() {
        job.shared.paused.store(paused, Ordering::Relaxed);
    }
    axum::Json(serde_json::json!({ "ok": true, "paused": paused })).into_response()
}

fn cached_or_downloaded_cover(
    file: &std::path::Path,
    artist: &str,
    tagged_album: &str,
    cache_dir: &std::path::Path,
    allow_download: bool,
    fetch: impl FnMut(&str, u32) -> Option<Vec<u8>>,
) -> Option<Vec<u8>> {
    use kog_audio::cover_art;

    let album = cover_art::fallback_album(file, tagged_album);
    if album.is_empty() {
        return None;
    }
    let (key, may_download) = cover_art::track_cache_key(artist, tagged_album, &album, file);
    if let Some(bytes) = cover_art::cache_lookup(cache_dir, &key)
        .and_then(|path| std::fs::read(path).ok())
        .filter(|bytes| cover_art::sniff_image_kind(bytes).is_some())
    {
        return Some(bytes);
    }
    if !may_download || !allow_download {
        return None;
    }
    cover_art::download_cover(artist, &album, fetch, || false, |bytes| {
        let _ = cover_art::store_cache(cache_dir, &key, &bytes);
        Some(bytes)
    })
}

/// `GET /api/art` — local, cached, or downloaded cover art for one library
/// file, addressed like streaming (`kind`, `path`). A track without artwork
/// answers 404 and the player falls back to its logo.
pub async fn art(State(state): State<AppState>, Query(query): Query<ArtQuery>) -> Response {
    let kind = query.kind.unwrap_or_else(|| "local".to_owned());
    let library = state.library.clone();
    let streams = state.streams.clone();
    let result = tokio::task::spawn_blocking(move || {
        // Archives are not materialized for art lookups: their files only
        // exist once streamed, and chiptune containers carry no artwork.
        if kind != "local" {
            return None;
        }
        let Ok(file) = library.resolve(query.path.as_deref()) else {
            return None;
        };
        if !file.is_file() {
            return None;
        }
        use kog_audio::cover_art;
        if let Some(bytes) = cover_art::embedded_cover_bytes(&file)
            .or_else(|| cover_art::sibling_cover_bytes(&file))
        {
            return Some(bytes);
        }
        let source = PlaylistEntry::from_locator("local", &file.to_string_lossy(), "", None).ok()?;
        let properties = streams.probe_entry(source).ok()?;
        let artist = properties.artist.unwrap_or_default();
        let tagged_album = properties.album.unwrap_or_default();
        let cache_dir = directories::ProjectDirs::from("org", "Kog", "Kog")
            .map(|directories| cover_art::cache_directory(directories.cache_dir()))
            .unwrap_or_else(|| std::env::temp_dir().join("kog-covers"));
        cached_or_downloaded_cover(
            &file,
            &artist,
            &tagged_album,
            &cache_dir,
            kog_audio::settings::AppSettings::load().download_cover_art,
            crate::cover_network::fetch,
        )
    })
    .await
    .unwrap_or_else(|error| {
        let _ = error;
        None
    });
    let Some(bytes) = result else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = match kog_audio::cover_art::sniff_image_kind(&bytes) {
        Some(kog_audio::cover_art::ImageKind::Jpeg) => "image/jpeg",
        Some(kog_audio::cover_art::ImageKind::Png) => "image/png",
        _ => "application/octet-stream",
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, content_type)
        .header(
            axum::http::header::CACHE_CONTROL,
            "private, max-age=86400",
        )
        .body(axum::body::Body::from(bytes))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

/// `GET /api/library/search/more` — the matches gathered since the client's
/// last poll. The walk advances on its own thread; a poll only reads. A
/// generation older than the current one means the query was superseded and
/// the walk is gone: done, with nothing.
pub async fn search_more(
    State(state): State<AppState>,
    Query(query): Query<MoreQuery>,
) -> Response {
    let generation = query.g.unwrap_or_default();
    let offset = query.offset.unwrap_or(0);
    search_snapshot(&state, generation, offset)
}

/// One match as the walk found it. Archive members keep their container's
/// path plus the member name, the same locator streaming and downloads use.
#[derive(Clone)]
pub(crate) struct SearchMatch {
    name: String,
    path: String,
    entry: String,
    /// `dir` for folders and archive containers, `local` for files,
    /// `archive` for members inside an archive.
    kind: &'static str,
    is_dir: bool,
}

/// The shared state of the one active search walk, plus the progress
/// counters the desktop status line reports.
#[derive(Default)]
pub struct SearchShared {
    matches: Mutex<Vec<SearchMatch>>,
    done: AtomicBool,
    /// The walk stopped at the match limit: tell the visitor to narrow it.
    limited: AtomicBool,
    cancel: AtomicBool,
    /// Items visited by the filesystem pass.
    scanned: AtomicU64,
    /// Archives discovered while walking, and how many were listed so far.
    archive_count: AtomicU64,
    archives_scanned: AtomicU64,
    /// Listings that failed (corrupt archive, permissions, ...).
    unreadable_archives: AtomicU64,
    /// True once the walk leaves the filesystem pass and starts listing.
    scanning_archives: AtomicBool,
    /// Set when the visitor pauses the search from the spinner.
    paused: AtomicBool,
}

struct SearchJob {
    shared: Arc<SearchShared>,
}

/// Native frontend handle for the same background search used by the web UI.
/// Dropping the handle cancels the walk before it opens another directory.
pub struct LocalSearch {
    shared: Arc<SearchShared>,
}

pub struct LocalSearchHit {
    pub name: String,
    pub path: String,
    pub entry: String,
    pub kind: &'static str,
    pub is_dir: bool,
}

/// A cheap snapshot of the same counters sent to Web search clients.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LocalSearchProgress {
    pub scanned: u64,
    pub archive_count: u64,
    pub archives_scanned: u64,
    pub unreadable_archives: u64,
    pub scanning_archives: bool,
    pub limited: bool,
    pub done: bool,
}

impl LocalSearch {
    pub fn start(library: Arc<Library>, query: &str) -> Result<Self, String> {
        let tokens: Vec<String> = query
            .trim()
            .to_lowercase()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        if tokens.is_empty() {
            return Err("a search term is required".to_owned());
        }
        let shared = Arc::new(SearchShared::default());
        let worker_shared = shared.clone();
        std::thread::Builder::new()
            .name("kog-terminal-search".to_owned())
            .spawn(move || walk_library_for_search(library, tokens, worker_shared))
            .map_err(|error| format!("starting search: {error}"))?;
        Ok(Self { shared })
    }

    /// Results since `offset`, plus whether the scan is complete.
    pub fn results_since(&self, offset: usize) -> (Vec<LocalSearchHit>, bool) {
        let matches = self.shared.matches.lock().unwrap();
        let results = matches
            .iter()
            .skip(offset)
            .map(|hit| LocalSearchHit {
                name: hit.name.clone(),
                path: hit.path.clone(),
                entry: hit.entry.clone(),
                kind: hit.kind,
                is_dir: hit.is_dir,
            })
            .collect();
        let done = self.shared.done.load(Ordering::Relaxed);
        (results, done)
    }

    pub fn progress(&self) -> LocalSearchProgress {
        LocalSearchProgress {
            scanned: self.shared.scanned.load(Ordering::Relaxed),
            archive_count: self.shared.archive_count.load(Ordering::Relaxed),
            archives_scanned: self.shared.archives_scanned.load(Ordering::Relaxed),
            unreadable_archives: self.shared.unreadable_archives.load(Ordering::Relaxed),
            scanning_archives: self.shared.scanning_archives.load(Ordering::Relaxed),
            limited: self.shared.limited.load(Ordering::Relaxed),
            done: self.shared.done.load(Ordering::Relaxed),
        }
    }
}

impl Drop for LocalSearch {
    fn drop(&mut self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
    }
}

/// The desktop's match limit: a walk that reaches it stops and says so.
const SEARCH_MATCH_LIMIT: usize = 2000;

/// How many matches one poll carries.
const SEARCH_BATCH: usize = 500;

fn search_snapshot(state: &AppState, generation: u64, offset: usize) -> Response {
    let current = state.search.generation.load(Ordering::Relaxed);
    let (slice, total, done, limited) = if generation == current {
        match state.search.job.lock().unwrap().as_ref() {
            Some(job) => {
                let all = job.shared.matches.lock().unwrap();
                let total = all.len();
                let start = offset.min(total);
                let end = (start + SEARCH_BATCH).min(total);
                let slice: Vec<SearchMatch> = all[start..end].to_vec();
                let done = job.shared.done.load(Ordering::Relaxed) && end >= total;
                let limited = job.shared.limited.load(Ordering::Relaxed);
                (slice, total, done, limited)
            }
            None => (Vec::new(), 0, true, false),
        }
    } else {
        (Vec::new(), 0, true, false)
    };
    let progress = state.search.job.lock().unwrap().as_ref().map_or(
        (0, 0, 0, 0, false),
        |job| {
            (
                job.shared.scanned.load(Ordering::Relaxed),
                job.shared.archive_count.load(Ordering::Relaxed),
                job.shared.archives_scanned.load(Ordering::Relaxed),
                job.shared.unreadable_archives.load(Ordering::Relaxed),
                job.shared.scanning_archives.load(Ordering::Relaxed),
            )
        },
    );
    axum::Json(serde_json::json!({
        "results": slice
            .into_iter()
            .map(|m| {
                serde_json::json!({
                    "name": m.name,
                    "path": m.path,
                    "entry": m.entry,
                    "kind": m.kind,
                    "is_dir": m.is_dir,
                })
            })
            .collect::<Vec<_>>(),
        "total": total,
        "done": done,
        "limited": limited,
        "generation": generation,
        "scanned": progress.0,
        "archive_count": progress.1,
        "archives_scanned": progress.2,
        "unreadable_archives": progress.3,
        "scanning_archives": progress.4,
    }))
    .into_response()
}

/// The background scanner, the web twin of the desktop's tree search: the
/// whole music folder at native speed (no slicing — polls only read), then
/// each archive's member list, so ordinary files never wait behind an
/// archive. Matches stream into `shared`; a newer query sets `cancel` and
/// this thread quits at the next directory.
fn walk_library_for_search(
    library: Arc<Library>,
    tokens: Vec<String>,
    shared: Arc<SearchShared>,
) {
    let finish = |matches: &mut Vec<SearchMatch>, limited: bool, shared: &SearchShared| {
        if !matches.is_empty() {
            shared.matches.lock().unwrap().extend(matches.drain(..));
        }
        shared.limited.store(limited, Ordering::Relaxed);
        shared.done.store(true, Ordering::Relaxed);
    };
    let Ok(root) = library.resolve(None) else {
        shared.done.store(true, Ordering::Relaxed);
        return;
    };
    let decoders = kog_audio::decoder::DecoderRegistry::new(
        kog_audio::settings::AppSettings::load().decoder_settings(),
    );
    let extensions = decoders.audio_extensions();
    let ascii_query = tokens.iter().all(|token| token.is_ascii());
    let matched = |name: &str| {
        if ascii_query && name.is_ascii() {
            tokens.iter().all(|token| {
                name.as_bytes()
                    .windows(token.len())
                    .any(|window| window.eq_ignore_ascii_case(token.as_bytes()))
            })
        } else {
            let folded = name.to_lowercase();
            tokens.iter().all(|token| folded.contains(token))
        }
    };
    let mut matches: Vec<SearchMatch> = Vec::new();
    let mut archives: Vec<std::path::PathBuf> = Vec::new();
    let mut limited = false;
    let mut published = 0_usize;
    let cancel = || shared.cancel.load(Ordering::Relaxed);
    let publish =
        |shared: &SearchShared, matches: &mut Vec<SearchMatch>, limited: &mut bool, published: &mut usize| {
            *published += matches.len();
            if *published >= SEARCH_MATCH_LIMIT {
                *limited = true;
            }
            if !matches.is_empty() {
                shared.matches.lock().unwrap().extend(matches.drain(..));
            }
        };

    // Filesystem pass: every folder and supported file, a match when its own
    // name carries every word, or when it sits under a folder that matched
    // (the folder's whole contents are exposed, matching the desktop).
    let mut pending = vec![(root, false)];
    while let Some((directory, inherited)) = pending.pop() {
        if cancel() {
            return;
        }
        while shared.paused.load(Ordering::Relaxed) && !cancel() {
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        // The flag is inherited by every child when a directory matches.
        // Rechecking all previously matched directories here made a broad
        // search increasingly expensive as the walk progressed.
        let under_matched = inherited;
        let dir_is_metadata = kog_core::media_path::is_metadata(&directory);
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            if cancel() {
                return;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if dir_is_metadata || crate::media_filter::is_hidden_name(&entry.file_name()) {
                continue;
            }
            shared.scanned.fetch_add(1, Ordering::Relaxed);
            let path = entry.path();
            if file_type.is_dir() {
                let name_match = matched(&name);
                if name_match {
                    matches.push(SearchMatch {
                        name: name.clone(),
                        path: path.to_string_lossy().into_owned(),
                        entry: String::new(),
                        kind: "dir",
                        is_dir: true,
                    });
                    publish(&shared, &mut matches, &mut limited, &mut published);
                    if limited {
                        break;
                    }
                }
                pending.push((path, under_matched || name_match));
                continue;
            }
            let Some(extension) = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.to_ascii_lowercase())
            else {
                continue;
            };
            if !extensions.contains(&extension) {
                continue;
            }
            // An archive is a container: it browses like a folder, and its
            // members are searched in the pass below.
            let container = kog_audio::archive::is_path(&path);
            if container {
                archives.push(path.clone());
            }
            if under_matched || matched(&name) {
                // A matching archive surfaces as a folder of its members.
                matches.push(SearchMatch {
                    name,
                    path: path.to_string_lossy().into_owned(),
                    entry: String::new(),
                    kind: if container { "dir" } else { "local" },
                    is_dir: container,
                });
                publish(&shared, &mut matches, &mut limited, &mut published);
                if limited {
                    break;
                }
            }
        }
        if limited {
            break;
        }
        if !matches.is_empty() {
            shared.matches.lock().unwrap().extend(matches.drain(..));
        }
    }

    // Archive pass: list each archive's members and match their names, the
    // desktop's "Archives %2 of %3" stage.
    shared
        .archive_count
        .store(archives.len() as u64, Ordering::Relaxed);
    shared.scanning_archives.store(true, Ordering::Relaxed);
    if !limited && !archives.is_empty() {
        // libarchive can spend noticeable time opening an individual solid
        // archive. A small fixed number of workers keeps the search moving
        // without opening thousands of files or saturating the disk at once.
        let next_archive = AtomicUsize::new(0);
        let workers = std::thread::available_parallelism()
            .map_or(2, |count| count.get())
            .min(4)
            .min(archives.len());
        std::thread::scope(|scope| {
            for _ in 0..workers {
                let shared = &shared;
                let archives = &archives;
                let extensions = &extensions;
                let matched = &matched;
                let next_archive = &next_archive;
                scope.spawn(move || {
                    loop {
                        if shared.cancel.load(Ordering::Relaxed)
                            || shared.limited.load(Ordering::Relaxed)
                        {
                            break;
                        }
                        while shared.paused.load(Ordering::Relaxed)
                            && !shared.cancel.load(Ordering::Relaxed)
                        {
                            std::thread::sleep(std::time::Duration::from_millis(40));
                        }
                        if shared.cancel.load(Ordering::Relaxed) {
                            break;
                        }
                        let index = next_archive.fetch_add(1, Ordering::Relaxed);
                        let Some(archive) = archives.get(index) else {
                            break;
                        };
                        let members = match kog_audio::archive::list_archive_names_shared(archive) {
                            Ok(members) => members,
                            Err(_) => {
                                shared.unreadable_archives.fetch_add(1, Ordering::Relaxed);
                                shared.archives_scanned.fetch_add(1, Ordering::Relaxed);
                                continue;
                            }
                        };
                        let mut found = Vec::new();
                        for member in members.iter() {
                            if shared.cancel.load(Ordering::Relaxed) {
                                break;
                            }
                            let entry = member.trim_end_matches('/');
                            if entry.is_empty()
                                || kog_core::media_path::is_metadata(std::path::Path::new(entry))
                            {
                                continue;
                            }
                            let container = member.ends_with('/');
                            if !container {
                                let Some(extension) = std::path::Path::new(entry)
                                    .extension()
                                    .and_then(|extension| extension.to_str())
                                    .map(str::to_ascii_lowercase)
                                else {
                                    continue;
                                };
                                if !extensions.contains(&extension) {
                                    continue;
                                }
                            }
                            let base = entry.rsplit('/').next().unwrap_or(entry);
                            if matched(base) {
                                found.push(SearchMatch {
                                    name: base.to_owned(),
                                    path: archive.to_string_lossy().into_owned(),
                                    entry: entry.to_owned(),
                                    kind: "archive",
                                    is_dir: container,
                                });
                                if found.len() >= SEARCH_MATCH_LIMIT {
                                    break;
                                }
                            }
                        }
                        if !found.is_empty() {
                            let mut all = shared.matches.lock().unwrap();
                            let remaining = SEARCH_MATCH_LIMIT.saturating_sub(all.len());
                            all.extend(found.into_iter().take(remaining));
                            if all.len() >= SEARCH_MATCH_LIMIT {
                                shared.limited.store(true, Ordering::Relaxed);
                            }
                        }
                        shared.archives_scanned.fetch_add(1, Ordering::Relaxed);
                    }
                });
            }
        });
    }
    finish(&mut matches, limited || shared.limited.load(Ordering::Relaxed), &shared);
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
    let decoders = kog_audio::decoder::DecoderRegistry::new(
        kog_audio::settings::AppSettings::load().decoder_settings(),
    );
    let mut playable_extensions = decoders.audio_extensions();
    playable_extensions.extend(
        Playlist::supported_extensions()
            .iter()
            .map(|extension| extension.to_ascii_lowercase()),
    );
    Ok(archive_listing_from_names(
        archive_path,
        subpath,
        members,
        &playable_extensions,
    ))
}

fn archive_listing_from_names(
    archive_path: &std::path::Path,
    subpath: &str,
    members: Vec<String>,
    playable_extensions: &HashSet<String>,
) -> serde_json::Value {
    let prefix = if subpath.is_empty() {
        String::new()
    } else {
        format!("{}/", subpath.trim_matches('/'))
    };
    let archive = archive_path.to_string_lossy().into_owned();
    let current_path = if prefix.is_empty() {
        archive.clone()
    } else {
        format!("{archive}/{}", prefix.trim_end_matches('/'))
    };
    let mut directory_names: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut files: Vec<serde_json::Value> = Vec::new();
    // libarchive may report a directory as a bare member without a trailing
    // slash. Derive directories from descendants so such entries can never
    // become bogus playlist tracks.
    let member_directories = kog_audio::archive::member_directory_names(&members);
    for member in members {
        let normalized = member.replace('\\', "/");
        if member_directories.contains(normalized.trim_end_matches('/')) {
            continue;
        }
        let Some(relative) = normalized.strip_prefix(&prefix) else {
            continue;
        };
        if relative.is_empty() {
            continue;
        }
        let playable = std::path::Path::new(relative)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| playable_extensions.contains(&extension.to_ascii_lowercase()));
        if !playable || crate::media_filter::is_hidden(std::path::Path::new(relative)) {
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
                "path": format!("{current_path}/{name}"),
            })
        })
        .collect();
    serde_json::json!({
        "path": current_path,
        "directories": directories,
        "files": files,
    })
}

#[derive(Default)]
pub struct SearchState {
    generation: AtomicU64,
    job: Mutex<Option<SearchJob>>,
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

/// `PUT /api/playlists/{id}/entries` — replace a playlist's entries, the
/// "overwrite" path when a save reuses an existing name. Favorites (0) are
/// refused like everywhere else.
pub async fn replace_playlist_entries(
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
        let count = entries.len();
        library.db().replace_entries(id, &entries)?;
        Ok::<_, String>(serde_json::json!({ "ok": true, "id": id, "count": count }))
    })
    .await
    .unwrap_or_else(|error| Err(format!("replacing the playlist failed: {error}")));
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
        .route("/api/library/collect", get(collect_folder_http))
        .route("/api/library/search", get(search))
        .route("/api/library/search/more", get(search_more))
        .route("/api/library/search/pause", post(pause_search))
        .route("/api/media/download", get(media_download))
        .route("/api/art", get(art))
        .route("/api/metadata", get(metadata_one).post(metadata_batch))
        .route("/api/expand", post(expand_entries))
        .route("/api/playlists", get(list_playlists).post(create_playlist))
        .route(
            "/api/playlists/{id}",
            get(playlist_entries).delete(delete_playlist),
        )
        .route("/api/playlists/{id}/rename", post(rename_playlist))
        .route("/api/playlists/{id}/move", post(move_playlist))
        .route(
            "/api/playlists/{id}/entries",
            post(append_playlist_entries).put(replace_playlist_entries),
        )
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
    use std::path::Path;

    #[test]
    fn web_cover_uses_shared_search_then_cache_without_redownloading() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("song.mp3");
        let jpeg = vec![0xff, 0xd8, 0xff, 0xe0];
        let mut requested = Vec::new();
        let resolved = cached_or_downloaded_cover(
            &file,
            "Artist",
            "Album",
            directory.path(),
            true,
            |url, _| {
                requested.push(url.to_owned());
                if url.starts_with("https://api.deezer.com/") {
                    Some(br#"{"data":[{"title":"Album","artist":{"name":"Artist"},"cover_xl":"https://example.test/cover.jpg"}]}"#.to_vec())
                } else if url == "https://example.test/cover.jpg" {
                    Some(jpeg.clone())
                } else {
                    None
                }
            },
        );
        assert_eq!(resolved, Some(jpeg.clone()));
        assert_eq!(requested.len(), 2);
        assert_eq!(
            cached_or_downloaded_cover(
                &file,
                "Artist",
                "Album",
                directory.path(),
                false,
                |_, _| panic!("cached art should not request the network"),
            ),
            Some(jpeg),
        );
        assert_eq!(
            cached_or_downloaded_cover(
                &file,
                "",
                "",
                directory.path(),
                true,
                |_, _| panic!("untagged files should not request the network"),
            ),
            None,
        );
    }

    #[test]
    fn archive_browse_omits_images_and_explicit_directory_members() {
        let archive = Path::new("/music/album.rar");
        let members = vec![
            "Disc".to_owned(),
            "Disc/01 Theme.MP3".to_owned(),
            "Disc/cover.jpg".to_owned(),
            "Disc/BK".to_owned(),
            "Disc/BK/page.jpg".to_owned(),
            "Disc\\02 End.flac".to_owned(),
        ];
        let extensions = HashSet::from(["mp3".to_owned(), "flac".to_owned()]);

        let root = archive_listing_from_names(archive, "", members.clone(), &extensions);
        assert_eq!(root["directories"].as_array().unwrap().len(), 1);
        assert_eq!(root["directories"][0]["name"], "Disc");
        assert!(root["files"].as_array().unwrap().is_empty());

        let disc = archive_listing_from_names(archive, "Disc", members, &extensions);
        let files = disc["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0]["name"], "01 Theme.MP3");
        assert_eq!(files[1]["entry"], "Disc\\02 End.flac");
        assert!(disc["directories"].as_array().unwrap().is_empty());
    }

    #[test]
    fn terminal_can_browse_above_music_root_without_exposing_it_over_http() {
        let directory = tempfile::tempdir().unwrap();
        let music = directory.path().join("music");
        let sibling = directory.path().join("other");
        std::fs::create_dir(&music).unwrap();
        std::fs::create_dir(&sibling).unwrap();
        std::fs::write(sibling.join("outside.zip"), b"not an archive").unwrap();
        let library = Arc::new(Library::new(
            Some(music),
            LibraryDb::open_in_memory().unwrap(),
        ));

        let local = browse_local_unrestricted(&library, sibling.to_str()).unwrap();
        assert_eq!(local["path"], sibling.to_string_lossy().as_ref());
        assert_eq!(local["directories"][0]["name"], "outside.zip");
        assert!(browse_local(&library, sibling.to_str())
            .unwrap_err()
            .contains("outside the music directory"));
        assert!(browse_local(&library, sibling.join("outside.zip").to_str())
            .unwrap_err()
            .contains("outside the music directory"));
    }

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
            file_size_bytes: None,
        }
    }

    #[test]
    fn metadata_serializes_exact_file_bytes() {
        let row = MetadataRow::from_properties(StreamProperties::default(), Some(1_572_864));
        let json = serde_json::to_value(row).unwrap();
        assert_eq!(json["fileSizeBytes"], 1_572_864);
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

    #[test]
    fn expand_single_file_passes_through_bare() {
        let directory = tempfile::tempdir().unwrap();
        let wav = directory.path().join("song.wav");
        write_test_wav(&wav);
        let decoders = kog_audio::decoder::DecoderRegistry::new(
            kog_audio::settings::AppSettings::load().decoder_settings(),
        );
        let tracks = expand_locator(
            &decoders,
            None,
            &ExpandEntry {
                kind: "local".to_owned(),
                path: wav.to_string_lossy().into_owned(),
                entry: None,
                fragment: None,
                name: None,
            },
        );
        assert_eq!(tracks.len(), 1, "a plain file is one track");
        assert_eq!(tracks[0].fragment, None);
        assert_eq!(tracks[0].name, "song.wav");
    }

    #[test]
    fn expand_playlist_file_yields_its_tracks() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("a.wav");
        let second = directory.path().join("b.wav");
        write_test_wav(&first);
        write_test_wav(&second);
        let list = directory.path().join("list.m3u");
        std::fs::write(&list, "a.wav\nb.wav\n").unwrap();
        let decoders = kog_audio::decoder::DecoderRegistry::new(
            kog_audio::settings::AppSettings::load().decoder_settings(),
        );
        let tracks = expand_locator(
            &decoders,
            None,
            &ExpandEntry {
                kind: "local".to_owned(),
                path: list.to_string_lossy().into_owned(),
                entry: None,
                fragment: None,
                name: None,
            },
        );
        assert_eq!(tracks.len(), 2, "a playlist expands to its tracks");
    }

    #[test]
    fn expand_multi_song_file_yields_one_track_per_subsong() {
        let directory = tempfile::tempdir().unwrap();
        let nsf = directory.path().join("game.nsf");
        write_test_nsf(&nsf, 3);
        let decoders = kog_audio::decoder::DecoderRegistry::new(
            kog_audio::settings::AppSettings::load().decoder_settings(),
        );
        let tracks = expand_locator(
            &decoders,
            None,
            &ExpandEntry {
                kind: "local".to_owned(),
                path: nsf.to_string_lossy().into_owned(),
                entry: None,
                fragment: None,
                name: None,
            },
        );
        assert_eq!(tracks.len(), 3, "each subsong is its own track");
        let fragments: Vec<_> = tracks
            .iter()
            .map(|track| track.fragment.clone().unwrap_or_default())
            .collect();
        assert_eq!(fragments, vec!["0", "1", "2"]);
        assert!(
            tracks.iter().all(|track| track.name.ends_with(']')),
            "subsongs are disambiguated like the desktop: {tracks:?}"
        );
    }

    #[test]
    fn expand_specific_locator_passes_through_untouched() {
        let decoders = kog_audio::decoder::DecoderRegistry::new(
            kog_audio::settings::AppSettings::load().decoder_settings(),
        );
        let tracks = expand_locator(
            &decoders,
            None,
            &ExpandEntry {
                kind: "local".to_owned(),
                path: "/music/game.nsf".to_owned(),
                entry: None,
                fragment: Some("2".to_owned()),
                name: Some("Game [3]".to_owned()),
            },
        );
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].fragment.as_deref(), Some("2"));
        assert_eq!(tracks[0].name, "Game [3]");
    }

    #[test]
    fn expand_rejects_garbage_quietly() {
        let decoders = kog_audio::decoder::DecoderRegistry::new(
            kog_audio::settings::AppSettings::load().decoder_settings(),
        );
        for request in [
            ExpandEntry {
                kind: "bogus".to_owned(),
                path: "/music/x".to_owned(),
                entry: None,
                fragment: None,
                name: None,
            },
            ExpandEntry {
                kind: "local".to_owned(),
                path: "/music/does-not-exist.nsf".to_owned(),
                entry: None,
                fragment: None,
                name: None,
            },
            ExpandEntry {
                kind: "local".to_owned(),
                path: String::new(),
                entry: None,
                fragment: None,
                name: None,
            },
        ] {
            assert!(
                expand_locator(&decoders, None, &request).is_empty(),
                "unresolvable locators expand to nothing: {request:?}"
            );
        }
    }

    /// A few silent frames in a WAV container: enough for structural
    /// expansion, which never decodes.
    fn write_test_wav(path: &Path) {
        let data = vec![0u8; 800];
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8_000_u32.to_le_bytes());
        wav.extend_from_slice(&8_000_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&8_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(&data);
        std::fs::write(path, wav).unwrap();
    }

    /// A minimal NSF: a valid 128-byte header advertising `songs` tracks and
    /// two RTS opcodes as its code. Expansion only reads the header, so this
    /// never executes.
    fn write_test_nsf(path: &Path, songs: u8) {
        let mut header = vec![0u8; 128];
        header[0..5].copy_from_slice(b"NESM\x1a");
        header[5] = 1;
        header[6] = songs;
        header[7] = 1;
        header[8..10].copy_from_slice(&0x8000_u16.to_le_bytes());
        header[10..12].copy_from_slice(&0x8000_u16.to_le_bytes());
        header[12..14].copy_from_slice(&0x8001_u16.to_le_bytes());
        header[14..18].copy_from_slice(b"game");
        let mut bytes = header;
        bytes.extend_from_slice(&[0x60, 0x60]);
        std::fs::write(path, bytes).unwrap();
    }
}
