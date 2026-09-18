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

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use kog_core::db::{LibraryDb, StoredEntry};

use crate::routes::{AppState, bad_request};

/// Shared library access for the handlers.
pub struct Library {
    root: Mutex<Option<PathBuf>>,
    db: Mutex<LibraryDb>,
}

impl Library {
    pub fn new(root: Option<PathBuf>, db: LibraryDb) -> Self {
        Self {
            root: Mutex::new(root),
            db: Mutex::new(db),
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
    use axum::routing::get;
    axum::Router::new()
        .route("/api/library", get(browse))
        .route("/api/library/search", get(search))
        .route("/api/playlists", get(list_playlists).post(create_playlist))
        .route(
            "/api/playlists/{id}",
            get(playlist_entries).delete(delete_playlist),
        )
        .route("/api/stars", get(list_stars).post(set_star))
}
