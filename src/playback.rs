use std::time::Duration;

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
        Self {
            output: None,
            player: None,
            decoders,
            equalizer: EqualizerControl::new(equalizer),
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

        let output = DeviceSinkBuilder::open_default_sink()
            .map_err(|error| format!("opening the default audio output: {error}"))?;
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
        self.output = Some(output);
        self.player = Some(player);
        Ok(())
    }
}
