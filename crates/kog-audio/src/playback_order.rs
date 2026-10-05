//! Native adapter for the platform-neutral playback-order policy.

pub use kog_playback_policy::{
    NavigationEvent, PlaybackDecision, PlaybackOrder, SelectionState, TrackOrderInfo,
};
pub use kog_playback_policy::{bridge, ffi, radio, selection, session, sort, workspace};

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
    let path = track
        .source
        .archive_origin
        .as_ref()
        .map(|origin| format!("{}/{}", origin.archive_path.display(), origin.entry_name))
        .unwrap_or_else(|| {
            track
                .source
                .remote_url
                .clone()
                .unwrap_or_else(|| track.source.path.to_string_lossy().into_owned())
        });
    sort::SortRow {
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        album_artist: track.album_artist.clone(),
        composer: track.composer.clone(),
        genre: track.genre.clone(),
        year: track.year.map(f64::from),
        disc_number: track.disc_number.map(f64::from),
        track_number: track.track_number.map(f64::from),
        duration: track.duration.map(|value| value.as_secs_f64()),
        file_size_bytes: track.file_size_bytes.map(|value| value as f64),
        sample_rate: track.sample_rate.map(f64::from),
        bits_per_sample: track.bits_per_sample.map(f64::from),
        bitrate: track.bitrate.map(f64::from),
        channels: track.channels.map(f64::from),
        codec: track.codec.clone(),
        filename: path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or_default()
            .to_owned(),
        path,
        ..sort::SortRow::default()
    }
}

use crate::playlist::{PlaylistEntry, PlaylistLocation};
use crate::track::Track;
pub fn playlist_entry_for_track(track: &Track) -> Result<PlaylistEntry, String> {
    let location = if let Some(origin) = &track.source.archive_origin {
        PlaylistLocation::Archive {
            archive_path: origin.archive_path.clone(),
            entry_name: origin.entry_name.clone(),
        }
    } else if let Some(url) = &track.source.remote_url {
        PlaylistLocation::Remote(url.clone())
    } else {
        PlaylistLocation::Local(track.source.path.clone())
    };
    let fragment = match track.source.subsong {
        None => None,
        Some(_) if track.backend_id == "cuesheet" => Some(
            track
                .track_number
                .ok_or_else(|| {
                    format!(
                        "CueSheet track {} has no declared track number",
                        track.source.display_label()
                    )
                })?
                .to_string(),
        ),
        Some(subsong) => Some(subsong.to_string()),
    };
    Ok(PlaylistEntry { location, fragment })
}
pub fn stored_entry_for_track(track: &Track) -> Option<kog_core::db::StoredEntry> {
    let entry = playlist_entry_for_track(track).ok()?;
    let (kind, path, name) = match entry.location {
        crate::playlist::PlaylistLocation::Local(path) => (
            kog_core::db::KIND_LOCAL.to_owned(),
            path.to_string_lossy().into_owned(),
            String::new(),
        ),
        crate::playlist::PlaylistLocation::Archive {
            archive_path,
            entry_name,
        } => (
            kog_core::db::KIND_ARCHIVE.to_owned(),
            archive_path.to_string_lossy().into_owned(),
            entry_name,
        ),
        crate::playlist::PlaylistLocation::Remote(url) => {
            (kog_core::db::KIND_REMOTE.to_owned(), url, String::new())
        }
    };
    Some(kog_core::db::StoredEntry {
        kind,
        path,
        entry: name,
        fragment: entry.fragment,
    })
}

impl session::Item for Track {
    fn entry(&self) -> serde_json::Value {
        stored_entry_for_track(self).map(|e| serde_json::json!({"kind":e.kind,"path":e.path,"entry":e.entry,"fragment":e.fragment})).unwrap_or_default()
    }
    fn metadata(&self) -> sort::SortRow {
        sort_row(self)
    }
}
/// Separate, filesystem-safe persistence namespace for each application session.
pub fn session_path(id: &str, suffix: &str) -> Option<std::path::PathBuf> {
    use sha2::{Digest, Sha256};
    let key = format!("{:x}", Sha256::digest(id.as_bytes()));
    crate::settings::setting_path(&format!("sessions/{key}.{suffix}.json"))
}
