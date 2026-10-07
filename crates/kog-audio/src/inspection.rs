//! Musical state from the decoder that is producing the audio.
//!
//! The producer never waits for the UI. Its bounded queue carries snapshots
//! with media timestamps; consumers choose the state at their audible play
//! position, rather than showing the decoder's (possibly buffered) future.
use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

use concurrent_queue::ConcurrentQueue;

#[cfg(any(windows, test))]
pub(crate) mod helper_ring;
#[cfg(test)]
mod integration;
pub mod midi;
pub(crate) mod native;
pub mod recording;
pub(crate) mod remote;

const QUEUE_FRAMES: usize = 1024;
const HISTORY_FRAMES: usize = 2048;
const HISTORY_ROWS: usize = 256;

pub use kog_inspection::*;

struct Frame {
    time: f64,
    epoch: u64,
    data: FrameData,
}

struct Shared {
    capture: AtomicBool,
    dropped: AtomicU64,
    next_epoch: AtomicU64,
    queue: ConcurrentQueue<Frame>,
    description: Mutex<Description>,
    midi: Mutex<Option<midi::Timeline>>,
    history: Mutex<History>,
}

#[derive(Default)]
struct History {
    epoch: u64,
    frames: VecDeque<Frame>,
    rows: VecDeque<Row>,
    previous: Vec<Channel>,
}

/// One feed per loaded source. Replacing a track replaces the feed, so an old
/// decoder or seek completion cannot publish into the new song's display.
#[derive(Clone)]
pub struct Monitor(Arc<Shared>);

impl Default for Monitor {
    fn default() -> Self {
        Self(Arc::new(Shared {
            capture: AtomicBool::new(true),
            dropped: AtomicU64::new(0),
            next_epoch: AtomicU64::new(1),
            queue: ConcurrentQueue::bounded(QUEUE_FRAMES),
            description: Mutex::new(Description {
                kind: "unavailable".into(),
                detail: "No musical channel data is available for this source.".into(),
                ..Description::default()
            }),
            midi: Mutex::new(None),
            history: Mutex::new(History::default()),
        }))
    }
}

impl Monitor {
    pub fn describe(&self, backend: &str, kind: &str, detail: &str) {
        *self.0.description.lock().unwrap_or_else(|e| e.into_inner()) = Description {
            backend: backend.into(),
            kind: kind.into(),
            detail: detail.into(),
        };
    }

    pub fn set_midi(&self, timeline: midi::Timeline) {
        *self.0.midi.lock().unwrap_or_else(|e| e.into_inner()) = Some(timeline);
    }

    pub fn producer(&self, sample_rate: u32) -> Producer {
        Producer {
            monitor: self.clone(),
            sample_rate,
            frames: 0,
            epoch: self.0.next_epoch.fetch_add(1, Ordering::Relaxed),
        }
    }

    pub fn enabled(&self) -> bool {
        self.0.capture.load(Ordering::Relaxed)
    }

    /// Capture recent state even before the inspector opens: this lets a
    /// paused player show the state at its consumed clock immediately.
    /// The bounded queue discards old unobserved frames without blocking audio.
    pub fn set_enabled(&self, enabled: bool) {
        self.0.capture.store(enabled, Ordering::Relaxed);
    }

    pub fn snapshot(&self, position: Duration, playing: bool, seeking: bool) -> Snapshot {
        let mut snapshot = Snapshot {
            version: 1,
            description: self
                .0
                .description
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone(),
            position: position.as_secs_f64(),
            playing,
            seeking,
            dropped_frames: self.0.dropped.load(Ordering::Relaxed),
            ..Snapshot::default()
        };
        if let Some(timeline) = self
            .0
            .midi
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
        {
            timeline.fill_snapshot(&mut snapshot);
            return snapshot;
        }
        let mut history = self.0.history.lock().unwrap_or_else(|e| e.into_inner());
        while let Ok(frame) = self.0.queue.pop() {
            if frame.epoch < history.epoch {
                continue;
            }
            if frame.epoch != history.epoch {
                *history = History {
                    epoch: frame.epoch,
                    ..History::default()
                };
            }
            let row = frame
                .data
                .row
                .clone()
                .or_else(|| changes_row(&history.previous, &frame.data, frame.time));
            if let Some(mut row) = row {
                row.time = frame.time;
                let repeated = history.rows.back().is_some_and(|last| {
                    last.label == row.label && last.cells == row.cells && last.global == row.global
                });
                if !repeated {
                    history.rows.push_back(row);
                    if history.rows.len() > HISTORY_ROWS {
                        history.rows.pop_front();
                    }
                }
            }
            history.previous = frame.data.channels.clone();
            history.frames.push_back(frame);
            if history.frames.len() > HISTORY_FRAMES {
                history.frames.pop_front();
            }
        }
        if seeking {
            return snapshot;
        }
        if let Some(frame) = history
            .frames
            .iter()
            .rev()
            .find(|frame| frame.time <= snapshot.position + 0.000_001)
        {
            snapshot.channels = frame.data.channels.clone();
            snapshot.global = frame.data.global.clone();
        }
        // Retain a little future data: native trackers decode ahead of the
        // sound device, while the highlighted row follows audible position.
        let cursor = history
            .rows
            .partition_point(|row| row.time <= snapshot.position + 0.000_001);
        let begin = cursor.saturating_sub(24);
        snapshot.rows = history.rows.iter().skip(begin).take(48).cloned().collect();
        snapshot.current_row = cursor.checked_sub(1).and_then(|i| i.checked_sub(begin));
        snapshot
    }

    /// Drain complete decoder frames for the stream encoder. The caller uses
    /// its consumed PCM cursor, including any nonzero stream start position.
    pub fn recording_frames(&self, after: Option<f64>, through: Duration) -> Vec<TimedFrame> {
        let _ = self.snapshot(through, true, false);
        if let Some(timeline) = self
            .0
            .midi
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_mut()
        {
            return timeline.recording_frames(after, through.as_secs_f64());
        }
        let history = self.0.history.lock().unwrap_or_else(|e| e.into_inner());
        history
            .frames
            .iter()
            .filter(|frame| {
                after.is_none_or(|after| frame.time > after)
                    && frame.time <= through.as_secs_f64() + 0.000_001
            })
            .map(|frame| TimedFrame {
                time: frame.time,
                data: frame.data.clone(),
            })
            .collect()
    }

    pub fn json(&self, position: Duration, playing: bool, seeking: bool) -> String {
        serde_json::to_string(&self.snapshot(position, playing, seeking))
            .unwrap_or_else(|_| "{}".into())
    }
}

/// Owned by one decoder, alongside its render cursor. Call `seek` only after
/// the decoder has completed a seek; frames from an earlier epoch are then
/// discarded by the consumer.
pub struct Producer {
    monitor: Monitor,
    sample_rate: u32,
    frames: u64,
    epoch: u64,
}

impl Producer {
    /// A helper starts recording before its header is read. Move those early
    /// frames into the player's feed without losing short tracks or intros.
    pub(crate) fn into_monitor(self, monitor: &Monitor, sample_rate: u32) -> Self {
        let producer=monitor.producer(sample_rate);
        while let Ok(frame)=self.monitor.0.queue.pop() { producer.publish_at(frame.data,frame.time); }
        producer
    }
    pub fn enabled(&self) -> bool {
        self.monitor.enabled()
    }

    pub fn render_frames(&self, maximum: usize) -> usize {
        if self.enabled() {
            maximum.min((self.sample_rate / 200).max(1) as usize)
        } else {
            maximum
        }
    }

    pub fn advance(&mut self, frames: usize) {
        self.frames = self.frames.saturating_add(frames as u64);
    }

    pub fn seek(&mut self, position: Duration) {
        self.frames = (position.as_secs_f64() * f64::from(self.sample_rate)) as u64;
        self.epoch = self.monitor.0.next_epoch.fetch_add(1, Ordering::Relaxed);
    }

    pub fn publish(&self, data: FrameData) {
        self.publish_offset(data, 0.0);
    }

    /// Internal emulator buffers can run ahead of the source's PCM cursor.
    pub fn publish_offset(&self, data: FrameData, ahead_seconds: f64) {
        self.publish_at(
            data,
            self.frames as f64 / f64::from(self.sample_rate.max(1)) + ahead_seconds.max(0.0),
        );
    }

    pub fn publish_at(&self, data: FrameData, time: f64) {
        if !self.enabled() {
            return;
        }
        let frame = Frame {
            time,
            epoch: self.epoch,
            data,
        };
        if self
            .monitor
            .0
            .queue
            .force_push(frame)
            .ok()
            .flatten()
            .is_some()
        {
            self.monitor.0.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(key: f32) -> FrameData {
        FrameData {
            channels: vec![Channel {
                id: 0,
                name: "Voice 1".into(),
                kind: "tonal".into(),
                active: true,
                notes: vec![Note {
                    key,
                    velocity: 1.0,
                    held: true,
                }],
                ..Channel::default()
            }],
            ..FrameData::default()
        }
    }

    #[test]
    fn snapshots_follow_consumed_audio_and_reset_on_seek() {
        let monitor = Monitor::default();
        monitor.snapshot(Duration::ZERO, true, false);
        let mut producer = monitor.producer(1000);
        producer.publish(frame(60.0));
        producer.advance(1000);
        producer.publish(frame(64.0));
        let snapshot = monitor.snapshot(Duration::from_millis(500), true, false);
        assert_eq!(snapshot.channels[0].notes[0].key, 60.0);
        assert_eq!(snapshot.current_row, Some(0));
        assert_eq!(
            monitor
                .snapshot(Duration::from_secs(1), false, false)
                .channels[0]
                .notes[0]
                .key,
            64.0
        );
        producer.seek(Duration::ZERO);
        producer.publish(frame(67.0));
        let snapshot = monitor.snapshot(Duration::ZERO, false, false);
        assert_eq!(snapshot.channels[0].notes[0].key, 67.0);
        assert_eq!(snapshot.rows.len(), 1);
        assert!(
            monitor
                .snapshot(Duration::ZERO, true, true)
                .channels
                .is_empty()
        );
    }
}
