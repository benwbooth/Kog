use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::libvgm::LibVgm;

const LIBVGM_EXTENSIONS: &[&str] = &["vgm", "vgz", "s98", "dro", "gym"];
const LIBVGM_SAMPLE_RATE: u32 = 44_100;
const LIBVGM_CHANNELS: u16 = 2;
const LIBVGM_BITS_PER_SAMPLE: u8 = 24;
const LIBVGM_RENDER_FRAMES: usize = 2_048;
const LIBVGM_LOOP_COUNT: u32 = 2;
const LIBVGM_FADE: Duration = Duration::from_secs(8);
const LIBVGM_END_SILENCE: Duration = Duration::from_millis(500);

pub struct LibVgmBackend;

impl LibVgmBackend {
    fn open(source: &PlaybackSource) -> Result<LibVgm, String> {
        LibVgm::open(
            &source.path,
            LIBVGM_SAMPLE_RATE,
            LIBVGM_LOOP_COUNT,
            LIBVGM_FADE,
            LIBVGM_END_SILENCE,
        )
    }
}

impl DecoderBackend for LibVgmBackend {
    fn id(&self) -> &'static str {
        "libvgm"
    }

    fn display_name(&self) -> &'static str {
        "libvgm (Cog pin 867223e)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        LIBVGM_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            loop_metadata: true,
            companion_files: true,
            ..DecoderCapabilities::default()
        }
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let decoder = Self::open(source)?;
        let metadata = decoder.metadata();
        Ok(StreamProperties {
            duration: Some(decoder.total_duration()),
            sample_rate: Some(LIBVGM_SAMPLE_RATE),
            channels: Some(LIBVGM_CHANNELS),
            title: nonempty(&metadata.title),
            artist: nonempty(&metadata.artist),
            album: nonempty(&metadata.album),
            year: parse_year(&metadata.date),
            codec: nonempty(&metadata.codec),
            bits_per_sample: Some(LIBVGM_BITS_PER_SAMPLE),
            warning: decoder.warning().map(str::to_owned),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(LibVgmSource::new(Self::open(source)?));
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
        .find(|digits| digits.iter().all(u8::is_ascii_digit))
        .and_then(|digits| std::str::from_utf8(digits).ok())
        .and_then(|digits| digits.parse().ok())
}

struct LibVgmSource {
    decoder: LibVgm,
    duration: Duration,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl LibVgmSource {
    fn new(decoder: LibVgm) -> Self {
        let duration = decoder.total_duration();
        Self {
            decoder,
            duration,
            pcm: vec![0.0; LIBVGM_RENDER_FRAMES * usize::from(LIBVGM_CHANNELS)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(LIBVGM_CHANNELS),
            Err(error) => {
                eprintln!("Kog libvgm playback error: {error}");
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

impl Iterator for LibVgmSource {
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

impl Source for LibVgmSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(LIBVGM_CHANNELS).expect("libvgm output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(LIBVGM_SAMPLE_RATE).expect("libvgm sample rate is nonzero")
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
            "kog-libvgm-backend-{}-{test_name}.vgm",
            std::process::id()
        ));
        std::fs::write(&path, crate::libvgm::test_vgm_bytes()).expect("write VGM fixture");
        path
    }

    #[test]
    fn registry_routes_and_probes_vgm() {
        let path = fixture_path("probe");
        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("libvgm"));

        let properties = registry
            .probe(&PlaybackSource::from_path(path.clone()))
            .expect("probe VGM");
        assert_eq!(properties.duration, Some(Duration::from_secs(1)));
        assert_eq!(properties.sample_rate, Some(44_100));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.bits_per_sample, Some(24));
        assert_eq!(properties.codec.as_deref(), Some("VGM v1.50"));

        let track =
            crate::track::Track::from_source(PlaybackSource::from_path(path.clone()), &registry);
        assert_eq!(track.duration, Some(Duration::from_secs(1)));
        assert_eq!(track.sample_rate, Some(44_100));
        assert_eq!(track.bits_per_sample, Some(24));
        assert_eq!(track.codec, "VGM v1.50");

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn source_renders_non_silent_pcm_and_seeks() {
        let path = fixture_path("source");
        let decoder = LibVgmBackend::open(&PlaybackSource::from_path(path.clone()))
            .expect("open VGM fixture");
        let mut source = LibVgmSource::new(decoder);
        assert!(
            source
                .by_ref()
                .take(4_410 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "rendered VGM PCM was silent"
        );
        source
            .try_seek(Duration::from_millis(500))
            .expect("seek VGM source");
        assert!(
            source
                .by_ref()
                .take(2_205 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "rendered VGM PCM was silent after seeking"
        );

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn year_parser_accepts_common_libvgm_date_tags() {
        assert_eq!(parse_year("1994"), Some(1994));
        assert_eq!(parse_year("1994/12/03"), Some(1994));
        assert_eq!(parse_year("released 2001"), Some(2001));
        assert_eq!(parse_year("unknown"), None);
    }
}
