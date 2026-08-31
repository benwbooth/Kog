use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::ncsf::Ncsf;

const NCSF_EXTENSIONS: &[&str] = &["ncsf", "minincsf"];
const NCSF_DEFAULT_LENGTH: Duration = Duration::from_secs(150);
const NCSF_DEFAULT_FADE: Duration = Duration::from_secs(8);
const NCSF_RENDER_FRAMES: usize = 2_048;

pub struct NcsfBackend;

impl NcsfBackend {
    fn open(source: &PlaybackSource) -> Result<Ncsf, String> {
        Ncsf::open(&source.path, NCSF_DEFAULT_LENGTH, NCSF_DEFAULT_FADE)
    }
}

impl DecoderBackend for NcsfBackend {
    fn id(&self) -> &'static str {
        "sseqplayer-ncsf"
    }

    fn display_name(&self) -> &'static str {
        "SSEQPlayer / NCSF"
    }

    fn extensions(&self) -> &'static [&'static str] {
        NCSF_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            companion_files: true,
            ..DecoderCapabilities::default()
        }
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
            genre: metadata.genre.clone(),
            year: metadata.date.as_deref().and_then(tag_year),
            codec: Some("Nintendo DS Sound Format (NCSF) / SSEQPlayer".to_owned()),
            bits_per_sample: Some(16),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(NcsfSource::new(Self::open(source)?));
        Ok(())
    }
}

fn tag_year(value: &str) -> Option<u32> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| part.len() == 4)
        .and_then(|part| part.parse().ok())
}

struct NcsfSource {
    decoder: Ncsf,
    duration: Duration,
    channels: u16,
    sample_rate: u32,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl NcsfSource {
    fn new(decoder: Ncsf) -> Self {
        let duration = decoder.duration();
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();
        Self {
            decoder,
            duration,
            channels,
            sample_rate,
            pcm: vec![0.0; NCSF_RENDER_FRAMES * usize::from(channels)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(self.channels),
            Err(error) => {
                eprintln!("Kog NCSF playback error: {error}");
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

impl Iterator for NcsfSource {
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

impl Source for NcsfSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(self.channels).expect("NCSF channel count is nonzero")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.sample_rate).expect("NCSF sample rate is nonzero")
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
    use crate::ncsf::{test_ncsf_bytes, test_sdat_bytes};

    fn fixture_tags() -> &'static str {
        concat!(
            "title=Kog NCSF fixture\n",
            "artist=Kog tests\n",
            "game=Synthetic SDAT\n",
            "genre=Chiptune\n",
            "date=2026-08-31\n",
            "length=0:01.000\n",
            "fade=0:00.100\n",
        )
    }

    #[test]
    fn registry_routes_probes_and_decodes_generated_ncsf() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("fixture.ncsf");
        std::fs::write(
            &path,
            test_ncsf_bytes(Some(&test_sdat_bytes()), fixture_tags()),
        )
        .unwrap();

        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("sseqplayer-ncsf"));
        let source = PlaybackSource::from_path(path);
        let properties = registry.probe(&source).expect("probe generated NCSF");
        assert_eq!(properties.duration, Some(Duration::from_millis(1_100)));
        assert_eq!(properties.sample_rate, Some(44_100));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.title.as_deref(), Some("Kog NCSF fixture"));
        assert_eq!(properties.artist.as_deref(), Some("Kog tests"));
        assert_eq!(properties.album.as_deref(), Some("Synthetic SDAT"));
        assert_eq!(properties.genre.as_deref(), Some("Chiptune"));
        assert_eq!(properties.year, Some(2026));
        assert_eq!(properties.bits_per_sample, Some(16));
    }

    #[test]
    fn generated_ncsf_renders_audible_pcm_seeks_and_ends_exactly() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("fixture.ncsf");
        std::fs::write(
            &path,
            test_ncsf_bytes(Some(&test_sdat_bytes()), fixture_tags()),
        )
        .unwrap();
        let mut decoder =
            Ncsf::open(&path, NCSF_DEFAULT_LENGTH, NCSF_DEFAULT_FADE).expect("open generated NCSF");
        let mut pcm = vec![0.0; 8_192 * 2];
        assert_eq!(decoder.render(&mut pcm).expect("render NCSF"), 8_192);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        assert_eq!(
            decoder.seek(Duration::from_millis(500)).unwrap(),
            Duration::from_millis(500)
        );
        pcm.fill(0.0);
        assert_eq!(
            decoder.render(&mut pcm).expect("render NCSF after seek"),
            8_192
        );
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        decoder.seek(Duration::from_secs(10)).unwrap();
        assert_eq!(decoder.render(&mut pcm).expect("render NCSF at end"), 0);
    }

    #[test]
    fn minincsf_resolves_its_relative_library_and_outer_tags() {
        let fixture = tempfile::tempdir().unwrap();
        let library = fixture.path().join("fixture.ncsflib");
        let mini = fixture.path().join("selection.minincsf");
        std::fs::write(
            &library,
            test_ncsf_bytes(Some(&test_sdat_bytes()), "title=library title\n"),
        )
        .unwrap();
        std::fs::write(
            &mini,
            test_ncsf_bytes(
                None,
                "_lib=fixture.ncsflib\ntitle=Mini selection\nlength=0:00.250\n",
            ),
        )
        .unwrap();

        let decoder = Ncsf::open(&mini, NCSF_DEFAULT_LENGTH, NCSF_DEFAULT_FADE)
            .expect("open minincsf library chain");
        assert_eq!(decoder.duration(), Duration::from_millis(250));
        assert_eq!(decoder.metadata().title.as_deref(), Some("Mini selection"));
    }

    #[test]
    fn missing_length_uses_cogs_150_second_default_and_eight_second_fade() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("untimed.ncsf");
        std::fs::write(
            &path,
            test_ncsf_bytes(Some(&test_sdat_bytes()), "title=Untimed fixture\n"),
        )
        .unwrap();

        let decoder =
            Ncsf::open(&path, NCSF_DEFAULT_LENGTH, NCSF_DEFAULT_FADE).expect("open untimed NCSF");
        assert_eq!(decoder.duration(), Duration::from_secs(158));
    }

    #[test]
    fn malformed_sdat_and_missing_library_are_reported_without_entering_the_replayer() {
        let fixture = tempfile::tempdir().unwrap();
        let malformed_path = fixture.path().join("malformed.ncsf");
        let mut sdat = test_sdat_bytes();
        sdat[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
        std::fs::write(&malformed_path, test_ncsf_bytes(Some(&sdat), "")).unwrap();
        let error = match Ncsf::open(&malformed_path, NCSF_DEFAULT_LENGTH, NCSF_DEFAULT_FADE) {
            Ok(_) => panic!("malformed SDAT unexpectedly opened"),
            Err(error) => error,
        };
        assert!(error.contains("malformed NCSF SDAT"), "{error}");

        let missing_path = fixture.path().join("missing.minincsf");
        std::fs::write(
            &missing_path,
            test_ncsf_bytes(None, "_lib=does-not-exist.ncsflib\n"),
        )
        .unwrap();
        let error = match Ncsf::open(&missing_path, NCSF_DEFAULT_LENGTH, NCSF_DEFAULT_FADE) {
            Ok(_) => panic!("minincsf with a missing library unexpectedly opened"),
            Err(error) => error,
        };
        assert!(!error.is_empty());
    }
}
