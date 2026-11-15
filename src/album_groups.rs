//! The cover grid's tiles: one per album, grouped by the album tag plus the
//! album artist, or by the album tag alone for songs without an album
//! artist (so an album whose songs credit different composers stays one
//! tile). Songs with no album tag stay tiles of their own; folders and paths
//! never group songs.
use std::collections::HashMap;
use std::time::Duration;

use kog_audio::track::Track;
use serde_json::{Value, json};

/// One tile: its songs as positions in the list the grid shows, in order,
/// with each song's number there (what `track_number_at` gives, which the
/// playing song is matched by).
#[derive(Debug, Default, PartialEq)]
pub struct AlbumGroup {
    /// The album and album artist folded to lower case; empty for a lone
    /// song.
    pub key: String,
    pub album: String,
    pub artist: String,
    pub year: Option<u32>,
    pub rows: Vec<usize>,
    pub numbers: Vec<usize>,
    pub length: Duration,
}

impl AlbumGroup {
    /// Whether this tile is a song without an album rather than an album.
    pub fn single(&self) -> bool {
        self.album.is_empty()
    }
}

fn group_artist(track: &Track) -> &str {
    let album_artist = track.album_artist.trim();
    if album_artist.is_empty() { track.artist.trim() } else { album_artist }
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

/// Groups the shown songs, as their numbers and tracks (`None` for a row
/// without metadata yet), into tiles in the order each album first appears.
pub fn group<'a>(tracks: impl IntoIterator<Item = (usize, Option<&'a Track>)>) -> Vec<AlbumGroup> {
    let mut groups: Vec<AlbumGroup> = Vec::new();
    let mut by_key: HashMap<String, usize> = HashMap::new();
    // Each album-artist-less group's track artists, to pick the one it shows.
    let mut track_artists: HashMap<usize, Vec<&'a str>> = HashMap::new();
    for (row, (number, track)) in tracks.into_iter().enumerate() {
        let album = track.map(|track| track.album.trim()).unwrap_or_default();
        if album.is_empty() {
            groups.push(AlbumGroup {
                rows: vec![row],
                numbers: vec![number],
                length: track.and_then(|track| track.duration).unwrap_or_default(),
                year: track.and_then(|track| track.year),
                artist: track.map(group_artist).unwrap_or_default().to_owned(),
                ..AlbumGroup::default()
            });
            continue;
        }
        // A row without metadata has no album, so it was a single above.
        let Some(track) = track else { continue };
        let album_artist = track.album_artist.trim();
        let key = format!("{}\u{1}{}", album.to_lowercase(), album_artist.to_lowercase());
        let at = *by_key.entry(key.clone()).or_insert_with(|| {
            groups.push(AlbumGroup {
                key,
                album: album.to_owned(),
                artist: album_artist.to_owned(),
                ..AlbumGroup::default()
            });
            groups.len() - 1
        });
        if album_artist.is_empty() {
            track_artists.entry(at).or_default().push(track.artist.trim());
        }
        let group = &mut groups[at];
        group.rows.push(row);
        group.numbers.push(number);
        group.length += track.duration.unwrap_or_default();
        if group.year.is_none() {
            group.year = track.year;
        }
    }
    for (at, artists) in track_artists {
        groups[at].artist = common_artist(artists);
    }
    groups
}

fn length_label(length: Duration) -> String {
    let seconds = length.as_secs();
    match (seconds / 3_600, seconds / 60 % 60) {
        (0, 0) => format!("{seconds} sec"),
        (0, minutes) => format!("{minutes} min {} sec", seconds % 60),
        (hours, minutes) => format!("{hours} hr {minutes} min"),
    }
}

/// The tiles for QML: `[{album, artist, year, length, single, rows}]`.
pub fn to_json(groups: &[AlbumGroup]) -> String {
    Value::Array(
        groups
            .iter()
            .map(|group| {
                json!({
                    "key": group.key,
                    "album": group.album,
                    "artist": group.artist,
                    "year": group.year.map(|year| year.to_string()).unwrap_or_default(),
                    "length": length_label(group.length),
                    "single": group.single(),
                    "rows": group.rows,
                    "numbers": group.numbers,
                })
            })
            .collect(),
    )
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(album: &str, album_artist: &str, artist: &str, seconds: u64) -> Track {
        Track {
            album: album.to_owned(),
            album_artist: album_artist.to_owned(),
            artist: artist.to_owned(),
            duration: Some(Duration::from_secs(seconds)),
            ..Track::default()
        }
    }

    #[test]
    fn groups_by_album_and_album_artist() {
        let tracks = [
            track("Blue", "", "Joni", 60),
            track("", "", "Loose", 30),
            track("blue", "", "joni", 90),
            track("Blue", "Various", "Someone", 10),
            track("Blue", "Various", "Other", 10),
        ];
        let groups = group(tracks.iter().enumerate().map(|(row, track)| (row + 1, Some(track))));
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].rows, vec![0, 2]);
        assert_eq!(groups[0].numbers, vec![1, 3]);
        assert_eq!(groups[0].length, Duration::from_secs(150));
        assert!(groups[1].single());
        assert_eq!(groups[1].rows, vec![1]);
        assert_eq!(groups[2].artist, "Various");
        assert_eq!(groups[2].rows, vec![3, 4]);
    }

    #[test]
    fn an_album_crediting_different_composers_stays_one_tile() {
        let tracks = [
            track("Final Fight", "", "Manami Matsumae", 60),
            track("Final Fight", "", "Yoshihiro Sakaguchi", 60),
            track("Final Fight", "", "Manami Matsumae", 60),
            track("Final Fight", "", "Harumi Fujita", 60),
        ];
        let groups = group(tracks.iter().enumerate().map(|(row, track)| (row + 1, Some(track))));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].rows, vec![0, 1, 2, 3]);
        // Two of four songs: the most common artist covers half of them.
        assert_eq!(groups[0].artist, "Manami Matsumae");
    }

    #[test]
    fn no_artist_covering_half_the_album_shows_various_artists() {
        let tracks = [
            track("Out Run", "", "A", 60),
            track("Out Run", "", "B", 60),
            track("Out Run", "", "C", 60),
        ];
        let groups = group(tracks.iter().enumerate().map(|(row, track)| (row + 1, Some(track))));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].artist, "Various Artists");
    }

    #[test]
    fn rows_without_metadata_stay_single() {
        let groups = group([(1, None), (2, None)]);
        assert_eq!(groups.len(), 2);
        assert!(groups.iter().all(AlbumGroup::single));
    }

    #[test]
    fn lengths_read_like_an_album_page() {
        assert_eq!(length_label(Duration::from_secs(45)), "45 sec");
        assert_eq!(length_label(Duration::from_secs(185)), "3 min 5 sec");
        assert_eq!(length_label(Duration::from_secs(3_900)), "1 hr 5 min");
    }
}
