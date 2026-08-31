use std::time::Duration;

use rodio::Player;

use crate::apl::AplLink;
use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::ffmpeg::Ffmpeg;
use crate::ffmpeg_decoder::FfmpegSource;

const APL_EXTENSIONS: &[&str] = &["apl"];

pub struct AplBackend;

impl DecoderBackend for AplBackend {
    fn id(&self) -> &'static str {
        "apl"
    }

    fn display_name(&self) -> &'static str {
        "Monkey's Audio Image Link"
    }

    fn extensions(&self) -> &'static [&'static str] {
        APL_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            companion_files: true,
            ..DecoderCapabilities::default()
        }
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let link = AplLink::open(&source.path)?;
        let decoder = Ffmpeg::open(&link.audio_path)?;
        let duration = range_duration(&link, &decoder);
        Ok(StreamProperties {
            duration,
            sample_rate: Some(decoder.sample_rate()),
            channels: Some(decoder.channels()),
            codec: Some(decoder.codec().to_owned()),
            bitrate: decoder.bitrate().map(|bits| bits / 1_000),
            bits_per_sample: decoder.bits_per_sample(),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        let link = AplLink::open(&source.path)?;
        let decoder = Ffmpeg::open(&link.audio_path)?;
        player.append(FfmpegSource::with_frame_range(
            decoder,
            link.start_frame,
            link.end_frame,
        )?);
        Ok(())
    }
}

fn range_duration(link: &AplLink, decoder: &Ffmpeg) -> Option<Duration> {
    let sample_rate = u64::from(decoder.sample_rate());
    let frames = match link.end_frame {
        Some(end_frame) => end_frame - link.start_frame,
        None => {
            let total = decoder.duration()?;
            duration_frames(total, decoder.sample_rate()).saturating_sub(link.start_frame)
        }
    };
    Some(Duration::new(
        frames / sample_rate,
        ((u128::from(frames % sample_rate) * 1_000_000_000_u128) / u128::from(sample_rate)) as u32,
    ))
}

fn duration_frames(duration: Duration, sample_rate: u32) -> u64 {
    duration
        .as_secs()
        .saturating_mul(u64::from(sample_rate))
        .saturating_add(
            ((u128::from(duration.subsec_nanos()) * u128::from(sample_rate)) / 1_000_000_000_u128)
                as u64,
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::{DecoderRegistry, DecoderSettings};
    use rodio::Source;
    use rodio::source::SeekError;
    use std::io::Write;
    use std::num::{NonZeroU16, NonZeroU32};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);
    const SAMPLE_RATE: u32 = 8_000;
    const START_FRAME: u32 = 200;
    const END_FRAME: u32 = 600;

    struct Fixture {
        directory: PathBuf,
        apl: PathBuf,
        wav: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "kog-apl-decoder-fixture-{}-{id}",
                std::process::id()
            ));
            std::fs::create_dir_all(&directory).expect("create APL fixture directory");
            let wav = directory.join("image.wav");
            write_wav(&wav);
            let apl = directory.join("selection.apl");
            std::fs::write(
                &apl,
                format!(
                    "[Monkey's Audio Image Link File]\r\nImage File=image.wav\r\nStart Block={START_FRAME}\r\nFinish Block={END_FRAME}\r\n----- APE TAG (DO NOT TOUCH!!!) -----\r\n"
                ),
            )
            .expect("write APL fixture");
            Self {
                directory,
                apl,
                wav,
            }
        }

        fn link(&self) -> AplLink {
            AplLink::open(&self.apl).expect("parse fixture APL")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.directory).ok();
        }
    }

    fn write_wav(path: &Path) {
        let sample_count = 800_u32;
        let data_size = sample_count * 2;
        let mut file = std::fs::File::create(path).expect("create APL wave image");
        file.write_all(b"RIFF").unwrap();
        file.write_all(&(36 + data_size).to_le_bytes()).unwrap();
        file.write_all(b"WAVEfmt ").unwrap();
        file.write_all(&16_u32.to_le_bytes()).unwrap();
        file.write_all(&1_u16.to_le_bytes()).unwrap();
        file.write_all(&1_u16.to_le_bytes()).unwrap();
        file.write_all(&SAMPLE_RATE.to_le_bytes()).unwrap();
        file.write_all(&(SAMPLE_RATE * 2).to_le_bytes()).unwrap();
        file.write_all(&2_u16.to_le_bytes()).unwrap();
        file.write_all(&16_u16.to_le_bytes()).unwrap();
        file.write_all(b"data").unwrap();
        file.write_all(&data_size.to_le_bytes()).unwrap();
        for frame in 0..sample_count {
            let phase = (frame % 32) as i32;
            let sample = ((phase - 16) * 1_500) as i16;
            file.write_all(&sample.to_le_bytes()).unwrap();
        }
    }

    fn expected_sample(frame: u32) -> f32 {
        let phase = (frame % 32) as i32;
        ((phase - 16) * 1_500) as f32 / 32_768.0
    }

    #[test]
    fn registry_routes_and_probes_the_apl_selection() {
        let fixture = Fixture::new();
        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&fixture.apl), Some("apl"));
        assert_eq!(
            registry.backend_id_for(&fixture.wav),
            Some("rodio-symphonia")
        );

        let properties = registry
            .probe(&PlaybackSource::from_path(fixture.apl.clone()))
            .expect("probe APL selection");
        assert_eq!(properties.sample_rate, Some(SAMPLE_RATE));
        assert_eq!(properties.channels, Some(1));
        assert_eq!(properties.duration, Some(Duration::from_millis(50)));
        assert!(properties.codec.is_some_and(|codec| codec.contains("PCM")));
    }

    #[test]
    fn source_renders_only_the_apl_range_seeks_and_ends_exactly() {
        let fixture = Fixture::new();
        let link = fixture.link();
        let decoder = Ffmpeg::open(&link.audio_path).expect("open APL image");
        let mut source = FfmpegSource::with_frame_range(decoder, link.start_frame, link.end_frame)
            .expect("create ranged source");
        assert_eq!(source.sample_rate(), NonZeroU32::new(SAMPLE_RATE).unwrap());
        assert_eq!(source.channels(), NonZeroU16::new(1).unwrap());
        assert_eq!(source.total_duration(), Some(Duration::from_millis(50)));

        let samples = source.by_ref().collect::<Vec<_>>();
        assert_eq!(samples.len(), (END_FRAME - START_FRAME) as usize);
        assert!((samples[0] - expected_sample(START_FRAME)).abs() < 0.000_1);
        assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        assert_eq!(source.next(), None);

        source
            .try_seek(Duration::from_millis(25))
            .map_err(|error: SeekError| error.to_string())
            .expect("seek within APL selection");
        let tail = source.by_ref().collect::<Vec<_>>();
        assert_eq!(tail.len(), ((END_FRAME - START_FRAME) / 2) as usize);
        assert!((tail[0] - expected_sample(400)).abs() < 0.000_1);
        assert!(tail.iter().any(|sample| sample.abs() > 0.01));
    }
}
