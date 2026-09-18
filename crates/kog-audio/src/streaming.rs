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
use std::path::Path;

use rodio::Player;
use rodio::mixer::{MixerSource, mixer};

use crate::decoder::{DecoderRegistry, DecoderSettings, PlaybackSource};

/// Streaming format. Fixed so every encoder gets one input shape; rodio's
/// mixer resamples and remixes each backend's native rate and channel count.
pub const STREAM_SAMPLE_RATE: u32 = 48_000;
pub const STREAM_CHANNELS: u16 = 2;
/// Samples pulled per refill. Bounds the work done inside a single `read`.
const PULL_BATCH: usize = 2_048;

/// Upper bound on one streamed track, used when the decoder reports no
/// duration. Six hours of 48 kHz stereo is far beyond any real track and keeps
/// a mislabelled file from streaming silence forever.
const MAX_STREAM_FRAMES: u64 = 6 * 60 * 60 * STREAM_SAMPLE_RATE as u64;

/// Read-adapter over one decoded track, yielding interleaved little-endian
/// f32 samples at [`STREAM_SAMPLE_RATE`] / [`STREAM_CHANNELS`].
///
/// The player, mixer and registry are held for as long as the reader lives:
/// dropping the player would stop feeding the mixer, and dropping the registry
/// would tear down the decoder the mixer is pulling from.
pub struct PcmReader {
    source: MixerSource,
    /// Held so the decoded audio keeps being pushed into the mixer.
    _player: Option<Player>,
    _registry: Option<DecoderRegistry>,
    pending: Vec<u8>,
    position: usize,
    finished: bool,
    /// Frames to deliver before stopping. A rodio player keeps its mixer alive
    /// by feeding silence, so end-of-stream has to come from the track's known
    /// duration rather than from the mixer running dry.
    remaining_frames: Option<u64>,
}

impl PcmReader {
    /// Open a track from the library for streaming. Decoding starts lazily as
    /// the reader is polled, so opening is quick even for emulator backends.
    pub fn open(source: PlaybackSource, settings: DecoderSettings) -> Result<Self, String> {
        let registry = DecoderRegistry::new(settings);
        // The track's declared length is the only reliable end marker here.
        // A source that reports no duration falls back to the cap, so a
        // mislabelled file can never stream forever.
        let remaining_frames = Some(
            registry
                .probe(&source)
                .ok()
                .and_then(|properties| properties.duration)
                .map(|duration| {
                    (duration.as_secs_f64() * f64::from(STREAM_SAMPLE_RATE)).ceil() as u64
                })
                .unwrap_or(MAX_STREAM_FRAMES)
                .min(MAX_STREAM_FRAMES),
        );
        let (mixer_input, mixer_output) = stream_mixer();
        let player = Player::connect_new(&mixer_input);
        registry.append(&source, &player)?;
        Ok(Self {
            source: mixer_output,
            _player: Some(player),
            _registry: Some(registry),
            pending: Vec::with_capacity(PULL_BATCH * 4),
            position: 0,
            finished: false,
            remaining_frames,
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
            // No player: it would feed endless silence and the mixer would
            // never report the end of the source.
            _player: None,
            _registry: None,
            pending: Vec::with_capacity(PULL_BATCH * 4),
            position: 0,
            finished: false,
            remaining_frames: None,
        }
    }

    pub const fn sample_rate(&self) -> u32 {
        STREAM_SAMPLE_RATE
    }

    pub const fn channels(&self) -> u16 {
        STREAM_CHANNELS
    }

    fn refill(&mut self) {
        self.pending.clear();
        self.position = 0;
        if self.remaining_frames == Some(0) {
            self.finished = true;
            return;
        }
        for _ in 0..PULL_BATCH {
            match self.source.next() {
                Some(sample) => self.pending.extend_from_slice(&sample.to_le_bytes()),
                None => {
                    self.finished = true;
                    break;
                }
            }
        }
        if let Some(remaining) = self.remaining_frames.as_mut() {
            let delivered = (self.pending.len() / 4) as u64;
            *remaining = remaining.saturating_sub(delivered);
            if *remaining == 0 {
                self.finished = true;
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

/// Resolve a playlist entry to the single source a stream can be built from.
///
/// Reuses the same path the app uses for imports: the entry is written to a
/// one-line playlist and expanded by the registry, which is what gives archive
/// members, cue fragments and subsongs their correct addressing. A cue or
/// multi-song file addressed without a fragment yields its first track.
pub fn resolve_entry(
    entry: &crate::playlist::PlaylistEntry,
    decoders: &DecoderRegistry,
    scratch: &Path,
) -> Result<PlaybackSource, String> {
    // The expansion writes a one-line playlist here, and the playlist writer
    // creates its temporary file beside it. Make sure the directory exists:
    // the caller may be a fresh server whose scratch dir is still missing.
    std::fs::create_dir_all(scratch)
        .map_err(|error| format!("preparing {}: {error}", scratch.display()))?;
    let scratch_path = scratch.join("stream-entry.m3u");
    crate::playlist::Playlist::save(&scratch_path, std::slice::from_ref(entry))?;
    let expansion = decoders.expand_detailed(scratch_path)?;
    expansion
        .sources
        .into_iter()
        .next()
        .ok_or_else(|| "the entry did not resolve to a playable source".to_owned())
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
