use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::gsf::Gsf;

const GSF_EXTENSIONS: &[&str] = &["gsf", "minigsf"];
const GSF_DEFAULT_LENGTH: Duration = Duration::from_secs(150);
const GSF_DEFAULT_FADE: Duration = Duration::from_secs(8);
const GSF_RENDER_FRAMES: usize = 2_048;

pub struct GsfBackend;

impl GsfBackend {
    fn open(source: &PlaybackSource) -> Result<Gsf, String> {
        Gsf::open(&source.path, GSF_DEFAULT_LENGTH, GSF_DEFAULT_FADE)
    }
}

impl DecoderBackend for GsfBackend {
    fn id(&self) -> &'static str {
        "mgba-gsf"
    }

    fn display_name(&self) -> &'static str {
        "mGBA / GSF"
    }

    fn extensions(&self) -> &'static [&'static str] {
        GSF_EXTENSIONS
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
            codec: Some("Game Boy Advance Sound Format (GSF) / mGBA".to_owned()),
            bits_per_sample: Some(16),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(GsfSource::new(Self::open(source)?));
        Ok(())
    }
}

fn tag_year(value: &str) -> Option<u32> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| part.len() == 4)
        .and_then(|part| part.parse().ok())
}

struct GsfSource {
    decoder: Gsf,
    duration: Duration,
    channels: u16,
    sample_rate: u32,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl GsfSource {
    fn new(decoder: Gsf) -> Self {
        let duration = decoder.duration();
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();
        Self {
            decoder,
            duration,
            channels,
            sample_rate,
            pcm: vec![0.0; GSF_RENDER_FRAMES * usize::from(channels)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(self.channels),
            Err(error) => {
                eprintln!("Kog GSF playback error: {error}");
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

impl Iterator for GsfSource {
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

impl Source for GsfSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(self.channels).expect("GSF channel count is nonzero")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.sample_rate).expect("GSF sample rate is nonzero")
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
    use crate::gsf::{test_gba_rom, test_gsf_bytes, test_raw_gsf_bytes};

    fn fixture_tags() -> &'static str {
        concat!(
            "title=Kog GSF fixture\n",
            "artist=Kog tests\n",
            "game=Synthetic GBA ROM\n",
            "genre=Chiptune\n",
            "date=2026-08-31\n",
            "length=0:01.000\n",
            "fade=0:00.100\n",
        )
    }

    fn assert_duration_within_one_frame(actual: Option<Duration>, expected: Duration) {
        let actual = actual.expect("GSF duration");
        let frame = Duration::from_nanos(1_000_000_000 / 32_768 + 1);
        assert!(
            actual.abs_diff(expected) <= frame,
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn registry_routes_probes_and_decodes_generated_gsf() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("fixture.gsf");
        std::fs::write(&path, test_gsf_bytes(Some(&test_gba_rom()), fixture_tags())).unwrap();

        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("mgba-gsf"));
        let source = PlaybackSource::from_path(path);
        let properties = registry.probe(&source).expect("probe generated GSF");
        assert_duration_within_one_frame(properties.duration, Duration::from_millis(1_100));
        assert_eq!(properties.sample_rate, Some(32_768));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.title.as_deref(), Some("Kog GSF fixture"));
        assert_eq!(properties.artist.as_deref(), Some("Kog tests"));
        assert_eq!(properties.album.as_deref(), Some("Synthetic GBA ROM"));
        assert_eq!(properties.genre.as_deref(), Some("Chiptune"));
        assert_eq!(properties.year, Some(2026));
        assert_eq!(properties.bits_per_sample, Some(16));
    }

    #[test]
    fn generated_gsf_renders_audible_pcm_seeks_and_ends_exactly() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("fixture.gsf");
        std::fs::write(&path, test_gsf_bytes(Some(&test_gba_rom()), fixture_tags())).unwrap();
        let mut decoder =
            Gsf::open(&path, GSF_DEFAULT_LENGTH, GSF_DEFAULT_FADE).expect("open generated GSF");
        let mut pcm = vec![0.0; 8_192 * 2];
        assert_eq!(decoder.render(&mut pcm).expect("render GSF"), 8_192);
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        assert_eq!(
            decoder.seek(Duration::from_millis(500)).unwrap(),
            Duration::from_millis(500)
        );
        pcm.fill(0.0);
        assert_eq!(
            decoder.render(&mut pcm).expect("render GSF after seek"),
            8_192
        );
        assert!(pcm.iter().any(|sample| sample.abs() > 0.000_01));

        decoder.seek(Duration::from_secs(10)).unwrap();
        assert_eq!(decoder.render(&mut pcm).expect("render GSF at end"), 0);
    }

    #[test]
    fn minigsf_resolves_its_relative_library_and_outer_tags() {
        let fixture = tempfile::tempdir().unwrap();
        let library = fixture.path().join("fixture.gsflib");
        let mini = fixture.path().join("selection.minigsf");
        std::fs::write(
            &library,
            test_gsf_bytes(Some(&test_gba_rom()), "title=library title\n"),
        )
        .unwrap();
        std::fs::write(
            &mini,
            test_gsf_bytes(
                None,
                "_lib=fixture.gsflib\ntitle=Mini selection\nlength=0:00.250\n",
            ),
        )
        .unwrap();

        let decoder = Gsf::open(&mini, GSF_DEFAULT_LENGTH, GSF_DEFAULT_FADE)
            .expect("open minigsf library chain");
        assert_eq!(decoder.duration(), Duration::from_millis(250));
        assert_eq!(decoder.metadata().title.as_deref(), Some("Mini selection"));
    }

    #[test]
    fn missing_length_uses_cogs_150_second_default_and_eight_second_fade() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("untimed.gsf");
        std::fs::write(
            &path,
            test_gsf_bytes(Some(&test_gba_rom()), "title=Untimed fixture\n"),
        )
        .unwrap();

        let decoder =
            Gsf::open(&path, GSF_DEFAULT_LENGTH, GSF_DEFAULT_FADE).expect("open untimed GSF");
        assert_eq!(decoder.duration(), Duration::from_secs(158));
    }

    #[test]
    fn malformed_program_and_missing_library_are_rejected() {
        let fixture = tempfile::tempdir().unwrap();
        let malformed_path = fixture.path().join("malformed.gsf");
        let mut bad_program = Vec::new();
        bad_program.extend_from_slice(&0_u32.to_le_bytes());
        bad_program.extend_from_slice(&0_u32.to_le_bytes());
        bad_program.extend_from_slice(&u32::MAX.to_le_bytes());
        bad_program.extend_from_slice(&[0; 4]);
        let malformed = test_raw_gsf_bytes(&bad_program, "");
        std::fs::write(&malformed_path, malformed).unwrap();
        assert!(Gsf::open(&malformed_path, GSF_DEFAULT_LENGTH, GSF_DEFAULT_FADE).is_err());

        let missing_path = fixture.path().join("missing.minigsf");
        std::fs::write(
            &missing_path,
            test_gsf_bytes(None, "_lib=does-not-exist.gsflib\n"),
        )
        .unwrap();
        assert!(Gsf::open(&missing_path, GSF_DEFAULT_LENGTH, GSF_DEFAULT_FADE).is_err());
    }
}
