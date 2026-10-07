//! MIDI command state, evaluated at the audible position for every MIDI synth.
//! These are sequenced keys and controllers, not estimates of synth release tails.
use std::collections::BTreeMap;

use super::{Cell, Channel, Field, Note, Row, Snapshot, note_name};
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

#[derive(Clone)]
struct Voice {
    presses: u16,
    velocity: u8,
    sustained: bool,
    sostenuto: bool,
}

#[derive(Clone)]
struct MidiChannel {
    voices: BTreeMap<u8, Voice>,
    controls: [u8; 128],
    program: u8,
    bend: i16,
    bend_semitones: u8,
    bend_cents: u8,
    fine_tune: u16,
    coarse_tune: u8,
    pressure: u8,
}

impl Default for MidiChannel {
    fn default() -> Self {
        let mut controls = [0; 128];
        controls[7] = 100;
        controls[10] = 64;
        controls[11] = 127;
        controls[100] = 127;
        controls[101] = 127;
        Self {
            voices: BTreeMap::new(),
            controls,
            program: 0,
            bend: 0,
            bend_semitones: 2,
            bend_cents: 0,
            fine_tune: 8192,
            coarse_tune: 64,
            pressure: 0,
        }
    }
}

impl MidiChannel {
    fn release(&mut self, key: u8) {
        if let Some(voice) = self.voices.get_mut(&key) {
            voice.presses = voice.presses.saturating_sub(1);
            voice.sustained = self.controls[64] >= 64;
        }
        self.trim();
    }

    fn trim(&mut self) {
        self.voices
            .retain(|_, voice| voice.presses > 0 || voice.sustained || voice.sostenuto);
    }

    fn apply(&mut self, message: MidiMessage) {
        match message {
            MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                let voice = self.voices.entry(key.as_int()).or_insert(Voice {
                    presses: 0,
                    velocity: 0,
                    sustained: false,
                    sostenuto: false,
                });
                voice.presses = voice.presses.saturating_add(1);
                voice.velocity = vel.as_int();
                voice.sustained = false;
            }
            MidiMessage::NoteOff { key, .. } | MidiMessage::NoteOn { key, .. } => {
                self.release(key.as_int())
            }
            MidiMessage::ProgramChange { program } => self.program = program.as_int(),
            MidiMessage::PitchBend { bend } => self.bend = bend.as_int(),
            MidiMessage::ChannelAftertouch { vel } => self.pressure = vel.as_int(),
            MidiMessage::Aftertouch { .. } => {}
            MidiMessage::Controller { controller, value } => {
                let controller = controller.as_int() as usize;
                let value = value.as_int();
                let old = self.controls[controller];
                self.controls[controller] = value;
                match controller {
                    64 if value < 64 => {
                        for voice in self.voices.values_mut() {
                            voice.sustained = false;
                        }
                    }
                    66 if value >= 64 && old < 64 => {
                        for voice in self.voices.values_mut() {
                            voice.sostenuto = voice.presses > 0;
                        }
                    }
                    66 if value < 64 => {
                        for voice in self.voices.values_mut() {
                            voice.sostenuto = false;
                        }
                    }
                    120 => self.voices.clear(),
                    121 => {
                        // Reset controllers preserves program, volume, pan and bank.
                        self.bend = 0;
                        self.pressure = 0;
                        self.controls[1] = 0;
                        self.controls[11] = 127;
                        self.controls[64] = 0;
                        self.controls[66] = 0;
                        self.controls[100] = 127;
                        self.controls[101] = 127;
                        for voice in self.voices.values_mut() {
                            voice.sustained = false;
                            voice.sostenuto = false;
                        }
                    }
                    123..=127 => {
                        let keys = self.voices.keys().copied().collect::<Vec<_>>();
                        for key in keys {
                            if let Some(voice) = self.voices.get_mut(&key) {
                                voice.presses = 1;
                            }
                            self.release(key);
                        }
                    }
                    6 | 38 => match (self.controls[101], self.controls[100]) {
                        (0, 0) => {
                            self.bend_semitones = self.controls[6];
                            self.bend_cents = self.controls[38];
                        }
                        (0, 1) => {
                            self.fine_tune =
                                u16::from(self.controls[6]) * 128 + u16::from(self.controls[38])
                        }
                        (0, 2) => self.coarse_tune = self.controls[6],
                        _ => {}
                    },
                    98 | 99 => {
                        self.controls[100] = 127;
                        self.controls[101] = 127;
                    }
                    _ => {}
                }
                self.trim();
            }
        }
    }

    fn channel(&self, id: u8) -> Channel {
        let bend = f32::from(self.bend) / 8192.0
            * (f32::from(self.bend_semitones) + f32::from(self.bend_cents) / 100.0);
        let tuning =
            (f32::from(self.fine_tune) - 8192.0) / 8192.0 + f32::from(self.coarse_tune) - 64.0;
        let notes = self
            .voices
            .iter()
            .map(|(&key, voice)| Note {
                key: f32::from(key) + if id == 9 { 0.0 } else { bend + tuning },
                velocity: f32::from(voice.velocity) / 127.0,
                held: voice.presses > 0,
            })
            .collect::<Vec<_>>();
        let level = notes
            .iter()
            .map(|note| note.velocity)
            .fold(0.0_f32, f32::max)
            * f32::from(self.controls[7])
            / 127.0
            * f32::from(self.controls[11])
            / 127.0;
        Channel {
            id: u32::from(id),
            name: format!("MIDI {}", id + 1),
            kind: if id == 9 { "percussion" } else { "tonal" }.into(),
            active: !notes.is_empty(),
            notes,
            instrument: format!(
                "Bank {}:{} · Program {}",
                self.controls[0],
                self.controls[32],
                u16::from(self.program) + 1
            ),
            level,
            pan: (f32::from(self.controls[10]) - 64.0) / 64.0,
            fields: vec![
                Field::new("Volume", self.controls[7]),
                Field::new("Expression", self.controls[11]),
                Field::new(
                    "Sustain",
                    if self.controls[64] >= 64 { "On" } else { "Off" },
                ),
                Field::new(
                    "Sostenuto",
                    if self.controls[66] >= 64 { "On" } else { "Off" },
                ),
                Field::new("Bend", format!("{bend:+.2} st")),
                Field::new("Modulation", self.controls[1]),
                Field::new("Pressure", self.pressure),
            ],
        }
    }
}

#[derive(Clone, Default)]
struct State {
    channels: BTreeMap<u8, MidiChannel>,
    global: BTreeMap<String, String>,
}

#[derive(Clone)]
enum EventKind {
    Midi(u8, MidiMessage),
    Global(Field),
    Reset,
    Other,
}

struct Event {
    time: f64,
    kind: EventKind,
}

impl State {
    fn apply(&mut self, event: &Event) {
        match &event.kind {
            EventKind::Midi(id, message) => self.channels.entry(*id).or_default().apply(*message),
            EventKind::Global(field) => {
                self.global.insert(field.name.clone(), field.value.clone());
            }
            EventKind::Reset => {
                for channel in self.channels.values_mut() {
                    *channel = MidiChannel::default();
                }
            }
            EventKind::Other => {}
        }
    }
}

pub struct Timeline {
    events: Vec<Event>,
    rows: Vec<Row>,
    checkpoints: Vec<(usize, State)>,
    state: State,
    cursor: usize,
}

impl Timeline {
    pub fn recording_frames(&mut self, after: Option<f64>, through: f64) -> Vec<super::TimedFrame> {
        let rows = self
            .rows
            .iter()
            .filter(|row| {
                after.is_none_or(|after| row.time > after) && row.time <= through + 0.000_001
            })
            .cloned()
            .collect::<Vec<_>>();
        rows.into_iter()
            .map(|row| {
                let mut snapshot = Snapshot {
                    position: row.time,
                    ..Snapshot::default()
                };
                self.fill_snapshot(&mut snapshot);
                super::TimedFrame {
                    time: row.time,
                    data: super::FrameData {
                        channels: snapshot.channels,
                        global: snapshot.global,
                        row: Some(row),
                    },
                }
            })
            .collect()
    }

    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let smf = Smf::parse(bytes).map_err(|error| format!("MIDI channel inspection: {error}"))?;
        let mut ordered = Vec::new();
        for (track, events) in smf.tracks.iter().enumerate() {
            let mut tick = 0_u64;
            for (index, event) in events.iter().enumerate() {
                tick = tick.saturating_add(u64::from(event.delta.as_int()));
                ordered.push((tick, track, index, event.kind));
            }
        }
        ordered.sort_by_key(|&(tick, track, index, _)| (tick, track, index));
        let mut events = Vec::new();
        let mut rows = Vec::<Row>::new();
        let mut state = State::default();
        state.global.insert("Tempo".into(), "120.00 BPM".into());
        for (_, _, _, kind) in &ordered {
            if let TrackEventKind::Midi { channel, .. } = kind {
                state.channels.entry(channel.as_int()).or_default();
            }
        }
        let initial = state.clone();
        let mut checkpoints = vec![(0, state.clone())];
        let mut time = 0.0;
        let mut last_tick = 0;
        let mut tempo = 500_000_u32;
        for (tick, track, _, kind) in ordered {
            let seconds_per_tick = match smf.header.timing {
                Timing::Metrical(ticks) => {
                    f64::from(tempo) / 1_000_000.0 / f64::from(ticks.as_int().max(1))
                }
                Timing::Timecode(fps, ticks) => {
                    1.0 / (f64::from(fps.as_f32()) * f64::from(ticks.max(1)))
                }
            };
            time += (tick - last_tick) as f64 * seconds_per_tick;
            last_tick = tick;
            let mut cell = None;
            let mut global = Vec::new();
            let event_kind = match kind {
                TrackEventKind::Midi { channel, message } => {
                    let mut value = Cell {
                        channel: u32::from(channel.as_int()),
                        ..Cell::default()
                    };
                    match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            value.notes = note_name(f32::from(key.as_int()));
                            value.volume = format!("{:02X}", vel.as_int());
                        }
                        MidiMessage::NoteOff { key, vel } | MidiMessage::NoteOn { key, vel } => {
                            value.notes = format!("OFF {}", note_name(f32::from(key.as_int())));
                            value
                                .effects
                                .push(Field::new("Release velocity", vel.as_int()));
                        }
                        MidiMessage::ProgramChange { program } => {
                            value.instrument = format!("P{:03}", u16::from(program.as_int()) + 1)
                        }
                        MidiMessage::Controller {
                            controller,
                            value: v,
                        } => value.effects.push(Field::new(
                            format!(
                                "CC{} {}",
                                controller.as_int(),
                                controller_name(controller.as_int())
                            ),
                            v.as_int(),
                        )),
                        MidiMessage::PitchBend { bend } => {
                            value.effects.push(Field::new("Pitch bend", bend.as_int()))
                        }
                        MidiMessage::ChannelAftertouch { vel } => {
                            value.effects.push(Field::new("Pressure", vel.as_int()))
                        }
                        MidiMessage::Aftertouch { key, vel } => value.effects.push(Field::new(
                            format!("Pressure {}", note_name(f32::from(key.as_int()))),
                            vel.as_int(),
                        )),
                    }
                    cell = Some(value);
                    EventKind::Midi(channel.as_int(), message)
                }
                TrackEventKind::Meta(meta) => {
                    let field = match meta {
                        MetaMessage::Tempo(value) => {
                            tempo = value.as_int().max(1);
                            Field::new(
                                "Tempo",
                                format!("{:.2} BPM", 60_000_000.0 / f64::from(tempo)),
                            )
                        }
                        MetaMessage::TimeSignature(n, d, _, _) => Field::new(
                            "Meter",
                            format!("{n}/{}", 2_u64.checked_pow(u32::from(d)).unwrap_or(0)),
                        ),
                        MetaMessage::KeySignature(key, minor) => Field::new(
                            "Key signature",
                            format!("{key:+} {}", if minor { "minor" } else { "major" }),
                        ),
                        MetaMessage::TrackName(value) => Field::new(
                            format!("Track {}", track + 1),
                            String::from_utf8_lossy(value),
                        ),
                        MetaMessage::InstrumentName(value) => Field::new(
                            format!("Track {} instrument", track + 1),
                            String::from_utf8_lossy(value),
                        ),
                        MetaMessage::Text(value)
                        | MetaMessage::Marker(value)
                        | MetaMessage::CuePoint(value)
                        | MetaMessage::Lyric(value) => {
                            Field::new("Text", String::from_utf8_lossy(value))
                        }
                        MetaMessage::EndOfTrack => continue,
                        other => Field::new("Meta", format!("{other:?}")),
                    };
                    global.push(field.clone());
                    EventKind::Global(field)
                }
                TrackEventKind::SysEx(data) | TrackEventKind::Escape(data) => {
                    global.push(Field::new(
                        "SysEx",
                        data.iter()
                            .map(|b| format!("{b:02X}"))
                            .collect::<Vec<_>>()
                            .join(" "),
                    ));
                    if (data.len() >= 4
                        && data[0] == 0x7e
                        && data[2] == 0x09
                        && matches!(data[3], 1..=3))
                        || data.starts_with(&[0x43, 0x10, 0x4c, 0x00, 0x00, 0x7e, 0x00])
                    {
                        EventKind::Reset
                    } else {
                        EventKind::Other
                    }
                }
            };
            if rows
                .last()
                .is_none_or(|row| (row.time - time).abs() > 0.000_001)
            {
                rows.push(Row {
                    time,
                    label: format!("{time:08.3}"),
                    ..Row::default()
                });
            }
            let row = rows.last_mut().unwrap();
            if let Some(cell) = cell {
                row.cells.push(cell);
            }
            row.global.extend(global);
            let event = Event {
                time,
                kind: event_kind,
            };
            state.apply(&event);
            events.push(event);
            if events.len().is_multiple_of(1024) {
                checkpoints.push((events.len(), state.clone()));
            }
        }
        Ok(Self {
            events,
            rows,
            checkpoints,
            state: initial,
            cursor: 0,
        })
    }

    pub fn fill_snapshot(&mut self, snapshot: &mut Snapshot) {
        if snapshot.seeking {
            return;
        }
        let target = self
            .events
            .partition_point(|event| event.time <= snapshot.position + 0.000_001);
        if target < self.cursor || target.saturating_sub(self.cursor) > 2048 {
            let (cursor, state) = self
                .checkpoints
                .iter()
                .rev()
                .find(|(cursor, _)| *cursor <= target)
                .unwrap();
            self.cursor = *cursor;
            self.state = state.clone();
        }
        for event in &self.events[self.cursor..target] {
            self.state.apply(event);
        }
        self.cursor = target;
        snapshot.channels = self
            .state
            .channels
            .iter()
            .map(|(&id, channel)| channel.channel(id))
            .collect();
        snapshot.global = self
            .state
            .global
            .iter()
            .map(|(name, value)| Field::new(name, value))
            .collect();
        let cursor = self
            .rows
            .partition_point(|row| row.time <= snapshot.position + 0.000_001);
        let begin = cursor.saturating_sub(24);
        snapshot.rows = self.rows.iter().skip(begin).take(48).cloned().collect();
        snapshot.current_row = cursor.checked_sub(1).and_then(|i| i.checked_sub(begin));
    }
}

fn controller_name(controller: u8) -> &'static str {
    match controller {
        0 | 32 => "Bank",
        1 => "Modulation",
        6 | 38 => "Data entry",
        7 => "Volume",
        10 => "Pan",
        11 => "Expression",
        64 => "Sustain",
        65 => "Portamento",
        66 => "Sostenuto",
        67 => "Soft pedal",
        91 => "Reverb",
        93 => "Chorus",
        98 | 99 => "NRPN",
        100 | 101 => "RPN",
        120 => "All sound off",
        121 => "Reset controllers",
        123 => "All notes off",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn cc(channel: &mut MidiChannel, controller: u8, value: u8) {
        channel.apply(MidiMessage::Controller {
            controller: controller.into(),
            value: value.into(),
        });
    }
    fn on(channel: &mut MidiChannel, key: u8) {
        channel.apply(MidiMessage::NoteOn {
            key: key.into(),
            vel: 100.into(),
        });
    }

    #[test]
    fn overlapping_keys_sustain_sostenuto_and_all_sound_off() {
        let mut channel = MidiChannel::default();
        on(&mut channel, 60);
        on(&mut channel, 60);
        on(&mut channel, 64);
        channel.release(60);
        assert!(channel.channel(0).notes.iter().all(|note| note.held));
        cc(&mut channel, 66, 127);
        on(&mut channel, 67);
        for key in [60, 64, 67] {
            channel.release(key);
        }
        assert_eq!(channel.voices.len(), 2);
        assert!(channel.channel(0).notes.iter().all(|note| !note.held));
        cc(&mut channel, 66, 0);
        assert!(channel.voices.is_empty());
        on(&mut channel, 60);
        cc(&mut channel, 64, 127);
        channel.release(60);
        assert_eq!(channel.voices.len(), 1);
        cc(&mut channel, 120, 0);
        assert!(channel.voices.is_empty());
    }

    #[test]
    fn tempo_chords_effects_and_backwards_seek_use_same_clock() {
        use midly::{Format, Header, TrackEvent};
        let mut bytes = Vec::new();
        let smf = Smf {
            header: Header::new(Format::SingleTrack, Timing::Metrical(480.into())),
            tracks: vec![vec![
                TrackEvent {
                    delta: 0.into(),
                    kind: TrackEventKind::Midi {
                        channel: 0.into(),
                        message: MidiMessage::NoteOn {
                            key: 60.into(),
                            vel: 100.into(),
                        },
                    },
                },
                TrackEvent {
                    delta: 0.into(),
                    kind: TrackEventKind::Midi {
                        channel: 0.into(),
                        message: MidiMessage::NoteOn {
                            key: 64.into(),
                            vel: 100.into(),
                        },
                    },
                },
                TrackEvent {
                    delta: 480.into(),
                    kind: TrackEventKind::Meta(MetaMessage::Tempo(1_000_000.into())),
                },
                TrackEvent {
                    delta: 480.into(),
                    kind: TrackEventKind::Midi {
                        channel: 0.into(),
                        message: MidiMessage::Controller {
                            controller: 120.into(),
                            value: 0.into(),
                        },
                    },
                },
            ]],
        };
        smf.write_std(&mut bytes).unwrap();
        let mut timeline = Timeline::parse(&bytes).unwrap();
        let mut snapshot = Snapshot {
            position: 1.49,
            ..Snapshot::default()
        };
        timeline.fill_snapshot(&mut snapshot);
        assert_eq!(snapshot.channels[0].notes.len(), 2);
        assert_eq!(snapshot.rows[0].cells.len(), 2);
        assert_eq!(snapshot.rows[2].time, 1.5);
        snapshot.position = 1.5;
        timeline.fill_snapshot(&mut snapshot);
        assert!(snapshot.channels[0].notes.is_empty());
        snapshot.position = 0.0;
        timeline.fill_snapshot(&mut snapshot);
        assert_eq!(snapshot.channels[0].notes.len(), 2);
        assert!(
            snapshot
                .global
                .iter()
                .any(|field| field.value == "120.00 BPM")
        );
    }
}

/// Command state driven by the legacy sequencer's public event callback.
/// The owner stamps each batch with its audio render clock (at most 5 ms).
pub(crate) struct Live {
    state: State,
    cells: Vec<Cell>,
    events: Vec<Field>,
    serial: u64,
}
impl Default for Live {
    fn default() -> Self {
        let mut state=State::default();
        for id in 0..16 { state.channels.insert(id,MidiChannel::default()); }
        Self {state,cells:Vec::new(),events:Vec::new(),serial:0}
    }
}
impl Live {
    pub fn discard_pending(&mut self) { self.cells.clear(); self.events.clear(); }
    pub fn event(&mut self, kind:u8, subtype:u8, channel:u8, data:&[u8]) {
        if (8..=14).contains(&kind) && channel<16 {
            let mut bytes=vec![(kind<<4)|channel];bytes.extend_from_slice(data);
            if let Ok(midly::live::LiveEvent::Midi {channel,message})=midly::live::LiveEvent::parse(&bytes) {
                self.state.channels.entry(channel.as_int()).or_default().apply(message);
                let mut cell=Cell {channel:u32::from(channel.as_int()),..Cell::default()};
                match message {
                    MidiMessage::NoteOn {key,vel} if vel.as_int()>0 => {cell.notes=note_name(f32::from(key.as_int()));cell.volume=format!("{:02X}",vel.as_int());}
                    MidiMessage::NoteOn {key,..}|MidiMessage::NoteOff{key,..}=>cell.notes=format!("OFF {}",note_name(f32::from(key.as_int()))),
                    MidiMessage::ProgramChange{program}=>cell.instrument=format!("P{:03}",program.as_int() as u16+1),
                    MidiMessage::Controller{controller,value}=>cell.effects.push(Field::new(format!("CC{} {}",controller.as_int(),controller_name(controller.as_int())),value.as_int())),
                    MidiMessage::PitchBend{bend}=>cell.effects.push(Field::new("Pitch bend",bend.as_int())),
                    MidiMessage::ChannelAftertouch{vel}=>cell.effects.push(Field::new("Pressure",vel.as_int())),
                    MidiMessage::Aftertouch{key,vel}=>cell.effects.push(Field::new(format!("Pressure {}",note_name(f32::from(key.as_int()))),vel.as_int())),
                }
                self.cells.push(cell);
            }
        } else {
            let field=if kind==0xff && subtype==0x51 && data.len()==3 {
                let tempo=u32::from_be_bytes([0,data[0],data[1],data[2]]).max(1);
                Field::new("Tempo",format!("{:.2} BPM",60_000_000.0/f64::from(tempo)))
            } else {Field::new(format!("Event {kind:02X}:{subtype:02X}"),data.iter().take(256).map(|byte|format!("{byte:02X}")).collect::<Vec<_>>().join(" "))};
            self.state.global.insert(field.name.clone(),field.value.clone());self.events.push(field);
        }
    }
    pub fn frame(&mut self) -> super::FrameData {
        self.serial+=1;
        let row=if self.cells.is_empty() && self.events.is_empty() {None} else {Some(Row {
            label:format!("E{:06X}",self.serial),cells:std::mem::take(&mut self.cells),global:std::mem::take(&mut self.events),..Row::default()
        })};
        super::FrameData {channels:self.state.channels.iter().map(|(id,c)|c.channel(*id)).collect(),row,
            global:self.state.global.iter().map(|(k,v)|Field::new(k,v)).collect()}
    }
}
