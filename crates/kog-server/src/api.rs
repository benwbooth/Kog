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

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use kog_audio::decoder::StreamProperties;
use kog_audio::playlist::{PlaylistEntry, PlaylistLocation};
use kog_core::db::{LibraryDb, StoredEntry};

use crate::routes::{AppState, bad_request};

/// Shared library access for the handlers.
pub struct Library {
    root: Mutex<Option<PathBuf>>,
    db: Mutex<LibraryDb>,
    /// Tag lookups are the expensive part of a playlist refresh, so the server
    /// remembers each answer (including "no metadata") across requests.
    metadata: Mutex<MetadataCache>,
}

impl Library {
    pub fn new(root: Option<PathBuf>, db: LibraryDb) -> Self {
        Self {
            root: Mutex::new(root),
            db: Mutex::new(db),
            metadata: Mutex::new(MetadataCache::default()),
        }
    }

    /// Open the real library: the music directory from settings and Kog's
    /// database, falling back to an in-memory store when it cannot be opened
    /// (the server still serves streams and the codec list).
    pub fn open() -> Self {
        let settings = kog_audio::settings::AppSettings::load();
        let db = LibraryDb::open().or_else(|_| LibraryDb::open_in_memory());
        Self::new(
            settings.music_directory.clone(),
            db.unwrap_or_else(|_| {
                LibraryDb::open_in_memory().expect("in-memory library always opens")
            }),
        )
    }

    pub fn root(&self) -> Option<PathBuf> {
        lock(&self.root).clone()
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

fn browse_blocking(library: &Arc<Library>, requested: Option<&str>) -> Result<serde_json::Value, String> {
    let directory = library.resolve(requested)?;
    if !directory.is_dir() {
        return Err(format!("{} is not a directory", directory.display()));
    }
    let decoders = kog_audio::decoder::DecoderRegistry::new(
        kog_audio::settings::AppSettings::load().decoder_settings(),
    );
    let mut directories = Vec::new();
    let mut files = Vec::new();
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
            files.push(path);
        }
    }
    directories.sort();
    files.sort();
    let root = library.root();
    let list = |paths: Vec<PathBuf>| -> Vec<serde_json::Value> {
        paths
            .into_iter()
            .map(|path| {
                serde_json::json!({
                    "name": path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
                    "path": path.to_string_lossy(),
                    "relative": root
                        .as_deref()
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
        "files": list(files),
    }))
}

/// `GET /api/library/search` — filename search under the music directory.
pub async fn search(State(state): State<AppState>, Query(query): Query<SearchQuery>) -> Response {
    let needle = query.q.trim().to_lowercase();
    if needle.is_empty() {
        return bad_request("provide a search query");
    }
    let limit = query.limit.unwrap_or(200).min(1_000);
    let library = state.library.clone();
    let result = tokio::task::spawn_blocking(move || search_blocking(&library, &needle, limit))
        .await
        .unwrap_or_else(|error| Err(format!("library search failed: {error}")));
    match result {
        Ok(value) => axum::Json(value).into_response(),
        Err(error) => bad_request(&error),
    }
}

fn search_blocking(
    library: &Arc<Library>,
    needle: &str,
    limit: usize,
) -> Result<serde_json::Value, String> {
    let root = library.resolve(None)?;
    let decoders = kog_audio::decoder::DecoderRegistry::new(
        kog_audio::settings::AppSettings::load().decoder_settings(),
    );
    let mut found = Vec::new();
    let mut pending = vec![root.clone()];
    while let Some(directory) = pending.pop() {
        if found.len() >= limit {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if crate::media_filter::is_hidden(&path) {
                continue;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file()
                && decoders.accepts_path(&path)
                && path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_lowercase().contains(needle))
                    .unwrap_or(false)
            {
                found.push(path);
                if found.len() >= limit {
                    break;
                }
            }
        }
    }
    found.sort();
    Ok(serde_json::json!({
        "results": found
            .into_iter()
            .map(|path| {
                let relative = path
                    .strip_prefix(&root)
                    .map(|relative| relative.to_string_lossy().into_owned())
                    .unwrap_or_default();
                serde_json::json!({
                    "name": path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
                    "path": path.to_string_lossy(),
                    "relative": relative,
                })
            })
            .collect::<Vec<_>>(),
    }))
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
pub fn router() -> axum::Router<AppState> {
    use axum::routing::{get, post};
    axum::Router::new()
        .route("/api/library", get(browse))
        .route("/api/library/search", get(search))
        .route("/api/metadata", get(metadata_one).post(metadata_batch))
        .route("/api/playlists", get(list_playlists).post(create_playlist))
        .route(
            "/api/playlists/{id}",
            get(playlist_entries).delete(delete_playlist),
        )
        .route("/api/playlists/{id}/entries", post(append_playlist_entries))
        .route("/api/stars", get(list_stars).post(set_star))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(title: &str) -> MetadataRow {
        MetadataRow {
            title: Some(title.to_owned()),
            artist: None,
            album: None,
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
