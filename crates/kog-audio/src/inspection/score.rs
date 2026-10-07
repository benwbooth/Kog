//! Whole-track scores for the MML view.
//!
//! The channel inspector follows the decoder that is playing. The MML view
//! needs every note of the song before it reaches them, so this renders the
//! track once more with the same decoder and records all of its frames.
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use kog_inspection::mml::{self, Document, Score};

use crate::decoder::{DecoderSettings, PlaybackSource};
use crate::streaming::PcmReader;

/// Audio seconds between drains of the monitor's bounded frame history.
const DRAIN_SECONDS: f64 = 0.25;

pub fn analyze(
    source: PlaybackSource,
    settings: DecoderSettings,
    title: &str,
    cancel: &AtomicBool,
) -> Result<Score, String> {
    let mut pcm = PcmReader::open(source, settings)?;
    record(&mut pcm, title, cancel, MAX_SECONDS)
}

/// Endless or mislabelled tracks stop here instead of rendering for hours.
pub const MAX_SECONDS: f64 = 30.0 * 60.0;

pub fn record(
    pcm: &mut PcmReader,
    title: &str,
    cancel: &AtomicBool,
    max_seconds: f64,
) -> Result<Score, String> {
    let monitor = pcm.inspection_monitor();
    let rate = pcm.sample_rate();
    let frame_bytes = 4 * u64::from(pcm.channels());
    let mut buffer = vec![0u8; 64 * 1024];
    let mut consumed = 0u64;
    let mut drained = 0.0;
    let mut after = None;
    let mut frames = Vec::new();
    let mut drain = |through: f64, after: &mut Option<f64>, frames: &mut Vec<_>| {
        let batch = monitor.recording_frames(*after, Duration::from_secs_f64(through));
        if let Some(last) = batch.last() {
            *after = Some(last.time);
        }
        frames.extend(batch);
    };
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Score analysis was cancelled".into());
        }
        let count = pcm.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        consumed += count as u64;
        let seconds = (consumed / frame_bytes) as f64 / f64::from(rate);
        if seconds - drained >= DRAIN_SECONDS {
            drain(seconds, &mut after, &mut frames);
            drained = seconds;
        }
        if seconds >= max_seconds {
            break;
        }
    }
    let end = (consumed / frame_bytes) as f64 / f64::from(rate);
    drain(end + 1.0, &mut after, &mut frames);
    let description = monitor.snapshot(Duration::ZERO, false, false).description;
    if description.kind == "unavailable" || frames.is_empty() {
        return Err(if description.detail.is_empty() {
            "This source has no musical channel data".into()
        } else {
            description.detail
        });
    }
    Ok(mml::score_from_frames(
        &frames,
        &description,
        title,
        rate,
        Some(pcm.duration().map_or(end, |d| d.as_secs_f64().min(end.max(0.0)))),
    ))
}

pub fn document(score: &Score) -> Document {
    mml::encode(score)
}
