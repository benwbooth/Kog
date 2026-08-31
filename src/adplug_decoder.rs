use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::adplug::AdPlug;
use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};

const ADPLUG_SAMPLE_RATE: u32 = 44_100;
const ADPLUG_CHANNELS: u16 = 2;
const ADPLUG_RENDER_FRAMES: usize = 2_048;

pub struct AdPlugBackend;

impl AdPlugBackend {
    fn open(source: &PlaybackSource) -> Result<AdPlug, String> {
        AdPlug::open(
            &source.path,
            source.subsong.unwrap_or(0),
            ADPLUG_SAMPLE_RATE,
        )
    }
}

impl DecoderBackend for AdPlugBackend {
    fn id(&self) -> &'static str {
        "adplug"
    }

    fn display_name(&self) -> &'static str {
        "AdPlug (Cog pin) + Nuked OPL3"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            subsongs: true,
            ..DecoderCapabilities::default()
        }
    }

    fn accepts(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(AdPlug::supports_extension)
    }

    fn subsong_count(&self, path: &Path) -> Result<Option<u32>, String> {
        let decoder = AdPlug::open(path, 0, ADPLUG_SAMPLE_RATE)?;
        Ok(Some(decoder.subsong_count()))
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let decoder = Self::open(source)?;
        let metadata = decoder.metadata();
        Ok(StreamProperties {
            duration: Some(decoder.duration()),
            sample_rate: Some(decoder.sample_rate()),
            channels: Some(ADPLUG_CHANNELS),
            title: metadata.title.clone(),
            artist: metadata.artist.clone(),
            track_number: Some(decoder.selected_subsong() + 1),
            codec: Some(decoder.codec().to_owned()),
            bits_per_sample: Some(16),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(AdPlugSource::new(Self::open(source)?));
        Ok(())
    }
}

struct AdPlugSource {
    decoder: AdPlug,
    duration: Duration,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl AdPlugSource {
    fn new(decoder: AdPlug) -> Self {
        let duration = decoder.duration();
        Self {
            decoder,
            duration,
            pcm: vec![0.0; ADPLUG_RENDER_FRAMES * usize::from(ADPLUG_CHANNELS)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(ADPLUG_CHANNELS),
            Err(error) => {
                eprintln!("Kog AdPlug playback error: {error}");
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

impl Iterator for AdPlugSource {
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

impl Source for AdPlugSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(ADPLUG_CHANNELS).expect("AdPlug channel count is nonzero")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(ADPLUG_SAMPLE_RATE).expect("AdPlug sample rate is nonzero")
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

    fn fixture_path() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("native/adplug/test/2.CMF")
    }

    #[test]
    fn registry_expands_routes_and_probes_cmf_without_stealing_priority() {
        let path = fixture_path();
        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("adplug"));
        assert_eq!(
            registry.backend_id_for(Path::new("priority.mid")),
            Some("midi-rustysynth-sf2")
        );
        assert_eq!(
            registry.backend_id_for(Path::new("priority.s3m")),
            Some("libopenmpt")
        );
        assert_eq!(
            registry.backend_id_for(Path::new("priority.vgm")),
            Some("libvgm")
        );

        let sources = registry.expand(path).expect("expand CMF");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].subsong, Some(0));
        let properties = registry.probe(&sources[0]).expect("probe CMF");
        assert_eq!(properties.sample_rate, Some(ADPLUG_SAMPLE_RATE));
        assert_eq!(properties.channels, Some(ADPLUG_CHANNELS));
        assert_eq!(properties.bits_per_sample, Some(16));
        assert_eq!(properties.track_number, Some(1));
        assert_eq!(
            properties.codec.as_deref(),
            Some("Creative Music File (CMF)")
        );
        assert!(
            properties
                .duration
                .is_some_and(|value| value > Duration::ZERO)
        );
    }

    #[test]
    fn source_renders_non_silent_pcm_and_seeks() {
        let decoder = AdPlugBackend::open(&PlaybackSource {
            path: fixture_path(),
            subsong: Some(0),
            archive_origin: None,
        })
        .expect("open AdPlug source");
        let mut source = AdPlugSource::new(decoder);
        assert!(
            source
                .by_ref()
                .take(4_096)
                .any(|sample| sample.abs() > 0.000_01)
        );
        source
            .try_seek(Duration::from_millis(500))
            .expect("seek AdPlug source");
        assert!(
            source
                .by_ref()
                .take(4_096)
                .any(|sample| sample.abs() > 0.000_01)
        );
    }
}
