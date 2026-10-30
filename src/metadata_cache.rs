//! Song info read from files, saved between launches so open playlist tabs
//! fill in at startup instead of opening every file again. Each row is tied
//! to its file's size and modification time and is re-read when either
//! changes.
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use kog_audio::decoder::PlaybackSource;
use kog_audio::track::Track;
use kog_core::db::StoredEntry;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::workspace_tracks::entry_key;

/// Bump when `Track::from_source` changes what it makes of a file (titles,
/// albums), so saved rows are read again.
const FORMAT: u32 = 1;

pub struct MetadataCache {
    conn: Connection,
}

fn cache_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "Kog", "Kog").map(|dirs| dirs.cache_dir().join("song-info.db"))
}

impl MetadataCache {
    pub fn open() -> Option<Self> {
        let path = cache_path()?;
        std::fs::create_dir_all(path.parent()?).ok()?;
        let conn = Connection::open(&path).ok()?;
        conn.busy_timeout(Duration::from_secs(5)).ok()?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS songs (key TEXT PRIMARY KEY, stamp TEXT NOT NULL, track TEXT NOT NULL);",
        )
        .ok()?;
        Some(Self { conn })
    }

    pub fn get(&self, entry: &StoredEntry) -> Option<Track> {
        let stamp = stamp(entry)?;
        let (saved, text): (String, String) = self
            .conn
            .query_row("SELECT stamp, track FROM songs WHERE key = ?1", [entry_key(entry)], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .optional()
            .ok()??;
        (saved == stamp).then(|| track_from_json(entry, &serde_json::from_str(&text).ok()?))?
    }

    pub fn put_all(&mut self, songs: &[(StoredEntry, Track)]) {
        let Ok(transaction) = self.conn.transaction() else { return };
        for (entry, track) in songs {
            if let Some(stamp) = stamp(entry) {
                let _ = transaction.execute(
                    "INSERT OR REPLACE INTO songs (key, stamp, track) VALUES (?1, ?2, ?3)",
                    params![entry_key(entry), stamp, track_json(track).to_string()],
                );
            }
        }
        let _ = transaction.commit();
    }
}

/// The file's size and modification time; none for streams and lost files.
fn stamp(entry: &StoredEntry) -> Option<String> {
    if entry.kind != "local" && entry.kind != "archive" {
        return None;
    }
    let metadata = std::fs::metadata(&entry.path).ok()?;
    let modified = metadata.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos();
    // Looping songs' lengths follow the looping setting.
    let looping = kog_audio::looping::current().setting_value();
    Some(format!("{FORMAT}:{looping}:{}:{modified}", metadata.len()))
}

fn track_json(track: &Track) -> Value {
    json!({
        "title": track.title, "artist": track.artist, "album_artist": track.album_artist,
        "album": track.album, "genre": track.genre, "composer": track.composer,
        "year": track.year, "disc": track.disc_number, "track": track.track_number,
        "duration_ms": track.duration.map(|d| d.as_millis() as u64),
        "size": track.file_size_bytes, "rate": track.sample_rate, "channels": track.channels,
        "bitrate": track.bitrate, "bits": track.bits_per_sample, "warning": track.decoder_warning,
        "codec": track.codec, "backend": track.backend_id, "search": track.search_text,
    })
}

fn track_from_json(entry: &StoredEntry, value: &Value) -> Option<Track> {
    let text = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default().to_owned();
    let number = |key: &str| value.get(key).and_then(Value::as_u64);
    let small = |key: &str| number(key).and_then(|n| u32::try_from(n).ok());
    Some(Track {
        source: PlaybackSource::from_path(PathBuf::from(&entry.path)),
        title: value.get("title")?.as_str()?.to_owned(),
        artist: text("artist"),
        album_artist: text("album_artist"),
        album: text("album"),
        genre: text("genre"),
        composer: text("composer"),
        lyrics: String::new(),
        year: small("year"),
        disc_number: small("disc"),
        track_number: small("track"),
        duration: number("duration_ms").map(Duration::from_millis),
        file_size_bytes: number("size"),
        sample_rate: small("rate"),
        channels: number("channels").and_then(|n| u16::try_from(n).ok()),
        bitrate: small("bitrate"),
        bits_per_sample: number("bits").and_then(|n| u8::try_from(n).ok()),
        decoder_warning: value.get("warning").and_then(Value::as_str).map(str::to_owned),
        codec: text("codec"),
        backend_id: text("backend"),
        search_text: text("search"),
        missing: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_song_info_round_trips() {
        let entry = StoredEntry { kind: "local".into(), path: "/a.mid".into(), entry: String::new(), fragment: None };
        let track = Track {
            title: "Intro".into(),
            album: "Ultima 8".into(),
            duration: Some(Duration::from_millis(61_500)),
            track_number: Some(3),
            ..Track::default()
        };
        let back = track_from_json(&entry, &track_json(&track)).unwrap();
        assert_eq!(back.title, "Intro");
        assert_eq!(back.album, "Ultima 8");
        assert_eq!(back.duration, Some(Duration::from_millis(61_500)));
        assert_eq!(back.track_number, Some(3));
    }
}
