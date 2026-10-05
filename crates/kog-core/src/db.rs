//! SQLite application store: library, preferences, and session checkpoints.
//!
//! One database, `kog.db`, holds what used to need ad-hoc files: the star
//! set (favorites source of truth) and named playlists with full-fidelity
//! entries (local files, archive members, remote URLs, subsong fragments).
//! Connections use WAL and a bounded busy timeout. Read/modify/write operations
//! acquire SQLite's writer lock before reading, including across processes.
//!
//! Locator keys reuse the radio identity scheme (plain path,
//! `outer :: member`, remote URL) so stars, the dead set, and dedup logic
//! all name a track the same way.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS stars (
    locator TEXT PRIMARY KEY,
    kind TEXT NOT NULL DEFAULT '',
    path TEXT NOT NULL DEFAULT '',
    entry TEXT NOT NULL DEFAULT '',
    fragment TEXT,
    starred_at INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    position INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS playlist_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    kind TEXT NOT NULL,
    path TEXT NOT NULL,
    entry TEXT NOT NULL DEFAULT '',
    fragment TEXT
);
CREATE INDEX IF NOT EXISTS playlist_entries_by_playlist
    ON playlist_entries(playlist_id, position);
CREATE TABLE IF NOT EXISTS blacklist (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    path TEXT NOT NULL DEFAULT '',
    entry TEXT NOT NULL DEFAULT '',
    UNIQUE(kind, path, entry)
);
CREATE TABLE IF NOT EXISTS app_state (
    namespace TEXT NOT NULL,
    key TEXT NOT NULL,
    value TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision > 0),
    updated_at INTEGER NOT NULL,
    PRIMARY KEY(namespace, key)
);
";

const SCHEMA_VERSION: &str = "2";

/// Entry kinds stored in `kind` columns. Archive members keep outer path +
/// member name; remote tracks keep the URL in `path`.
pub const KIND_LOCAL: &str = "local";
pub const KIND_ARCHIVE: &str = "archive";
pub const KIND_REMOTE: &str = "remote";

/// Blacklist entry kinds: whole songs or whole folders.
pub const BLACKLIST_SONG: &str = "song";
pub const BLACKLIST_FOLDER: &str = "folder";

/// One blacklist row: songs match radio picks exactly (archive members
/// keep outer path + member name), folders match by path prefix.
pub struct BlacklistEntry {
    pub id: i64,
    pub kind: String,
    pub path: String,
    pub entry: String,
}

pub struct StoredPlaylist {
    pub id: i64,
    pub name: String,
    pub position: i64,
    pub entry_count: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredEntry {
    pub kind: String,
    pub path: String,
    pub entry: String,
    pub fragment: Option<String>,
}

pub struct LibraryDb {
    conn: Connection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredState {
    pub value: String,
    pub revision: i64,
}

#[derive(Debug)]
pub enum StateWriteError {
    Conflict,
    Database(String),
}

impl std::fmt::Display for StateWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conflict => f.write_str("This session changed in another instance. Your current session has not overwritten its saved state."),
            Self::Database(error) => f.write_str(error),
        }
    }
}

impl std::error::Error for StateWriteError {}

fn database_path() -> PathBuf {
    directories::ProjectDirs::from("org", "Kog", "Kog")
        .map(|directories| directories.data_dir().join("kog.db"))
        .unwrap_or_else(|| std::env::temp_dir().join("kog.db"))
}

#[cfg(any(target_os = "android", target_os = "ios"))]
static DEVICE_DATABASE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Mobile has one application-private store. Configure it before any shared
/// service reads preferences, so platform directory fallbacks cannot create
/// a second store for the native library.
pub fn configure_device_database(path: &Path) -> Result<(), String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let current = DEVICE_DATABASE.get_or_init(|| path.to_owned());
        if current != path {
            return Err("The device database is already configured at another path".into());
        }
    }
    let _ = path;
    Ok(())
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|age| age.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

impl LibraryDb {
    pub fn path(&self) -> Option<PathBuf> {
        self.conn
            .path()
            .filter(|path| !path.is_empty())
            .map(PathBuf::from)
    }
    pub fn open() -> Result<Self, String> {
        #[cfg(any(target_os = "android", target_os = "ios"))]
        return Self::open_at(
            DEVICE_DATABASE
                .get()
                .ok_or("The device database has not been configured")?,
        );
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        Self::open_at(&database_path())
    }

    /// Memory-only database for tests and for degraded startup when the
    /// data directory is unavailable (stars/playlists won't persist).
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory()
            .map_err(|error| format!("opening library database: {error}"))?;
        Self::configure(&conn, false)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_at(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("creating {}: {error}", parent.display()))?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            // Closing any raw descriptor for an existing SQLite file releases
            // this process's POSIX locks, including those of other connections.
            // Only create a new inode ourselves; chmod existing files by path.
            // Serialize creation so another opener cannot establish SQLite
            // locks before the new file's initial descriptor has been closed.
            static CREATE_PRIVATE_DATABASE: std::sync::Mutex<()> = std::sync::Mutex::new(());
            let _creation = CREATE_PRIVATE_DATABASE
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)
            {
                Ok(file) => drop(file),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(format!("creating private database: {error}")),
            }
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("protecting database: {e}"))?;
            for suffix in ["-wal", "-shm"] {
                let mut sibling = path.as_os_str().to_owned();
                sibling.push(suffix);
                match std::fs::set_permissions(&sibling, std::fs::Permissions::from_mode(0o600)) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(format!("protecting database journal: {error}")),
                }
            }
        }
        let conn = Connection::open(path)
            .map_err(|error| format!("opening {}: {error}", path.display()))?;
        Self::configure(&conn, true)?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn configure(conn: &Connection, on_disk: bool) -> Result<(), String> {
        conn.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(|e| format!("configuring database timeout: {e}"))?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|e| format!("configuring database: {e}"))?;
        if on_disk {
            let mode: String = conn
                .pragma_query_value(None, "journal_mode", |row| row.get(0))
                .map_err(|e| format!("reading database journal mode: {e}"))?;
            if !mode.eq_ignore_ascii_case("wal") {
                let mode: String = conn
                    .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
                    .map_err(|e| format!("enabling concurrent database readers: {e}"))?;
                if !mode.eq_ignore_ascii_case("wal") {
                    return Err("The database filesystem does not support WAL journaling".into());
                }
            }
        }
        Ok(())
    }

    /// Compose library mutations into one transaction without nested BEGINs.
    /// The connection is exclusively borrowed by its owner/Mutex; nested calls
    /// here can only belong to the same synchronous operation.
    pub fn write_transaction<T>(
        &self,
        operation: impl FnOnce(&Self) -> Result<T, String>,
    ) -> Result<T, String> {
        if !self.conn.is_autocommit() {
            return operation(self);
        }
        let transaction = Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)
            .map_err(|e| format!("starting database transaction: {e}"))?;
        let result = operation(self)?;
        transaction
            .commit()
            .map_err(|e| format!("committing database transaction: {e}"))?;
        Ok(result)
    }

    fn migrate(&self) -> Result<(), String> {
        let version = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional();
        if let Ok(Some(version)) = version {
            if version == SCHEMA_VERSION {
                return Ok(());
            }
            if version.parse::<u32>().unwrap_or(u32::MAX) > SCHEMA_VERSION.parse::<u32>().unwrap() {
                return Err("This database was created by a newer Kog version".into());
            }
        }
        self.write_transaction(|db| db.migrate_in_transaction())
    }

    fn migrate_in_transaction(&self) -> Result<(), String> {
        self.conn
            .execute_batch(SCHEMA)
            .map_err(|error| format!("creating library schema: {error}"))?;
        let version: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| format!("reading library schema version: {error}"))?;
        if version.as_ref().is_some_and(|v| {
            v.parse::<u32>().unwrap_or(u32::MAX) > SCHEMA_VERSION.parse::<u32>().unwrap()
        }) {
            return Err("This database was created by a newer version of Kog".into());
        }
        if version.as_deref() != Some(SCHEMA_VERSION) {
            self.conn
                .execute(
                    "INSERT INTO meta (key, value) VALUES ('schema_version', ?1)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    [SCHEMA_VERSION],
                )
                .map_err(|error| format!("writing library schema version: {error}"))?;
        }
        Ok(())
    }

    pub fn load_state(&self, namespace: &str, key: &str) -> Result<Option<StoredState>, String> {
        self.conn
            .query_row(
                "SELECT value, revision FROM app_state WHERE namespace = ?1 AND key = ?2",
                rusqlite::params![namespace, key],
                |row| {
                    Ok(StoredState {
                        value: row.get(0)?,
                        revision: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(|e| format!("reading saved state: {e}"))
    }

    pub fn list_state(&self, namespace: &str) -> Result<Vec<(String, StoredState)>, String> {
        let mut query = self
            .conn
            .prepare("SELECT key, value, revision FROM app_state WHERE namespace = ?1 ORDER BY key")
            .map_err(|e| format!("reading preferences: {e}"))?;
        query
            .query_map([namespace], |row| {
                Ok((
                    row.get(0)?,
                    StoredState {
                        value: row.get(1)?,
                        revision: row.get(2)?,
                    },
                ))
            })
            .map_err(|e| e.to_string())?
            .collect::<Result<_, _>>()
            .map_err(|e| e.to_string())
    }

    /// Each preference is an independent key, so updating one never rewrites
    /// an older in-memory copy of unrelated preferences.
    pub fn put_state(&self, namespace: &str, key: &str, value: &str) -> Result<(), String> {
        self.conn.execute(
            "INSERT INTO app_state(namespace, key, value, revision, updated_at) VALUES(?1, ?2, ?3, 1, ?4)
             ON CONFLICT(namespace, key) DO UPDATE SET value = excluded.value,
                 revision = app_state.revision + 1, updated_at = excluded.updated_at",
            rusqlite::params![namespace, key, value, now_millis()],
        ).map_err(|e| format!("saving preference: {e}"))?;
        Ok(())
    }

    /// Legacy data is imported once. A concurrent writer's SQLite value wins.
    pub fn import_state(
        &self,
        namespace: &str,
        key: &str,
        value: &str,
    ) -> Result<StoredState, String> {
        self.write_transaction(|db| {
            db.conn.execute(
                "INSERT INTO app_state(namespace, key, value, revision, updated_at) VALUES(?1, ?2, ?3, 1, ?4)
                 ON CONFLICT(namespace, key) DO NOTHING",
                rusqlite::params![namespace, key, value, now_millis()],
            ).map_err(|e| format!("migrating saved state: {e}"))?;
            db.load_state(namespace, key)?.ok_or_else(|| "Migrated state is missing".into())
        })
    }

    /// Compare-and-swap a checkpoint. Revision zero means it has never been
    /// saved; every subsequent save must present the last observed revision.
    pub fn save_state_checked(
        &self,
        namespace: &str,
        key: &str,
        value: &str,
        expected_revision: i64,
    ) -> Result<i64, StateWriteError> {
        if !(0..i64::MAX).contains(&expected_revision) {
            return Err(StateWriteError::Database(
                "Invalid saved-state revision".into(),
            ));
        }
        let changed = if expected_revision == 0 {
            self.conn.execute(
                "INSERT INTO app_state(namespace, key, value, revision, updated_at) VALUES(?1, ?2, ?3, 1, ?4)
                 ON CONFLICT(namespace, key) DO NOTHING",
                rusqlite::params![namespace, key, value, now_millis()],
            )
        } else {
            self.conn.execute(
                "UPDATE app_state SET value = ?3, revision = revision + 1, updated_at = ?4
                 WHERE namespace = ?1 AND key = ?2 AND revision = ?5",
                rusqlite::params![namespace, key, value, now_millis(), expected_revision],
            )
        }.map_err(|e| StateWriteError::Database(format!("saving session: {e}")))?;
        if changed == 0 {
            // An HTTP reply can be lost after commit. Retrying the identical
            // value acknowledges that save without making a second write.
            if let Some(saved) = self
                .load_state(namespace, key)
                .map_err(StateWriteError::Database)?
            {
                if saved.value == value {
                    return Ok(saved.revision);
                }
            }
            return Err(StateWriteError::Conflict);
        }
        Ok(expected_revision + 1)
    }

    // ---- stars ----

    pub fn set_star(
        &self,
        locator: &str,
        kind: &str,
        path: &str,
        entry: &str,
        fragment: Option<&str>,
        starred: bool,
    ) -> Result<(), String> {
        if starred {
            self.conn
                .execute(
                    "INSERT INTO stars (locator, kind, path, entry, fragment, starred_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                     ON CONFLICT(locator) DO UPDATE SET
                       kind = excluded.kind, path = excluded.path,
                       entry = excluded.entry, fragment = excluded.fragment,
                       starred_at = excluded.starred_at",
                    rusqlite::params![locator, kind, path, entry, fragment, now_millis()],
                )
                .map_err(|error| format!("starring {locator:?}: {error}"))?;
        } else {
            self.conn
                .execute("DELETE FROM stars WHERE locator = ?1", [locator])
                .map_err(|error| format!("unstarring {locator:?}: {error}"))?;
        }
        Ok(())
    }

    pub fn is_starred(&self, locator: &str) -> bool {
        self.conn
            .query_row("SELECT 1 FROM stars WHERE locator = ?1", [locator], |_| {
                Ok(())
            })
            .optional()
            .map(|hit| hit.is_some())
            .unwrap_or(false)
    }

    pub fn starred_locators(&self) -> Result<Vec<String>, String> {
        let mut statement = self
            .conn
            .prepare("SELECT locator FROM stars")
            .map_err(|error| format!("reading stars: {error}"))?;
        statement
            .query_map([], |row| row.get(0))
            .map_err(|error| format!("reading stars: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("reading stars: {error}"))
    }

    pub fn starred_entries(&self) -> Result<Vec<StoredEntry>, String> {
        let mut statement = self
            .conn
            .prepare("SELECT kind, path, entry, fragment FROM stars ORDER BY starred_at, locator")
            .map_err(|error| format!("reading stars: {error}"))?;
        statement
            .query_map([], |row| {
                Ok(StoredEntry {
                    kind: row.get(0)?,
                    path: row.get(1)?,
                    entry: row.get(2)?,
                    fragment: row.get(3)?,
                })
            })
            .map_err(|error| format!("reading stars: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("reading stars: {error}"))
    }

    // ---- playlists ----

    fn require_playlist(&self, id: i64) -> Result<(), String> {
        let exists: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM playlists WHERE id = ?1)",
                [id],
                |row| row.get(0),
            )
            .map_err(|e| format!("reading playlist: {e}"))?;
        if exists {
            Ok(())
        } else {
            Err("Playlist no longer exists".into())
        }
    }

    pub fn create_playlist_with_entries(
        &self,
        name: &str,
        entries: &[StoredEntry],
    ) -> Result<i64, String> {
        self.write_transaction(|db| {
            let id = db.create_playlist(name)?;
            db.append_entries(id, entries)?;
            Ok(id)
        })
    }

    pub fn list_playlists(&self) -> Result<Vec<StoredPlaylist>, String> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT p.id, p.name, p.position, COUNT(e.id)
                 FROM playlists p LEFT JOIN playlist_entries e ON e.playlist_id = p.id
                 GROUP BY p.id ORDER BY p.position, p.id",
            )
            .map_err(|error| format!("listing playlists: {error}"))?;
        statement
            .query_map([], |row| {
                Ok(StoredPlaylist {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    position: row.get(2)?,
                    entry_count: row.get(3)?,
                })
            })
            .map_err(|error| format!("listing playlists: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("listing playlists: {error}"))
    }

    pub fn create_playlist(&self, name: &str) -> Result<i64, String> {
        self.write_transaction(|db| db.create_playlist_in_transaction(name))
    }

    fn create_playlist_in_transaction(&self, name: &str) -> Result<i64, String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Playlist name cannot be empty".to_owned());
        }
        let position: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) + 1 FROM playlists",
                [],
                |row| row.get(0),
            )
            .map_err(|error| format!("creating playlist {name:?}: {error}"))?;
        self.conn
            .execute(
                "INSERT INTO playlists (name, position) VALUES (?1, ?2)",
                rusqlite::params![name, position],
            )
            .map_err(|error| {
                if error.to_string().contains("UNIQUE") {
                    format!("A playlist named {name:?} already exists")
                } else {
                    format!("creating playlist {name:?}: {error}")
                }
            })?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<(), String> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Playlist name cannot be empty".to_owned());
        }
        let changed = self
            .conn
            .execute(
                "UPDATE playlists SET name = ?1 WHERE id = ?2",
                rusqlite::params![name, id],
            )
            .map_err(|error| {
                if error.to_string().contains("UNIQUE") {
                    format!("A playlist named {name:?} already exists")
                } else {
                    format!("renaming playlist: {error}")
                }
            })?;
        if changed == 0 {
            return Err("Playlist no longer exists".to_owned());
        }
        Ok(())
    }

    pub fn duplicate_playlist(&self, id: i64, name: &str) -> Result<i64, String> {
        self.write_transaction(|db| db.duplicate_playlist_in_transaction(id, name))
    }

    fn duplicate_playlist_in_transaction(&self, id: i64, name: &str) -> Result<i64, String> {
        self.require_playlist(id)?;
        let new_id = self.create_playlist(name)?;
        let entries = self.playlist_entries(id)?;
        self.append_entries(new_id, &entries)?;
        Ok(new_id)
    }

    /// Drop every entry of a playlist and store `entries` in their place:
    /// the "overwrite" path when a save reuses an existing name.
    pub fn replace_entries(&self, playlist_id: i64, entries: &[StoredEntry]) -> Result<(), String> {
        self.replace_entries_checked(playlist_id, entries, None)
    }

    /// Save a draft atomically and, when supplied, check its original contents.
    /// A concurrent editor must never silently overwrite another client's save.
    pub fn replace_entries_checked(
        &self,
        playlist_id: i64,
        entries: &[StoredEntry],
        expected: Option<&[StoredEntry]>,
    ) -> Result<(), String> {
        self.write_transaction(|db| {
            db.replace_entries_in_transaction(playlist_id, entries, expected)
        })
    }

    fn replace_entries_in_transaction(
        &self,
        playlist_id: i64,
        entries: &[StoredEntry],
        expected: Option<&[StoredEntry]>,
    ) -> Result<(), String> {
        self.require_playlist(playlist_id)?;
        if let Some(expected) = expected {
            let equal = |a: &StoredEntry, b: &StoredEntry| {
                a.kind == b.kind
                    && a.path == b.path
                    && a.entry == b.entry
                    && a.fragment.as_deref().unwrap_or_default()
                        == b.fragment.as_deref().unwrap_or_default()
            };
            let current = self.playlist_entries(playlist_id)?;
            if current.len() != expected.len()
                || !current.iter().zip(expected).all(|(a, b)| equal(a, b))
            {
                return Err("This playlist changed elsewhere. Your draft is preserved; reopen the playlist to load the latest version.".into());
            }
        }
        self.conn
            .execute(
                "DELETE FROM playlist_entries WHERE playlist_id = ?1",
                [playlist_id],
            )
            .map_err(|e| format!("saving playlist: {e}"))?;
        self.append_entries(playlist_id, entries)
    }

    pub fn delete_playlist(&self, id: i64) -> Result<(), String> {
        let changed = self
            .conn
            .execute("DELETE FROM playlists WHERE id = ?1", [id])
            .map_err(|error| format!("deleting playlist: {error}"))?;
        if changed == 0 {
            return Err("Playlist no longer exists".to_owned());
        }
        Ok(())
    }

    /// Move playlist `id` to zero-based `to_position`, shifting neighbours.
    pub fn move_playlist(&self, id: i64, to_position: usize) -> Result<(), String> {
        self.write_transaction(|db| db.move_playlist_in_transaction(id, to_position))
    }

    fn move_playlist_in_transaction(&self, id: i64, to_position: usize) -> Result<(), String> {
        let mut playlists = self.list_playlists()?;
        let Some(from) = playlists.iter().position(|playlist| playlist.id == id) else {
            return Err("Playlist no longer exists".to_owned());
        };
        let moved = playlists.remove(from);
        let to_position = to_position.min(playlists.len());
        playlists.insert(to_position, moved);
        for (position, playlist) in playlists.iter().enumerate() {
            self.conn
                .execute(
                    "UPDATE playlists SET position = ?1 WHERE id = ?2",
                    rusqlite::params![position as i64, playlist.id],
                )
                .map_err(|error| format!("reordering playlists: {error}"))?;
        }
        Ok(())
    }

    pub fn append_entries(&self, playlist_id: i64, entries: &[StoredEntry]) -> Result<(), String> {
        self.write_transaction(|db| db.append_entries_in_transaction(playlist_id, entries))
    }

    fn append_entries_in_transaction(
        &self,
        playlist_id: i64,
        entries: &[StoredEntry],
    ) -> Result<(), String> {
        self.require_playlist(playlist_id)?;
        let base: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(position), -1) FROM playlist_entries WHERE playlist_id = ?1",
                [playlist_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("adding to playlist: {error}"))?;
        for (offset, entry) in entries.iter().enumerate() {
            self.conn
                .execute(
                    "INSERT INTO playlist_entries
                     (playlist_id, position, kind, path, entry, fragment)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![
                        playlist_id,
                        base + 1 + offset as i64,
                        entry.kind,
                        entry.path,
                        entry.entry,
                        entry.fragment,
                    ],
                )
                .map_err(|error| format!("adding to playlist: {error}"))?;
        }
        Ok(())
    }

    pub fn playlist_entries(&self, playlist_id: i64) -> Result<Vec<StoredEntry>, String> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT kind, path, entry, fragment FROM playlist_entries
                 WHERE playlist_id = ?1 ORDER BY position, id",
            )
            .map_err(|error| format!("reading playlist: {error}"))?;
        statement
            .query_map([playlist_id], |row| {
                Ok(StoredEntry {
                    kind: row.get(0)?,
                    path: row.get(1)?,
                    entry: row.get(2)?,
                    fragment: row.get(3)?,
                })
            })
            .map_err(|error| format!("reading playlist: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("reading playlist: {error}"))
    }

    /// Entry rows with their row ids, for surgical deletes.
    pub fn playlist_entry_rows(&self, playlist_id: i64) -> Result<Vec<(i64, StoredEntry)>, String> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT id, kind, path, entry, fragment FROM playlist_entries
                 WHERE playlist_id = ?1 ORDER BY position, id",
            )
            .map_err(|error| format!("reading playlist: {error}"))?;
        statement
            .query_map([playlist_id], |row| {
                Ok((
                    row.get(0)?,
                    StoredEntry {
                        kind: row.get(1)?,
                        path: row.get(2)?,
                        entry: row.get(3)?,
                        fragment: row.get(4)?,
                    },
                ))
            })
            .map_err(|error| format!("reading playlist: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("reading playlist: {error}"))
    }

    /// Delete specific entry rows. Returns the number removed.
    pub fn delete_entry_rows(&self, playlist_id: i64, ids: &[i64]) -> Result<usize, String> {
        self.write_transaction(|db| db.delete_entry_rows_in_transaction(playlist_id, ids))
    }

    fn delete_entry_rows_in_transaction(
        &self,
        playlist_id: i64,
        ids: &[i64],
    ) -> Result<usize, String> {
        let mut removed = 0_usize;
        for id in ids {
            removed += self
                .conn
                .execute(
                    "DELETE FROM playlist_entries WHERE playlist_id = ?1 AND id = ?2",
                    rusqlite::params![playlist_id, id],
                )
                .map_err(|error| format!("cleaning playlist: {error}"))?;
        }
        Ok(removed)
    }

    /// Add blacklist rows, skipping kinds/paths already listed. Returns
    /// the number actually added.
    pub fn add_blacklist_entries(&self, entries: &[BlacklistEntry]) -> Result<usize, String> {
        self.write_transaction(|db| db.add_blacklist_entries_in_transaction(entries))
    }

    fn add_blacklist_entries_in_transaction(
        &self,
        entries: &[BlacklistEntry],
    ) -> Result<usize, String> {
        let mut added = 0_usize;
        for entry in entries {
            if entry.kind != BLACKLIST_SONG && entry.kind != BLACKLIST_FOLDER {
                return Err(format!("unknown blacklist kind: {}", entry.kind));
            }
            if entry.path.trim().is_empty() {
                return Err("Blacklist entries need a path".to_owned());
            }
            added += self
                .conn
                .execute(
                    "INSERT INTO blacklist (kind, path, entry) VALUES (?1, ?2, ?3)
                     ON CONFLICT(kind, path, entry) DO NOTHING",
                    rusqlite::params![entry.kind, entry.path, entry.entry],
                )
                .map_err(|error| format!("updating the blacklist: {error}"))?;
        }
        Ok(added)
    }

    pub fn remove_blacklist_entry(&self, id: i64) -> Result<(), String> {
        let changed = self
            .conn
            .execute("DELETE FROM blacklist WHERE id = ?1", [id])
            .map_err(|error| format!("updating the blacklist: {error}"))?;
        if changed == 0 {
            return Err("Blacklist entry no longer exists".to_owned());
        }
        Ok(())
    }

    pub fn list_blacklist(&self) -> Result<Vec<BlacklistEntry>, String> {
        let mut statement = self
            .conn
            .prepare("SELECT id, kind, path, entry FROM blacklist ORDER BY id")
            .map_err(|error| format!("reading the blacklist: {error}"))?;
        statement
            .query_map([], |row| {
                Ok(BlacklistEntry {
                    id: row.get(0)?,
                    kind: row.get(1)?,
                    path: row.get(2)?,
                    entry: row.get(3)?,
                })
            })
            .map_err(|error| format!("reading the blacklist: {error}"))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| format!("reading the blacklist: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> LibraryDb {
        LibraryDb::open_in_memory().expect("memory database")
    }

    #[test]
    #[ignore = "subprocess helper for checkpoint visibility"]
    fn checkpoint_reader_subprocess() {
        let path = std::env::var_os("KOG_TEST_CHECKPOINT_PATH").unwrap();
        let expected = std::env::var("KOG_TEST_CHECKPOINT_VALUE").unwrap();
        let connection = Connection::open(path).unwrap();
        connection.busy_timeout(std::time::Duration::ZERO).unwrap();
        let exclusive = connection.execute_batch("PRAGMA locking_mode=EXCLUSIVE; BEGIN EXCLUSIVE;");
        assert!(
            matches!(&exclusive, Err(rusqlite::Error::SqliteFailure(error, _)) if error.code == rusqlite::ErrorCode::DatabaseBusy),
            "Another process acquired an exclusive lock while the writer was open: {exclusive:?}"
        );
        connection
            .execute_batch("PRAGMA locking_mode=NORMAL;")
            .unwrap();
        let value: String = connection
            .query_row(
                "SELECT value FROM app_state WHERE namespace='sessions' AND key='lock-test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(value, expected);
    }

    #[test]
    fn additional_connections_keep_checkpoints_visible_to_other_processes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("kog.db");
        let writer = LibraryDb::open_at(&path).unwrap();
        writer
            .save_state_checked("sessions", "lock-test", "first", 0)
            .unwrap();
        // Opening preferences/metadata connections must not release this
        // process's SQLite locks or let an external reader remove its WAL.
        let _second = LibraryDb::open_at(&path).unwrap();
        let read_from_another_process = |expected: &str| {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "db::tests::checkpoint_reader_subprocess",
                    "--ignored",
                ])
                .env("KOG_TEST_CHECKPOINT_PATH", &path)
                .env("KOG_TEST_CHECKPOINT_VALUE", expected)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        };
        read_from_another_process("first");
        writer
            .save_state_checked("sessions", "lock-test", "second", 1)
            .unwrap();
        read_from_another_process("second");
    }

    #[test]
    fn independent_connections_keep_readers_live_and_reject_stale_checkpoints() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("kog.db");
        let first = LibraryDb::open_at(&path).unwrap();
        let second = LibraryDb::open_at(&path).unwrap();
        assert_eq!(
            first
                .conn
                .pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
        assert_eq!(
            first
                .save_state_checked("sessions", "qt:test", "first", 0)
                .unwrap(),
            1
        );
        let transaction =
            Transaction::new_unchecked(&first.conn, TransactionBehavior::Immediate).unwrap();
        first
            .save_state_checked("sessions", "qt:test", "uncommitted", 1)
            .unwrap();
        // This read must not need the writer's lock or observe its uncommitted row.
        assert_eq!(
            second
                .load_state("sessions", "qt:test")
                .unwrap()
                .unwrap()
                .value,
            "first"
        );
        transaction.commit().unwrap();
        assert!(matches!(
            second.save_state_checked("sessions", "qt:test", "stale", 1),
            Err(StateWriteError::Conflict)
        ));
        assert_eq!(
            second
                .load_state("sessions", "qt:test")
                .unwrap()
                .unwrap()
                .value,
            "uncommitted"
        );
        assert_eq!(
            second
                .save_state_checked("sessions", "web:test", "independent", 0)
                .unwrap(),
            1
        );
        assert_eq!(
            second
                .import_state("sessions", "qt:test", "old JSON")
                .unwrap()
                .value,
            "uncommitted"
        );
    }

    #[test]
    fn compound_mutations_roll_back_completely() {
        let db = memory_db();
        let item = |path: &str| StoredEntry {
            kind: KIND_LOCAL.into(),
            path: path.into(),
            entry: String::new(),
            fragment: None,
        };
        let id = db
            .create_playlist_with_entries("Original", &[item("kept")])
            .unwrap();
        db.conn.execute_batch("CREATE TRIGGER reject_entry BEFORE INSERT ON playlist_entries WHEN NEW.path = 'reject' BEGIN SELECT RAISE(ABORT, 'rejected'); END;").unwrap();
        assert!(
            db.append_entries(id, &[item("partial"), item("reject")])
                .is_err()
        );
        assert_eq!(db.playlist_entries(id).unwrap(), vec![item("kept")]);
        assert!(
            db.create_playlist_with_entries("Incomplete", &[item("partial"), item("reject")])
                .is_err()
        );
        assert!(db.duplicate_playlist(123456, "Missing source").is_err());
        assert_eq!(db.list_playlists().unwrap().len(), 1);
        assert!(
            db.add_blacklist_entries(&[
                BlacklistEntry {
                    id: 0,
                    kind: BLACKLIST_SONG.into(),
                    path: "partial".into(),
                    entry: String::new()
                },
                BlacklistEntry {
                    id: 0,
                    kind: "invalid".into(),
                    path: "reject".into(),
                    entry: String::new()
                },
            ])
            .is_err()
        );
        assert!(db.list_blacklist().unwrap().is_empty());
        // Foreign-key enforcement applies to the in-memory fallback as well.
        db.delete_playlist(id).unwrap();
        assert!(db.playlist_entries(id).unwrap().is_empty());
    }

    #[test]
    fn stars_set_lookup_and_clear() {
        let db = memory_db();
        assert!(!db.is_starred("a"));
        db.set_star("a", KIND_LOCAL, "/music/a.flac", "", None, true)
            .unwrap();
        assert!(db.is_starred("a"));
        db.set_star("a", KIND_LOCAL, "/music/a.flac", "", None, false)
            .unwrap();
        assert!(!db.is_starred("a"));
    }

    #[test]
    fn playlist_entry_rows_delete_by_id() {
        let db = memory_db();
        let list = db.create_playlist("Cleanup").unwrap();
        let entry = |path: &str| StoredEntry {
            kind: KIND_LOCAL.to_owned(),
            path: path.to_owned(),
            entry: String::new(),
            fragment: None,
        };
        db.append_entries(
            list,
            &[entry("/music/gone.flac"), entry("/music/kept.flac")],
        )
        .unwrap();
        let rows = db.playlist_entry_rows(list).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(db.delete_entry_rows(list, &[]).unwrap(), 0);
        let gone = rows
            .iter()
            .find(|(_, stored)| stored.path == "/music/gone.flac")
            .map(|(row_id, _)| *row_id)
            .expect("missing row id");
        assert_eq!(db.delete_entry_rows(list, &[gone, 999_999]).unwrap(), 1);
        let remaining = db.playlist_entry_rows(list).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].1.path, "/music/kept.flac");
    }

    #[test]
    fn blacklist_add_list_and_remove() {
        let db = memory_db();
        assert!(db.list_blacklist().unwrap().is_empty());
        let song = |path: &str| BlacklistEntry {
            id: 0,
            kind: BLACKLIST_SONG.to_owned(),
            path: path.to_owned(),
            entry: String::new(),
        };
        let folder = |path: &str| BlacklistEntry {
            id: 0,
            kind: BLACKLIST_FOLDER.to_owned(),
            path: path.to_owned(),
            entry: String::new(),
        };
        assert_eq!(
            db.add_blacklist_entries(&[song("/music/a.flac"), folder("/music/nope")])
                .unwrap(),
            2
        );
        // Duplicates are ignored, bad kinds and empty paths rejected.
        assert_eq!(
            db.add_blacklist_entries(&[song("/music/a.flac")]).unwrap(),
            0
        );
        assert!(
            db.add_blacklist_entries(&[BlacklistEntry {
                id: 0,
                kind: "bogus".to_owned(),
                path: "/music/a.flac".to_owned(),
                entry: String::new(),
            }])
            .is_err()
        );
        assert!(db.add_blacklist_entries(&[song("   ")]).is_err());
        let rows = db.list_blacklist().unwrap();
        assert_eq!(rows.len(), 2);
        assert!(db.remove_blacklist_entry(999_999).is_err());
        db.remove_blacklist_entry(rows[0].id).unwrap();
        assert_eq!(db.list_blacklist().unwrap().len(), 1);
    }

    #[test]
    fn checked_playlist_save_preserves_conflicts_and_rolls_back_failed_writes() {
        let db = memory_db();
        let id = db.create_playlist("Draft").unwrap();
        let entry = |path: &str| StoredEntry {
            kind: KIND_LOCAL.into(),
            path: path.into(),
            entry: String::new(),
            fragment: None,
        };
        let original = vec![entry("/music/a.flac"), entry("/music/a.flac")];
        db.append_entries(id, &original).unwrap();
        let updated = vec![entry("/music/b.flac")];
        db.replace_entries_checked(id, &updated, Some(&original))
            .unwrap();
        assert!(
            db.replace_entries_checked(id, &original, Some(&original))
                .unwrap_err()
                .contains("changed elsewhere")
        );
        assert_eq!(db.playlist_entries(id).unwrap(), updated);
        db.conn.execute_batch("CREATE TRIGGER reject_bad_playlist_entry BEFORE INSERT ON playlist_entries WHEN NEW.path = 'reject' BEGIN SELECT RAISE(ABORT, 'test write failure'); END;").unwrap();
        assert!(
            db.replace_entries_checked(
                id,
                &[entry("/music/c.flac"), entry("reject")],
                Some(&updated)
            )
            .is_err()
        );
        assert_eq!(db.playlist_entries(id).unwrap(), updated);
        db.delete_playlist(id).unwrap();
        assert!(db.replace_entries_checked(id, &original, None).is_err());
    }

    #[test]
    fn playlists_crud_reorder_and_duplicate() {
        let db = memory_db();
        assert!(db.list_playlists().unwrap().is_empty());
        assert!(db.create_playlist("  ").is_err());
        let rock = db.create_playlist("Rock").unwrap();
        assert!(db.create_playlist("Rock").is_err());
        let jazz = db.create_playlist("Jazz").unwrap();
        db.append_entries(
            rock,
            &[StoredEntry {
                kind: KIND_LOCAL.to_owned(),
                path: "/music/r.flac".to_owned(),
                entry: String::new(),
                fragment: None,
            }],
        )
        .unwrap();
        assert_eq!(db.playlist_entries(rock).unwrap().len(), 1);
        db.move_playlist(jazz, 0).unwrap();
        let names: Vec<String> = db
            .list_playlists()
            .unwrap()
            .into_iter()
            .map(|playlist| playlist.name)
            .collect();
        assert_eq!(names, vec!["Jazz".to_owned(), "Rock".to_owned()]);
        let copy = db.duplicate_playlist(rock, "Rock copy").unwrap();
        assert_eq!(db.playlist_entries(copy).unwrap().len(), 1);
        db.rename_playlist(copy, "Rock 2").unwrap();
        assert!(db.rename_playlist(copy, "Jazz").is_err());
        db.delete_playlist(copy).unwrap();
        assert!(db.delete_playlist(99999).is_err());
        assert_eq!(db.list_playlists().unwrap().len(), 2);
    }
}
