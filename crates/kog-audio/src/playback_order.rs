//! Native adapter for the platform-neutral playback-order policy.

pub use kog_playback_policy::{PlaybackOrder, SelectionState, TrackOrderInfo};
pub use kog_playback_policy::sort;

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
