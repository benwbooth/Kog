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
use std::path::{Path, PathBuf};
use std::time::Duration;

use rodio::Player;
use rodio::mixer::{MixerSource, mixer};

use crate::decoder::{DecoderRegistry, DecoderSettings, PlaybackSource};

/// Streaming format. Fixed so every encoder gets one input shape; rodio's
/// mixer resamples and remixes each backend's native rate and channel count.
pub const STREAM_SAMPLE_RATE: u32 = 48_000;
pub const STREAM_CHANNELS: u16 = 2;
/// Samples pulled per refill. Bounds the work done inside a single `read`.
const PULL_BATCH: usize = 2_048;

/// An unknown length needs a conservative end marker because the mixer can
/// otherwise feed silence forever. Known lengths also have a generous safety
/// ceiling for corrupt duration tags; audiobooks can exceed six hours.
const UNKNOWN_DURATION_STREAM_FRAMES: u64 = 6 * 60 * 60 * STREAM_SAMPLE_RATE as u64;
const MAX_STREAM_FRAMES: u64 = 72 * 60 * 60 * STREAM_SAMPLE_RATE as u64;

fn stream_frame_limit(duration: Option<std::time::Duration>) -> u64 {
    duration
        .map(|duration| (duration.as_secs_f64() * f64::from(STREAM_SAMPLE_RATE)).ceil() as u64)
        .unwrap_or(UNKNOWN_DURATION_STREAM_FRAMES)
        .min(MAX_STREAM_FRAMES)
}

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
    duration: Option<Duration>,
    total_frames: Option<u64>,
    /// Frames to deliver before stopping. A rodio player keeps its mixer alive
    /// by feeding silence, so end-of-stream has to come from the track's known
    /// duration rather than from the mixer running dry.
    remaining_frames: Option<u64>,
}

impl PcmReader {
    /// Expand a device path through the same archive, cue, and subsong rules as
    /// the desktop library. The registry stays alive with the PCM reader so
    /// extracted archive members remain available while playback runs.
    pub fn open_path(path: PathBuf, settings: DecoderSettings) -> Result<Self, String> {
        let registry = DecoderRegistry::new(settings);
        let source = registry
            .expand_detailed(path)?
            .sources
            .into_iter()
            .next()
            .ok_or_else(|| "No playable track was found in this file".to_owned())?;
        Self::open_with_registry(source, registry)
    }

    /// Open a track from the library for streaming. Decoding starts lazily as
    /// the reader is polled, so opening is quick even for emulator backends.
    pub fn open(source: PlaybackSource, settings: DecoderSettings) -> Result<Self, String> {
        let registry = DecoderRegistry::new(settings);
        Self::open_with_registry(source, registry)
    }

    fn open_with_registry(
        source: PlaybackSource,
        registry: DecoderRegistry,
    ) -> Result<Self, String> {
        // The track's declared length is the only reliable end marker here.
        // A source that reports no duration falls back to the cap, so a
        // mislabelled file can never stream forever.
        let duration = registry
            .probe(&source)
            .ok()
            .and_then(|properties| properties.duration);
        let total_frames = Some(stream_frame_limit(duration));
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
            duration,
            total_frames,
            remaining_frames: total_frames,
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
            duration: None,
            total_frames: None,
            remaining_frames: None,
        }
    }

    pub const fn sample_rate(&self) -> u32 {
        STREAM_SAMPLE_RATE
    }

    pub const fn channels(&self) -> u16 {
        STREAM_CHANNELS
    }

    pub fn duration(&self) -> Option<Duration> {
        self.duration
    }

    /// Move an open decoder before the next PCM read. Sources that do not
    /// support seeking report the same error as desktop playback.
    pub fn seek(&mut self, position: Duration) -> Result<(), String> {
        let player = self
            ._player
            .as_ref()
            .ok_or_else(|| "This source cannot seek".to_owned())?;
        // Rodio's seek order is applied by the mixer when its source is
        // polled. In a pull stream there is no separate audio thread polling
        // it, so calling try_seek here directly would wait forever. Drive the
        // mixer while the seek runs on a scoped thread, discarding samples
        // until the seek order has been processed.
        std::thread::scope(|scope| -> Result<(), String> {
            let seek = scope.spawn(|| player.try_seek(position));
            while !seek.is_finished() {
                if self.source.next().is_none() {
                    return Err("Audio source ended while seeking".to_owned());
                }
                std::thread::yield_now();
            }
            seek.join()
                .map_err(|_| "Audio decoder panicked while seeking".to_owned())?
                .map_err(|error| error.to_string())
        })?;
        self.pending.clear();
        self.position = 0;
        self.finished = false;
        self.remaining_frames = self.total_frames.map(|total| {
            total.saturating_sub((position.as_secs_f64() * f64::from(STREAM_SAMPLE_RATE)) as u64)
        });
        Ok(())
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
                Some(sample) => {
                    self.pending.extend_from_slice(&sample.to_le_bytes());
                }
                None => {
                    self.finished = true;
                    break;
                }
            }
        }
        if let Some(remaining) = self.remaining_frames.as_mut() {
            // `pending` is interleaved samples at STREAM_CHANNELS, but the
            // bound is in frames. Dividing by one f32 counted two stereo
            // samples as two frames, so every stream stopped at half its
            // declared length once the mixer padded past the real audio.
            let frame_bytes = std::mem::size_of::<f32>() * usize::from(STREAM_CHANNELS);
            let delivered = (self.pending.len() / frame_bytes) as u64;
            let allowed = delivered.min(*remaining);
            *remaining -= allowed;
            if allowed < delivered {
                // Never deliver past the declared end: trim the final batch.
                self.pending.truncate(allowed as usize * frame_bytes);
                self.finished = true;
            }
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

    #[test]
    fn known_audiobook_duration_is_not_cut_off_at_six_hours() {
        let eight_hours = std::time::Duration::from_secs(8 * 60 * 60);
        assert_eq!(
            stream_frame_limit(Some(eight_hours)),
            8 * 60 * 60 * u64::from(STREAM_SAMPLE_RATE)
        );
        assert_eq!(stream_frame_limit(None), UNKNOWN_DURATION_STREAM_FRAMES);
    }

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

    #[test]
    fn a_pull_reader_can_seek_without_an_audio_thread() {
        let frames = STREAM_SAMPLE_RATE as usize;
        let samples: Vec<f32> = (0..frames * usize::from(STREAM_CHANNELS))
            .map(|sample| if sample < frames { 0.25 } else { 0.75 })
            .collect();
        let (input, source) = stream_mixer();
        let player = Player::connect_new(&input);
        player.append(rodio::buffer::SamplesBuffer::new(
            NonZeroU16::new(STREAM_CHANNELS).unwrap(),
            NonZeroU32::new(STREAM_SAMPLE_RATE).unwrap(),
            samples,
        ));
        let mut reader = PcmReader {
            source,
            _player: Some(player),
            _registry: None,
            pending: Vec::new(),
            position: 0,
            finished: false,
            duration: Some(Duration::from_secs(1)),
            total_frames: Some(frames as u64),
            remaining_frames: Some(frames as u64),
        };
        reader.seek(Duration::from_millis(750)).expect("seek");
        let mut output = [0_u8; 4];
        reader.read_exact(&mut output).expect("read after seek");
        assert!((f32::from_le_bytes(output) - 0.75).abs() < 0.01);
    }

    #[test]
    fn the_duration_bound_counts_frames_not_samples() {
        // One second of stereo audio against a one-second bound: the reader
        // must deliver all 48 000 frames (two samples each), not 48 000
        // samples, which would stop at half a second.
        let frames = STREAM_SAMPLE_RATE as usize;
        let mut reader = PcmReader::from_rodio_source(Ramp {
            remaining: frames * usize::from(STREAM_CHANNELS),
            next: 0.0,
        });
        reader.remaining_frames = Some(frames as u64);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).expect("read pcm");
        assert_eq!(
            bytes.len(),
            frames * usize::from(STREAM_CHANNELS) * std::mem::size_of::<f32>(),
            "the bound is frames, and the final batch is trimmed to it"
        );
    }
}
