//! SQLite library store (single file): starred songs and custom playlists.
//!
//! One database, `kog.db`, holds what used to need ad-hoc files: the star
//! set (favorites source of truth) and named playlists with full-fidelity
//! entries (local files, archive members, remote URLs, subsong fragments).
//! Small settings files and the hot-path radio round stay where they are;
//! this store is for relational library data with keyed lookups.
//!
//! Locator keys reuse the radio identity scheme (plain path,
//! `outer :: member`, remote URL) so stars, the dead set, and dedup logic
//! all name a track the same way.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

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
";

const SCHEMA_VERSION: &str = "1";

/// Entry kinds stored in `kind` columns. Archive members keep outer path +
/// member name; remote tracks keep the URL in `path`.
pub const KIND_LOCAL: &str = "local";
pub const KIND_ARCHIVE: &str = "archive";
pub const KIND_REMOTE: &str = "remote";

pub struct StoredPlaylist {
    pub id: i64,
    pub name: String,
    pub position: i64,
    pub entry_count: i64,
}

pub struct StoredEntry {
    pub kind: String,
    pub path: String,
    pub entry: String,
    pub fragment: Option<String>,
}

pub struct LibraryDb {
    conn: Connection,
}

fn database_path() -> PathBuf {
    directories::ProjectDirs::from("org", "Kog", "Kog")
        .map(|directories| directories.data_dir().join("kog.db"))
        .unwrap_or_else(|| std::env::temp_dir().join("kog.db"))
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|age| age.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

impl LibraryDb {
    pub fn open() -> Result<Self, String> {
        Self::open_at(&database_path())
    }

    /// Memory-only database for tests and for degraded startup when the
    /// data directory is unavailable (stars/playlists won't persist).
    pub fn open_in_memory() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|error| format!("opening library database: {error}"))?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    pub fn open_at(path: &Path) -> Result<Self, String> {        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("creating {}: {error}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .map_err(|error| format!("opening {}: {error}", path.display()))?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(|error| format!("configuring {}: {error}", path.display()))?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<(), String> {
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
            .query_row(
                "SELECT 1 FROM stars WHERE locator = ?1",
                [locator],
                |_| Ok(()),
            )
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
            .prepare(
                "SELECT kind, path, entry, fragment FROM stars ORDER BY starred_at, locator",
            )
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
        let new_id = self.create_playlist(name)?;
        let entries = self.playlist_entries(id)?;
        self.append_entries(new_id, &entries)?;
        Ok(new_id)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> LibraryDb {
        let conn = Connection::open_in_memory().expect("memory database");
        let db = LibraryDb { conn };
        db.migrate().expect("migrate memory database");
        db
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
