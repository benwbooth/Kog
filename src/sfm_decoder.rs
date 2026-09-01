use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::sfm::Sfm;

const SFM_EXTENSIONS: &[&str] = &["sfm"];
const SFM_RENDER_FRAMES: usize = 2_048;

pub struct SfmBackend;

impl DecoderBackend for SfmBackend {
    fn id(&self) -> &'static str {
        "cog-gme-sfm"
    }

    fn display_name(&self) -> &'static str {
        "Cog GME SFM core (isolated helper)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        SFM_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            loop_metadata: true,
            ..DecoderCapabilities::default()
        }
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let decoder = Sfm::open(&source.path)?;
        Ok(StreamProperties {
            duration: Some(decoder.duration()),
            sample_rate: Some(decoder.sample_rate()),
            channels: Some(decoder.channels()),
            title: nonempty(decoder.title()),
            artist: nonempty(decoder.author()),
            album: nonempty(decoder.game()),
            genre: nonempty(decoder.system()),
            year: parse_year(decoder.date()),
            codec: Some("Super Nintendo SFM (Cog GME core)".to_owned()),
            bits_per_sample: Some(16),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(SfmSource::new(Sfm::open(&source.path)?));
        Ok(())
    }
}

fn nonempty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_owned())
}

fn parse_year(value: &str) -> Option<u32> {
    value
        .as_bytes()
        .windows(4)
        .find(|window| window.iter().all(u8::is_ascii_digit))
        .and_then(|window| std::str::from_utf8(window).ok())
        .and_then(|year| year.parse().ok())
}

struct SfmSource {
    decoder: Sfm,
    duration: Duration,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl SfmSource {
    fn new(decoder: Sfm) -> Self {
        let duration = decoder.duration();
        let channels = usize::from(decoder.channels());
        Self {
            decoder,
            duration,
            pcm: vec![0.0; SFM_RENDER_FRAMES * channels],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(self.decoder.channels()),
            Err(error) => {
                eprintln!("Kog SFM playback error: {error}");
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

impl Iterator for SfmSource {
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

impl Source for SfmSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(self.decoder.channels()).expect("SFM output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.decoder.sample_rate()).expect("SFM sample rate is nonzero")
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
            "kog-sfm-backend-{}-{test_name}.sfm",
            std::process::id()
        ));
        std::fs::write(&path, crate::sfm::test_sfm_bytes()).expect("write generated SFM fixture");
        path
    }

    #[test]
    fn registry_routes_and_probes_synthetic_sfm() {
        let path = fixture_path("probe");
        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("cog-gme-sfm"));
        let sources = registry.expand(path.clone()).expect("expand SFM");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].subsong, None);
        let properties = registry.probe(&sources[0]).expect("probe SFM");
        assert_eq!(properties.duration, Some(Duration::from_millis(600)));
        assert_eq!(properties.sample_rate, Some(32_000));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.bits_per_sample, Some(16));
        assert_eq!(properties.title.as_deref(), Some("Synthetic SFM"));
        assert_eq!(properties.artist.as_deref(), Some("Kog tests"));
        assert_eq!(properties.album.as_deref(), Some("Kog test suite"));
        assert_eq!(properties.genre.as_deref(), Some("Super Nintendo with log"));
        assert_eq!(properties.year, Some(2026));
        assert_eq!(
            properties.codec.as_deref(),
            Some("Super Nintendo SFM (Cog GME core)")
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn source_renders_non_silent_pcm_seeks_and_ends_exactly() {
        let path = fixture_path("source");
        let decoder = Sfm::open(&path).expect("open generated SFM");
        let expected_samples = decoder.total_frames() * 2;
        let mut source = SfmSource::new(decoder);
        let rendered = source.by_ref().collect::<Vec<_>>();
        assert_eq!(rendered.len() as u64, expected_samples);
        assert!(
            rendered.iter().any(|sample| sample.abs() > 0.000_01),
            "generated SFM was silent"
        );

        let decoder = Sfm::open(&path).expect("reopen generated SFM");
        let mut source = SfmSource::new(decoder);
        source
            .try_seek(Duration::from_millis(250))
            .expect("seek SFM source");
        assert!(
            source.take(3_200).any(|sample| sample.abs() > 0.000_01),
            "generated SFM was silent after seek"
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn malformed_state_is_rejected_by_the_helper_boundary() {
        let path = fixture_path("malformed");
        let mut bytes = crate::sfm::test_sfm_bytes();
        bytes[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        std::fs::write(&path, bytes).expect("write malformed SFM fixture");
        let error = Sfm::open(&path).err().expect("malformed SFM must fail");
        assert!(error.contains("metadata exceeds"), "{error}");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn year_parser_accepts_embedded_years() {
        assert_eq!(parse_year("released 1998-03-17"), Some(1998));
        assert_eq!(parse_year("unknown"), None);
    }
}
