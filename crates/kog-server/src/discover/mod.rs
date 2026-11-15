//! Discovery: the listening history, a library index built slowly in the
//! background, sound features for similarity, and the Home tab's mixes.
//!
//! One [`Discover`] runs per process ([`shared`]). Its worker thread runs
//! at idle CPU and disk priority and never blocks a caller: the library may
//! hold millions of songs on a spinning disk, so it is read a folder at a
//! time, the songs just played and the folders around them first, then the
//! rest of the library in a random walk that resumes where it stopped.
//! The desktop app and the server share the index; whichever holds the
//! lease does the crawling and makes the mixes, and the others read them.

pub mod features;
pub mod mixes;
pub mod store;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant, UNIX_EPOCH};

use kog_audio::decoder::{DecoderRegistry, DecoderSettings, PlaybackSource};
use kog_audio::playback_order::session::Listen;
use kog_core::db::{LibraryDb, PlayRecord, StoredEntry};
use serde::Serialize;

use mixes::{Home, Inputs, MixSong};
use store::{Song, Store, now_ms};

/// The lease is renewed this often and lapses after `LEASE_TTL` without.
const LEASE_RENEW: Duration = Duration::from_secs(30);
const LEASE_TTL: Duration = Duration::from_secs(120);
/// How long reading one file's songs may take before it is given up on: a
/// wedged emulator must not stop the crawl.
const UNIT_TIMEOUT: Duration = Duration::from_secs(90);
const ANALYSIS_TIMEOUT: Duration = Duration::from_secs(150);
/// Songs read from one file at most (an archive holds many files).
const MAX_SONGS_PER_UNIT: usize = 3000;
/// Subsongs read from one multi-song file at most.
const MAX_SUBSONGS: u32 = 64;
/// Pause between files and between analyses, so the disk and CPU stay
/// available to whatever is playing.
const FILE_PAUSE: Duration = Duration::from_millis(15);
const ANALYSIS_PAUSE: Duration = Duration::from_millis(120);
/// A finished crawl starts over after this long, to find new files.
const RECRAWL_AFTER_MS: i64 = 7 * mixes::DAY_MS;
/// How often a non-builder looks for mixes saved by the builder.
const HOME_POLL: Duration = Duration::from_secs(20);
const STATE_NAMESPACE: &str = "discover";
const STATE_HOME: &str = "home";

/// How far along the library index is.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Status {
    pub songs: i64,
    pub analysed: i64,
    pub files: i64,
    /// This process holds the lease and is crawling.
    pub crawling: bool,
}

#[derive(Default)]
struct Inner {
    home: Option<Home>,
    home_revision: i64,
    /// Played files to read soon, then the folders around them.
    priority_files: VecDeque<StoredEntry>,
    priority_folders: VecDeque<String>,
    /// Songs to analyse before the rest.
    analyse_first: VecDeque<String>,
    refresh: bool,
    status: Status,
}

pub struct Discover {
    inner: Mutex<Inner>,
    wake: Condvar,
    generation: AtomicU64,
}

static SHARED: OnceLock<Arc<Discover>> = OnceLock::new();

/// The process's discovery service, started on first use.
pub fn shared() -> Arc<Discover> {
    SHARED
        .get_or_init(|| {
            let discover = Arc::new(Discover {
                inner: Mutex::new(Inner::default()),
                wake: Condvar::new(),
                generation: AtomicU64::new(1),
            });
            let worker = Arc::clone(&discover);
            let _ = std::thread::Builder::new()
                .name("kog-discover".into())
                .spawn(move || worker.run());
            discover
        })
        .clone()
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Run the calling thread at idle priority: lowest CPU niceness and, on
/// Linux, the idle disk class, so it only gets the disk when nothing else
/// wants it.
pub fn lower_priority() {
    #[cfg(unix)]
    unsafe {
        // On Linux niceness is per thread; elsewhere this lowers the process,
        // which only runs background work here.
        #[cfg(target_os = "linux")]
        {
            libc::setpriority(libc::PRIO_PROCESS, 0, 19);
            const IOPRIO_WHO_PROCESS: libc::c_long = 1;
            const IOPRIO_CLASS_IDLE: libc::c_long = 3;
            libc::syscall(libc::SYS_ioprio_set, IOPRIO_WHO_PROCESS, 0 as libc::c_long, IOPRIO_CLASS_IDLE << 13);
        }
    }
}

/// Run `work` on its own idle-priority thread, giving up after `timeout`
/// (the thread is left to finish or hang on its own).
fn with_timeout<T: Send + 'static>(
    timeout: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (send, receive) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("kog-discover-job".into())
        .spawn(move || {
            lower_priority();
            let _ = send.send(work());
        })
        .ok()?;
    receive.recv_timeout(timeout).ok()
}

/// The library key of a stored entry without its subsong.
fn file_key(entry: &StoredEntry) -> String {
    match entry.kind.as_str() {
        "archive" => format!("{}::{}", entry.path, entry.entry),
        _ => entry.path.clone(),
    }
}

/// The file on disk an entry is read from: the music file, or the archive
/// holding it.
fn unit_of(entry: &StoredEntry) -> Option<PathBuf> {
    match entry.kind.as_str() {
        "local" | "archive" => Some(PathBuf::from(&entry.path)),
        _ => None,
    }
}

fn fs_node(path: &Path) -> String {
    format!("fs\0{}", path.display())
}

/// A file's size and modification time, which change when it does.
fn stamp_of(path: &Path) -> Option<(String, i64)> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |age| age.as_millis() as i64);
    Some((format!("{}:{modified}", metadata.len()), modified))
}

impl Discover {
    /// Changes whenever the mixes do, so a frontend knows to fetch them.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    pub fn home(&self) -> Option<Home> {
        lock(&self.inner).home.clone()
    }

    pub fn status(&self) -> Status {
        lock(&self.inner).status.clone()
    }

    /// What the Home tab shows, as JSON.
    pub fn home_json(&self) -> serde_json::Value {
        let inner = lock(&self.inner);
        serde_json::json!({
            "generation": self.generation(),
            "home": inner.home,
            "status": inner.status,
        })
    }

    /// One mix's songs as queue entries, with its title.
    pub fn mix_entries(&self, id: &str) -> Option<(String, Vec<serde_json::Value>)> {
        let inner = lock(&self.inner);
        let mix = inner.home.as_ref()?.mix(id)?;
        Some((mix.title.clone(), mix.songs.iter().map(MixSong::entry_json).collect()))
    }

    /// Make the mixes again now instead of waiting for tomorrow.
    pub fn refresh(&self) {
        lock(&self.inner).refresh = true;
        self.wake.notify_all();
    }

    /// Record songs that stopped playing in the listening history, and
    /// read them (and the songs around them) into the index soon.
    pub fn record_listens(&self, listens: &[Listen]) {
        let plays: Vec<PlayRecord> = listens.iter().filter_map(play_from_listen).collect();
        if plays.is_empty() {
            return;
        }
        if let Ok(db) = LibraryDb::open() {
            for play in &plays {
                if let Err(error) = db.record_play(play) {
                    eprintln!("kog: {error}");
                }
            }
        }
        self.prioritize(plays.iter().map(PlayRecord::stored_entry));
    }

    /// Record one play reported by a client (the web player).
    pub fn record_play(&self, play: PlayRecord) -> Result<(), String> {
        LibraryDb::open()?.record_play(&play)?;
        self.prioritize(std::iter::once(play.stored_entry()));
        Ok(())
    }

    fn prioritize(&self, entries: impl Iterator<Item = StoredEntry>) {
        let mut inner = lock(&self.inner);
        for entry in entries {
            if unit_of(&entry).is_none() {
                continue;
            }
            if let Ok(locator) = crate::media_filter::locator_for(&entry) {
                if !inner.analyse_first.contains(&locator) {
                    inner.analyse_first.push_front(locator);
                }
            }
            if !inner.priority_files.contains(&entry) {
                inner.priority_files.push_back(entry);
            }
        }
        drop(inner);
        self.wake.notify_all();
    }

    fn sleep(&self, duration: Duration) {
        let inner = lock(&self.inner);
        let _ = self.wake.wait_timeout(inner, duration);
    }

    fn run(self: Arc<Self>) {
        lower_priority();
        let owner = format!("{}-{:x}", std::process::id(), kog_audio::radio::random_seed());
        let mut store = loop {
            match Store::open() {
                Ok(store) => break store,
                Err(error) => {
                    eprintln!("kog: the library index is unavailable: {error}");
                    std::thread::sleep(Duration::from_secs(300));
                }
            }
        };
        let settings = kog_audio::settings::AppSettings::load();
        let decoder_settings = settings.decoder_settings();
        let decoders = Arc::new(DecoderRegistry::new(decoder_settings.clone()));
        let root = settings.music_directory.clone().filter(|root| root.is_dir());
        let read_cue = settings.read_cue_sheets_in_folders;
        let mut crawler = Crawler {
            decoders,
            decoder_settings,
            root,
            read_cue,
            steps: 0,
        };
        let mut leased = false;
        let mut last_lease: Option<Instant> = None;
        let mut last_poll: Option<Instant> = None;
        let mut last_status: Option<Instant> = None;
        self.seed_from_history();
        loop {
            if last_lease.is_none_or(|at| at.elapsed() >= LEASE_RENEW) {
                leased = store.take_lease(&owner, LEASE_TTL);
                last_lease = Some(Instant::now());
            }
            if last_status.is_none_or(|at| at.elapsed() >= Duration::from_secs(30)) {
                let (songs, analysed, files) = store.counts();
                lock(&self.inner).status = Status { songs, analysed, files, crawling: leased };
                last_status = Some(Instant::now());
            }
            if last_poll.is_none_or(|at| at.elapsed() >= HOME_POLL) || lock(&self.inner).refresh {
                self.maintain_home(&store, leased);
                last_poll = Some(Instant::now());
            }
            if !leased {
                self.sleep(HOME_POLL);
                continue;
            }
            match crawler.step(&self, &mut store) {
                Step::Indexed => std::thread::sleep(FILE_PAUSE),
                Step::Analysed => std::thread::sleep(ANALYSIS_PAUSE),
                Step::Idle => self.sleep(Duration::from_secs(15)),
            }
        }
    }

    /// Read the files of recent plays first, so a new index starts where
    /// the listener is.
    fn seed_from_history(&self) {
        let Ok(db) = LibraryDb::open() else { return };
        let since = now_ms() - 90 * mixes::DAY_MS;
        let Ok(plays) = db.plays_since(since) else { return };
        let mut entries: Vec<StoredEntry> = Vec::new();
        for play in plays.iter().rev() {
            let entry = play.stored_entry();
            if !entries.contains(&entry) {
                entries.push(entry);
            }
            if entries.len() >= 300 {
                break;
            }
        }
        self.prioritize(entries.into_iter());
    }

    /// Keep the mixes current: the lease holder makes them when due and
    /// saves them for everyone; the others load what it saved.
    fn maintain_home(&self, store: &Store, builder: bool) {
        let Ok(db) = LibraryDb::open() else { return };
        let saved = db.load_state(STATE_NAMESPACE, STATE_HOME).ok().flatten();
        {
            let mut inner = lock(&self.inner);
            if let Some(saved) = &saved
                && saved.revision != inner.home_revision
            {
                if let Ok(home) = serde_json::from_str::<Home>(&saved.value) {
                    inner.home = Some(home);
                    inner.home_revision = saved.revision;
                    self.generation.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        if !builder {
            return;
        }
        let now = now_ms();
        let (day, week) = mixes::local_day_and_week(now);
        let (refresh, previous) = {
            let mut inner = lock(&self.inner);
            (std::mem::take(&mut inner.refresh), inner.home.clone())
        };
        let (indexed, ..) = store.counts();
        let due = match &previous {
            None => indexed > 0 || db.plays_stamp() > 0,
            Some(home) => {
                refresh
                    || home.day != day
                    || (home.thin
                        && now - home.made_at > 10 * 60 * 1000
                        && indexed >= (home.indexed * 2).max(home.indexed + 500))
            }
        };
        if !due {
            return;
        }
        let plays = db.plays_since(0).unwrap_or_default();
        let stars = db.starred_entries().unwrap_or_default();
        let home = mixes::make_home(&Inputs {
            store,
            plays: &plays,
            stars: &stars,
            now,
            day,
            week,
            previous: previous.as_ref(),
            remake_weekly: refresh,
        });
        let Ok(text) = serde_json::to_string(&home) else { return };
        if let Err(error) = db.put_state(STATE_NAMESPACE, STATE_HOME, &text) {
            eprintln!("kog: saving the Home mixes: {error}");
        }
        let revision = db
            .load_state(STATE_NAMESPACE, STATE_HOME)
            .ok()
            .flatten()
            .map_or(0, |saved| saved.revision);
        let mut inner = lock(&self.inner);
        inner.home = Some(home);
        inner.home_revision = revision;
        self.generation.fetch_add(1, Ordering::Relaxed);
    }
}

fn play_from_listen(listen: &Listen) -> Option<PlayRecord> {
    let text = |key: &str| listen.entry.get(key).and_then(|v| v.as_str()).unwrap_or("").to_owned();
    let entry = StoredEntry {
        kind: Some(text("kind")).filter(|kind| !kind.is_empty()).unwrap_or_else(|| "local".into()),
        path: text("path"),
        entry: text("entry"),
        fragment: Some(text("fragment")).filter(|fragment| !fragment.is_empty()),
    };
    let locator = crate::media_filter::locator_for(&entry).ok()?;
    let metadata = &listen.metadata;
    let duration = if listen.duration > 0.0 { listen.duration } else { metadata.duration.unwrap_or(0.0) };
    Some(PlayRecord {
        locator,
        kind: entry.kind,
        path: entry.path,
        entry: entry.entry,
        fragment: entry.fragment,
        played_at: now_ms(),
        listened_ms: (listen.listened * 1000.0) as i64,
        length_ms: (duration * 1000.0) as i64,
        finished: listen.finished,
        title: metadata.title.clone(),
        artist: metadata.artist.clone(),
        album: metadata.album.clone(),
        album_artist: metadata.album_artist.clone(),
        composer: metadata.composer.clone(),
        genre: metadata.genre.clone(),
        codec: metadata.codec.clone(),
        year: metadata.year.map(|year| year as i64),
    })
}

enum Step {
    Indexed,
    Analysed,
    Idle,
}

struct Crawler {
    decoders: Arc<DecoderRegistry>,
    decoder_settings: DecoderSettings,
    root: Option<PathBuf>,
    read_cue: bool,
    steps: u64,
}

impl Crawler {
    /// Do one piece of work, most wanted first: files just played, songs to
    /// analyse first, the folders around played files, then the rest of the
    /// library, with an analysis every few folders.
    fn step(&mut self, discover: &Discover, store: &mut Store) -> Step {
        self.steps += 1;
        let next_file = lock(&discover.inner).priority_files.pop_front();
        if let Some(entry) = next_file {
            if let Some(unit) = unit_of(&entry) {
                let folder = unit.parent().map(fs_node);
                self.index_unit(store, &unit);
                if let Some(folder) = folder
                    && entry.kind == "local"
                    && !store.folder_done(&folder)
                {
                    let mut inner = lock(&discover.inner);
                    if !inner.priority_folders.contains(&folder) {
                        inner.priority_folders.push_back(folder);
                    }
                }
            }
            return Step::Indexed;
        }
        let next_song = lock(&discover.inner).analyse_first.pop_front();
        if let Some(locator) = next_song {
            if let Some(song) = store.unanalysed(&[locator], 1).into_iter().next() {
                self.analyse(store, &song);
                return Step::Analysed;
            }
            return Step::Indexed;
        }
        let next_folder = lock(&discover.inner).priority_folders.pop_front();
        if let Some(node) = next_folder {
            let folder_key = node.strip_prefix("fs\0").unwrap_or(&node).to_owned();
            self.index_folder(discover, store, &node);
            let mut inner = lock(&discover.inner);
            for song in store.unanalysed_in_folder(&folder_key, 40) {
                if !inner.analyse_first.contains(&song.locator) {
                    inner.analyse_first.push_back(song.locator);
                }
            }
            return Step::Indexed;
        }
        // The rest of the library: an analysis every fourth step once
        // there is something to analyse, folders otherwise.
        if self.steps % 4 == 0 {
            let max = store.max_id();
            if max > 0 {
                let start = 1 + (kog_audio::radio::random_seed().wrapping_mul(0x9E37_79B9) % max as u64) as i64;
                if let Some(song) = store.unanalysed_from(start, 1).into_iter().next() {
                    self.analyse(store, &song);
                    return Step::Analysed;
                }
            }
        }
        let Some(root) = self.root.clone() else { return Step::Idle };
        if store.max_folder_id() == 0 {
            let _ = store.add_folders(&[fs_node(&root)]);
        }
        let max = store.max_folder_id().max(1);
        let start = 1 + (kog_audio::radio::random_seed().wrapping_mul(0x2545_F491) % max as u64) as i64;
        match store.next_folder(start) {
            Some((_, node)) => {
                self.index_folder(discover, store, &node);
                Step::Indexed
            }
            None => {
                let finished: i64 = store
                    .meta("crawl_finished_at")
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(0);
                if finished == 0 {
                    let _ = store.set_meta("crawl_finished_at", &now_ms().to_string());
                } else if now_ms() - finished > RECRAWL_AFTER_MS {
                    let _ = store.set_meta("crawl_finished_at", "0");
                    store.restart_crawl();
                    return Step::Indexed;
                }
                Step::Idle
            }
        }
    }

    /// Read one folder: its music files and archives into the index, its
    /// subfolders onto the crawl. Files just played are read in between.
    fn index_folder(&mut self, discover: &Discover, store: &mut Store, node: &str) {
        let Some(path) = node.strip_prefix("fs\0").map(PathBuf::from) else {
            store.finish_folder(node);
            return;
        };
        let listing = with_timeout(UNIT_TIMEOUT, {
            let path = path.clone();
            move || std::fs::read_dir(&path).map(|entries| entries.filter_map(Result::ok).collect::<Vec<_>>())
        });
        let Some(Ok(entries)) = listing else {
            store.finish_folder(node);
            return;
        };
        let mut folders = Vec::new();
        let mut files = Vec::new();
        for entry in entries {
            let name = entry.file_name();
            if crate::media_filter::is_hidden_name(&name) {
                continue;
            }
            let Ok(kind) = entry.file_type() else { continue };
            let child = entry.path();
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                folders.push(fs_node(&child));
            } else if kind.is_file() && self.wanted(&child) {
                files.push(child);
            }
        }
        let _ = store.add_folders(&folders);
        files.sort();
        for file in files {
            // Songs played meanwhile come first.
            loop {
                let played = lock(&discover.inner).priority_files.pop_front();
                let Some(entry) = played else { break };
                if let Some(unit) = unit_of(&entry) {
                    self.index_unit(store, &unit);
                }
            }
            if self.index_unit(store, &file) {
                std::thread::sleep(FILE_PAUSE);
            }
        }
        store.finish_folder(node);
    }

    /// Whether a file in a folder is music to index: something a decoder
    /// plays or an archive, but not a playlist (its songs are indexed where
    /// they are) and not a cue sheet unless the settings read them.
    fn wanted(&self, path: &Path) -> bool {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if matches!(extension.as_str(), "m3u" | "m3u8" | "pls") {
            return false;
        }
        if extension == "cue" {
            return self.read_cue;
        }
        if kog_core::media_path::is_metadata(path) {
            return false;
        }
        kog_audio::archive::is_path(path) || self.decoders.accepts_path(path)
    }

    /// Read one file's songs into the index unless it is unchanged since it
    /// was last read. Returns whether it was read.
    fn index_unit(&mut self, store: &mut Store, unit: &Path) -> bool {
        let Some((stamp, added_at)) = stamp_of(unit) else { return false };
        let unit_key = unit.display().to_string();
        if store.file_stamp(&unit_key).as_deref() == Some(stamp.as_str()) {
            return false;
        }
        let decoders = Arc::clone(&self.decoders);
        let path = unit.to_path_buf();
        let songs = with_timeout(UNIT_TIMEOUT, move || read_unit(&decoders, &path, added_at))
            .unwrap_or_else(|| {
                eprintln!("kog: reading {} for the library index timed out", unit.display());
                Vec::new()
            });
        if let Err(error) = store.put_file(&unit_key, &stamp, &songs) {
            eprintln!("kog: indexing {}: {error}", unit.display());
        }
        true
    }

    fn analyse(&mut self, store: &mut Store, song: &Song) {
        let decoders = Arc::clone(&self.decoders);
        let settings = self.decoder_settings.clone();
        let entry = song.stored_entry();
        let vector = with_timeout(ANALYSIS_TIMEOUT, move || {
            let playlist_entry = kog_audio::playlist::PlaylistEntry::from_locator(
                &entry.kind,
                &entry.path,
                &entry.entry,
                entry.fragment.clone(),
            )?;
            let source = decoders
                .expand_entry(&playlist_entry)?
                .sources
                .into_iter()
                .next()
                .ok_or_else(|| "nothing to play".to_owned())?;
            // `decoders` stays alive here, keeping an extracted archive
            // member on disk while it is measured.
            let result = features::analyze(source, &settings);
            drop(decoders);
            result
        });
        let features = vector.and_then(Result::ok);
        let _ = store.set_features(song.id, features.as_deref());
    }
}

/// Every song in one file (or archive), with its tags.
fn read_unit(decoders: &DecoderRegistry, unit: &Path, added_at: i64) -> Vec<Song> {
    let Ok(expansion) = decoders.expand_detailed(unit.to_path_buf()) else {
        return Vec::new();
    };
    let folder = match unit.parent() {
        Some(parent) if !kog_audio::archive::is_path(unit) => parent.display().to_string(),
        _ => unit.display().to_string(),
    };
    let mut songs = Vec::new();
    let mut tags_for: Option<(PathBuf, Option<String>, Option<String>)> = None;
    for source in expansion.sources.into_iter().take(MAX_SONGS_PER_UNIT) {
        if source.subsong.is_some_and(|subsong| subsong >= MAX_SUBSONGS) {
            continue;
        }
        let Ok(mut properties) = decoders.probe(&source) else { continue };
        if !source.is_remote() {
            // Album artist and composer come from the file's tags; read them
            // once per file, not once per subsong.
            let same = tags_for.as_ref().is_some_and(|(path, ..)| *path == source.path);
            if !same {
                let mut tags = kog_audio::decoder::StreamProperties::default();
                tags.fill_local_tags(&source.path);
                tags_for = Some((source.path.clone(), tags.album_artist, tags.composer));
            }
            if let Some((_, album_artist, composer)) = &tags_for {
                properties.album_artist = properties.album_artist.or_else(|| album_artist.clone());
                properties.composer = properties.composer.or_else(|| composer.clone());
            }
        }
        let Some(entry) = crate::radio::entry_from_source(decoders, &source, &properties, Path::new(""))
        else {
            continue;
        };
        let stored = StoredEntry {
            kind: entry.kind.clone(),
            path: entry.path.clone(),
            entry: entry.entry.clone(),
            fragment: entry.fragment.clone(),
        };
        let Ok(locator) = crate::media_filter::locator_for(&stored) else { continue };
        songs.push(Song {
            locator,
            file_key: file_key(&stored),
            folder: folder.clone(),
            title: properties
                .title
                .clone()
                .filter(|title| !title.trim().is_empty())
                .unwrap_or_else(|| fallback_title(&source)),
            artist: properties.artist.clone().unwrap_or_default(),
            album: properties.album.clone().unwrap_or_default(),
            album_artist: properties.album_artist.clone().unwrap_or_default(),
            composer: properties.composer.clone().unwrap_or_default(),
            genre: properties.genre.clone().unwrap_or_default(),
            year: properties.year.map(i64::from),
            system: system_of(decoders, &source, &properties),
            backend: decoders.selected_backend_id(&source).unwrap_or_default().to_owned(),
            duration_ms: properties.duration.map_or(0, |duration| duration.as_millis() as i64),
            added_at,
            kind: stored.kind,
            path: stored.path,
            entry: stored.entry,
            fragment: stored.fragment,
            ..Song::default()
        });
    }
    songs
}

/// The format or sound chip a song is grouped by: MIDI whichever synth
/// renders it, else what its decoder reports, else its file's extension.
fn system_of(
    decoders: &DecoderRegistry,
    source: &PlaybackSource,
    properties: &kog_audio::decoder::StreamProperties,
) -> String {
    if decoders.selected_backend_id(source).is_some_and(|id| id.starts_with("midi-")) {
        return "MIDI".to_owned();
    }
    let reported = mixes::system_name(properties.codec.as_deref().unwrap_or_default());
    if !reported.is_empty() {
        return reported;
    }
    let name = source
        .archive_origin
        .as_ref()
        .map(|origin| PathBuf::from(&origin.entry_name))
        .unwrap_or_else(|| source.path.clone());
    name.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_uppercase)
        .unwrap_or_default()
}

/// A song's name when its file has no title tag: the file's name, with the
/// song's number for the later songs of a multi-song file.
fn fallback_title(source: &PlaybackSource) -> String {
    let name = source
        .archive_origin
        .as_ref()
        .map(|origin| PathBuf::from(&origin.entry_name))
        .unwrap_or_else(|| source.path.clone());
    let mut title = name
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Untitled")
        .to_owned();
    if let Some(subsong) = source.subsong.filter(|subsong| *subsong > 0) {
        title.push_str(&format!(" [{}]", subsong + 1));
    }
    title
}

// ---- HTTP ----

use axum::extract::Query;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};

/// `GET /api/discover` — the Home tab's mixes and the index's progress.
async fn home_route() -> Response {
    let discover = shared();
    axum::Json(discover.home_json()).into_response()
}

/// `POST /api/discover/refresh` — make the mixes again now.
async fn refresh_route() -> Response {
    shared().refresh();
    axum::Json(serde_json::json!({ "ok": true })).into_response()
}

#[derive(serde::Deserialize)]
struct MixQuery {
    id: String,
}

/// `GET /api/discover/mix?id=` — one mix's songs, as queue entries.
async fn mix_route(Query(query): Query<MixQuery>) -> Response {
    match shared().mix_entries(&query.id) {
        Some((title, entries)) => {
            axum::Json(serde_json::json!({ "title": title, "entries": entries })).into_response()
        }
        None => crate::routes::bad_request("That mix has been replaced by newer ones"),
    }
}

/// One song's playback as a browser session reports it.
#[derive(serde::Deserialize)]
struct ListenRequest {
    entry: serde_json::Value,
    #[serde(default)]
    metadata: kog_audio::playback_order::sort::SortRow,
    #[serde(default)]
    listened: f64,
    #[serde(default)]
    duration: f64,
    #[serde(default)]
    finished: bool,
}

/// `POST /api/history` — record songs a client played.
async fn history_route(axum::Json(listens): axum::Json<Vec<ListenRequest>>) -> Response {
    let listens: Vec<Listen> = listens
        .into_iter()
        .map(|request| Listen {
            entry: request.entry,
            metadata: request.metadata,
            listened: request.listened.max(0.0),
            duration: request.duration.max(0.0),
            finished: request.finished,
        })
        .collect();
    let result = tokio::task::spawn_blocking(move || shared().record_listens(&listens)).await;
    match result {
        Ok(()) => axum::Json(serde_json::json!({ "ok": true })).into_response(),
        Err(error) => crate::routes::bad_request(&format!("recording plays failed: {error}")),
    }
}

pub fn router() -> axum::Router<crate::routes::AppState> {
    axum::Router::new()
        .route("/api/discover", get(home_route))
        .route("/api/discover/refresh", post(refresh_route))
        .route("/api/discover/mix", get(mix_route))
        .route("/api/history", post(history_route))
}

/// Look up the covers the Home cards show (the first songs of each mix from
/// different albums) whenever the mixes change, so the cover cache has them
/// before anyone looks. Runs in a serving process, beside the playlist
/// covers' worker, with the same cover lookup and pace.
pub(crate) fn start_cover_prefetch(state: &crate::routes::AppState) {
    let covers = state.covers.clone();
    let streams = state.streams.clone();
    tokio::spawn(async move {
        let discover = shared();
        let mut seen = 0u64;
        loop {
            tokio::time::sleep(Duration::from_secs(20)).await;
            let generation = discover.generation();
            if generation == seen {
                continue;
            }
            seen = generation;
            let Some(home) = discover.home() else { continue };
            let allow_download = kog_audio::settings::AppSettings::load().download_cover_art;
            let mut files = Vec::new();
            for mix in home.sections.iter().flat_map(|section| &section.mixes) {
                let mut albums = std::collections::HashSet::new();
                for song in &mix.songs {
                    if albums.len() >= 4 {
                        break;
                    }
                    if song.kind != "local" || !albums.insert(song.album.to_lowercase()) {
                        continue;
                    }
                    let file = PathBuf::from(&song.path);
                    if !files.contains(&file) {
                        files.push(file);
                    }
                }
            }
            for file in files {
                if discover.generation() != seen {
                    break;
                }
                let exists = {
                    let file = file.clone();
                    tokio::task::spawn_blocking(move || file.is_file()).await.unwrap_or(false)
                };
                if exists {
                    let _ = crate::covers::wait(covers.lookup(file, streams.clone(), allow_download)).await;
                    tokio::time::sleep(Duration::from_millis(150)).await;
                }
            }
        }
    });
}
