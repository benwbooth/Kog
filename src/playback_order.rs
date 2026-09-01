use std::collections::HashSet;

use crate::settings::{RepeatMode, ShuffleMode};
use crate::track::Track;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionState {
    None,
    Mixed,
    All,
}

#[derive(Debug)]
pub struct PlaybackOrder {
    shuffle_mode: ShuffleMode,
    repeat_mode: RepeatMode,
    shuffle_order: Vec<usize>,
    queue: Vec<usize>,
    stop_after: HashSet<usize>,
    seed: u64,
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
        }
    }

    pub const fn shuffle_mode(&self) -> ShuffleMode {
        self.shuffle_mode
    }

    pub const fn repeat_mode(&self) -> RepeatMode {
        self.repeat_mode
    }

    pub fn set_shuffle_mode(
        &mut self,
        mode: ShuffleMode,
        tracks: &[Track],
        current: Option<usize>,
    ) {
        self.shuffle_mode = mode;
        self.reset_shuffle_order(tracks, current);
    }

    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.repeat_mode = mode;
    }

    pub fn tracks_changed(&mut self, tracks: &[Track], current: Option<usize>) {
        self.queue.retain(|index| *index < tracks.len());
        self.stop_after.retain(|index| *index < tracks.len());
        self.reset_shuffle_order(tracks, current);
    }

    pub fn clear_tracks(&mut self) {
        self.shuffle_order.clear();
        self.queue.clear();
        self.stop_after.clear();
    }

    pub fn remap_tracks(&mut self, old_to_new: &[Option<usize>]) {
        self.queue = remap_indices(&self.queue, old_to_new);
        self.stop_after = self
            .stop_after
            .iter()
            .filter_map(|index| old_to_new.get(*index).copied().flatten())
            .collect();
        self.shuffle_order.clear();
    }

    pub fn next(
        &mut self,
        tracks: &[Track],
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

    pub fn previous(&mut self, tracks: &[Track], current: Option<usize>) -> Option<usize> {
        if tracks.is_empty() {
            return None;
        }
        match self.shuffle_mode {
            ShuffleMode::Off => match current.filter(|index| *index < tracks.len()) {
                Some(index) if index > 0 => Some(index - 1),
                Some(index) if self.repeat_mode != RepeatMode::All => Some(index),
                Some(_) => Some(tracks.len() - 1),
                None => Some(0),
            },
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

    fn next_in_playlist(&self, tracks: &[Track], current: Option<usize>) -> Option<usize> {
        let Some(current) = current.filter(|index| *index < tracks.len()) else {
            return Some(0);
        };
        let next = current + 1;
        if self.repeat_mode == RepeatMode::Album {
            let album = &tracks[current].album;
            if next < tracks.len() && tracks[next].album.eq_ignore_ascii_case(album) {
                return Some(next);
            }
            return tracks
                .iter()
                .position(|track| track.album.eq_ignore_ascii_case(album));
        }
        if next < tracks.len() {
            Some(next)
        } else if self.repeat_mode == RepeatMode::All {
            Some(0)
        } else {
            None
        }
    }

    fn next_shuffled(&mut self, tracks: &[Track], current: Option<usize>) -> Option<usize> {
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

    fn previous_shuffled(&mut self, tracks: &[Track], current: Option<usize>) -> Option<usize> {
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

    fn ensure_shuffle_order(&mut self, tracks: &[Track], current: Option<usize>) {
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

    fn reset_shuffle_order(&mut self, tracks: &[Track], current: Option<usize>) {
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
                let album = &tracks[current].album;
                let (mut current_album, remainder): (Vec<_>, Vec<_>) = self
                    .shuffle_order
                    .drain(..)
                    .partition(|index| tracks[*index].album.eq_ignore_ascii_case(album));
                current_album.extend(remainder);
                self.shuffle_order = current_album;
            }
        }
    }

    fn build_shuffle_order(&mut self, tracks: &[Track]) -> Vec<usize> {
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

fn shuffled_album_indices(tracks: &[Track], seed: &mut u64) -> Vec<usize> {
    let mut albums: Vec<(String, Vec<usize>)> = Vec::new();
    for (index, track) in tracks.iter().enumerate() {
        if let Some((_, indices)) = albums
            .iter_mut()
            .find(|(album, _)| album.eq_ignore_ascii_case(&track.album))
        {
            indices.push(index);
        } else {
            albums.push((track.album.clone(), vec![index]));
        }
    }
    for (_, indices) in &mut albums {
        indices.sort_by_key(|index| {
            let track = &tracks[*index];
            (track.disc_number, track.track_number, *index)
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

    fn track(album: &str, disc: u32, number: u32) -> Track {
        Track {
            album: album.to_owned(),
            disc_number: Some(disc),
            track_number: Some(number),
            ..Track::default()
        }
    }

    #[test]
    fn repeat_one_precedes_queue_but_manual_next_consumes_it() {
        let tracks = vec![Track::default(), Track::default(), Track::default()];
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
        let tracks = vec![Track::default(), Track::default()];
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
