//! The cover grid's tiles: one per album, grouped by the album tag plus the
//! album artist (or the artist without one), as the desktop groups them.
//! Songs with no album tag stay tiles of their own; folders and paths never
//! group songs.
use std::collections::HashMap;

use super::{Entry, MetaRow};

/// One tile: its songs as positions in the pane's shown rows, in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AlbumTile {
    /// The album and artist folded to lower case; empty for a lone song.
    pub key: String,
    pub album: String,
    pub artist: String,
    pub year: Option<u32>,
    pub rows: Vec<usize>,
    pub seconds: f64,
}

impl AlbumTile {
    pub fn single(&self) -> bool {
        self.key.is_empty()
    }

    /// "12 songs", or "1 song".
    pub fn count_label(&self) -> String {
        match self.rows.len() {
            1 => "1 song".to_owned(),
            count => format!("{count} songs"),
        }
    }

    /// The album page's length: "45 min 12 sec", or "1 hr 5 min".
    pub fn length_label(&self) -> String {
        let seconds = self.seconds.max(0.0).round() as u64;
        match (seconds / 3_600, seconds / 60 % 60) {
            (0, 0) => format!("{seconds} sec"),
            (0, minutes) => format!("{minutes} min {} sec", seconds % 60),
            (hours, minutes) => format!("{hours} hr {minutes} min"),
        }
    }
}

fn trimmed(text: &Option<String>) -> &str {
    text.as_deref().map(str::trim).unwrap_or_default()
}

/// Groups the shown rows into tiles, in the order each album first appears.
pub fn group<'a>(
    rows: &[(usize, Entry)],
    meta: impl Fn(&Entry) -> Option<&'a MetaRow>,
) -> Vec<AlbumTile> {
    let mut tiles: Vec<AlbumTile> = Vec::new();
    let mut by_key: HashMap<String, usize> = HashMap::new();
    for (position, (_, entry)) in rows.iter().enumerate() {
        let tags = meta(entry);
        let album = tags.map(|tags| trimmed(&tags.album)).unwrap_or_default();
        let artist = tags
            .map(|tags| {
                let album_artist = trimmed(&tags.album_artist);
                if album_artist.is_empty() { trimmed(&tags.artist) } else { album_artist }
            })
            .unwrap_or_default();
        let seconds = tags.and_then(|tags| tags.duration).unwrap_or_default();
        let year = tags.and_then(|tags| tags.year);
        if album.is_empty() {
            tiles.push(AlbumTile {
                artist: artist.to_owned(),
                year,
                rows: vec![position],
                seconds,
                ..AlbumTile::default()
            });
            continue;
        }
        let key = format!("{}\u{1}{}", album.to_lowercase(), artist.to_lowercase());
        let at = *by_key.entry(key.clone()).or_insert_with(|| {
            tiles.push(AlbumTile {
                key,
                album: album.to_owned(),
                artist: artist.to_owned(),
                ..AlbumTile::default()
            });
            tiles.len() - 1
        });
        let tile = &mut tiles[at];
        tile.rows.push(position);
        tile.seconds += seconds;
        if tile.year.is_none() {
            tile.year = year;
        }
    }
    tiles
}
