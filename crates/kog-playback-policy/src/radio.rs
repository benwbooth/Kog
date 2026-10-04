//! Client-side radio lifecycle, shared by native, browser, and mobile players.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const READY_TARGET: usize = 10;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RadioBuffer<T> {
    ready: VecDeque<T>,
    enabled: bool,
    waiting: bool,
    pending: bool,
    exhausted: bool,
    generation: u64,
}

impl<T> Default for RadioBuffer<T> {
    fn default() -> Self {
        Self {
            ready: VecDeque::new(),
            enabled: false,
            waiting: false,
            pending: false,
            exhausted: false,
            generation: 0,
        }
    }
}

impl<T> RadioBuffer<T> {
    /// Toggling, reshuffling, and changing root invalidate pending responses.
    /// None of them starts playback. Only an explicit transport command or
    /// end-of-stream can call `request_next` and arm a pending start.
    pub fn reset(&mut self, enabled: bool) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.ready.clear();
        self.enabled = enabled;
        self.waiting = false;
        self.pending = false;
        self.exhausted = false;
        self.generation
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn waiting(&self) -> bool {
        self.waiting
    }
    pub fn pending(&self) -> bool {
        self.pending
    }
    pub fn exhausted(&self) -> bool {
        self.exhausted
    }
    pub fn ready_len(&self) -> usize {
        self.ready.len()
    }

    pub fn needs_refill(&self) -> bool {
        self.enabled && !self.pending && !self.exhausted && self.ready.len() < READY_TARGET
    }

    pub fn begin_request(&mut self) -> u64 {
        self.pending = true;
        self.generation
    }

    pub fn accept(
        &mut self,
        generation: u64,
        entries: impl IntoIterator<Item = T>,
        exhausted: bool,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        self.pending = false;
        if self.enabled {
            self.ready.extend(entries);
            self.exhausted = exhausted;
            if self.exhausted && self.ready.is_empty() {
                self.waiting = false;
            }
        }
        true
    }

    pub fn fail(&mut self, generation: u64) -> bool {
        self.accept(generation, std::iter::empty(), true)
    }

    pub fn cancel_waiting(&mut self) {
        self.waiting = false;
    }

    pub fn request_next(&mut self) -> Option<T> {
        if !self.enabled {
            return None;
        }
        let result = self.ready.pop_front();
        self.waiting = result.is_none() && !self.exhausted;
        result
    }

    pub fn take_pending(&mut self) -> Option<T> {
        if self.waiting {
            self.request_next()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_and_reshuffle_stage_without_autoplay() {
        let mut radio = RadioBuffer::default();
        let generation = radio.reset(true);
        radio.begin_request();
        radio.accept(generation, [1, 2], false);
        assert_eq!(radio.take_pending(), None);
        assert_eq!(radio.request_next(), Some(1));
        radio.reset(true);
        assert!(!radio.waiting());
        assert_eq!(radio.ready_len(), 0);
    }

    #[test]
    fn an_empty_buffer_waits_for_next_even_with_existing_playlist_history() {
        let mut radio = RadioBuffer::default();
        let generation = radio.reset(true);
        assert_eq!(radio.request_next(), None);
        assert!(radio.waiting());
        radio.begin_request();
        radio.accept(generation, [9], false);
        assert_eq!(radio.take_pending(), Some(9));
        assert!(!radio.waiting());
        assert!(radio.needs_refill());
    }

    #[test]
    fn cancellation_and_old_responses_cannot_restart_playback() {
        let mut radio = RadioBuffer::default();
        let old = radio.reset(true);
        radio.request_next();
        let current = radio.reset(true);
        radio.begin_request();
        assert!(!radio.accept(old, [1], false));
        assert!(radio.pending());
        radio.request_next();
        radio.cancel_waiting();
        radio.accept(current, [2], false);
        assert_eq!(radio.take_pending(), None);
        assert_eq!(radio.request_next(), Some(2));
    }

    #[test]
    fn refill_is_bounded_and_small_library_revisits_are_preserved() {
        let mut radio = RadioBuffer::default();
        let generation = radio.reset(true);
        radio.accept(generation, [7; READY_TARGET], false);
        assert!(!radio.needs_refill());
        assert_eq!(radio.request_next(), Some(7));
        assert!(radio.needs_refill());
        radio.begin_request();
        assert!(!radio.needs_refill());
        radio.accept(generation, [7], false);
        assert_eq!(radio.ready_len(), READY_TARGET);
        radio.reset(false);
        assert!(!radio.needs_refill());
        assert_eq!(radio.request_next(), None);
    }
}
