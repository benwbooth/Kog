use std::collections::{HashMap, HashSet};
use std::time::Duration;

use rodio::cpal;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::source::Zero;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, mixer};

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
    player: Option<Player>,
    decoders: DecoderRegistry,
    equalizer: EqualizerControl,
    output_device_id: Option<String>,
    volume: f32,
    state: PlaybackState,
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
            player: None,
            decoders,
            equalizer: EqualizerControl::new(equalizer),
            output_device_id,
            volume: 0.75,
            state: PlaybackState::Stopped,
        }
    }

    pub fn play_source(&mut self, source: &PlaybackSource) -> Result<SelectedBackend, String> {
        self.ensure_output()?;
        let player = self.player.as_ref().expect("output creates player");
        player.stop();
        self.equalizer.reset();
        let backend = self.decoders.append(source, player)?;
        player.set_volume(self.volume);
        player.play();
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
        if let Some(player) = self.player.as_ref() {
            player.stop();
            self.equalizer.reset();
        }
        self.state = PlaybackState::Stopped;
    }

    pub fn seek(&self, position: Duration) -> Result<(), String> {
        let player = self
            .player
            .as_ref()
            .ok_or_else(|| "Nothing is loaded".to_owned())?;
        player
            .try_seek(position)
            .map_err(|error| format!("seeking: {error}"))?;
        self.equalizer.reset();
        Ok(())
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
            let (output, player) = self.create_output(output_device_id.as_deref())?;
            self.output = Some(output);
            self.player = Some(player);
            self.state = PlaybackState::Stopped;
        }
        self.output_device_id = output_device_id;
        Ok(())
    }

    pub fn position(&self) -> Duration {
        self.player
            .as_ref()
            .map(Player::get_pos)
            .unwrap_or_default()
    }

    pub fn finished(&self) -> bool {
        self.state == PlaybackState::Playing && self.player.as_ref().is_some_and(Player::empty)
    }

    pub fn state(&self) -> PlaybackState {
        self.state
    }

    fn ensure_output(&mut self) -> Result<(), String> {
        if self.output.is_some() {
            return Ok(());
        }

        let (output, player) = self.create_output(self.output_device_id.as_deref())?;
        self.output = Some(output);
        self.player = Some(player);
        Ok(())
    }

    fn create_output(
        &self,
        output_device_id: Option<&str>,
    ) -> Result<(MixerDeviceSink, Player), String> {
        let output = open_output(output_device_id)?;
        let channels = output.config().channel_count();
        let sample_rate = output.config().sample_rate();
        let (processing_mixer, processing_source) = mixer::mixer(channels, sample_rate);
        // Rodio removes an empty mixer from its parent. Permanent silence keeps
        // this DSP bus alive between tracks without changing Player::empty().
        processing_mixer.add(Zero::new(channels, sample_rate));
        output.mixer().add(EqualizerSource::new(
            processing_source,
            self.equalizer.clone(),
        ));
        let player = Player::connect_new(&processing_mixer);
        Ok((output, player))
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
    use super::*;

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
}
