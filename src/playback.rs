use std::time::Duration;

use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player};

use crate::decoder::{DecoderRegistry, PlaybackSource, SelectedBackend};

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
        Self {
            output: None,
            player: None,
            decoders,
            volume: 0.75,
            state: PlaybackState::Stopped,
        }
    }

    pub fn play_source(&mut self, source: &PlaybackSource) -> Result<SelectedBackend, String> {
        self.ensure_output()?;
        let player = self.player.as_ref().expect("output creates player");
        player.stop();
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
            .map_err(|error| format!("seeking: {error}"))
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
        if let Some(player) = self.player.as_ref() {
            player.set_volume(self.volume);
        }
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
        let player = Player::connect_new(output.mixer());
        self.output = Some(output);
        self.player = Some(player);
        Ok(())
    }
}
