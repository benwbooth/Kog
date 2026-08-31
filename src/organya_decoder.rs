use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::organya::Organya;

const ORGANYA_EXTENSIONS: &[&str] = &["org"];
const ORGANYA_SAMPLE_RATE: u32 = 44_100;
const ORGANYA_CHANNELS: u16 = 2;
const ORGANYA_BITS_PER_SAMPLE: u8 = 32;
const ORGANYA_RENDER_FRAMES: usize = 1_024;
const ORGANYA_LOOP_COUNT: u32 = 2;
const ORGANYA_FADE: Duration = Duration::from_secs(8);

pub struct OrganyaBackend;

impl OrganyaBackend {
    fn open(source: &PlaybackSource) -> Result<Organya, String> {
        Organya::open(
            &source.path,
            ORGANYA_SAMPLE_RATE,
            ORGANYA_LOOP_COUNT,
            ORGANYA_FADE,
        )
    }
}

impl DecoderBackend for OrganyaBackend {
    fn id(&self) -> &'static str {
        "orgorg"
    }

    fn display_name(&self) -> &'static str {
        "orgorg 0.2.1"
    }

    fn extensions(&self) -> &'static [&'static str] {
        ORGANYA_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            loop_metadata: true,
            ..DecoderCapabilities::default()
        }
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let decoder = Self::open(source)?;
        Ok(StreamProperties {
            duration: Some(decoder.duration()),
            sample_rate: Some(ORGANYA_SAMPLE_RATE),
            channels: Some(ORGANYA_CHANNELS),
            codec: Some(decoder.codec_name().to_owned()),
            bits_per_sample: Some(ORGANYA_BITS_PER_SAMPLE),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(OrganyaSource::new(Self::open(source)?));
        Ok(())
    }
}

struct OrganyaSource {
    decoder: Organya,
    duration: Duration,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl OrganyaSource {
    fn new(decoder: Organya) -> Self {
        let duration = decoder.duration();
        Self {
            decoder,
            duration,
            pcm: vec![0.0; ORGANYA_RENDER_FRAMES * usize::from(ORGANYA_CHANNELS)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(ORGANYA_CHANNELS),
            Err(error) => {
                eprintln!("Kog Organya playback error: {error}");
                0
            }
        };
    }

    fn seek_to(&mut self, position: Duration) -> Result<(), String> {
        self.decoder.seek(position)?;
        self.pcm_samples = 0;
        self.pcm_index = 0;
        Ok(())
    }
}

impl Iterator for OrganyaSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pcm_index == self.pcm_samples {
            self.fill_pcm();
        }
        let sample = *self
            .pcm
            .get(self.pcm_index)
            .filter(|_| self.pcm_index < self.pcm_samples)?;
        self.pcm_index += 1;
        Some(sample)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl Source for OrganyaSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(ORGANYA_CHANNELS).expect("Organya output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(ORGANYA_SAMPLE_RATE).expect("Organya sample rate is nonzero")
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(self.duration)
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.seek_to(position)
            .map_err(|error| SeekError::Other(Arc::new(std::io::Error::other(error))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::{DecoderRegistry, DecoderSettings};

    fn fixture(test_name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let directory = std::env::temp_dir().join(format!(
            "kog-organya-backend-{}-{test_name}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).expect("create Organya fixture directory");
        let path = directory.join("fixture.org");
        std::fs::write(&path, crate::organya::test_org_bytes()).expect("write ORG fixture");
        std::fs::write(
            directory.join("soundbank.wdb"),
            crate::organya::test_soundbank_wdb_bytes(),
        )
        .expect("write soundbank fixture");
        (directory, path)
    }

    #[test]
    fn registry_routes_and_probes_organya() {
        let (directory, path) = fixture("probe");
        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("orgorg"));

        let source = PlaybackSource::from_path(path);
        let properties = registry.probe(&source).expect("probe ORG");
        assert_eq!(properties.duration, Some(Duration::from_millis(8_800)));
        assert_eq!(properties.sample_rate, Some(44_100));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.bits_per_sample, Some(32));
        assert_eq!(properties.codec.as_deref(), Some("Organya Org-02"));

        let track = crate::track::Track::from_source(source, &registry);
        assert_eq!(track.sample_rate, Some(44_100));
        assert_eq!(track.bits_per_sample, Some(32));
        assert_eq!(track.codec, "Organya Org-02");

        std::fs::remove_dir_all(directory).ok();
    }

    #[test]
    fn source_renders_non_silent_pcm_and_seeks() {
        let (directory, path) = fixture("source");
        let decoder =
            OrganyaBackend::open(&PlaybackSource::from_path(path)).expect("open ORG fixture");
        let mut source = OrganyaSource::new(decoder);
        assert!(
            source
                .by_ref()
                .take(4_410 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "rendered Organya PCM was silent"
        );
        source
            .try_seek(Duration::from_millis(250))
            .expect("seek Organya source");
        assert!(
            source
                .by_ref()
                .take(4_410 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "rendered Organya PCM was silent after seeking"
        );

        std::fs::remove_dir_all(directory).ok();
    }
}
