use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::openmpt::OpenMpt;

const OPENMPT_EXTENSIONS: &[&str] = &[
    "mptm", "mod", "s3m", "xm", "it", "667", "669", "amf", "ams", "c67", "cba", "dbm", "digi",
    "dmf", "dsm", "dsym", "dtm", "etx", "far", "fc", "fc13", "fc14", "fmt", "fst", "ftm", "imf",
    "ims", "ice", "j2b", "m15", "mdl", "med", "mms", "mt2", "mtm", "mus", "nst", "okt", "plm",
    "psm", "pt36", "ptm", "puma", "rtm", "sfx", "sfx2", "smod", "st26", "stk", "stm", "stx", "stp",
    "symmod", "tcb", "gmc", "gtk", "gt2", "ult", "unic", "wow", "xmf", "gdm", "mo3", "oxm", "umx",
    "xpk", "ppm", "mmcmp",
];
const OPENMPT_SAMPLE_RATE: u32 = 44_100;
const OPENMPT_CHANNELS: u16 = 2;
const OPENMPT_BITS_PER_SAMPLE: u8 = 32;
const OPENMPT_RENDER_FRAMES: usize = 1_024;

pub struct OpenMptBackend;

impl OpenMptBackend {
    fn open(source: &PlaybackSource) -> Result<OpenMpt, String> {
        OpenMpt::open(&source.path, OPENMPT_SAMPLE_RATE, source.subsong)
    }
}

impl DecoderBackend for OpenMptBackend {
    fn id(&self) -> &'static str {
        "libopenmpt"
    }

    fn display_name(&self) -> &'static str {
        "libopenmpt 0.8.7 (Cog pin)"
    }

    fn extensions(&self) -> &'static [&'static str] {
        OPENMPT_EXTENSIONS
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
            .is_some_and(OpenMpt::supports_extension)
    }

    fn subsong_count(&self, path: &Path) -> Result<Option<u32>, String> {
        let decoder = OpenMpt::open(path, OPENMPT_SAMPLE_RATE, Some(0))?;
        Ok(Some(decoder.subsong_count()))
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let decoder = Self::open(source)?;
        let metadata = decoder.metadata();
        Ok(StreamProperties {
            duration: Some(decoder.duration()),
            sample_rate: Some(OPENMPT_SAMPLE_RATE),
            channels: Some(OPENMPT_CHANNELS),
            title: owned(metadata.get("title")),
            artist: owned(metadata.get("artist")),
            year: metadata.get("date").and_then(parse_year),
            track_number: Some(decoder.selected_subsong() + 1),
            codec: owned(metadata.get("type_long")),
            bits_per_sample: Some(OPENMPT_BITS_PER_SAMPLE),
            warning: owned(metadata.get("warnings")),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(OpenMptSource::new(Self::open(source)?));
        Ok(())
    }
}

fn owned(value: Option<&str>) -> Option<String> {
    value.map(str::to_owned)
}

fn parse_year(value: &str) -> Option<u32> {
    value
        .as_bytes()
        .windows(4)
        .find(|digits| digits.iter().all(u8::is_ascii_digit))
        .and_then(|digits| std::str::from_utf8(digits).ok())
        .and_then(|digits| digits.parse().ok())
}

struct OpenMptSource {
    decoder: OpenMpt,
    duration: Duration,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl OpenMptSource {
    fn new(decoder: OpenMpt) -> Self {
        let duration = decoder.duration();
        Self {
            decoder,
            duration,
            pcm: vec![0.0; OPENMPT_RENDER_FRAMES * usize::from(OPENMPT_CHANNELS)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(OPENMPT_CHANNELS),
            Err(error) => {
                eprintln!("Kog libopenmpt playback error: {error}");
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

impl Iterator for OpenMptSource {
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

impl Source for OpenMptSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(OPENMPT_CHANNELS).expect("libopenmpt output is stereo")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(OPENMPT_SAMPLE_RATE).expect("libopenmpt sample rate is nonzero")
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
            "kog-openmpt-backend-{}-{test_name}.mod",
            std::process::id()
        ));
        std::fs::write(&path, crate::openmpt::test_mod_bytes()).expect("write MOD fixture");
        path
    }

    #[test]
    fn registry_expands_routes_and_probes_mod() {
        let path = fixture_path("probe");
        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&path), Some("libopenmpt"));

        let sources = registry.expand(path.clone()).expect("expand MOD subsongs");
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].subsong, Some(0));
        let properties = registry.probe(&sources[0]).expect("probe MOD");
        assert!(
            properties
                .duration
                .is_some_and(|duration| duration.as_secs() >= 7)
        );
        assert_eq!(properties.sample_rate, Some(44_100));
        assert_eq!(properties.channels, Some(2));
        assert_eq!(properties.bits_per_sample, Some(32));
        assert_eq!(properties.title.as_deref(), Some("Kog OpenMPT Test"));
        assert_eq!(properties.codec.as_deref(), Some("ProTracker MOD (M.K.)"));
        assert_eq!(properties.track_number, Some(1));

        let track = crate::track::Track::from_source(sources[0].clone(), &registry);
        assert_eq!(track.title, "Kog OpenMPT Test");
        assert_eq!(track.sample_rate, Some(44_100));
        assert_eq!(track.bits_per_sample, Some(32));
        assert_eq!(track.codec, "ProTracker MOD (M.K.)");

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn source_renders_non_silent_pcm_and_seeks() {
        let path = fixture_path("source");
        let decoder = OpenMptBackend::open(&PlaybackSource {
            path: path.clone(),
            subsong: Some(0),
        })
        .expect("open MOD fixture");
        let mut source = OpenMptSource::new(decoder);
        assert!(
            source
                .by_ref()
                .take(4_410 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "rendered OpenMPT PCM was silent"
        );
        source
            .try_seek(Duration::from_secs(1))
            .expect("seek OpenMPT source");
        assert!(
            source
                .by_ref()
                .take(4_410 * 2)
                .any(|sample| sample.abs() > 0.000_01),
            "rendered OpenMPT PCM was silent after seeking"
        );

        std::fs::remove_file(path).ok();
    }

    #[test]
    fn year_parser_accepts_iso_dates() {
        assert_eq!(parse_year("1998-03-17"), Some(1998));
        assert_eq!(parse_year("unknown"), None);
    }

    #[test]
    fn advertised_extensions_match_the_pinned_native_build() {
        let advertised = OPENMPT_EXTENSIONS
            .iter()
            .map(|extension| (*extension).to_owned())
            .collect::<Vec<_>>();
        assert_eq!(advertised, OpenMpt::supported_extensions());
    }
}
