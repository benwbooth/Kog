use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::sdsf::Sdsf;

const SDSF_EXTENSIONS: &[&str] = &["ssf", "minissf", "dsf", "minidsf"];
const SDSF_DEFAULT_LENGTH: Duration = Duration::from_secs(150);
const SDSF_DEFAULT_FADE: Duration = Duration::from_secs(8);
const SDSF_RENDER_FRAMES: usize = 2_048;

pub struct SdsfBackend;

impl SdsfBackend {
    fn open(source: &PlaybackSource) -> Result<Sdsf, String> {
        Sdsf::open(&source.path, SDSF_DEFAULT_LENGTH, SDSF_DEFAULT_FADE)
    }
}

impl DecoderBackend for SdsfBackend {
    fn id(&self) -> &'static str {
        "highly-theoretical-sdsf"
    }

    fn display_name(&self) -> &'static str {
        "Highly Theoretical / SSF + DSF"
    }

    fn extensions(&self) -> &'static [&'static str] {
        SDSF_EXTENSIONS
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
            codec: Some(decoder.kind().codec().to_owned()),
            bits_per_sample: Some(16),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(SdsfSource::new(Self::open(source)?));
        Ok(())
    }
}

fn tag_year(value: &str) -> Option<u32> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| part.len() == 4)
        .and_then(|part| part.parse().ok())
}

struct SdsfSource {
    decoder: Sdsf,
    duration: Duration,
    channels: u16,
    sample_rate: u32,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl SdsfSource {
    fn new(decoder: Sdsf) -> Self {
        let duration = decoder.duration();
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();
        Self {
            decoder,
            duration,
            channels,
            sample_rate,
            pcm: vec![0.0; SDSF_RENDER_FRAMES * usize::from(channels)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(self.channels),
            Err(error) => {
                eprintln!("Kog SSF/DSF playback error: {error}");
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

impl Iterator for SdsfSource {
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

impl Source for SdsfSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(self.channels).expect("SSF/DSF channel count is nonzero")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.sample_rate).expect("SSF/DSF sample rate is nonzero")
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
    use crate::sdsf::{
        SdsfKind, test_dsf_program, test_sdsf_bytes, test_sdsf_out_of_bounds_program,
        test_ssf_program,
    };

    fn fixture_tags(title: &str) -> String {
        format!(
            "title={title}\nartist=Kog tests\ngame=Synthetic Sega sound program\ngenre=Chiptune\ndate=2026-08-31\nlength=0:00.500\nfade=0:00.100\n"
        )
    }

    fn assert_duration_within_one_frame(actual: Option<Duration>, expected: Duration) {
        let actual = actual.expect("SSF/DSF duration");
        let frame = Duration::from_nanos(1_000_000_000 / 44_100 + 1);
        assert!(
            actual.abs_diff(expected) <= frame,
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn registry_routes_and_probes_generated_ssf_and_dsf() {
        let fixture = tempfile::tempdir().unwrap();
        for (name, version, program, expected_kind, codec) in [
            (
                "fixture.ssf",
                0x11,
                test_ssf_program(),
                SdsfKind::Ssf,
                "Sega Saturn Sound Format (SSF) / Highly Theoretical",
            ),
            (
                "fixture.dsf",
                0x12,
                test_dsf_program(),
                SdsfKind::Dsf,
                "Dreamcast Sound Format (DSF) / Highly Theoretical",
            ),
        ] {
            let path = fixture.path().join(name);
            std::fs::write(
                &path,
                test_sdsf_bytes(version, Some(&program), &fixture_tags(name)),
            )
            .unwrap();

            let registry = DecoderRegistry::new(DecoderSettings::default());
            assert_eq!(
                registry.backend_id_for(&path),
                Some("highly-theoretical-sdsf")
            );
            let source = PlaybackSource::from_path(path.clone());
            let properties = registry.probe(&source).expect("probe generated SSF/DSF");
            assert_duration_within_one_frame(properties.duration, Duration::from_millis(600));
            assert_eq!(properties.sample_rate, Some(44_100));
            assert_eq!(properties.channels, Some(2));
            assert_eq!(properties.title.as_deref(), Some(name));
            assert_eq!(properties.artist.as_deref(), Some("Kog tests"));
            assert_eq!(
                properties.album.as_deref(),
                Some("Synthetic Sega sound program")
            );
            assert_eq!(properties.genre.as_deref(), Some("Chiptune"));
            assert_eq!(properties.year, Some(2026));
            assert_eq!(properties.codec.as_deref(), Some(codec));
            assert_eq!(properties.bits_per_sample, Some(16));

            let decoder = Sdsf::open(&path, SDSF_DEFAULT_LENGTH, SDSF_DEFAULT_FADE)
                .expect("open generated SSF/DSF");
            assert_eq!(decoder.kind(), expected_kind);
        }
    }

    #[test]
    fn generated_ssf_and_dsf_render_seek_fade_and_end_exactly() {
        let fixture = tempfile::tempdir().unwrap();
        for (name, version, program) in [
            ("fixture.ssf", 0x11, test_ssf_program()),
            ("fixture.dsf", 0x12, test_dsf_program()),
        ] {
            let path = fixture.path().join(name);
            std::fs::write(
                &path,
                test_sdsf_bytes(version, Some(&program), &fixture_tags(name)),
            )
            .unwrap();
            let mut decoder = Sdsf::open(&path, SDSF_DEFAULT_LENGTH, SDSF_DEFAULT_FADE)
                .expect("open generated SSF/DSF");
            let mut pcm = vec![0.0; 512 * 2];
            assert_eq!(decoder.render(&mut pcm).expect("render SSF/DSF"), 512);
            assert!(
                pcm.iter().any(|sample| sample.abs() > 0.000_01),
                "generated {name} was silent before seek"
            );

            assert_eq!(
                decoder.seek(Duration::from_millis(250)).unwrap(),
                Duration::from_millis(250)
            );
            pcm.fill(0.0);
            assert_eq!(
                decoder.render(&mut pcm).expect("render SSF/DSF after seek"),
                512
            );
            assert!(
                pcm.iter().any(|sample| sample.abs() > 0.000_01),
                "generated {name} was silent after seek"
            );

            decoder.seek(Duration::from_secs(10)).unwrap();
            assert_eq!(decoder.render(&mut pcm).expect("render SSF/DSF at end"), 0);
        }
    }

    #[test]
    fn minissf_and_minidsf_resolve_relative_libraries_and_outer_tags() {
        let fixture = tempfile::tempdir().unwrap();
        for (library_name, mini_name, version, program) in [
            (
                "music.ssflib",
                "selection.minissf",
                0x11,
                test_ssf_program(),
            ),
            (
                "music.dsflib",
                "selection.minidsf",
                0x12,
                test_dsf_program(),
            ),
        ] {
            let library = fixture.path().join(library_name);
            let mini = fixture.path().join(mini_name);
            std::fs::write(
                &library,
                test_sdsf_bytes(version, Some(&program), "title=Library title\n"),
            )
            .unwrap();
            std::fs::write(
                &mini,
                test_sdsf_bytes(
                    version,
                    None,
                    &format!("_lib={library_name}\ntitle=Mini selection\nlength=0:00.250\n"),
                ),
            )
            .unwrap();

            let decoder = Sdsf::open(&mini, SDSF_DEFAULT_LENGTH, SDSF_DEFAULT_FADE)
                .expect("open mini SSF/DSF library chain");
            assert_duration_within_one_frame(Some(decoder.duration()), Duration::from_millis(250));
            assert_eq!(decoder.metadata().title.as_deref(), Some("Mini selection"));
        }
    }

    #[test]
    fn missing_length_uses_cogs_default_and_malformed_inputs_are_rejected() {
        let fixture = tempfile::tempdir().unwrap();
        let untimed = fixture.path().join("untimed.ssf");
        std::fs::write(
            &untimed,
            test_sdsf_bytes(0x11, Some(&test_ssf_program()), "title=Untimed fixture\n"),
        )
        .unwrap();
        let decoder =
            Sdsf::open(&untimed, SDSF_DEFAULT_LENGTH, SDSF_DEFAULT_FADE).expect("open untimed SSF");
        assert_duration_within_one_frame(Some(decoder.duration()), Duration::from_secs(158));

        for (name, version) in [("bad.ssf", 0x11), ("bad.dsf", 0x12)] {
            let path = fixture.path().join(name);
            std::fs::write(
                &path,
                test_sdsf_bytes(version, Some(&test_sdsf_out_of_bounds_program(version)), ""),
            )
            .unwrap();
            assert!(Sdsf::open(&path, SDSF_DEFAULT_LENGTH, SDSF_DEFAULT_FADE).is_err());
        }

        let missing = fixture.path().join("missing.minissf");
        std::fs::write(
            &missing,
            test_sdsf_bytes(0x11, None, "_lib=does-not-exist.ssflib\n"),
        )
        .unwrap();
        assert!(Sdsf::open(&missing, SDSF_DEFAULT_LENGTH, SDSF_DEFAULT_FADE).is_err());
    }
}
