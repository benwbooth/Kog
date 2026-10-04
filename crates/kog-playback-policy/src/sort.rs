//! Playlist comparisons shared by native and browser frontends.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Frontends supply column values; the backend owns missing-value handling,
/// natural text order, favorite priority, direction, and stable tie breaking.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum SortValue {
    Text(String),
    Number(Option<f64>),
    Star(bool),
    Group(Vec<SortValue>),
}

pub fn compare_values(left: &SortValue, right: &SortValue) -> Ordering {
    match (left, right) {
        (SortValue::Text(left), SortValue::Text(right)) => natural_compare(left, right),
        (SortValue::Number(left), SortValue::Number(right)) => {
            let finite = |value: &Option<f64>| value.filter(|value| value.is_finite());
            finite(left)
                .partial_cmp(&finite(right))
                .unwrap_or(Ordering::Equal)
        }
        (SortValue::Star(left), SortValue::Star(right)) => favorites_first(*left, *right),
        (SortValue::Group(left), SortValue::Group(right)) => left
            .iter()
            .zip(right)
            .map(|(left, right)| compare_values(left, right))
            .find(|order| !order.is_eq())
            .unwrap_or_else(|| left.len().cmp(&right.len())),
        // A column adapter always emits one type. Keep malformed mixed input
        // deterministic rather than letting each platform invent a fallback.
        _ => value_kind(left).cmp(&value_kind(right)),
    }
}

fn value_kind(value: &SortValue) -> u8 {
    match value {
        SortValue::Text(_) => 0,
        SortValue::Number(_) => 1,
        SortValue::Star(_) => 2,
        SortValue::Group(_) => 3,
    }
}

/// Canonical column projection. Adapters only supply metadata and locators.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SortRow {
    pub original: Option<f64>,
    pub title: String,
    pub artist: String,
    pub album: String,
    #[serde(alias = "albumArtist")]
    pub album_artist: String,
    pub composer: String,
    pub genre: String,
    pub year: Option<f64>,
    #[serde(alias = "discNumber")]
    pub disc_number: Option<f64>,
    #[serde(alias = "trackNumber")]
    pub track_number: Option<f64>,
    pub duration: Option<f64>,
    #[serde(alias = "fileSizeBytes")]
    pub file_size_bytes: Option<f64>,
    #[serde(alias = "sampleRate")]
    pub sample_rate: Option<f64>,
    #[serde(alias = "bitsPerSample")]
    pub bits_per_sample: Option<f64>,
    pub bitrate: Option<f64>,
    pub channels: Option<f64>,
    pub codec: String,
    pub path: String,
    pub filename: String,
    pub star: bool,
}

impl SortRow {
    pub fn matches(&self, query: &str) -> bool {
        matches_query(
            &[
                &self.title,
                &self.artist,
                &self.album_artist,
                &self.album,
                &self.genre,
                &self.composer,
                &self.filename,
            ],
            query,
        )
    }
    pub fn value(&self, column: &str) -> SortValue {
        use SortValue::{Group, Number, Star, Text};
        match column.to_ascii_lowercase().as_str() {
            "index" | "original" => Number(self.original),
            "title" => Text(self.title.clone()),
            "artist" => Text(self.artist.clone()),
            "album" => Text(self.album.clone()),
            "albumartist" => Text(self.album_artist.clone()),
            "composer" => Text(self.composer.clone()),
            "genre" => Text(self.genre.clone()),
            "year" | "date" => Number(self.year),
            "discnumber" => Number(self.disc_number),
            "track" | "tracknumber" => Group(vec![
                Text(
                    if self.album_artist.is_empty() {
                        &self.artist
                    } else {
                        &self.album_artist
                    }
                    .clone(),
                ),
                Text(self.album.clone()),
                Number(self.disc_number),
                Number(self.track_number),
            ]),
            "length" | "duration" => Number(self.duration),
            "filesizebytes" | "filesize" => Number(self.file_size_bytes),
            "samplerate" => Number(self.sample_rate),
            "bitspersample" => Number(self.bits_per_sample),
            "bitrate" => Number(self.bitrate),
            "channels" => Number(self.channels),
            "codec" => Text(self.codec.clone()),
            "path" => Text(self.path.clone()),
            "filename" => Text(self.filename.clone()),
            "star" => Star(self.star),
            _ => Text(String::new()),
        }
    }
}

pub fn matches_query(fields: &[&str], query: &str) -> bool {
    let fields = fields.join("\n").to_lowercase();
    query
        .split_whitespace()
        .all(|word| fields.contains(&word.to_lowercase()))
}

pub fn sorted_rows(rows: &[SortRow], column: &str, descending: bool) -> Vec<usize> {
    sorted_indices(
        &rows.iter().map(|row| row.value(column)).collect::<Vec<_>>(),
        descending,
    )
}

pub fn sorted_indices(values: &[SortValue], descending: bool) -> Vec<usize> {
    let mut indices: Vec<_> = (0..values.len()).collect();
    indices.sort_by(|left, right| {
        let order = compare_values(&values[*left], &values[*right]);
        (if descending { order.reverse() } else { order }).then(left.cmp(right))
    });
    indices
}

/// Compare text case-insensitively, treating runs of ASCII digits as numbers.
/// Leading zeroes do not change the numeric value.
pub fn natural_compare(left: &str, right: &str) -> Ordering {
    let left = left.to_lowercase().chars().collect::<Vec<_>>();
    let right = right.to_lowercase().chars().collect::<Vec<_>>();
    let mut left_index = 0;
    let mut right_index = 0;

    while left_index < left.len() && right_index < right.len() {
        if left[left_index].is_ascii_digit() && right[right_index].is_ascii_digit() {
            let left_end = left[left_index..]
                .iter()
                .position(|character| !character.is_ascii_digit())
                .map_or(left.len(), |offset| left_index + offset);
            let right_end = right[right_index..]
                .iter()
                .position(|character| !character.is_ascii_digit())
                .map_or(right.len(), |offset| right_index + offset);
            let left_significant = left_index
                + left[left_index..left_end]
                    .iter()
                    .take_while(|character| **character == '0')
                    .count();
            let right_significant = right_index
                + right[right_index..right_end]
                    .iter()
                    .take_while(|character| **character == '0')
                    .count();
            let left_digits = &left[left_significant..left_end];
            let right_digits = &right[right_significant..right_end];

            match left_digits.len().cmp(&right_digits.len()) {
                Ordering::Equal => match left_digits.cmp(right_digits) {
                    Ordering::Equal => {}
                    ordering => return ordering,
                },
                ordering => return ordering,
            }
            left_index = left_end;
            right_index = right_end;
            continue;
        }

        match left[left_index].cmp(&right[right_index]) {
            Ordering::Equal => {
                left_index += 1;
                right_index += 1;
            }
            ordering => return ordering,
        }
    }

    (left.len() - left_index).cmp(&(right.len() - right_index))
}

/// A star sort starts with favorites in ascending mode.
pub fn favorites_first(left: bool, right: bool) -> Ordering {
    right.cmp(&left)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbered_titles_sort_naturally() {
        assert_eq!(natural_compare("Track 2", "track 10"), Ordering::Less);
        assert_eq!(natural_compare("SONG 01", "song 1"), Ordering::Equal);
        assert_eq!(natural_compare("Alpha", "beta"), Ordering::Less);
    }

    #[test]
    fn ascending_stars_put_favorites_first() {
        assert_eq!(favorites_first(true, false), Ordering::Less);
        assert_eq!(favorites_first(false, true), Ordering::Greater);
        assert_eq!(favorites_first(false, false), Ordering::Equal);
    }
}
