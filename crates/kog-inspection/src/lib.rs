//! Platform-neutral channel inspection data and playback-clock selection.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub value: String,
}

impl Field {
    pub fn new(name: impl Into<String>, value: impl ToString) -> Self {
        Self {
            name: name.into(),
            value: value.to_string(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Note {
    /// MIDI key number, including fractional semitones for bends and slides.
    pub key: f32,
    pub velocity: f32,
    /// A released note retained by sustain is distinguished from a held key.
    pub held: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub id: u32,
    pub name: String,
    /// tonal, percussion, noise, sample, or mixed. Unpitched activity must
    /// never be converted into a made-up piano key.
    pub kind: String,
    pub notes: Vec<Note>,
    pub instrument: String,
    pub level: f32,
    pub pan: f32,
    pub active: bool,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Cell {
    pub channel: u32,
    pub notes: String,
    pub instrument: String,
    pub volume: String,
    pub effects: Vec<Field>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub time: f64,
    pub label: String,
    pub cells: Vec<Cell>,
    pub global: Vec<Field>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FrameData {
    pub channels: Vec<Channel>,
    /// Native tracker row or sequencer event. For register snapshots the
    /// consumer builds rows from changes, retaining their observed times.
    pub row: Option<Row>,
    pub global: Vec<Field>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Description {
    pub backend: String,
    /// events, patterns, registers, or unavailable.
    pub kind: String,
    pub detail: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: u32,
    pub description: Description,
    pub position: f64,
    pub playing: bool,
    pub seeking: bool,
    pub channels: Vec<Channel>,
    pub rows: Vec<Row>,
    pub current_row: Option<usize>,
    pub global: Vec<Field>,
    pub dropped_frames: u64,
}

pub fn note_name(key: f32) -> String {
    if !key.is_finite() {
        return "—".into();
    }
    let rounded = key.round() as i32;
    let names = [
        "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
    ];
    format!(
        "{}{}",
        names[rounded.rem_euclid(12) as usize],
        rounded.div_euclid(12) - 1
    )
}

pub fn frequency_key(hz: f64) -> Option<f32> {
    (hz.is_finite() && hz > 0.0).then(|| (69.0 + 12.0 * (hz / 440.0).log2()) as f32)
}

pub fn changes_row(previous: &[Channel], data: &FrameData, time: f64) -> Option<Row> {
    let cells = data
        .channels
        .iter()
        .filter_map(|channel| {
            let before = previous.iter().find(|before| before.id == channel.id);
            let changed = before.is_none_or(|before| {
                before.active != channel.active
                    || before.notes != channel.notes
                    || before.instrument != channel.instrument
                    || before.fields != channel.fields
            });
            changed.then(|| Cell {
                channel: channel.id,
                notes: if !channel.active {
                    "OFF".into()
                } else if channel.notes.is_empty() {
                    channel.kind.to_uppercase()
                } else {
                    channel
                        .notes
                        .iter()
                        .map(|note| note_name(note.key))
                        .collect::<Vec<_>>()
                        .join(" ")
                },
                instrument: channel.instrument.clone(),
                volume: format!(
                    "{:02X}",
                    (channel.level.clamp(0.0, 1.0) * 255.0).round() as u8
                ),
                effects: channel.fields.clone(),
            })
        })
        .collect::<Vec<_>>();
    (!cells.is_empty()).then(|| Row {
        time,
        label: format!("{time:08.3}"),
        cells,
        global: data.global.clone(),
    })
}

/// A decoder state stamped in seconds on the original track's timeline.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TimedFrame {
    pub time: f64,
    pub data: FrameData,
}

/// Only changed channels travel between full checkpoints. This keeps cached
/// metadata and HTTP windows small even on chips with dozens of voices.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Delta {
    pub time: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub channels: Vec<Channel>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub removed: Vec<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub global: Option<Vec<Field>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub row: Option<Row>,
}

impl Delta {
    pub fn between(previous: &FrameData, next: &TimedFrame) -> Self {
        Self {
            time: next.time,
            channels: next
                .data
                .channels
                .iter()
                .filter(|channel| {
                    previous.channels.iter().find(|old| old.id == channel.id) != Some(*channel)
                })
                .cloned()
                .collect(),
            removed: previous
                .channels
                .iter()
                .filter(|old| !next.data.channels.iter().any(|c| c.id == old.id))
                .map(|c| c.id)
                .collect(),
            global: (previous.global != next.data.global).then(|| next.data.global.clone()),
            row: next
                .data
                .row
                .clone()
                .or_else(|| changes_row(&previous.channels, &next.data, next.time)),
        }
    }

    pub fn apply(&self, data: &mut FrameData) {
        data.channels
            .retain(|channel| !self.removed.contains(&channel.id));
        for channel in &self.channels {
            if let Some(old) = data.channels.iter_mut().find(|old| old.id == channel.id) {
                *old = channel.clone();
            } else {
                data.channels.push(channel.clone());
            }
        }
        data.channels.sort_by_key(|channel| channel.id);
        if let Some(global) = &self.global {
            data.global = global.clone();
        }
        data.row = self.row.clone();
    }

    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
            && self.removed.is_empty()
            && self.global.is_none()
            && self.row.is_none()
    }
}

/// A bounded piece of recorded playback. Positions stay on the source clock
/// even when the audio stream starts at a nonzero seek position.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Window {
    pub version: u32,
    pub description: Description,
    pub start: f64,
    pub end: f64,
    pub initial: FrameData,
    pub rows: Vec<Row>,
    pub frames: Vec<Delta>,
}

impl Window {
    pub fn snapshot(&self, position: f64, playing: bool, seeking: bool) -> Snapshot {
        let mut snapshot = Snapshot {
            version: self.version,
            description: self.description.clone(),
            position,
            playing,
            seeking,
            ..Snapshot::default()
        };
        if seeking || !position.is_finite() || position < self.start || position >= self.end {
            return snapshot;
        }
        let mut data = self.initial.clone();
        for frame in self
            .frames
            .iter()
            .take_while(|frame| frame.time <= position + 0.000_001)
        {
            frame.apply(&mut data);
        }
        snapshot.channels = data.channels;
        snapshot.global = data.global;
        let mut rows = self.rows.clone();
        for row in self.frames.iter().filter_map(|frame| frame.row.as_ref()) {
            if rows.last().is_none_or(|last| {
                last.label != row.label || last.cells != row.cells || last.global != row.global
            }) {
                rows.push(row.clone());
            }
        }
        let cursor = rows.partition_point(|row| row.time <= position + 0.000_001);
        let begin = cursor.saturating_sub(24);
        snapshot.rows = rows.into_iter().skip(begin).take(48).collect();
        snapshot.current_row = cursor.checked_sub(1).and_then(|i| i.checked_sub(begin));
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn buffered_windows_follow_the_consumer_clock_in_both_directions() {
        let channel = |key| Channel {
            id: 2,
            active: true,
            notes: vec![Note {
                key,
                velocity: 1.0,
                held: true,
            }],
            ..Channel::default()
        };
        let initial = FrameData {
            channels: vec![channel(60.0)],
            ..FrameData::default()
        };
        let next = TimedFrame {
            time: 5.4,
            data: FrameData {
                channels: vec![channel(64.0)],
                ..FrameData::default()
            },
        };
        let window = Window {
            version: 1,
            start: 5.0,
            end: 6.0,
            initial: initial.clone(),
            frames: vec![Delta::between(&initial, &next)],
            ..Window::default()
        };
        for (position, key) in [(5.1, 60.0), (5.5, 64.0), (5.2, 60.0)] {
            let snapshot = window.snapshot(position, false, false);
            assert_eq!(snapshot.channels[0].notes[0].key, key);
            assert!(!snapshot.playing);
        }
        assert!(window.snapshot(5.5, true, true).channels.is_empty());
        assert!(window.snapshot(6.0, true, false).channels.is_empty());
    }
}
