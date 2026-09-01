use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::syntrax::Syntrax;

const SYNTRAX_EXTENSIONS: &[&str] = &["jxs"];
const SYNTRAX_RENDER_FRAMES: usize = 2_048;

pub struct SyntraxBackend;

impl SyntraxBackend {
    fn open(source: &PlaybackSource) -> Result<Syntrax, String> {
        Syntrax::open(&source.path, source.subsong)
    }
}

impl DecoderBackend for SyntraxBackend {
    fn id(&self) -> &'static str {
        "syntrax"
    }

    fn display_name(&self) -> &'static str {
        "syntrax-c (upstream 1184fb9)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        SYNTRAX_EXTENSIONS
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
        Ok(Some(Syntrax::open(path, Some(0))?.subsong_count()))
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let decoder = Self::open(source)?;
        Ok(StreamProperties {
            duration: Some(decoder.duration()),
            sample_rate: Some(decoder.sample_rate()),
            channels: Some(decoder.channels()),
            title: (!decoder.title().is_empty()).then(|| decoder.title().to_owned()),
            track_number: Some(decoder.selected_subsong() + 1),
            codec: Some("Syntrax".to_owned()),
            bits_per_sample: Some(16),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(SyntraxSource::new(Self::open(source)?));
        Ok(())
    }
}

struct SyntraxSource {
    decoder: Syntrax,
    duration: Duration,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl SyntraxSource {
    fn new(decoder: Syntrax) -> Self {
        let duration = decoder.duration();
        let channels = usize::from(decoder.channels());
        Self {
            decoder,
            duration,
            pcm: vec![0.0; SYNTRAX_RENDER_FRAMES * channels],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(self.decoder.channels()),
            Err(error) => {
                eprintln!("Kog Syntrax playback error: {error}");
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

impl Iterator for SyntraxSource {
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

impl Source for SyntraxSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(self.decoder.channels()).expect("Syntrax output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.decoder.sample_rate()).expect("Syntrax sample rate is nonzero")
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
            "kog-syntrax-backend-{}-{test_name}.jxs",
            std::process::id()
        ));
        std::fs::write(&path, crate::syntrax::test_jxs_bytes())
            .expect("write generated JXS fixture");
        path
    }

    #[test]
    fn registry_expands_routes_and_probes_jxs_subsongs() {
        let path = fixture_path("probe");
        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("syntrax"));
        let sources = registry.expand(path.clone()).expect("expand JXS subsongs");
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].subsong, Some(0));
        assert_eq!(sources[1].subsong, Some(1));
        let properties = registry.probe(&sources[1]).expect("probe JXS");
        assert_eq!(properties.sample_rate, Some(44_100));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.bits_per_sample, Some(16));
        assert_eq!(properties.track_number, Some(2));
        assert_eq!(properties.codec.as_deref(), Some("Syntrax"));
        assert_eq!(properties.title.as_deref(), Some("Synthetic JXS B"));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn source_renders_non_silent_pcm_seeks_and_ends_exactly() {
        let path = fixture_path("source");
        let decoder = SyntraxBackend::open(&PlaybackSource {
            path: path.clone(),
            subsong: Some(0),
            archive_origin: None,
        })
        .expect("open generated JXS");
        let expected_samples = decoder.total_frames() * 2;
        let mut source = SyntraxSource::new(decoder);
        let rendered = source.by_ref().collect::<Vec<_>>();
        assert_eq!(rendered.len() as u64, expected_samples);
        assert!(rendered.iter().any(|sample| sample.abs() > 0.000_01));

        let decoder = SyntraxBackend::open(&PlaybackSource {
            path: path.clone(),
            subsong: Some(0),
            archive_origin: None,
        })
        .expect("reopen generated JXS");
        let mut source = SyntraxSource::new(decoder);
        assert!(source.try_seek(Duration::from_millis(50)).is_ok());
        assert!(source.take(4_410).any(|sample| sample.abs() > 0.000_01));
        std::fs::remove_file(path).ok();
    }
}
