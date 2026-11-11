//! Playlist files in the music folder's Playlists folders, kept imported
//! without the UI thread ever looking at them. The music folder is usually
//! on a spinning disk, and the check (a folder listing, two lookups and a
//! database query per playlist file) ran every two seconds; now a thread of
//! its own does it, with its own connection to the library, and the window
//! only collects what it found.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kog_audio::playlist::PlaylistFolderSync;

/// How often the folders are checked.
const INTERVAL: Duration = Duration::from_secs(2);

#[derive(Default)]
struct Shared {
    /// The music folder to watch (it can change while the app runs).
    music: Option<PathBuf>,
    /// What the checks found since the window last asked.
    found: Vec<PlaylistFolderSync>,
}

pub struct PlaylistFolders {
    shared: Arc<Mutex<Shared>>,
    started: bool,
}

impl PlaylistFolders {
    pub fn new() -> Self {
        Self {
            shared: Arc::default(),
            started: false,
        }
    }

    /// Watch `music`'s playlist folders, importing into the library at
    /// `database`, and return what changed since the last call. Never waits
    /// on the disk.
    pub fn poll(&mut self, music: PathBuf, database: Option<PathBuf>) -> Vec<PlaylistFolderSync> {
        let found = {
            let mut shared = self.shared.lock().unwrap_or_else(|error| error.into_inner());
            shared.music = Some(music);
            std::mem::take(&mut shared.found)
        };
        if !self.started
            && let Some(database) = database
        {
            self.started = true;
            let shared = self.shared.clone();
            let _ = std::thread::Builder::new()
                .name("kog-playlist-folders".into())
                .spawn(move || watch(shared, database));
        }
        found
    }
}

fn watch(shared: Arc<Mutex<Shared>>, database: PathBuf) {
    let db = match kog_core::db::LibraryDb::open_at(&database) {
        Ok(db) => db,
        Err(error) => {
            eprintln!("kog: playlist folders not watched: {error}");
            return;
        }
    };
    loop {
        let music = shared.lock().unwrap_or_else(|error| error.into_inner()).music.clone();
        if let Some(music) = music {
            let folders = kog_audio::playlist::watched_playlist_folders(&music);
            let sync = kog_audio::playlist::sync_playlist_folders(&db, &folders);
            if !sync.imported.is_empty() || !sync.updated.is_empty() || !sync.failed.is_empty() {
                shared.lock().unwrap_or_else(|error| error.into_inner()).found.push(sync);
            }
        }
        std::thread::sleep(INTERVAL);
    }
}
