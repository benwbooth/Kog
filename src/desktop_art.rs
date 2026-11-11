//! Album covers on the playlist's song rows and playlist covers in the
//! sidebar, without ever making the UI thread wait. A row asks for its
//! song's cover once it has stayed on screen a moment; the answer comes from
//! memory when the song (or another of its album) has been asked about
//! before, and otherwise the row keeps its format icon while a worker looks
//! on disk (never by playing the song or searching the network) and makes a
//! small rounded image, kept on disk for next time. When answers arrive the
//! revision moves and the rows on screen ask again. The newest asks go
//! first and old ones fall off the queue, so a fast scroll never leaves a
//! backlog of rows long gone.
//!
//! The playlists' icons come from the tiles the server's playlist-cover
//! worker keeps on disk, made again when it saves new ones.
//!
//! The server's worker (the app's own, or the docker one sharing the cover
//! cache) keeps downloading covers in the background, so when the cover
//! cache changes, rows that found none look again.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

/// Workers looking up covers at once.
const WORKERS: usize = 3;
/// Asks waiting at most; older ones are dropped (and asked again if their
/// row is still on screen when the revision next moves).
const QUEUED: usize = 48;
/// A worker with nothing to do for this long ends.
const IDLE: Duration = Duration::from_secs(30);
/// How often the playlist tiles and the cover cache are checked for changes.
const TILES_CHECK: Duration = Duration::from_secs(3);
/// Answers remembered, before starting over.
const REMEMBERED: usize = 16384;

struct Ask {
    song: String,
    file: PathBuf,
    artist: String,
    album: String,
    album_key: String,
}

#[derive(Default)]
struct State {
    /// Song to its icon's URL (empty: no cover).
    songs: HashMap<String, String>,
    /// Album cover key to its icon's URL.
    albums: HashMap<String, String>,
    queue: VecDeque<Ask>,
    queued: HashSet<String>,
    workers: usize,
    /// Playlist id (0: Favorites) to its icon's URL.
    playlists: HashMap<i64, String>,
    /// The tiles and the cover cache are being watched.
    watching: bool,
    changed: bool,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    cover_cache: PathBuf,
}

pub struct DesktopArt {
    shared: Arc<Shared>,
}

/// A file URL for `path`, as QML's Image takes it.
fn file_url(path: &Path) -> String {
    let mut url = String::from("file://");
    for byte in path.to_string_lossy().bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            url.push(byte as char);
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    url
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }
}

fn work(shared: Arc<Shared>) {
    loop {
        let ask = {
            let mut state = shared.lock();
            loop {
                if let Some(ask) = state.queue.pop_front() {
                    break ask;
                }
                let (next, timeout) = shared
                    .wake
                    .wait_timeout(state, IDLE)
                    .unwrap_or_else(|error| error.into_inner());
                state = next;
                if timeout.timed_out() && state.queue.is_empty() {
                    state.workers -= 1;
                    return;
                }
            }
        };
        let url = kog_server::desktop_art::song_icon(&shared.cover_cache, &ask.file, &ask.artist, &ask.album)
            .map(|path| file_url(&path))
            .unwrap_or_default();
        let mut state = shared.lock();
        if state.songs.len() >= REMEMBERED {
            state.songs.clear();
            state.albums.clear();
        }
        // Only a found cover changes what a row shows.
        if !url.is_empty() {
            state.albums.insert(ask.album_key, url.clone());
            state.changed = true;
        }
        state.songs.insert(ask.song.clone(), url);
        state.queued.remove(&ask.song);
    }
}

/// When a file last came or went in the cover cache.
fn cover_cache_stamp(cover_cache: &Path) -> Option<SystemTime> {
    std::fs::metadata(cover_cache).and_then(|metadata| metadata.modified()).ok()
}

fn watch(shared: Arc<Shared>) {
    let mut seen = None;
    let mut covers_seen = cover_cache_stamp(&shared.cover_cache);
    loop {
        let covers = cover_cache_stamp(&shared.cover_cache);
        if covers != covers_seen {
            covers_seen = covers;
            // New covers: the rows without one (on screen) ask again.
            let mut state = shared.lock();
            let before = state.songs.len();
            state.songs.retain(|_, url| !url.is_empty());
            if state.songs.len() != before {
                state.changed = true;
            }
        }
        let stamp: Option<SystemTime> = kog_server::desktop_art::playlist_tiles_stamp(&shared.cover_cache);
        if stamp != seen {
            seen = stamp;
            let icons = kog_server::desktop_art::playlist_icons(&shared.cover_cache)
                .into_iter()
                .map(|(id, path)| (id, file_url(&path)))
                .collect::<HashMap<_, _>>();
            let mut state = shared.lock();
            if state.playlists != icons {
                state.playlists = icons;
                state.changed = true;
            }
        }
        std::thread::sleep(TILES_CHECK);
    }
}

impl DesktopArt {
    pub fn new(cover_cache: PathBuf) -> Self {
        Self {
            shared: Arc::new(Shared {
                state: Mutex::default(),
                wake: Condvar::new(),
                cover_cache,
            }),
        }
    }

    /// The URL of the song's cover icon, or empty while it is looked up
    /// (the revision moves when it is found) and for songs without one.
    pub fn song_icon(&self, song: &str, file: &Path, artist: &str, album: &str) -> String {
        let mut state = self.shared.lock();
        if let Some(url) = state.songs.get(song) {
            return url.clone();
        }
        let album_key = kog_server::desktop_art::album_key(file, album);
        if let Some(url) = state.albums.get(&album_key) {
            return url.clone();
        }
        self.start_watching(&mut state);
        if state.queued.insert(song.to_owned()) {
            state.queue.push_front(Ask {
                song: song.to_owned(),
                file: file.to_path_buf(),
                artist: artist.to_owned(),
                album: album.to_owned(),
                album_key,
            });
            while state.queue.len() > QUEUED {
                if let Some(old) = state.queue.pop_back() {
                    state.queued.remove(&old.song);
                }
            }
            if state.workers < WORKERS {
                state.workers += 1;
                let shared = self.shared.clone();
                if std::thread::Builder::new()
                    .name("kog-row-covers".into())
                    .spawn(move || work(shared))
                    .is_err()
                {
                    state.workers -= 1;
                }
            }
            self.shared.wake.notify_one();
        }
        String::new()
    }

    /// The URL of the playlist's icon (0: Favorites), or empty for one
    /// without covers.
    pub fn playlist_icon(&self, id: i64) -> String {
        let mut state = self.shared.lock();
        self.start_watching(&mut state);
        state.playlists.get(&id).cloned().unwrap_or_default()
    }

    fn start_watching(&self, state: &mut State) {
        if !state.watching {
            state.watching = true;
            let shared = self.shared.clone();
            let _ = std::thread::Builder::new()
                .name("kog-cover-watch".into())
                .spawn(move || watch(shared));
        }
    }

    /// Whether answers arrived since the last call.
    pub fn take_changed(&self) -> bool {
        std::mem::take(&mut self.shared.lock().changed)
    }

    /// A cover was found for the playing song: songs without one look
    /// again.
    pub fn forget_missing(&self) {
        self.shared.lock().songs.retain(|_, url| !url.is_empty());
    }

    /// A cover was rejected or chosen: songs look theirs up again.
    pub fn forget_songs(&self) {
        let mut state = self.shared.lock();
        state.songs.clear();
        state.albums.clear();
        state.changed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_urls_escape_what_a_url_cannot_hold() {
        assert_eq!(
            file_url(Path::new("/music/AC DC/#1 100%.png")),
            "file:///music/AC%20DC/%231%20100%25.png"
        );
    }
}
