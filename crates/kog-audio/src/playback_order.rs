//! Native adapter for the platform-neutral playback-order policy.

pub use kog_playback_policy::{NavigationEvent, PlaybackDecision, PlaybackOrder, SelectionState, TrackOrderInfo};
pub use kog_playback_policy::{bridge, radio, sort, workspace};

impl TrackOrderInfo for crate::track::Track {
    fn album(&self) -> &str {
        &self.album
    }
    fn disc_number(&self) -> Option<u32> {
        self.disc_number
    }
    fn track_number(&self) -> Option<u32> {
        self.track_number
    }
}

/// Canonical metadata projection for every native column sort.
pub fn sort_row(track: &crate::track::Track) -> sort::SortRow {
    let path = track.source.archive_origin.as_ref().map(|origin| {
        format!("{}/{}", origin.archive_path.display(), origin.entry_name)
    }).unwrap_or_else(|| track.source.remote_url.clone().unwrap_or_else(|| track.source.path.to_string_lossy().into_owned()));
    sort::SortRow {
        title: track.title.clone(), artist: track.artist.clone(), album: track.album.clone(),
        album_artist: track.album_artist.clone(), composer: track.composer.clone(), genre: track.genre.clone(),
        year: track.year.map(f64::from), disc_number: track.disc_number.map(f64::from),
        track_number: track.track_number.map(f64::from), duration: track.duration.map(|value| value.as_secs_f64()),
        file_size_bytes: track.file_size_bytes.map(|value| value as f64), sample_rate: track.sample_rate.map(f64::from),
        bits_per_sample: track.bits_per_sample.map(f64::from), bitrate: track.bitrate.map(f64::from), channels: track.channels.map(f64::from),
        codec: track.codec.clone(), filename: path.rsplit(['/', '\\']).next().unwrap_or_default().to_owned(), path,
        ..sort::SortRow::default()
    }
}
