use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::psf::Psf;

const PSF_EXTENSIONS: &[&str] = &[
    "psf", "minipsf", "psf2", "minipsf2", "snsf", "minisnsf", "2sf", "mini2sf",
];
const PSF_DEFAULT_LENGTH: Duration = Duration::from_secs(150);
const PSF_DEFAULT_FADE: Duration = Duration::from_secs(8);
const PSF_RENDER_FRAMES: usize = 2_048;

pub struct PsfBackend;

impl PsfBackend {
    fn open(source: &PlaybackSource) -> Result<Psf, String> {
        Psf::open(&source.path, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE)
    }
}

impl DecoderBackend for PsfBackend {
    fn id(&self) -> &'static str {
        "psf-family"
    }

    fn display_name(&self) -> &'static str {
        "libupse + Play! + libsnsf9x + melonDS / xSF family"
    }

    fn extensions(&self) -> &'static [&'static str] {
        PSF_EXTENSIONS
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
            codec: Some(match decoder.format_version() {
                2 => "PlayStation 2 Sound Format (PSF2) / Play! helper".to_owned(),
                0x23 => "Super Nintendo Sound Format (SNSF) / libsnsf9x helper".to_owned(),
                0x24 => "Nintendo DS Sound Format (2SF) / melonDS helper".to_owned(),
                _ => "PlayStation Sound Format (PSF) / libupse helper".to_owned(),
            }),
            bits_per_sample: Some(16),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(PsfSource::new(Self::open(source)?));
        Ok(())
    }
}

fn tag_year(value: &str) -> Option<u32> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| part.len() == 4)
        .and_then(|part| part.parse().ok())
}

struct PsfSource {
    decoder: Psf,
    duration: Duration,
    channels: u16,
    sample_rate: u32,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl PsfSource {
    fn new(decoder: Psf) -> Self {
        let duration = decoder.duration();
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();
        Self {
            decoder,
            duration,
            channels,
            sample_rate,
            pcm: vec![0.0; PSF_RENDER_FRAMES * usize::from(channels)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(self.channels),
            Err(error) => {
                eprintln!("Kog PSF playback error: {error}");
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

impl Iterator for PsfSource {
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

impl Source for PsfSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(self.channels).expect("PSF channel count is nonzero")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.sample_rate).expect("PSF sample rate is nonzero")
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
    use crate::psf::{
        test_psf_bytes, test_psf_executable, test_psf_out_of_bounds_executable, test_psf2_bytes,
        test_psf2_irx, test_snsf_bytes, test_snsf_rom, test_twosf_bytes, test_twosf_rom,
    };

    fn fixture_tags(title: &str) -> String {
        format!(
            "title={title}\nartist=Kog tests\ngame=Synthetic PlayStation sound program\ngenre=Chiptune\ndate=2026-08-31\nlength=0:00.500\nfade=0:00.100\n"
        )
    }

    fn assert_duration_within_one_frame(actual: Option<Duration>, expected: Duration) {
        let actual = actual.expect("PSF duration");
        let frame = Duration::from_nanos(1_000_000_000 / 32_000 + 1);
        assert!(
            actual.abs_diff(expected) <= frame,
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn registry_routes_and_probes_generated_psf() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("fixture.psf");
        std::fs::write(
            &path,
            test_psf_bytes(Some(&test_psf_executable()), &fixture_tags("Synthetic PSF")),
        )
        .unwrap();

        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("psf-family"));
        let source = PlaybackSource::from_path(path);
        let properties = registry.probe(&source).expect("probe generated PSF");
        assert_duration_within_one_frame(properties.duration, Duration::from_millis(600));
        assert_eq!(properties.sample_rate, Some(44_100));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.title.as_deref(), Some("Synthetic PSF"));
        assert_eq!(properties.artist.as_deref(), Some("Kog tests"));
        assert_eq!(
            properties.album.as_deref(),
            Some("Synthetic PlayStation sound program")
        );
        assert_eq!(properties.genre.as_deref(), Some("Chiptune"));
        assert_eq!(properties.year, Some(2026));
        assert_eq!(
            properties.codec.as_deref(),
            Some("PlayStation Sound Format (PSF) / libupse helper")
        );
        assert_eq!(properties.bits_per_sample, Some(16));
    }

    #[test]
    fn generated_psf_renders_seeks_and_ends_exactly() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("fixture.psf");
        std::fs::write(
            &path,
            test_psf_bytes(Some(&test_psf_executable()), &fixture_tags("Synthetic PSF")),
        )
        .unwrap();
        let mut decoder =
            Psf::open(&path, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).expect("open generated PSF");

        let mut pcm = vec![0.0; 512 * 2];
        assert_eq!(decoder.render(&mut pcm).expect("render PSF"), 512);
        assert!(
            pcm.iter().any(|sample| sample.abs() > 0.000_01),
            "generated PSF was silent"
        );

        assert_eq!(
            decoder.seek(Duration::from_millis(250)).unwrap(),
            Duration::from_millis(250)
        );
        pcm.fill(0.0);
        assert_eq!(
            decoder.render(&mut pcm).expect("render after PSF seek"),
            512
        );
        assert!(
            pcm.iter().any(|sample| sample.abs() > 0.000_01),
            "generated PSF was silent after seek"
        );

        decoder.seek(Duration::from_millis(550)).unwrap();
        pcm.fill(0.0);
        assert_eq!(
            decoder.render(&mut pcm).expect("render during PSF fade"),
            512
        );
        assert!(
            pcm.iter().all(|sample| sample.abs() <= 0.51),
            "generated PSF did not apply its tagged fade"
        );

        decoder.seek(Duration::from_secs(10)).unwrap();
        assert_eq!(decoder.render(&mut pcm).expect("render PSF at end"), 0);
    }

    #[test]
    fn generated_psf2_routes_renders_seeks_and_ends_exactly() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("fixture.psf2");
        let irx = test_psf2_irx();
        std::fs::write(
            &path,
            test_psf2_bytes(&[("psf2.irx", &irx)], &fixture_tags("Synthetic PSF2")),
        )
        .unwrap();

        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("psf-family"));
        let source = PlaybackSource::from_path(path.clone());
        let properties = registry.probe(&source).expect("probe generated PSF2");
        assert_duration_within_one_frame(properties.duration, Duration::from_millis(600));
        assert_eq!(properties.sample_rate, Some(44_100));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.title.as_deref(), Some("Synthetic PSF2"));
        assert_eq!(
            properties.codec.as_deref(),
            Some("PlayStation 2 Sound Format (PSF2) / Play! helper")
        );

        let mut decoder =
            Psf::open(&path, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).expect("open generated PSF2");
        let mut pcm = vec![0.0; 512 * 2];
        assert_eq!(decoder.render(&mut pcm).expect("render PSF2"), 512);
        assert!(
            pcm.iter().any(|sample| sample.abs() > 0.000_01),
            "generated PSF2 was silent"
        );
        assert_eq!(
            decoder.seek(Duration::from_millis(250)).unwrap(),
            Duration::from_millis(250)
        );
        pcm.fill(0.0);
        assert_eq!(
            decoder.render(&mut pcm).expect("render after PSF2 seek"),
            512
        );
        assert!(
            pcm.iter().any(|sample| sample.abs() > 0.000_01),
            "generated PSF2 was silent after seek"
        );
        decoder.seek(Duration::from_secs(10)).unwrap();
        assert_eq!(decoder.render(&mut pcm).expect("render PSF2 at end"), 0);
    }

    #[test]
    fn minipsf2_resolves_relative_library_and_outer_tags() {
        let fixture = tempfile::tempdir().unwrap();
        let library = fixture.path().join("music.psflib2");
        let mini = fixture.path().join("selection.minipsf2");
        let irx = test_psf2_irx();
        std::fs::write(
            &library,
            test_psf2_bytes(&[("psf2.irx", &irx)], "title=Library title\n"),
        )
        .unwrap();
        std::fs::write(
            &mini,
            test_psf2_bytes(
                &[],
                "_lib=music.psflib2\ntitle=Mini PSF2 selection\nlength=0:00.250\n",
            ),
        )
        .unwrap();

        let decoder = Psf::open(&mini, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE)
            .expect("open miniPSF2 library chain");
        assert_duration_within_one_frame(Some(decoder.duration()), Duration::from_millis(250));
        assert_eq!(
            decoder.metadata().title.as_deref(),
            Some("Mini PSF2 selection")
        );
    }

    #[test]
    fn generated_snsf_routes_renders_seeks_and_ends_exactly() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("fixture.snsf");
        std::fs::write(
            &path,
            test_snsf_bytes(
                0,
                &test_snsf_rom(),
                &[],
                "title=Synthetic SNSF\nartist=Kog tests\ngame=Synthetic Super NES sound program\ngenre=Chiptune\ndate=2026-08-31\nlength=0:00.500\nfade=0:00.100\n",
            ),
        )
        .unwrap();

        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("psf-family"));
        let source = PlaybackSource::from_path(path.clone());
        let properties = registry.probe(&source).expect("probe generated SNSF");
        assert_duration_within_one_frame(properties.duration, Duration::from_millis(600));
        assert_eq!(properties.sample_rate, Some(32_000));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.title.as_deref(), Some("Synthetic SNSF"));
        assert_eq!(properties.artist.as_deref(), Some("Kog tests"));
        assert_eq!(
            properties.album.as_deref(),
            Some("Synthetic Super NES sound program")
        );
        assert_eq!(
            properties.codec.as_deref(),
            Some("Super Nintendo Sound Format (SNSF) / libsnsf9x helper")
        );

        let mut decoder =
            Psf::open(&path, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).expect("open generated SNSF");
        let mut pcm = vec![0.0; 512 * 2];
        assert_eq!(decoder.render(&mut pcm).expect("render SNSF"), 512);
        assert!(
            pcm.iter().any(|sample| sample.abs() > 0.000_01),
            "generated SNSF was silent"
        );
        assert_eq!(
            decoder.seek(Duration::from_millis(250)).unwrap(),
            Duration::from_millis(250)
        );
        pcm.fill(0.0);
        assert_eq!(
            decoder.render(&mut pcm).expect("render after SNSF seek"),
            512
        );
        assert!(
            pcm.iter().any(|sample| sample.abs() > 0.000_01),
            "generated SNSF was silent after seek"
        );
        decoder.seek(Duration::from_secs(10)).unwrap();
        assert_eq!(decoder.render(&mut pcm).expect("render SNSF at end"), 0);
    }

    #[test]
    fn minisnsf_resolves_relative_library_and_outer_tags() {
        let fixture = tempfile::tempdir().unwrap();
        let library = fixture.path().join("music.snsflib");
        let mini = fixture.path().join("selection.minisnsf");
        std::fs::write(
            &library,
            test_snsf_bytes(0, &test_snsf_rom(), &[], "title=Library title\n"),
        )
        .unwrap();
        std::fs::write(
            &mini,
            test_snsf_bytes(
                0,
                &[],
                &[],
                "_lib=music.snsflib\ntitle=Mini SNSF selection\nlength=0:00.250\n",
            ),
        )
        .unwrap();

        let decoder = Psf::open(&mini, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE)
            .expect("open miniSNSF library chain");
        assert_duration_within_one_frame(Some(decoder.duration()), Duration::from_millis(250));
        assert_eq!(
            decoder.metadata().title.as_deref(),
            Some("Mini SNSF selection")
        );
    }

    #[test]
    fn snsf_defaults_and_malformed_inputs_are_rejected() {
        let fixture = tempfile::tempdir().unwrap();

        let untimed = fixture.path().join("untimed.snsf");
        std::fs::write(
            &untimed,
            test_snsf_bytes(0, &test_snsf_rom(), &[], "title=Untimed SNSF fixture\n"),
        )
        .unwrap();
        let decoder =
            Psf::open(&untimed, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).expect("open untimed SNSF");
        assert_duration_within_one_frame(Some(decoder.duration()), Duration::from_secs(158));

        let short_rom = fixture.path().join("short.snsf");
        std::fs::write(
            &short_rom,
            test_snsf_bytes(0, &[0_u8; 4], &[], "length=0:00.010\n"),
        )
        .unwrap();
        assert!(Psf::open(&short_rom, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let oversized_mapping = fixture.path().join("oversized.snsf");
        std::fs::write(
            &oversized_mapping,
            test_snsf_bytes(u32::MAX, &[1_u8; 4], &[], ""),
        )
        .unwrap();
        assert!(Psf::open(&oversized_mapping, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let malformed_reserved = fixture.path().join("malformed-reserved.snsf");
        let reserved = [0_u8, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0];
        std::fs::write(
            &malformed_reserved,
            test_snsf_bytes(0, &test_snsf_rom(), &reserved, ""),
        )
        .unwrap();
        assert!(Psf::open(&malformed_reserved, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let missing = fixture.path().join("missing.minisnsf");
        std::fs::write(
            &missing,
            test_snsf_bytes(0, &[], &[], "_lib=does-not-exist.snsflib\n"),
        )
        .unwrap();
        assert!(Psf::open(&missing, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let cycle_a = fixture.path().join("cycle-a.minisnsf");
        let cycle_b = fixture.path().join("cycle-b.snsflib");
        std::fs::write(
            &cycle_a,
            test_snsf_bytes(0, &[], &[], "_lib=cycle-b.snsflib\n"),
        )
        .unwrap();
        std::fs::write(
            &cycle_b,
            test_snsf_bytes(0, &[], &[], "_lib=cycle-a.minisnsf\n"),
        )
        .unwrap();
        assert!(Psf::open(&cycle_a, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let nested = fixture.path().join("nested");
        std::fs::create_dir(&nested).unwrap();
        let outside = fixture.path().join("outside.snsflib");
        std::fs::write(&outside, test_snsf_bytes(0, &test_snsf_rom(), &[], "")).unwrap();
        let traversal = nested.join("traversal.minisnsf");
        std::fs::write(
            &traversal,
            test_snsf_bytes(0, &[], &[], "_lib=../outside.snsflib\n"),
        )
        .unwrap();
        assert!(Psf::open(&traversal, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());
    }

    #[test]
    fn generated_twosf_routes_renders_seeks_and_ends_exactly() {
        let fixture = tempfile::tempdir().unwrap();
        let path = fixture.path().join("fixture.2sf");
        std::fs::write(
            &path,
            test_twosf_bytes(
                0,
                &test_twosf_rom(),
                "title=Synthetic 2SF\nartist=Kog tests\ngame=Synthetic Nintendo DS sound program\ngenre=Chiptune\ndate=2026-08-31\nlength=0:00.500\nfade=0:00.100\n",
            ),
        )
        .unwrap();

        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("psf-family"));
        let source = PlaybackSource::from_path(path.clone());
        let properties = registry.probe(&source).expect("probe generated 2SF");
        assert_duration_within_one_frame(properties.duration, Duration::from_millis(600));
        assert_eq!(properties.sample_rate, Some(32_728));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.title.as_deref(), Some("Synthetic 2SF"));
        assert_eq!(properties.artist.as_deref(), Some("Kog tests"));
        assert_eq!(
            properties.album.as_deref(),
            Some("Synthetic Nintendo DS sound program")
        );
        assert_eq!(
            properties.codec.as_deref(),
            Some("Nintendo DS Sound Format (2SF) / melonDS helper")
        );

        let mut decoder =
            Psf::open(&path, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).expect("open generated 2SF");
        let mut pcm = vec![0.0; 512 * 2];
        assert_eq!(decoder.render(&mut pcm).expect("render 2SF"), 512);
        assert!(
            pcm.iter().any(|sample| sample.abs() > 0.000_01),
            "generated 2SF was silent"
        );
        assert_eq!(
            decoder.seek(Duration::from_millis(250)).unwrap(),
            Duration::from_millis(250)
        );
        pcm.fill(0.0);
        assert_eq!(
            decoder.render(&mut pcm).expect("render after 2SF seek"),
            512
        );
        assert!(
            pcm.iter().any(|sample| sample.abs() > 0.000_01),
            "generated 2SF was silent after seek"
        );
        decoder.seek(Duration::from_secs(10)).unwrap();
        assert_eq!(decoder.render(&mut pcm).expect("render 2SF at end"), 0);
    }

    #[test]
    fn mini2sf_resolves_relative_library_and_outer_tags() {
        let fixture = tempfile::tempdir().unwrap();
        let library = fixture.path().join("music.2sflib");
        let mini = fixture.path().join("selection.mini2sf");
        std::fs::write(
            &library,
            test_twosf_bytes(0, &test_twosf_rom(), "title=Library title\n"),
        )
        .unwrap();
        std::fs::write(
            &mini,
            test_twosf_bytes(
                0,
                &[],
                "_lib=music.2sflib\ntitle=Mini 2SF selection\nlength=0:00.250\n",
            ),
        )
        .unwrap();

        let decoder = Psf::open(&mini, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE)
            .expect("open mini2SF library chain");
        assert_duration_within_one_frame(Some(decoder.duration()), Duration::from_millis(250));
        assert_eq!(
            decoder.metadata().title.as_deref(),
            Some("Mini 2SF selection")
        );
    }

    #[test]
    fn twosf_defaults_and_malformed_inputs_are_rejected() {
        let fixture = tempfile::tempdir().unwrap();

        let untimed = fixture.path().join("untimed.2sf");
        std::fs::write(
            &untimed,
            test_twosf_bytes(0, &test_twosf_rom(), "title=Untimed 2SF fixture\n"),
        )
        .unwrap();
        let decoder =
            Psf::open(&untimed, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).expect("open untimed 2SF");
        assert_duration_within_one_frame(Some(decoder.duration()), Duration::from_secs(158));

        let short_rom = fixture.path().join("short.2sf");
        std::fs::write(&short_rom, test_twosf_bytes(0, &[0_u8; 4], "")).unwrap();
        assert!(Psf::open(&short_rom, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let oversized_mapping = fixture.path().join("oversized.2sf");
        std::fs::write(
            &oversized_mapping,
            test_twosf_bytes(u32::MAX, &[1_u8; 4], ""),
        )
        .unwrap();
        assert!(Psf::open(&oversized_mapping, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let invalid_header = fixture.path().join("invalid-header.2sf");
        let mut rom = test_twosf_rom();
        rom[0x2c..0x30].copy_from_slice(&u32::MAX.to_le_bytes());
        std::fs::write(&invalid_header, test_twosf_bytes(0, &rom, "")).unwrap();
        assert!(Psf::open(&invalid_header, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let missing = fixture.path().join("missing.mini2sf");
        std::fs::write(
            &missing,
            test_twosf_bytes(0, &[], "_lib=does-not-exist.2sflib\n"),
        )
        .unwrap();
        assert!(Psf::open(&missing, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let cycle_a = fixture.path().join("cycle-a.mini2sf");
        let cycle_b = fixture.path().join("cycle-b.2sflib");
        std::fs::write(&cycle_a, test_twosf_bytes(0, &[], "_lib=cycle-b.2sflib\n")).unwrap();
        std::fs::write(&cycle_b, test_twosf_bytes(0, &[], "_lib=cycle-a.mini2sf\n")).unwrap();
        assert!(Psf::open(&cycle_a, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());
    }

    #[test]
    fn psf2_defaults_and_malformed_inputs_are_rejected() {
        let fixture = tempfile::tempdir().unwrap();
        let irx = test_psf2_irx();

        let untimed = fixture.path().join("untimed.psf2");
        std::fs::write(
            &untimed,
            test_psf2_bytes(&[("psf2.irx", &irx)], "title=Untimed PSF2 fixture\n"),
        )
        .unwrap();
        let decoder =
            Psf::open(&untimed, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).expect("open untimed PSF2");
        assert_duration_within_one_frame(Some(decoder.duration()), Duration::from_secs(158));

        let no_root = fixture.path().join("no-root.psf2");
        std::fs::write(&no_root, test_psf2_bytes(&[], "")).unwrap();
        assert!(Psf::open(&no_root, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let malformed_irx = fixture.path().join("malformed-irx.psf2");
        std::fs::write(
            &malformed_irx,
            test_psf2_bytes(&[("psf2.irx", &[0_u8; 52])], ""),
        )
        .unwrap();
        assert!(Psf::open(&malformed_irx, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let malformed_fs = fixture.path().join("malformed-fs.psf2");
        let mut malformed_fs_bytes = test_psf2_bytes(&[("psf2.irx", &irx)], "");
        malformed_fs_bytes[56..60].copy_from_slice(&u32::MAX.to_le_bytes());
        std::fs::write(&malformed_fs, malformed_fs_bytes).unwrap();
        assert!(Psf::open(&malformed_fs, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let missing = fixture.path().join("missing.minipsf2");
        std::fs::write(
            &missing,
            test_psf2_bytes(&[], "_lib=does-not-exist.psflib2\n"),
        )
        .unwrap();
        assert!(Psf::open(&missing, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let cycle_a = fixture.path().join("cycle-a.minipsf2");
        let cycle_b = fixture.path().join("cycle-b.psflib2");
        std::fs::write(&cycle_a, test_psf2_bytes(&[], "_lib=cycle-b.psflib2\n")).unwrap();
        std::fs::write(&cycle_b, test_psf2_bytes(&[], "_lib=cycle-a.minipsf2\n")).unwrap();
        assert!(Psf::open(&cycle_a, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());
    }

    #[test]
    fn minipsf_resolves_relative_library_and_outer_tags() {
        let fixture = tempfile::tempdir().unwrap();
        let library = fixture.path().join("music.psflib");
        let mini = fixture.path().join("selection.minipsf");
        std::fs::write(
            &library,
            test_psf_bytes(Some(&test_psf_executable()), "title=Library title\n"),
        )
        .unwrap();
        std::fs::write(
            &mini,
            test_psf_bytes(
                None,
                "_lib=music.psflib\ntitle=Mini selection\nlength=0:00.250\n",
            ),
        )
        .unwrap();

        let decoder = Psf::open(&mini, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE)
            .expect("open miniPSF library chain");
        assert_duration_within_one_frame(Some(decoder.duration()), Duration::from_millis(250));
        assert_eq!(decoder.metadata().title.as_deref(), Some("Mini selection"));
    }

    #[test]
    fn missing_length_uses_cogs_default_and_malformed_inputs_are_rejected() {
        let fixture = tempfile::tempdir().unwrap();
        let untimed = fixture.path().join("untimed.psf");
        std::fs::write(
            &untimed,
            test_psf_bytes(Some(&test_psf_executable()), "title=Untimed fixture\n"),
        )
        .unwrap();
        let decoder =
            Psf::open(&untimed, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).expect("open untimed PSF");
        assert_duration_within_one_frame(Some(decoder.duration()), Duration::from_secs(158));

        let out_of_bounds = fixture.path().join("out-of-bounds.psf");
        std::fs::write(
            &out_of_bounds,
            test_psf_bytes(Some(&test_psf_out_of_bounds_executable()), ""),
        )
        .unwrap();
        assert!(Psf::open(&out_of_bounds, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let psf2 = fixture.path().join("unsupported.psf");
        let mut psf2_bytes = test_psf_bytes(Some(&test_psf_executable()), "title=Not PSF1\n");
        psf2_bytes[3] = 2;
        std::fs::write(&psf2, psf2_bytes).unwrap();
        assert!(Psf::open(&psf2, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());

        let missing = fixture.path().join("missing.minipsf");
        std::fs::write(
            &missing,
            test_psf_bytes(None, "_lib=does-not-exist.psflib\n"),
        )
        .unwrap();
        assert!(Psf::open(&missing, PSF_DEFAULT_LENGTH, PSF_DEFAULT_FADE).is_err());
    }
}
