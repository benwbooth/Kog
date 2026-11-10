//! Automatic volume leveling: every song plays at about the same loudness.
//!
//! Most of Kog's formats carry no ReplayGain tags (an NSF or a VGM has
//! nothing to read), and measuring a whole emulated song before it starts
//! would delay every track, so the leveler listens while the song plays. It
//! measures loudness the way EBU R128 does (K-weighted, 400 ms blocks,
//! gated), keeps a running estimate of the song's level, and eases the gain
//! toward one target.
//!
//! It levels songs, not passages: blocks far below the song's level (a quiet
//! bridge, a fade) are left out of the estimate, and once the first seconds
//! have been heard the gain only drifts slowly, so a song keeps its own
//! dynamics. A peak limiter keeps boosted songs from clipping.
//!
//! One process-wide choice, saved with the preferences like the looping
//! setting. The desktop engine levels its output bus; the streaming server
//! levels the PCM it encodes, which also covers the web player.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, SampleRate, Source};

const SETTING_FILE: &str = "volume-leveling";

/// The loudness every song is brought to, in LUFS. A little below what
/// streaming services use, which leaves boosted chiptunes room under the
/// limiter.
pub const TARGET_LUFS: f32 = -16.0;
/// The most a quiet song is raised, and the most a loud one is lowered.
const MAX_BOOST_DB: f32 = 12.0;
const MAX_CUT_DB: f32 = -18.0;

/// The detector updates ten times a second over 400 ms blocks.
const HOP_SECONDS: f32 = 0.1;
const BLOCK_HOPS: usize = 4;
/// Blocks quieter than this are silence and never count.
const ABSOLUTE_GATE_LUFS: f32 = -55.0;
/// Blocks this far below the song's level are a quiet passage, not the song.
const RELATIVE_GATE_LU: f32 = 18.0;
/// Seconds of counted audio after which the song's level is known well
/// enough to settle: before it the estimate and gain move quickly, after it
/// slowly.
const LEARNING_SECONDS: f32 = 4.0;
const FAST_TIME_CONSTANT: f32 = 0.8;
const SLOW_TIME_CONSTANT: f32 = 12.0;
/// How fast the gain may move, in dB per second.
const FAST_SLEW_DB: f32 = 18.0;
const SLOW_SLEW_DB: f32 = 1.5;
/// The limiter keeps peaks under -0.5 dBFS and lets go over 150 ms.
const CEILING: f32 = 0.944;
const LIMITER_RELEASE_SECONDS: f32 = 0.15;
/// How much audio a pull stream measures before its first samples go out,
/// so a song starts at its own level instead of swelling into it.
pub const PRIME_DURATION: Duration = Duration::from_millis(2_500);

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Whether songs opened in this process are leveled.
pub fn current() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Level (or stop leveling) from now on in this process.
pub fn set(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

/// The saved choice; off unless turned on.
pub fn saved() -> bool {
    crate::settings::load_text(SETTING_FILE)
        .is_some_and(|value| matches!(value.trim(), "on" | "true" | "1"))
}

/// Apply the saved choice to this process.
pub fn load() -> bool {
    let enabled = saved();
    set(enabled);
    enabled
}

/// Save the choice and apply it to this process.
pub fn save(enabled: bool) -> Result<(), String> {
    set(enabled);
    crate::settings::save_text(SETTING_FILE, if enabled { "on" } else { "off" })
}

fn loudness(mean_square: f64) -> f32 {
    (-0.691 + 10.0 * mean_square.max(1e-12).log10()) as f32
}

fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

#[derive(Clone, Copy, Debug, Default)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct BiquadState {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl BiquadState {
    fn process(&mut self, filter: Biquad, input: f64) -> f64 {
        let output = filter.b0 * input + filter.b1 * self.x1 + filter.b2 * self.x2
            - filter.a1 * self.y1
            - filter.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = if output.is_finite() { output } else { 0.0 };
        self.y1
    }
}

/// BS.1770's K-weighting at any sample rate: a high shelf for the head's
/// effect, then a high-pass that ignores rumble.
fn k_weighting(sample_rate: f64) -> (Biquad, Biquad) {
    let (frequency, gain_db, q) = (1_681.974_450_955_533, 3.999_843_853_973_347, 0.707_175_236_955_419_6);
    let k = (std::f64::consts::PI * frequency / sample_rate).tan();
    let vh = 10.0_f64.powf(gain_db / 20.0);
    let vb = vh.powf(0.499_666_774_154_541_6);
    let a0 = 1.0 + k / q + k * k;
    let shelf = Biquad {
        b0: (vh + vb * k / q + k * k) / a0,
        b1: 2.0 * (k * k - vh) / a0,
        b2: (vh - vb * k / q + k * k) / a0,
        a1: 2.0 * (k * k - 1.0) / a0,
        a2: (1.0 - k / q + k * k) / a0,
    };
    let (frequency, q) = (38.135_470_876_024_44, 0.500_327_037_323_877_3);
    let k = (std::f64::consts::PI * frequency / sample_rate).tan();
    let a0 = 1.0 + k / q + k * k;
    let high_pass = Biquad {
        b0: 1.0,
        b1: -2.0,
        b2: 1.0,
        a1: 2.0 * (k * k - 1.0) / a0,
        a2: (1.0 - k / q + k * k) / a0,
    };
    (shelf, high_pass)
}

/// The leveler for one stream of interleaved frames.
#[derive(Clone, Debug)]
pub struct Leveler {
    channels: usize,
    shelf: Biquad,
    high_pass: Biquad,
    filters: Vec<[BiquadState; 2]>,
    hop_frames: usize,
    hop_frame: usize,
    hop_energy: f64,
    hops: [f64; BLOCK_HOPS],
    hop_count: usize,
    hop_index: usize,
    /// The song's mean-square level, once anything has been counted.
    estimate: Option<f64>,
    /// Seconds of audio counted into the estimate.
    learned: f32,
    gain_db: f32,
    gain: f32,
    gain_end: f32,
    gain_step: f32,
    limiter: f32,
    limiter_release: f32,
}

impl Leveler {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        let sample_rate = sample_rate.max(1);
        let channels = usize::from(channels.max(1));
        let (shelf, high_pass) = k_weighting(f64::from(sample_rate));
        let hop_frames = ((sample_rate as f32 * HOP_SECONDS) as usize).max(1);
        Self {
            channels,
            shelf,
            high_pass,
            filters: vec![[BiquadState::default(); 2]; channels],
            hop_frames,
            hop_frame: 0,
            hop_energy: 0.0,
            hops: [0.0; BLOCK_HOPS],
            hop_count: 0,
            hop_index: 0,
            estimate: None,
            learned: 0.0,
            gain_db: 0.0,
            gain: 1.0,
            gain_end: 1.0,
            gain_step: 0.0,
            limiter: 1.0,
            limiter_release: 1.0 - (-1.0 / (LIMITER_RELEASE_SECONDS * sample_rate as f32)).exp(),
        }
    }

    /// Start a song already measured once: its level is known, so it plays
    /// at the right gain from the first sample and only drifts slowly.
    pub fn with_known_level(mut self, mean_square: f64) -> Self {
        if mean_square.is_finite() && mean_square > 0.0 {
            self.estimate = Some(mean_square);
            self.learned = LEARNING_SECONDS;
            self.jump_to_desired_gain();
        }
        self
    }

    /// Start a new song from the gain the last one ended at. Songs played
    /// in a row (one game's soundtrack) tend to sit at the same level, so
    /// this is a better first guess than unity.
    pub fn with_starting_gain(mut self, gain_db: f32) -> Self {
        if gain_db.is_finite() {
            self.gain_db = gain_db.clamp(MAX_CUT_DB, MAX_BOOST_DB);
            self.gain = db_to_gain(self.gain_db);
            self.gain_end = self.gain;
        }
        self
    }

    /// The song's level, once it has been heard long enough to trust.
    pub fn settled_level(&self) -> Option<f64> {
        self.estimate.filter(|_| self.learned >= LEARNING_SECONDS)
    }

    pub fn gain_db(&self) -> f32 {
        self.gain_db
    }

    pub fn has_estimate(&self) -> bool {
        self.estimate.is_some()
    }

    /// Measure audio ahead of playing it, then start at the gain it calls
    /// for. The same samples are processed afterwards as usual.
    pub fn prime(&mut self, samples: &[f32]) {
        for frame in samples.chunks_exact(self.channels) {
            self.detect(frame);
        }
        if self.estimate.is_some() {
            self.jump_to_desired_gain();
        }
    }

    /// Level one interleaved frame in place, then scale it by `volume`.
    pub fn process_frame(&mut self, frame: &mut [f32], volume: f32) {
        self.detect(frame);
        if self.gain_step != 0.0 {
            self.gain += self.gain_step;
            if (self.gain_step > 0.0 && self.gain >= self.gain_end)
                || (self.gain_step < 0.0 && self.gain <= self.gain_end)
            {
                self.gain = self.gain_end;
                self.gain_step = 0.0;
            }
        }
        let gain = self.gain;
        let peak = frame
            .iter()
            .map(|sample| if sample.is_finite() { (sample * gain).abs() } else { 0.0 })
            .fold(0.0_f32, f32::max);
        if peak * self.limiter > CEILING {
            self.limiter = CEILING / peak;
        } else {
            self.limiter += (1.0 - self.limiter) * self.limiter_release;
        }
        let scale = gain * self.limiter * volume;
        for sample in frame {
            *sample = if sample.is_finite() { *sample * scale } else { 0.0 };
        }
    }

    /// Level interleaved samples in place; a trailing partial frame is only
    /// scaled.
    pub fn process(&mut self, samples: &mut [f32], volume: f32) {
        let channels = self.channels;
        let whole = samples.len() / channels * channels;
        let (frames, rest) = samples.split_at_mut(whole);
        for frame in frames.chunks_exact_mut(channels) {
            self.process_frame(frame, volume);
        }
        let scale = self.gain * self.limiter * volume;
        for sample in rest {
            *sample = (*sample * scale).clamp(-CEILING, CEILING);
        }
    }

    fn detect(&mut self, frame: &[f32]) {
        let mut energy = 0.0;
        for (channel, sample) in frame.iter().take(self.channels).enumerate() {
            let input = if sample.is_finite() { f64::from(*sample) } else { 0.0 };
            let [shelf, high_pass] = &mut self.filters[channel];
            let weighted = high_pass.process(self.high_pass, shelf.process(self.shelf, input));
            energy += weighted * weighted;
        }
        self.hop_energy += energy;
        self.hop_frame += 1;
        if self.hop_frame >= self.hop_frames {
            self.end_hop();
        }
    }

    fn end_hop(&mut self) {
        self.hops[self.hop_index] = self.hop_energy / self.hop_frames as f64;
        self.hop_index = (self.hop_index + 1) % BLOCK_HOPS;
        self.hop_count = (self.hop_count + 1).min(BLOCK_HOPS);
        self.hop_energy = 0.0;
        self.hop_frame = 0;

        let block = self.hops.iter().take(self.hop_count).sum::<f64>() / self.hop_count as f64;
        let block_loudness = loudness(block);
        let counted = block_loudness > ABSOLUTE_GATE_LUFS
            && self
                .estimate
                .is_none_or(|estimate| block_loudness > loudness(estimate) - RELATIVE_GATE_LU);
        if counted {
            let time_constant =
                if self.learned < LEARNING_SECONDS { FAST_TIME_CONSTANT } else { SLOW_TIME_CONSTANT };
            let alpha = 1.0 - (-f64::from(HOP_SECONDS / time_constant)).exp();
            self.estimate = Some(match self.estimate {
                None => block,
                Some(estimate) => estimate + alpha * (block - estimate),
            });
            self.learned += HOP_SECONDS;
        }

        let Some(desired) = self.desired_gain_db() else { return };
        let slew = if self.learned < LEARNING_SECONDS { FAST_SLEW_DB } else { SLOW_SLEW_DB } * HOP_SECONDS;
        self.gain_db += (desired - self.gain_db).clamp(-slew, slew);
        self.gain_end = db_to_gain(self.gain_db);
        self.gain_step = (self.gain_end - self.gain) / self.hop_frames as f32;
    }

    fn desired_gain_db(&self) -> Option<f32> {
        self.estimate
            .map(|estimate| (TARGET_LUFS - loudness(estimate)).clamp(MAX_CUT_DB, MAX_BOOST_DB))
    }

    fn jump_to_desired_gain(&mut self) {
        if let Some(desired) = self.desired_gain_db() {
            self.gain_db = desired;
            self.gain = db_to_gain(desired);
            self.gain_end = self.gain;
            self.gain_step = 0.0;
        }
    }
}

/// Shared between the playback engine and the leveler on its output bus.
#[derive(Clone)]
pub struct LevelingControl {
    shared: Arc<LevelingShared>,
}

struct LevelingShared {
    enabled: AtomicBool,
    volume: AtomicU32,
    /// Bumped when a new song starts.
    revision: AtomicU64,
    /// The new song's known level (f64 bits), or 0 when it is new to Kog.
    known_level: AtomicU64,
    /// The playing song's settled level (f64 bits), or 0 while unsettled.
    settled_level: AtomicU64,
}

impl LevelingControl {
    pub fn new(enabled: bool, volume: f32) -> Self {
        Self {
            shared: Arc::new(LevelingShared {
                enabled: AtomicBool::new(enabled),
                volume: AtomicU32::new(volume.to_bits()),
                revision: AtomicU64::new(0),
                known_level: AtomicU64::new(0),
                settled_level: AtomicU64::new(0),
            }),
        }
    }

    pub fn enabled(&self) -> bool {
        self.shared.enabled.load(Ordering::Acquire)
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.shared.enabled.store(enabled, Ordering::Release);
    }

    /// The listener's volume, applied after leveling while it is on (the
    /// leveler has to hear the song before the volume knob does).
    pub fn set_volume(&self, volume: f32) {
        self.shared.volume.store(volume.to_bits(), Ordering::Relaxed);
    }

    /// A new song starts; `known_level` is what it measured last time.
    pub fn start_song(&self, known_level: Option<f64>) {
        self.shared.settled_level.store(0, Ordering::Relaxed);
        self.shared
            .known_level
            .store(known_level.unwrap_or(0.0).to_bits(), Ordering::Relaxed);
        self.shared.revision.fetch_add(1, Ordering::Release);
    }

    /// The playing song's level, once it has settled.
    pub fn settled_level(&self) -> Option<f64> {
        let level = f64::from_bits(self.shared.settled_level.load(Ordering::Relaxed));
        (level > 0.0).then_some(level)
    }

    fn volume(&self) -> f32 {
        f32::from_bits(self.shared.volume.load(Ordering::Relaxed))
    }
}

/// Levels a playback bus: passes audio through untouched while leveling is
/// off.
pub struct LevelingSource<S> {
    input: S,
    control: LevelingControl,
    leveler: Option<Leveler>,
    revision: u64,
    channels: ChannelCount,
    sample_rate: SampleRate,
    frame: Vec<f32>,
    cursor: usize,
}

impl<S: Source<Item = f32>> LevelingSource<S> {
    pub fn new(input: S, control: LevelingControl) -> Self {
        let channels = input.channels();
        let sample_rate = input.sample_rate();
        let revision = control.shared.revision.load(Ordering::Acquire);
        Self {
            input,
            control,
            leveler: None,
            revision,
            channels,
            sample_rate,
            frame: Vec::with_capacity(usize::from(channels.get())),
            cursor: 0,
        }
    }

    fn fresh_leveler(&self) -> Leveler {
        Leveler::new(self.sample_rate.get(), self.channels.get())
    }

    fn refill(&mut self) -> bool {
        self.frame.clear();
        self.cursor = 0;
        for _ in 0..self.channels.get() {
            match self.input.next() {
                Some(sample) => self.frame.push(sample),
                None => break,
            }
        }
        if self.frame.is_empty() {
            return false;
        }
        if !self.control.enabled() {
            self.leveler = None;
            return true;
        }
        let revision = self.control.shared.revision.load(Ordering::Acquire);
        if revision != self.revision {
            self.revision = revision;
            let known = f64::from_bits(self.control.shared.known_level.load(Ordering::Relaxed));
            let carried = self.leveler.as_ref().map(Leveler::gain_db);
            let fresh = self.fresh_leveler();
            self.leveler = Some(if known > 0.0 {
                fresh.with_known_level(known)
            } else if let Some(gain) = carried {
                fresh.with_starting_gain(gain)
            } else {
                fresh
            });
        }
        let volume = self.control.volume();
        let fresh = self.leveler.is_none().then(|| self.fresh_leveler());
        let leveler = self.leveler.get_or_insert_with(|| fresh.expect("checked"));
        if self.frame.len() == usize::from(self.channels.get()) {
            leveler.process_frame(&mut self.frame, volume);
        } else {
            leveler.process(&mut self.frame, volume);
        }
        let settled = leveler.settled_level().unwrap_or(0.0);
        self.control.shared.settled_level.store(settled.to_bits(), Ordering::Relaxed);
        true
    }
}

impl<S: Source<Item = f32>> Iterator for LevelingSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.cursor >= self.frame.len() && !self.refill() {
            return None;
        }
        let sample = self.frame[self.cursor];
        self.cursor += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

impl<S: Source<Item = f32>> Source for LevelingSource<S> {
    fn current_span_len(&self) -> Option<usize> {
        self.input.current_span_len()
    }

    fn channels(&self) -> ChannelCount {
        self.channels
    }

    fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        self.input.total_duration()
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.input.try_seek(position)?;
        self.frame.clear();
        self.cursor = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    fn tone(amplitude: f32, seconds: f32) -> Vec<f32> {
        let frames = (RATE as f32 * seconds) as usize;
        (0..frames)
            .flat_map(|index| {
                let value = amplitude * (index as f32 * 1_000.0 * std::f32::consts::TAU / RATE as f32).sin();
                [value, value]
            })
            .collect()
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
    }

    fn leveled(amplitude: f32) -> Vec<f32> {
        let mut samples = tone(amplitude, 12.0);
        let mut leveler = Leveler::new(RATE, 2);
        leveler.process(&mut samples, 1.0);
        samples
    }

    #[test]
    fn quiet_and_loud_songs_end_up_at_the_same_level() {
        let quiet = leveled(0.05);
        let loud = leveled(0.5);
        let tail = |samples: &[f32]| rms(&samples[samples.len() - RATE as usize * 2..]);
        let difference_db = 20.0 * (tail(&quiet) / tail(&loud)).log10();
        assert!(difference_db.abs() < 1.0, "levels differ by {difference_db} dB");
    }

    #[test]
    fn peaks_never_pass_the_ceiling() {
        // A quiet, very peaky song: the boost would clip without the limiter.
        let mut samples = tone(0.02, 6.0);
        for frame in samples.chunks_exact_mut(2).step_by(4_800) {
            frame.fill(0.9);
        }
        let mut leveler = Leveler::new(RATE, 2);
        leveler.process(&mut samples, 1.0);
        assert!(samples.iter().all(|sample| sample.abs() <= CEILING + 1e-4));
    }

    #[test]
    fn a_quiet_passage_inside_a_song_stays_quiet() {
        let mut samples = tone(0.3, 8.0);
        samples.extend(tone(0.003, 4.0));
        let mut leveler = Leveler::new(RATE, 2);
        leveler.process(&mut samples, 1.0);
        let loud = rms(&samples[RATE as usize * 2 * 6..RATE as usize * 2 * 8]);
        let quiet = rms(&samples[samples.len() - RATE as usize * 2..]);
        assert!(20.0 * (loud / quiet).log10() > 30.0, "the quiet passage was pulled up");
    }

    #[test]
    fn priming_starts_at_the_songs_own_level() {
        let mut samples = tone(0.02, 3.0);
        let mut leveler = Leveler::new(RATE, 2);
        leveler.prime(&samples);
        let gain = leveler.gain_db();
        assert!(gain > 6.0, "a quiet song should start boosted, got {gain} dB");
        leveler.process(&mut samples, 1.0);
        assert!((leveler.gain_db() - gain).abs() < 1.5);
    }

    #[test]
    fn silence_is_left_alone() {
        let mut samples = vec![0.0; RATE as usize * 2 * 3];
        let mut leveler = Leveler::new(RATE, 2);
        leveler.process(&mut samples, 1.0);
        assert_eq!(leveler.gain_db(), 0.0);
        assert!(!leveler.has_estimate());
    }

    #[test]
    fn the_bus_source_passes_audio_through_while_off_and_levels_while_on() {
        use std::num::NonZero;
        let buffer = |samples: Vec<f32>| {
            rodio::buffer::SamplesBuffer::new(NonZero::new(2).unwrap(), NonZero::new(RATE).unwrap(), samples)
        };
        let input = tone(0.05, 8.0);
        let off: Vec<f32> = LevelingSource::new(buffer(input.clone()), LevelingControl::new(false, 0.5)).collect();
        assert_eq!(off, input);
        let control = LevelingControl::new(true, 1.0);
        control.start_song(None);
        let on: Vec<f32> = LevelingSource::new(buffer(input.clone()), control.clone()).collect();
        assert_eq!(on.len(), input.len());
        let tail = |samples: &[f32]| rms(&samples[samples.len() - RATE as usize * 2..]);
        assert!(tail(&on) > tail(&input) * 2.0, "a quiet song should be raised");
        assert!(control.settled_level().is_some());
    }

    #[test]
    fn a_known_level_plays_at_its_gain_from_the_start() {
        let mut first = Leveler::new(RATE, 2);
        let mut samples = tone(0.05, 10.0);
        first.process(&mut samples, 1.0);
        let level = first.settled_level().expect("ten seconds settle");
        let second = Leveler::new(RATE, 2).with_known_level(level);
        assert!((second.gain_db() - first.gain_db()).abs() < 1.0);
    }
}
