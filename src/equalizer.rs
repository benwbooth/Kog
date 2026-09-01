//! Cog-compatible 31-band graphic equalizer and preset library.

use std::f64::consts::PI;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, SampleRate, Source};

pub const EQUALIZER_FREQUENCIES: [f32; 31] = [
    20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0,
    500.0, 630.0, 800.0, 1_000.0, 1_200.0, 1_600.0, 2_000.0, 2_500.0, 3_100.0, 4_000.0, 5_000.0,
    6_300.0, 8_000.0, 10_000.0, 12_000.0, 16_000.0, 20_000.0,
];
const COG_PRESET_FREQUENCIES: [f32; 10] = [
    32.0, 64.0, 128.0, 256.0, 512.0, 1_000.0, 2_000.0, 4_000.0, 8_000.0, 16_000.0,
];
const COG_PRESET_FIELDS: [&str; 10] = [
    "hz32", "hz64", "hz128", "hz256", "hz512", "hz1000", "hz2000", "hz4000", "hz8000", "hz16000",
];
const EQUALIZER_Q: f64 = 1.4;
const MIN_GAIN_DB: f32 = -20.0;
const MAX_GAIN_DB: f32 = 20.0;

#[derive(Clone, Debug, PartialEq)]
pub struct EqualizerSettings {
    pub enabled: bool,
    pub track_genre: bool,
    pub preset_name: String,
    pub preamp_db: f32,
    pub gains_db: [f32; 31],
}

impl Default for EqualizerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            track_genre: false,
            preset_name: "Flat".to_owned(),
            preamp_db: 0.0,
            gains_db: [0.0; 31],
        }
    }
}

impl EqualizerSettings {
    pub fn is_valid(&self) -> bool {
        !self.preset_name.contains(['\0', '\r', '\n'])
            && self.preamp_db.is_finite()
            && (MIN_GAIN_DB..=MAX_GAIN_DB).contains(&self.preamp_db)
            && self
                .gains_db
                .iter()
                .all(|gain| gain.is_finite() && (MIN_GAIN_DB..=MAX_GAIN_DB).contains(gain))
    }

    pub fn serialize(&self) -> Result<String, String> {
        if !self.is_valid() {
            return Err("Equalizer settings contain an invalid gain or preset name".to_owned());
        }
        let gains = self
            .gains_db
            .iter()
            .map(f32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        Ok(format!(
            "version=1\nenabled={}\ntrack_genre={}\npreset={}\npreamp_db={}\ngains_db={gains}",
            self.enabled, self.track_genre, self.preset_name, self.preamp_db
        ))
    }

    pub fn parse(value: &str) -> Option<Self> {
        if value.len() > 4_096 {
            return None;
        }
        let mut version = None;
        let mut enabled = None;
        let mut track_genre = None;
        let mut preset_name = None;
        let mut preamp_db = None;
        let mut gains_db = None;
        for line in value.lines() {
            let (key, value) = line.split_once('=')?;
            match key {
                "version" => version = Some(value),
                "enabled" => enabled = parse_bool(value),
                "track_genre" => track_genre = parse_bool(value),
                "preset" => preset_name = Some(value.to_owned()),
                "preamp_db" => preamp_db = value.parse::<f32>().ok(),
                "gains_db" => {
                    let values = value
                        .split(',')
                        .map(str::parse::<f32>)
                        .collect::<Result<Vec<_>, _>>()
                        .ok()?;
                    gains_db = values.try_into().ok();
                }
                _ => return None,
            }
        }
        if version != Some("1") {
            return None;
        }
        let settings = Self {
            enabled: enabled?,
            track_genre: track_genre?,
            preset_name: preset_name?,
            preamp_db: preamp_db?,
            gains_db: gains_db?,
        };
        settings.is_valid().then_some(settings)
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EqualizerPreset {
    pub name: String,
    pub preamp_db: f32,
    pub gains_db: [f32; 31],
    aliases: Vec<String>,
}

pub fn presets() -> &'static [EqualizerPreset] {
    static PRESETS: OnceLock<Vec<EqualizerPreset>> = OnceLock::new();
    PRESETS.get_or_init(load_presets)
}

pub fn preset_names() -> Vec<&'static str> {
    presets()
        .iter()
        .map(|preset| preset.name.as_str())
        .chain(std::iter::once("Custom"))
        .collect()
}

pub fn preset_named(name: &str) -> Option<&'static EqualizerPreset> {
    presets().iter().find(|preset| preset.name == name)
}

pub fn preset_for_genre(genre: &str) -> &'static EqualizerPreset {
    if let Some(exact) = presets()
        .iter()
        .find(|preset| preset.name == genre || preset.aliases.iter().any(|alias| alias == genre))
    {
        return exact;
    }
    let genre = genre.to_lowercase();
    presets()
        .iter()
        .flat_map(|preset| {
            std::iter::once(preset.name.as_str())
                .chain(preset.aliases.iter().map(String::as_str))
                .map(move |key| (preset, key))
        })
        .filter(|(_, key)| genre.contains(&key.to_lowercase()))
        .max_by_key(|(_, key)| key.len())
        .map(|(preset, _)| preset)
        .or_else(|| preset_named("Flat"))
        .expect("the bundled Cog preset library contains Flat")
}

pub fn apply_preset(settings: &mut EqualizerSettings, preset: &EqualizerPreset) {
    settings.preset_name.clone_from(&preset.name);
    settings.preamp_db = preset.preamp_db;
    settings.gains_db = preset.gains_db;
}

fn load_presets() -> Vec<EqualizerPreset> {
    let root: serde_json::Value = serde_json::from_str(include_str!("../assets/Cog.q1.json"))
        .expect("the bundled Cog equalizer preset library is valid JSON");
    assert_eq!(
        root.get("type").and_then(serde_json::Value::as_str),
        Some("Cog EQ library file v1.0"),
        "unsupported bundled Cog equalizer library"
    );
    root.get("presets")
        .and_then(serde_json::Value::as_array)
        .expect("the bundled Cog equalizer library has presets")
        .iter()
        .map(|value| {
            let name = value
                .get("name")
                .and_then(serde_json::Value::as_str)
                .expect("Cog preset has a name")
                .to_owned();
            let ten_band_gains = COG_PRESET_FIELDS.map(|field| preset_gain(value, field));
            let aliases = value
                .get("altGenres")
                .and_then(serde_json::Value::as_array)
                .map(|values| {
                    values
                        .iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            EqualizerPreset {
                name,
                preamp_db: preset_gain(value, "preamp"),
                gains_db: EQUALIZER_FREQUENCIES
                    .map(|frequency| interpolate_preset_gain(&ten_band_gains, frequency)),
                aliases,
            }
        })
        .collect()
}

fn preset_gain(value: &serde_json::Value, field: &str) -> f32 {
    value
        .get(field)
        .and_then(serde_json::Value::as_i64)
        .filter(|value| (1..=401).contains(value))
        .map_or(0.0, |value| (value - 201) as f32 / 10.0)
}

fn interpolate_preset_gain(gains: &[f32; 10], target: f32) -> f32 {
    if target < COG_PRESET_FREQUENCIES[0] {
        let mut work = [0.0_f32; 14];
        let mut frequencies = [0.0_f32; 14];
        for index in 0..10 {
            work[9 - index] = gains[index];
            frequencies[9 - index] = COG_PRESET_FREQUENCIES[index];
        }
        extrapolate(&mut work, &mut frequencies);
        for index in 0..13 {
            let low = 13 - index;
            let high = 12 - index;
            if target >= frequencies[low] && target < frequencies[high] {
                return interpolate(
                    frequencies[low],
                    frequencies[high],
                    work[low],
                    work[high],
                    target,
                );
            }
        }
        return work[13];
    }
    if target > COG_PRESET_FREQUENCIES[9] {
        let mut work = [0.0_f32; 14];
        let mut frequencies = [0.0_f32; 14];
        work[..10].copy_from_slice(gains);
        frequencies[..10].copy_from_slice(&COG_PRESET_FREQUENCIES);
        extrapolate(&mut work, &mut frequencies);
        for index in 0..13 {
            if target >= frequencies[index] && target < frequencies[index + 1] {
                return interpolate(
                    frequencies[index],
                    frequencies[index + 1],
                    work[index],
                    work[index + 1],
                    target,
                );
            }
        }
        return work[13];
    }
    if target == COG_PRESET_FREQUENCIES[0] {
        return gains[0];
    }
    if target == COG_PRESET_FREQUENCIES[9] {
        return gains[9];
    }
    for index in 0..9 {
        if target >= COG_PRESET_FREQUENCIES[index] && target < COG_PRESET_FREQUENCIES[index + 1] {
            return interpolate(
                COG_PRESET_FREQUENCIES[index],
                COG_PRESET_FREQUENCIES[index + 1],
                gains[index],
                gains[index + 1],
                target,
            );
        }
    }
    0.0
}

fn extrapolate(work: &mut [f32; 14], frequencies: &mut [f32; 14]) {
    for index in 10..14 {
        work[index] = work[index - 1] + (work[index - 1] - work[index - 2]) * 1.05;
        frequencies[index] =
            frequencies[index - 1] + (frequencies[index - 1] - frequencies[index - 2]) * 1.05;
    }
}

fn interpolate(low: f32, high: f32, low_gain: f32, high_gain: f32, target: f32) -> f32 {
    low_gain + (high_gain - low_gain) * ((target - low) / (high - low))
}

#[derive(Clone)]
pub struct EqualizerControl {
    shared: Arc<EqualizerShared>,
}

struct EqualizerShared {
    settings: RwLock<EqualizerSettings>,
    revision: AtomicU64,
}

impl EqualizerControl {
    pub fn new(settings: EqualizerSettings) -> Self {
        Self {
            shared: Arc::new(EqualizerShared {
                settings: RwLock::new(settings),
                revision: AtomicU64::new(1),
            }),
        }
    }

    pub fn set(&self, settings: EqualizerSettings) {
        *self
            .shared
            .settings
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = settings;
        self.shared.revision.fetch_add(1, Ordering::Release);
    }

    pub fn reset(&self) {
        self.shared.revision.fetch_add(1, Ordering::Release);
    }

    fn snapshot(&self) -> (u64, EqualizerSettings) {
        let revision = self.shared.revision.load(Ordering::Acquire);
        let settings = self
            .shared
            .settings
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        (revision, settings)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct BiquadCoefficients {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl BiquadCoefficients {
    fn peaking(frequency: f64, gain_db: f32, sample_rate: f64) -> Self {
        if frequency <= 0.0 || frequency >= sample_rate / 2.0 {
            return Self {
                b0: 1.0,
                ..Self::default()
            };
        }
        let amplitude = 10.0_f64.powf(f64::from(gain_db) / 40.0);
        let omega = 2.0 * PI * frequency / sample_rate;
        let alpha = omega.sin() / (2.0 * EQUALIZER_Q);
        let cosine = omega.cos();
        let a0 = 1.0 + alpha / amplitude;
        Self {
            b0: (1.0 + alpha * amplitude) / a0,
            b1: (-2.0 * cosine) / a0,
            b2: (1.0 - alpha * amplitude) / a0,
            a1: (-2.0 * cosine) / a0,
            a2: (1.0 - alpha / amplitude) / a0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct BiquadState {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

impl BiquadState {
    fn process(&mut self, coefficients: BiquadCoefficients, input: f64) -> f64 {
        let output =
            coefficients.b0 * input + coefficients.b1 * self.x1 + coefficients.b2 * self.x2
                - coefficients.a1 * self.y1
                - coefficients.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;
        output
    }
}

pub struct EqualizerSource<S> {
    input: S,
    control: EqualizerControl,
    revision: u64,
    settings: EqualizerSettings,
    coefficients: [BiquadCoefficients; 31],
    states: Vec<[BiquadState; 31]>,
    channel_cursor: usize,
    channels: ChannelCount,
    sample_rate: SampleRate,
}

impl<S: Source<Item = f32>> EqualizerSource<S> {
    pub fn new(input: S, control: EqualizerControl) -> Self {
        let channels = input.channels();
        let sample_rate = input.sample_rate();
        let (revision, settings) = control.snapshot();
        let coefficients = coefficients_for(&settings, sample_rate.get());
        Self {
            input,
            control,
            revision,
            settings,
            coefficients,
            states: vec![[BiquadState::default(); 31]; usize::from(channels.get())],
            channel_cursor: 0,
            channels,
            sample_rate,
        }
    }

    fn refresh_at_frame_boundary(&mut self) {
        if self.channel_cursor != 0 {
            return;
        }
        let revision = self.control.shared.revision.load(Ordering::Acquire);
        if revision == self.revision {
            return;
        }
        let (revision, settings) = self.control.snapshot();
        self.revision = revision;
        self.coefficients = coefficients_for(&settings, self.sample_rate.get());
        self.settings = settings;
        self.states.fill([BiquadState::default(); 31]);
    }

    fn reset(&mut self) {
        self.states.fill([BiquadState::default(); 31]);
        self.channel_cursor = 0;
    }
}

fn coefficients_for(settings: &EqualizerSettings, sample_rate: u32) -> [BiquadCoefficients; 31] {
    std::array::from_fn(|index| {
        BiquadCoefficients::peaking(
            f64::from(EQUALIZER_FREQUENCIES[index]),
            settings.gains_db[index],
            f64::from(sample_rate),
        )
    })
}

impl<S: Source<Item = f32>> Iterator for EqualizerSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let input = self.input.next()?;
        self.refresh_at_frame_boundary();
        let channel = self.channel_cursor;
        self.channel_cursor = (self.channel_cursor + 1) % usize::from(self.channels.get());
        if !self.settings.enabled {
            return Some(input);
        }
        let mut output =
            f64::from(input) * 10.0_f64.powf(f64::from(self.settings.preamp_db) / 20.0);
        for (section, coefficients) in self.coefficients.iter().copied().enumerate() {
            output = self.states[channel][section].process(coefficients, output);
        }
        Some(output as f32)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

impl<S: Source<Item = f32>> Source for EqualizerSource<S> {
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
        self.reset();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZero;

    use rodio::buffer::SamplesBuffer;

    use super::*;

    fn buffer(samples: Vec<f32>, channels: u16, rate: u32) -> SamplesBuffer {
        SamplesBuffer::new(
            NonZero::new(channels).unwrap(),
            NonZero::new(rate).unwrap(),
            samples,
        )
    }

    #[test]
    fn bundled_library_matches_cogs_surface_and_interpolation() {
        assert_eq!(presets().len(), 22);
        assert_eq!(preset_named("Flat").unwrap().gains_db, [0.0; 31]);
        assert_eq!(preset_named("Rock").unwrap().gains_db[17], -1.0);
        assert_eq!(preset_for_genre("Progressive Rock").name, "Rock");
        assert_eq!(preset_for_genre("unmatched").name, "Flat");
    }

    #[test]
    fn settings_roundtrip_is_strict_and_bounded() {
        let mut gains_db = [0.0; 31];
        gains_db[17] = 6.25;
        let settings = EqualizerSettings {
            enabled: true,
            track_genre: true,
            preset_name: "Custom".to_owned(),
            preamp_db: -4.5,
            gains_db,
        };
        assert_eq!(
            EqualizerSettings::parse(&settings.serialize().unwrap()),
            Some(settings)
        );
        assert!(EqualizerSettings::parse("version=1").is_none());
        assert!(EqualizerSettings::parse(
            "version=1\nenabled=true\ntrack_genre=false\npreset=Custom\npreamp_db=NaN\ngains_db=0"
        )
        .is_none());
    }

    #[test]
    fn disabled_and_flat_equalizer_are_transparent() {
        let samples = vec![0.25, -0.5, 0.75, -1.0];
        let control = EqualizerControl::new(EqualizerSettings::default());
        assert_eq!(
            EqualizerSource::new(buffer(samples.clone(), 2, 48_000), control.clone())
                .collect::<Vec<_>>(),
            samples
        );

        let settings = EqualizerSettings {
            enabled: true,
            ..EqualizerSettings::default()
        };
        control.set(settings);
        let output =
            EqualizerSource::new(buffer(samples.clone(), 2, 48_000), control).collect::<Vec<_>>();
        for (actual, expected) in output.iter().zip(samples) {
            assert!((actual - expected).abs() < 1.0e-6);
        }
    }

    #[test]
    fn one_kilohertz_band_has_cogs_twelve_decibel_response() {
        let rate = 48_000_u32;
        let frames = rate as usize;
        let input = (0..frames)
            .map(|frame| {
                (2.0 * std::f32::consts::PI * 1_000.0 * frame as f32 / rate as f32).sin() * 0.1
            })
            .collect::<Vec<_>>();
        let mut gains_db = [0.0; 31];
        gains_db[17] = 12.0;
        let settings = EqualizerSettings {
            enabled: true,
            gains_db,
            ..EqualizerSettings::default()
        };
        let output = EqualizerSource::new(
            buffer(input.clone(), 1, rate),
            EqualizerControl::new(settings),
        )
        .collect::<Vec<_>>();
        let skip = rate as usize / 4;
        let input_rms = rms(&input[skip..]);
        let output_rms = rms(&output[skip..]);
        let gain = output_rms / input_rms;
        assert!((3.85..4.1).contains(&gain), "gain was {gain}");
    }

    #[test]
    fn channel_state_is_independent_and_live_updates_apply_on_frame_boundaries() {
        let control = EqualizerControl::new(EqualizerSettings::default());
        let mut source = EqualizerSource::new(
            buffer(vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0], 2, 48_000),
            control.clone(),
        );
        assert_eq!(source.next(), Some(1.0));
        let mut gains_db = [0.0; 31];
        gains_db[17] = 12.0;
        let settings = EqualizerSettings {
            enabled: true,
            gains_db,
            ..EqualizerSettings::default()
        };
        control.set(settings);
        assert_eq!(source.next(), Some(0.0));
        assert_eq!(source.next(), Some(0.0));
        assert_eq!(source.next(), Some(0.0));
    }

    #[test]
    fn explicit_reset_discards_filter_history() {
        let mut gains_db = [0.0; 31];
        gains_db[17] = 12.0;
        let settings = EqualizerSettings {
            enabled: true,
            gains_db,
            ..EqualizerSettings::default()
        };
        let control = EqualizerControl::new(settings);
        let mut source =
            EqualizerSource::new(buffer(vec![1.0, 0.0, 0.0], 1, 48_000), control.clone());
        assert_ne!(source.next(), Some(0.0));
        assert_ne!(source.next(), Some(0.0));
        control.reset();
        assert_eq!(source.next(), Some(0.0));
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
    }
}
