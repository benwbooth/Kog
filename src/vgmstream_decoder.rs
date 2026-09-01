use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::vgmstream::Vgmstream;

const VGMSTREAM_LOOP_COUNT: f64 = 2.0;
const VGMSTREAM_FADE: Duration = Duration::from_secs(8);
const VGMSTREAM_RENDER_FRAMES: usize = 2_048;

pub struct VgmstreamBackend;

impl VgmstreamBackend {
    fn open(source: &PlaybackSource) -> Result<Vgmstream, String> {
        Vgmstream::open(
            &source.path,
            source.subsong,
            VGMSTREAM_LOOP_COUNT,
            VGMSTREAM_FADE,
        )
    }
}

impl DecoderBackend for VgmstreamBackend {
    fn id(&self) -> &'static str {
        "vgmstream"
    }

    fn display_name(&self) -> &'static str {
        "vgmstream r2117 (built-in codecs)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }

    fn advertised_extensions(&self) -> Vec<String> {
        Vgmstream::supported_extensions()
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            subsongs: true,
            loop_metadata: true,
            companion_files: true,
        }
    }

    fn accepts(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(Vgmstream::supports_extension)
    }

    fn subsong_count(&self, path: &Path) -> Result<Option<u32>, String> {
        let decoder = Vgmstream::open(path, None, VGMSTREAM_LOOP_COUNT, VGMSTREAM_FADE)?;
        Ok(Some(decoder.subsong_count()))
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let decoder = Self::open(source)?;
        let metadata = decoder.metadata();
        Ok(StreamProperties {
            duration: Some(decoder.duration()),
            sample_rate: Some(decoder.sample_rate()),
            channels: Some(decoder.channels()),
            title: metadata.title.clone(),
            artist: metadata.artist.clone(),
            album: metadata.album.clone(),
            year: metadata.year,
            track_number: metadata
                .track_number
                .or(Some(decoder.selected_subsong() + 1)),
            codec: Some(decoder.codec().to_owned()),
            bitrate: decoder.bitrate(),
            bits_per_sample: Some(32),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(VgmstreamSource::new(Self::open(source)?));
        Ok(())
    }
}

struct VgmstreamSource {
    decoder: Vgmstream,
    duration: Duration,
    sample_rate: u32,
    channels: u16,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl VgmstreamSource {
    fn new(decoder: Vgmstream) -> Self {
        let duration = decoder.duration();
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();
        Self {
            decoder,
            duration,
            sample_rate,
            channels,
            pcm: vec![0.0; VGMSTREAM_RENDER_FRAMES * usize::from(channels)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(self.channels),
            Err(error) => {
                eprintln!("Kog vgmstream playback error: {error}");
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

impl Iterator for VgmstreamSource {
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

impl Source for VgmstreamSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(self.channels).expect("vgmstream channel count is nonzero")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.sample_rate).expect("vgmstream sample rate is nonzero")
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
        let directory = std::env::temp_dir().join(format!(
            "kog-vgmstream-backend-{}-{test_name}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).expect("create vgmstream fixture directory");
        let path = directory.join("fixture.vag");
        std::fs::write(&path, crate::vgmstream::test_vag_bytes()).expect("write VAG fixture");
        std::fs::write(
            directory.join("!tags.m3u"),
            "# %TITLE    Kog VGMStream Backend\n# %ARTIST   Kog Fixture Artist\nfixture.vag\n",
        )
        .expect("write vgmstream tag fixture");
        path
    }

    fn remove_fixture(path: &Path) {
        if let Some(directory) = path.parent() {
            std::fs::remove_dir_all(directory).ok();
        }
    }

    #[test]
    fn registry_routes_expands_and_probes_vag_last() {
        let path = fixture_path("probe");
        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("vgmstream"));
        assert_eq!(
            registry.backend_id_for(Path::new("priority.ahx")),
            Some("hivelytracker")
        );
        assert_eq!(
            registry.backend_id_for(Path::new("priority.wav")),
            Some("rodio-symphonia")
        );

        let sources = registry.expand(path.clone()).expect("expand VAG");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].subsong, Some(0));
        let properties = registry.probe(&sources[0]).expect("probe VAG");
        assert_eq!(properties.sample_rate, Some(44_100));
        assert_eq!(properties.channels, Some(1));
        assert_eq!(properties.bits_per_sample, Some(32));
        assert_eq!(properties.title.as_deref(), Some("Kog VGMStream Backend"));
        assert_eq!(properties.artist.as_deref(), Some("Kog Fixture Artist"));
        assert_eq!(properties.codec.as_deref(), Some("PlayStation 4-bit ADPCM"));
        assert_eq!(properties.track_number, Some(1));
        assert!(
            properties
                .duration
                .is_some_and(|value| value > Duration::from_millis(100))
        );

        let track = crate::track::Track::from_source(sources[0].clone(), &registry);
        assert_eq!(track.title, "Kog VGMStream Backend");
        assert_eq!(track.artist, "Kog Fixture Artist");
        assert_eq!(track.sample_rate, Some(44_100));
        assert_eq!(track.channels, Some(1));
        assert_eq!(track.codec, "PlayStation 4-bit ADPCM");
        remove_fixture(&path);
    }

    #[test]
    fn source_renders_non_silent_pcm_seeks_and_ends() {
        let path = fixture_path("source");
        let decoder = VgmstreamBackend::open(&PlaybackSource {
            path: path.clone(),
            remote_url: None,
            subsong: Some(0),
            archive_origin: None,
        })
        .expect("open VAG fixture");
        let mut source = VgmstreamSource::new(decoder);
        assert!(
            source
                .by_ref()
                .take(2_048)
                .any(|sample| sample.abs() > 0.000_01)
        );
        source
            .try_seek(Duration::from_millis(50))
            .expect("seek vgmstream source");
        assert!(
            source
                .by_ref()
                .take(2_048)
                .any(|sample| sample.abs() > 0.000_01)
        );
        source.try_seek(Duration::ZERO).expect("rewind VAG source");
        assert_eq!(source.count(), 256 * 28);
        remove_fixture(&path);
    }
}
