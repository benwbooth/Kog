use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::sid::Sid;

const SID_EXTENSIONS: &[&str] = &["sid"];
const SID_SAMPLE_RATE: u32 = 44_100;
const SID_DEFAULT_LENGTH: Duration = Duration::from_secs(150);
const SID_DEFAULT_FADE: Duration = Duration::from_secs(8);
const SID_RENDER_FRAMES: usize = 2_048;

pub struct SidBackend;

impl SidBackend {
    fn open(source: &PlaybackSource) -> Result<Sid, String> {
        Sid::open(
            &source.path,
            source.subsong.unwrap_or(0),
            SID_SAMPLE_RATE,
            SID_DEFAULT_LENGTH,
            SID_DEFAULT_FADE,
        )
    }
}

impl DecoderBackend for SidBackend {
    fn id(&self) -> &'static str {
        "libsidplayfp-residfp"
    }

    fn display_name(&self) -> &'static str {
        "libsidplayfp / reSIDfp"
    }

    fn extensions(&self) -> &'static [&'static str] {
        SID_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            subsongs: true,
            ..DecoderCapabilities::default()
        }
    }

    fn subsong_count(&self, path: &Path) -> Result<Option<u32>, String> {
        let decoder = Sid::open(
            path,
            0,
            SID_SAMPLE_RATE,
            SID_DEFAULT_LENGTH,
            SID_DEFAULT_FADE,
        )?;
        Ok(Some(decoder.subsong_count()))
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let decoder = Self::open(source)?;
        let metadata = decoder.metadata();
        let multiple_subsongs = decoder.subsong_count() > 1;
        let (title, album) = if multiple_subsongs {
            (None, metadata.title.clone())
        } else {
            (metadata.title.clone(), None)
        };
        Ok(StreamProperties {
            duration: Some(decoder.duration()),
            sample_rate: Some(decoder.sample_rate()),
            channels: Some(decoder.channels()),
            title,
            artist: metadata.artist.clone(),
            album,
            year: metadata.released.as_deref().and_then(released_year),
            track_number: Some(decoder.selected_subsong() + 1),
            codec: Some(format!("{} / reSIDfp", decoder.codec())),
            bits_per_sample: Some(16),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(SidSource::new(Self::open(source)?));
        Ok(())
    }
}

fn released_year(value: &str) -> Option<u32> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| part.len() == 4)
        .and_then(|part| part.parse().ok())
}

struct SidSource {
    decoder: Sid,
    duration: Duration,
    channels: u16,
    sample_rate: u32,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl SidSource {
    fn new(decoder: Sid) -> Self {
        let duration = decoder.duration();
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();
        Self {
            decoder,
            duration,
            channels,
            sample_rate,
            pcm: vec![0.0; SID_RENDER_FRAMES * usize::from(channels)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(self.channels),
            Err(error) => {
                eprintln!("Kog SID playback error: {error}");
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

impl Iterator for SidSource {
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

impl Source for SidSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(self.channels).expect("SID channel count is nonzero")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.sample_rate).expect("SID sample rate is nonzero")
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
    use crate::sid::test_psid_bytes;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

    struct Fixture(std::path::PathBuf);

    impl Fixture {
        fn new() -> Self {
            let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("kog-sid-fixture-{}-{id}.sid", std::process::id()));
            std::fs::write(&path, test_psid_bytes(false)).expect("write PSID fixture");
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn registry_expands_routes_and_probes_psid() {
        let fixture = Fixture::new();
        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(
            registry.backend_id_for(&fixture.0),
            Some("libsidplayfp-residfp")
        );
        let sources = registry.expand(fixture.0.clone()).expect("expand PSID");
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].subsong, Some(0));
        assert_eq!(sources[1].subsong, Some(1));

        let properties = registry.probe(&sources[1]).expect("probe PSID");
        assert_eq!(properties.duration, Some(Duration::from_secs(158)));
        assert_eq!(properties.sample_rate, Some(SID_SAMPLE_RATE));
        assert_eq!(properties.channels, Some(1));
        assert_eq!(properties.album.as_deref(), Some("Kog SID fixture"));
        assert_eq!(properties.title, None);
        assert_eq!(properties.artist.as_deref(), Some("Kog tests"));
        assert_eq!(properties.year, Some(2026));
        assert_eq!(properties.track_number, Some(2));
        assert_eq!(properties.bits_per_sample, Some(16));
        assert_eq!(
            properties.codec.as_deref(),
            Some("PlaySID one-file format (PSID) / reSIDfp")
        );
    }

    #[test]
    fn source_renders_non_silent_pcm_and_seeks() {
        let fixture = Fixture::new();
        let decoder = SidBackend::open(&PlaybackSource {
            path: fixture.0.clone(),
            subsong: Some(0),
        })
        .expect("open SID source");
        let mut source = SidSource::new(decoder);
        assert!(
            source
                .by_ref()
                .take(8_192)
                .any(|sample| sample.abs() > 0.000_01)
        );
        source
            .try_seek(Duration::from_millis(500))
            .expect("seek SID source");
        assert!(
            source
                .by_ref()
                .take(8_192)
                .any(|sample| sample.abs() > 0.000_01)
        );
    }
}
