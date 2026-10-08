//! Kog MML: one text notation for every chip and sequencer Kog can inspect.
//!
//! A [`Score`] is the piano roll recorded from the decoder that produced the
//! audio: per-voice notes on an integer tick grid, plus the chip parameters
//! that changed while they played. [`encode`] writes it as MML split into
//! tracks and bars, and [`parse`] reads that text back into the identical
//! score, so the notation can be edited or stored without losing events.
//!
//! The syntax follows classic MML (`o4 l8 c d+ > e-4. r ^16`) with a few
//! extensions; see `docs/KOG_MML.md`.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use crate::{Description, TimedFrame};

pub const FORMAT: &str = "KOG-MML 1";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Score {
    pub title: String,
    pub backend: String,
    /// Quarter notes per minute, in thousandths.
    pub tempo: u32,
    /// Ticks per quarter note. A multiple of 32 lets every length be written
    /// with note values down to 1/128.
    pub quarter: u32,
    /// Quarter notes per bar. Bars only lay out the text.
    pub beats: u32,
    /// True when the tempo was estimated from note onsets.
    pub inferred: bool,
    pub length: u64,
    pub tracks: Vec<Track>,
    /// Per-note automation (envelopes, vibrato, duty sequences) shared by
    /// every note that repeats it.
    pub macros: Vec<Macro>,
    /// Octave shifts for instruments whose pitch is only relative (sample
    /// playback rates), applied when notes are written and read back.
    pub transpose: Vec<(String, i32)>,
    /// Ticks before the first full bar, so bars follow the music's grid.
    pub pickup: u64,
    /// Tick and time (microseconds) pairs where the tempo has drifted from a
    /// straight line; empty when `tempo` alone gives every tick's time.
    pub timing: Vec<(u64, u64)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Target {
    Level,
    Pan,
    Bend,
    Param(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Macro {
    pub target: Target,
    /// Ticks after the macro starts, and the value set then.
    pub steps: Vec<(u64, String)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Track {
    pub label: String,
    pub channel: u32,
    pub voice: u32,
    pub name: String,
    pub kind: String,
    /// Sorted by tick. Commands at a note's onset tick precede the note.
    pub events: Vec<Event>,
    /// Registers that follow the note's pitch (for example a period or
    /// frequency register): set at each note whose pitch changes them, so they
    /// are listed once per pitch instead of at every note.
    pub pitched: Vec<PitchedParam>,
    /// Each key's usual detune in cents. Notes with that detune omit it; a
    /// chip's fixed period table gives every pitch its own small offset.
    pub tuning: Vec<(i32, i32)>,
}

impl Track {
    fn tuning(&self, key: i32) -> i32 {
        self.tuning.iter().find(|(k, _)| *k == key).map_or(0, |(_, cents)| *cents)
    }
}

/// Record each key's most common detune on this track.
fn extract_tuning(track: &mut Track) {
    let mut counts: BTreeMap<(i32, i32), usize> = BTreeMap::new();
    for event in &track.events {
        if let EventKind::Note { key, cents, .. } = event.kind {
            *counts.entry((key, cents)).or_default() += 1;
        }
    }
    // Most common detune per key; ties go to the detune nearest zero.
    let mut best: BTreeMap<i32, (usize, i32)> = BTreeMap::new();
    for (&(key, cents), &count) in &counts {
        let entry = best.entry(key).or_insert((0, 0));
        if (count, -cents.abs()) > (entry.0, -entry.1.abs()) {
            *entry = (count, cents);
        }
    }
    track.tuning = best
        .into_iter()
        .filter(|(_, (_, cents))| *cents != 0)
        .map(|(key, (_, cents))| (key, cents))
        .collect();
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PitchedParam {
    pub name: String,
    /// Key, cents, and the register value for that pitch.
    pub values: Vec<(i32, i32, String)>,
}

impl PitchedParam {
    fn value(&self, key: i32, cents: i32) -> Option<&str> {
        self.values
            .iter()
            .find(|(k, c, _)| *k == key && *c == cents)
            .map(|(_, _, value)| value.as_str())
    }
}

/// Keep only the last value a command sets at each tick.
fn drop_overwritten(events: &mut Vec<Event>) {
    let mut keep = vec![true; events.len()];
    for (index, event) in events.iter().enumerate() {
        let Some((target, _)) = target_value(&event.kind) else { continue };
        keep[index] = !events[index + 1..]
            .iter()
            .take_while(|later| later.tick == event.tick)
            .any(|later| target_value(&later.kind).is_some_and(|(t, _)| t == target));
    }
    let mut index = 0;
    events.retain(|_| {
        index += 1;
        keep[index - 1]
    });
}

/// Move registers that are a function of the pitch into a per-track table.
/// A note's register value is the one in force at its onset; the settings
/// made at note onsets are dropped from the events and implied by the table.
fn extract_pitched(track: &mut Track) {
    // When notes overlap (chords, held notes) one register setting serves
    // several pitches.
    let mut end = 0;
    for event in track.events.iter().filter(|e| e.kind.length() > 0) {
        if event.tick < end {
            return;
        }
        end = event.tick + event.kind.length();
    }
    let mut names: Vec<String> = Vec::new();
    for event in &track.events {
        if let EventKind::Param(name, _) = &event.kind {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }
    }
    for name in names {
        let mut table = PitchedParam { name: name.clone(), values: Vec::new() };
        let mut current: Option<String> = None;
        let mut onset_settings = Vec::new();
        let mut consistent = true;
        for (index, event) in track.events.iter().enumerate() {
            match &event.kind {
                EventKind::Param(n, value) if *n == name => current = Some(value.clone()),
                EventKind::Note { key, cents, .. } => {
                    let Some(value) = current.clone() else {
                        consistent = false;
                        break;
                    };
                    match table.value(*key, *cents) {
                        Some(known) if known != value => {
                            consistent = false;
                            break;
                        }
                        Some(_) => {}
                        None => table.values.push((*key, *cents, value.clone())),
                    }
                    // The last setting at this tick, made for this note.
                    if let Some(setting) = (0..index).rev().take_while(|i| track.events[*i].tick == event.tick).find(|i| {
                        matches!(&track.events[*i].kind, EventKind::Param(n, _) if *n == name)
                    }) {
                        onset_settings.push(setting);
                    }
                }
                _ => {}
            }
        }
        // A register that never varies with the pitch is not pitch-tied.
        let distinct = table.values.iter().any(|(_, _, value)| *value != table.values[0].2);
        if !consistent || onset_settings.len() < 4 || !distinct {
            continue;
        }
        table.values.sort();
        let mut index = 0;
        track.events.retain(|_| {
            index += 1;
            !onset_settings.contains(&(index - 1))
        });
        track.pitched.push(table);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub tick: u64,
    pub kind: EventKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    /// `key` is the MIDI semitone and `cents` the onset detune (-50..=50).
    Note {
        key: i32,
        cents: i32,
        velocity: u8,
        legato: bool,
        length: u64,
    },
    Hit {
        velocity: u8,
        length: u64,
    },
    /// Pitch offset in cents from the sounding note's semitone.
    Bend(i32),
    Level(i32),
    Pan(i32),
    Instrument(String),
    Param(String, String),
    /// Start a macro from [`Score::macros`] at this tick.
    Macro(usize),
}

impl EventKind {
    fn length(&self) -> u64 {
        match self {
            Self::Note { length, .. } | Self::Hit { length, .. } => *length,
            _ => 0,
        }
    }
}

/// One sounding note in the piano roll.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollNote {
    pub channel: u32,
    pub voice: u32,
    /// MIDI key in cents (`key * 100 + cents`); `None` for unpitched hits.
    pub cents: Option<i32>,
    pub start: u64,
    pub end: u64,
    pub velocity: u8,
}

impl Score {
    pub fn tick_seconds(&self) -> f64 {
        60_000.0 / f64::from(self.tempo.max(1)) / f64::from(self.quarter.max(1))
    }

    /// Seconds from the song's start to `tick`.
    pub fn seconds_at(&self, tick: u64) -> f64 {
        seconds_at(&self.timing_seconds(), self.tick_seconds(), tick)
    }

    fn timing_seconds(&self) -> Vec<(u64, f64)> {
        self.timing.iter().map(|(tick, micros)| (*tick, *micros as f64 / 1e6)).collect()
    }

    pub fn bar_ticks(&self) -> u64 {
        u64::from(self.quarter.max(1)) * u64::from(self.beats.max(1))
    }

    /// The first bar line after `tick`. The pickup lengthens the first bar.
    pub fn next_bar(&self, tick: u64) -> u64 {
        let bar = self.bar_ticks();
        self.pickup + (tick.saturating_sub(self.pickup) / bar + 1) * bar
    }

    fn bar_start(&self, index: usize) -> u64 {
        if index == 0 { 0 } else { self.pickup + index as u64 * self.bar_ticks() }
    }

    fn bar_count(&self) -> usize {
        let mut count = 1;
        while self.bar_start(count) < self.length {
            count += 1;
        }
        count
    }

    pub fn piano_roll(&self) -> Vec<RollNote> {
        let mut roll = Vec::new();
        for track in &self.tracks {
            for event in &track.events {
                let (cents, velocity, length) = match &event.kind {
                    EventKind::Note {
                        key,
                        cents,
                        velocity,
                        length,
                        ..
                    } => (Some(key * 100 + cents), *velocity, *length),
                    EventKind::Hit { velocity, length } => (None, *velocity, *length),
                    _ => continue,
                };
                roll.push(RollNote {
                    channel: track.channel,
                    voice: track.voice,
                    cents,
                    start: event.tick,
                    end: event.tick + length,
                    velocity,
                });
            }
        }
        roll.sort_by_key(|note| (note.start, note.channel, note.voice));
        roll
    }
}

// ---------------------------------------------------------------------------
// Recording frames -> score

#[derive(Clone)]
struct Sounding {
    key: i32,
    voice: usize,
    start: u64,
    event: usize,
    bend: i32,
}

#[derive(Clone, Default)]
struct ChannelState {
    /// Pitches are sample playback rates without a known tuning.
    relative: bool,
    name: String,
    kind: String,
    voices: Vec<Option<i32>>,
    sounding: Vec<Sounding>,
    hit: Option<(u64, usize)>,
    key_on: Option<String>,
    instrument: Option<String>,
    level: Option<i32>,
    pan: Option<i32>,
    fields: BTreeMap<String, String>,
    /// Events per voice, before they become tracks.
    events: Vec<Vec<Event>>,
}

impl ChannelState {
    fn voice_events(&mut self, voice: usize) -> &mut Vec<Event> {
        if self.events.len() <= voice {
            self.events.resize_with(voice + 1, Vec::new);
        }
        &mut self.events[voice]
    }

    fn free_voice(&mut self, key: i32) -> usize {
        let voice = self
            .voices
            .iter()
            .position(Option::is_none)
            .unwrap_or(self.voices.len());
        if voice == self.voices.len() {
            self.voices.push(None);
        }
        self.voices[voice] = Some(key);
        voice
    }

    fn end(&mut self, index: usize, tick: u64) {
        let sounding = self.sounding.remove(index);
        self.voices[sounding.voice] = None;
        if let EventKind::Note { length, .. } =
            &mut self.events[sounding.voice][sounding.event].kind
        {
            *length = tick.saturating_sub(sounding.start).max(1);
        }
    }

    fn end_hit(&mut self, tick: u64) {
        if let Some((start, event)) = self.hit.take() {
            if let EventKind::Hit { length, .. } = &mut self.events[0][event].kind {
                *length = tick.saturating_sub(start).max(1);
            }
        }
    }
}

const KEY_FIELDS: [&str; 2] = ["Gate", "Key on"];

fn gate(channel: &crate::Channel) -> bool {
    channel.active && channel.field("Gate") != Some("Off")
}

fn unit(value: f32) -> i32 {
    if value.is_finite() {
        (value.clamp(-1.0, 1.0) * 1000.0).round() as i32
    } else {
        0
    }
}

fn velocity(value: f32) -> u8 {
    if value.is_finite() {
        (value.clamp(0.0, 1.0) * 127.0).round() as u8
    } else {
        0
    }
}

/// Values that move on nearly every frame (envelopes, sample addresses,
/// measured levels) are recorded at key-on only. Writing every 5 ms sample of
/// them would bury the notes; the live values remain in the channel inspector.
/// Rates are measured as frames arrive, so long songs need not be buffered.
#[derive(Clone, Default)]
struct Continuity {
    previous: HashMap<u32, crate::Channel>,
    active: HashMap<u32, u32>,
    changes: HashMap<(u32, String), u32>,
}

impl Continuity {
    fn observe(&mut self, frame: &TimedFrame) {
        for channel in &frame.data.channels {
            if gate(channel) {
                *self.active.entry(channel.id).or_default() += 1;
                if let Some(old) = self.previous.get(&channel.id).filter(|old| gate(old)) {
                    let mut changed = |name: &str| {
                        *self
                            .changes
                            .entry((channel.id, name.to_owned()))
                            .or_default() += 1;
                    };
                    if unit(old.level) != unit(channel.level) {
                        changed("\0level");
                    }
                    if unit(old.pan) != unit(channel.pan) {
                        changed("\0pan");
                    }
                    for field in &channel.fields {
                        if old.field(&field.name) != Some(field.value.as_str()) {
                            changed(&field.name);
                        }
                    }
                }
            }
        }
    }

    /// Keep this frame's channels for the next comparison, without copying.
    fn remember(&mut self, frame: TimedFrame) {
        for channel in frame.data.channels {
            self.previous.insert(channel.id, channel);
        }
    }

    fn continuous(&self, channel: u32, name: &str) -> bool {
        let active = self.active.get(&channel).copied().unwrap_or_default();
        let changes = self
            .changes
            .get(&(channel, name.to_owned()))
            .copied()
            .unwrap_or_default();
        active >= 16 && changes * 2 > active
    }
}

fn mode(values: impl Iterator<Item = u64>) -> Option<u64> {
    let mut counts = BTreeMap::<u64, usize>::new();
    for value in values {
        *counts.entry(value).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by_key(|(value, count)| (*count, std::cmp::Reverse(*value)))
        .map(|(value, _)| value)
}

fn tempo_bpm(frame: &TimedFrame) -> Option<f64> {
    frame
        .data
        .global
        .iter()
        .find(|field| field.name == "Tempo")
        .and_then(|field| field.value.split_whitespace().next()?.parse::<f64>().ok())
        .filter(|bpm| bpm.is_finite() && *bpm > 0.0)
}

fn meter(frame: &TimedFrame) -> Option<u32> {
    let field = frame
        .data
        .global
        .iter()
        .find(|field| field.name == "Event FF:58")?;
    let bytes: Vec<u32> = field
        .value
        .split_whitespace()
        .filter_map(|byte| u32::from_str_radix(byte, 16).ok())
        .collect();
    let (numerator, denominator) = (*bytes.first()?, *bytes.get(1)?);
    // Quarters per bar: 6/8 is three quarters.
    (numerator > 0 && denominator < 8).then(|| numerator * 4 / (1 << denominator))
}

/// Build the score from every frame a decoder produced for one track.
pub fn score_from_frames(
    frames: &[TimedFrame],
    description: &Description,
    title: &str,
    rate: u32,
    duration: Option<f64>,
) -> Score {
    let mut builder = ScoreBuilder::new(rate);
    for frame in frames {
        builder.push(frame.clone());
    }
    builder.finish(description, title, duration)
}

/// Frames used to find the decoder's frame step before streaming begins.
const WARMUP_FRAMES: usize = 256;

/// Turns a decoder's frames into a score as they arrive, so a long song never
/// has to be held in memory as raw channel snapshots.
#[derive(Clone)]
pub struct ScoreBuilder {
    rate: u32,
    warmup: Vec<TimedFrame>,
    timebase: Option<(u32, u32, f64)>,
    continuity: Continuity,
    channels: BTreeMap<u32, ChannelState>,
    last_tick: u64,
    tempo: Option<f64>,
    meter: Option<u32>,
}

impl ScoreBuilder {
    pub fn new(rate: u32) -> Self {
        Self {
            rate: rate.max(1),
            warmup: Vec::new(),
            timebase: None,
            continuity: Continuity::default(),
            channels: BTreeMap::new(),
            last_tick: 0,
            tempo: None,
            meter: None,
        }
    }

    pub fn push(&mut self, frame: TimedFrame) {
        if self.timebase.is_some() {
            self.process(&frame);
            self.continuity.remember(frame);
            return;
        }
        self.warmup.push(frame);
        if self.warmup.len() >= WARMUP_FRAMES {
            self.start();
        }
    }

    /// Decoders report frames at their own rate (for example every 220
    /// samples at 44.1 kHz), which is rarely a whole number of output samples.
    /// Scale the timebase so the typical frame step is a whole number of ticks.
    fn start(&mut self) {
        let rate = self.rate;
        let mut steps: Vec<f64> = self
            .warmup
            .windows(2)
            .map(|pair| (pair[1].time - pair[0].time) * f64::from(rate))
            .filter(|step| step.is_finite() && *step > 0.5)
            .collect();
        steps.sort_by(f64::total_cmp);
        let step = steps
            .get(steps.len() / 2)
            .copied()
            .unwrap_or(f64::from(rate) / 200.0);
        let scale = (1..=20u32)
            .find(|scale| {
                let scaled = step * f64::from(*scale);
                (scaled - scaled.round()).abs() < 0.02
            })
            .unwrap_or(1);
        let tick_samples = ((step * f64::from(scale)).round() as u32).max(1);
        let rate = rate * scale;
        self.timebase = Some((rate, tick_samples, f64::from(tick_samples) / f64::from(rate)));
        for frame in std::mem::take(&mut self.warmup) {
            self.process(&frame);
            self.continuity.remember(frame);
        }
    }

    fn to_tick(&self, time: f64) -> u64 {
        let tick_seconds = self.timebase.map_or(1.0, |(_, _, seconds)| seconds);
        (time.max(0.0) / tick_seconds).round() as u64
    }

    fn process(&mut self, frame: &TimedFrame) {
        if self.tempo.is_none() {
            self.tempo = tempo_bpm(frame);
        }
        if self.meter.is_none() {
            self.meter = meter(frame);
        }
        self.continuity.observe(frame);
        let continuity = &self.continuity;
        let tick = self.to_tick(frame.time);
        let channels = &mut self.channels;
        self.last_tick = self.last_tick.max(tick);
        let present: Vec<u32> = frame.data.channels.iter().map(|c| c.id).collect();
        for (id, state) in channels.iter_mut() {
            if !present.contains(id) {
                while !state.sounding.is_empty() {
                    state.end(0, tick);
                }
                state.end_hit(tick);
            }
        }
        for channel in &frame.data.channels {
            let state = channels.entry(channel.id).or_default();
            state.name.clone_from(&channel.name);
            state.relative |= channel.has_relative_pitch();
            if state.kind.is_empty() || !channel.notes.is_empty() {
                state.kind.clone_from(&channel.kind);
            }
            let down = gate(channel);
            let key_on = channel.field("Key on").map(str::to_owned);
            let retrigger = down && key_on.is_some() && state.key_on.is_some() && key_on != state.key_on;
            state.key_on = key_on;
            let mut keys: Vec<(i32, i32, u8)> = if down {
                channel
                    .notes
                    .iter()
                    .filter(|note| note.key.is_finite())
                    .map(|note| {
                        let cents = (f64::from(note.key) * 100.0).round() as i32;
                        let key = cents.div_euclid(100) + i32::from(cents.rem_euclid(100) > 50);
                        (key, cents - key * 100, velocity(note.velocity))
                    })
                    .collect()
            } else {
                Vec::new()
            };
            keys.sort_unstable();
            keys.dedup_by_key(|(key, _, _)| *key);

            // Releases and retriggers end notes before new commands apply.
            let mut index = 0;
            while index < state.sounding.len() {
                let key = state.sounding[index].key;
                if retrigger || !keys.iter().any(|(k, _, _)| *k == key) {
                    state.end(index, tick);
                } else {
                    index += 1;
                }
            }
            let hit_down = down && channel.notes.is_empty();
            if !hit_down || retrigger {
                state.end_hit(tick);
            }

            let onset = (keys.len() > state.sounding.len()) || (hit_down && state.hit.is_none());
            let mut commands = Vec::new();
            if channel.active {
                if state.instrument.as_deref() != Some(channel.instrument.as_str()) {
                    if !(channel.instrument.is_empty() && state.instrument.is_none()) {
                        commands.push(EventKind::Instrument(channel.instrument.clone()));
                    }
                    state.instrument = Some(channel.instrument.clone());
                }
                let value_change = |name: &str, value: i32, last: &mut Option<i32>| {
                    if *last != Some(value) && (onset || !continuity.continuous(channel.id, name)) {
                        *last = Some(value);
                        Some(value)
                    } else {
                        None
                    }
                };
                if let Some(level) = value_change("\0level", unit(channel.level), &mut state.level) {
                    commands.push(EventKind::Level(level));
                }
                if let Some(pan) = value_change("\0pan", unit(channel.pan), &mut state.pan) {
                    commands.push(EventKind::Pan(pan));
                }
                for field in &channel.fields {
                    if KEY_FIELDS.contains(&field.name.as_str()) {
                        continue;
                    }
                    let changed = state.fields.get(&field.name) != Some(&field.value);
                    if changed && (onset || !continuity.continuous(channel.id, &field.name)) {
                        state.fields.insert(field.name.clone(), field.value.clone());
                        commands.push(EventKind::Param(field.name.clone(), field.value.clone()));
                    }
                }
            }
            if !commands.is_empty() {
                let events = state.voice_events(0);
                events.extend(commands.into_iter().map(|kind| Event { tick, kind }));
            }

            // Pitch movement of a held key is a bend, not another note.
            for sounding in &mut state.sounding {
                if let Some((_, cents, _)) = keys.iter().find(|(k, _, _)| *k == sounding.key) {
                    if *cents != sounding.bend {
                        sounding.bend = *cents;
                        let voice = sounding.voice;
                        if state.events.len() <= voice {
                            state.events.resize_with(voice + 1, Vec::new);
                        }
                        state.events[voice].push(Event {
                            tick,
                            kind: EventKind::Bend(*cents),
                        });
                    }
                }
            }
            let ended_here = |state: &ChannelState| {
                state.events.iter().any(|events| {
                    events.iter().rev().take(4).any(|event| {
                        matches!(event.kind, EventKind::Note { .. })
                            && event.tick + event.kind.length() == tick
                            && tick > event.tick
                    })
                })
            };
            // Without key-on counters a pitch change is indistinguishable from
            // a new articulation, so only chips that report them get legato.
            let legato = down && !retrigger && state.key_on.is_some() && ended_here(state);
            for (key, cents, velocity) in keys {
                if state.sounding.iter().any(|sounding| sounding.key == key) {
                    continue;
                }
                let voice = state.free_voice(key);
                let events = state.voice_events(voice);
                events.push(Event {
                    tick,
                    kind: EventKind::Note {
                        key,
                        cents,
                        velocity,
                        legato,
                        length: 0,
                    },
                });
                let event = events.len() - 1;
                state.sounding.push(Sounding {
                    key,
                    voice,
                    start: tick,
                    event,
                    bend: cents,
                });
            }
            if hit_down && state.hit.is_none() {
                let events = state.voice_events(0);
                events.push(Event {
                    tick,
                    kind: EventKind::Hit {
                        velocity: velocity(channel.level),
                        length: 0,
                    },
                });
                state.hit = Some((tick, events.len() - 1));
            }
        }
    }

    pub fn finish(mut self, description: &Description, title: &str, duration: Option<f64>) -> Score {
        if self.timebase.is_none() {
            self.start();
        }
        let tick_seconds = self.timebase.map_or(1.0, |(_, _, seconds)| seconds);
        let last_tick = self.last_tick;
        let end = duration
            .map(|seconds| self.to_tick(seconds))
            .unwrap_or(last_tick)
            .max(last_tick + 1);
        let mut channels = std::mem::take(&mut self.channels);
        for state in channels.values_mut() {
            while !state.sounding.is_empty() {
                state.end(0, end);
            }
            state.end_hit(end);
        }

        let mut tracks = Vec::new();
        let mut relative = Vec::new();
        for (id, state) in channels {
            for (voice, events) in state.events.into_iter().enumerate() {
                if events.is_empty() && voice > 0 {
                    continue;
                }
                relative.push(state.relative);
                tracks.push(Track {
                    label: String::new(),
                    channel: id,
                    voice: voice as u32,
                    name: state.name.clone(),
                    kind: if state.kind.is_empty() {
                        "tonal".into()
                    } else {
                        state.kind.clone()
                    },
                    events,
                    pitched: Vec::new(),
                    tuning: Vec::new(),
                });
            }
        }
        let mut index = 0;
        relative.retain(|_| {
            index += 1;
            !tracks[index - 1].events.is_empty()
        });
        tracks.retain(|track| !track.events.is_empty());

        // Frames arrive every few milliseconds, but music moves on a beat.
        // Follow the beat through the song (tempo drifts, and some decoders
        // report key-ons tens of milliseconds late), give each onset its place
        // on that beat, and keep the time of each beat for highlighting.
        let grid = match self.tempo {
            Some(bpm) => Grid { quarter: (60.0 / bpm) / tick_seconds, steps: 4.0, jitter: 1.0 },
            None => infer_grid(&tracks, tick_seconds),
        };
        let steps = grid.steps;
        let unit_frames = grid.quarter.max(1.0) / steps;
        let unit_ticks = (f64::from(QUARTER) / steps).round() as u64;
        // Finer lines between beat steps (halves, triplets, quarters …) that
        // divide into whole ticks, coarsest first.
        // Finer lines would also catch late or early key-ons, so stop at quarters.
        let subdivisions: Vec<u64> = [1u64, 2, 3, 4]
            .into_iter()
            .filter(|parts| unit_ticks % parts == 0)
            .collect();
        let mut onsets: Vec<u64> = tracks
            .iter()
            .flat_map(|track| track.events.iter())
            .filter(|event| event.kind.length() > 0)
            .map(|event| event.tick)
            .collect();
        onsets.sort_unstable();
        onsets.dedup();
        let anchors = track_beats(&onsets, unit_frames, unit_ticks, &subdivisions, grid.jitter);
        let timeline = Timeline { anchors, unit_frames, unit_ticks };
        // Anything off the beat still lands on a quarter of a beat step, so
        // stray key-ons, releases and commands do not leave 1/128 slivers.
        let resolution = [4u64, 2, 1].into_iter().find(|parts| unit_ticks % parts == 0).map_or(1, |parts| unit_ticks / parts);
        let on_grid = |tick: u64| (tick as f64 / resolution as f64).round() as u64 * resolution;
        // Onsets the beat tracker could not place exactly still snap to the
        // nearest beat, half or third of one when they are close to it.
        let near_line = |tick: u64| {
            for parts in &subdivisions {
                let line = (unit_ticks / parts) as f64;
                let nearest = (tick as f64 / line).round();
                if (tick as f64 - nearest * line).abs() <= 0.35 * line {
                    return (nearest * line) as u64;
                }
            }
            on_grid(tick)
        };
        let place = |frame: u64| {
            let tick = timeline.tick(frame as f64);
            if timeline.is_anchor(frame as f64) { tick } else { near_line(tick) }
        };
        let snap_end = |frame: u64| {
            let tick = timeline.tick(frame as f64);
            let nearest = (tick as f64 / unit_ticks as f64).round() as u64 * unit_ticks;
            // Releases are looser than key-ons: a note let go just before the
            // next beat still ends on it.
            if (tick as f64 - nearest as f64).abs() <= 0.45 * unit_ticks as f64 { nearest } else { near_line(tick) }
        };
        for track in &mut tracks {
            // Commands inside a note keep full resolution so envelopes and
            // vibrato stay intact (they become macros); commands between notes
            // snap like key-ons so they do not split rests into slivers.
            let sounding: Vec<(u64, u64)> = track
                .events
                .iter()
                .filter(|event| event.kind.length() > 0)
                .map(|event| (event.tick, event.tick + event.kind.length()))
                .collect();
            let inside = |frame: u64| {
                // A command at a note's own onset moves with the note.
                let after = sounding.partition_point(|(start, _)| *start < frame);
                after > 0 && frame < sounding[after - 1].1
            };
            let mut events: Vec<Event> = std::mem::take(&mut track.events)
                .into_iter()
                .filter_map(|mut event| {
                    let start = if event.kind.length() == 0 && inside(event.tick) {
                        timeline.tick(event.tick as f64)
                    } else {
                        place(event.tick)
                    };
                    match &mut event.kind {
                        EventKind::Note { length, .. } | EventKind::Hit { length, .. } => {
                            let end = snap_end(event.tick + *length);
                            let end = if end > start { end } else { place(event.tick + *length).max(start + resolution) };
                            *length = end.saturating_sub(start);
                            if *length == 0 {
                                return None;
                            }
                        }
                        _ => {}
                    }
                    event.tick = start;
                    Some(event)
                })
                .collect();
            // A release snapped past the next note's key-on ends at the key-on,
            // and each sound starts after the previous one ends.
            let starts: Vec<u64> = events.iter().filter(|e| e.kind.length() > 0).map(|e| e.tick).collect();
            let mut next = 1;
            for event in &mut events {
                if let EventKind::Note { length, .. } | EventKind::Hit { length, .. } = &mut event.kind {
                    if let Some(following) = starts.get(next) {
                        if event.tick < *following && event.tick + *length > *following {
                            *length = following - event.tick;
                        }
                    }
                    next += 1;
                }
            }
            let mut previous_end = 0;
            events.retain_mut(|event| {
                let length = event.kind.length();
                if length == 0 {
                    return true;
                }
                let start = event.tick.max(previous_end);
                let end = (event.tick + length).max(start);
                if end == start {
                    return false;
                }
                if let EventKind::Note { length, .. } | EventKind::Hit { length, .. } = &mut event.kind {
                    *length = end - start;
                }
                event.tick = start;
                previous_end = end;
                true
            });
            events.sort_by_key(|event| (event.tick, event.kind.length() > 0));
            track.events = events;
        }
        let length = snap_end(end).max(1);
        // Nothing may fall after the song's end.
        for track in &mut tracks {
            track.events.retain_mut(|event| {
                event.tick = event.tick.min(length);
                if let EventKind::Note { length: l, .. } | EventKind::Hit { length: l, .. } = &mut event.kind {
                    *l = (*l).min(length - event.tick);
                    return *l > 0;
                }
                true
            });
        }
        // Notes of a polyphonic channel that start and end together are one
        // chord on one track, not one track per voice.
        let relative_of: BTreeMap<u32, bool> =
            tracks.iter().zip(&relative).map(|(track, relative)| (track.channel, *relative)).collect();
        let mut tracks = merge_chords(tracks);
        let relative: Vec<bool> = tracks.iter().map(|track| relative_of[&track.channel]).collect();
        // Start bars where most notes start: the beat tracker knows the beat
        // but not which beat is the downbeat.
        let bar = u64::from(QUARTER) * u64::from(self.meter.unwrap_or(4).max(1));
        // Chords mark strong beats: weight each onset tick by the square of
        // how many voices start on it.
        let mut starts: BTreeMap<u64, usize> = BTreeMap::new();
        for track in &tracks {
            for event in track.events.iter().filter(|e| e.kind.length() > 0) {
                *starts.entry(event.tick).or_default() += 1;
            }
        }
        let mut residues: BTreeMap<u64, usize> = BTreeMap::new();
        for (tick, voices) in starts {
            // Only beat lines can be downbeats.
            if tick % unit_ticks == 0 {
                *residues.entry(tick % bar).or_default() += voices * voices;
            }
        }
        let pickup = residues
            .into_iter()
            .max_by_key(|(residue, count)| (*count, std::cmp::Reverse(*residue)))
            .map_or(0, |(residue, _)| residue);
        let timing = timeline.timing(tick_seconds, length);
        let seconds = timing.last().map_or(0, |(_, micros)| *micros) as f64 / 1e6;
        let ticks = timing.last().map_or(0, |(tick, _)| *tick) as f64;
        let tempo = if ticks > 0.0 && seconds > 0.0 {
            (60_000.0 * ticks / f64::from(QUARTER) / seconds).round().max(1.0) as u32
        } else {
            (60_000.0 / (unit_frames * steps * tick_seconds)).round().max(1.0) as u32
        };

        // Values that move on most frames (sample playback addresses, raw
        // envelope levels) are measurements, not settings a score could
        // reproduce; the channel inspector still shows them live.
        for track in &mut tracks {
            let channel = track.channel;
            let continuity = &self.continuity;
            track.events.retain(|event| {
                !matches!(&event.kind, EventKind::Param(name, _) if continuity.continuous(channel, name))
            });
            decimal_params(&mut track.events);
        }
        let transpose = relative_transpose(&tracks, &relative);
        for track in &mut tracks {
            drop_overwritten(&mut track.events);
            extract_pitched(track);
            extract_tuning(track);
        }
        let mut macros = Vec::new();
        for track in &mut tracks {
            fold_macros(&mut track.events, &mut macros);
        }
        for (index, track) in tracks.iter_mut().enumerate() {
            track.label = track_label(index);
        }
        Score {
            title: title.into(),
            backend: description.backend.clone(),
            tempo,
            quarter: QUARTER,
            beats: self.meter.unwrap_or(4).max(1),
            inferred: self.tempo.is_none(),
            length,
            tracks,
            macros,
            transpose,
            pickup,
            timing,
        }
    }
}

/// A polyphonic channel (a MIDI channel, a tracker channel with background
/// voices) becomes one track: notes that start together are a chord, and a
/// note may hold on while later notes play. Chips with one voice per channel
/// keep their one track, as do channels with drum hits.
fn merge_chords(tracks: Vec<Track>) -> Vec<Track> {
    let mut channels: Vec<(u32, Vec<Track>)> = Vec::new();
    for track in tracks {
        match channels.iter_mut().find(|(id, _)| *id == track.channel) {
            Some((_, group)) => group.push(track),
            None => channels.push((track.channel, vec![track])),
        }
    }
    let mut out = Vec::new();
    for (_, group) in channels {
        match merge_channel(&group) {
            Some(merged) => out.push(merged),
            None => out.extend(group),
        }
    }
    out
}

fn merge_channel(group: &[Track]) -> Option<Track> {
    if group.len() < 2 || group.iter().flat_map(|t| &t.events).any(|e| matches!(e.kind, EventKind::Hit { .. })) {
        return None;
    }
    let mut events = Vec::new();
    let mut bends: BTreeMap<u64, i32> = BTreeMap::new();
    // Every voice of a MIDI channel bends together; keep one copy. Frames a
    // few milliseconds apart can land on one tick with different values, so
    // the voice that saw the most of the bend decides.
    let mut voices: Vec<&Track> = group.iter().collect();
    voices.sort_by_key(|track| std::cmp::Reverse(track.events.iter().filter(|e| matches!(e.kind, EventKind::Bend(_))).count()));
    for event in voices.iter().flat_map(|track| &track.events) {
        match event.kind {
            EventKind::Bend(cents) => {
                bends.entry(event.tick).or_insert(cents);
            }
            _ => events.push(event.clone()),
        }
    }
    events.extend(bends.into_iter().map(|(tick, cents)| Event { tick, kind: EventKind::Bend(cents) }));
    // Commands first at each tick, then the chord's notes from low to high.
    events.sort_by_key(|event| {
        let key = match event.kind {
            EventKind::Note { key, cents, .. } => (key, cents),
            _ => (0, 0),
        };
        (event.tick, event.kind.length() > 0, key)
    });
    let first = &group[0];
    Some(Track {
        label: String::new(),
        channel: first.channel,
        voice: 0,
        name: first.name.clone(),
        kind: first.kind.clone(),
        events,
        pitched: Vec::new(),
        tuning: Vec::new(),
    })
}

/// How many notes start together at `index` (a chord); 1 for a lone note or
/// a hit.
fn group_len(events: &[Event], index: usize) -> usize {
    let Event { tick, kind } = &events[index];
    if !matches!(kind, EventKind::Note { .. }) {
        return 1;
    }
    1 + events[index + 1..]
        .iter()
        .take_while(|e| e.tick == *tick && matches!(e.kind, EventKind::Note { .. }))
        .count()
}

/// Where the sound at `index` (with `count` notes) hands over to what follows:
/// its longest note's end, or the next onset if that comes first. Notes that
/// end there are written without a length of their own and follow the
/// track; others carry their own length.
fn follow_end(events: &[Event], index: usize, count: usize) -> u64 {
    let group = &events[index..index + count];
    let end = group.iter().map(|e| e.tick + e.kind.length()).max().unwrap_or(0);
    let next = events[index + count..].iter().find(|e| e.kind.length() > 0).map_or(u64::MAX, |e| e.tick);
    end.min(next)
}

/// Give each onset its place on the beat. Onsets near a beat line (or a half,
/// third, quarter … of one) become anchors; the step length follows the
/// measured gaps between anchors so slow tempo drift does not accumulate.
fn track_beats(onsets: &[u64], unit: f64, unit_ticks: u64, subdivisions: &[u64], jitter: f64) -> Vec<(f64, u64)> {
    let Some(&first) = onsets.first() else { return Vec::new() };
    let mut anchors = vec![(first as f64, 0u64)];
    let mut step = unit;
    for &onset in &onsets[1..] {
        let (at, tick) = *anchors.last().unwrap();
        let steps_in = (onset as f64 - at) / step;
        for parts in subdivisions {
            let lines = steps_in * *parts as f64;
            let nearest = lines.round();
            let line = step / *parts as f64;
            // Coarse lines first, each with a window of 30% of its spacing;
            // the jitter estimate only widens it for very regular songs.
            // Over a long held chord the tempo may have drifted a few percent, so
            // whole beat steps get extra room in proportion to the gap.
            let drift = if *parts == 1 { 0.04 * (onset as f64 - at) } else { 0.0 };
            let tolerance = (0.3 * line).max((1.5 * jitter).min(0.4 * line)) + drift;
            if nearest >= 1.0 && (lines - nearest).abs() * line <= tolerance {
                let ticks = nearest as u64 * (unit_ticks / parts);
                // Whole steps measure the tempo; adapt gently and stay near the
                // detected step so one late key-on cannot run away with it.
                if ticks >= unit_ticks {
                    let measured = (onset as f64 - at) * unit_ticks as f64 / ticks as f64;
                    step = (0.8 * step + 0.2 * measured).clamp(unit * 0.9, unit * 1.1);
                }
                anchors.push((onset as f64, tick + ticks));
                break;
            }
        }
    }
    anchors
}

/// Frame-to-tick mapping through the beat anchors.
struct Timeline {
    anchors: Vec<(f64, u64)>,
    unit_frames: f64,
    unit_ticks: u64,
}

impl Timeline {
    fn is_anchor(&self, frame: f64) -> bool {
        self.anchors.binary_search_by(|(at, _)| at.total_cmp(&frame)).is_ok()
    }

    fn tick(&self, frame: f64) -> u64 {
        let per_frame = self.unit_ticks as f64 / self.unit_frames;
        let Some(&(first_frame, first_tick)) = self.anchors.first() else {
            return (frame * per_frame).round().max(0.0) as u64;
        };
        if frame <= first_frame {
            return (first_tick as f64 - (first_frame - frame) * per_frame).round().max(0.0) as u64;
        }
        let after = self.anchors.partition_point(|(at, _)| *at <= frame);
        if after == self.anchors.len() {
            let (at, tick) = self.anchors[after - 1];
            let rate = if after >= 2 {
                let (before, earlier) = self.anchors[after - 2];
                (tick - earlier) as f64 / (at - before)
            } else {
                per_frame
            };
            return tick + ((frame - at) * rate).round() as u64;
        }
        let (a_frame, a_tick) = self.anchors[after - 1];
        let (b_frame, b_tick) = self.anchors[after];
        a_tick + ((frame - a_frame) / (b_frame - a_frame) * (b_tick - a_tick) as f64).round() as u64
    }

    /// Tick and time (microseconds) pairs that reproduce the beat's timing to
    /// within 5 ms, plus the song's end.
    fn timing(&self, tick_seconds: f64, length: u64) -> Vec<(u64, u64)> {
        let mut points: Vec<(u64, f64)> = vec![(0, 0.0)];
        let first_seconds = self.anchors.first().map_or(0.0, |(frame, _)| frame * tick_seconds);
        if let Some(&(frame, tick)) = self.anchors.first() {
            if tick == 0 && frame > 0.0 {
                // Time before the first onset belongs to bar one.
                points[0] = (0, first_seconds);
            }
        }
        points.extend(self.anchors.iter().skip(1).map(|(frame, tick)| (*tick, frame * tick_seconds)));
        if points.last().is_some_and(|(tick, _)| *tick < length) {
            let seconds_per_tick = self.unit_frames * tick_seconds / self.unit_ticks as f64;
            let (tick, at) = *points.last().unwrap();
            points.push((length, at + (length - tick) as f64 * seconds_per_tick));
        }
        simplify(&points, 0.005)
            .into_iter()
            .map(|(tick, seconds)| (tick, (seconds * 1e6).round().max(0.0) as u64))
            .collect()
    }
}

/// Drop timing points that a straight line between their neighbours already
/// gives to within `tolerance` seconds (Douglas–Peucker).
fn simplify(points: &[(u64, f64)], tolerance: f64) -> Vec<(u64, f64)> {
    if points.len() <= 2 {
        return points.to_vec();
    }
    let (first, last) = (points[0], points[points.len() - 1]);
    let span = (last.0 - first.0).max(1) as f64;
    let (worst, distance) = points[1..points.len() - 1]
        .iter()
        .enumerate()
        .map(|(index, (tick, seconds))| {
            let expected = first.1 + (last.1 - first.1) * (*tick - first.0) as f64 / span;
            (index + 1, (seconds - expected).abs())
        })
        .fold((0, 0.0), |best, item| if item.1 > best.1 { item } else { best });
    if distance <= tolerance {
        return vec![first, last];
    }
    let mut left = simplify(&points[..=worst], tolerance);
    left.pop();
    left.extend(simplify(&points[worst..], tolerance));
    left
}

/// Registers are reported in hex. Write a parameter in decimal when every
/// value it takes is made only of hex numbers (separated by spaces, `/` or
/// `:`) and at least one of them uses a hex letter, so `Period=0FD` becomes
/// `Period=253` and `"Volume L/R"=02EE/0120` becomes `750/288`. Values wider
/// than 16 bits stay hex: they are memory addresses or packed registers,
/// which read better that way.
fn decimal_params(events: &mut [Event]) {
    let mut names: BTreeMap<String, bool> = BTreeMap::new();
    for event in events.iter() {
        if let EventKind::Param(name, value) = &event.kind {
            let tokens: Vec<&str> = value.split([' ', '/', ':']).filter(|t| !t.is_empty()).collect();
            let all_hex = !tokens.is_empty() && tokens.iter().all(|t| t.bytes().all(|b| b.is_ascii_hexdigit()) && t.len() <= 4);
            let letters = tokens.iter().any(|t| t.bytes().any(|b| matches!(b, b'A'..=b'F' | b'a'..=b'f')));
            let zero_padded = tokens.iter().any(|t| t.len() > 1 && t.starts_with('0'));
            let entry = names.entry(name.clone()).or_insert(true);
            *entry &= all_hex;
            if all_hex && (letters || zero_padded) {
                names.entry(format!("\0{name}")).or_insert(true);
            }
        }
    }
    let convert: Vec<&String> = names
        .iter()
        .filter(|(name, hex)| **hex && !name.starts_with('\0') && names.contains_key(&format!("\0{name}")))
        .map(|(name, _)| name)
        .collect();
    if convert.is_empty() {
        return;
    }
    let convert: Vec<String> = convert.into_iter().cloned().collect();
    // In mixed values such as "7FFF7FFF state 4", only tokens that are clearly
    // hex numbers (hex letters and at least one digit) are converted.
    let clearly_hex = |token: &str| {
        token.len() >= 3
            && token.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'A'..=b'F'))
            && token.bytes().any(|b| b.is_ascii_digit())
            && token.bytes().any(|b| matches!(b, b'A'..=b'F'))
            && token.len() <= 4
    };
    for event in events.iter_mut() {
        if let EventKind::Param(name, value) = &mut event.kind {
            if !convert.contains(name) {
                if value.split(' ').any(clearly_hex) {
                    *value = value
                        .split(' ')
                        .map(|token| {
                            if clearly_hex(token) {
                                u64::from_str_radix(token, 16).unwrap_or(0).to_string()
                            } else {
                                token.to_owned()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                }
                continue;
            }
            {
                let mut out = String::new();
                let mut digits = String::new();
                for character in value.chars().chain(std::iter::once(' ')) {
                    if character.is_ascii_hexdigit() {
                        digits.push(character);
                    } else {
                        if !digits.is_empty() {
                            out.push_str(&u64::from_str_radix(&digits, 16).unwrap_or(0).to_string());
                            digits.clear();
                        }
                        out.push(character);
                    }
                }
                out.pop();
                *value = out;
            }
        }
    }
}

/// Seconds at `tick` through `timing` points, or at a fixed rate without them.
fn seconds_at(timing: &[(u64, f64)], tick_seconds: f64, tick: u64) -> f64 {
    let after = timing.partition_point(|(at, _)| *at <= tick);
    match (after.checked_sub(1).map(|i| timing[i]), timing.get(after)) {
        (Some((a, sa)), Some((b, sb))) => sa + (sb - sa) * (tick - a) as f64 / (b - a).max(1) as f64,
        (Some((a, sa)), None) => sa + (tick - a) as f64 * tick_seconds,
        (None, Some((b, sb))) => sb - (b - tick) as f64 * tick_seconds,
        (None, None) => tick as f64 * tick_seconds,
    }
}

/// Ticks per quarter note in recorded scores: 1/128 notes and triplets of
/// every note value down to 1/96 are whole numbers of ticks.
pub const QUARTER: u32 = 96;

/// Sample playback rates give pitches relative to an unknown original key, so
/// they can sit many octaves away from the music. Move each sample by whole
/// octaves until its middle note is near middle C.
fn relative_transpose(tracks: &[Track], relative: &[bool]) -> Vec<(String, i32)> {
    let mut keys: BTreeMap<String, Vec<i32>> = BTreeMap::new();
    for (track, _) in tracks.iter().zip(relative).filter(|(_, relative)| **relative) {
        let mut instrument = String::new();
        for event in &track.events {
            match &event.kind {
                EventKind::Instrument(name) => instrument.clone_from(name),
                EventKind::Note { key, .. } if !instrument.is_empty() => {
                    keys.entry(instrument.clone()).or_default().push(*key);
                }
                _ => {}
            }
        }
    }
    keys.into_iter()
        .filter_map(|(instrument, mut keys)| {
            keys.sort_unstable();
            let middle = keys[keys.len() / 2];
            let shift = ((60 - middle) as f64 / 12.0).round() as i32 * 12;
            (shift != 0).then_some((instrument, shift))
        })
        .collect()
}

fn target_value(kind: &EventKind) -> Option<(Target, String)> {
    Some(match kind {
        EventKind::Level(value) => (Target::Level, value.to_string()),
        EventKind::Pan(value) => (Target::Pan, value.to_string()),
        EventKind::Bend(value) => (Target::Bend, value.to_string()),
        EventKind::Param(name, value) => (Target::Param(name.clone()), value.clone()),
        _ => return None,
    })
}

fn target_event(target: &Target, value: &str) -> Option<EventKind> {
    Some(match target {
        Target::Level => EventKind::Level(value.parse().ok()?),
        Target::Pan => EventKind::Pan(value.parse().ok()?),
        Target::Bend => EventKind::Bend(value.parse().ok()?),
        Target::Param(name) => EventKind::Param(name.clone(), value.into()),
    })
}

/// Replace a value that steps more than once inside a sounding note with a
/// macro started at the note's onset. Identical step lists share one macro.
fn fold_macros(events: &mut Vec<Event>, macros: &mut Vec<Macro>) {
    let sounds: Vec<(usize, u64, u64)> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| event.kind.length() > 0)
        .map(|(index, event)| (index, event.tick, event.tick + event.kind.length()))
        .collect();
    let mut removed = vec![false; events.len()];
    let mut inserts: Vec<(usize, Event)> = Vec::new();
    let mut previous = None;
    for (sound, start, end) in sounds {
        // The rest of a chord shares its first note's macros.
        if previous.replace(start) == Some(start) {
            continue;
        }
        let mut groups: Vec<(Target, Vec<usize>)> = Vec::new();
        for (index, event) in events.iter().enumerate().skip(sound + 1) {
            if event.tick >= end {
                break;
            }
            if event.tick <= start {
                continue;
            }
            if removed[index] {
                continue;
            }
            if let Some((target, _)) = target_value(&event.kind) {
                match groups.iter_mut().find(|(t, _)| *t == target) {
                    Some((_, members)) => members.push(index),
                    None => groups.push((target, vec![index])),
                }
            }
        }
        for (target, members) in groups {
            if members.len() < 2 {
                continue;
            }
            let mut members = members;
            // The value set at the note's own onset starts the macro.
            if let Some(onset) = (0..sound).rev().take_while(|i| events[*i].tick == start).find(|i| {
                !removed[*i] && target_value(&events[*i].kind).is_some_and(|(t, _)| t == target)
            }) {
                members.insert(0, onset);
            }
            let steps: Vec<(u64, String)> = members
                .iter()
                .map(|index| {
                    let event = &events[*index];
                    (event.tick - start, target_value(&event.kind).unwrap().1)
                })
                .collect();
            let candidate = Macro { target, steps };
            let id = macros
                .iter()
                .position(|existing| *existing == candidate)
                .unwrap_or_else(|| {
                    macros.push(candidate);
                    macros.len() - 1
                });
            for index in members {
                removed[index] = true;
            }
            inserts.push((sound, Event { tick: start, kind: EventKind::Macro(id) }));
        }
    }
    if inserts.is_empty() {
        return;
    }
    let mut folded = Vec::with_capacity(events.len());
    let mut pending = inserts.into_iter().peekable();
    for (index, event) in std::mem::take(events).into_iter().enumerate() {
        while pending.peek().is_some_and(|(before, _)| *before == index) {
            folded.push(pending.next().unwrap().1);
        }
        if !removed[index] {
            folded.push(event);
        }
    }
    *events = folded;
}

impl Score {
    /// Events with every macro replaced by the commands it stands for.
    pub fn expanded_events(&self, track: &Track) -> Vec<Event> {
        let mut events = Vec::new();
        for event in &track.events {
            match &event.kind {
                EventKind::Macro(id) => {
                    if let Some(found) = self.macros.get(*id) {
                        for (offset, value) in &found.steps {
                            if let Some(kind) = target_event(&found.target, value) {
                                events.push(Event { tick: event.tick + offset, kind });
                            }
                        }
                    }
                }
                _ => events.push(event.clone()),
            }
        }
        events.sort_by_key(|event| (event.tick, event.kind.length() > 0));
        events
    }
}

/// Find the rhythmic grid: the longest step that nearly every gap between
/// onsets is a whole multiple of, and where that grid starts. Returns frames
/// per quarter note (taking the step as a sixteenth or eighth as the tempo
/// allows) and the grid's offset in frames.
fn infer_grid(tracks: &[Track], tick_seconds: f64) -> Grid {
    let mut onsets: Vec<u64> = tracks
        .iter()
        .flat_map(|track| track.events.iter())
        .filter(|event| matches!(event.kind, EventKind::Note { .. } | EventKind::Hit { .. }))
        .map(|event| event.tick)
        .collect();
    onsets.sort_unstable();
    onsets.dedup();
    let fallback = || {
        let quarter = f64::from(infer_quarter(tracks, tick_seconds));
        Grid { quarter, steps: 4.0, jitter: 1.0 }
    };
    if onsets.len() < 8 {
        return fallback();
    }
    let times: Vec<f64> = onsets.iter().map(|tick| *tick as f64).collect();
    let fit = |unit: f64| {
        // The grid phase is the circular mean of each onset's position in a step.
        let (mut x, mut y) = (0.0, 0.0);
        for time in &times {
            let angle = std::f64::consts::TAU * (time / unit).fract();
            x += angle.cos();
            y += angle.sin();
        }
        let phase = (y.atan2(x) / std::f64::consts::TAU).rem_euclid(1.0) * unit;
        let tolerance = (0.08 * unit).max(0.75);
        let hits = times
            .iter()
            .filter(|time| {
                let offset = ((**time - phase) / unit).rem_euclid(1.0) * unit;
                offset.min(unit - offset) <= tolerance
            })
            .count();
        (hits as f64 / times.len() as f64, phase)
    };
    let first = (0.045 / tick_seconds).max(2.0);
    // Up to a slow quarter note: some songs move in eighths of ~0.4 s.
    let last = 1.2 / tick_seconds;
    let mut candidates = Vec::new();
    let mut unit = first;
    while unit <= last {
        let (score, phase) = fit(unit);
        candidates.push((unit, score, phase));
        unit += 0.1;
    }
    // A step that is slightly off drifts across a long song, so refine the
    // most promising steps much more finely before choosing.
    let mut coarse = candidates.clone();
    coarse.sort_by(|a, b| b.1.total_cmp(&a.1));
    for &(around, _, _) in coarse.iter().take(12) {
        let mut unit = around - 0.1;
        while unit <= around + 0.1 {
            let (score, phase) = fit(unit);
            candidates.push((unit, score, phase));
            unit += 0.002;
        }
    }
    candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
    let best = candidates.iter().map(|c| c.1).fold(0.0, f64::max);
    if std::env::var_os("KOG_MML_DEBUG").is_some() {
        let mut sorted = candidates.clone();
        sorted.sort_by(|a, b| b.1.total_cmp(&a.1));
        eprintln!("onsets {} tick {:.5}s", times.len(), tick_seconds);
        for (unit, fit, phase) in sorted.iter().take(15) {
            eprintln!("unit {:.2} frames ({:.4}s) fit {:.3} phase {:.2}", unit, unit * tick_seconds, fit, phase);
        }
        let gaps: Vec<_> = times.windows(2).take(60).map(|w| w[1] - w[0]).collect();
        eprintln!("first gaps {gaps:?}");
    }
    // The longest step that explains almost as many onsets as the best one;
    // shorter steps always fit, so they are only chosen when nothing else does.
    let Some(&(unit, _, phase)) = candidates.iter().rev().find(|c| c.1 >= best - 0.03) else {
        return fallback();
    };
    // How far onsets that belong to the grid sit from it: decoders that report
    // key-ons per audio block place them a few frames late or early.
    let mut offsets: Vec<f64> = times
        .iter()
        .map(|time| {
            let offset = ((time - phase) / unit).rem_euclid(1.0) * unit;
            offset.min(unit - offset)
        })
        .filter(|offset| *offset <= (0.08 * unit).max(0.75))
        .collect();
    offsets.sort_by(f64::total_cmp);
    let jitter = offsets.get(offsets.len() * 3 / 4).copied().unwrap_or(0.0).max(0.5);
    // How many grid steps make a quarter note: choose the count that writes
    // the most gaps between onsets as plain (or dotted) note values, so a
    // grid of triplet steps reads as triplets rather than odd ties.
    let gaps: Vec<u64> = times
        .windows(2)
        .map(|pair| ((pair[1] - pair[0]) / unit).round() as u64)
        .filter(|gap| *gap > 0)
        .collect();
    let bpm = |quarter: f64| 60.0 / (quarter * tick_seconds);
    // gap/steps quarters as a plain note value scores 2, dotted scores 1.
    let plain = |gap: u64, steps: u64| {
        let (numerator, denominator) = (gap * 32, steps);
        if numerator % denominator != 0 {
            return 0;
        }
        let length = numerator / denominator; // in 1/32 quarters
        if length.is_power_of_two() {
            2
        } else if length % 3 == 0 && (length / 3).is_power_of_two() {
            1
        } else {
            0
        }
    };
    let steps = [1u64, 2, 3, 4, 6, 8, 12, 16]
        .into_iter()
        .filter(|steps| (60.0..=200.0).contains(&bpm(unit * *steps as f64)))
        .max_by(|a, b| {
            let score = |steps: u64| gaps.iter().map(|gap| plain(*gap, steps)).sum::<u64>();
            score(*a)
                .cmp(&score(*b))
                .then((bpm(unit * *b as f64) - 125.0).abs().total_cmp(&(bpm(unit * *a as f64) - 125.0).abs()))
        })
        .map_or(4.0, |steps| steps as f64);
    Grid { quarter: unit * steps, steps, jitter }
}

/// A rhythmic grid in decoder frames.
struct Grid {
    quarter: f64,
    /// Grid steps per quarter note.
    steps: f64,
    /// Typical distance of an on-grid onset from its grid line.
    jitter: f64,
}

/// Estimate a beat from the most common gap between onsets on the same voice,
/// ignoring arpeggio-speed gaps, then double it into a plausible tempo.
fn infer_quarter(tracks: &[Track], tick_seconds: f64) -> u32 {
    let minimum = (0.06 / tick_seconds).ceil() as u64;
    let gaps = tracks.iter().flat_map(|track| {
        let onsets: Vec<u64> = track
            .events
            .iter()
            .filter(|event| matches!(event.kind, EventKind::Note { .. } | EventKind::Hit { .. }))
            .map(|event| event.tick)
            .collect();
        onsets
            .windows(2)
            .map(|pair| pair[1] - pair[0])
            .filter(|gap| *gap >= minimum)
            .collect::<Vec<_>>()
    });
    let mut quarter = mode(gaps).unwrap_or_else(|| (0.5 / tick_seconds).round() as u64).max(1);
    while (quarter as f64) * tick_seconds < 0.3 {
        quarter *= 2;
    }
    while (quarter as f64) * tick_seconds > 1.2 && quarter % 2 == 0 {
        quarter /= 2;
    }
    quarter.min(u64::from(u32::MAX)) as u32
}

pub fn track_label(index: usize) -> String {
    let mut label = String::new();
    let mut value = index + 1;
    while value > 0 {
        value -= 1;
        label.insert(0, char::from(b'A' + (value % 26) as u8));
        value /= 26;
    }
    label
}

// ---------------------------------------------------------------------------
// Score -> MML text

/// A span of rendered text and the ticks it covers, for live highlighting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub track: usize,
    pub start: u64,
    pub end: u64,
    /// Byte offsets; the text is ASCII, so these are also character offsets.
    pub from: usize,
    pub to: usize,
    /// note, hit, rest, or command
    pub kind: String,
    /// Onset tick of the note or hit this piece belongs to. A note split by
    /// bar lines or commands has several pieces that share it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sound: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bar {
    /// Index of this block of bars (one line per track).
    pub index: usize,
    /// Zero-based number of the first bar in the block.
    #[serde(default)]
    pub first_bar: usize,
    pub start: u64,
    pub end: u64,
    pub from: usize,
    pub to: usize,
}

/// Bars written on each line of every track unless a view asks otherwise.
pub const BARS_PER_LINE: usize = 4;

/// Token classes for syntax colouring, with the colour every frontend uses.
pub const STYLES: [(&str, &str); 17] = [
    ("header", "#6f8794"),
    ("comment", "#7d9aa8"),
    ("label", "#ffcb6b"),
    ("bar", "#46606e"),
    ("note", "#f4f7f9"),
    ("cents", "#f78c6c"),
    ("length", "#6fb3d9"),
    ("tie", "#6fb3d9"),
    ("octave", "#c792ea"),
    ("rest", "#5f7482"),
    ("hit", "#f78c6c"),
    ("key", "#82aaff"),
    ("value", "#c3e88d"),
    ("instrument", "#ffcb6b"),
    ("macro", "#c792ea"),
    ("control", "#89ddff"),
    ("legato", "#f07178"),
];

fn style(name: &str) -> u8 {
    STYLES.iter().position(|(style, _)| *style == name).unwrap_or(0) as u8
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub text: String,
    pub tick_seconds: f64,
    pub spans: Vec<Span>,
    pub bars: Vec<Bar>,
    pub tracks: Vec<TrackInfo>,
    /// `[from, to, class]` runs in text order; classes index [`Document::palette`].
    pub styles: Vec<(u32, u32, u8)>,
    /// `#rrggbb` for each class in [`STYLES`].
    pub palette: Vec<String>,
    /// Tick and seconds pairs for songs whose tempo drifts; between them time
    /// is linear. Empty when `tick_seconds` alone gives every tick's time.
    #[serde(default)]
    pub timing: Vec<(u64, f64)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackInfo {
    pub label: String,
    pub name: String,
    pub kind: String,
}

impl Document {
    /// Spans sounding at `seconds`, plus the bar containing it.
    pub fn active(&self, seconds: f64) -> (Vec<&Span>, Option<&Bar>) {
        let (indices, bar) = self.active_indices(seconds);
        (indices.into_iter().map(|index| &self.spans[index]).collect(), bar)
    }

    /// Like [`Document::active`], as indices into [`Document::spans`]. Every
    /// piece of a sounding note is included, even across bar lines.
    pub fn active_indices(&self, seconds: f64) -> (Vec<usize>, Option<&Bar>) {
        let tick = self.tick_at(seconds);
        let sounding: Vec<(usize, u64)> = self
            .spans
            .iter()
            .filter(|span| span.start <= tick && tick < span.end)
            .filter_map(|span| span.sound.map(|sound| (span.track, sound)))
            .collect();
        let indices = self
            .spans
            .iter()
            .enumerate()
            .filter(|(_, span)| {
                span.sound
                    .is_some_and(|sound| sounding.contains(&(span.track, sound)))
            })
            .map(|(index, _)| index)
            .collect();
        let bar = self.bars.iter().find(|bar| bar.start <= tick && tick < bar.end);
        (indices, bar)
    }

    /// The tick playing `seconds` into the song.
    pub fn tick_at(&self, seconds: f64) -> u64 {
        let seconds = seconds.max(0.0);
        let rate = self.tick_seconds.max(f64::MIN_POSITIVE);
        let after = self.timing.partition_point(|(_, at)| *at <= seconds);
        let tick = match (after.checked_sub(1).map(|i| self.timing[i]), self.timing.get(after).copied()) {
            (Some((a, sa)), Some((b, sb))) => a as f64 + (seconds - sa) / (sb - sa).max(f64::MIN_POSITIVE) * (b - a) as f64,
            (Some((a, sa)), None) => a as f64 + (seconds - sa) / rate,
            (None, Some((b, sb))) => (b as f64 - (sb - seconds) / rate).max(0.0),
            (None, None) => seconds / rate,
        };
        tick.max(0.0) as u64
    }

    /// The colour runs inside `from..to`, for drawing one bar or line.
    pub fn styles_in(&self, from: usize, to: usize) -> &[(u32, u32, u8)] {
        let first = self.styles.partition_point(|run| (run.1 as usize) <= from);
        let last = self.styles.partition_point(|run| (run.0 as usize) < to);
        &self.styles[first..last.max(first)]
    }
}

/// Words that need no quotes: printable ASCII without spaces or MML syntax.
fn bare(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_graphic() && !matches!(b, b'"' | b'=' | b'|' | b';' | b'\\'))
}

fn quote(value: &str) -> String {
    let mut out = String::from("\"");
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            ' '..='~' => out.push(character),
            other => {
                let _ = write!(out, "\\u{{{:x}}}", u32::from(other));
            }
        }
    }
    out.push('"');
    out
}

fn word(value: &str) -> String {
    if bare(value) { value.to_owned() } else { quote(value) }
}

/// `1 2 4 … 128` with up to two dots, then triplet values `3 6 12 … 96`, as
/// fractions of a whole note.
fn standard_lengths(quarter: u64) -> Vec<(u64, String)> {
    let whole = quarter * 4;
    let mut lengths = Vec::new();
    for divisor in [1u64, 2, 4, 8, 16, 32, 64, 128, 3, 6, 12, 24, 48, 96] {
        if whole % divisor != 0 {
            continue;
        }
        let base = whole / divisor;
        lengths.push((base, divisor.to_string()));
        if base % 2 == 0 {
            lengths.push((base + base / 2, format!("{divisor}.")));
            if base % 4 == 0 {
                lengths.push((base + base / 2 + base / 4, format!("{divisor}..")));
            }
        }
    }
    lengths
}

/// A length as one note value, or tied note values (`4^16`). Empty when it
/// equals the track's default length.
fn length_text(ticks: u64, quarter: u64, default: Option<u64>) -> String {
    if Some(ticks) == default {
        return String::new();
    }
    let lengths = standard_lengths(quarter);
    if let Some((_, text)) = lengths.iter().find(|(length, _)| *length == ticks) {
        return text.clone();
    }
    // Whole notes first, then the largest plain values that fit: ordinary
    // values when they add up exactly, otherwise triplets as well.
    let mut parts = Vec::new();
    let mut left = ticks;
    let whole = quarter * 4;
    while left >= whole {
        parts.push("1".to_owned());
        left -= whole;
    }
    let greedy = |left: u64, triplets: bool| {
        let mut plain: Vec<&(u64, String)> = lengths
            .iter()
            .filter(|(_, text)| !text.ends_with('.'))
            .filter(|(_, text)| triplets || text.parse::<u64>().is_ok_and(u64::is_power_of_two))
            .collect();
        plain.sort_by(|a, b| b.0.cmp(&a.0));
        let mut parts = Vec::new();
        let mut left = left;
        while left > 0 {
            let Some((length, text)) = plain.iter().find(|(length, _)| *length <= left).map(|p| (p.0, &p.1)) else {
                return (parts, left);
            };
            parts.push(text.clone());
            left -= length;
        }
        (parts, 0)
    };
    let (mut tail, rest) = greedy(left, false);
    if rest > 0 {
        let (with_triplets, rest) = greedy(left, true);
        tail = with_triplets;
        if rest > 0 {
            tail.push(format!("{rest}t"));
        }
    }
    parts.extend(tail);
    parts.join("^")
}

const NAMES: [&str; 12] = ["c", "c+", "d", "d+", "e", "f", "f+", "g", "g+", "a", "a+", "b"];

struct Writer<'a> {
    score: &'a Score,
    text: String,
    spans: Vec<Span>,
    octave: Vec<Option<i32>>,
    velocity: Vec<Option<u8>>,
    instrument: Vec<Option<String>>,
    default: Vec<u64>,
}

impl Writer<'_> {
    fn token(&mut self, track: usize, start: u64, end: u64, kind: &str, text: &str, sound: Option<u64>) {
        if !self.text.ends_with(' ') && !self.text.ends_with('\n') {
            self.text.push(' ');
        }
        let from = self.text.len();
        self.text.push_str(text);
        self.spans.push(Span {
            track,
            start,
            end,
            from,
            to: self.text.len(),
            kind: kind.into(),
            sound,
        });
    }

    fn command(&mut self, track: usize, tick: u64, kind: &EventKind) {
        let text = match kind {
            EventKind::Bend(cents) => format!("P{cents:+}"),
            EventKind::Level(level) => format!("v{level}"),
            EventKind::Pan(pan) => format!("p{pan}"),
            EventKind::Instrument(name) => {
                self.instrument[track] = Some(name.clone());
                format!("@{}", word(name))
            }
            EventKind::Param(name, value) => format!("{}={}", word(name), word(value)),
            EventKind::Macro(id) => format!("~{id}"),
            _ => return,
        };
        self.token(track, tick, tick, "command", &text, None);
    }

    fn transpose(&self, track: usize) -> i32 {
        self.instrument[track]
            .as_ref()
            .and_then(|name| self.score.transpose.iter().find(|(n, _)| n == name))
            .map_or(0, |(_, shift)| *shift)
    }

    /// A note's octave, written as a change from the current one.
    fn octave_change(&mut self, track: usize, octave: i32) -> String {
        let text = match self.octave[track].map(|current| octave - current) {
            Some(0) => String::new(),
            Some(shift @ 1..=2) => ">".repeat(shift as usize),
            Some(shift @ -2..=-1) => "<".repeat(-shift as usize),
            _ => format!("o{octave}"),
        };
        self.octave[track] = Some(octave);
        text
    }

    /// A note's name and any detune that `#TUNE` does not imply.
    fn pitch(&self, track: usize, key: i32, cents: i32) -> String {
        let written = key + self.transpose(track);
        let mut text = NAMES[written.rem_euclid(12) as usize].to_owned();
        if cents != self.score.tracks[track].tuning(key) {
            let _ = write!(text, "({cents:+})");
        }
        text
    }

    fn sound(&mut self, track: usize, start: u64, ticks: u64, sound: Option<usize>, first: bool) {
        let score = self.score;
        let events = &score.tracks[track].events;
        let kind = sound.map_or(&EventKind::Bend(0), |index| &events[index].kind);
        let onset = sound.map(|index| events[index].tick);
        let quarter = u64::from(self.score.quarter);
        let end = start + ticks;
        let kind_name = match kind {
            EventKind::Note { .. } => "note",
            EventKind::Hit { .. } => "hit",
            _ => "rest",
        };
        if !first && onset.is_some() {
            let text = format!("^{}", length_text(ticks, quarter, None));
            self.token(track, start, end, kind_name, &text, onset);
            return;
        }
        let length = length_text(ticks, quarter, Some(self.default[track]));
        match kind {
            EventKind::Note {
                key,
                cents,
                velocity,
                legato,
                ..
            } => {
                if self.velocity[track] != Some(*velocity) {
                    self.velocity[track] = Some(*velocity);
                    self.token(track, start, start, "command", &format!("V{velocity}"), None);
                }
                let octave = (key + self.transpose(track)).div_euclid(12) - 1;
                let change = self.octave_change(track, octave);
                if !change.is_empty() {
                    self.token(track, start, start, "command", &change, None);
                }
                let mut text = String::new();
                if *legato {
                    text.push('&');
                }
                // Detune listed in #TUNE is implied; anything else is spelled out.
                let index = sound.unwrap_or_default();
                let count = group_len(events, index);
                let follow = follow_end(events, index, count);
                let group = &events[index..index + count];
                let mut end = end;
                if count == 1 && group[0].tick + group[0].kind.length() == follow {
                    text.push_str(&self.pitch(track, *key, *cents));
                } else {
                    // A chord: its notes between quotes, then how far the
                    // track moves on. A note that holds on past that (or stops
                    // short of it) carries its own length.
                    text.clear();
                    text.push('\'');
                    for (position, event) in group.iter().enumerate() {
                        let EventKind::Note { key, cents, velocity, legato, length } = event.kind else { continue };
                        if position > 0 {
                            if self.velocity[track] != Some(velocity) {
                                self.velocity[track] = Some(velocity);
                                let _ = write!(text, "V{velocity}");
                            }
                            let octave = (key + self.transpose(track)).div_euclid(12) - 1;
                            text.push_str(&self.octave_change(track, octave));
                        }
                        if legato {
                            text.push('&');
                        }
                        text.push_str(&self.pitch(track, key, cents));
                        if event.tick + length != follow {
                            text.push_str(&length_text(length, quarter, None));
                        }
                        end = end.max(event.tick + length);
                    }
                    text.push('\'');
                }
                text.push_str(&length);
                self.token(track, start, end, "note", &text, onset);
            }
            EventKind::Hit { velocity, .. } => {
                if self.velocity[track] != Some(*velocity) {
                    self.velocity[track] = Some(*velocity);
                    self.token(track, start, start, "command", &format!("V{velocity}"), None);
                }
                self.token(track, start, end, "hit", &format!("x{length}"), onset);
            }
            _ => self.token(track, start, end, "rest", &format!("r{length}"), None),
        }
    }
}

enum Item {
    Command(u64, usize),
    /// start, end, the sound's event (None for a gap), first piece of it
    Piece(u64, u64, Option<usize>, bool),
}

/// A track as commands and sound pieces in time order. Sounds and gaps are
/// split where a command falls inside them and at bar lines.
fn items(events: &[Event], length: u64, score: &Score) -> Vec<Item> {
    let mut sounds = Vec::new();
    let mut cursor = 0;
    let mut group_end = 0;
    for (index, event) in events.iter().enumerate() {
        if index < group_end {
            continue;
        }
        if event.kind.length() > 0 {
            let count = group_len(events, index);
            group_end = index + count;
            if event.tick > cursor {
                sounds.push((cursor, event.tick, None));
            }
            let end = follow_end(events, index, count);
            sounds.push((event.tick, end, Some(index)));
            cursor = end;
        }
    }
    if cursor < length {
        sounds.push((cursor, length, None));
    }
    let commands: Vec<(u64, usize)> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| event.kind.length() == 0)
        .map(|(index, event)| (event.tick, index))
        .collect();
    let mut items = Vec::new();
    let mut next_command = 0;
    for (start, end, sound) in sounds {
        let mut piece_start = start;
        loop {
            while next_command < commands.len() && commands[next_command].0 <= piece_start {
                items.push(Item::Command(commands[next_command].0, commands[next_command].1));
                next_command += 1;
            }
            let bar_end = score.next_bar(piece_start);
            let command_tick = commands.get(next_command).map_or(u64::MAX, |c| c.0);
            let piece_end = end.min(bar_end).min(command_tick);
            items.push(Item::Piece(piece_start, piece_end, sound, piece_start == start));
            piece_start = piece_end;
            if piece_start >= end {
                break;
            }
        }
    }
    for &(tick, index) in &commands[next_command..] {
        items.push(Item::Command(tick, index));
    }
    items
}

fn macro_target(target: &Target) -> String {
    match target {
        Target::Level => "v".into(),
        Target::Pan => "p".into(),
        Target::Bend => "P".into(),
        Target::Param(name) => format!("{}=", word(name)),
    }
}

pub fn encode(score: &Score) -> Document {
    encode_lines(score, BARS_PER_LINE)
}

/// Like [`encode`], with `bars_per_line` bars on each track's line (at least 1).
pub fn encode_lines(score: &Score, bars_per_line: usize) -> Document {
    let bars_per_line = bars_per_line.max(1);
    let quarter = u64::from(score.quarter.max(1));
    let tracks = score.tracks.len();
    let mut writer = Writer {
        score,
        text: String::new(),
        spans: Vec::new(),
        octave: vec![None; tracks],
        velocity: vec![None; tracks],
        instrument: vec![None; tracks],
        default: Vec::with_capacity(tracks),
    };
    let text = &mut writer.text;
    let _ = writeln!(text, "#{FORMAT}");
    let _ = writeln!(text, "#TITLE {}", quote(&score.title));
    let _ = writeln!(text, "#SOURCE {}", quote(&score.backend));
    let _ = writeln!(
        text,
        "#TEMPO {}.{:03}{}",
        score.tempo / 1000,
        score.tempo % 1000,
        if score.inferred { " inferred" } else { "" }
    );
    let _ = writeln!(text, "#TICKS {} ; per quarter note", score.quarter);
    let _ = writeln!(text, "#BAR {} ; quarter notes", score.beats);
    let _ = writeln!(text, "#LENGTH {} ; ticks", score.length);
    for chunk in score.timing.chunks(8) {
        let _ = write!(text, "#TIMING");
        for (tick, micros) in chunk {
            let _ = write!(text, " {tick}:{}.{:06}", micros / 1_000_000, micros % 1_000_000);
        }
        text.push('\n');
    }
    if score.pickup > 0 {
        let _ = writeln!(text, "#PICKUP {} ; ticks before the first full bar", score.pickup);
    }
    for (instrument, shift) in &score.transpose {
        let _ = writeln!(
            text,
            "#TRANSPOSE {} {shift:+} ; relative sample pitch, moved by octaves",
            word(instrument)
        );
    }
    for track in &score.tracks {
        let lengths = track.events.iter().map(|e| e.kind.length()).filter(|l| *l > 0);
        let standard = standard_lengths(quarter);
        let default = mode(lengths.filter(|l| standard.iter().any(|(s, _)| s == l))).unwrap_or(quarter);
        writer.default.push(default);
        let _ = writeln!(
            writer.text,
            "#TRACK {} {} channel={} voice={} kind={} l{}",
            track.label,
            word(&track.name),
            track.channel,
            track.voice,
            word(&track.kind),
            length_text(default, quarter, None)
        );
    }
    for track in &score.tracks {
        if !track.tuning.is_empty() {
            let _ = write!(writer.text, "#TUNE {}", track.label);
            for (key, cents) in &track.tuning {
                let _ = write!(writer.text, " {key}:{cents:+}");
            }
            writer.text.push('\n');
        }
        for table in &track.pitched {
            let _ = write!(writer.text, "#PITCH {} {}=", track.label, word(&table.name));
            for (key, cents, value) in &table.values {
                let _ = write!(writer.text, " {key}");
                if *cents != 0 {
                    let _ = write!(writer.text, "({cents:+})");
                }
                let _ = write!(writer.text, ":{}", word(value));
            }
            writer.text.push('\n');
        }
    }
    for (id, found) in score.macros.iter().enumerate() {
        let _ = write!(writer.text, "#MACRO {id} {}", macro_target(&found.target));
        for (offset, value) in &found.steps {
            let _ = write!(writer.text, " {offset}:{}", word(value));
        }
        writer.text.push('\n');
    }
    let label_width = score.tracks.iter().map(|t| t.label.len()).max().unwrap_or(1);
    let plans: Vec<_> = score
        .tracks
        .iter()
        .map(|track| items(&track.events, score.length, score))
        .collect();
    let mut cursors = vec![0usize; tracks];
    let mut bars = Vec::new();
    let bar_count = score.bar_count();
    // Bars are written bars_per_line to a line; each group is one block that
    // frontends draw, follow, and highlight together.
    for (block, first) in (0..bar_count).step_by(bars_per_line).enumerate() {
        let group = first..(first + bars_per_line).min(bar_count);
        let block_start = score.bar_start(first);
        let block_end = score.bar_start(group.end).min(score.length).max(block_start);
        let from = writer.text.len();
        let seconds = score.seconds_at(block_start);
        let numbers = if group.len() > 1 {
            format!("bars {}-{}", first + 1, group.end)
        } else {
            format!("bar {}", first + 1)
        };
        let _ = writeln!(writer.text, "; {numbers} {}:{:06.3}", (seconds / 60.0) as u64, seconds % 60.0);
        for (track_index, track) in score.tracks.iter().enumerate() {
            let _ = write!(writer.text, "{:<label_width$} |", track.label);
            for index in group.clone() {
                let end = score.bar_start(index + 1).min(score.length).max(score.bar_start(index));
                let last = index + 1 == bar_count;
                let plan = &plans[track_index];
                let mut cursor = cursors[track_index];
                while let Some(item) = plan.get(cursor) {
                    match *item {
                        Item::Command(tick, event) => {
                            if tick >= end && !last {
                                break;
                            }
                            writer.command(track_index, tick, &track.events[event].kind);
                        }
                        Item::Piece(piece_start, piece_end, sound, first) => {
                            if piece_start >= end {
                                break;
                            }
                            writer.sound(
                                track_index,
                                piece_start,
                                piece_end - piece_start,
                                sound,
                                first,
                            );
                        }
                    }
                    cursor += 1;
                }
                cursors[track_index] = cursor;
                writer.text.push_str(" |");
            }
            writer.text.push('\n');
        }
        bars.push(Bar {
            index: block,
            first_bar: first,
            start: block_start,
            end: block_end,
            from,
            to: writer.text.len(),
        });
    }
    let styles = lex(&writer.text);
    Document {
        text: writer.text,
        tick_seconds: score.tick_seconds(),
        spans: writer.spans,
        bars,
        tracks: score
            .tracks
            .iter()
            .map(|track| TrackInfo {
                label: track.label.clone(),
                name: track.name.clone(),
                kind: track.kind.clone(),
            })
            .collect(),
        styles,
        palette: STYLES.iter().map(|(_, colour)| (*colour).to_owned()).collect(),
        timing: score.timing_seconds(),
    }
}

/// Colour runs for Kog MML text. Only needs to be good enough to paint.
pub fn lex(text: &str) -> Vec<(u32, u32, u8)> {
    let bytes = text.as_bytes();
    let mut runs = Vec::new();
    let mut push = |from: usize, to: usize, name: &str| {
        if to > from {
            runs.push((from as u32, to as u32, style(name)));
        }
    };
    let mut line_start = 0;
    while line_start < bytes.len() {
        let line_end = text[line_start..].find('\n').map_or(bytes.len(), |at| line_start + at);
        let line = &bytes[line_start..line_end];
        let at = |offset: usize| line_start + offset;
        match line.first() {
            Some(b'#') => {
                let comment = line.iter().position(|b| *b == b';').unwrap_or(line.len());
                push(at(0), at(comment), "header");
                push(at(comment), at(line.len()), "comment");
            }
            Some(b';') => push(at(0), at(line.len()), "comment"),
            Some(_) => {
                let label = line.iter().position(|b| *b == b' ').unwrap_or(line.len());
                push(at(0), at(label), "label");
                let mut i = label;
                let skip_string = |mut i: usize| {
                    i += 1;
                    while i < line.len() && line[i] != b'"' {
                        i += if line[i] == b'\\' { 2 } else { 1 };
                    }
                    (i + 1).min(line.len())
                };
                let word_end = |mut i: usize| {
                    if line.get(i) == Some(&b'"') {
                        return skip_string(i);
                    }
                    while i < line.len() && line[i] != b' ' && line[i] != b'=' {
                        i += 1;
                    }
                    i
                };
                while i < line.len() {
                    if line[i] == b' ' {
                        i += 1;
                        continue;
                    }
                    let token_end = word_end(i);
                    if line.get(token_end) == Some(&b'=') {
                        push(at(i), at(token_end + 1), "key");
                        let value_end = word_end(token_end + 1);
                        push(at(token_end + 1), at(value_end), "value");
                        i = value_end;
                        continue;
                    }
                    let end = line[i..].iter().position(|b| *b == b' ').map_or(line.len(), |n| i + n);
                    match line[i] {
                        b'|' => push(at(i), at(end), "bar"),
                        b'@' => {
                            let end = if line.get(i + 1) == Some(&b'"') { skip_string(i + 1) } else { end };
                            push(at(i), at(end), "instrument");
                            i = end;
                            continue;
                        }
                        b'~' => push(at(i), at(end), "macro"),
                        b'o' | b'<' | b'>' => push(at(i), at(end), "octave"),
                        b'V' | b'v' | b'p' | b'P' => push(at(i), at(end), "control"),
                        b'r' => push(at(i), at(end), "rest"),
                        b'^' => push(at(i), at(end), "tie"),
                        b'x' => {
                            push(at(i), at(i + 1), "hit");
                            push(at(i + 1), at(end), "length");
                        }
                        b'\'' => {
                            // A chord: notes, their own lengths, and octave and
                            // velocity changes, between quotes.
                            let close = line[i + 1..end].iter().position(|b| *b == b'\'').map_or(end, |n| i + 1 + n + 1);
                            let mut k = i;
                            while k < close {
                                let from = k;
                                k += 1;
                                let name = match line[from] {
                                    b'\'' => "note",
                                    b'&' => "legato",
                                    b'<' | b'>' => "octave",
                                    b'o' | b'V' => {
                                        while k < close && matches!(line[k], b'0'..=b'9' | b'+' | b'-') {
                                            k += 1;
                                        }
                                        if line[from] == b'o' { "octave" } else { "control" }
                                    }
                                    b'(' => {
                                        while k < close && line[k - 1] != b')' {
                                            k += 1;
                                        }
                                        "cents"
                                    }
                                    b'a'..=b'g' => {
                                        while k < close && matches!(line[k], b'+' | b'#' | b'-') {
                                            k += 1;
                                        }
                                        "note"
                                    }
                                    _ => {
                                        while k < close && matches!(line[k], b'0'..=b'9' | b'.' | b'^' | b't') {
                                            k += 1;
                                        }
                                        "length"
                                    }
                                };
                                push(at(from), at(k), name);
                            }
                            push(at(close), at(end), "length");
                        }
                        b'&' | b'a'..=b'g' => {
                            let mut j = i;
                            if line[j] == b'&' {
                                push(at(j), at(j + 1), "legato");
                                j += 1;
                            }
                            let name = j;
                            j += 1;
                            while j < end && matches!(line[j], b'+' | b'#' | b'-') {
                                j += 1;
                            }
                            push(at(name), at(j), "note");
                            if line.get(j) == Some(&b'(') {
                                let close = line[j..end].iter().position(|b| *b == b')').map_or(end, |n| j + n + 1);
                                push(at(j), at(close), "cents");
                                j = close;
                            }
                            push(at(j), at(end), "length");
                        }
                        _ => {}
                    }
                    i = end;
                }
            }
            None => {}
        }
        line_start = line_end + 1;
    }
    runs
}

// ---------------------------------------------------------------------------
// MML text -> score

#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
    line: usize,
}

impl Cursor<'_> {
    fn error<T>(&self, message: impl Into<String>) -> Result<T, ParseError> {
        Err(ParseError {
            line: self.line,
            message: message.into(),
        })
    }
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }
    fn skip_space(&mut self) {
        while let Some(byte) = self.peek() {
            if byte == b' ' || byte == b'\t' || byte == b'\r' {
                self.at += 1;
            } else {
                break;
            }
        }
    }
    fn eat(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn number(&mut self) -> Result<i64, ParseError> {
        let start = self.at;
        if matches!(self.peek(), Some(b'+' | b'-')) {
            self.at += 1;
        }
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.at += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.at]).unwrap_or_default();
        match text.parse() {
            Ok(value) => Ok(value),
            Err(_) => self.error(format!("expected a number at {text:?}")),
        }
    }
    fn string(&mut self) -> Result<String, ParseError> {
        if !self.eat(b'"') {
            return self.error("expected a quoted string");
        }
        let mut out = String::new();
        loop {
            match self.peek() {
                None | Some(b'\n') => return self.error("unterminated string"),
                Some(b'"') => {
                    self.at += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.at += 1;
                    match self.peek() {
                        Some(b'u') => {
                            self.at += 1;
                            if !self.eat(b'{') {
                                return self.error("expected \\u{hex}");
                            }
                            let start = self.at;
                            while self.peek().is_some_and(|b| b.is_ascii_hexdigit()) {
                                self.at += 1;
                            }
                            let hex = std::str::from_utf8(&self.bytes[start..self.at]).unwrap_or_default();
                            let character = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
                            if !self.eat(b'}') || character.is_none() {
                                return self.error("invalid \\u{hex} escape");
                            }
                            out.extend(character);
                        }
                        Some(byte) => {
                            self.at += 1;
                            out.push(char::from(byte));
                        }
                        None => return self.error("unterminated escape"),
                    }
                }
                Some(byte) => {
                    let rest = std::str::from_utf8(&self.bytes[self.at..]).unwrap_or_default();
                    let character = rest.chars().next().unwrap_or(char::from(byte));
                    self.at += character.len_utf8();
                    out.push(character);
                }
            }
        }
    }
    /// A bare word or a quoted string.
    fn word(&mut self) -> Result<String, ParseError> {
        self.skip_space();
        if self.peek() == Some(b'"') {
            return self.string();
        }
        let start = self.at;
        while self
            .peek()
            .is_some_and(|b| b.is_ascii_graphic() && !matches!(b, b'"' | b'=' | b'|' | b';'))
        {
            self.at += 1;
        }
        if start == self.at {
            return self.error("expected a word");
        }
        Ok(String::from_utf8_lossy(&self.bytes[start..self.at]).into_owned())
    }
    /// Whether the token here is `name=value`.
    fn at_assignment(&self) -> bool {
        let mut i = self.at;
        if self.bytes.get(i) == Some(&b'"') {
            i += 1;
            while i < self.bytes.len() && self.bytes[i] != b'"' {
                i += if self.bytes[i] == b'\\' { 2 } else { 1 };
            }
            return self.bytes.get(i + 1) == Some(&b'=');
        }
        while i < self.bytes.len() && self.bytes[i].is_ascii_graphic() && !matches!(self.bytes[i], b'=' | b'"') {
            i += 1;
        }
        i > self.at && self.bytes.get(i) == Some(&b'=')
    }
    /// One note value: a divisor of the whole note with dots, or `Nt` ticks.
    fn note_value(&mut self, quarter: u64) -> Result<u64, ParseError> {
        let divisor = self.number()?;
        if self.eat(b't') {
            return u64::try_from(divisor)
                .ok()
                .filter(|ticks| *ticks > 0)
                .map_or_else(|| self.error("tick lengths must be positive"), Ok);
        }
        let whole = quarter * 4;
        let divisor = u64::try_from(divisor).unwrap_or(0);
        if divisor == 0 || whole % divisor != 0 {
            return self.error(format!("length {divisor} is not a whole number of ticks"));
        }
        let base = whole / divisor;
        let mut ticks = base;
        let mut part = base;
        while self.eat(b'.') {
            if part % 2 != 0 {
                return self.error("dotted length is not a whole number of ticks");
            }
            part /= 2;
            ticks += part;
        }
        Ok(ticks)
    }
    /// A length, possibly tied (`4^16`), or the default when none is written.
    fn length(&mut self, quarter: u64, default: u64) -> Result<u64, ParseError> {
        if !self.peek().is_some_and(|b| b.is_ascii_digit()) {
            return Ok(default);
        }
        let mut ticks = self.note_value(quarter)?;
        while self.peek() == Some(b'^') && self.bytes.get(self.at + 1).is_some_and(u8::is_ascii_digit) {
            self.at += 1;
            ticks += self.note_value(quarter)?;
        }
        Ok(ticks)
    }
}

struct TrackParse {
    tick: u64,
    octave: i32,
    velocity: u8,
    default: u64,
    instrument: Option<String>,
    /// The last sound's events that follow the track (several for a chord),
    /// which `^` lengthens.
    last: Vec<usize>,
    rest: bool,
}

pub fn parse(text: &str) -> Result<Score, ParseError> {
    let mut score = Score::default();
    let mut states: Vec<TrackParse> = Vec::new();
    let mut labels: HashMap<String, usize> = HashMap::new();
    let mut header_done = false;
    let mut seen_format = false;
    for (number, line) in text.lines().enumerate() {
        let mut cursor = Cursor {
            bytes: line.as_bytes(),
            at: 0,
            line: number + 1,
        };
        cursor.skip_space();
        match cursor.peek() {
            None | Some(b';') => continue,
            Some(b'#') => {
                if header_done {
                    return cursor.error("headers must precede the bars");
                }
                cursor.at += 1;
                let name = cursor.word()?;
                cursor.skip_space();
                match name.as_str() {
                    "KOG-MML" => {
                        if cursor.word()? != "1" {
                            return cursor.error("unsupported KOG-MML version");
                        }
                        seen_format = true;
                    }
                    "TITLE" => score.title = cursor.string()?,
                    "SOURCE" => score.backend = cursor.string()?,
                    "TEMPO" => {
                        let whole = cursor.number()?;
                        let mut milli = 0;
                        if cursor.eat(b'.') {
                            let start = cursor.at;
                            let fraction = cursor.number()?;
                            let digits = cursor.at - start;
                            if digits != 3 {
                                return cursor.error("#TEMPO has three decimal places");
                            }
                            milli = fraction;
                        }
                        score.tempo = u32::try_from(whole * 1000 + milli).unwrap_or(0);
                        cursor.skip_space();
                        score.inferred = line[cursor.at..].starts_with("inferred");
                        if score.tempo == 0 {
                            return cursor.error("#TEMPO must be positive");
                        }
                    }
                    "TICKS" => score.quarter = cursor.number()?.max(0) as u32,
                    "BAR" => score.beats = cursor.number()?.max(0) as u32,
                    "LENGTH" => score.length = cursor.number()?.max(0) as u64,
                    "PICKUP" => score.pickup = cursor.number()?.max(0) as u64,
                    "TIMING" => loop {
                        cursor.skip_space();
                        if matches!(cursor.peek(), None | Some(b';')) {
                            break;
                        }
                        let tick = cursor.number()?.max(0) as u64;
                        if !cursor.eat(b':') {
                            return cursor.error("#TIMING entries are tick:seconds");
                        }
                        let whole = cursor.number()?.max(0) as u64;
                        let start = cursor.at;
                        let fraction = if cursor.eat(b'.') { cursor.number()?.max(0) as u64 } else { 0 };
                        if cursor.at - start != 7 {
                            return cursor.error("#TIMING seconds have six decimal places");
                        }
                        score.timing.push((tick, whole * 1_000_000 + fraction));
                    },
                    "TUNE" => {
                        let label = cursor.word()?;
                        let Some(&index) = labels.get(&label) else {
                            return cursor.error(format!("#TUNE names undeclared track {label}"));
                        };
                        loop {
                            cursor.skip_space();
                            if matches!(cursor.peek(), None | Some(b';')) {
                                break;
                            }
                            let key = cursor.number()? as i32;
                            if !cursor.eat(b':') {
                                return cursor.error("#TUNE entries are key:cents");
                            }
                            let cents = cursor.number()? as i32;
                            score.tracks[index].tuning.push((key, cents));
                        }
                    }
                    "TRANSPOSE" => {
                        let instrument = cursor.word()?;
                        cursor.skip_space();
                        let shift = cursor.number()? as i32;
                        score.transpose.push((instrument, shift));
                    }
                    "PITCH" => {
                        let label = cursor.word()?;
                        let Some(&index) = labels.get(&label) else {
                            return cursor.error(format!("#PITCH names undeclared track {label}"));
                        };
                        cursor.skip_space();
                        if !cursor.at_assignment() {
                            return cursor.error("#PITCH needs a register name followed by =");
                        }
                        let name = cursor.word()?;
                        cursor.eat(b'=');
                        let mut table = PitchedParam { name, values: Vec::new() };
                        loop {
                            cursor.skip_space();
                            if matches!(cursor.peek(), None | Some(b';')) {
                                break;
                            }
                            let key = cursor.number()? as i32;
                            let cents = if cursor.eat(b'(') {
                                let cents = cursor.number()? as i32;
                                if !cursor.eat(b')') {
                                    return cursor.error("expected ) after cents");
                                }
                                cents
                            } else {
                                0
                            };
                            if !cursor.eat(b':') {
                                return cursor.error("#PITCH entries are key:value");
                            }
                            table.values.push((key, cents, cursor.word()?));
                        }
                        score.tracks[index].pitched.push(table);
                    }
                    "MACRO" => {
                        let id = cursor.number()? as usize;
                        if id != score.macros.len() {
                            return cursor.error("macros must be numbered in order from 0");
                        }
                        cursor.skip_space();
                        let target = if cursor.at_assignment() {
                            let name = cursor.word()?;
                            cursor.eat(b'=');
                            Target::Param(name)
                        } else {
                            match cursor.word()?.as_str() {
                                "v" => Target::Level,
                                "p" => Target::Pan,
                                "P" => Target::Bend,
                                other => return cursor.error(format!("unknown macro target {other}")),
                            }
                        };
                        let mut steps = Vec::new();
                        loop {
                            cursor.skip_space();
                            if matches!(cursor.peek(), None | Some(b';')) {
                                break;
                            }
                            let offset = cursor.number()? as u64;
                            if !cursor.eat(b':') {
                                return cursor.error("macro steps are tick:value");
                            }
                            let value = cursor.word()?;
                            if target_event(&target, &value).is_none() {
                                return cursor.error(format!("invalid macro value {value:?}"));
                            }
                            steps.push((offset, value));
                        }
                        score.macros.push(Macro { target, steps });
                    }
                    "TRACK" => {
                        if score.quarter == 0 || score.beats == 0 {
                            return cursor.error("#TICKS and #BAR must precede the tracks");
                        }
                        let label = cursor.word()?;
                        let name = cursor.word()?;
                        let mut track = Track {
                            label: label.clone(),
                            name,
                            kind: "tonal".into(),
                            ..Track::default()
                        };
                        let mut default = None;
                        loop {
                            cursor.skip_space();
                            if matches!(cursor.peek(), None | Some(b';')) {
                                break;
                            }
                            if cursor.at_assignment() {
                                let key = cursor.word()?;
                                cursor.eat(b'=');
                                let value = cursor.word()?;
                                let number = || value.parse::<u32>().ok();
                                match key.as_str() {
                                    "channel" => track.channel = number().map_or_else(|| cursor.error("channel is a number"), Ok)?,
                                    "voice" => track.voice = number().map_or_else(|| cursor.error("voice is a number"), Ok)?,
                                    "kind" => track.kind = value,
                                    other => return cursor.error(format!("unknown track property {other}")),
                                }
                            } else if cursor.eat(b'l') {
                                default = Some(cursor.length(u64::from(score.quarter), 0)?);
                            } else {
                                return cursor.error("expected key=value or a default length");
                            }
                        }
                        let Some(default) = default.filter(|length| *length > 0) else {
                            return cursor.error("#TRACK needs a default length");
                        };
                        if label.is_empty() || !label.bytes().all(|b| b.is_ascii_uppercase()) {
                            return cursor.error("track labels are uppercase letters");
                        }
                        if labels.insert(label.clone(), score.tracks.len()).is_some() {
                            return cursor.error(format!("track {label} is declared twice"));
                        }
                        score.tracks.push(track);
                        states.push(TrackParse {
                            tick: 0,
                            octave: 4,
                            velocity: 0,
                            default,
                            instrument: None,
                            last: Vec::new(),
                            rest: false,
                        });
                    }
                    other => return cursor.error(format!("unknown header #{other}")),
                }
            }
            Some(_) => {
                if !seen_format {
                    return cursor.error(format!("missing #{FORMAT} header"));
                }
                header_done = true;
                let label = cursor.word()?;
                let Some(&index) = labels.get(&label) else {
                    return cursor.error(format!("undeclared track {label}"));
                };
                parse_track_line(&mut cursor, &mut score, index, &mut states[index])?;
            }
        }
    }
    if !seen_format {
        return Err(ParseError {
            line: 1,
            message: format!("missing #{FORMAT} header"),
        });
    }
    for (track, state) in score.tracks.iter().zip(&states) {
        if state.tick != score.length {
            return Err(ParseError {
                line: 0,
                message: format!(
                    "track {} lasts {} ticks, but #LENGTH is {}",
                    track.label, state.tick, score.length
                ),
            });
        }
    }
    Ok(score)
}

fn parse_track_line(
    cursor: &mut Cursor<'_>,
    score: &mut Score,
    index: usize,
    state: &mut TrackParse,
) -> Result<(), ParseError> {
    let quarter = u64::from(score.quarter.max(1));
    let mut bar_start: Option<u64> = None;
    loop {
        cursor.skip_space();
        let Some(byte) = cursor.peek() else {
            return Ok(());
        };
        let tick = state.tick;
        if cursor.at_assignment() {
            let name = cursor.word()?;
            cursor.eat(b'=');
            let value = cursor.word()?;
            score.tracks[index].events.push(Event { tick, kind: EventKind::Param(name, value) });
            continue;
        }
        cursor.at += 1;
        let events = &mut score.tracks[index].events;
        let mut command = |kind| events.push(Event { tick, kind });
        match byte {
            b';' => return Ok(()),
            b'|' => match bar_start {
                // Bar lines must fall on the bar grid; this catches lost ticks.
                None => {
                    let on_grid = state.tick == 0
                        || (state.tick >= score.pickup + score.bar_ticks()
                            && (state.tick - score.pickup) % score.bar_ticks() == 0);
                    if !on_grid {
                        return cursor.error("a bar starts off the bar grid");
                    }
                    bar_start = Some(state.tick);
                }
                Some(start) => {
                    let end = score.next_bar(start).min(score.length);
                    if state.tick != end {
                        return cursor.error(format!(
                            "bar holds {} ticks instead of {}",
                            state.tick - start,
                            end - start
                        ));
                    }
                    // Several bars share a line: this bar line opens the next.
                    bar_start = Some(state.tick);
                }
            },
            b'o' => state.octave = cursor.number()? as i32,
            b'>' => state.octave += 1,
            b'<' => state.octave -= 1,
            b'V' => state.velocity = cursor.number()?.clamp(0, 127) as u8,
            b'v' => command(EventKind::Level(cursor.number()? as i32)),
            b'p' => command(EventKind::Pan(cursor.number()? as i32)),
            b'P' => command(EventKind::Bend(cursor.number()? as i32)),
            b'@' => {
                let name = if cursor.peek() == Some(b'"') { cursor.string()? } else { cursor.word()? };
                state.instrument = Some(name.clone());
                command(EventKind::Instrument(name));
            }
            b'~' => {
                let id = cursor.number()? as usize;
                if id >= score.macros.len() {
                    return cursor.error(format!("undefined macro {id}"));
                }
                command(EventKind::Macro(id));
            }
            b'^' => {
                let length = cursor.length(quarter, state.default)?;
                if !state.rest {
                    for &last in &state.last {
                        match &mut score.tracks[index].events[last].kind {
                            EventKind::Note { length: l, .. } | EventKind::Hit { length: l, .. } => *l += length,
                            _ => {}
                        }
                    }
                }
                state.tick += length;
            }
            b'r' => {
                state.tick += cursor.length(quarter, state.default)?;
                state.rest = true;
            }
            b'x' => {
                let length = cursor.length(quarter, state.default)?;
                score.tracks[index].events.push(Event {
                    tick: state.tick,
                    kind: EventKind::Hit {
                        velocity: state.velocity,
                        length,
                    },
                });
                state.last = vec![score.tracks[index].events.len() - 1];
                state.rest = false;
                state.tick += length;
            }
            b'&' | b'a'..=b'g' => {
                let legato = byte == b'&';
                let name = if legato {
                    match cursor.peek() {
                        Some(name @ b'a'..=b'g') => {
                            cursor.at += 1;
                            name
                        }
                        _ => return cursor.error("& must precede a note"),
                    }
                } else {
                    byte
                };
                let shift = transpose_of(score, state);
                let (key, cents) = read_pitch(cursor, &score.tracks[index], state.octave, shift, name)?;
                let length = cursor.length(quarter, state.default)?;
                let events = &mut score.tracks[index].events;
                events.push(Event {
                    tick: state.tick,
                    kind: EventKind::Note { key, cents, velocity: state.velocity, legato, length },
                });
                state.last = vec![events.len() - 1];
                state.rest = false;
                state.tick += length;
            }
            b'\'' => {
                let shift = transpose_of(score, state);
                // Each note: key, cents, velocity, legato, and its own length
                // if it has one (otherwise it follows the track).
                let mut notes = Vec::new();
                loop {
                    let Some(next) = cursor.peek() else {
                        return cursor.error("a chord ends with '");
                    };
                    cursor.at += 1;
                    match next {
                        b'\'' => break,
                        b'o' => state.octave = cursor.number()? as i32,
                        b'>' => state.octave += 1,
                        b'<' => state.octave -= 1,
                        b'V' => state.velocity = cursor.number()?.clamp(0, 127) as u8,
                        b'&' | b'a'..=b'g' => {
                            let legato = next == b'&';
                            let name = if legato {
                                match cursor.peek() {
                                    Some(name @ b'a'..=b'g') => {
                                        cursor.at += 1;
                                        name
                                    }
                                    _ => return cursor.error("& must precede a note"),
                                }
                            } else {
                                next
                            };
                            let (key, cents) = read_pitch(cursor, &score.tracks[index], state.octave, shift, name)?;
                            let own = if cursor.peek().is_some_and(|b| b.is_ascii_digit()) {
                                Some(cursor.length(quarter, state.default)?)
                            } else {
                                None
                            };
                            notes.push((key, cents, state.velocity, legato, own));
                        }
                        _ => return cursor.error("a chord holds notes, o, <, >, V and &, and ends with '"),
                    }
                }
                if notes.is_empty() {
                    return cursor.error("a chord needs at least one note");
                }
                let length = cursor.length(quarter, state.default)?;
                let events = &mut score.tracks[index].events;
                state.last.clear();
                for (key, cents, velocity, legato, own) in notes {
                    if own.is_none() {
                        state.last.push(events.len());
                    }
                    events.push(Event {
                        tick: state.tick,
                        kind: EventKind::Note { key, cents, velocity, legato, length: own.unwrap_or(length) },
                    });
                }
                state.rest = false;
                state.tick += length;
            }
            other => {
                return cursor.error(format!("unexpected {:?}", char::from(other)));
            }
        }
    }
}

/// The `#TRANSPOSE` shift of the track's current instrument.
fn transpose_of(score: &Score, state: &TrackParse) -> i32 {
    state
        .instrument
        .as_ref()
        .and_then(|name| score.transpose.iter().find(|(n, _)| n == name))
        .map_or(0, |(_, shift)| *shift)
}

/// A note name already read, its accidentals and any `(cents)`: the key as
/// heard (after `#TRANSPOSE`) and its detune.
fn read_pitch(cursor: &mut Cursor<'_>, track: &Track, octave: i32, shift: i32, name: u8) -> Result<(i32, i32), ParseError> {
    let mut key = match name {
        b'c' => 0,
        b'd' => 2,
        b'e' => 4,
        b'f' => 5,
        b'g' => 7,
        b'a' => 9,
        _ => 11,
    } + (octave + 1) * 12;
    loop {
        if cursor.eat(b'+') || cursor.eat(b'#') {
            key += 1;
        } else if cursor.eat(b'-') {
            key -= 1;
        } else {
            break;
        }
    }
    let cents = if cursor.eat(b'(') {
        let cents = cursor.number()? as i32;
        if !cursor.eat(b')') {
            return cursor.error("expected ) after cents");
        }
        cents
    } else {
        track.tuning(key - shift)
    };
    Ok((key - shift, cents))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Channel, Field, FrameData, Note};

    fn note(key: i32, cents: i32, tick: u64, length: u64, legato: bool) -> Event {
        Event {
            tick,
            kind: EventKind::Note {
                key,
                cents,
                velocity: 100,
                legato,
                length,
            },
        }
    }

    fn sample_score() -> Score {
        Score {
            title: "Test \"tune\" é".into(),
            backend: "GME".into(),
            tempo: 150_000,
            quarter: 32,
            beats: 3,
            inferred: true,
            length: 300,
            tracks: vec![
                Track {
                    label: "A".into(),
                    channel: 0,
                    voice: 0,
                    name: "Pulse 1".into(),
                    kind: "tonal".into(),
                    pitched: Vec::new(),
                    tuning: Vec::new(),
                    events: vec![
                        Event { tick: 0, kind: EventKind::Instrument("Duty 25%".into()) },
                        Event { tick: 0, kind: EventKind::Level(750) },
                        note(60, 0, 0, 24, false),
                        note(64, 12, 24, 90, true),
                        Event { tick: 50, kind: EventKind::Bend(30) },
                        Event { tick: 100, kind: EventKind::Param("Sweep".into(), "on 3".into()) },
                        Event { tick: 150, kind: EventKind::Macro(0) },
                        note(-3, -50, 150, 7, false),
                        note(130, 0, 157, 143, false),
                    ],
                },
                Track {
                    label: "B".into(),
                    channel: 3,
                    voice: 0,
                    name: "Noise".into(),
                    kind: "noise".into(),
                    pitched: Vec::new(),
                    tuning: Vec::new(),
                    events: vec![
                        Event { tick: 12, kind: EventKind::Hit { velocity: 90, length: 6 } },
                        Event { tick: 96, kind: EventKind::Pan(-250) },
                        Event { tick: 96, kind: EventKind::Hit { velocity: 90, length: 200 } },
                        Event { tick: 300, kind: EventKind::Param("Period".into(), "7".into()) },
                    ],
                },
            ],
            macros: vec![Macro {
                target: Target::Param("Envelope \"x\"".into()),
                steps: vec![(1, "14".into()), (3, "1 3".into())],
            }],
            transpose: vec![("Duty 25%".into(), 24)],
            pickup: 0,
            timing: vec![(0, 0), (150, 1_400_000), (300, 2_350_500)],
        }
    }

    #[test]
    fn mml_round_trips_the_exact_score() {
        let score = sample_score();
        let document = encode(&score);
        assert!(document.text.is_ascii(), "{}", document.text);
        let parsed = parse(&document.text).unwrap_or_else(|e| panic!("{e}\n{}", document.text));
        assert_eq!(parsed, score, "\n{}", document.text);
        assert_eq!(parsed.piano_roll(), score.piano_roll());
        for bars in [1, 2, 3, 8] {
            let text = encode_lines(&score, bars).text;
            assert_eq!(parse(&text).unwrap(), score, "{bars} bars per line\n{text}");
        }
        // Four bars of 96 ticks share one block of lines.
        assert_eq!(document.bars.len(), 1);
        assert_eq!(document.text.lines().filter(|line| line.starts_with("A ")).next().unwrap().matches('|').count(), 5);
    }

    #[test]
    fn spans_follow_playback_position() {
        let score = sample_score();
        let document = encode(&score);
        let (active, bar) = document.active(score.seconds_at(30));
        assert_eq!(bar.unwrap().index, 0);
        let texts: Vec<_> = active.iter().map(|s| &document.text[s.from..s.to]).collect();
        // Every piece of the sounding note lights up, across its bar line.
        assert_eq!(texts, ["&e(+12)8^16^64", "^4^16^32^64", "^32", "^16.."]);
        let sounds = |active: &[&Span]| {
            let mut sounds: Vec<_> = active.iter().map(|s| (s.track, s.sound)).collect();
            sounds.dedup();
            sounds
        };
        assert_eq!(sounds(&active), [(0, Some(24))]);
        let (active, bar) = document.active(score.seconds_at(100));
        for tick in [0, 30, 149, 150, 151, 299, 400] {
            assert_eq!(document.tick_at(score.seconds_at(tick) + 1e-9), tick, "timing round trip at {tick}");
        }
        assert_eq!(bar.unwrap().index, 0);
        assert_eq!(sounds(&active), [(0, Some(24)), (1, Some(96))]);
    }

    /// Set KOG_MML_FIXTURE to a file path to export a document with the spans
    /// expected at several positions, for the Swift and Kotlin ports.
    #[test]
    fn export_active_span_fixture() {
        let Some(path) = std::env::var_os("KOG_MML_FIXTURE") else { return };
        let document = encode(&sample_score());
        let expected: Vec<_> = (0..40)
            .map(|step| {
                let seconds = f64::from(step) * 8.0 * document.tick_seconds;
                let (spans, bar) = document.active_indices(seconds);
                serde_json::json!({"seconds": seconds, "spans": spans, "bar": bar.map_or(-1, |bar| bar.index as i64)})
            })
            .collect();
        let reply = serde_json::json!({
            "status": "ready", "revision": 3, "recorded_ms": 1000, "total_ms": 1000,
            "document": document, "expected": expected,
        });
        std::fs::write(path, reply.to_string()).unwrap();
    }

    /// Random scores with every event kind, odd lengths, and commands inside
    /// notes and on bar lines must survive encode -> parse unchanged.
    #[test]
    fn random_scores_round_trip() {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = |limit: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % limit.max(1)
        };
        for case in 0..300 {
            let quarter = [32, 64, 96, 128][next(4) as usize];
            let mut score = Score {
                title: format!("case {case}"),
                backend: "Random".into(),
                tempo: 1 + next(400_000) as u32,
                quarter,
                beats: 1 + next(6) as u32,
                pickup: next(3) * next(40),
                timing: {
                    let mut timing: Vec<(u64, u64)> = (0..next(4)).map(|i| (i * 97 + next(50), i * 500_000 + next(400_000))).collect();
                    timing.dedup_by_key(|(tick, _)| *tick);
                    timing
                },
                transpose: (0..next(3)).map(|i| (format!("I{i} \"q\" ü"), (next(11) as i32 - 5) * 12)).collect(),
                inferred: next(2) == 0,
                length: 0,
                tracks: Vec::new(),
                macros: (0..next(4))
                    .map(|i| Macro {
                        target: match i % 4 {
                            0 => Target::Level,
                            1 => Target::Pan,
                            2 => Target::Bend,
                            _ => Target::Param(format!("M{i} ü")),
                        },
                        steps: (0..1 + next(5)).map(|j| (j * next(9), (next(200) as i32 - 100).to_string())).collect(),
                    })
                    .collect(),
            };
            let length = 1 + next(2000);
            score.length = length;
            for index in 0..(1 + next(4) as usize) {
                let mut events = Vec::new();
                let mut tick = 0;
                while tick < length {
                    tick += next(3) * next(40);
                    if tick > length {
                        break;
                    }
                    for _ in 0..next(3) {
                        events.push(Event {
                            tick,
                            kind: match next(6) {
                                5 if !score.macros.is_empty() => {
                                    EventKind::Macro(next(score.macros.len() as u64) as usize)
                                }
                                0 => EventKind::Bend(next(201) as i32 - 100),
                                1 => EventKind::Level(next(1001) as i32),
                                2 => EventKind::Pan(next(2001) as i32 - 1000),
                                3 => EventKind::Instrument(format!("I{} \"q\" ü", next(9))),
                                _ => EventKind::Param(format!("R{}", next(9)), format!("{:04X}", next(65536))),
                            },
                        });
                    }
                    if tick == length {
                        break;
                    }
                    let duration = 1 + next((length - tick).min(300));
                    events.push(Event {
                        tick,
                        kind: if next(4) == 0 {
                            EventKind::Hit { velocity: next(128) as u8, length: duration }
                        } else {
                            EventKind::Note {
                                key: next(140) as i32 - 10,
                                cents: next(101) as i32 - 50,
                                velocity: next(128) as u8,
                                legato: next(3) == 0,
                                length: duration,
                            }
                        },
                    });
                    // Chords: more notes starting together, some held on past
                    // the next sound or let go early.
                    if matches!(events.last().map(|e| &e.kind), Some(EventKind::Note { .. })) {
                        for _ in 0..[0, 0, 1, 2, 3][next(5) as usize] {
                            events.push(Event {
                                tick,
                                kind: EventKind::Note {
                                    key: next(140) as i32 - 10,
                                    cents: next(101) as i32 - 50,
                                    velocity: next(128) as u8,
                                    legato: next(3) == 0,
                                    length: match next(3) {
                                        // Never past the song's end.
                                        0 => 1 + next((duration * 3).min(length - tick)),
                                        _ => duration,
                                    },
                                },
                            });
                        }
                    }
                    if duration > 1 && next(2) == 0 {
                        events.push(Event {
                            tick: tick + 1 + next(duration - 1),
                            kind: EventKind::Bend(next(101) as i32 - 50),
                        });
                    }
                    tick += duration;
                }
                score.tracks.push(Track {
                    label: track_label(index),
                    channel: index as u32,
                    voice: next(3) as u32,
                    name: format!("Voice {index}"),
                    kind: "tonal".into(),
                    events,
                    tuning: {
                        let mut tuning: Vec<(i32, i32)> = (0..next(4)).map(|k| (k as i32 * 13 - 10, next(41) as i32 - 20)).collect();
                        tuning.dedup_by_key(|(key, _)| *key);
                        tuning
                    },
                    pitched: (0..next(2))
                        .map(|i| PitchedParam {
                            name: format!("Period {i}"),
                            values: (0..1 + next(5)).map(|k| (k as i32 * 7 - 10, next(3) as i32 - 1, format!("{:03X}", next(4096)))).collect(),
                        })
                        .collect(),
                });
            }
            let text = encode(&score).text;
            let parsed = parse(&text).unwrap_or_else(|e| panic!("case {case}: {e}\n{text}"));
            assert_eq!(parsed, score, "case {case}\n{text}");
        }
    }

    #[test]
    fn parser_reports_lost_ticks() {
        let mut text = encode(&sample_score()).text;
        let at = text.find(" c4").or_else(|| text.find(" c")).unwrap();
        text.insert_str(at, " r%1");
        assert!(parse(&text).is_err());
    }

    #[test]
    fn chords_hold_notes_and_ties_extend_only_following_notes() {
        let text = "#KOG-MML 1\n#TEMPO 120.000\n#TICKS 96\n#BAR 4\n#LENGTH 768\n\
                    #TRACK A \"Piano\" channel=0 voice=0 kind=tonal l4\n\
                    A | V90 o4 'c1eV70g'4 ^4 '&a<b'2 | '>c(+5)e'1 |\n";
        let score = parse(text).unwrap();
        let notes: Vec<(u64, i32, i32, u8, bool, u64)> = score.tracks[0]
            .events
            .iter()
            .filter_map(|event| match event.kind {
                EventKind::Note { key, cents, velocity, legato, length } => Some((event.tick, key, cents, velocity, legato, length)),
                _ => None,
            })
            .collect();
        assert_eq!(
            notes,
            vec![
                // c holds a whole note; e and g follow the track and the tie.
                (0, 60, 0, 90, false, 384),
                (0, 64, 0, 90, false, 192),
                (0, 67, 0, 70, false, 192),
                (192, 69, 0, 70, true, 192),
                (192, 59, 0, 70, false, 192),
                (384, 60, 5, 70, false, 384),
                (384, 64, 0, 70, false, 384),
            ]
        );
        assert_eq!(parse(&encode(&score).text).unwrap(), score);
        let document = encode(&score);
        let chord = document.spans.iter().find(|span| document.text[span.from..span.to].starts_with('\'')).unwrap();
        // The chord stays lit while its held note sounds.
        assert_eq!((chord.start, chord.end), (0, 384));
    }

    fn frame(time: f64, channels: Vec<Channel>) -> TimedFrame {
        TimedFrame {
            time,
            data: FrameData {
                channels,
                ..FrameData::default()
            },
        }
    }

    fn voice(key: Option<f32>, envelope: u32, key_on: u32) -> Channel {
        Channel {
            id: 0,
            name: "Pulse".into(),
            kind: "tonal".into(),
            active: key.is_some(),
            level: 0.5,
            instrument: "Duty 50%".into(),
            notes: key
                .map(|key| Note { key, velocity: 0.5, held: true })
                .into_iter()
                .collect(),
            fields: vec![
                Field::new("Envelope", envelope),
                Field::new("Key on", key_on),
                Field::new("Duty", "2"),
            ],
            ..Channel::default()
        }
    }

    #[test]
    fn frames_become_notes_bends_and_retriggers_without_envelope_spam() {
        let mut frames = Vec::new();
        // 60 frames per second: a held C4 with a moving envelope, a slide,
        // a same-pitch retrigger, a release, then a legato step to D4.
        for i in 0..240u32 {
            let time = f64::from(i) / 60.0;
            let channel = match i {
                0..60 => voice(Some(60.0), i, 1),
                60..90 => voice(Some(60.0 + (i - 60) as f32 * 0.01), i, 1),
                90..120 => voice(Some(60.0), i, 2),
                120..150 => voice(None, 0, 2),
                150..180 => voice(Some(62.0), i, 3),
                _ => voice(Some(64.0), i, 3),
            };
            frames.push(frame(time, vec![channel]));
        }
        let description = Description {
            backend: "Test".into(),
            ..Description::default()
        };
        let score = score_from_frames(&frames, &description, "t", 48_000, Some(4.0));
        // Onsets every half second become quarter notes of 120 BPM.
        assert_eq!((score.tempo, score.quarter, score.inferred), (120_000, QUARTER, true));
        let seconds = |tick: u64| tick as f64 * score.tick_seconds();
        let roll = score.piano_roll();
        let notes: Vec<_> = roll.iter().map(|n| (n.cents, seconds(n.start), seconds(n.end))).collect();
        assert_eq!(
            notes,
            vec![
                (Some(6000), 0.0, 1.5),
                (Some(6000), 1.5, 2.0),
                (Some(6200), 2.5, 3.0),
                (Some(6400), 3.0, 4.0),
            ]
        );
        let events = &score.tracks[0].events;
        let onsets: Vec<u64> = roll.iter().map(|n| n.start).collect();
        assert!(
            events
                .iter()
                .filter(|e| matches!(&e.kind, EventKind::Param(name, _) if name == "Envelope"))
                .all(|e| onsets.contains(&e.tick)),
            "envelope steps are recorded at key-on only"
        );
        let expanded = score.expanded_events(&score.tracks[0]);
        // Commands near a grid step snap to it, so allow one sixteenth.
        assert!(expanded
            .iter()
            .any(|e| e.kind == EventKind::Bend(29) && (seconds(e.tick) - 89.0 / 60.0).abs() <= 0.125));
        assert!(events.iter().any(|e| matches!(e.kind, EventKind::Note { key: 64, legato: true, .. })));
        let parsed = parse(&encode(&score).text).unwrap();
        assert_eq!(parsed, score);
    }
}
