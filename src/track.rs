use std::path::{Path, PathBuf};
use std::time::Duration;

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;

use crate::decoder::{DecoderRegistry, PlaybackSource};

#[derive(Clone, Debug, Default)]
pub struct Track {
    pub source: PlaybackSource,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: Option<u32>,
    pub track_number: Option<u32>,
    pub duration: Option<Duration>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub bitrate: Option<u32>,
    pub bits_per_sample: Option<u8>,
    pub decoder_warning: Option<String>,
    pub codec: String,
}

impl Track {
    pub fn from_source(source: PlaybackSource, decoders: &DecoderRegistry) -> Self {
        let mut fallback_title = source
            .path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("Untitled")
            .to_owned();
        if let Some(subsong) = source.subsong {
            fallback_title.push_str(&format!(" [{}]", subsong + 1));
        }
        let codec = source
            .path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_uppercase();
        let mut track = Self {
            source,
            title: fallback_title,
            codec,
            ..Self::default()
        };

        if let Ok(tagged_file) = lofty::read_from_path(&track.source.path) {
            let properties = tagged_file.properties();
            track.duration = Some(properties.duration());
            track.sample_rate = properties.sample_rate();
            track.channels = properties.channels().map(u16::from);
            track.bitrate = properties.audio_bitrate();
            track.bits_per_sample = properties.bit_depth();

            if let Some(tag) = tagged_file
                .primary_tag()
                .or_else(|| tagged_file.first_tag())
            {
                if let Some(title) = tag.title() {
                    track.title = title.to_string();
                }
                track.artist = tag
                    .artist()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                track.album = tag
                    .album()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                track.genre = tag
                    .genre()
                    .map(|value| value.to_string())
                    .unwrap_or_default();
                track.year = tag.date().map(|date| u32::from(date.year));
                track.track_number = tag.track();
            }
        }

        if let Ok(properties) = decoders.probe(&track.source) {
            track.duration = properties.duration.or(track.duration);
            track.sample_rate = properties.sample_rate.or(track.sample_rate);
            track.channels = properties.channels.or(track.channels);
            if let Some(title) = properties.title.filter(|value| !value.is_empty()) {
                track.title = title;
            }
            if let Some(artist) = properties.artist.filter(|value| !value.is_empty()) {
                track.artist = artist;
            }
            if let Some(album) = properties.album.filter(|value| !value.is_empty()) {
                track.album = album;
            }
            if let Some(genre) = properties.genre.filter(|value| !value.is_empty()) {
                track.genre = genre;
            }
            track.track_number = properties.track_number.or(track.track_number);
            track.decoder_warning = properties.warning;
        }

        track
    }

    pub fn matches(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let query = query.to_lowercase();
        [
            self.title.as_str(),
            self.artist.as_str(),
            self.album.as_str(),
            self.genre.as_str(),
            self.source.path.to_string_lossy().as_ref(),
        ]
        .iter()
        .any(|value| value.to_lowercase().contains(&query))
    }

    pub fn duration_label(&self) -> String {
        duration_label(self.duration.unwrap_or_default())
    }
}

pub fn duration_label(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

pub fn canonical_path(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("resolving {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_labels_match_music_player_conventions() {
        assert_eq!(duration_label(Duration::from_secs(0)), "0:00");
        assert_eq!(duration_label(Duration::from_secs(185)), "3:05");
        assert_eq!(duration_label(Duration::from_secs(3_661)), "1:01:01");
    }
}
