//! Playlist covers, found in the background. Phones show each playlist with
//! covers from its songs; rather than every client walking every playlist
//! and asking for art song by song, the server walks them itself, at startup
//! and whenever the playlists or the stars change, running the ordinary
//! cover lookup one song at a time. First each playlist's tile: songs from
//! different folders until four different covers are found. Then every
//! other song of every playlist, so all their covers are found and
//! downloaded into the cover cache before anyone plays them. Clients read
//! which songs a tile shows from `GET /api/playlists/covers` and load those
//! images through `/api/art` as usual.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use kog_core::db::StoredEntry;
use tokio::sync::Notify;

use crate::covers::{CoverResult, wait};
use crate::routes::AppState;

/// The most covers a tile shows (the "Four album covers" grid).
pub(crate) const COVERS_PER_PLAYLIST: usize = 4;
/// How often the worker checks whether another program (the desktop app,
/// the terminal player) changed the playlists or stars in the shared library.
const CHANGE_CHECK: Duration = Duration::from_secs(30);
/// Clients asking for the covers also make the worker look for changes made
/// through this server, but not more often than this.
const REQUESTED_CHECK: Duration = Duration::from_secs(2);
/// A pause between two lookups, so this worker never crowds out covers that
/// a listener is waiting for.
const LOOKUP_PAUSE: Duration = Duration::from_millis(100);
/// While filling in every song, the worker checks this often (in songs)
/// whether the playlists changed, to redo the changed tiles first.
const FILL_CHANGE_CHECK: usize = 16;

/// Favorites, which are the stars rather than a stored playlist.
const FAVORITES: i64 = 0;

#[derive(Clone, Default)]
struct Tile {
    /// What the covers were (or are being) found for: the candidate songs
    /// and the download setting. A different key means the tile is stale.
    key: u64,
    /// Library paths of the songs whose covers to show, in playlist order,
    /// each a different image.
    covers: Vec<String>,
    /// Still being worked on; `covers` holds the last finished answer (or
    /// what has been found so far, for a new playlist).
    pending: bool,
}

#[derive(Default)]
struct Tiles {
    /// In display order: Favorites first, then the playlists.
    order: Vec<i64>,
    by_id: HashMap<i64, Tile>,
}

#[derive(Default)]
pub(crate) struct PlaylistCovers {
    tiles: Mutex<Tiles>,
    /// Wakes the worker to look for changes now.
    wake: Notify,
    started: AtomicBool,
    last_requested: Mutex<Option<Instant>>,
    /// Songs already looked up (found, missing, or unreadable) with this
    /// download setting, so neither pass asks again.
    looked_up: Mutex<(bool, HashSet<String>)>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

/// A playlist's songs that have covers, each once, in playlist order.
/// Archive members share their container's cover, so the container stands
/// in for them.
fn songs(entries: &[StoredEntry]) -> Vec<String> {
    let mut seen = HashSet::new();
    entries
        .iter()
        .filter(|entry| entry.kind == "local" || entry.kind == "archive")
        .filter(|entry| seen.insert(entry.path.as_str()))
        .map(|entry| entry.path.clone())
        .collect()
}

/// The tile's candidates: one song per folder (the songs of an album in one
/// folder share a cover), in playlist order.
fn candidates(songs: &[String]) -> Vec<String> {
    let mut folders = HashSet::new();
    songs
        .iter()
        .filter(|song| folders.insert(song.rsplit_once('/').map(|(folder, _)| folder).unwrap_or_default()))
        .cloned()
        .collect()
}

fn tile_key(songs: &[String], allow_download: bool) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    songs.hash(&mut hasher);
    allow_download.hash(&mut hasher);
    hasher.finish()
}

fn image_print(bytes: &[u8]) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// Every playlist's songs, Favorites first.
fn read_playlists(library: &crate::api::Library) -> Result<Vec<(i64, Vec<String>)>, String> {
    let db = library.db();
    let mut lists = vec![(FAVORITES, songs(&db.starred_entries()?))];
    for playlist in db.list_playlists()? {
        lists.push((playlist.id, songs(&db.playlist_entries(playlist.id)?)));
    }
    Ok(lists)
}

/// Changes when another connection commits, or when the playlists or stars
/// change through this one.
fn change_stamp(library: &crate::api::Library) -> (i64, String) {
    let db = library.db();
    (db.data_version(), db.playlists_fingerprint())
}

impl PlaylistCovers {
    /// Start the worker for this server, once. Called when the server starts
    /// and again by the endpoint, for servers built without `serve`.
    pub(crate) fn start(state: &AppState) {
        if state.playlist_covers.started.swap(true, Ordering::SeqCst) {
            return;
        }
        let this = Arc::downgrade(&state.playlist_covers);
        let library = state.library.clone();
        let covers = state.covers.clone();
        let streams = state.streams.clone();
        tokio::spawn(run(this, library, covers, streams));
    }

    /// Mark changed playlists pending and drop removed ones. Returns the
    /// playlists to work on, in display order.
    fn plan(&self, lists: &[(i64, Vec<String>)], allow_download: bool) -> Vec<(i64, u64, Vec<String>)> {
        let mut tiles = lock(&self.tiles);
        let mut work = Vec::new();
        let mut by_id = HashMap::with_capacity(lists.len());
        let mut order = Vec::with_capacity(lists.len());
        for (id, songs) in lists {
            let id = *id;
            let songs = candidates(songs);
            let key = tile_key(&songs, allow_download);
            let mut tile = tiles.by_id.remove(&id).unwrap_or_default();
            if tile.key != key || tile.pending {
                tile.key = key;
                tile.pending = true;
                work.push((id, key, songs));
            }
            order.push(id);
            by_id.insert(id, tile);
        }
        *tiles = Tiles { order, by_id };
        work
    }

    /// Record progress for a playlist, unless it changed meanwhile (then the
    /// next pass has its own work for it).
    fn update(&self, id: i64, key: u64, covers: &[String], done: bool) {
        let mut tiles = lock(&self.tiles);
        if let Some(tile) = tiles.by_id.get_mut(&id).filter(|tile| tile.key == key) {
            // A stale tile keeps its old covers until the new ones are all
            // found, so it doesn't flicker; a new one shows covers as they come.
            if done || tile.covers.len() <= covers.len() {
                tile.covers = covers.to_vec();
            }
            tile.pending = !done;
        }
    }

    fn snapshot(&self) -> serde_json::Value {
        let tiles = lock(&self.tiles);
        serde_json::json!({
            "playlists": tiles
                .order
                .iter()
                .filter_map(|id| tiles.by_id.get(id).map(|tile| (id, tile)))
                .map(|(id, tile)| serde_json::json!({
                    "id": id,
                    // Addressed as `/api/art` addresses a song.
                    "covers": tile
                        .covers
                        .iter()
                        .map(|path| serde_json::json!({ "kind": "local", "path": path }))
                        .collect::<Vec<_>>(),
                    "pending": tile.pending,
                }))
                .collect::<Vec<_>>(),
        })
    }

    /// Whether `song` still needs a lookup with this download setting; a
    /// changed setting starts over, since a missing cover may now download.
    fn needs_lookup(&self, song: &str, allow_download: bool) -> bool {
        let mut looked_up = lock(&self.looked_up);
        if looked_up.0 != allow_download {
            *looked_up = (allow_download, HashSet::new());
        }
        !looked_up.1.contains(song)
    }

    fn looked_up(&self, song: String) {
        lock(&self.looked_up).1.insert(song);
    }

    /// A client asked: look for changes made through this server soon.
    fn requested(&self) {
        let mut last = lock(&self.last_requested);
        if last.is_none_or(|last| last.elapsed() >= REQUESTED_CHECK) {
            *last = Some(Instant::now());
            self.wake.notify_one();
        }
    }
}

/// What the worker works with: the server's library, covers and streams.
struct Worker {
    library: Arc<crate::api::Library>,
    covers: Arc<crate::covers::CoverService>,
    streams: Arc<crate::service::StreamService>,
}

impl Worker {
    /// The cover of one song, through the ordinary lookup (which answers
    /// from the cover cache, or its record of a miss, without searching).
    async fn cover(&self, service: &PlaylistCovers, song: &str, allow_download: bool) -> Option<axum::body::Bytes> {
        let library = self.library.clone();
        let path = song.to_owned();
        let file = tokio::task::spawn_blocking(move || {
            library.resolve(Some(&path)).ok().filter(|file| file.is_file())
        })
        .await
        .ok()
        .flatten();
        let result = match file {
            Some(file) => wait(self.covers.lookup(file, self.streams.clone(), allow_download)).await,
            None => CoverResult::Missing,
        };
        service.looked_up(song.to_owned());
        tokio::time::sleep(LOOKUP_PAUSE).await;
        match result {
            CoverResult::Ready(bytes) => Some(bytes),
            _ => None,
        }
    }

    /// The covers of up to [`COVERS_PER_PLAYLIST`] of `songs`, each a
    /// different image, found one lookup at a time. Reports progress
    /// through `found`.
    async fn tile(
        &self,
        service: &PlaylistCovers,
        songs: Vec<String>,
        allow_download: bool,
        mut found: impl FnMut(&[String]),
    ) -> Vec<String> {
        let mut chosen = Vec::<String>::new();
        let mut prints = Vec::<u64>::new();
        for song in songs {
            if chosen.len() >= COVERS_PER_PLAYLIST {
                break;
            }
            let Some(bytes) = self.cover(service, &song, allow_download).await else { continue };
            let print = image_print(&bytes);
            if !prints.contains(&print) {
                prints.push(print);
                chosen.push(song);
                found(&chosen);
            }
        }
        chosen
    }

    async fn stamp(&self) -> Option<(i64, String)> {
        let library = self.library.clone();
        tokio::task::spawn_blocking(move || change_stamp(&library)).await.ok()
    }
}

async fn run(
    this: Weak<PlaylistCovers>,
    library: Arc<crate::api::Library>,
    covers: Arc<crate::covers::CoverService>,
    streams: Arc<crate::service::StreamService>,
) {
    let worker = Worker { library, covers, streams };
    let mut stamp = None;
    let mut force = true;
    loop {
        let mut interrupted = false;
        // The server was stopped: its state, and this worker, go with it.
        let Some(service) = this.upgrade() else { return };
        let current = worker.stamp().await;
        if force || current != stamp {
            stamp = current;
            let allow_download = kog_audio::settings::AppSettings::load().download_cover_art;
            let library = worker.library.clone();
            let lists = tokio::task::spawn_blocking(move || read_playlists(&library))
                .await
                .unwrap_or_else(|error| Err(error.to_string()));
            match lists {
                Ok(lists) => {
                    // The tiles first: they are what the playlist list shows.
                    for (id, key, songs) in service.plan(&lists, allow_download) {
                        let chosen = worker
                            .tile(&service, songs, allow_download, |found| {
                                service.update(id, key, found, false)
                            })
                            .await;
                        service.update(id, key, &chosen, true);
                    }
                    // Then every other song, until the playlists change
                    // (the next pass resumes after redoing their tiles).
                    let mut checked = 0;
                    'fill: for song in lists.iter().flat_map(|(_, songs)| songs) {
                        if !service.needs_lookup(song, allow_download) {
                            continue;
                        }
                        checked += 1;
                        if checked % FILL_CHANGE_CHECK == 0 && worker.stamp().await != stamp {
                            interrupted = true;
                            break 'fill;
                        }
                        worker.cover(&service, song, allow_download).await;
                    }
                }
                Err(error) => eprintln!("kog-server: reading playlists for their covers failed: {error}"),
            }
        }
        // Wait for a client's request or the next change check, without
        // keeping a stopped server's state alive past it.
        force = interrupted
            || tokio::time::timeout(CHANGE_CHECK, service.wake.notified()).await.is_ok();
        drop(service);
    }
}

/// `GET /api/playlists/covers` — for every playlist (Favorites, id 0,
/// first), the songs whose covers its tile shows: up to four, each a
/// different image, in playlist order. `pending` means the server is still
/// finding them; ask again in a while.
pub async fn playlist_covers(State(state): State<AppState>) -> Response {
    PlaylistCovers::start(&state);
    state.playlist_covers.requested();
    (
        [("cache-control", "no-store")],
        axum::Json(state.playlist_covers.snapshot()),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: &str, path: &str) -> StoredEntry {
        StoredEntry {
            kind: kind.to_owned(),
            path: path.to_owned(),
            entry: String::new(),
            fragment: None,
        }
    }

    #[test]
    fn songs_once_each_and_candidates_one_per_folder_in_order() {
        let entries = [
            entry("local", "/music/A/1.mp3"),
            entry("local", "/music/A/2.mp3"),
            entry("youtube", "https://example.test/watch"),
            entry("archive", "/music/B/set.zip"),
            entry("archive", "/music/B/set.zip"),
            entry("local", "/music/C/1.flac"),
            entry("local", "/music/B/other.mp3"),
        ];
        let songs = songs(&entries);
        assert_eq!(
            songs,
            ["/music/A/1.mp3", "/music/A/2.mp3", "/music/B/set.zip", "/music/C/1.flac", "/music/B/other.mp3"]
        );
        assert_eq!(
            candidates(&songs),
            ["/music/A/1.mp3", "/music/B/set.zip", "/music/C/1.flac"]
        );
    }

    #[test]
    fn a_changed_download_setting_looks_songs_up_again() {
        let covers = PlaylistCovers::default();
        assert!(covers.needs_lookup("/music/A/1.mp3", false));
        covers.looked_up("/music/A/1.mp3".to_owned());
        assert!(!covers.needs_lookup("/music/A/1.mp3", false));
        assert!(covers.needs_lookup("/music/A/1.mp3", true));
    }

    #[test]
    fn changed_playlists_are_redone_and_stale_tiles_keep_their_covers() {
        let covers = PlaylistCovers::default();
        let songs = vec!["/music/A/1.mp3".to_owned()];
        let work = covers.plan(&[(0, songs.clone()), (5, songs.clone())], true);
        assert_eq!(work.len(), 2);
        for (id, key, _) in &work {
            covers.update(*id, *key, &songs, true);
        }
        assert!(covers.plan(&[(0, songs.clone()), (5, songs.clone())], true).is_empty());

        // Playlist 5 changed and 0 was removed from the store's answer.
        let changed = vec!["/music/B/1.mp3".to_owned()];
        let work = covers.plan(&[(5, changed.clone())], true);
        assert_eq!(work.len(), 1);
        let snapshot = covers.snapshot();
        assert_eq!(snapshot["playlists"].as_array().unwrap().len(), 1);
        assert_eq!(snapshot["playlists"][0]["pending"], true);
        // Until the new covers are all found, the old one stays up.
        covers.update(5, work[0].1, &[], false);
        assert_eq!(covers.snapshot()["playlists"][0]["covers"][0]["path"], "/music/A/1.mp3");
        covers.update(5, work[0].1, &changed, true);
        let snapshot = covers.snapshot();
        assert_eq!(snapshot["playlists"][0]["covers"][0]["path"], "/music/B/1.mp3");
        assert_eq!(snapshot["playlists"][0]["pending"], false);
        // Progress for an outdated key is ignored.
        covers.update(5, work[0].1 ^ 1, &songs, true);
        assert_eq!(covers.snapshot()["playlists"][0]["covers"][0]["path"], "/music/B/1.mp3");
    }
}
