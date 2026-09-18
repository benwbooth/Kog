//! Pull decoded PCM out of the engine for the streaming server.
//!
//! Kog's backends push their output into a rodio `Player` rather than handing
//! back a source, so the obvious approach would be to refactor ~25 decoders
//! into a pull API. Instead this reuses rodio's own mixing path: a `Player`
//! connected to a private `Mixer` feeds decoded audio into it, and the mixer's
//! output side is a normal `Source` we pull. That keeps every backend as-is
//! and gives us one uniform format to encode: 48 kHz stereo f32, which is also
//! what Opus wants and what AAC and FLAC accept.

use std::io::Read;
use std::num::{NonZeroU16, NonZeroU32};

use rodio::Player;
use rodio::mixer::{MixerSource, mixer};

use crate::decoder::{DecoderRegistry, DecoderSettings, PlaybackSource};

/// Streaming format. Fixed so every encoder gets one input shape; rodio's
/// mixer resamples and remixes each backend's native rate and channel count.
pub const STREAM_SAMPLE_RATE: u32 = 48_000;
pub const STREAM_CHANNELS: u16 = 2;
/// Samples pulled per refill. Bounds the work done inside a single `read`.
const PULL_BATCH: usize = 2_048;

/// Read-adapter over one decoded track, yielding interleaved little-endian
/// f32 samples at [`STREAM_SAMPLE_RATE`] / [`STREAM_CHANNELS`].
///
/// The player, mixer and registry are held for as long as the reader lives:
/// dropping the player would stop feeding the mixer, and dropping the registry
/// would tear down the decoder the mixer is pulling from.
pub struct PcmReader {
    source: MixerSource,
    _player: Player,
    _registry: Option<DecoderRegistry>,
    pending: Vec<u8>,
    position: usize,
    finished: bool,
}

impl PcmReader {
    /// Open a track from the library for streaming. Decoding starts lazily as
    /// the reader is polled, so opening is quick even for emulator backends.
    pub fn open(source: PlaybackSource, settings: DecoderSettings) -> Result<Self, String> {
        let registry = DecoderRegistry::new(settings);
        let (mixer_input, mixer_output) = stream_mixer();
        let player = Player::connect_new(&mixer_input);
        registry.append(&source, &player)?;
        Ok(Self {
            source: mixer_output,
            _player: player,
            _registry: Some(registry),
            pending: Vec::with_capacity(PULL_BATCH * 4),
            position: 0,
            finished: false,
        })
    }

    /// Wrap an already-decoded rodio source. Used by tests, and by callers that
    /// obtained a source from somewhere other than the registry.
    pub fn from_rodio_source<S>(source: S) -> Self
    where
        S: rodio::Source<Item = f32> + Send + 'static,
    {
        let (mixer_input, mixer_output) = stream_mixer();
        mixer_input.add(source);
        Self {
            source: mixer_output,
            _player: Player::connect_new(&mixer_input),
            _registry: None,
            pending: Vec::with_capacity(PULL_BATCH * 4),
            position: 0,
            finished: false,
        }
    }

    pub const fn sample_rate() -> u32 {
        STREAM_SAMPLE_RATE
    }

    pub const fn channels() -> u16 {
        STREAM_CHANNELS
    }

    fn refill(&mut self) {
        self.pending.clear();
        self.position = 0;
        for _ in 0..PULL_BATCH {
            match self.source.next() {
                Some(sample) => self.pending.extend_from_slice(&sample.to_le_bytes()),
                None => {
                    self.finished = true;
                    break;
                }
            }
        }
    }
}

fn stream_mixer() -> (rodio::mixer::Mixer, MixerSource) {
    let channels = NonZeroU16::new(STREAM_CHANNELS).expect("stream channel count is non-zero");
    let sample_rate = NonZeroU32::new(STREAM_SAMPLE_RATE).expect("stream sample rate is non-zero");
    mixer(channels, sample_rate)
}

impl Read for PcmReader {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if out.is_empty() {
            return Ok(0);
        }
        while self.position >= self.pending.len() && !self.finished {
            self.refill();
        }
        let available = &self.pending[self.position..];
        let count = available.len().min(out.len());
        out[..count].copy_from_slice(&available[..count]);
        self.position += count;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A source that yields a known ramp, so the byte conversion and the
    /// mixer's uniformization can be checked exactly.
    struct Ramp {
        remaining: usize,
        next: f32,
    }

    impl Iterator for Ramp {
        type Item = f32;
        fn next(&mut self) -> Option<f32> {
            if self.remaining == 0 {
                return None;
            }
            self.remaining -= 1;
            let value = self.next;
            self.next += 1.0;
            Some(value)
        }
    }

    impl rodio::Source for Ramp {
        fn current_span_len(&self) -> Option<usize> {
            Some(self.remaining)
        }
        fn channels(&self) -> NonZeroU16 {
            NonZeroU16::new(STREAM_CHANNELS).unwrap()
        }
        fn sample_rate(&self) -> NonZeroU32 {
            NonZeroU32::new(STREAM_SAMPLE_RATE).unwrap()
        }
        fn total_duration(&self) -> Option<std::time::Duration> {
            None
        }
    }

    #[test]
    fn pcm_reader_yields_little_endian_f32_and_then_eof() {
        let mut reader = PcmReader::from_rodio_source(Ramp {
            remaining: 8,
            next: 0.5,
        });
        let mut bytes = Vec::new();
        assert_eq!(reader.sample_rate(), STREAM_SAMPLE_RATE);
        assert_eq!(reader.channels(), STREAM_CHANNELS);
        reader.read_to_end(&mut bytes).expect("read pcm");
        assert_eq!(bytes.len(), 8 * 4, "one f32 per sample");
        let samples: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
            .collect();
        assert_eq!(samples, [0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5]);
    }

    #[test]
    fn an_empty_source_reads_zero_and_does_not_hang() {
        let mut reader = PcmReader::from_rodio_source(Ramp {
            remaining: 0,
            next: 0.0,
        });
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).expect("read pcm");
        assert!(bytes.is_empty());
    }

    #[test]
    fn reads_respect_the_output_buffer_length() {
        let mut reader = PcmReader::from_rodio_source(Ramp {
            remaining: 4,
            next: 0.0,
        });
        let mut small = [0_u8; 5];
        let mut total = 0;
        loop {
            let count = reader.read(&mut small).expect("read");
            if count == 0 {
                break;
            }
            total += count;
            assert!(count <= small.len());
        }
        assert_eq!(total, 16, "four f32 samples are still delivered in full");
    }
}
