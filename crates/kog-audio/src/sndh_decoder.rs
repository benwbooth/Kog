use std::num::{NonZeroU16, NonZeroU32};
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::sndh::Sndh;

const SNDH_EXTENSIONS: &[&str] = &["sndh", "snd", "sc68"];
const SNDH_FADE: Duration = Duration::from_secs(8);
const SNDH_SAMPLE_RATE: u32 = 44_100;
const SNDH_CHANNELS: u16 = 2;
const SNDH_RENDER_FRAMES: usize = 1_024;

pub struct SndhBackend;

impl DecoderBackend for SndhBackend {
    fn id(&self) -> &'static str {
        "sc68"
    }

    fn display_name(&self) -> &'static str {
        "sc68 3.0 (Atari ST)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        SNDH_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities { seek: true, subsongs: true, ..DecoderCapabilities::default() }
    }

    // .snd is also vgmstream's (and other programs'); claim only SNDH data.
    fn accepts(&self, path: &std::path::Path) -> bool {
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
        SNDH_EXTENSIONS.contains(&extension.as_str()) && looks_like_sndh(path, &extension)
    }

    fn subsong_count(&self, path: &std::path::Path) -> Result<Option<u32>, String> {
        Ok(Some(Sndh::open(path, SNDH_SAMPLE_RATE, Some(0), SNDH_FADE)?.track_count()))
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let song = Sndh::open(&source.path, SNDH_SAMPLE_RATE, source.subsong, SNDH_FADE)?;
        let text = |which| Some(song.text(which)).filter(|text| !text.is_empty());
        Ok(StreamProperties {
            duration: Some(song.duration()),
            sample_rate: Some(SNDH_SAMPLE_RATE),
            channels: Some(SNDH_CHANNELS),
            title: text(0),
            artist: text(1),
            album: text(2),
            track_number: (song.track_count() > 1).then(|| song.track() + 1),
            codec: Some(text(3).map_or("SNDH".to_owned(), |hardware| format!("SNDH ({hardware})"))),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        let song = Sndh::open(&source.path, SNDH_SAMPLE_RATE, source.subsong, SNDH_FADE)?;
        crate::silence::append_trimmed(player, SndhSource::new(song));
        Ok(())
    }
}

/// SNDH files start with a branch then "SNDH" (often ICE-packed: "ICE!");
/// sc68 files with "SC68".
fn looks_like_sndh(path: &std::path::Path, extension: &str) -> bool {
    use std::io::Read;
    let mut head = [0_u8; 16];
    let Ok(mut file) = std::fs::File::open(path) else { return false };
    let read = file.read(&mut head).unwrap_or(0);
    let head = &head[..read];
    let has = |needle: &[u8]| head.windows(needle.len()).any(|window| window == needle);
    has(b"SNDH") || has(b"ICE!") || has(b"Ice!") || (extension == "sc68" && has(b"SC68"))
}

struct SndhSource {
    player: Sndh,
    duration: Duration,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl SndhSource {
    fn new(player: Sndh) -> Self {
        let duration = player.duration();
        Self {
            player,
            duration,
            pcm: vec![0.0; SNDH_RENDER_FRAMES * usize::from(SNDH_CHANNELS)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }
}

impl Iterator for SndhSource {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.pcm_index == self.pcm_samples {
            self.pcm_index = 0;
            self.pcm_samples = self.player.render(&mut self.pcm) * usize::from(SNDH_CHANNELS);
        }
        let sample = *self.pcm.get(self.pcm_index).filter(|_| self.pcm_index < self.pcm_samples)?;
        self.pcm_index += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl Source for SndhSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(SNDH_CHANNELS).expect("SNDH output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(SNDH_SAMPLE_RATE).expect("SNDH sample rate is nonzero")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.duration)
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.player
            .seek(position)
            .map_err(|error| SeekError::Other(std::sync::Arc::new(std::io::Error::other(error))))?;
        self.pcm_samples = 0;
        self.pcm_index = 0;
        Ok(())
    }
}
