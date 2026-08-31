use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::usf::Usf;

const USF_EXTENSIONS: &[&str] = &["usf", "miniusf"];
const USF_DEFAULT_LENGTH: Duration = Duration::from_secs(150);
const USF_DEFAULT_FADE: Duration = Duration::from_secs(8);
const USF_RENDER_FRAMES: usize = 2_048;

pub struct UsfBackend;

impl UsfBackend {
    fn open(source: &PlaybackSource) -> Result<Usf, String> {
        Usf::open(&source.path, USF_DEFAULT_LENGTH, USF_DEFAULT_FADE)
    }
}

impl DecoderBackend for UsfBackend {
    fn id(&self) -> &'static str {
        "lazyusf2-usf"
    }

    fn display_name(&self) -> &'static str {
        "LazyUSF2 / USF"
    }

    fn extensions(&self) -> &'static [&'static str] {
        USF_EXTENSIONS
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
            codec: Some("Nintendo 64 Sound Format (USF) / LazyUSF2".to_owned()),
            bits_per_sample: Some(16),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(UsfSource::new(Self::open(source)?));
        Ok(())
    }
}

fn tag_year(value: &str) -> Option<u32> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| part.len() == 4)
        .and_then(|part| part.parse().ok())
}

struct UsfSource {
    decoder: Usf,
    duration: Duration,
    channels: u16,
    sample_rate: u32,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl UsfSource {
    fn new(decoder: Usf) -> Self {
        let duration = decoder.duration();
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();
        Self {
            decoder,
            duration,
            channels,
            sample_rate,
            pcm: vec![0.0; USF_RENDER_FRAMES * usize::from(channels)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(self.channels),
            Err(error) => {
                eprintln!("Kog USF playback error: {error}");
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

impl Iterator for UsfSource {
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

impl Source for UsfSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(self.channels).expect("USF channel count is nonzero")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.sample_rate).expect("USF sample rate is nonzero")
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
    use crate::usf::{test_usf_bytes, test_usf_out_of_bounds_reserved, test_usf_reserved};

    fn fixture_tags(title: &str) -> String {
        format!(
            "title={title}\nartist=Kog tests\ngame=Synthetic Nintendo 64 sound program\ngenre=Chiptune\ndate=2026-08-31\nlength=0:00.500\nfade=0:00.100\n"
        )
    }

    fn assert_duration_within_one_frame(actual: Option<Duration>, expected: Duration) {
        let actual = actual.expect("USF duration");
        let frame = Duration::from_nanos(1_000_000_000 / 44_100 + 1);
        assert!(
            actual.abs_diff(expected) <= frame,
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn registry_routes_and_probes_generated_usf() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("fixture.usf");
        std::fs::write(
            &path,
            test_usf_bytes(Some(&test_usf_reserved()), &fixture_tags("Synthetic USF")),
        )
        .unwrap();

        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("lazyusf2-usf"));
        let source = PlaybackSource::from_path(path);
        let properties = registry.probe(&source).expect("probe generated USF");
        assert_duration_within_one_frame(properties.duration, Duration::from_millis(600));
        assert_eq!(properties.sample_rate, Some(44_100));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.title.as_deref(), Some("Synthetic USF"));
        assert_eq!(properties.artist.as_deref(), Some("Kog tests"));
        assert_eq!(
            properties.album.as_deref(),
            Some("Synthetic Nintendo 64 sound program")
        );
        assert_eq!(properties.genre.as_deref(), Some("Chiptune"));
        assert_eq!(properties.year, Some(2026));
        assert_eq!(
            properties.codec.as_deref(),
            Some("Nintendo 64 Sound Format (USF) / LazyUSF2")
        );
        assert_eq!(properties.bits_per_sample, Some(16));
    }

    #[test]
    fn generated_usf_renders_seeks_and_ends_exactly() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("fixture.usf");
        std::fs::write(
            &path,
            test_usf_bytes(Some(&test_usf_reserved()), &fixture_tags("Synthetic USF")),
        )
        .unwrap();
        let mut decoder =
            Usf::open(&path, USF_DEFAULT_LENGTH, USF_DEFAULT_FADE).expect("open generated USF");

        let mut pcm = vec![0.0; 512 * 2];
        assert_eq!(decoder.render(&mut pcm).expect("render USF"), 512);
        assert!(
            pcm.iter().any(|sample| sample.abs() > 0.000_01),
            "generated USF was silent"
        );

        assert_eq!(
            decoder.seek(Duration::from_millis(250)).unwrap(),
            Duration::from_millis(250)
        );
        pcm.fill(0.0);
        assert_eq!(
            decoder.render(&mut pcm).expect("render after USF seek"),
            512
        );
        assert!(
            pcm.iter().any(|sample| sample.abs() > 0.000_01),
            "generated USF was silent after seek"
        );

        decoder.seek(Duration::from_millis(550)).unwrap();
        pcm.fill(0.0);
        assert_eq!(
            decoder.render(&mut pcm).expect("render during USF fade"),
            512
        );
        assert!(
            pcm.iter().all(|sample| sample.abs() <= 0.51),
            "generated USF did not apply its tagged fade"
        );

        decoder.seek(Duration::from_secs(10)).unwrap();
        assert_eq!(decoder.render(&mut pcm).expect("render USF at end"), 0);
    }

    #[test]
    fn miniusf_resolves_relative_library_and_outer_tags() {
        let fixture = tempfile::tempdir().unwrap();
        let library = fixture.path().join("music.usflib");
        let mini = fixture.path().join("selection.miniusf");
        std::fs::write(
            &library,
            test_usf_bytes(Some(&test_usf_reserved()), "title=Library title\n"),
        )
        .unwrap();
        std::fs::write(
            &mini,
            test_usf_bytes(
                None,
                "_lib=music.usflib\ntitle=Mini selection\nlength=0:00.250\n",
            ),
        )
        .unwrap();

        let decoder = Usf::open(&mini, USF_DEFAULT_LENGTH, USF_DEFAULT_FADE)
            .expect("open miniUSF library chain");
        assert_duration_within_one_frame(Some(decoder.duration()), Duration::from_millis(250));
        assert_eq!(decoder.metadata().title.as_deref(), Some("Mini selection"));
    }

    #[test]
    fn missing_length_uses_cogs_default_and_malformed_inputs_are_rejected() {
        let fixture = tempfile::tempdir().unwrap();
        let untimed = fixture.path().join("untimed.usf");
        std::fs::write(
            &untimed,
            test_usf_bytes(Some(&test_usf_reserved()), "title=Untimed fixture\n"),
        )
        .unwrap();
        let decoder =
            Usf::open(&untimed, USF_DEFAULT_LENGTH, USF_DEFAULT_FADE).expect("open untimed USF");
        assert_duration_within_one_frame(Some(decoder.duration()), Duration::from_secs(158));

        let out_of_bounds = fixture.path().join("out-of-bounds.usf");
        std::fs::write(
            &out_of_bounds,
            test_usf_bytes(Some(&test_usf_out_of_bounds_reserved()), ""),
        )
        .unwrap();
        assert!(Usf::open(&out_of_bounds, USF_DEFAULT_LENGTH, USF_DEFAULT_FADE).is_err());

        let missing = fixture.path().join("missing.miniusf");
        std::fs::write(
            &missing,
            test_usf_bytes(None, "_lib=does-not-exist.usflib\n"),
        )
        .unwrap();
        assert!(Usf::open(&missing, USF_DEFAULT_LENGTH, USF_DEFAULT_FADE).is_err());
    }
}
