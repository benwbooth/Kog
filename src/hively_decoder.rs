use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::hively::Hively;

const HIVELY_EXTENSIONS: &[&str] = &["hvl", "ahx"];
const HIVELY_SAMPLE_RATE: u32 = 44_100;
const HIVELY_CHANNELS: u16 = 2;
const HIVELY_BITS_PER_SAMPLE: u8 = 32;
const HIVELY_RENDER_FRAMES: usize = 1_024;
const HIVELY_LOOP_COUNT: u32 = 2;
const HIVELY_FADE: Duration = Duration::from_secs(8);

pub struct HivelyBackend;

impl HivelyBackend {
    fn open(source: &PlaybackSource) -> Result<Hively, String> {
        Hively::open(
            &source.path,
            HIVELY_SAMPLE_RATE,
            source.subsong,
            HIVELY_LOOP_COUNT,
            HIVELY_FADE,
        )
    }
}

impl DecoderBackend for HivelyBackend {
    fn id(&self) -> &'static str {
        "hivelytracker"
    }

    fn display_name(&self) -> &'static str {
        "HivelyTracker 1.9 (upstream f393ca7)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        HIVELY_EXTENSIONS
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
        let decoder = Hively::open(
            path,
            HIVELY_SAMPLE_RATE,
            Some(0),
            HIVELY_LOOP_COUNT,
            HIVELY_FADE,
        )?;
        Ok(Some(decoder.subsong_count()))
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let decoder = Self::open(source)?;
        Ok(StreamProperties {
            duration: Some(decoder.duration()),
            sample_rate: Some(HIVELY_SAMPLE_RATE),
            channels: Some(HIVELY_CHANNELS),
            title: (!decoder.title().is_empty()).then(|| decoder.title().to_owned()),
            track_number: Some(decoder.selected_subsong() + 1),
            codec: Some(codec_name(&source.path).to_owned()),
            bits_per_sample: Some(HIVELY_BITS_PER_SAMPLE),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(HivelySource::new(Self::open(source)?));
        Ok(())
    }
}

fn codec_name(path: &Path) -> &'static str {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ahx"))
    {
        "Abyss' Highest eXperience"
    } else {
        "HivelyTracker"
    }
}

struct HivelySource {
    decoder: Hively,
    duration: Duration,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl HivelySource {
    fn new(decoder: Hively) -> Self {
        let duration = decoder.duration();
        Self {
            decoder,
            duration,
            pcm: vec![0.0; HIVELY_RENDER_FRAMES * usize::from(HIVELY_CHANNELS)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(HIVELY_CHANNELS),
            Err(error) => {
                eprintln!("Kog HivelyTracker playback error: {error}");
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

impl Iterator for HivelySource {
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

impl Source for HivelySource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(HIVELY_CHANNELS).expect("HivelyTracker output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(HIVELY_SAMPLE_RATE).expect("HivelyTracker sample rate is nonzero")
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

    fn fixture_path(test_name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "kog-hively-backend-{}-{test_name}.hvl",
            std::process::id()
        ));
        std::fs::write(&path, crate::hively::test_multisubsong_hvl_bytes())
            .expect("write HVL fixture");
        path
    }

    #[test]
    fn registry_expands_routes_and_probes_hvl_subsongs() {
        let path = fixture_path("probe");
        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("hivelytracker"));

        let sources = registry.expand(path.clone()).expect("expand HVL subsongs");
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].subsong, Some(0));
        assert_eq!(sources[1].subsong, Some(1));
        let properties = registry.probe(&sources[1]).expect("probe HVL");
        assert!(
            properties
                .duration
                .is_some_and(|duration| duration.as_secs() > 8)
        );
        assert_eq!(properties.sample_rate, Some(44_100));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.bits_per_sample, Some(32));
        assert_eq!(properties.track_number, Some(2));
        assert_eq!(properties.codec.as_deref(), Some("HivelyTracker"));
        assert!(properties.title.is_some());

        let track = crate::track::Track::from_source(sources[1].clone(), &registry);
        assert_eq!(track.track_number, Some(2));
        assert_eq!(track.sample_rate, Some(44_100));
        assert_eq!(track.bits_per_sample, Some(32));
        assert_eq!(track.codec, "HivelyTracker");

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn source_renders_non_silent_pcm_and_seeks() {
        let path = fixture_path("source");
        let decoder = HivelyBackend::open(&PlaybackSource {
            path: path.clone(),
            subsong: Some(0),
        })
        .expect("open HVL fixture");
        let mut source = HivelySource::new(decoder);
        assert!(
            source
                .by_ref()
                .take(4_410 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "rendered HivelyTracker PCM was silent"
        );
        source
            .try_seek(Duration::from_secs(1))
            .expect("seek HivelyTracker source");
        assert!(
            source
                .by_ref()
                .take(4_410 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "rendered HivelyTracker PCM was silent after seeking"
        );

        std::fs::remove_file(path).ok();
    }
}
