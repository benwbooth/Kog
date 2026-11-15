//! The cover grid's tiles: one per album, grouped by the album tag plus the
//! album artist, or by the album tag alone for songs without an album artist
//! (so an album crediting different composers stays one tile), as the
//! desktop groups them. Songs with no album tag stay tiles of their own;
//! folders and paths never group songs.
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

/// The artist an album without an album artist shows: the track artist of
/// at least half its songs, else "Various Artists".
fn common_artist<'a>(artists: impl IntoIterator<Item = &'a str>) -> String {
    let mut counts: Vec<(&str, usize)> = Vec::new();
    let mut total = 0;
    for artist in artists {
        total += 1;
        if artist.is_empty() {
            continue;
        }
        match counts.iter_mut().find(|(seen, _)| seen.eq_ignore_ascii_case(artist)) {
            Some((_, count)) => *count += 1,
            None => counts.push((artist, 1)),
        }
    }
    match counts.iter().max_by_key(|(_, count)| *count) {
        Some((artist, count)) if count * 2 >= total => (*artist).to_owned(),
        Some(_) => "Various Artists".to_owned(),
        None => String::new(),
    }
}

/// Groups the shown rows into tiles, in the order each album first appears.
pub fn group<'a>(
    rows: &[(usize, Entry)],
    meta: impl Fn(&Entry) -> Option<&'a MetaRow>,
) -> Vec<AlbumTile> {
    let mut tiles: Vec<AlbumTile> = Vec::new();
    let mut by_key: HashMap<String, usize> = HashMap::new();
    // Each album-artist-less tile's track artists, to pick the one it shows.
    let mut track_artists: HashMap<usize, Vec<&'a str>> = HashMap::new();
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
        let album_artist = tags.map(|tags| trimmed(&tags.album_artist)).unwrap_or_default();
        let key = format!("{}\u{1}{}", album.to_lowercase(), album_artist.to_lowercase());
        let at = *by_key.entry(key.clone()).or_insert_with(|| {
            tiles.push(AlbumTile {
                key,
                album: album.to_owned(),
                artist: album_artist.to_owned(),
                ..AlbumTile::default()
            });
            tiles.len() - 1
        });
        if album_artist.is_empty() {
            track_artists
                .entry(at)
                .or_default()
                .push(tags.map(|tags| trimmed(&tags.artist)).unwrap_or_default());
        }
        let tile = &mut tiles[at];
        tile.rows.push(position);
        tile.seconds += seconds;
        if tile.year.is_none() {
            tile.year = year;
        }
    }
    for (at, artists) in track_artists {
        tiles[at].artist = common_artist(artists);
    }
    tiles
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(index: usize, album: &str, album_artist: &str, artist: &str) -> ((usize, Entry), MetaRow) {
        let entry = Entry {
            kind: "local".to_owned(),
            path: format!("/music/{index}.vgz"),
            entry: String::new(),
            fragment: None,
            name: format!("{index}.vgz"),
            location: String::new(),
        };
        let tags = MetaRow {
            album: Some(album.to_owned()),
            album_artist: Some(album_artist.to_owned()),
            artist: Some(artist.to_owned()),
            ..MetaRow::default()
        };
        ((index, entry), tags)
    }

    fn tiles(rows: Vec<((usize, Entry), MetaRow)>) -> Vec<AlbumTile> {
        let (entries, tags): (Vec<_>, Vec<_>) = rows.into_iter().unzip();
        group(&entries, |entry| {
            entries.iter().position(|(_, other)| other == entry).map(|at| &tags[at])
        })
    }

    #[test]
    fn an_album_crediting_different_composers_stays_one_tile() {
        let tiles = tiles(vec![
            row(0, "Final Fight", "", "Manami Matsumae"),
            row(1, "Final Fight", "", "Yoshihiro Sakaguchi"),
            row(2, "Final Fight", "", "Manami Matsumae"),
            row(3, "Final Fight", "", "Harumi Fujita"),
            row(4, "Final Fight", "Capcom", "Someone"),
        ]);
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].rows, vec![0, 1, 2, 3]);
        assert_eq!(tiles[0].artist, "Manami Matsumae");
        assert_eq!(tiles[1].artist, "Capcom");
    }

    #[test]
    fn no_artist_covering_half_the_album_shows_various_artists() {
        let tiles = tiles(vec![
            row(0, "Out Run", "", "A"),
            row(1, "Out Run", "", "B"),
            row(2, "Out Run", "", "C"),
        ]);
        assert_eq!(tiles.len(), 1);
        assert_eq!(tiles[0].artist, "Various Artists");
    }
}
