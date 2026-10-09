//! Platform-neutral channel inspection data and playback-clock selection.
pub mod guide;
pub mod mml;

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

impl Channel {
    pub fn field(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|field| field.name == name)
            .map(|field| field.value.as_str())
    }

    /// Sample-rate transposition has no known equal-tempered tuning reference.
    /// Its fractional part must not be presented as musical cents or a bend.
    pub fn has_relative_pitch(&self) -> bool {
        self.field("Pitch basis")
            .is_some_and(|value| value.starts_with("Relative"))
    }
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

impl Snapshot {
    /// Keep the tracker's lookahead stable across an HTTP window boundary.
    /// This adds future rows without advancing the current row or keyboard.
    pub fn append_upcoming_rows(&mut self, next: &Window) {
        if self.seeking || self.current_row.is_none() {
            return;
        }
        for row in next
            .rows
            .iter()
            .chain(next.frames.iter().filter_map(|frame| frame.row.as_ref()))
        {
            if self.rows.len() >= TRACKER_ROWS {
                break;
            }
            if self.rows.last().is_some_and(|last| row.time > last.time) {
                self.rows.push(row.clone());
            }
        }
    }
}

/// A tracker cell's classic columns for one channel: notes, instrument,
/// volume and effects. Effects named `FX…` are a tracker's own effect columns
/// and show their value alone.
pub fn tracker_parts(cells: &[&Cell]) -> [String; 4] {
    let first = |pick: fn(&Cell) -> &str| cells.iter().map(|cell| pick(cell)).find(|v| !v.is_empty()).unwrap_or_default().to_owned();
    [
        cells.iter().map(|cell| cell.notes.as_str()).filter(|v| !v.is_empty()).collect::<Vec<_>>().join(" "),
        first(|cell| &cell.instrument),
        first(|cell| &cell.volume),
        cells
            .iter()
            .flat_map(|cell| &cell.effects)
            .map(|field| {
                let tracker_column = field.name.strip_prefix("FX").is_some_and(|rest| rest.bytes().all(|b| b.is_ascii_digit()));
                if tracker_column { field.value.clone() } else { format!("{} {}", field.name, field.value) }
            })
            .collect::<Vec<_>>()
            .join(" "),
    ]
}

/// Fewest and most characters each tracker column takes.
pub const TRACKER_LIMITS: [(usize, usize); 4] = [(3, 7), (2, 8), (2, 4), (3, 18)];

/// Tracker rows a snapshot carries, and how many of them come before the
/// playing row: enough to fill a tall window.
pub const TRACKER_ROWS: usize = 96;
pub const TRACKER_HISTORY: usize = 72;

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
            if channel.has_relative_pitch() {
                return spu_event_cell(before, channel);
            }
            // A row is a musical event: a key-on or release, a new note or
            // instrument. Envelopes, vibrato and other register motion
            // change on almost every frame and would race the rows past;
            // the cell still shows their values at each event.
            let names = |channel: &Channel| channel.notes.iter().map(|note| note.key.round() as i32).collect::<Vec<_>>();
            let key_on = |channel: &Channel| (channel.field("Key on").map(str::to_owned), channel.field("Gate").map(str::to_owned));
            let changed = before.is_none_or(|before| {
                before.active != channel.active
                    || names(before) != names(channel)
                    || before.instrument != channel.instrument
                    || key_on(before) != key_on(channel)
            });
            changed.then(|| Cell {
                channel: channel.id,
                notes: if !channel.active {
                    "OFF".into()
                } else if channel.field("Pitch basis").is_some_and(|basis| basis.starts_with("Drum map")) {
                    // A drum map's key is a sample number: S01 is the first.
                    channel.notes.iter().map(|note| format!("S{:02}", note.key.round() as i32 - 35)).collect::<Vec<_>>().join(" ")
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

/// SPU tracker rows describe key gates and register commands, not each sample
/// of the running envelope or ADPCM address. Key-on counters retain same-note
/// retriggers even when a short release falls between inspection snapshots.
fn spu_event_cell(before: Option<&Channel>, channel: &Channel) -> Option<Cell> {
    let gate = |c: &Channel| c.active && c.field("Gate") != Some("Off");
    let down = gate(channel);
    let was_down = before.is_some_and(gate);
    let retrigger = before.is_some_and(|old| {
        channel.field("Key on").is_some() && old.field("Key on") != channel.field("Key on")
    });
    let started = down && (!was_down || retrigger);
    let stopped = was_down && !down;
    let effects = channel
        .fields
        .iter()
        .filter(|field| {
            matches!(
                field.name.as_str(),
                "Pitch rate" | "ADSR" | "Volume L/R" | "Loop" | "Reverb" | "Control"
            ) && (started
                || (down
                    && !matches!(field.name.as_str(), "Pitch rate" | "Volume L/R")
                    && before
                        .is_some_and(|old| old.field(&field.name) != Some(field.value.as_str()))))
        })
        .cloned()
        .collect::<Vec<_>>();
    if !started && !stopped && effects.is_empty() {
        return None;
    }
    Some(Cell {
        channel: channel.id,
        notes: if stopped {
            "OFF".into()
        } else if started {
            channel
                .notes
                .iter()
                .map(|note| note_name(note.key))
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            String::new()
        },
        instrument: if started {
            channel.instrument.clone()
        } else {
            String::new()
        },
        volume: if started {
            format!(
                "{:02X}",
                (channel.level.clamp(0.0, 1.0) * 255.0).round() as u8
            )
        } else {
            String::new()
        },
        effects,
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
        // Resolve references first: cloning every intermediate 24/48-voice
        // register frame at every UI tick is needlessly expensive.
        let mut channels = self
            .initial
            .channels
            .iter()
            .map(|channel| (channel.id, channel))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut global = &self.initial.global;
        for frame in self
            .frames
            .iter()
            .take_while(|frame| frame.time <= position + 0.000_001)
        {
            for id in &frame.removed {
                channels.remove(id);
            }
            for channel in &frame.channels {
                channels.insert(channel.id, channel);
            }
            if let Some(fields) = &frame.global {
                global = fields;
            }
        }
        snapshot.channels = channels.into_values().cloned().collect();
        snapshot.global = global.clone();
        let mut rows = self.rows.iter().collect::<Vec<_>>();
        for row in self.frames.iter().filter_map(|frame| frame.row.as_ref()) {
            if rows.last().is_none_or(|last| {
                last.label != row.label || last.cells != row.cells || last.global != row.global
            }) {
                rows.push(row);
            }
        }
        let cursor = rows.partition_point(|row| row.time <= position + 0.000_001);
        let begin = cursor.saturating_sub(TRACKER_HISTORY);
        snapshot.rows = rows.into_iter().skip(begin).take(TRACKER_ROWS).cloned().collect();
        snapshot.current_row = cursor.checked_sub(1).and_then(|i| i.checked_sub(begin));
        snapshot
    }
}

/// First missing window in a contiguous lookahead from the audible position.
/// A later cached window (for example after a backwards seek) cannot fill a gap.
pub fn missing_window_position(windows: &[Window], position: f64, lookahead: f64) -> Option<f64> {
    if !position.is_finite() || !lookahead.is_finite() || lookahead < 0.0 {
        return None;
    }
    let target = position + lookahead;
    let mut cursor = position;
    for _ in 0..=windows.len() {
        let Some(window) = windows.iter().find(|w| cursor >= w.start && cursor < w.end) else {
            return Some(cursor);
        };
        if window.end >= target {
            return None;
        }
        cursor = window.end;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn spu_rows_follow_gates_retriggers_and_commands_without_envelope_spam() {
        let mut channel = Channel {
            id: 0,
            active: true,
            kind: "sample".into(),
            instrument: "ADPCM 065010".into(),
            level: 0.8,
            notes: vec![Note {
                key: 56.285213,
                velocity: 0.8,
                held: true,
            }],
            fields: vec![
                Field::new("Pitch basis", "Relative (C4 = normal sample rate)"),
                Field::new("Gate", "On"),
                Field::new("Key on", 1),
                Field::new("Envelope", "7FFFFFFF state 4"),
                Field::new("Current", "065010"),
                Field::new("Pitch rate", "35582.1 Hz (0CE8)"),
                Field::new("Volume L/R", "1000/1000"),
            ],
            ..Channel::default()
        };
        let row = |before: &[Channel], channel: &Channel| {
            changes_row(
                before,
                &FrameData {
                    channels: vec![channel.clone()],
                    ..FrameData::default()
                },
                1.0,
            )
        };
        assert!(channel.has_relative_pitch());
        let first = row(&[], &channel).unwrap();
        assert_eq!(first.cells[0].notes, "G#3");
        assert!(
            !first.cells[0]
                .effects
                .iter()
                .any(|f| matches!(f.name.as_str(), "Envelope" | "Current" | "Key on"))
        );
        let old = channel.clone();
        channel.level = 0.4;
        channel.notes[0].velocity = 0.4;
        channel.fields[3].value = "3FFFFFFF state 4".into();
        channel.fields[4].value = "065123".into();
        assert!(
            row(&[old], &channel).is_none(),
            "a held note is not another note-on"
        );
        let old = channel.clone();
        channel.fields[2].value = "2".into();
        assert_eq!(
            row(&[old], &channel).unwrap().cells[0].notes,
            "G#3",
            "real same-note retrigger"
        );
        let old = channel.clone();
        channel.fields[6].value = "0800/0800".into();
        assert!(
            row(&[old], &channel).is_none(),
            "continuous volume automation stays in the live details"
        );
        let old = channel.clone();
        channel.fields.push(Field::new("ADSR", "00FF 5FC5"));
        let command = row(&[old], &channel).unwrap();
        assert!(
            command.cells[0].notes.is_empty(),
            "a control command does not retrigger the note"
        );
        assert_eq!(
            command.cells[0].effects,
            vec![Field::new("ADSR", "00FF 5FC5")]
        );
        let old = channel.clone();
        channel.fields[1].value = "Off".into();
        channel.notes[0].held = false;
        assert_eq!(
            row(&[old], &channel).unwrap().cells[0].notes,
            "OFF",
            "key-off precedes the release tail ending"
        );
        let old = channel.clone();
        channel.level = 0.1;
        channel.notes[0].velocity = 0.1;
        channel.fields[3].value = "01000000 state 5".into();
        assert!(row(&[old], &channel).is_none());
        let old = channel.clone();
        channel.active = false;
        channel.notes.clear();
        assert!(
            row(&[old], &channel).is_none(),
            "ending the release is not a second key-off"
        );
        assert!(
            row(&[], &channel).is_none(),
            "silent voices do not populate the tracker"
        );
    }

    #[test]
    fn tracker_parts_split_cells_into_classic_columns() {
        let cell = Cell {
            channel: 0,
            notes: "C-4".into(),
            instrument: "01".into(),
            volume: "40".into(),
            effects: vec![Field::new("FX", "A0F"), Field::new("Sustain", "On")],
        };
        let chord = Cell { notes: "E-4".into(), instrument: String::new(), ..cell.clone() };
        assert_eq!(tracker_parts(&[&cell, &chord]), ["C-4 E-4", "01", "40", "A0F Sustain On A0F Sustain On"].map(String::from));
        assert_eq!(tracker_parts(&[]), ["", "", "", ""].map(String::from));
    }

    #[test]
    fn inspection_tracker_keeps_upcoming_rows_across_window_boundary() {
        let row = |time: f64| Row {
            time,
            label: time.to_string(),
            ..Row::default()
        };
        let mut snapshot = Snapshot {
            position: 0.99,
            rows: (0..24).map(|i| row(0.75 + f64::from(i) * 0.01)).collect(),
            current_row: Some(23),
            ..Snapshot::default()
        };
        let next = Window {
            rows: snapshot.rows.clone(),
            frames: (0..30)
                .map(|i| Delta {
                    time: 1.0 + f64::from(i) * 0.01,
                    row: Some(row(1.0 + f64::from(i) * 0.01)),
                    ..Delta::default()
                })
                .collect(),
            ..Window::default()
        };
        snapshot.append_upcoming_rows(&next);
        assert_eq!(snapshot.rows.len(), 54);
        assert_eq!(snapshot.current_row, Some(23));
        assert_eq!(snapshot.position, 0.99);
        assert_eq!(snapshot.rows[24].time, 1.0);
        snapshot.append_upcoming_rows(&next);
        assert_eq!(snapshot.rows.len(), 54);
    }
    #[test]
    fn inspection_prefetch_fills_gaps_after_backwards_seek() {
        let window = |start| Window {
            start,
            end: start + 1.0,
            ..Window::default()
        };
        let mut windows = vec![window(6.0), window(7.0), window(0.0)];
        assert_eq!(missing_window_position(&windows, 0.1, 2.0), Some(1.0));
        windows.push(window(1.0));
        assert_eq!(missing_window_position(&windows, 0.1, 2.0), Some(2.0));
        windows.push(window(2.0));
        assert_eq!(missing_window_position(&windows, 0.9, 2.0), None);
        assert_eq!(missing_window_position(&windows, 3.0, 2.0), Some(3.0));
        assert_eq!(missing_window_position(&windows, 6.9, 2.0), Some(8.0));
    }

    #[test]
    fn inspection_snapshot_matches_delta_replay_with_removed_voices_and_globals() {
        let voice = |id, key| Channel {
            id,
            notes: vec![Note {
                key,
                held: true,
                velocity: 1.0,
            }],
            ..Channel::default()
        };
        let initial = FrameData {
            channels: vec![voice(0, 60.0), voice(1, 64.0)],
            global: vec![Field::new("Tempo", 120)],
            ..FrameData::default()
        };
        let frames = vec![
            Delta {
                time: 0.1,
                channels: vec![voice(1, 65.0)],
                removed: vec![0],
                global: Some(vec![Field::new("Tempo", 140)]),
                ..Delta::default()
            },
            Delta {
                time: 0.2,
                channels: vec![voice(0, 67.0)],
                ..Delta::default()
            },
        ];
        let window = Window {
            start: 0.0,
            end: 1.0,
            initial: initial.clone(),
            frames,
            ..Window::default()
        };
        for position in [0.0, 0.1, 0.15, 0.2, 0.5] {
            let mut expected = initial.clone();
            for frame in window.frames.iter().filter(|frame| frame.time <= position) {
                frame.apply(&mut expected);
            }
            let actual = window.snapshot(position, true, false);
            assert_eq!(actual.channels, expected.channels);
            assert_eq!(actual.global, expected.global);
        }
    }

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
