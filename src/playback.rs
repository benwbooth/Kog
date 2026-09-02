use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use rodio::cpal;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::source::{SeekError, Zero};
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, Source, mixer};

use crate::decoder::{DecoderRegistry, PlaybackSource, SelectedBackend};
use crate::equalizer::{EqualizerControl, EqualizerSettings, EqualizerSource};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackState {
    #[default]
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputDevice {
    pub id: String,
    pub name: String,
    pub label: String,
    pub is_default: bool,
}

pub fn available_output_devices() -> Result<Vec<OutputDevice>, String> {
    let host = cpal::default_host();
    let default_id = host
        .default_output_device()
        .and_then(|device| device.id().ok())
        .map(|id| id.to_string());
    let outputs = host
        .output_devices()
        .map_err(|error| format!("listing audio outputs: {error}"))?;
    Ok(sanitize_output_devices(
        outputs
            .filter_map(|output| {
                let id = output.id().ok()?.to_string();
                let description = output.description().ok()?;
                Some(OutputDevice {
                    is_default: default_id.as_deref() == Some(&id),
                    id,
                    name: description.name().to_owned(),
                    label: description.to_string(),
                })
            })
            .collect(),
    ))
}

fn sanitize_output_devices(devices: Vec<OutputDevice>) -> Vec<OutputDevice> {
    let mut seen = HashSet::new();
    let mut devices = devices
        .into_iter()
        .filter(|device| {
            valid_device_text(&device.id, 1_024)
                && valid_device_text(&device.name, 512)
                && valid_device_text(&device.label, 1_024)
                && seen.insert(device.id.clone())
        })
        .collect::<Vec<_>>();
    let counts = devices.iter().fold(HashMap::new(), |mut counts, device| {
        *counts.entry(device.label.clone()).or_insert(0_usize) += 1;
        counts
    });
    let mut occurrences = HashMap::new();
    for device in &mut devices {
        if counts.get(&device.label).copied().unwrap_or_default() > 1 {
            let occurrence = occurrences.entry(device.label.clone()).or_insert(0_usize);
            *occurrence += 1;
            device.label = format!("{} ({})", device.label, occurrence);
        }
    }
    devices
}

fn valid_device_text(value: &str, maximum_length: usize) -> bool {
    !value.is_empty() && value.len() <= maximum_length && !value.contains(['\0', '\r', '\n'])
}

impl PlaybackState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Playing => "playing",
            Self::Paused => "paused",
        }
    }
}

pub struct PlaybackEngine {
    output: Option<MixerDeviceSink>,
    processing_mixer: Option<mixer::Mixer>,
    player: Option<Arc<Player>>,
    seek_worker: AsyncSeekWorker,
    decoders: DecoderRegistry,
    equalizer: EqualizerControl,
    meter: AudioMeter,
    output_device_id: Option<String>,
    volume: f32,
    state: PlaybackState,
}

struct AsyncSeekWorker {
    shared: Arc<AsyncSeekShared>,
    worker: Option<JoinHandle<()>>,
}

struct AsyncSeekShared {
    state: Mutex<AsyncSeekState>,
    ready: Condvar,
}

#[derive(Default)]
struct AsyncSeekState {
    request: Option<AsyncSeekRequest>,
    latest_generation: u64,
    pending_target: Option<Duration>,
    result: Option<Result<Duration, String>>,
    shutdown: bool,
}

struct AsyncSeekRequest {
    player: Arc<Player>,
    position: Duration,
    generation: u64,
}

impl Default for PlaybackEngine {
    fn default() -> Self {
        Self::new(DecoderRegistry::default())
    }
}

impl PlaybackEngine {
    pub fn new(decoders: DecoderRegistry) -> Self {
        Self::with_equalizer(decoders, EqualizerSettings::default())
    }

    pub fn with_equalizer(decoders: DecoderRegistry, equalizer: EqualizerSettings) -> Self {
        Self::with_equalizer_and_output(decoders, equalizer, None)
    }

    pub fn with_equalizer_and_output(
        decoders: DecoderRegistry,
        equalizer: EqualizerSettings,
        output_device_id: Option<String>,
    ) -> Self {
        Self {
            output: None,
            processing_mixer: None,
            player: None,
            seek_worker: AsyncSeekWorker::new(),
            decoders,
            equalizer: EqualizerControl::new(equalizer),
            meter: AudioMeter::default(),
            output_device_id,
            volume: 0.75,
            state: PlaybackState::Stopped,
        }
    }

    pub fn play_source(&mut self, source: &PlaybackSource) -> Result<SelectedBackend, String> {
        self.ensure_output()?;
        self.seek_worker.cancel();
        if let Some(player) = self.player.take() {
            player.stop();
        }
        let player = Arc::new(Player::connect_new(
            self.processing_mixer
                .as_ref()
                .expect("output creates processing mixer"),
        ));
        self.equalizer.reset();
        self.meter.reset();
        let backend = match self.decoders.append(source, &player) {
            Ok(backend) => backend,
            Err(error) => {
                player.stop();
                return Err(error);
            }
        };
        player.set_volume(self.volume);
        player.play();
        self.player = Some(player);
        self.state = PlaybackState::Playing;
        Ok(backend)
    }

    pub fn play_pause(&mut self) {
        let Some(player) = self.player.as_ref() else {
            return;
        };
        match self.state {
            PlaybackState::Playing => {
                player.pause();
                self.state = PlaybackState::Paused;
            }
            PlaybackState::Paused => {
                player.play();
                self.state = PlaybackState::Playing;
            }
            PlaybackState::Stopped => {}
        }
    }

    pub fn stop(&mut self) {
        self.seek_worker.cancel();
        if let Some(player) = self.player.as_ref() {
            player.stop();
            self.equalizer.reset();
            self.meter.reset();
        }
        self.state = PlaybackState::Stopped;
    }

    pub fn seek(&self, position: Duration) -> Result<(), String> {
        let player = self
            .player
            .as_ref()
            .ok_or_else(|| "Nothing is loaded".to_owned())?;
        self.seek_worker.request(player.clone(), position);
        self.equalizer.reset();
        self.meter.reset();
        Ok(())
    }

    pub fn take_seek_result(&self) -> Option<Result<Duration, String>> {
        self.seek_worker.take_result()
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(player) = self.player.as_ref() {
            player.set_volume(self.volume);
        }
    }

    pub fn set_equalizer(&self, settings: EqualizerSettings) {
        self.equalizer.set(settings);
    }

    pub fn switch_output_device(&mut self, output_device_id: Option<String>) -> Result<(), String> {
        if self.output_device_id == output_device_id {
            return Ok(());
        }
        if self.output.is_some() {
            let (output, processing_mixer) = self.create_output(output_device_id.as_deref())?;
            self.seek_worker.cancel();
            if let Some(player) = self.player.take() {
                player.stop();
            }
            self.output = Some(output);
            self.processing_mixer = Some(processing_mixer);
            self.state = PlaybackState::Stopped;
            self.meter.reset();
        }
        self.output_device_id = output_device_id;
        Ok(())
    }

    pub fn position(&self) -> Duration {
        if let Some(position) = self.seek_worker.pending_target() {
            return position;
        }
        self.player
            .as_ref()
            .map(|player| player.get_pos())
            .unwrap_or_default()
    }

    pub fn finished(&self) -> bool {
        self.state == PlaybackState::Playing
            && self.player.as_ref().is_some_and(|player| player.empty())
    }

    pub fn state(&self) -> PlaybackState {
        self.state
    }

    pub fn audio_levels(&self) -> [f32; 5] {
        self.meter.levels()
    }

    fn ensure_output(&mut self) -> Result<(), String> {
        if self.output.is_some() {
            return Ok(());
        }

        let (output, processing_mixer) = self.create_output(self.output_device_id.as_deref())?;
        self.output = Some(output);
        self.processing_mixer = Some(processing_mixer);
        Ok(())
    }

    fn create_output(
        &self,
        output_device_id: Option<&str>,
    ) -> Result<(MixerDeviceSink, mixer::Mixer), String> {
        let output = open_output(output_device_id)?;
        let channels = output.config().channel_count();
        let sample_rate = output.config().sample_rate();
        let (processing_mixer, processing_source) = mixer::mixer(channels, sample_rate);
        // Rodio removes an empty mixer from its parent. Permanent silence keeps
        // this DSP bus alive between tracks without changing Player::empty().
        processing_mixer.add(Zero::new(channels, sample_rate));
        output.mixer().add(AudioMeterSource::new(
            EqualizerSource::new(processing_source, self.equalizer.clone()),
            self.meter.clone(),
        ));
        Ok((output, processing_mixer))
    }
}

impl AsyncSeekWorker {
    fn new() -> Self {
        let shared = Arc::new(AsyncSeekShared {
            state: Mutex::new(AsyncSeekState::default()),
            ready: Condvar::new(),
        });
        let worker_shared = shared.clone();
        let worker = std::thread::Builder::new()
            .name("kog-audio-seek".to_owned())
            .spawn(move || run_seek_worker(worker_shared))
            .expect("Kog requires a playback seek worker thread");
        Self {
            shared,
            worker: Some(worker),
        }
    }

    fn request(&self, player: Arc<Player>, position: Duration) {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.latest_generation = state.latest_generation.wrapping_add(1);
        let generation = state.latest_generation;
        state.pending_target = Some(position);
        state.result = None;
        state.request = Some(AsyncSeekRequest {
            player,
            position,
            generation,
        });
        drop(state);
        self.shared.ready.notify_one();
    }

    fn cancel(&self) {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.latest_generation = state.latest_generation.wrapping_add(1);
        state.request = None;
        state.pending_target = None;
        state.result = None;
    }

    fn pending_target(&self) -> Option<Duration> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pending_target
    }

    fn take_result(&self) -> Option<Result<Duration, String>> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .result
            .take()
    }
}

impl Drop for AsyncSeekWorker {
    fn drop(&mut self) {
        let mut state = self
            .shared
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.shutdown = true;
        state.request = None;
        drop(state);
        self.shared.ready.notify_one();
        // A third-party decoder may already be inside a broken seek call. Do
        // not make application shutdown wait for code Kog cannot interrupt.
        let _ = self.worker.take();
    }
}

fn run_seek_worker(shared: Arc<AsyncSeekShared>) {
    loop {
        let request = {
            let mut state = shared
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            while state.request.is_none() && !state.shutdown {
                state = shared
                    .ready
                    .wait(state)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
            if state.shutdown {
                return;
            }
            state.request.take().expect("seek request was checked")
        };
        let result = request
            .player
            .try_seek(request.position)
            .map(|()| request.position)
            .map_err(|error| format!("seeking: {error}"));
        let mut state = shared
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.latest_generation == request.generation {
            state.pending_target = None;
            state.result = Some(result);
        }
    }
}

const AUDIO_METER_BANDS: usize = 5;
const AUDIO_METER_SPLITS_HZ: [f32; AUDIO_METER_BANDS - 1] = [180.0, 700.0, 2_500.0, 7_000.0];
const AUDIO_METER_GAIN: [f32; AUDIO_METER_BANDS] = [1.35, 1.2, 1.0, 1.05, 1.2];

#[derive(Clone)]
struct AudioMeter {
    levels: Arc<[AtomicU32; AUDIO_METER_BANDS]>,
}

impl Default for AudioMeter {
    fn default() -> Self {
        Self {
            levels: Arc::new(std::array::from_fn(|_| AtomicU32::new(0))),
        }
    }
}

impl AudioMeter {
    fn levels(&self) -> [f32; AUDIO_METER_BANDS] {
        std::array::from_fn(|index| f32::from_bits(self.levels[index].load(Ordering::Relaxed)))
    }

    fn publish(&self, levels: [f32; AUDIO_METER_BANDS]) {
        for (target, level) in self.levels.iter().zip(levels) {
            target.store(level.to_bits(), Ordering::Relaxed);
        }
    }

    fn reset(&self) {
        self.publish([0.0; AUDIO_METER_BANDS]);
    }
}

struct AudioMeterSource<S> {
    input: S,
    meter: AudioMeter,
    channels: ChannelCount,
    sample_rate: SampleRate,
    channel_cursor: usize,
    frame_sum: f32,
    low_pass: [f32; AUDIO_METER_BANDS - 1],
    low_pass_alpha: [f32; AUDIO_METER_BANDS - 1],
    energy: [f64; AUDIO_METER_BANDS],
    frames_in_window: u32,
    frames_per_window: u32,
    smoothed: [f32; AUDIO_METER_BANDS],
}

impl<S: Source<Item = f32>> AudioMeterSource<S> {
    fn new(input: S, meter: AudioMeter) -> Self {
        let channels = input.channels();
        let sample_rate = input.sample_rate();
        let rate = sample_rate.get() as f32;
        let low_pass_alpha = AUDIO_METER_SPLITS_HZ.map(|frequency| {
            let frequency = frequency.min(rate * 0.45);
            1.0 - (-2.0 * std::f32::consts::PI * frequency / rate).exp()
        });
        Self {
            input,
            meter,
            channels,
            sample_rate,
            channel_cursor: 0,
            frame_sum: 0.0,
            low_pass: [0.0; AUDIO_METER_BANDS - 1],
            low_pass_alpha,
            energy: [0.0; AUDIO_METER_BANDS],
            frames_in_window: 0,
            frames_per_window: (sample_rate.get() / 50).max(64),
            smoothed: [0.0; AUDIO_METER_BANDS],
        }
    }

    fn observe_frame(&mut self, sample: f32) {
        for (low_pass, alpha) in self.low_pass.iter_mut().zip(self.low_pass_alpha) {
            *low_pass += alpha * (sample - *low_pass);
        }
        let bands = [
            self.low_pass[0],
            self.low_pass[1] - self.low_pass[0],
            self.low_pass[2] - self.low_pass[1],
            self.low_pass[3] - self.low_pass[2],
            sample - self.low_pass[3],
        ];
        for (energy, band) in self.energy.iter_mut().zip(bands) {
            *energy += f64::from(band) * f64::from(band);
        }
        self.frames_in_window += 1;
        if self.frames_in_window < self.frames_per_window {
            return;
        }

        let frames = f64::from(self.frames_in_window);
        for index in 0..AUDIO_METER_BANDS {
            let rms = (self.energy[index] / frames).sqrt() as f32 * AUDIO_METER_GAIN[index];
            let decibels = 20.0 * rms.max(0.000_001).log10();
            let target = ((decibels + 60.0) / 60.0).clamp(0.0, 1.0);
            let smoothing = if target > self.smoothed[index] {
                0.72
            } else {
                0.16
            };
            self.smoothed[index] += smoothing * (target - self.smoothed[index]);
            if self.smoothed[index] < 0.004 {
                self.smoothed[index] = 0.0;
            }
        }
        self.meter.publish(self.smoothed);
        self.energy.fill(0.0);
        self.frames_in_window = 0;
    }

    fn reset(&mut self) {
        self.channel_cursor = 0;
        self.frame_sum = 0.0;
        self.low_pass.fill(0.0);
        self.energy.fill(0.0);
        self.frames_in_window = 0;
        self.smoothed.fill(0.0);
        self.meter.reset();
    }
}

impl<S: Source<Item = f32>> Iterator for AudioMeterSource<S> {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.input.next()?;
        self.frame_sum += sample;
        self.channel_cursor += 1;
        if self.channel_cursor == usize::from(self.channels.get()) {
            let mono = self.frame_sum / f32::from(self.channels.get());
            self.observe_frame(mono);
            self.channel_cursor = 0;
            self.frame_sum = 0.0;
        }
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.input.size_hint()
    }
}

impl<S: Source<Item = f32>> Source for AudioMeterSource<S> {
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

fn open_output(output_device_id: Option<&str>) -> Result<MixerDeviceSink, String> {
    let Some(output_device_id) = output_device_id else {
        return DeviceSinkBuilder::open_default_sink()
            .map_err(|error| format!("opening the system default audio output: {error}"));
    };
    let output = cpal::default_host()
        .output_devices()
        .map_err(|error| format!("listing audio outputs: {error}"))?
        .find(|output| {
            output
                .id()
                .is_ok_and(|id| id.to_string() == output_device_id)
        })
        .ok_or_else(|| format!("The selected audio output is unavailable: {output_device_id}"))?;
    DeviceSinkBuilder::from_device(output)
        .and_then(DeviceSinkBuilder::open_stream)
        .map_err(|error| format!("opening audio output {output_device_id}: {error}"))
}

#[cfg(test)]
mod tests {
    use std::num::NonZero;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    use rodio::buffer::SamplesBuffer;

    use super::*;

    struct SlowSeekSource {
        entered_seek: Arc<AtomicBool>,
        positions: Arc<Mutex<Vec<Duration>>>,
    }

    impl Iterator for SlowSeekSource {
        type Item = f32;

        fn next(&mut self) -> Option<Self::Item> {
            Some(0.0)
        }
    }

    impl Source for SlowSeekSource {
        fn current_span_len(&self) -> Option<usize> {
            None
        }

        fn channels(&self) -> ChannelCount {
            NonZero::new(2).unwrap()
        }

        fn sample_rate(&self) -> SampleRate {
            NonZero::new(48_000).unwrap()
        }

        fn total_duration(&self) -> Option<Duration> {
            Some(Duration::from_secs(60))
        }

        fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
            self.entered_seek.store(true, Ordering::Release);
            std::thread::sleep(Duration::from_millis(150));
            self.positions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(position);
            Ok(())
        }
    }

    fn measured_sine(frequency: f32) -> [f32; AUDIO_METER_BANDS] {
        let sample_rate = 48_000_u32;
        let samples = (0..sample_rate / 4)
            .map(|frame| {
                (2.0 * std::f32::consts::PI * frequency * frame as f32 / sample_rate as f32).sin()
                    * 0.5
            })
            .collect::<Vec<_>>();
        let meter = AudioMeter::default();
        let source = SamplesBuffer::new(
            NonZero::new(1).unwrap(),
            NonZero::new(sample_rate).unwrap(),
            samples.clone(),
        );
        let output = AudioMeterSource::new(source, meter.clone()).collect::<Vec<_>>();
        assert_eq!(output, samples, "metering must not alter playback samples");
        meter.levels()
    }

    #[test]
    fn output_devices_are_unique_safe_and_disambiguated_for_qml() {
        assert_eq!(
            sanitize_output_devices(vec![
                OutputDevice {
                    id: "alsa:a".to_owned(),
                    name: "Speakers".to_owned(),
                    label: "Speakers".to_owned(),
                    is_default: true
                },
                OutputDevice {
                    id: "alsa:b".to_owned(),
                    name: "Speakers".to_owned(),
                    label: "Speakers".to_owned(),
                    is_default: false
                },
                OutputDevice {
                    id: "alsa:b".to_owned(),
                    name: "duplicate id".to_owned(),
                    label: "duplicate id".to_owned(),
                    is_default: false
                },
                OutputDevice {
                    id: "alsa:c".to_owned(),
                    name: "Line\nOut".to_owned(),
                    label: "Line Out".to_owned(),
                    is_default: false
                },
            ]),
            vec![
                OutputDevice {
                    id: "alsa:a".to_owned(),
                    name: "Speakers".to_owned(),
                    label: "Speakers (1)".to_owned(),
                    is_default: true
                },
                OutputDevice {
                    id: "alsa:b".to_owned(),
                    name: "Speakers".to_owned(),
                    label: "Speakers (2)".to_owned(),
                    is_default: false
                },
            ]
        );
    }

    #[test]
    fn selecting_an_output_before_playback_defers_opening_it() {
        let mut playback = PlaybackEngine::default();
        playback
            .switch_output_device(Some("Deferred device".to_owned()))
            .unwrap();
        assert_eq!(
            playback.output_device_id.as_deref(),
            Some("Deferred device")
        );
        assert_eq!(playback.state(), PlaybackState::Stopped);
        assert!(playback.output.is_none());
    }

    #[test]
    fn slow_decoder_seeks_never_block_the_caller_and_latest_request_wins() {
        let entered_seek = Arc::new(AtomicBool::new(false));
        let positions = Arc::new(Mutex::new(Vec::new()));
        let (player, mut output) = Player::new();
        player.append(SlowSeekSource {
            entered_seek: entered_seek.clone(),
            positions: positions.clone(),
        });
        let player = Arc::new(player);
        let consuming = Arc::new(AtomicBool::new(true));
        let consumer_flag = consuming.clone();
        let consumer = std::thread::spawn(move || {
            while consumer_flag.load(Ordering::Acquire) {
                let _ = output.next();
            }
        });
        let worker = AsyncSeekWorker::new();

        let started = Instant::now();
        worker.request(player.clone(), Duration::from_secs(1));
        assert!(
            started.elapsed() < Duration::from_millis(20),
            "queueing a seek blocked for {:?}",
            started.elapsed()
        );
        let deadline = Instant::now() + Duration::from_secs(1);
        while !entered_seek.load(Ordering::Acquire) && Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(entered_seek.load(Ordering::Acquire));

        worker.request(player.clone(), Duration::from_secs(2));
        worker.request(player.clone(), Duration::from_secs(3));
        assert_eq!(worker.pending_target(), Some(Duration::from_secs(3)));

        let result = loop {
            if let Some(result) = worker.take_result() {
                break result;
            }
            assert!(Instant::now() < deadline, "asynchronous seek timed out");
            std::thread::sleep(Duration::from_millis(5));
        };
        assert_eq!(result.unwrap(), Duration::from_secs(3));
        assert_eq!(
            *positions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            [Duration::from_secs(1), Duration::from_secs(3)]
        );

        consuming.store(false, Ordering::Release);
        consumer.join().unwrap();
    }

    #[test]
    fn audio_meter_levels_come_from_the_real_frequency_content() {
        let bass = measured_sine(90.0);
        let treble = measured_sine(10_000.0);

        assert!(
            bass[0] > bass[4],
            "bass should favor the low band: {bass:?}"
        );
        assert!(
            treble[4] > treble[0],
            "treble should favor the high band: {treble:?}"
        );
        assert!(bass.iter().copied().fold(0.0_f32, f32::max) > 0.25);
        assert!(treble.iter().copied().fold(0.0_f32, f32::max) > 0.25);
    }
}
