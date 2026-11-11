//! Playlist covers, found in the background. Phones show each playlist with
//! covers from its songs; rather than every client walking every playlist
//! and asking for art song by song, the server walks them itself, at startup
//! and whenever the playlists or the stars change, running the ordinary
//! cover lookup one song at a time. First each playlist's tile: songs from
//! different folders until four different covers are found. Then every
//! other song of every playlist, so all their covers are found and
//! downloaded into the cover cache before anyone plays them. Clients read
//! which songs a tile shows from `GET /api/playlists/covers`. The worker
//! keeps each tile's images in a folder beside the cover cache, served as
//! they are by `GET /api/playlist-covers/{image}` (no lookup, cacheable for
//! good, since an image's name is its fingerprint), and remembers the tiles
//! and the songs it has looked up there too, so a restarted server shows
//! every tile at once and carries on where it stopped.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use axum::extract::{Path as UrlPath, State};
use axum::http::StatusCode;
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
/// How many lookups the worker starts ahead of the one it waits for: most of
/// a lookup is reading the song (some formats are played through to learn
/// their tags) or waiting on a cover site, which a few can do at once.
const LOOKUP_AHEAD: usize = 3;
/// While filling in every song, the worker checks this often (in songs)
/// whether the playlists changed, to redo the changed tiles first.
const FILL_CHANGE_CHECK: usize = 16;

/// Favorites, which are the stars rather than a stored playlist.
const FAVORITES: i64 = 0;
/// The folder, beside the cover cache, holding the tiles' images and what
/// the worker found.
const TILE_DIRECTORY: &str = "playlist-tiles";
const TILES_FILE: &str = "tiles.json";
const LOOKED_UP_FILE: &str = "looked-up.json";
/// The fill pass writes down the songs it has looked up every this many.
const LOOKED_UP_SAVE: usize = 64;
/// What the looked-up list was made under; a list from another version is
/// dropped, so every song is looked up again. 1: the cover cache's budget
/// grew from 200 MB, which had evicted most of what the sweep downloaded.
const LOOKED_UP_VERSION: u32 = 1;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
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

#[derive(Default, serde::Serialize, serde::Deserialize)]
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
    /// Where the tiles' images and the worker's progress are kept.
    dir: OnceLock<PathBuf>,
    /// How far the worker has read the cover cache's log of covers changed
    /// by hand.
    cover_changes: Mutex<u64>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct LookedUp {
    #[serde(default)]
    version: u32,
    allow_download: bool,
    songs: HashSet<String>,
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

/// FNV-1a, which (unlike the standard hasher) stays the same across builds:
/// keys and image names are kept on disk.
fn fnv(hash: u64, bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(hash, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3))
}

const FNV_START: u64 = 0xcbf2_9ce4_8422_2325;

fn tile_key(songs: &[String], allow_download: bool) -> u64 {
    let hash = songs
        .iter()
        .fold(FNV_START, |hash, song| fnv(fnv(hash, song.as_bytes()), &[0]));
    fnv(hash, &[u8::from(allow_download)])
}

fn image_print(bytes: &[u8]) -> u64 {
    fnv(FNV_START, bytes)
}

/// An image's name in the tile folder and in `/api/playlist-covers`.
fn image_name(print: u64) -> String {
    format!("{print:016x}")
}

/// Write `contents` whole or not at all.
fn write_file(path: &Path, contents: &[u8]) {
    let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
    if std::fs::write(&temporary, contents).is_ok() && std::fs::rename(&temporary, path).is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
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
        let dir = state
            .covers
            .cache_dir()
            .parent()
            .map(|parent| parent.join(TILE_DIRECTORY))
            .unwrap_or_else(|| std::env::temp_dir().join("kog-playlist-tiles"));
        state.playlist_covers.restore(&dir);
        let _ = state.playlist_covers.dir.set(dir);
        *lock(&state.playlist_covers.cover_changes) =
            kog_audio::cover_art::cover_changes_stamp(state.covers.cache_dir());
        let this = Arc::downgrade(&state.playlist_covers);
        let library = state.library.clone();
        let covers = state.covers.clone();
        let streams = state.streams.clone();
        tokio::spawn(run(this, library, covers, streams));
    }

    /// Take up what an earlier run of the server found: its tiles (those
    /// whose images are all still there) and the songs it looked up.
    fn restore(&self, dir: &Path) {
        if let Some(mut tiles) = std::fs::read(dir.join(TILES_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Tiles>(&bytes).ok())
        {
            for tile in tiles.by_id.values_mut() {
                if tile.covers.len() != tile.prints.len()
                    || !tile.prints.iter().all(|print| dir.join(image_name(*print)).is_file())
                {
                    // Look again, showing nothing broken meanwhile.
                    *tile = Tile::default();
                }
            }
            *lock(&self.tiles) = tiles;
        }
        if let Some(looked_up) = std::fs::read(dir.join(LOOKED_UP_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<LookedUp>(&bytes).ok())
            .filter(|looked_up| looked_up.version == LOOKED_UP_VERSION)
        {
            *lock(&self.looked_up) = (looked_up.allow_download, looked_up.songs);
        }
    }

    fn save_tiles(&self) {
        let Some(dir) = self.dir.get() else { return };
        let Ok(json) = serde_json::to_vec(&*lock(&self.tiles)) else { return };
        let _ = std::fs::create_dir_all(dir);
        write_file(&dir.join(TILES_FILE), &json);
    }

    fn save_looked_up(&self) {
        let Some(dir) = self.dir.get() else { return };
        let json = {
            let looked_up = lock(&self.looked_up);
            serde_json::to_vec(&serde_json::json!({
                "version": LOOKED_UP_VERSION,
                "allow_download": looked_up.0,
                "songs": looked_up.1,
            }))
        };
        let Ok(json) = json else { return };
        let _ = std::fs::create_dir_all(dir);
        write_file(&dir.join(LOOKED_UP_FILE), &json);
    }

    /// Keep a tile's image, named by its fingerprint.
    fn keep_image(&self, print: u64, bytes: &[u8]) {
        let Some(dir) = self.dir.get() else { return };
        let path = dir.join(image_name(print));
        if !path.is_file() {
            let _ = std::fs::create_dir_all(dir);
            write_file(&path, bytes);
        }
    }

    /// Delete the images no tile shows any more.
    fn prune_images(&self) {
        let Some(dir) = self.dir.get() else { return };
        let shown: HashSet<String> = lock(&self.tiles)
            .by_id
            .values()
            .flat_map(|tile| tile.prints.iter().map(|print| image_name(*print)))
            .collect();
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.len() == 16 && name.bytes().all(|byte| byte.is_ascii_hexdigit()) && !shown.contains(&name) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    /// The listener rejected these bytes as a cover: take them off every
    /// tile now, and look for every tile's covers again.
    pub(crate) fn rejected(&self, bytes: &[u8]) {
        self.replaced(&[image_print(bytes)]);
        *lock(&self.looked_up) = (false, HashSet::new());
        self.wake.notify_one();
    }

    /// These images no longer show their albums: take them off every tile
    /// now, and redo every tile on the next pass.
    fn replaced(&self, prints: &[u64]) {
        {
            let mut tiles = lock(&self.tiles);
            for tile in tiles.by_id.values_mut() {
                for print in prints {
                    if let Some(index) = tile.prints.iter().position(|seen| seen == print) {
                        tile.prints.remove(index);
                        tile.covers.remove(index);
                    }
                }
                // A key no songs hash to: the next pass redoes the tile.
                tile.key = 0;
                tile.filled = false;
                tile.pending = true;
            }
        }
        if let Some(dir) = self.dir.get() {
            for print in prints {
                let _ = std::fs::remove_file(dir.join(image_name(*print)));
            }
        }
        self.save_tiles();
    }

    /// Whether covers were changed by hand since the worker last looked.
    pub(crate) fn cover_changes_waiting(&self, covers: &crate::covers::CoverService) -> bool {
        kog_audio::cover_art::cover_changes_stamp(covers.cache_dir()) != *lock(&self.cover_changes)
    }

    /// Covers changed by hand by anything sharing the cover cache (the
    /// desktop app's chooser, a reject through another server): redo the
    /// tiles, without the images replaced. Returns whether any changed.
    fn follow_cover_changes(&self, covers: &crate::covers::CoverService) -> bool {
        let changes = {
            let mut seen = lock(&self.cover_changes);
            let (stamp, changes) = kog_audio::cover_art::cover_changes_since(covers.cache_dir(), *seen);
            *seen = stamp;
            changes
        };
        if changes.is_empty() {
            return false;
        }
        // Lookups remembered from before would give the old covers back.
        covers.forget_finished();
        self.replaced(&changes.into_iter().flatten().collect::<Vec<_>>());
        true
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
            if tile.key != key || tile.pending || tile.covers.len() != tile.prints.len() {
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

    #[cfg(test)]
    fn offer(&self, id: i64, key: u64, song: &str, print: u64) -> bool {
        self.offer_image(id, key, song, print, &[])
    }

    /// The fill pass found `song`'s cover (`bytes`): add it to a short tile
    /// if it is a different image, keeping the image. Returns whether the
    /// tile still takes more.
    fn offer_image(&self, id: i64, key: u64, song: &str, print: u64, bytes: &[u8]) -> bool {
        let (added, short) = {
            let mut tiles = lock(&self.tiles);
            let Some(tile) = tiles.by_id.get_mut(&id).filter(|tile| tile.key == key) else { return false };
            let added = tile.short()
                && !tile.prints.contains(&print)
                && !tile.covers.iter().any(|cover| cover == song);
            if added {
                tile.covers.push(song.to_owned());
                tile.prints.push(print);
            }
            (added, tile.short())
        };
        if added && !bytes.is_empty() {
            self.keep_image(print, bytes);
            self.save_tiles();
        }
        short
    }

    /// The fill pass went through all of playlist `id`'s songs.
    fn filled(&self, id: i64, key: u64) {
        let changed = lock(&self.tiles)
            .by_id
            .get_mut(&id)
            .filter(|tile| tile.key == key && !tile.filled)
            .map(|tile| tile.filled = true)
            .is_some();
        if changed {
            self.save_tiles();
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
                    // Each with its image, for `/api/playlist-covers/{image}`.
                    "covers": tile
                        .covers
                        .iter()
                        .zip(&tile.prints)
                        .map(|(path, print)| serde_json::json!({
                            "kind": "local",
                            "path": path,
                            "image": image_name(*print),
                        }))
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

    /// Returns how many songs have been looked up.
    fn looked_up(&self, song: String) -> usize {
        let mut looked_up = lock(&self.looked_up);
        looked_up.1.insert(song);
        looked_up.1.len()
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
        let looked_up = |service: &PlaylistCovers| {
            if service.looked_up(song.to_owned()) % LOOKED_UP_SAVE == 0 {
                service.save_looked_up();
            }
        };
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
        looked_up(service);
        tokio::time::sleep(LOOKUP_PAUSE).await;
        match result {
            CoverResult::Ready(bytes) => Some(bytes),
            _ => None,
        }
    }

    /// Start looking up `song`'s cover without waiting for it; [`Self::cover`]
    /// then joins the running lookup.
    fn prefetch(&self, song: &str, allow_download: bool) {
        let library = self.library.clone();
        let covers = self.covers.clone();
        let streams = self.streams.clone();
        let path = song.to_owned();
        tokio::spawn(async move {
            let file = tokio::task::spawn_blocking(move || {
                library.resolve(Some(&path)).ok().filter(|file| file.is_file())
            })
            .await
            .ok()
            .flatten();
            if let Some(file) = file {
                drop(covers.lookup(file, streams, allow_download));
            }
        });
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
        let mut started = 0;
        for (index, song) in songs.iter().enumerate() {
            if chosen.len() >= COVERS_PER_PLAYLIST {
                break;
            }
            for ahead in songs.iter().take(index + 1 + LOOKUP_AHEAD).skip(started.max(index + 1)) {
                self.prefetch(ahead, allow_download);
            }
            started = started.max(index + 1 + LOOKUP_AHEAD);
            let song = song.clone();
            let Some(bytes) = self.cover(service, &song, allow_download).await else { continue };
            let print = image_print(&bytes);
            if !chosen.iter().any(|(_, seen)| *seen == print) {
                service.keep_image(print, &bytes);
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
        let covers_changed = service.follow_cover_changes(&worker.covers);
        if force || covers_changed || current != stamp {
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
                        service.save_tiles();
                    }
                    // Then every other song, until the playlists change
                    // (the next pass resumes after redoing their tiles). A
                    // tile still short of covers takes different ones from
                    // these, so it looks at every song of its playlist; those
                    // playlists go first, since the listener sees their tiles.
                    let mut checked = 0;
                    let mut fill: Vec<_> = lists
                        .iter()
                        .map(|(id, songs)| {
                            let key = service.key(*id);
                            (id, songs, key, key.is_some_and(|key| service.short(*id, key)))
                        })
                        .collect();
                    fill.sort_by_key(|(_, _, _, short)| !short);
                    'fill: for (id, songs, key, mut short) in fill {
                        let mut started = 0;
                        for (index, song) in songs.iter().enumerate() {
                            if !short && !service.needs_lookup(song, allow_download) {
                                continue;
                            }
                            for ahead in songs.iter().take(index + 1 + LOOKUP_AHEAD).skip(started.max(index + 1)) {
                                if short || service.needs_lookup(ahead, allow_download) {
                                    worker.prefetch(ahead, allow_download);
                                }
                            }
                            started = started.max(index + 1 + LOOKUP_AHEAD);
                            checked += 1;
                            if checked % FILL_CHANGE_CHECK == 0
                                && (worker.stamp().await != stamp
                                    || service.cover_changes_waiting(&worker.covers))
                            {
                                interrupted = true;
                                break 'fill;
                            }
                            let bytes = worker.cover(&service, song, allow_download).await;
                            if let (true, Some(key), Some(bytes)) = (short, key, bytes) {
                                short = service.offer_image(*id, key, song, image_print(&bytes), &bytes);
                            }
                        }
                        if let Some(key) = key {
                            service.filled(*id, key);
                        }
                    }
                    service.save_looked_up();
                    if !interrupted {
                        service.prune_images();
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
    let mut snapshot = state.playlist_covers.snapshot();
    // Covers changed by hand that the worker hasn't taken up yet: every tile
    // is about to be redone, so the page keeps asking until it is.
    if state.playlist_covers.cover_changes_waiting(&state.covers)
        && let Some(items) = snapshot["playlists"].as_array_mut()
    {
        for item in items {
            item["pending"] = serde_json::Value::Bool(true);
        }
    }
    (
        [("cache-control", "no-store")],
        axum::Json(snapshot),
    )
        .into_response()
}

/// `GET /api/playlist-covers/{image}` — one of the tiles' images, by the
/// name `/api/playlists/covers` gave it. The name is the image's fingerprint,
/// so it never changes and the client may keep it.
pub async fn playlist_cover_image(State(state): State<AppState>, UrlPath(image): UrlPath<String>) -> Response {
    let Some(dir) = state.playlist_covers.dir.get().cloned() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if image.len() != 16 || !image.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return StatusCode::NOT_FOUND.into_response();
    }
    let Ok(Ok(bytes)) = tokio::task::spawn_blocking(move || std::fs::read(dir.join(image))).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let content_type = match kog_audio::cover_art::sniff_image_kind(&bytes) {
        Some(kog_audio::cover_art::ImageKind::Jpeg) => "image/jpeg",
        Some(kog_audio::cover_art::ImageKind::Png) => "image/png",
        None => return StatusCode::NOT_FOUND.into_response(),
    };
    (
        [
            ("content-type", content_type),
            ("cache-control", "private, max-age=31536000, immutable"),
        ],
        bytes,
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
    fn a_restarted_server_shows_the_tiles_it_found_before() {
        let dir = std::env::temp_dir().join(format!("kog-tiles-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let covers = PlaylistCovers::default();
        covers.dir.set(dir.clone()).unwrap();
        let songs = vec!["/music/A/1.mp3".to_owned(), "/music/B/1.mp3".to_owned()];
        let lists = [(0, songs.clone()), (5, songs[..1].to_vec())];
        let work = covers.plan(&lists, true);
        let images = [b"\xff\xd8\xff one".as_slice(), b"\xff\xd8\xff two".as_slice()];
        for (id, key, _) in &work {
            let found: Vec<_> = songs
                .iter()
                .zip(images)
                .take(if *id == 0 { 2 } else { 1 })
                .map(|(song, bytes)| {
                    covers.keep_image(image_print(bytes), bytes);
                    (song.clone(), image_print(bytes))
                })
                .collect();
            covers.update(*id, *key, &found, true);
            covers.filled(*id, *key);
        }
        covers.looked_up(songs[0].clone());
        covers.save_looked_up();
        let before = covers.snapshot();
        assert_eq!(before["playlists"][0]["covers"][1]["image"], image_name(image_print(images[1])));

        let restarted = PlaylistCovers::default();
        restarted.restore(&dir);
        assert_eq!(restarted.snapshot(), before);
        assert!(restarted.plan(&lists, true).is_empty());
        assert!(!restarted.needs_lookup(&songs[0], true));
        assert!(restarted.needs_lookup(&songs[1], true));

        // A tile whose image went missing is looked for again.
        std::fs::remove_file(dir.join(image_name(image_print(images[1])))).unwrap();
        let restarted = PlaylistCovers::default();
        restarted.restore(&dir);
        assert_eq!(restarted.plan(&lists, true).len(), 1);
        assert_eq!(restarted.snapshot()["playlists"][1]["covers"][0]["image"], before["playlists"][1]["covers"][0]["image"]);
        let _ = std::fs::remove_dir_all(&dir);
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
