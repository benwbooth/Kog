//! The library index behind the Home mixes: one row per song the background
//! crawler has read, with its tags and, once analysed, its sound features.
//!
//! It lives in its own database, `discover.db`, beside `kog.db`: it is a
//! cache that can be thrown away and grows with the library (millions of
//! rows for a large one), so it stays out of the store that holds playlists
//! and preferences. The desktop app and the server share it; a lease row
//! makes sure only one of them crawls at a time.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::{Connection, OptionalExtension, params};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    node TEXT NOT NULL UNIQUE,
    done INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS folders_pending ON folders(done, id);
CREATE TABLE IF NOT EXISTS files (
    file_key TEXT PRIMARY KEY,
    stamp TEXT NOT NULL,
    songs INTEGER NOT NULL DEFAULT 0,
    dead INTEGER NOT NULL DEFAULT 0,
    indexed_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS songs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    locator TEXT NOT NULL UNIQUE,
    file_key TEXT NOT NULL,
    unit TEXT NOT NULL DEFAULT '',
    folder TEXT NOT NULL DEFAULT '',
    kind TEXT NOT NULL,
    path TEXT NOT NULL,
    entry TEXT NOT NULL DEFAULT '',
    fragment TEXT,
    title TEXT NOT NULL DEFAULT '',
    artist TEXT NOT NULL DEFAULT '',
    album TEXT NOT NULL DEFAULT '',
    album_artist TEXT NOT NULL DEFAULT '',
    composer TEXT NOT NULL DEFAULT '',
    genre TEXT NOT NULL DEFAULT '',
    year INTEGER,
    system TEXT NOT NULL DEFAULT '',
    backend TEXT NOT NULL DEFAULT '',
    duration_ms INTEGER NOT NULL DEFAULT 0,
    added_at INTEGER NOT NULL DEFAULT 0,
    indexed_at INTEGER NOT NULL DEFAULT 0,
    features BLOB,
    features_at INTEGER NOT NULL DEFAULT 0,
    embedding BLOB,
    embedding_at INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS songs_by_file ON songs(file_key);
CREATE INDEX IF NOT EXISTS songs_by_unit ON songs(unit);
CREATE INDEX IF NOT EXISTS songs_by_folder ON songs(folder);
CREATE INDEX IF NOT EXISTS songs_by_album ON songs(album COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS songs_by_artist ON songs(artist COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS songs_by_composer ON songs(composer COLLATE NOCASE);
CREATE INDEX IF NOT EXISTS songs_by_system ON songs(system);
CREATE INDEX IF NOT EXISTS songs_by_added ON songs(added_at);
CREATE INDEX IF NOT EXISTS songs_by_features ON songs(features_at);
";

/// Which way of reading songs filled the index; bump it when what is read
/// from a file changes, so every file is read again.
const INDEX_VERSION: &str = "2";

/// One indexed song. `locator` is the library's key for it (see
/// `media_filter::locator_for`); `file_key` is the same without the
/// subsong, shared by every song of a multi-song file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Song {
    pub id: i64,
    pub locator: String,
    pub file_key: String,
    /// The folder or archive the file sits in, as an opaque key: only used
    /// to find the songs next to a played one, never to name or group them.
    pub folder: String,
    pub kind: String,
    pub path: String,
    pub entry: String,
    pub fragment: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub composer: String,
    pub genre: String,
    pub year: Option<i64>,
    /// The format or sound chip, from the decoder (for example "NSF" or
    /// "VGM (YM2612, SN76489)").
    pub system: String,
    pub backend: String,
    pub duration_ms: i64,
    /// When the file appeared (its modification time), ms since the epoch.
    pub added_at: i64,
    pub features: Option<Vec<f32>>,
    pub embedding: Option<Vec<f32>>,
}

impl Song {
    pub fn stored_entry(&self) -> kog_core::db::StoredEntry {
        kog_core::db::StoredEntry {
            kind: self.kind.clone(),
            path: self.path.clone(),
            entry: self.entry.clone(),
            fragment: self.fragment.clone(),
        }
    }

    /// The artist a song is credited to for mixes: the composer when the
    /// tags name one (chiptune rips usually credit the composer as artist
    /// too), else the artist, else the album artist.
    pub fn credit(&self) -> &str {
        [&self.composer, &self.artist, &self.album_artist]
            .into_iter()
            .map(|value| value.trim())
            .find(|value| !value.is_empty())
            .unwrap_or("")
    }
}

pub struct Store {
    conn: Connection,
}

const SONG_COLUMNS: &str = "id, locator, file_key, folder, kind, path, entry, fragment, title, artist,
    album, album_artist, composer, genre, year, system, backend, duration_ms, added_at, features,
    embedding";

fn blob_to_vector(blob: Option<Vec<u8>>) -> Option<Vec<f32>> {
    let blob = blob?;
    (blob.len() % 4 == 0 && !blob.is_empty()).then(|| {
        blob.chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect()
    })
}

fn vector_to_blob(vector: &[f32]) -> Vec<u8> {
    vector.iter().flat_map(|value| value.to_le_bytes()).collect()
}

fn song_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Song> {
    Ok(Song {
        id: row.get(0)?,
        locator: row.get(1)?,
        file_key: row.get(2)?,
        folder: row.get(3)?,
        kind: row.get(4)?,
        path: row.get(5)?,
        entry: row.get(6)?,
        fragment: row.get(7)?,
        title: row.get(8)?,
        artist: row.get(9)?,
        album: row.get(10)?,
        album_artist: row.get(11)?,
        composer: row.get(12)?,
        genre: row.get(13)?,
        year: row.get(14)?,
        system: row.get(15)?,
        backend: row.get(16)?,
        duration_ms: row.get(17)?,
        added_at: row.get(18)?,
        features: blob_to_vector(row.get(19)?),
        embedding: blob_to_vector(row.get(20)?),
    })
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|age| age.as_millis() as i64)
        .unwrap_or(0)
}

/// Where the index lives: beside `kog.db`, in Kog's data folder.
pub fn default_path() -> Option<PathBuf> {
    let db = kog_core::db::LibraryDb::open().ok()?.path()?;
    Some(db.with_file_name("discover.db"))
}

impl Store {
    pub fn open() -> Result<Self, String> {
        let path = default_path().ok_or("Kog's data folder is not available")?;
        Self::open_at(&path)
    }

    pub fn open_at(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("creating {}: {e}", parent.display()))?;
        }
        let conn = Connection::open(path).map_err(|e| format!("opening the library index: {e}"))?;
        Self::configure(conn)
    }

    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        Self::configure(conn)
    }

    fn configure(conn: Connection) -> Result<Self, String> {
        conn.busy_timeout(Duration::from_secs(10)).map_err(|e| e.to_string())?;
        let _ = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get::<_, String>(0));
        conn.execute_batch("PRAGMA synchronous = NORMAL;")
            .map_err(|e| e.to_string())?;
        conn.execute_batch(SCHEMA)
            .map_err(|e| format!("creating the library index: {e}"))?;
        let store = Self { conn };
        // Songs read by an older way of reading are read again (their sound
        // features are kept): forget the files' stamps and revisit folders.
        if store.meta("index_version").as_deref() != Some(INDEX_VERSION) {
            let _ = store.conn.execute_batch("DELETE FROM files; UPDATE folders SET done = 0;");
            let _ = store.set_meta("index_version", INDEX_VERSION);
        }
        Ok(store)
    }

    pub fn meta(&self, key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| row.get(0))
            .optional()
            .ok()
            .flatten()
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Take or renew the crawling lease for `owner`, held until `ttl` passes
    /// without a renewal. Another live owner keeps it.
    pub fn take_lease(&self, owner: &str, ttl: Duration) -> bool {
        self.take_named_lease("lease", owner, ttl)
    }

    /// Like [`Self::take_lease`] for another job (`name` is its key).
    pub fn take_named_lease(&self, name: &str, owner: &str, ttl: Duration) -> bool {
        let now = now_ms();
        let Ok(transaction) = rusqlite::Transaction::new_unchecked(
            &self.conn,
            rusqlite::TransactionBehavior::Immediate,
        ) else {
            return false;
        };
        let current = self.meta(name);
        let free = match current.as_deref().and_then(|value| value.split_once('|')) {
            Some((holder, expires)) => {
                holder == owner || expires.parse::<i64>().map_or(true, |expires| expires < now)
            }
            None => true,
        };
        if free {
            let value = format!("{owner}|{}", now + ttl.as_millis() as i64);
            if self.set_meta(name, &value).is_err() {
                return false;
            }
        }
        transaction.commit().is_ok() && free
    }

    pub fn release_lease(&self, owner: &str) {
        self.release_named_lease("lease", owner);
    }

    /// Give up a job's lease if `owner` holds it, so another Kog can take
    /// the job at once instead of waiting for it to lapse.
    pub fn release_named_lease(&self, name: &str, owner: &str) {
        let _ = self.conn.execute(
            "DELETE FROM meta WHERE key = ?1 AND value LIKE ?2",
            params![name, format!("{owner}|%")],
        );
    }

    /// The stamp a file was indexed with, when it was.
    pub fn file_stamp(&self, file_key: &str) -> Option<String> {
        self.conn
            .query_row("SELECT stamp FROM files WHERE file_key = ?1", [file_key], |row| row.get(0))
            .optional()
            .ok()
            .flatten()
    }

    /// Replace the songs read from one file on disk (`unit`: a music file,
    /// or an archive with every song inside it) with `songs`, none for a
    /// file that plays nothing, keeping the features of songs still there.
    pub fn put_file(&mut self, unit: &str, stamp: &str, songs: &[Song]) -> Result<(), String> {
        let file_key = unit;
        let transaction = self.conn.transaction().map_err(|e| e.to_string())?;
        {
            let keep: Vec<&str> = songs.iter().map(|song| song.locator.as_str()).collect();
            let mut existing = transaction
                .prepare("SELECT locator FROM songs WHERE unit = ?1")
                .map_err(|e| e.to_string())?;
            let gone: Vec<String> = existing
                .query_map([file_key], |row| row.get::<_, String>(0))
                .map_err(|e| e.to_string())?
                .filter_map(Result::ok)
                .filter(|locator| !keep.contains(&locator.as_str()))
                .collect();
            for locator in gone {
                transaction
                    .execute("DELETE FROM songs WHERE locator = ?1", [locator])
                    .map_err(|e| e.to_string())?;
            }
            let now = now_ms();
            for song in songs {
                transaction
                    .execute(
                        "INSERT INTO songs (locator, file_key, folder, kind, path, entry, fragment,
                            title, artist, album, album_artist, composer, genre, year, system,
                            backend, duration_ms, added_at, indexed_at, unit)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                            ?16, ?17, ?18, ?19, ?20)
                         ON CONFLICT(locator) DO UPDATE SET
                            file_key = excluded.file_key, folder = excluded.folder,
                            unit = excluded.unit,
                            kind = excluded.kind, path = excluded.path, entry = excluded.entry,
                            fragment = excluded.fragment, title = excluded.title,
                            artist = excluded.artist, album = excluded.album,
                            album_artist = excluded.album_artist, composer = excluded.composer,
                            genre = excluded.genre, year = excluded.year, system = excluded.system,
                            backend = excluded.backend, duration_ms = excluded.duration_ms,
                            added_at = excluded.added_at, indexed_at = excluded.indexed_at",
                        params![
                            song.locator,
                            if song.file_key.is_empty() { file_key } else { song.file_key.as_str() },
                            song.folder,
                            song.kind,
                            song.path,
                            song.entry,
                            song.fragment,
                            song.title,
                            song.artist,
                            song.album,
                            song.album_artist,
                            song.composer,
                            song.genre,
                            song.year,
                            song.system,
                            song.backend,
                            song.duration_ms,
                            song.added_at,
                            now,
                            unit,
                        ],
                    )
                    .map_err(|e| e.to_string())?;
            }
            transaction
                .execute(
                    "INSERT INTO files (file_key, stamp, songs, dead, indexed_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(file_key) DO UPDATE SET stamp = excluded.stamp,
                        songs = excluded.songs, dead = excluded.dead,
                        indexed_at = excluded.indexed_at",
                    params![file_key, stamp, songs.len() as i64, songs.is_empty(), now],
                )
                .map_err(|e| e.to_string())?;
        }
        transaction.commit().map_err(|e| e.to_string())
    }

    // ---- the crawl frontier ----

    /// Add folders to visit (already known ones keep their state).
    pub fn add_folders(&mut self, nodes: &[String]) -> Result<(), String> {
        let transaction = self.conn.transaction().map_err(|e| e.to_string())?;
        for node in nodes {
            transaction
                .execute("INSERT OR IGNORE INTO folders (node) VALUES (?1)", [node])
                .map_err(|e| e.to_string())?;
        }
        transaction.commit().map_err(|e| e.to_string())
    }

    /// A folder still to visit, from around a row id (random for a random
    /// walk through the library).
    pub fn next_folder(&self, start: i64) -> Option<(i64, String)> {
        let query = |sql: &str| {
            self.conn
                .query_row(sql, [start], |row| Ok((row.get(0)?, row.get(1)?)))
                .optional()
                .ok()
                .flatten()
        };
        query("SELECT id, node FROM folders WHERE done = 0 AND id >= ?1 ORDER BY id LIMIT 1")
            .or_else(|| query("SELECT id, node FROM folders WHERE done = 0 AND id < ?1 ORDER BY id LIMIT 1"))
    }

    pub fn max_folder_id(&self) -> i64 {
        self.conn
            .query_row("SELECT COALESCE(MAX(id), 0) FROM folders", [], |row| row.get(0))
            .unwrap_or(0)
    }

    pub fn finish_folder(&self, node: &str) {
        let _ = self.conn.execute(
            "INSERT INTO folders (node, done) VALUES (?1, 1)
             ON CONFLICT(node) DO UPDATE SET done = 1",
            [node],
        );
    }

    pub fn folder_done(&self, node: &str) -> bool {
        self.conn
            .query_row("SELECT done FROM folders WHERE node = ?1", [node], |row| row.get::<_, bool>(0))
            .optional()
            .ok()
            .flatten()
            .unwrap_or(false)
    }

    /// Visit every folder again, to find files added since.
    pub fn restart_crawl(&self) {
        let _ = self.conn.execute("UPDATE folders SET done = 0", []);
    }

    pub fn set_features(&self, id: i64, features: Option<&[f32]>) -> Result<(), String> {
        // A song that could not be analysed gets an empty value, so it is
        // not tried again until it is indexed anew.
        let blob = features.map(vector_to_blob).unwrap_or_default();
        self.conn
            .execute(
                "UPDATE songs SET features = ?1, features_at = ?2 WHERE id = ?3",
                params![blob, now_ms(), id],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn set_embedding(&self, id: i64, embedding: Option<&[f32]>) -> Result<(), String> {
        let blob = embedding.map(vector_to_blob).unwrap_or_default();
        self.conn
            .execute(
                "UPDATE songs SET embedding = ?1, embedding_at = ?2 WHERE id = ?3",
                params![blob, now_ms(), id],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn query(&self, sql: &str, params: impl rusqlite::Params) -> Vec<Song> {
        let Ok(mut statement) = self.conn.prepare_cached(sql) else {
            return Vec::new();
        };
        statement
            .query_map(params, song_from_row)
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }

    pub fn song(&self, locator: &str) -> Option<Song> {
        self.query(&format!("SELECT {SONG_COLUMNS} FROM songs WHERE locator = ?1"), [locator])
            .into_iter()
            .next()
    }

    pub fn songs_of_file(&self, file_key: &str) -> Vec<Song> {
        self.query(
            &format!("SELECT {SONG_COLUMNS} FROM songs WHERE file_key = ?1 ORDER BY id"),
            [file_key],
        )
    }

    pub fn songs_in_folder(&self, folder: &str, limit: usize) -> Vec<Song> {
        self.query(
            &format!("SELECT {SONG_COLUMNS} FROM songs WHERE folder = ?1 ORDER BY id LIMIT ?2"),
            params![folder, limit as i64],
        )
    }

    pub fn songs_by_album(&self, album: &str, limit: usize) -> Vec<Song> {
        self.query(
            &format!(
                "SELECT {SONG_COLUMNS} FROM songs WHERE album = ?1 COLLATE NOCASE ORDER BY id LIMIT ?2"
            ),
            params![album, limit as i64],
        )
    }

    /// Songs credited to `name` as composer, artist or album artist.
    pub fn songs_by_credit(&self, name: &str, limit: usize) -> Vec<Song> {
        self.query(
            &format!(
                "SELECT {SONG_COLUMNS} FROM songs WHERE composer = ?1 COLLATE NOCASE
                 UNION SELECT {SONG_COLUMNS} FROM songs WHERE artist = ?1 COLLATE NOCASE
                 UNION SELECT {SONG_COLUMNS} FROM songs WHERE album_artist = ?1 COLLATE NOCASE
                 LIMIT ?2"
            ),
            params![name, limit as i64],
        )
    }

    pub fn songs_of_system(&self, system: &str, start: i64, limit: usize) -> Vec<Song> {
        self.query(
            &format!(
                "SELECT {SONG_COLUMNS} FROM songs WHERE system = ?1 AND id >= ?2 ORDER BY id LIMIT ?3"
            ),
            params![system, start, limit as i64],
        )
    }

    /// The newest files in the library, by when they appeared.
    pub fn newest(&self, limit: usize) -> Vec<Song> {
        self.query(
            &format!("SELECT {SONG_COLUMNS} FROM songs ORDER BY added_at DESC LIMIT ?1"),
            [limit as i64],
        )
    }

    /// A run of songs starting at a row id, wrapping to the start: a cheap
    /// random sample of a large index when `start` is random.
    pub fn sample(&self, start: i64, limit: usize) -> Vec<Song> {
        let mut songs = self.query(
            &format!("SELECT {SONG_COLUMNS} FROM songs WHERE id >= ?1 ORDER BY id LIMIT ?2"),
            params![start, limit as i64],
        );
        if songs.len() < limit {
            songs.extend(self.query(
                &format!("SELECT {SONG_COLUMNS} FROM songs WHERE id < ?1 ORDER BY id LIMIT ?2"),
                params![start, (limit - songs.len()) as i64],
            ));
        }
        songs
    }

    /// Like [`Self::sample`], only songs with sound features.
    pub fn sample_with_features(&self, start: i64, limit: usize) -> Vec<Song> {
        let mut songs = self.query(
            &format!(
                "SELECT {SONG_COLUMNS} FROM songs WHERE id >= ?1 AND (length(features) > 0 OR length(embedding) > 0)
                 ORDER BY id LIMIT ?2"
            ),
            params![start, limit as i64],
        );
        if songs.len() < limit {
            songs.extend(self.query(
                &format!(
                    "SELECT {SONG_COLUMNS} FROM songs WHERE id < ?1 AND (length(features) > 0 OR length(embedding) > 0)
                     ORDER BY id LIMIT ?2"
                ),
                params![start, (limit - songs.len()) as i64],
            ));
        }
        songs
    }

    /// Songs not analysed yet, among `locators` first (in that order).
    pub fn unanalysed(&self, locators: &[String], limit: usize) -> Vec<Song> {
        let mut found = Vec::new();
        for locator in locators {
            if found.len() >= limit {
                break;
            }
            found.extend(self.query(
                &format!("SELECT {SONG_COLUMNS} FROM songs WHERE locator = ?1 AND features_at = 0"),
                [locator],
            ));
        }
        found
    }

    /// Songs in the given folders not analysed yet.
    pub fn unanalysed_in_folder(&self, folder: &str, limit: usize) -> Vec<Song> {
        self.query(
            &format!(
                "SELECT {SONG_COLUMNS} FROM songs WHERE folder = ?1 AND features_at = 0 LIMIT ?2"
            ),
            params![folder, limit as i64],
        )
    }

    /// Any songs not analysed yet, from a random point.
    pub fn unanalysed_from(&self, start: i64, limit: usize) -> Vec<Song> {
        self.query(
            &format!(
                "SELECT {SONG_COLUMNS} FROM songs WHERE features_at = 0 AND id >= ?1 ORDER BY id LIMIT ?2"
            ),
            params![start, limit as i64],
        )
    }

    /// Songs without an embedding, among `locators` first.
    pub fn unembedded(&self, locators: &[String], limit: usize) -> Vec<Song> {
        let mut found = Vec::new();
        for locator in locators {
            if found.len() >= limit {
                break;
            }
            found.extend(self.query(
                &format!("SELECT {SONG_COLUMNS} FROM songs WHERE locator = ?1 AND embedding_at = 0"),
                [locator],
            ));
        }
        found
    }

    /// Any songs without an embedding, from a random point.
    pub fn unembedded_from(&self, start: i64, limit: usize) -> Vec<Song> {
        let mut songs = self.query(
            &format!(
                "SELECT {SONG_COLUMNS} FROM songs WHERE embedding_at = 0 AND id >= ?1 ORDER BY id LIMIT ?2"
            ),
            params![start, limit as i64],
        );
        if songs.is_empty() {
            songs = self.query(
                &format!("SELECT {SONG_COLUMNS} FROM songs WHERE embedding_at = 0 ORDER BY id LIMIT ?1"),
                [limit as i64],
            );
        }
        songs
    }

    /// Forget every embedding, when another model makes them.
    pub fn clear_embeddings(&self) {
        let _ = self.conn.execute("UPDATE songs SET embedding = NULL, embedding_at = 0", []);
    }

    pub fn pending_folders(&self) -> i64 {
        self.conn
            .query_row("SELECT COUNT(*) FROM folders WHERE done = 0", [], |row| row.get(0))
            .unwrap_or(0)
    }

    pub fn embedded_count(&self) -> i64 {
        self.conn
            .query_row("SELECT COUNT(*) FROM songs WHERE length(embedding) > 0", [], |row| row.get(0))
            .unwrap_or(0)
    }

    pub fn max_id(&self) -> i64 {
        self.conn
            .query_row("SELECT COALESCE(MAX(id), 0) FROM songs", [], |row| row.get(0))
            .unwrap_or(0)
    }

    pub fn counts(&self) -> (i64, i64, i64) {
        let songs = self
            .conn
            .query_row("SELECT COUNT(*) FROM songs", [], |row| row.get(0))
            .unwrap_or(0);
        let analysed = self
            .conn
            .query_row("SELECT COUNT(*) FROM songs WHERE features_at > 0", [], |row| row.get(0))
            .unwrap_or(0);
        let files = self
            .conn
            .query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))
            .unwrap_or(0);
        (songs, analysed, files)
    }

    /// The systems in the index with how many songs each has.
    pub fn systems(&self) -> Vec<(String, i64)> {
        let Ok(mut statement) = self.conn.prepare_cached(
            "SELECT system, COUNT(*) FROM songs WHERE system != '' GROUP BY system ORDER BY 2 DESC",
        ) else {
            return Vec::new();
        };
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(locator: &str, file: &str, album: &str) -> Song {
        Song {
            locator: locator.into(),
            file_key: file.into(),
            folder: "/music".into(),
            kind: "local".into(),
            path: file.into(),
            title: locator.into(),
            album: album.into(),
            composer: "Composer".into(),
            system: "NSF".into(),
            ..Song::default()
        }
    }

    #[test]
    fn files_replace_their_songs_and_keep_features() {
        let mut store = Store::open_in_memory().unwrap();
        store
            .put_file("/a.nsf", "1", &[song("/a.nsf#0", "/a.nsf", "A"), song("/a.nsf#1", "/a.nsf", "A")])
            .unwrap();
        let first = store.song("/a.nsf#0").unwrap();
        store.set_features(first.id, Some(&[1.0, 2.0])).unwrap();
        store.put_file("/a.nsf", "2", &[song("/a.nsf#0", "/a.nsf", "A")]).unwrap();
        assert!(store.song("/a.nsf#1").is_none());
        assert_eq!(store.song("/a.nsf#0").unwrap().features, Some(vec![1.0, 2.0]));
        assert_eq!(store.file_stamp("/a.nsf").as_deref(), Some("2"));
        assert_eq!(store.songs_by_credit("composer", 10).len(), 1);
        assert_eq!(store.songs_by_album("a", 10).len(), 1);
    }

    #[test]
    fn one_owner_holds_the_lease() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.take_lease("one", Duration::from_secs(60)));
        assert!(!store.take_lease("two", Duration::from_secs(60)));
        assert!(store.take_lease("one", Duration::from_secs(60)));
        store.release_lease("one");
        assert!(store.take_lease("two", Duration::from_secs(60)));
    }
}
