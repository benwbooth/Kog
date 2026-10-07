//! Kog MML: one text notation for every chip and sequencer Kog can inspect.
//!
//! A [`Score`] is the piano roll recorded from the decoder that produced the
//! audio: per-voice notes on an integer tick grid, plus the chip parameters
//! that changed while they played. [`encode`] writes it as MML split into
//! tracks and bars, and [`parse`] reads that text back into the identical
//! score, so the notation can be edited or stored without losing events.
//!
//! The syntax follows classic MML (`o4 l8 c d+ e-4. r ^`) with these
//! extensions:
//!
//! | Syntax | Meaning |
//! | --- | --- |
//! | `%n` | length of exactly `n` ticks |
//! | `c(+37)` | note detuned by +37 cents at its onset |
//! | `&c` | legato: the pitch changes without a new key-on |
//! | `^8` | continue the previous note or rest |
//! | `x` | unpitched hit (noise, drums, untuned samples) |
//! | `P+12` | pitch offset in cents from the sounding note's semitone |
//! | `V100` | key-on velocity, 0-127 |
//! | `v750` / `p-250` | channel level / pan in thousandths |
//! | `@"Duty 25%"` | instrument |
//! | `{"Name"="value" …}` | chip-specific parameters |
//! | `~3` | start macro 3: the steps of `#MACRO 3`, timed from this tick |
//! | `\|` | bar line |
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use crate::{Description, TimedFrame};

pub const FORMAT: &str = "KOG-MML 1";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Score {
    pub title: String,
    pub backend: String,
    /// Ticks are `tick_samples / rate` seconds long.
    pub rate: u32,
    pub tick_samples: u32,
    /// Ticks per quarter note and per bar. Bars only lay out the text; event
    /// times are absolute ticks and do not depend on them.
    pub quarter: u32,
    pub bar: u32,
    /// True when the beat was estimated from note onsets.
    pub inferred: bool,
    pub length: u64,
    pub tracks: Vec<Track>,
    /// Per-note automation (envelopes, vibrato, duty sequences) shared by
    /// every note that repeats it.
    pub macros: Vec<Macro>,
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
        f64::from(self.tick_samples.max(1)) / f64::from(self.rate.max(1))
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
            self.previous.insert(channel.id, channel.clone());
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
        let (rate, tick_samples, tick_seconds) = self.timebase.unwrap_or((self.rate, 1, 1.0));
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
        for (id, state) in channels {
            for (voice, events) in state.events.into_iter().enumerate() {
                if events.is_empty() && voice > 0 {
                    continue;
                }
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
                });
            }
        }
        tracks.retain(|track| !track.events.is_empty());
        let mut macros = Vec::new();
        for track in &mut tracks {
            fold_macros(&mut track.events, &mut macros);
        }
        for (index, track) in tracks.iter_mut().enumerate() {
            track.label = track_label(index);
        }

        let (quarter, inferred) = match self.tempo {
            Some(bpm) => (((60.0 / bpm) / tick_seconds).round().max(1.0) as u32, false),
            None => (infer_quarter(&tracks, tick_seconds), true),
        };
        let bar = quarter * self.meter.unwrap_or(4).max(1);
        Score {
            title: title.into(),
            backend: description.backend.clone(),
            rate,
            tick_samples,
            quarter,
            bar,
            inferred,
            length: end,
            tracks,
            macros,
        }
    }
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
    for (sound, start, end) in sounds {
        let mut groups: Vec<(Target, Vec<usize>)> = Vec::new();
        for (index, event) in events.iter().enumerate().skip(sound + 1) {
            if event.tick >= end {
                break;
            }
            if event.tick <= start {
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
    pub index: usize,
    pub start: u64,
    pub end: u64,
    pub from: usize,
    pub to: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub text: String,
    pub tick_seconds: f64,
    pub spans: Vec<Span>,
    pub bars: Vec<Bar>,
    pub tracks: Vec<TrackInfo>,
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
        let tick = (seconds.max(0.0) / self.tick_seconds.max(f64::MIN_POSITIVE)) as u64;
        let sounding: Vec<(usize, u64)> = self
            .spans
            .iter()
            .filter(|span| span.start <= tick && tick < span.end)
            .filter_map(|span| span.sound.map(|sound| (span.track, sound)))
            .collect();
        let spans = self
            .spans
            .iter()
            .filter(|span| span.sound.is_some_and(|sound| sounding.contains(&(span.track, sound))))
            .collect();
        let bar = self.bars.iter().find(|bar| bar.start <= tick && tick < bar.end);
        (spans, bar)
    }
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

fn length_text(ticks: u64, quarter: u64, default: Option<u64>) -> String {
    if Some(ticks) == default {
        return String::new();
    }
    let whole = quarter * 4;
    for divisor in [1, 2, 4, 8, 16, 32, 64] {
        if whole % divisor == 0 && whole / divisor == ticks {
            return divisor.to_string();
        }
        let plain = whole / divisor;
        if whole % (divisor * 2) == 0 && plain + plain / 2 == ticks {
            return format!("{divisor}.");
        }
    }
    format!("%{ticks}")
}

fn standard_length(ticks: u64, quarter: u64) -> bool {
    !length_text(ticks, quarter, None).starts_with('%')
}

const NAMES: [&str; 12] = ["c", "c+", "d", "d+", "e", "f", "f+", "g", "g+", "a", "a+", "b"];

struct Writer<'a> {
    score: &'a Score,
    text: String,
    spans: Vec<Span>,
    octave: Vec<i32>,
    velocity: Vec<Option<u8>>,
    default: Vec<u64>,
}

impl Writer<'_> {
    fn token(&mut self, track: usize, start: u64, end: u64, kind: &str, text: &str) {
        self.sound_token(track, start, end, kind, text, None);
    }

    fn sound_token(
        &mut self,
        track: usize,
        start: u64,
        end: u64,
        kind: &str,
        text: &str,
        sound: Option<u64>,
    ) {
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
            EventKind::Instrument(name) => format!("@{}", quote(name)),
            EventKind::Param(name, value) => format!("{{{}={}}}", quote(name), quote(value)),
            EventKind::Macro(id) => format!("~{id}"),
            _ => return,
        };
        self.token(track, tick, tick, "command", &text);
    }

    fn sound(
        &mut self,
        track: usize,
        start: u64,
        ticks: u64,
        kind: &EventKind,
        first: bool,
        onset: Option<u64>,
    ) {
        let quarter = u64::from(self.score.quarter);
        let length = length_text(ticks, quarter, Some(self.default[track]));
        let text = match (first, kind) {
            (false, _) => format!("^{}", length_text(ticks, quarter, None)),
            (true, EventKind::Note {
                key,
                cents,
                velocity,
                legato,
                ..
            }) => {
                let mut text = String::new();
                if self.velocity[track] != Some(*velocity) {
                    self.velocity[track] = Some(*velocity);
                    let _ = write!(text, "V{velocity} ");
                }
                let octave = key.div_euclid(12) - 1;
                let current = self.octave[track];
                if octave == current + 1 {
                    text.push_str("> ");
                } else if octave == current - 1 {
                    text.push_str("< ");
                } else if octave != current {
                    let _ = write!(text, "o{octave} ");
                }
                self.octave[track] = octave;
                if *legato {
                    text.push('&');
                }
                text.push_str(NAMES[key.rem_euclid(12) as usize]);
                if *cents != 0 {
                    let _ = write!(text, "({cents:+})");
                }
                text.push_str(&length);
                text
            }
            (true, EventKind::Hit { velocity, .. }) => {
                let mut text = String::new();
                if self.velocity[track] != Some(*velocity) {
                    self.velocity[track] = Some(*velocity);
                    let _ = write!(text, "V{velocity} ");
                }
                text.push('x');
                text.push_str(&length);
                text
            }
            _ => format!("r{length}"),
        };
        let kind = match kind {
            EventKind::Note { .. } => "note",
            EventKind::Hit { .. } => "hit",
            _ => "rest",
        };
        // Velocity and octave prefixes are separate tokens for highlighting.
        let (prefix, sound) = text.rsplit_once(' ').map_or(("", text.as_str()), |(a, b)| (a, b));
        for word in prefix.split_whitespace() {
            self.token(track, start, start, "command", word);
        }
        self.sound_token(track, start, start + ticks, kind, sound, onset);
    }
}

enum Item {
    Command(u64, usize),
    /// start, end, the sound's event (None for a gap), first piece of it
    Piece(u64, u64, Option<usize>, bool),
}

/// A track as commands and sound pieces in time order. Sounds and gaps are
/// split where a command falls inside them and at bar lines.
fn items(events: &[Event], length: u64, bar: u64) -> Vec<Item> {
    let mut sounds = Vec::new();
    let mut cursor = 0;
    for (index, event) in events.iter().enumerate() {
        if event.kind.length() > 0 {
            if event.tick > cursor {
                sounds.push((cursor, event.tick, None));
            }
            let end = event.tick + event.kind.length();
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
            let bar_end = (piece_start / bar + 1) * bar;
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

pub fn encode(score: &Score) -> Document {
    let quarter = u64::from(score.quarter.max(1));
    let bar = u64::from(score.bar.max(1));
    let tracks = score.tracks.len();
    let mut writer = Writer {
        score,
        text: String::new(),
        spans: Vec::new(),
        octave: vec![4; tracks],
        velocity: vec![None; tracks],
        default: Vec::with_capacity(tracks),
    };
    let text = &mut writer.text;
    let _ = writeln!(text, "#{FORMAT}");
    let _ = writeln!(text, "#TITLE {}", quote(&score.title));
    let _ = writeln!(text, "#SOURCE {}", quote(&score.backend));
    let _ = writeln!(
        text,
        "#TIMEBASE {} {} ; tick = {} / {} s",
        score.rate, score.tick_samples, score.tick_samples, score.rate
    );
    let _ = writeln!(
        text,
        "#METER {} {}{} ; ticks per quarter, per bar; {:.2} BPM",
        score.quarter,
        score.bar,
        if score.inferred { " inferred" } else { "" },
        60.0 / (f64::from(score.quarter.max(1)) * score.tick_seconds())
    );
    let _ = writeln!(text, "#LENGTH {}", score.length);
    for track in &score.tracks {
        let lengths = track.events.iter().map(|e| e.kind.length()).filter(|l| *l > 0);
        let default = mode(lengths.filter(|l| standard_length(*l, quarter))).unwrap_or(quarter);
        writer.default.push(default);
        let _ = writeln!(
            writer.text,
            "#TRACK {} {} {} {} {} l{}",
            track.label,
            track.channel,
            track.voice,
            quote(&track.kind),
            quote(&track.name),
            length_text(default, quarter, None)
        );
    }
    for (id, found) in score.macros.iter().enumerate() {
        let target = match &found.target {
            Target::Level => "v".to_owned(),
            Target::Pan => "p".to_owned(),
            Target::Bend => "P".to_owned(),
            Target::Param(name) => format!("{{{}}}", quote(name)),
        };
        let _ = write!(writer.text, "#MACRO {id} {target}");
        for (offset, value) in &found.steps {
            let _ = write!(writer.text, " {offset}:{}", quote(value));
        }
        writer.text.push('\n');
    }
    let label_width = score.tracks.iter().map(|t| t.label.len()).max().unwrap_or(1);
    let plans: Vec<_> = score
        .tracks
        .iter()
        .map(|track| items(&track.events, score.length, bar))
        .collect();
    let mut cursors = vec![0usize; tracks];
    let mut bars = Vec::new();
    let bar_count = score.length.div_ceil(bar).max(1) as usize;
    for index in 0..bar_count {
        let start = index as u64 * bar;
        let end = (start + bar).min(score.length).max(start);
        let last = index + 1 == bar_count;
        let from = writer.text.len();
        let seconds = start as f64 * score.tick_seconds();
        let _ = writeln!(
            writer.text,
            "; bar {} {}:{:06.3}",
            index + 1,
            (seconds / 60.0) as u64,
            seconds % 60.0
        );
        for (track_index, track) in score.tracks.iter().enumerate() {
            let _ = write!(writer.text, "{:<label_width$} |", track.label);
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
                        let kind = sound.map_or(EventKind::Bend(0), |i| track.events[i].kind.clone());
                        writer.sound(
                            track_index,
                            piece_start,
                            piece_end - piece_start,
                            &kind,
                            first,
                            sound.map(|i| track.events[i].tick),
                        );
                    }
                }
                cursor += 1;
            }
            cursors[track_index] = cursor;
            writer.text.push_str(" |\n");
        }
        bars.push(Bar {
            index,
            start,
            end,
            from,
            to: writer.text.len(),
        });
    }
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
    }
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
    fn word(&mut self) -> &str {
        self.skip_space();
        let start = self.at;
        while self.peek().is_some_and(|b| !b.is_ascii_whitespace()) {
            self.at += 1;
        }
        std::str::from_utf8(&self.bytes[start..self.at]).unwrap_or_default()
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
            Err(_) => self.error(format!("expected a number at {:?}", text)),
        }
    }
    fn string(&mut self) -> Result<String, ParseError> {
        self.skip_space();
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
                            let character = u32::from_str_radix(hex, 16)
                                .ok()
                                .and_then(char::from_u32);
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
                    // Strings are written as ASCII, but accept UTF-8 input.
                    let rest = std::str::from_utf8(&self.bytes[self.at..]).unwrap_or_default();
                    let character = rest.chars().next().unwrap_or(char::from(byte));
                    self.at += character.len_utf8();
                    out.push(character);
                }
            }
        }
    }
    fn length(&mut self, quarter: u64, default: u64) -> Result<u64, ParseError> {
        if self.eat(b'%') {
            let ticks = self.number()?;
            return if ticks > 0 {
                Ok(ticks as u64)
            } else {
                self.error("a %length must be positive")
            };
        }
        if !self.peek().is_some_and(|b| b.is_ascii_digit()) {
            return Ok(default);
        }
        let divisor = self.number()? as u64;
        let whole = quarter * 4;
        if divisor == 0 || whole % divisor != 0 {
            return self.error(format!("length {divisor} is not a whole number of ticks"));
        }
        let mut ticks = whole / divisor;
        if self.eat(b'.') {
            if ticks % 2 != 0 {
                return self.error("dotted length is not a whole number of ticks");
            }
            ticks += ticks / 2;
        }
        Ok(ticks)
    }
}

struct TrackParse {
    tick: u64,
    octave: i32,
    velocity: u8,
    default: u64,
    /// Index of the last sound event, for `^` continuation.
    last: Option<usize>,
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
                let name = cursor.word().to_owned();
                match name.as_str() {
                    "KOG-MML" => {
                        if cursor.word() != "1" {
                            return cursor.error("unsupported KOG-MML version");
                        }
                        seen_format = true;
                    }
                    "TITLE" => score.title = cursor.string()?,
                    "SOURCE" => score.backend = cursor.string()?,
                    "TIMEBASE" => {
                        cursor.skip_space();
                        score.rate = cursor.number()? as u32;
                        cursor.skip_space();
                        score.tick_samples = cursor.number()? as u32;
                    }
                    "METER" => {
                        cursor.skip_space();
                        score.quarter = cursor.number()? as u32;
                        cursor.skip_space();
                        score.bar = cursor.number()? as u32;
                        cursor.skip_space();
                        score.inferred = line[cursor.at..].starts_with("inferred");
                        if score.quarter == 0 || score.bar == 0 {
                            return cursor.error("#METER values must be positive");
                        }
                    }
                    "LENGTH" => {
                        cursor.skip_space();
                        score.length = cursor.number()? as u64;
                    }
                    "MACRO" => {
                        cursor.skip_space();
                        let id = cursor.number()? as usize;
                        if id != score.macros.len() {
                            return cursor.error("macros must be numbered in order from 0");
                        }
                        cursor.skip_space();
                        let target = match cursor.peek() {
                            Some(b'v') => Target::Level,
                            Some(b'p') => Target::Pan,
                            Some(b'P') => Target::Bend,
                            Some(b'{') => {
                                cursor.at += 1;
                                let name = cursor.string()?;
                                if !cursor.eat(b'}') {
                                    return cursor.error("expected } after the parameter name");
                                }
                                cursor.at -= 1;
                                Target::Param(name)
                            }
                            _ => return cursor.error("a macro targets v, p, P, or {\"name\"}"),
                        };
                        cursor.at += 1;
                        let mut steps = Vec::new();
                        loop {
                            cursor.skip_space();
                            match cursor.peek() {
                                None | Some(b';') => break,
                                _ => {}
                            }
                            let offset = cursor.number()? as u64;
                            if !cursor.eat(b':') {
                                return cursor.error("macro steps are tick:\"value\"");
                            }
                            let value = cursor.string()?;
                            if target_event(&target, &value).is_none() {
                                return cursor.error(format!("invalid macro value {value:?}"));
                            }
                            steps.push((offset, value));
                        }
                        score.macros.push(Macro { target, steps });
                    }
                    "TRACK" => {
                        let label = cursor.word().to_owned();
                        cursor.skip_space();
                        let channel = cursor.number()? as u32;
                        cursor.skip_space();
                        let voice = cursor.number()? as u32;
                        let kind = cursor.string()?;
                        let name = cursor.string()?;
                        cursor.skip_space();
                        if !cursor.eat(b'l') {
                            return cursor.error("#TRACK needs a default length");
                        }
                        let default = cursor.length(u64::from(score.quarter.max(1)), 0)?;
                        if label.is_empty() || !label.bytes().all(|b| b.is_ascii_uppercase()) {
                            return cursor.error("track labels are uppercase letters");
                        }
                        if labels.insert(label.clone(), score.tracks.len()).is_some() {
                            return cursor.error(format!("track {label} is declared twice"));
                        }
                        score.tracks.push(Track {
                            label,
                            channel,
                            voice,
                            name,
                            kind,
                            events: Vec::new(),
                        });
                        states.push(TrackParse {
                            tick: 0,
                            octave: 4,
                            velocity: 0,
                            default,
                            last: None,
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
                let label = cursor.word().to_owned();
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
    let bar = u64::from(score.bar.max(1));
    let mut bar_start: Option<u64> = None;
    loop {
        cursor.skip_space();
        let Some(byte) = cursor.peek() else {
            return Ok(());
        };
        cursor.at += 1;
        let events = &mut score.tracks[index].events;
        let tick = state.tick;
        let mut command = |kind| events.push(Event { tick, kind });
        match byte {
            b';' => return Ok(()),
            b'|' => {
                // Bar lines must fall on the bar grid; this catches lost ticks.
                match bar_start {
                    None => {
                        if state.tick % bar != 0 {
                            return cursor.error("a bar starts off the bar grid");
                        }
                        bar_start = Some(state.tick);
                    }
                    Some(start) => {
                        if state.tick != (start + bar).min(score.length) {
                            return cursor.error(format!(
                                "bar holds {} ticks instead of {}",
                                state.tick - start,
                                bar
                            ));
                        }
                    }
                }
            }
            b'o' => state.octave = cursor.number()? as i32,
            b'>' => state.octave += 1,
            b'<' => state.octave -= 1,
            b'V' => state.velocity = cursor.number()?.clamp(0, 127) as u8,
            b'v' => command(EventKind::Level(cursor.number()? as i32)),
            b'p' => command(EventKind::Pan(cursor.number()? as i32)),
            b'P' => command(EventKind::Bend(cursor.number()? as i32)),
            b'@' => command(EventKind::Instrument(cursor.string()?)),
            b'~' => {
                let id = cursor.number()? as usize;
                if id >= score.macros.len() {
                    return cursor.error(format!("undefined macro {id}"));
                }
                score.tracks[index].events.push(Event {
                    tick: state.tick,
                    kind: EventKind::Macro(id),
                });
            }
            b'{' => loop {
                cursor.skip_space();
                if cursor.eat(b'}') {
                    break;
                }
                let name = cursor.string()?;
                if !cursor.eat(b'=') {
                    return cursor.error("expected = in a parameter");
                }
                let value = cursor.string()?;
                score.tracks[index].events.push(Event {
                    tick: state.tick,
                    kind: EventKind::Param(name, value),
                });
            },
            b'^' => {
                let length = cursor.length(quarter, state.default)?;
                if state.rest || state.last.is_none() {
                    state.rest = true;
                } else if let Some(last) = state.last {
                    match &mut score.tracks[index].events[last].kind {
                        EventKind::Note { length: l, .. } | EventKind::Hit { length: l, .. } => *l += length,
                        _ => {}
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
                state.last = Some(score.tracks[index].events.len() - 1);
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
                let mut key = match name {
                    b'c' => 0,
                    b'd' => 2,
                    b'e' => 4,
                    b'f' => 5,
                    b'g' => 7,
                    b'a' => 9,
                    _ => 11,
                } + (state.octave + 1) * 12;
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
                    0
                };
                let length = cursor.length(quarter, state.default)?;
                score.tracks[index].events.push(Event {
                    tick: state.tick,
                    kind: EventKind::Note {
                        key,
                        cents,
                        velocity: state.velocity,
                        legato,
                        length,
                    },
                });
                state.last = Some(score.tracks[index].events.len() - 1);
                state.rest = false;
                state.tick += length;
            }
            other => {
                return cursor.error(format!("unexpected {:?}", char::from(other)));
            }
        }
    }
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
            rate: 48_000,
            tick_samples: 800,
            quarter: 24,
            bar: 96,
            inferred: true,
            length: 300,
            tracks: vec![
                Track {
                    label: "A".into(),
                    channel: 0,
                    voice: 0,
                    name: "Pulse 1".into(),
                    kind: "tonal".into(),
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
        assert_eq!(document.bars.len(), 4);
    }

    #[test]
    fn spans_follow_playback_position() {
        let document = encode(&sample_score());
        let (active, bar) = document.active(30.0 * document.tick_seconds);
        assert_eq!(bar.unwrap().index, 0);
        let texts: Vec<_> = active.iter().map(|s| &document.text[s.from..s.to]).collect();
        // Every piece of the sounding note lights up, across its bar line.
        assert_eq!(texts, ["&e(+12)%26", "^%46", "^%4", "^%14"]);
        let sounds = |active: &[&Span]| {
            let mut sounds: Vec<_> = active.iter().map(|s| (s.track, s.sound)).collect();
            sounds.dedup();
            sounds
        };
        assert_eq!(sounds(&active), [(0, Some(24))]);
        let (active, bar) = document.active(100.0 * document.tick_seconds);
        assert_eq!(bar.unwrap().index, 1);
        assert_eq!(sounds(&active), [(0, Some(24)), (1, Some(96))]);
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
            let quarter = [1, 3, 12, 24, 25, 48, 96][next(7) as usize];
            let bar = quarter * (1 + next(6)) as u32;
            let mut score = Score {
                title: format!("case {case}"),
                backend: "Random".into(),
                rate: 44_100,
                tick_samples: 1 + next(900) as u32,
                quarter,
                bar,
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
        assert_eq!(score.tick_samples, 800);
        let roll = score.piano_roll();
        let starts: Vec<_> = roll.iter().map(|n| (n.cents, n.start, n.end)).collect();
        assert_eq!(
            starts,
            vec![
                (Some(6000), 0, 90),
                (Some(6000), 90, 120),
                (Some(6200), 150, 180),
                (Some(6400), 180, 240),
            ]
        );
        let events = &score.tracks[0].events;
        assert!(
            !events.iter().any(|e| matches!(&e.kind, EventKind::Param(name, _) if name == "Envelope")
                && e.tick % 60 != 0
                && !matches!(e.tick, 0 | 90 | 150 | 180)),
            "envelope steps are recorded at key-on only"
        );
        let expanded = score.expanded_events(&score.tracks[0]);
        assert!(expanded.iter().any(|e| e.kind == EventKind::Bend(29) && e.tick == 89));
        assert!(
            !events.iter().any(|e| matches!(e.kind, EventKind::Bend(_))),
            "the slide inside one note becomes a macro"
        );
        assert!(events.iter().any(|e| matches!(e.kind, EventKind::Note { key: 64, legato: true, .. })));
        let parsed = parse(&encode(&score).text).unwrap();
        assert_eq!(parsed, score);
    }
}
