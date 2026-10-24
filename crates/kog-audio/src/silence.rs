//! Chiptune rips often open with silence: the driver boots, waits out a
//! game's fade-in timer, or the song simply begins on a rest. Every
//! emulated format starts playing at its first sound instead.

use std::collections::VecDeque;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

/// Below this a sample is silent (about -80 dBFS).
const SILENT: f32 = 0.000_1;
/// Never skip more than this: a song that stays silent longer is quiet on
/// purpose, or not a song.
const LONGEST_SKIP: Duration = Duration::from_secs(30);

/// Append a decoded song that starts at its first sound.
pub fn append_trimmed<S>(player: &Player, source: S)
where
    S: Source + Send + 'static,
{
    player.append(SkipLeadingSilence::new(source));
}

pub struct SkipLeadingSilence<S> {
    inner: S,
    /// Samples read ahead while looking for sound, played once found.
    pending: VecDeque<f32>,
    /// Frames of silence dropped from the start; positions and seeks are
    /// offset by this so the song's timeline starts at its first sound.
    skipped_frames: u64,
    searched: bool,
}

impl<S: Source> SkipLeadingSilence<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            pending: VecDeque::new(),
            skipped_frames: 0,
            searched: false,
        }
    }

    fn skipped(&self) -> Duration {
        let rate = u64::from(self.inner.sample_rate().get());
        Duration::from_nanos(self.skipped_frames.saturating_mul(1_000_000_000) / rate.max(1))
    }

    /// Read whole frames until one has sound, keeping that frame.
    fn search(&mut self) {
        self.searched = true;
        let channels = usize::from(self.inner.channels().get()).max(1);
        let rate = u64::from(self.inner.sample_rate().get());
        let limit = LONGEST_SKIP.as_secs() * rate;
        let mut frame = Vec::with_capacity(channels);
        loop {
            frame.clear();
            for _ in 0..channels {
                match self.inner.next() {
                    Some(sample) => frame.push(sample),
                    None => break,
                }
            }
            if frame.len() < channels || frame.iter().any(|sample| sample.abs() >= SILENT) {
                self.pending.extend(frame.iter().copied());
                return;
            }
            self.skipped_frames += 1;
            if self.skipped_frames >= limit {
                // Nothing yet: play it as it is from here.
                self.pending.extend(frame.iter().copied());
                return;
            }
        }
    }
}

impl<S: Source> Iterator for SkipLeadingSilence<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if !self.searched {
            self.search();
        }
        self.pending.pop_front().or_else(|| self.inner.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.pending.len(), None)
    }
}

impl<S: Source> Source for SkipLeadingSilence<S> {
    fn current_span_len(&self) -> Option<usize> {
        if self.pending.is_empty() {
            self.inner.current_span_len()
        } else {
            None
        }
    }

    fn channels(&self) -> ChannelCount {
        self.inner.channels()
    }

    fn sample_rate(&self) -> SampleRate {
        self.inner.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.inner.total_duration().map(|total| total.saturating_sub(self.skipped()))
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        // A song resumed partway through plays from there, untrimmed.
        self.searched = true;
        self.pending.clear();
        self.inner.try_seek(position + self.skipped())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rodio::buffer::SamplesBuffer;
    use std::num::NonZero;

    #[test]
    fn playback_starts_at_the_first_sound_and_seeks_follow_it() {
        let rate = 1_000;
        let mut samples = vec![0.0_f32; 2 * 1_500];
        samples.extend([0.5, -0.5, 0.25, -0.25]);
        let source = SamplesBuffer::new(NonZero::new(2).unwrap(), NonZero::new(rate).unwrap(), samples);
        let mut trimmed = SkipLeadingSilence::new(source);
        assert_eq!(trimmed.next(), Some(0.5));
        assert_eq!(trimmed.next(), Some(-0.5));
        assert_eq!(trimmed.skipped(), Duration::from_millis(1_500));
        assert_eq!(trimmed.total_duration(), Some(Duration::from_millis(2)));
        trimmed.try_seek(Duration::from_millis(1)).unwrap();
        assert_eq!(trimmed.next(), Some(0.25));
    }

    #[test]
    fn a_song_that_starts_with_sound_is_untouched() {
        let source = SamplesBuffer::new(NonZero::new(1).unwrap(), NonZero::new(1_000).unwrap(), vec![0.3, 0.0, 0.2]);
        let collected: Vec<f32> = SkipLeadingSilence::new(source).collect();
        assert_eq!(collected, vec![0.3, 0.0, 0.2]);
    }
}
