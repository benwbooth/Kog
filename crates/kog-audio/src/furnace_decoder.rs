use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::furnace::Furnace;

const FURNACE_EXTENSIONS: &[&str] = &["ftm", "0cc", "dnm", "fur", "dmf"];
const FURNACE_SAMPLE_RATE: u32 = 44_100;
const FURNACE_CHANNELS: u16 = 2;
const FURNACE_RENDER_FRAMES: usize = 1_024;
const FURNACE_LOOP_COUNT: u32 = 2;
const FURNACE_FADE: Duration = Duration::from_secs(8);

pub struct FurnaceBackend;

impl FurnaceBackend {
    fn open(source: &PlaybackSource) -> Result<Furnace, String> {
        Furnace::open(&source.path, FURNACE_SAMPLE_RATE, source.subsong, FURNACE_LOOP_COUNT, FURNACE_FADE)
    }
}

impl DecoderBackend for FurnaceBackend {
    fn id(&self) -> &'static str {
        "furnace"
    }

    fn display_name(&self) -> &'static str {
        "Furnace 0.6.8.3"
    }

    fn extensions(&self) -> &'static [&'static str] {
        FURNACE_EXTENSIONS
    }

    // .ftm and .dmf are shared with OpenMPT's Face The Music and X-Tracker
    // formats: claim only files whose first bytes are Furnace's.
    fn accepts(&self, path: &Path) -> bool {
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            return false;
        };
        FURNACE_EXTENSIONS.iter().any(|candidate| candidate.eq_ignore_ascii_case(extension))
            && crate::furnace::is_furnace_module(path)
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            subsongs: true,
            loop_metadata: true,
            ..DecoderCapabilities::default()
        }
    }

    fn subsong_count(&self, path: &Path) -> Result<Option<u32>, String> {
        let decoder = Furnace::open(path, FURNACE_SAMPLE_RATE, Some(0), FURNACE_LOOP_COUNT, FURNACE_FADE)?;
        Ok(Some(decoder.subsong_count()))
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let decoder = Self::open(source)?;
        let title = decoder.title();
        Ok(StreamProperties {
            duration: Some(decoder.duration()),
            sample_rate: Some(FURNACE_SAMPLE_RATE),
            channels: Some(FURNACE_CHANNELS),
            title: (!title.is_empty()).then_some(title),
            artist: (!decoder.author().is_empty()).then(|| decoder.author().to_owned()),
            album: (!decoder.album().is_empty()).then(|| decoder.album().to_owned()),
            track_number: (decoder.subsong_count() > 1).then(|| decoder.subsong() + 1),
            codec: Some(codec_name(&source.path, decoder.system())),
            bits_per_sample: Some(32),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(FurnaceSource::new(Self::open(source)?));
        Ok(())
    }
}

fn codec_name(path: &Path, system: &str) -> String {
    let format = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("ftm") => "FamiTracker",
        Some("0cc") => "0CC-FamiTracker",
        Some("dnm") => "Dn-FamiTracker",
        Some("dmf") => "DefleMask",
        _ => "Furnace",
    };
    if system.is_empty() { format.to_owned() } else { format!("{format} ({system})") }
}

struct FurnaceSource {
    decoder: Furnace,
    duration: Duration,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl FurnaceSource {
    fn new(decoder: Furnace) -> Self {
        let duration = decoder.duration();
        Self {
            decoder,
            duration,
            pcm: vec![0.0; FURNACE_RENDER_FRAMES * usize::from(FURNACE_CHANNELS)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }
}

impl Iterator for FurnaceSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pcm_index == self.pcm_samples {
            self.pcm_index = 0;
            self.pcm_samples = self.decoder.render(&mut self.pcm) * usize::from(FURNACE_CHANNELS);
        }
        let sample = *self.pcm.get(self.pcm_index).filter(|_| self.pcm_index < self.pcm_samples)?;
        self.pcm_index += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl Source for FurnaceSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(FURNACE_CHANNELS).expect("Furnace output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(FURNACE_SAMPLE_RATE).expect("Furnace sample rate is nonzero")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.duration)
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.decoder
            .seek(position)
            .map(|_| {
                self.pcm_samples = 0;
                self.pcm_index = 0;
            })
            .map_err(|error| SeekError::Other(Arc::new(std::io::Error::other(error))))
    }
}
