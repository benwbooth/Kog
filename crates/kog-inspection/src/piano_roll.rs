//! The piano roll: each channel's notes as bars at their pitch, scrolling
//! left as the song plays. Built up from the inspector's snapshots, so every
//! decoder that fills in channel notes draws one, live or from a recording.
use serde::Serialize;

use crate::Channel;
use crate::mml::Score;

/// Seconds of history kept; views show as much of it as fits.
const HISTORY: f64 = 30.0;
/// A pitch moving further than this starts a new bar, so slides and vibrato
/// draw as steps instead of one bar at the starting pitch.
const BAR_STEP: f32 = 0.5;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Bar {
    /// Index into `PianoRoll::channels`, for the channel's color.
    pub lane: usize,
    pub key: f32,
    pub start: f64,
    pub end: f64,
    pub velocity: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Lane {
    pub id: u32,
    pub name: String,
    /// The lane's color, as hue degrees.
    pub hue: f32,
    /// The lowest and highest keys the channel has played this song. Ranges
    /// only grow, so the roll settles on a song's range instead of zooming
    /// with every phrase.
    pub low: f32,
    pub high: f32,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PianoRoll {
    pub position: f64,
    pub lanes: Vec<Lane>,
    pub bars: Vec<Bar>,
    /// Bars still sounding at `position`, as indices into `bars`.
    #[serde(skip)]
    open: Vec<usize>,
    /// Every channel and its range over the whole song, from a recording made
    /// ahead of playback: the roll starts from these, so it never resizes.
    #[serde(skip)]
    plan: Vec<Lane>,
}

impl PianoRoll {
    /// Add the notes sounding at `position`. A jump backwards or far ahead
    /// (a seek or a new song) starts the roll over.
    pub fn update(&mut self, position: f64, channels: &[Channel]) {
        if !position.is_finite() {
            return;
        }
        if position < self.position - 0.05 || position > self.position + 2.0 {
            self.clear();
        }
        let mut still_open = Vec::new();
        for channel in channels.iter().filter(|channel| channel.active) {
            let lane = match self.lanes.iter().position(|lane| lane.id == channel.id) {
                Some(lane) => {
                    // A planned lane takes the channel's own name.
                    if self.lanes[lane].name != channel.name && !channel.name.is_empty() {
                        self.lanes[lane].name = channel.name.clone();
                    }
                    lane
                }
                None => {
                    let hue = lane_hue(self.lanes.len());
                    self.lanes.push(Lane { id: channel.id, name: channel.name.clone(), hue, low: f32::MAX, high: f32::MIN });
                    self.lanes.len() - 1
                }
            };
            for note in channel.notes.iter().filter(|note| note.velocity > 0.0) {
                let seen = &mut self.lanes[lane];
                (seen.low, seen.high) = (seen.low.min(note.key), seen.high.max(note.key));
                let sounding = self.open.iter().copied().find(|index| {
                    let bar = &self.bars[*index];
                    bar.lane == lane && (bar.key - note.key).abs() < BAR_STEP && !still_open.contains(index)
                });
                match sounding {
                    Some(index) => {
                        self.bars[index].end = position;
                        still_open.push(index);
                    }
                    None => {
                        self.bars.push(Bar {
                            lane,
                            key: note.key,
                            start: position,
                            end: position,
                            velocity: note.velocity,
                        });
                        still_open.push(self.bars.len() - 1);
                    }
                }
            }
        }
        // A bar that stopped sounding ends where this update sees it gone.
        for index in &self.open {
            if !still_open.contains(index) {
                self.bars[*index].end = self.bars[*index].end.max(position);
            }
        }
        self.open = still_open;
        self.position = position;
        let expired = self.bars.iter().take_while(|bar| bar.end < position - HISTORY).count();
        if expired > 0 {
            self.bars.drain(..expired);
            self.open = self.open.iter().map(|index| index - expired).collect();
        }
    }

    /// Take every channel's range from a recording of the song (it may still
    /// be growing), so the bands and their ranges are set before notes play.
    pub fn plan(&mut self, score: &Score) {
        let mut plan: Vec<Lane> = Vec::new();
        for note in score.piano_roll() {
            let Some(cents) = note.cents else { continue };
            let key = cents as f32 / 100.0;
            let index = match plan.iter().position(|lane| lane.id == note.channel) {
                Some(index) => index,
                None => {
                    let name = score
                        .tracks
                        .iter()
                        .find(|track| track.channel == note.channel)
                        .map(|track| track.name.clone())
                        .unwrap_or_default();
                    plan.push(Lane { id: note.channel, name, hue: 0.0, low: f32::MAX, high: f32::MIN });
                    plan.len() - 1
                }
            };
            let lane = &mut plan[index];
            (lane.low, lane.high) = (lane.low.min(key), lane.high.max(key));
        }
        plan.sort_by_key(|lane| lane.id);
        for (index, lane) in plan.iter_mut().enumerate() {
            lane.hue = lane_hue(index);
        }
        self.plan = plan;
        self.adopt_plan();
    }

    /// Lanes follow the plan's order and colors, keeping what they've seen.
    fn adopt_plan(&mut self) {
        if self.plan.is_empty() {
            return;
        }
        let old = std::mem::take(&mut self.lanes);
        let mut lanes = self.plan.clone();
        for lane in &old {
            match lanes.iter_mut().find(|planned| planned.id == lane.id) {
                Some(planned) => (planned.low, planned.high) = (planned.low.min(lane.low), planned.high.max(lane.high)),
                None => lanes.push(Lane { hue: lane_hue(lanes.len()), ..lane.clone() }),
            }
        }
        // Bars point at lanes by index, which the plan may have reordered.
        let moved: Vec<usize> = old
            .iter()
            .map(|lane| lanes.iter().position(|planned| planned.id == lane.id).unwrap_or(0))
            .collect();
        for bar in &mut self.bars {
            bar.lane = moved[bar.lane];
        }
        self.lanes = lanes;
    }

    pub fn clear(&mut self) {
        self.lanes.clear();
        self.bars.clear();
        self.open.clear();
        self.adopt_plan();
    }

    /// A new song: forget its predecessor's plan too.
    pub fn reset(&mut self) {
        self.plan.clear();
        self.clear();
    }

    /// The keys every channel has played this song, padded to at least two
    /// octaves around their middle so a quiet song doesn't fill the height.
    pub fn key_range(&self) -> (f32, f32) {
        fit_keys(self.lanes.iter(), 12.0)
    }

    /// One lane's key range, for a roll per channel: at least an octave.
    pub fn lane_key_range(&self, lane: usize) -> (f32, f32) {
        fit_keys(self.lanes.get(lane).into_iter(), 6.0)
    }

    /// Where to draw "now" `elapsed` seconds after the last update while
    /// playing, so a view redrawn every frame scrolls smoothly between
    /// updates. Capped so a stalled decoder doesn't run the roll away.
    pub fn now(&self, elapsed: f64, playing: bool) -> f64 {
        if playing { self.position + elapsed.clamp(0.0, 0.25) } else { self.position }
    }
}

fn fit_keys<'a>(lanes: impl Iterator<Item = &'a Lane>, half_span: f32) -> (f32, f32) {
    let (low, high) = lanes.fold((f32::MAX, f32::MIN), |(low, high), lane| (low.min(lane.low), high.max(lane.high)));
    if low > high {
        return (60.0 - half_span, 60.0 + half_span);
    }
    let (low, high) = (low.floor() - 1.0, high.ceil() + 1.0);
    let middle = (low + high) / 2.0;
    (low.min(middle - half_span), high.max(middle + half_span))
}

/// A distinct, readable color for each lane, as hue degrees: the golden angle
/// apart, so neighbors never share a color.
pub fn lane_hue(lane: usize) -> f32 {
    (200.0 + lane as f32 * 137.508) % 360.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Note;

    fn channel(id: u32, keys: &[f32]) -> Channel {
        Channel {
            id,
            name: format!("Pulse {id}"),
            active: true,
            notes: keys.iter().map(|key| Note { key: *key, velocity: 1.0, held: true }).collect(),
            ..Channel::default()
        }
    }

    #[test]
    fn notes_become_bars_that_end_when_they_stop() {
        let mut roll = PianoRoll::default();
        roll.update(0.0, &[channel(1, &[60.0]), channel(2, &[64.0])]);
        roll.update(0.5, &[channel(1, &[60.0]), channel(2, &[])]);
        roll.update(1.0, &[channel(1, &[62.0])]);
        assert_eq!(roll.lanes.len(), 2);
        assert_eq!(roll.bars.len(), 3);
        assert_eq!((roll.bars[0].start, roll.bars[0].end, roll.bars[0].lane), (0.0, 1.0, 0));
        assert_eq!((roll.bars[1].start, roll.bars[1].end, roll.bars[1].lane), (0.0, 0.5, 1));
        assert_eq!((roll.bars[2].key, roll.bars[2].start), (62.0, 1.0));
        // A small bend stays one bar.
        roll.update(1.2, &[channel(1, &[62.2])]);
        assert_eq!(roll.bars.len(), 3);
        assert_eq!(roll.bars[2].end, 1.2);
    }

    #[test]
    fn a_recording_sets_every_lane_and_range_before_the_notes_play() {
        use crate::mml::{Event, EventKind, Score, Track};
        let note = |tick, key| Event { tick, kind: EventKind::Note { key, cents: 0, velocity: 100, legato: false, length: 96 } };
        let score = Score {
            tracks: vec![
                Track { channel: 2, name: "Triangle".into(), events: vec![note(0, 40), note(96, 52)], ..Track::default() },
                Track { channel: 0, name: "Pulse 1".into(), events: vec![note(0, 72), note(500, 84)], ..Track::default() },
            ],
            ..Score::default()
        };
        let mut roll = PianoRoll::default();
        roll.update(0.0, &[channel(2, &[45.0])]);
        roll.plan(&score);
        assert_eq!(roll.lanes.iter().map(|lane| lane.id).collect::<Vec<_>>(), vec![0, 2]);
        assert_eq!((roll.lanes[0].low, roll.lanes[0].high), (72.0, 84.0));
        assert_eq!((roll.lanes[1].low, roll.lanes[1].high), (40.0, 52.0));
        assert_eq!(roll.bars[0].lane, 1, "bars follow their lane to its planned place");
        // A seek keeps the plan.
        roll.update(30.0, &[]);
        assert_eq!(roll.lanes.len(), 2);
        roll.reset();
        assert!(roll.lanes.is_empty());
    }

    #[test]
    fn a_seek_starts_over_and_old_bars_expire() {
        let mut roll = PianoRoll::default();
        roll.update(10.0, &[channel(1, &[60.0])]);
        roll.update(5.0, &[channel(1, &[67.0])]);
        assert_eq!(roll.bars.len(), 1);
        assert_eq!(roll.bars[0].key, 67.0);
        for step in 1..=40 {
            roll.update(5.0 + f64::from(step), &[channel(1, &[if step < 35 { 67.0 } else { 70.0 }])]);
        }
        assert_eq!(roll.bars.len(), 2, "the long bar is still in view");
        // Ranges keep every key played since the seek, even out of view.
        assert_eq!(roll.key_range(), (56.5, 80.5));
        assert_eq!(roll.lane_key_range(0), (62.5, 74.5));
        assert_eq!(roll.lane_key_range(5), (54.0, 66.0));
        assert_eq!(roll.now(0.1, true), roll.position + 0.1);
        assert_eq!(roll.now(3.0, true), roll.position + 0.25);
    }
}
