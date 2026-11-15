//! The cover grid's tiles: one per album, grouped by the album tag plus the
//! album artist (or the artist without one). Songs with no album tag stay
//! tiles of their own; folders and paths never group songs.
use std::collections::HashMap;
use std::time::Duration;

use kog_audio::track::Track;
use serde_json::{Value, json};

/// One tile: its songs as positions in the list the grid shows, in order,
/// with each song's number there (what `track_number_at` gives, which the
/// playing song is matched by).
#[derive(Debug, Default, PartialEq)]
pub struct AlbumGroup {
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

/// Groups the shown songs, as their numbers and tracks (`None` for a row
/// without metadata yet), into tiles in the order each album first appears.
pub fn group<'a>(tracks: impl IntoIterator<Item = (usize, Option<&'a Track>)>) -> Vec<AlbumGroup> {
    let mut groups: Vec<AlbumGroup> = Vec::new();
    let mut by_key: HashMap<(String, String), usize> = HashMap::new();
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
        let artist = group_artist(track);
        let key = (album.to_lowercase(), artist.to_lowercase());
        let at = *by_key.entry(key).or_insert_with(|| {
            groups.push(AlbumGroup {
                album: album.to_owned(),
                artist: artist.to_owned(),
                ..AlbumGroup::default()
            });
            groups.len() - 1
        });
        let group = &mut groups[at];
        group.rows.push(row);
        group.numbers.push(number);
        group.length += track.duration.unwrap_or_default();
        if group.year.is_none() {
            group.year = track.year;
        }
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
    fn groups_by_album_and_album_artist_falling_back_to_artist() {
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
