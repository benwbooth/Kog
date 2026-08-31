use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::qsf::Qsf;

const QSF_EXTENSIONS: &[&str] = &["qsf", "miniqsf"];
const QSF_DEFAULT_LENGTH: Duration = Duration::from_secs(150);
const QSF_DEFAULT_FADE: Duration = Duration::from_secs(8);
const QSF_RENDER_FRAMES: usize = 2_048;

pub struct QsfBackend;

impl QsfBackend {
    fn open(source: &PlaybackSource) -> Result<Qsf, String> {
        Qsf::open(&source.path, QSF_DEFAULT_LENGTH, QSF_DEFAULT_FADE)
    }
}

impl DecoderBackend for QsfBackend {
    fn id(&self) -> &'static str {
        "highly-quixotic-qsf"
    }

    fn display_name(&self) -> &'static str {
        "Highly Quixotic / QSF"
    }

    fn extensions(&self) -> &'static [&'static str] {
        QSF_EXTENSIONS
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
            codec: Some("Capcom QSound Format (QSF) / Highly Quixotic".to_owned()),
            bits_per_sample: Some(16),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(QsfSource::new(Self::open(source)?));
        Ok(())
    }
}

fn tag_year(value: &str) -> Option<u32> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| part.len() == 4)
        .and_then(|part| part.parse().ok())
}

struct QsfSource {
    decoder: Qsf,
    duration: Duration,
    channels: u16,
    sample_rate: u32,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl QsfSource {
    fn new(decoder: Qsf) -> Self {
        let duration = decoder.duration();
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();
        Self {
            decoder,
            duration,
            channels,
            sample_rate,
            pcm: vec![0.0; QSF_RENDER_FRAMES * usize::from(channels)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(self.channels),
            Err(error) => {
                eprintln!("Kog QSF playback error: {error}");
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

impl Iterator for QsfSource {
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

impl Source for QsfSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(self.channels).expect("QSF channel count is nonzero")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.sample_rate).expect("QSF sample rate is nonzero")
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
    use crate::qsf::{test_qsf_bytes, test_qsf_malformed_section, test_qsf_program};

    fn fixture_tags() -> &'static str {
        concat!(
            "title=Kog QSF fixture\n",
            "artist=Kog tests\n",
            "game=Synthetic QSound program\n",
            "genre=Chiptune\n",
            "date=2026-08-31\n",
            "length=0:01.000\n",
            "fade=0:00.100\n",
        )
    }

    fn assert_duration_within_one_frame(actual: Option<Duration>, expected: Duration) {
        let actual = actual.expect("QSF duration");
        let frame = Duration::from_nanos(1_000_000_000 / 24_038 + 1);
        assert!(
            actual.abs_diff(expected) <= frame,
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn registry_routes_probes_and_decodes_generated_qsf() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("fixture.qsf");
        std::fs::write(
            &path,
            test_qsf_bytes(Some(&test_qsf_program()), fixture_tags()),
        )
        .unwrap();

        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("highly-quixotic-qsf"));
        let source = PlaybackSource::from_path(path);
        let properties = registry.probe(&source).expect("probe generated QSF");
        assert_duration_within_one_frame(properties.duration, Duration::from_millis(1_100));
        assert_eq!(properties.sample_rate, Some(24_038));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.title.as_deref(), Some("Kog QSF fixture"));
        assert_eq!(properties.artist.as_deref(), Some("Kog tests"));
        assert_eq!(
            properties.album.as_deref(),
            Some("Synthetic QSound program")
        );
        assert_eq!(properties.genre.as_deref(), Some("Chiptune"));
        assert_eq!(properties.year, Some(2026));
        assert_eq!(properties.bits_per_sample, Some(16));
    }

    #[test]
    fn generated_qsf_renders_audible_pcm_seeks_and_ends_exactly() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("fixture.qsf");
        std::fs::write(
            &path,
            test_qsf_bytes(Some(&test_qsf_program()), fixture_tags()),
        )
        .unwrap();
        let mut decoder =
            Qsf::open(&path, QSF_DEFAULT_LENGTH, QSF_DEFAULT_FADE).expect("open generated QSF");
        let mut pcm = vec![0.0; 8_192 * 2];
        assert_eq!(decoder.render(&mut pcm).expect("render QSF"), 8_192);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        assert_eq!(
            decoder.seek(Duration::from_millis(500)).unwrap(),
            Duration::from_millis(500)
        );
        pcm.fill(0.0);
        assert_eq!(
            decoder.render(&mut pcm).expect("render QSF after seek"),
            8_192
        );
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        decoder.seek(Duration::from_secs(10)).unwrap();
        assert_eq!(decoder.render(&mut pcm).expect("render QSF at end"), 0);
    }

    #[test]
    fn miniqsf_resolves_its_relative_library_and_outer_tags() {
        let fixture = tempfile::tempdir().unwrap();
        let library = fixture.path().join("fixture.qsflib");
        let mini = fixture.path().join("selection.miniqsf");
        std::fs::write(
            &library,
            test_qsf_bytes(Some(&test_qsf_program()), "title=library title\n"),
        )
        .unwrap();
        std::fs::write(
            &mini,
            test_qsf_bytes(
                None,
                "_lib=fixture.qsflib\ntitle=Mini selection\nlength=0:00.250\n",
            ),
        )
        .unwrap();

        let decoder = Qsf::open(&mini, QSF_DEFAULT_LENGTH, QSF_DEFAULT_FADE)
            .expect("open miniqsf library chain");
        assert_duration_within_one_frame(Some(decoder.duration()), Duration::from_millis(250));
        assert_eq!(decoder.metadata().title.as_deref(), Some("Mini selection"));
    }

    #[test]
    fn missing_length_uses_cogs_150_second_default_and_eight_second_fade() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("untimed.qsf");
        std::fs::write(
            &path,
            test_qsf_bytes(Some(&test_qsf_program()), "title=Untimed fixture\n"),
        )
        .unwrap();

        let decoder =
            Qsf::open(&path, QSF_DEFAULT_LENGTH, QSF_DEFAULT_FADE).expect("open untimed QSF");
        assert_duration_within_one_frame(Some(decoder.duration()), Duration::from_secs(158));
    }

    #[test]
    fn malformed_sections_and_missing_libraries_are_rejected() {
        let fixture = tempfile::tempdir().unwrap();
        let malformed_path = fixture.path().join("malformed.qsf");
        std::fs::write(
            &malformed_path,
            test_qsf_bytes(Some(&test_qsf_malformed_section()), ""),
        )
        .unwrap();
        assert!(Qsf::open(&malformed_path, QSF_DEFAULT_LENGTH, QSF_DEFAULT_FADE).is_err());

        let missing_path = fixture.path().join("missing.miniqsf");
        std::fs::write(
            &missing_path,
            test_qsf_bytes(None, "_lib=does-not-exist.qsflib\n"),
        )
        .unwrap();
        assert!(Qsf::open(&missing_path, QSF_DEFAULT_LENGTH, QSF_DEFAULT_FADE).is_err());
    }
}
