use std::num::{NonZeroU16, NonZeroU32};
use std::sync::Arc;
use std::time::Duration;

use rodio::source::SeekError;
use rodio::{ChannelCount, Player, SampleRate, Source};

use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::ffmpeg::Ffmpeg;

// This mirrors Cog's FFmpeg family and includes the conventional formats that
// Cog routes through separate WavPack, Musepack, and Shorten plugins. Existing
// Symphonia routes remain ahead of this fallback in DecoderRegistry.
const FFMPEG_EXTENSIONS: &[&str] = &[
    "wma", "asf", "tak", "mp4", "m4a", "m4b", "m4r", "aac", "mp3", "mp2", "m2a", "mpa", "ape",
    "ac3", "dts", "dtshd", "wav", "tta", "vqf", "vqe", "vql", "ra", "rm", "rmj", "mka", "mkv",
    "weba", "webm", "dsf", "dff", "iff", "dsdiff", "wsd", "aiff", "aif", "wv", "wvp", "mpc", "shn",
];
const RENDER_FRAMES: usize = 2_048;

pub struct FfmpegBackend;

impl DecoderBackend for FfmpegBackend {
    fn id(&self) -> &'static str {
        "ffmpeg"
    }

    fn display_name(&self) -> &'static str {
        "FFmpeg"
    }

    fn extensions(&self) -> &'static [&'static str] {
        FFMPEG_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            ..DecoderCapabilities::default()
        }
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let decoder = Ffmpeg::open(&source.path)?;
        let metadata = decoder.metadata();
        Ok(StreamProperties {
            duration: decoder.duration(),
            sample_rate: Some(decoder.sample_rate()),
            channels: Some(decoder.channels()),
            title: metadata.title.clone(),
            artist: metadata.artist.clone(),
            album: metadata.album.clone(),
            genre: metadata.genre.clone(),
            year: metadata.year,
            track_number: metadata.track,
            codec: Some(decoder.codec().to_owned()),
            bitrate: decoder.bitrate().map(|bits| bits / 1_000),
            bits_per_sample: decoder.bits_per_sample(),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(FfmpegSource::new(Ffmpeg::open(&source.path)?));
        Ok(())
    }
}

struct FfmpegSource {
    decoder: Ffmpeg,
    duration: Option<Duration>,
    channels: u16,
    sample_rate: u32,
    pcm: Vec<f32>,
    pcm_samples: usize,
    pcm_index: usize,
}

impl FfmpegSource {
    fn new(decoder: Ffmpeg) -> Self {
        let duration = decoder.duration();
        let channels = decoder.channels();
        let sample_rate = decoder.sample_rate();
        Self {
            decoder,
            duration,
            channels,
            sample_rate,
            pcm: vec![0.0; RENDER_FRAMES * usize::from(channels)],
            pcm_samples: 0,
            pcm_index: 0,
        }
    }

    fn fill_pcm(&mut self) {
        self.pcm_index = 0;
        self.pcm_samples = match self.decoder.render(&mut self.pcm) {
            Ok(frames) => frames * usize::from(self.channels),
            Err(error) => {
                eprintln!("Kog FFmpeg playback error: {error}");
                0
            }
        };
    }
}

impl Iterator for FfmpegSource {
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

impl Source for FfmpegSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> ChannelCount {
        NonZeroU16::new(self.channels).expect("FFmpeg channel count is nonzero")
    }

    fn sample_rate(&self) -> SampleRate {
        NonZeroU32::new(self.sample_rate).expect("FFmpeg sample rate is nonzero")
    }

    fn total_duration(&self) -> Option<Duration> {
        self.duration
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), SeekError> {
        self.decoder
            .seek(position)
            .map_err(|error| SeekError::Other(Arc::new(std::io::Error::other(error))))?;
        self.pcm_samples = 0;
        self.pcm_index = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::{DecoderRegistry, DecoderSettings};
    use crate::ffmpeg::test_ac3_bytes;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

    struct Fixture(std::path::PathBuf);

    impl Fixture {
        fn new() -> Self {
            let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "kog-ffmpeg-registry-fixture-{}-{id}.ac3",
                std::process::id()
            ));
            std::fs::write(&path, test_ac3_bytes()).expect("write AC-3 fixture");
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn registry_routes_and_probes_ac3_without_stealing_symphonia_formats() {
        let fixture = Fixture::new();
        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&fixture.0), Some("ffmpeg"));
        assert_eq!(
            registry.backend_id_for(std::path::Path::new("song.mp3")),
            Some("rodio-symphonia")
        );
        assert_eq!(
            registry.backend_id_for(std::path::Path::new("song.wv")),
            Some("ffmpeg")
        );

        let source = PlaybackSource::from_path(fixture.0.clone());
        let properties = registry.probe(&source).expect("probe AC-3");
        assert_eq!(properties.sample_rate, Some(32_000));
        assert_eq!(properties.channels, Some(1));
        assert_eq!(properties.bitrate, Some(32));
        assert!(properties.codec.is_some_and(|codec| codec.contains("AC-3")));
        assert!(properties.duration.is_some());
    }

    #[test]
    fn source_renders_non_silent_pcm_and_seeks() {
        let fixture = Fixture::new();
        let mut source = FfmpegSource::new(Ffmpeg::open(&fixture.0).expect("open AC-3 source"));
        assert!(
            source
                .by_ref()
                .take(4_096)
                .any(|sample| sample.abs() > 0.000_01)
        );
        source
            .try_seek(Duration::from_millis(48))
            .expect("seek AC-3 source");
        assert!(
            source
                .by_ref()
                .take(4_096)
                .any(|sample| sample.abs() > 0.000_01)
        );
    }
}
