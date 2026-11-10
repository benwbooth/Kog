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
    /// Fingerprints of `covers`' images, to add only different ones.
    prints: Vec<u64>,
    /// Still being worked on; `covers` holds the last finished answer (or
    /// what has been found so far, for a new playlist).
    pending: bool,
    /// The fill pass went through every song for this key. Until then a tile
    /// short of [`COVERS_PER_PLAYLIST`] covers takes more from the rest of
    /// the playlist's songs (one folder can hold several albums, as C64 tunes
    /// filed by musician do), and counts as pending.
    filled: bool,
}

impl Tile {
    fn short(&self) -> bool {
        self.covers.len() < COVERS_PER_PLAYLIST && !self.filled
    }
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
                if tile.key != key {
                    tile.filled = false;
                }
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
    fn update(&self, id: i64, key: u64, covers: &[(String, u64)], done: bool) {
        let mut tiles = lock(&self.tiles);
        if let Some(tile) = tiles.by_id.get_mut(&id).filter(|tile| tile.key == key) {
            // A stale tile keeps its old covers until the new ones are all
            // found, so it doesn't flicker; a new one shows covers as they come.
            if done || tile.covers.len() <= covers.len() {
                tile.covers = covers.iter().map(|(song, _)| song.clone()).collect();
                tile.prints = covers.iter().map(|(_, print)| *print).collect();
            }
            tile.pending = !done;
        }
    }

    /// Whether playlist `id`'s tile (for `key`) still takes more covers.
    fn short(&self, id: i64, key: u64) -> bool {
        lock(&self.tiles).by_id.get(&id).is_some_and(|tile| tile.key == key && tile.short())
    }

    /// The fill pass found `song`'s cover: add it to a short tile if it is a
    /// different image. Returns whether the tile still takes more.
    fn offer(&self, id: i64, key: u64, song: &str, print: u64) -> bool {
        let mut tiles = lock(&self.tiles);
        let Some(tile) = tiles.by_id.get_mut(&id).filter(|tile| tile.key == key) else { return false };
        if tile.short() && !tile.prints.contains(&print) && !tile.covers.iter().any(|cover| cover == song) {
            tile.covers.push(song.to_owned());
            tile.prints.push(print);
        }
        tile.short()
    }

    /// The fill pass went through all of playlist `id`'s songs.
    fn filled(&self, id: i64, key: u64) {
        if let Some(tile) = lock(&self.tiles).by_id.get_mut(&id).filter(|tile| tile.key == key) {
            tile.filled = true;
        }
    }

    fn key(&self, id: i64) -> Option<u64> {
        lock(&self.tiles).by_id.get(&id).map(|tile| tile.key)
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
                    // A short tile may still get more from the fill pass.
                    "pending": tile.pending || tile.short(),
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
        mut found: impl FnMut(&[(String, u64)]),
    ) -> Vec<(String, u64)> {
        let mut chosen = Vec::<(String, u64)>::new();
        for song in songs {
            if chosen.len() >= COVERS_PER_PLAYLIST {
                break;
            }
            let Some(bytes) = self.cover(service, &song, allow_download).await else { continue };
            let print = image_print(&bytes);
            if !chosen.iter().any(|(_, seen)| *seen == print) {
                chosen.push((song, print));
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
                    // (the next pass resumes after redoing their tiles). A
                    // tile still short of covers takes different ones from
                    // these, so it looks at every song of its playlist.
                    let mut checked = 0;
                    'fill: for (id, songs) in &lists {
                        let key = service.key(*id);
                        let mut short = key.is_some_and(|key| service.short(*id, key));
                        for song in songs {
                            if !short && !service.needs_lookup(song, allow_download) {
                                continue;
                            }
                            checked += 1;
                            if checked % FILL_CHANGE_CHECK == 0 && worker.stamp().await != stamp {
                                interrupted = true;
                                break 'fill;
                            }
                            let bytes = worker.cover(&service, song, allow_download).await;
                            if let (true, Some(key), Some(bytes)) = (short, key, bytes) {
                                short = service.offer(*id, key, song, image_print(&bytes));
                            }
                        }
                        if let Some(key) = key {
                            service.filled(*id, key);
                        }
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
    fn a_short_tile_takes_different_covers_from_the_fill_pass_until_filled() {
        let covers = PlaylistCovers::default();
        let songs: Vec<String> = (1..=6).map(|n| format!("/c64/Hubbard/{n}.sid")).collect();
        let work = covers.plan(&[(7, songs.clone())], false);
        let key = work[0].1;
        // One folder: the tile pass found only the first song's cover.
        covers.update(7, key, &[(songs[0].clone(), 10)], true);
        assert_eq!(covers.snapshot()["playlists"][0]["pending"], true);
        assert!(covers.short(7, key));
        assert!(covers.offer(7, key, &songs[0], 10));
        assert!(covers.offer(7, key, &songs[1], 10)); // the same image
        assert!(covers.offer(7, key, &songs[2], 11));
        assert!(covers.offer(7, key, &songs[3], 12));
        assert!(!covers.offer(7, key, &songs[4], 13));
        assert!(!covers.offer(7, key, &songs[5], 14));
        let snapshot = covers.snapshot();
        let paths: Vec<_> = snapshot["playlists"][0]["covers"]
            .as_array()
            .unwrap()
            .iter()
            .map(|cover| cover["path"].as_str().unwrap().to_owned())
            .collect();
        assert_eq!(paths, [&songs[0], &songs[2], &songs[3], &songs[4]]);
        assert_eq!(snapshot["playlists"][0]["pending"], false);

        // A playlist with fewer albums than that is done once filled.
        let work = covers.plan(&[(8, songs[..1].to_vec())], false);
        covers.update(8, work[0].1, &[(songs[0].clone(), 10)], true);
        assert_eq!(covers.snapshot()["playlists"][0]["pending"], true);
        covers.filled(8, work[0].1);
        assert_eq!(covers.snapshot()["playlists"][0]["pending"], false);
        // Replanning the same songs keeps it filled.
        assert!(covers.plan(&[(8, songs[..1].to_vec())], false).is_empty());
        assert!(!covers.short(8, work[0].1));
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
        let found = [("/music/A/1.mp3".to_owned(), 1)];
        for (id, key, _) in &work {
            covers.update(*id, *key, &found, true);
        }
        assert!(covers.plan(&[(0, songs.clone()), (5, songs.clone())], true).is_empty());

        // Playlist 5 changed and 0 was removed from the store's answer.
        let changed = vec!["/music/B/1.mp3".to_owned()];
        let found_changed = [("/music/B/1.mp3".to_owned(), 2)];
        let work = covers.plan(&[(5, changed.clone())], true);
        assert_eq!(work.len(), 1);
        let snapshot = covers.snapshot();
        assert_eq!(snapshot["playlists"].as_array().unwrap().len(), 1);
        assert_eq!(snapshot["playlists"][0]["pending"], true);
        // Until the new covers are all found, the old one stays up.
        covers.update(5, work[0].1, &[], false);
        assert_eq!(covers.snapshot()["playlists"][0]["covers"][0]["path"], "/music/A/1.mp3");
        covers.update(5, work[0].1, &found_changed, true);
        covers.filled(5, work[0].1);
        let snapshot = covers.snapshot();
        assert_eq!(snapshot["playlists"][0]["covers"][0]["path"], "/music/B/1.mp3");
        assert_eq!(snapshot["playlists"][0]["pending"], false);
        // Progress for an outdated key is ignored.
        covers.update(5, work[0].1 ^ 1, &found, true);
        assert_eq!(covers.snapshot()["playlists"][0]["covers"][0]["path"], "/music/B/1.mp3");
    }
}
