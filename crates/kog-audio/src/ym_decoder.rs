use std::num::{NonZeroU16, NonZeroU32};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::ym::{YmPlayer, YmSong};

const YM_EXTENSIONS: &[&str] = &["ym"];
const YM_SAMPLE_RATE: u32 = 44_100;
const YM_CHANNELS: u16 = 2;
const YM_RENDER_FRAMES: usize = 1_024;

pub struct YmBackend;

impl DecoderBackend for YmBackend {
    fn id(&self) -> &'static str {
        "ym-ayumi"
    }

    fn display_name(&self) -> &'static str {
        "YM register dumps / ayumi"
    }

    fn extensions(&self) -> &'static [&'static str] {
        YM_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities { seek: true, ..DecoderCapabilities::default() }
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let song = YmSong::open(&source.path)?;
        Ok(StreamProperties {
            duration: Some(song.duration()),
            sample_rate: Some(YM_SAMPLE_RATE),
            channels: Some(YM_CHANNELS),
            title: (!song.title.is_empty()).then(|| song.title.clone()),
            artist: (!song.author.is_empty()).then(|| song.author.clone()),
            codec: Some(match song.clock {
                2_000_000 => "YM2149 (Atari ST)".to_owned(),
                clock => format!("AY-3-8910 at {} kHz", clock / 1_000),
            }),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        let song = YmSong::open(&source.path)?;
        crate::silence::append_trimmed(player, YmSource::new(YmPlayer::new(song, YM_SAMPLE_RATE)?));
        Ok(())
    }
}

struct YmSource {
    player: YmPlayer,
    duration: Duration,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl YmSource {
    fn new(player: YmPlayer) -> Self {
        let duration = player.song().duration();
        Self {
            player,
            duration,
            pcm: vec![0.0; YM_RENDER_FRAMES * usize::from(YM_CHANNELS)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }
}

impl Iterator for YmSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.pcm_index == self.pcm_samples {
            self.pcm_index = 0;
            self.pcm_samples = self.player.render(&mut self.pcm) * usize::from(YM_CHANNELS);
        }
        let sample = *self.pcm.get(self.pcm_index).filter(|_| self.pcm_index < self.pcm_samples)?;
        self.pcm_index += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl Source for YmSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(YM_CHANNELS).expect("YM output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(YM_SAMPLE_RATE).expect("YM sample rate is nonzero")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.duration)
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.player.seek(position);
        self.pcm_samples = 0;
        self.pcm_index = 0;
        Ok(())
    }
}
