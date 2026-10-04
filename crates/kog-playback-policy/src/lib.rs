//! Shared queue, shuffle, and repeat policy for native and browser players.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub mod bridge;
pub mod radio;
pub mod sort;
pub mod workspace;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShuffleMode {
    #[default]
    Off,
    Albums,
    All,
}

impl ShuffleMode {
    pub const fn setting_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Albums => "albums",
            Self::All => "all",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::Albums,
            Self::Albums => Self::All,
            Self::All => Self::Off,
        }
    }

    pub fn from_setting(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "0" => Some(Self::Off),
            "albums" | "album" | "1" => Some(Self::Albums),
            "all" | "tracks" | "songs" | "2" => Some(Self::All),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    #[default]
    Off,
    One,
    Album,
    All,
}

impl RepeatMode {
    pub const fn setting_value(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::One => "one",
            Self::Album => "album",
            Self::All => "all",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::One,
            Self::One => Self::Album,
            Self::Album => Self::All,
            Self::All => Self::Off,
        }
    }

    pub fn from_setting(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "off" | "none" | "0" => Some(Self::Off),
            "one" | "track" | "1" => Some(Self::One),
            "album" | "2" => Some(Self::Album),
            "all" | "playlist" | "3" => Some(Self::All),
            _ => None,
        }
    }
}

/// Metadata needed to order a queue. Audio decoding and browser transport
/// remain outside this platform-neutral policy crate.
pub trait TrackOrderInfo {
    fn album(&self) -> &str;
    fn disc_number(&self) -> Option<u32>;
    fn track_number(&self) -> Option<u32>;
}

/// Match retained rows by identity and occurrence, including duplicate songs.
/// Callers with an explicit move/removal map can pass that exact map instead.
pub fn remap_track_ids(previous: &[String], next: &[String]) -> Vec<Option<usize>> {
    let mut used = HashSet::new();
    previous
        .iter()
        .map(|old| {
            let index = next
                .iter()
                .enumerate()
                .find(|(index, id)| !used.contains(index) && *id == old)
                .map(|(index, _)| index);
            if let Some(index) = index {
                used.insert(index);
            }
            index
        })
        .collect()
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OrderTrack {
    pub album: String,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
}

impl TrackOrderInfo for OrderTrack {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionState {
    None,
    Mixed,
    All,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlaybackOrder {
    shuffle_mode: ShuffleMode,
    repeat_mode: RepeatMode,
    shuffle_order: Vec<usize>,
    queue: Vec<usize>,
    stop_after: HashSet<usize>,
    seed: u64,
    #[serde(default)]
    radio_enabled: bool,
    #[serde(default)]
    navigation: Option<Navigation>,
    #[serde(default)]
    sequence: Vec<usize>,
    #[serde(default)]
    original: Vec<usize>,
}

/// Commands have the same meaning for buttons, media keys, and end-of-stream
/// callbacks. In particular, manual Next ignores Repeat One.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationEvent {
    Next,
    Previous,
    Ended,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", content = "index", rename_all = "snake_case")]
pub enum PlaybackDecision {
    Play(usize),
    Radio,
    Stop,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Navigation {
    cursor: Option<usize>,
    previous: bool,
    repeat_one: bool,
    attempted: HashSet<usize>,
}

impl PlaybackOrder {
    pub fn new(shuffle_mode: ShuffleMode, repeat_mode: RepeatMode, seed: u64) -> Self {
        Self {
            shuffle_mode,
            repeat_mode,
            shuffle_order: Vec::new(),
            queue: Vec::new(),
            stop_after: HashSet::new(),
            seed,
            radio_enabled: false,
            navigation: None,
            sequence: Vec::new(),
            original: Vec::new(),
        }
    }

    pub const fn shuffle_mode(&self) -> ShuffleMode {
        self.shuffle_mode
    }

    pub const fn repeat_mode(&self) -> RepeatMode {
        self.repeat_mode
    }

    pub fn set_shuffle_mode<T: TrackOrderInfo>(
        &mut self,
        mode: ShuffleMode,
        tracks: &[T],
        current: Option<usize>,
    ) {
        self.shuffle_mode = mode;
        if mode != ShuffleMode::Off {
            self.radio_enabled = false;
        }
        self.reset_shuffle_order(tracks, current);
    }

    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.repeat_mode = mode;
        if mode != RepeatMode::Off {
            self.radio_enabled = false;
        }
    }

    pub const fn radio_enabled(&self) -> bool {
        self.radio_enabled
    }

    /// Radio supplies its own shuffled order. Selecting repeat or shuffle
    /// turns radio off; enabling radio clears both modes in every frontend.
    pub fn set_radio_enabled<T: TrackOrderInfo>(
        &mut self,
        enabled: bool,
        tracks: &[T],
        current: Option<usize>,
    ) {
        if enabled {
            self.set_repeat_mode(RepeatMode::Off);
            self.set_shuffle_mode(ShuffleMode::Off, tracks, current);
        }
        self.radio_enabled = enabled;
    }

    /// Select a candidate or an end-of-queue action. A failed asynchronous
    /// open feeds `Failed` back into this same state machine, so all clients
    /// skip each broken candidate at most once, including under Repeat One
    /// and Repeat All. Direct row activation calls `cancel_navigation` first.
    pub fn navigate<T: TrackOrderInfo>(
        &mut self,
        tracks: &[T],
        current: Option<usize>,
        event: NavigationEvent,
    ) -> PlaybackDecision {
        if event != NavigationEvent::Failed {
            self.navigation = None;
            if event == NavigationEvent::Ended
                && current.is_some_and(|index| self.should_stop_after(index))
            {
                return PlaybackDecision::Stop;
            }
            self.navigation = Some(Navigation {
                cursor: current.filter(|index| *index < tracks.len()),
                previous: event == NavigationEvent::Previous,
                repeat_one: event == NavigationEvent::Ended,
                attempted: HashSet::new(),
            });
        }
        let Some(mut navigation) = self.navigation.take() else {
            return PlaybackDecision::Stop;
        };
        // Invalid queue entries and repeat wraparound cannot spin forever.
        let budget = tracks
            .len()
            .saturating_add(self.queue.len())
            .saturating_add(1);
        for _ in 0..budget {
            let next = if navigation.previous {
                self.previous(tracks, navigation.cursor)
            } else {
                self.next(tracks, navigation.cursor, navigation.repeat_one)
            };
            navigation.repeat_one = false;
            let Some(next) = next.filter(|index| *index < tracks.len()) else {
                break;
            };
            navigation.cursor = Some(next);
            if navigation.attempted.insert(next) {
                self.navigation = Some(navigation);
                return PlaybackDecision::Play(next);
            }
        }
        if self.radio_enabled && !navigation.previous {
            PlaybackDecision::Radio
        } else {
            PlaybackDecision::Stop
        }
    }

    pub fn cancel_navigation(&mut self) {
        self.navigation = None;
    }

    /// A radio candidate is already selected by the shared radio service.
    /// Keep its failure in the same traversal as a normal Next candidate.
    pub fn radio_candidate(&mut self, index: usize) {
        self.navigation = Some(Navigation {
            cursor: Some(index),
            previous: false,
            repeat_one: false,
            attempted: HashSet::from([index]),
        });
    }

    pub fn started(&mut self, previous: Option<usize>, index: usize) {
        self.cancel_navigation();
        self.clear_stop_after_when_leaving(previous, index);
    }

    pub fn queued_indices(&self) -> &[usize] {
        &self.queue
    }

    pub fn stop_after_indices(&self) -> impl Iterator<Item = usize> + '_ {
        self.stop_after.iter().copied()
    }

    pub fn tracks_changed<T: TrackOrderInfo>(&mut self, tracks: &[T], current: Option<usize>) {
        self.original.retain(|index| *index < tracks.len());
        for index in 0..tracks.len() {
            if !self.original.contains(&index) {
                self.original.push(index);
            }
        }
        if !self.sequence.is_empty() {
            self.set_sequence(self.sequence.clone(), tracks.len());
        }
        self.queue.retain(|index| *index < tracks.len());
        self.stop_after.retain(|index| *index < tracks.len());
        self.reset_shuffle_order(tracks, current);
    }

    /// Rebuild the unplayed part of album shuffle after tags arrive. The
    /// played prefix must stay in place or a late metadata probe can replay a
    /// song that was already heard in this shuffle round.
    pub fn album_metadata_changed<T: TrackOrderInfo>(
        &mut self,
        tracks: &[T],
        current: Option<usize>,
    ) {
        if self.shuffle_mode != ShuffleMode::Albums {
            return;
        }
        self.ensure_shuffle_order(tracks, current);
        let Some(position) = current.and_then(|current| {
            self.shuffle_order
                .iter()
                .position(|index| *index == current)
        }) else {
            self.reset_shuffle_order(tracks, current);
            return;
        };
        let played: HashSet<_> = self.shuffle_order[..=position].iter().copied().collect();
        let album = tracks[self.shuffle_order[position]].album();
        let mut same_album: Vec<_> = (0..tracks.len())
            .filter(|index| {
                !played.contains(index) && tracks[*index].album().eq_ignore_ascii_case(album)
            })
            .collect();
        same_album.sort_by_key(|index| {
            let track = &tracks[*index];
            (track.disc_number(), track.track_number(), *index)
        });
        let mut next = self.shuffle_order[..=position].to_vec();
        next.extend(same_album);
        let scheduled: HashSet<_> = next.iter().copied().collect();
        next.extend(
            self.build_shuffle_order(tracks)
                .into_iter()
                .filter(|index| !scheduled.contains(index)),
        );
        self.shuffle_order = next;
    }

    pub fn clear_tracks(&mut self) {
        self.cancel_navigation();
        self.shuffle_order.clear();
        self.queue.clear();
        self.stop_after.clear();
        self.sequence.clear();
        self.original.clear();
    }

    pub fn remap_tracks(&mut self, old_to_new: &[Option<usize>]) {
        self.cancel_navigation();
        self.queue = remap_indices(&self.queue, old_to_new);
        self.stop_after = self
            .stop_after
            .iter()
            .filter_map(|index| old_to_new.get(*index).copied().flatten())
            .collect();
        self.shuffle_order.clear();
        self.sequence = remap_indices(&self.sequence, old_to_new);
        self.original = remap_indices(&self.original, old_to_new);
    }

    /// A sorted projection can retain stable row identities while navigating
    /// in the displayed order. An empty sequence means the physical order.
    pub fn set_sequence(&mut self, mut sequence: Vec<usize>, count: usize) {
        if sequence.is_empty() {
            self.sequence.clear();
            return;
        }
        let mut used = HashSet::new();
        sequence.retain(|index| *index < count && used.insert(*index));
        sequence.extend((0..count).filter(|index| !used.contains(index)));
        self.sequence = sequence;
    }

    pub fn original_position(&self, index: usize) -> usize {
        self.original
            .iter()
            .position(|old| *old == index)
            .unwrap_or(index)
    }

    fn sequence(&self, count: usize) -> Vec<usize> {
        if self.sequence.len() == count {
            self.sequence.clone()
        } else {
            (0..count).collect()
        }
    }

    pub fn next<T: TrackOrderInfo>(
        &mut self,
        tracks: &[T],
        current: Option<usize>,
        honor_repeat_one: bool,
    ) -> Option<usize> {
        if tracks.is_empty() {
            return None;
        }
        if honor_repeat_one && self.repeat_mode == RepeatMode::One {
            return current.filter(|index| *index < tracks.len());
        }
        if !self.queue.is_empty() {
            return Some(self.queue.remove(0));
        }

        match self.shuffle_mode {
            ShuffleMode::Off => self.next_in_playlist(tracks, current),
            ShuffleMode::Albums | ShuffleMode::All => self.next_shuffled(tracks, current),
        }
    }

    pub fn previous<T: TrackOrderInfo>(
        &mut self,
        tracks: &[T],
        current: Option<usize>,
    ) -> Option<usize> {
        if tracks.is_empty() {
            return None;
        }
        match self.shuffle_mode {
            ShuffleMode::Off => {
                let sequence = self.sequence(tracks.len());
                let position =
                    current.and_then(|current| sequence.iter().position(|index| *index == current));
                match position {
                    Some(position) if position > 0 => sequence.get(position - 1).copied(),
                    Some(_) if self.repeat_mode == RepeatMode::All => sequence.last().copied(),
                    Some(_) => current,
                    None => sequence.first().copied(),
                }
            }
            ShuffleMode::Albums | ShuffleMode::All => self.previous_shuffled(tracks, current),
        }
    }

    pub fn should_stop_after(&self, index: usize) -> bool {
        self.stop_after.contains(&index)
    }

    pub fn clear_stop_after_when_leaving(&mut self, previous: Option<usize>, next: usize) {
        if let Some(previous) = previous
            && previous != next
        {
            self.stop_after.remove(&previous);
        }
    }

    pub fn toggle_queue(&mut self, indices: &[usize]) {
        for &index in indices {
            if let Some(position) = self.queue.iter().position(|queued| *queued == index) {
                self.queue.remove(position);
            } else {
                self.queue.push(index);
            }
        }
    }

    /// Explicit Play Next places a stable selection before earlier overrides.
    pub fn queue_next(&mut self, indices: &[usize]) {
        let mut next = Vec::new();
        for &index in indices {
            if !next.contains(&index) { next.push(index); }
        }
        next.extend(self.queue.iter().copied().filter(|index| !indices.contains(index)));
        self.queue = next;
    }

    /// Execute a workspace action after the adapter has appended its resolved
    /// entries. The actual added range can include expanded archive/subsong rows.
    pub fn apply_queue_action(&mut self, action: workspace::QueueAction, start: usize, count: usize) -> Option<PlaybackDecision> {
        if count == 0 { return None; }
        match action {
            workspace::QueueAction::PlayNow => { self.cancel_navigation(); Some(PlaybackDecision::Play(start)) }
            workspace::QueueAction::PlayNext => { self.queue_next(&(start..start.saturating_add(count)).collect::<Vec<_>>()); None }
            workspace::QueueAction::AddToQueue => None,
        }
    }

    pub fn clear_queue(&mut self) {
        self.queue.clear();
    }

    pub fn queue_count(&self) -> usize {
        self.queue.len()
    }

    pub fn queue_position(&self, index: usize) -> Option<usize> {
        self.queue.iter().position(|queued| *queued == index)
    }

    pub fn queue_selection_state(&self, indices: &[usize]) -> SelectionState {
        selection_state(indices, |index| self.queue.contains(&index))
    }

    pub fn toggle_stop_after(&mut self, indices: &[usize]) {
        for &index in indices {
            if !self.stop_after.remove(&index) {
                self.stop_after.insert(index);
            }
        }
    }

    pub fn stop_after_selection_state(&self, indices: &[usize]) -> SelectionState {
        selection_state(indices, |index| self.stop_after.contains(&index))
    }

    fn next_in_playlist<T: TrackOrderInfo>(
        &self,
        tracks: &[T],
        current: Option<usize>,
    ) -> Option<usize> {
        let sequence = self.sequence(tracks.len());
        let Some(current) = current.filter(|index| *index < tracks.len()) else {
            return sequence.first().copied();
        };
        let next = sequence
            .iter()
            .position(|index| *index == current)
            .and_then(|position| sequence.get(position + 1))
            .copied();
        if self.repeat_mode == RepeatMode::Album {
            let album = tracks[current].album();
            if next.is_some_and(|next| tracks[next].album().eq_ignore_ascii_case(album)) {
                return next;
            }
            return sequence
                .into_iter()
                .find(|index| tracks[*index].album().eq_ignore_ascii_case(album));
        }
        if next.is_some() {
            next
        } else if self.repeat_mode == RepeatMode::All {
            sequence.first().copied()
        } else {
            None
        }
    }

    fn next_shuffled<T: TrackOrderInfo>(
        &mut self,
        tracks: &[T],
        current: Option<usize>,
    ) -> Option<usize> {
        self.ensure_shuffle_order(tracks, current);
        let position = current.and_then(|current| {
            self.shuffle_order
                .iter()
                .position(|index| *index == current)
        });
        if let Some(next) = position.and_then(|position| self.shuffle_order.get(position + 1)) {
            return Some(*next);
        }
        if position.is_none() {
            return self.shuffle_order.first().copied();
        }
        if self.repeat_mode != RepeatMode::All {
            return None;
        }

        let previous = current;
        self.shuffle_order = self.build_shuffle_order(tracks);
        if tracks.len() > 1 && self.shuffle_order.first().copied() == previous {
            self.shuffle_order.swap(0, 1);
        }
        self.shuffle_order.first().copied()
    }

    fn previous_shuffled<T: TrackOrderInfo>(
        &mut self,
        tracks: &[T],
        current: Option<usize>,
    ) -> Option<usize> {
        self.ensure_shuffle_order(tracks, current);
        let position = current.and_then(|current| {
            self.shuffle_order
                .iter()
                .position(|index| *index == current)
        });
        match position {
            Some(position) if position > 0 => self.shuffle_order.get(position - 1).copied(),
            Some(_) if self.repeat_mode == RepeatMode::All => {
                self.shuffle_order = self.build_shuffle_order(tracks);
                self.shuffle_order.last().copied()
            }
            Some(_) => current,
            None => self.shuffle_order.first().copied(),
        }
    }

    fn ensure_shuffle_order<T: TrackOrderInfo>(&mut self, tracks: &[T], current: Option<usize>) {
        let mut unique = HashSet::with_capacity(self.shuffle_order.len());
        let valid = self.shuffle_order.len() == tracks.len()
            && self
                .shuffle_order
                .iter()
                .all(|index| *index < tracks.len() && unique.insert(*index));
        if !valid {
            self.reset_shuffle_order(tracks, current);
        }
    }

    fn reset_shuffle_order<T: TrackOrderInfo>(&mut self, tracks: &[T], current: Option<usize>) {
        if self.shuffle_mode == ShuffleMode::Off {
            self.shuffle_order.clear();
            return;
        }
        self.shuffle_order = self.build_shuffle_order(tracks);
        let Some(current) = current.filter(|index| *index < tracks.len()) else {
            return;
        };
        match self.shuffle_mode {
            ShuffleMode::Off => {}
            ShuffleMode::All => {
                if let Some(position) = self
                    .shuffle_order
                    .iter()
                    .position(|index| *index == current)
                {
                    self.shuffle_order.swap(0, position);
                }
            }
            ShuffleMode::Albums => {
                let album = tracks[current].album();
                let (mut current_album, remainder): (Vec<_>, Vec<_>) = self
                    .shuffle_order
                    .drain(..)
                    .partition(|index| tracks[*index].album().eq_ignore_ascii_case(album));
                current_album.extend(remainder);
                self.shuffle_order = current_album;
            }
        }
    }

    fn build_shuffle_order<T: TrackOrderInfo>(&mut self, tracks: &[T]) -> Vec<usize> {
        match self.shuffle_mode {
            ShuffleMode::Off => Vec::new(),
            ShuffleMode::All => shuffled_indices(tracks.len(), &mut self.seed),
            ShuffleMode::Albums => shuffled_album_indices(tracks, &mut self.seed),
        }
    }
}

fn selection_state(indices: &[usize], mut selected: impl FnMut(usize) -> bool) -> SelectionState {
    if indices.is_empty() {
        return SelectionState::None;
    }
    let selected_count = indices.iter().filter(|index| selected(**index)).count();
    if selected_count == 0 {
        SelectionState::None
    } else if selected_count == indices.len() {
        SelectionState::All
    } else {
        SelectionState::Mixed
    }
}

fn remap_indices(indices: &[usize], old_to_new: &[Option<usize>]) -> Vec<usize> {
    let mut remapped = Vec::with_capacity(indices.len());
    for index in indices {
        if let Some(new_index) = old_to_new.get(*index).copied().flatten()
            && !remapped.contains(&new_index)
        {
            remapped.push(new_index);
        }
    }
    remapped
}

fn shuffled_album_indices<T: TrackOrderInfo>(tracks: &[T], seed: &mut u64) -> Vec<usize> {
    let mut albums: Vec<(String, Vec<usize>)> = Vec::new();
    for (index, track) in tracks.iter().enumerate() {
        if let Some((_, indices)) = albums
            .iter_mut()
            .find(|(album, _)| album.eq_ignore_ascii_case(track.album()))
        {
            indices.push(index);
        } else {
            albums.push((track.album().to_owned(), vec![index]));
        }
    }
    for (_, indices) in &mut albums {
        indices.sort_by_key(|index| {
            let track = &tracks[*index];
            (track.disc_number(), track.track_number(), *index)
        });
    }
    shuffle_slice(&mut albums, seed);
    albums
        .into_iter()
        .flat_map(|(_, indices)| indices)
        .collect()
}

fn shuffled_indices(length: usize, seed: &mut u64) -> Vec<usize> {
    let mut indices = (0..length).collect::<Vec<_>>();
    shuffle_slice(&mut indices, seed);
    indices
}

fn shuffle_slice<T>(values: &mut [T], seed: &mut u64) {
    for index in (1..values.len()).rev() {
        // xorshift64* gives playback order a small dependency-free PRNG.
        if *seed == 0 {
            *seed = 0x9e37_79b9_7f4a_7c15;
        }
        *seed ^= *seed >> 12;
        *seed ^= *seed << 25;
        *seed ^= *seed >> 27;
        let random = seed.wrapping_mul(0x2545_f491_4f6c_dd1d);
        values.swap(index, (random as usize) % (index + 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(album: &str, disc: u32, number: u32) -> OrderTrack {
        OrderTrack {
            album: album.to_owned(),
            disc_number: Some(disc),
            track_number: Some(number),
            ..OrderTrack::default()
        }
    }

    #[test]
    fn repeat_one_precedes_queue_but_manual_next_consumes_it() {
        let tracks = vec![
            OrderTrack::default(),
            OrderTrack::default(),
            OrderTrack::default(),
        ];
        let mut order = PlaybackOrder::new(ShuffleMode::Off, RepeatMode::One, 1);
        order.toggle_queue(&[2]);

        assert_eq!(order.next(&tracks, Some(0), true), Some(0));
        assert_eq!(order.queue_count(), 1);
        assert_eq!(order.next(&tracks, Some(0), false), Some(2));
        assert_eq!(order.queue_count(), 0);
    }

    #[test]
    fn repeat_album_wraps_within_the_matching_album() {
        let tracks = vec![
            track("Alpha", 1, 1),
            track("Alpha", 1, 2),
            track("Beta", 1, 1),
        ];
        let mut order = PlaybackOrder::new(ShuffleMode::Off, RepeatMode::Album, 2);

        assert_eq!(order.next(&tracks, Some(0), true), Some(1));
        assert_eq!(order.next(&tracks, Some(1), true), Some(0));
        assert_eq!(order.next(&tracks, Some(2), true), Some(2));
    }

    #[test]
    fn album_shuffle_keeps_disc_track_order_and_current_album_first() {
        let tracks = vec![
            track("Alpha", 1, 2),
            track("Beta", 1, 1),
            track("Alpha", 1, 1),
            track("Beta", 2, 1),
        ];
        let mut order = PlaybackOrder::new(ShuffleMode::Albums, RepeatMode::Off, 3);
        order.set_shuffle_mode(ShuffleMode::Albums, &tracks, Some(0));

        assert_eq!(order.shuffle_order[..2], [2, 0]);
        assert_eq!(order.shuffle_order[2..], [1, 3]);
        assert_eq!(order.next(&tracks, Some(0), true), Some(1));
    }

    #[test]
    fn late_album_tags_regroup_only_unplayed_shuffle_tracks() {
        let mut tracks = vec![OrderTrack::default(); 5];
        let mut order = PlaybackOrder::new(ShuffleMode::Albums, RepeatMode::Off, 19);
        order.set_shuffle_mode(ShuffleMode::Albums, &tracks, None);
        let first = order.shuffle_order[0];
        let current = order.shuffle_order[1];
        let partner = order.shuffle_order[4];
        for (index, track) in tracks.iter_mut().enumerate() {
            track.album = format!("Album {index}");
        }
        tracks[current].album = "Shared".to_owned();
        tracks[partner].album = "Shared".to_owned();
        tracks[current].track_number = Some(1);
        tracks[partner].track_number = Some(2);

        order.album_metadata_changed(&tracks, Some(current));

        assert_eq!(&order.shuffle_order[..2], &[first, current]);
        assert_eq!(order.next(&tracks, Some(current), false), Some(partner));
        let mut visited = vec![first, current];
        let mut position = current;
        while let Some(next) = order.next(&tracks, Some(position), false) {
            assert!(!visited.contains(&next));
            visited.push(next);
            position = next;
        }
        assert_eq!(visited.len(), tracks.len());
    }

    #[test]
    fn queue_and_stop_after_survive_reordering_and_drop_removed_rows() {
        let mut order = PlaybackOrder::new(ShuffleMode::Off, RepeatMode::Off, 4);
        order.toggle_queue(&[1, 3]);
        order.toggle_stop_after(&[0, 3]);
        order.remap_tracks(&[Some(2), None, Some(0), Some(1)]);

        assert_eq!(order.queue_count(), 1);
        assert_eq!(order.queue_position(1), Some(0));
        assert!(order.should_stop_after(2));
        assert!(order.should_stop_after(1));
        assert!(!order.should_stop_after(0));
    }

    #[test]
    fn repeat_all_wraps_but_other_modes_stop_at_playlist_end() {
        let tracks = vec![OrderTrack::default(), OrderTrack::default()];
        let mut order = PlaybackOrder::new(ShuffleMode::Off, RepeatMode::Off, 5);
        assert_eq!(order.next(&tracks, Some(1), true), None);

        order.set_repeat_mode(RepeatMode::All);
        assert_eq!(order.next(&tracks, Some(1), true), Some(0));
        assert_eq!(order.previous(&tracks, Some(0)), Some(1));
    }

    #[test]
    fn toggles_match_cogs_per_entry_queue_and_stop_after_behavior() {
        let mut order = PlaybackOrder::new(ShuffleMode::Off, RepeatMode::Off, 6);
        order.toggle_queue(&[1, 2]);
        order.toggle_queue(&[2, 3]);
        assert_eq!(order.queue, [1, 3]);
        assert_eq!(order.queue_selection_state(&[1, 2]), SelectionState::Mixed);

        order.toggle_stop_after(&[1, 2]);
        order.toggle_stop_after(&[2, 3]);
        assert!(order.should_stop_after(1));
        assert!(!order.should_stop_after(2));
        assert!(order.should_stop_after(3));
    }

    #[test]
    fn leaving_a_stop_after_track_clears_only_that_tracks_marker() {
        let mut order = PlaybackOrder::new(ShuffleMode::Off, RepeatMode::Off, 7);
        order.toggle_stop_after(&[0, 2]);

        order.clear_stop_after_when_leaving(Some(0), 1);
        assert!(!order.should_stop_after(0));
        assert!(order.should_stop_after(2));

        order.clear_stop_after_when_leaving(Some(2), 2);
        assert!(order.should_stop_after(2));
    }
}
