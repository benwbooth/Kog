//! Whole-track scores for the MML view.
//!
//! The channel inspector follows the decoder that is playing. The MML view
//! needs every note of the song before it reaches them, so this renders the
//! track once more with the same decoder and records all of its frames.
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use kog_inspection::mml::{Score, ScoreBuilder};

use crate::decoder::{DecoderSettings, PlaybackSource};
use crate::streaming::PcmReader;

/// Audio seconds between drains of the monitor's bounded frame history.
const DRAIN_SECONDS: f64 = 0.25;

/// Audio seconds between partial scores while a song is still recording.
const PARTIAL_SECONDS: f64 = 20.0;
const FIRST_PARTIAL_SECONDS: f64 = 4.0;

/// Songs without a length (endless loops) are scored up to this point.
pub const MAX_SECONDS: f64 = 10.0 * 60.0;

/// Progress of a recording, readable from another thread.
#[derive(Default)]
pub struct Progress {
    pub recorded_ms: AtomicU64,
    pub total_ms: AtomicU64,
    pub cancel: AtomicBool,
}

pub fn analyze(
    source: PlaybackSource,
    settings: DecoderSettings,
    title: &str,
    progress: &Progress,
) -> Result<Score, String> {
    let mut pcm = PcmReader::open(source, settings.for_recording())?;
    record(&mut pcm, title, progress, MAX_SECONDS, &mut |_| {})
}

/// Like [`analyze`], also handing over the score recorded so far every
/// [`PARTIAL_SECONDS`] of audio so a view can show it before the end.
pub fn analyze_progressively(
    source: PlaybackSource,
    settings: DecoderSettings,
    title: &str,
    progress: &Progress,
    partial: &mut dyn FnMut(Score),
) -> Result<Score, String> {
    let mut pcm = PcmReader::open(source, settings.for_recording())?;
    record(&mut pcm, title, progress, MAX_SECONDS, partial)
}

pub fn record(
    pcm: &mut PcmReader,
    title: &str,
    progress: &Progress,
    max_seconds: f64,
    partial: &mut dyn FnMut(Score),
) -> Result<Score, String> {
    let monitor = pcm.inspection_monitor();
    let rate = pcm.sample_rate();
    let frame_bytes = 4 * u64::from(pcm.channels());
    let limit = pcm
        .duration()
        .map_or(max_seconds, |duration| duration.as_secs_f64().min(max_seconds));
    progress
        .total_ms
        .store((limit * 1000.0) as u64, Ordering::Relaxed);
    let mut builder = ScoreBuilder::new(rate);
    let mut buffer = vec![0u8; 64 * 1024];
    let mut consumed = 0u64;
    let mut drained = 0.0;
    let mut shared = 0.0;
    let mut after = None;
    let mut frames = 0usize;
    let drain = |through: f64, after: &mut Option<f64>, builder: &mut ScoreBuilder, frames: &mut usize| {
        let batch = monitor
            .take_frames()
            .unwrap_or_else(|| monitor.recording_frames(*after, Duration::from_secs_f64(through)));
        if let Some(last) = batch.last() {
            *after = Some(last.time);
        }
        *frames += batch.len();
        for frame in batch {
            builder.push(frame);
        }
    };
    loop {
        if progress.cancel.load(Ordering::Relaxed) {
            return Err("Score recording was cancelled".into());
        }
        let count = pcm.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        consumed += count as u64;
        let seconds = (consumed / frame_bytes) as f64 / f64::from(rate);
        if seconds - drained >= DRAIN_SECONDS {
            drain(seconds, &mut after, &mut builder, &mut frames);
            drained = seconds;
            progress
                .recorded_ms
                .store((seconds * 1000.0) as u64, Ordering::Relaxed);
            // The first bars appear quickly; later updates are spaced out
            // because each one encodes the whole score so far.
            let due = if shared == 0.0 { FIRST_PARTIAL_SECONDS } else { PARTIAL_SECONDS };
            if seconds - shared >= due && frames > 0 {
                shared = seconds;
                let description = monitor.snapshot(Duration::ZERO, false, false).description;
                partial(builder.clone().finish(&description, title, Some(seconds)));
            }
        }
        if seconds >= limit {
            break;
        }
    }
    let end = (consumed / frame_bytes) as f64 / f64::from(rate);
    drain(end + 1.0, &mut after, &mut builder, &mut frames);
    let description = monitor.snapshot(Duration::ZERO, false, false).description;
    if description.kind == "unavailable" || frames == 0 {
        return Err(if description.detail.is_empty() {
            "This source has no musical channel data".into()
        } else {
            description.detail
        });
    }
    Ok(builder.finish(&description, title, Some(end.min(limit))))
}
