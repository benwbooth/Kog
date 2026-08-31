//! CueSheet container backend with delegated FFmpeg decoding.

use std::path::{Path, PathBuf};
use std::time::Duration;

use rodio::Player;

use crate::cuesheet::CueSheet;
use crate::decoder::{DecoderBackend, DecoderCapabilities, PlaybackSource, StreamProperties};
use crate::ffmpeg::Ffmpeg;
use crate::ffmpeg_decoder::FfmpegSource;

const CUE_EXTENSIONS: &[&str] = &["cue", "ogg", "opus", "flac", "wv", "mp3"];
const EMBEDDED_CUE_EXTENSIONS: &[&str] = &["ogg", "opus", "flac", "wv", "mp3"];

pub struct CueSheetBackend;

impl DecoderBackend for CueSheetBackend {
    fn id(&self) -> &'static str {
        "cuesheet"
    }

    fn display_name(&self) -> &'static str {
        "CueSheet"
    }

    fn extensions(&self) -> &'static [&'static str] {
        CUE_EXTENSIONS
    }

    fn capabilities(&self) -> DecoderCapabilities {
        DecoderCapabilities {
            seek: true,
            subsongs: true,
            companion_files: true,
            ..DecoderCapabilities::default()
        }
    }

    fn accepts(&self, path: &Path) -> bool {
        if has_extension(path, "cue") {
            return true;
        }
        if !EMBEDDED_CUE_EXTENSIONS
            .iter()
            .any(|extension| has_extension(path, extension))
        {
            return false;
        }
        Ffmpeg::open(path)
            .ok()
            .is_some_and(|decoder| decoder.metadata().cuesheet.is_some())
    }

    fn subsong_count(&self, path: &Path) -> Result<Option<u32>, String> {
        let sheet = load_sheet(path)?;
        u32::try_from(sheet.tracks().len())
            .map(Some)
            .map_err(|_| format!("{} contains too many CUE tracks", path.display()))
    }

    fn source_for_fragment(&self, path: PathBuf, fragment: &str) -> Result<PlaybackSource, String> {
        let requested = fragment.parse::<u32>().map_err(|error| {
            format!(
                "{} has invalid CUE track fragment #{fragment}: {error}",
                path.display()
            )
        })?;
        let sheet = load_sheet(&path)?;
        let index = sheet
            .tracks()
            .iter()
            .position(|track| track.number == requested)
            .ok_or_else(|| format!("{} has no CUE track numbered {fragment}", path.display()))?;
        Ok(PlaybackSource {
            path,
            subsong: Some(
                u32::try_from(index)
                    .map_err(|_| "CUE track index exceeds Kog's source model".to_owned())?,
            ),
            archive_origin: None,
        })
    }

    fn probe(&self, source: &PlaybackSource) -> Result<StreamProperties, String> {
        let sheet = load_sheet(&source.path)?;
        let index = source.subsong.unwrap_or(0);
        let track = sheet.track(index)?;
        let decoder = Ffmpeg::open(&track.audio_path)?;
        let (start, end) = sheet.frame_range(index, decoder.sample_rate())?;
        let metadata = decoder.metadata();
        Ok(StreamProperties {
            duration: range_duration(&decoder, start, end),
            sample_rate: Some(decoder.sample_rate()),
            channels: Some(decoder.channels()),
            title: track.title.clone().or_else(|| metadata.title.clone()),
            artist: track.artist.clone().or_else(|| metadata.artist.clone()),
            album: track.album.clone().or_else(|| metadata.album.clone()),
            genre: track.genre.clone().or_else(|| metadata.genre.clone()),
            year: track.year.or(metadata.year),
            track_number: Some(track.number),
            codec: Some(decoder.codec().to_owned()),
            bitrate: decoder.bitrate().map(|bits| bits / 1_000),
            bits_per_sample: decoder.bits_per_sample(),
            ..StreamProperties::default()
        })
    }

    fn append(&self, source: &PlaybackSource, player: &Player) -> Result<(), String> {
        player.append(open_source(source)?);
        Ok(())
    }
}

fn load_sheet(path: &Path) -> Result<CueSheet, String> {
    if has_extension(path, "cue") {
        return CueSheet::open(path);
    }
    let decoder = Ffmpeg::open(path)?;
    let cuesheet = decoder
        .metadata()
        .cuesheet
        .as_deref()
        .ok_or_else(|| format!("{} has no embedded CUESHEET metadata field", path.display()))?;
    CueSheet::embedded(path, cuesheet)
}

fn open_source(source: &PlaybackSource) -> Result<FfmpegSource, String> {
    let sheet = load_sheet(&source.path)?;
    let index = source.subsong.unwrap_or(0);
    let track = sheet.track(index)?;
    let decoder = Ffmpeg::open(&track.audio_path)?;
    let (start, end) = sheet.frame_range(index, decoder.sample_rate())?;
    FfmpegSource::with_frame_range(decoder, start, end)
}

fn range_duration(decoder: &Ffmpeg, start: u64, end: Option<u64>) -> Option<Duration> {
    let sample_rate = u64::from(decoder.sample_rate());
    let frames = match end {
        Some(end) => end.saturating_sub(start),
        None => duration_frames(decoder.duration()?, decoder.sample_rate()).saturating_sub(start),
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

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(extension))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::{DecoderRegistry, DecoderSettings};
    use rodio::Source;
    use std::io::Write;
    use std::num::{NonZeroU16, NonZeroU32};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);
    const SAMPLE_RATE: u32 = 48_000;
    const IMAGE_FRAMES: u32 = 1_920;

    struct Fixture {
        directory: PathBuf,
        cue: PathBuf,
        wav: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "kog-cuesheet-decoder-fixture-{}-{id}",
                std::process::id()
            ));
            std::fs::create_dir_all(&directory).expect("create CueSheet fixture directory");
            let wav = directory.join("image.wav");
            write_wav(&wav);
            write_wav(&directory.join("tail.wav"));
            let cue = directory.join("album.cue");
            std::fs::write(
                &cue,
                concat!(
                    "PERFORMER \"Album Artist\"\n",
                    "TITLE \"Album Name\"\n",
                    "REM GENRE Chiptune\n",
                    "REM DATE 1999\n",
                    "FILE \"image.wav\" WAVE\n",
                    "  TRACK 01 AUDIO\n",
                    "    TITLE \"First\"\n",
                    "    INDEX 01 00:00:00\n",
                    "  TRACK 02 AUDIO\n",
                    "    TITLE \"Middle\"\n",
                    "    PERFORMER \"Guest\"\n",
                    "    INDEX 01 00:00:01\n",
                    "  TRACK 03 AUDIO\n",
                    "    TITLE \"Last\"\n",
                    "    INDEX 01 00:00:02\n",
                    "FILE \"tail.wav\" WAVE\n",
                    "  TRACK 04 AUDIO\n",
                    "    TITLE \"Other File\"\n",
                    "    INDEX 01 00:00:00\n",
                ),
            )
            .expect("write CueSheet fixture");
            Self {
                directory,
                cue,
                wav,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.directory).ok();
        }
    }

    fn write_wav(path: &Path) {
        let data_size = IMAGE_FRAMES * 2;
        let mut file = std::fs::File::create(path).expect("create CueSheet wave image");
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
        for frame in 0..IMAGE_FRAMES {
            file.write_all(&sample_at(frame).to_le_bytes()).unwrap();
        }
    }

    fn sample_at(frame: u32) -> i16 {
        let phase = (frame % 32) as i32;
        ((phase - 16) * 1_500) as i16
    }

    fn expected_sample(frame: u32) -> f32 {
        f32::from(sample_at(frame)) / 32_768.0
    }

    fn embedded_cue_mp3_bytes() -> Vec<u8> {
        // 300 ms of a generated 880 Hz sine with an ID3v2 CUESHEET TXXX field.
        // FFmpeg with libmp3lame produced this redistributable fixture.
        let hex = concat!(
            "49443304000000000206545858580000015a000003435545534845455400504552464f524d45522022456d62656464656420417274697374220a5449544c452022456d62656464656420416c62756d220a46494c45202269676e6f7265642e6d70332220574156450a2020545241434b20303120415544494f0a202020205449544c452022456d626564646564204669727374220a20202020494e4445582030312030303a30303a30300a2020545241434b20303220415544494f0a202020205449544c452022456d626564646564205365636f6e64220a20202020494e4445582030312030303a30303a303100545353450000000e0000034c61766636332e312e3130310000000000000000000000ffe338c0000000000000000000496e666f0000000f00000007000002d000666666666666666666666666666680808080808080808080808080809999999999999999999999999999b3b3b3b3b3b3b3b3b3b3b3b3b3b3b3cccccccccccccccccccccccccccce6e6e6e6e6e6e6e6e6e6e6e6e6e6ffffffffffffffffffffffffffff000000004c61766336332e312e00000000000000000000000024042000000000000002d02392adb60000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "ffe318c4000ce90ebe59412800b6dd6d02efc018c6318c7e00000bcc6318f900005f8c7fc971ff20700300c3e77fe18e9e90c72fcb820e9404c3f93043977f774a124010c65ff285ffe318c4070e6942aca982980031bfc08a1407fd2322f189748afff93439c0d4401fe14b102312edc129082c4ebf3813d0172155f2ea40d094240d7fe253a0d5ffffffffffffd4a8ffe318c4080e512a3c01c2f001050d51a084c02000d8a1805821183c844184284f185f9809afc35b98d8083986e03d98010002191805800407116bb0d5f595fe7103d03a6ad85bb0ffe318c4090e492e300000bcc000051806811981402a183787498e70e09ffe99d18f08011835005808118200450a97abcd3952ae5ffffffe9ffecffff42affffceffa1dff21cf46effe318c40a0ea94a700142e00040317234ee12372fd6162dbb132606451d896062e0518200c8d1c89cbf3ce92c733cfbff9d20609d6087287386394770c518710dfe388617fe0041",
            "ffe318c40a0f194ea4018288006009a79a1b9142e13e399a9557e4505ce08300051280b9ccc8a24894927affe4d9d30335826243fff58a8b0b8a8b37ffad82ca4c414d45332e3130ffe318c4080000034801c0000030aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        );
        hex.as_bytes()
            .chunks_exact(2)
            .map(|pair| {
                let pair = std::str::from_utf8(pair).expect("ASCII fixture hex");
                u8::from_str_radix(pair, 16).expect("valid fixture hex")
            })
            .collect()
    }

    #[test]
    fn registry_expands_routes_and_probes_external_cue_tracks() {
        let fixture = Fixture::new();
        let registry = DecoderRegistry::new(DecoderSettings::default());
        assert_eq!(registry.backend_id_for(&fixture.cue), Some("cuesheet"));
        assert_eq!(
            registry.backend_id_for(&fixture.wav),
            Some("rodio-symphonia")
        );

        let sources = registry.expand(fixture.cue.clone()).expect("expand CUE");
        assert_eq!(sources.len(), 4);
        assert_eq!(sources[0].subsong, Some(0));
        assert_eq!(sources[3].subsong, Some(3));

        let properties = registry.probe(&sources[1]).expect("probe second CUE track");
        assert_eq!(properties.duration, Some(Duration::from_millis(40) / 3));
        assert_eq!(properties.sample_rate, Some(SAMPLE_RATE));
        assert_eq!(properties.channels, Some(1));
        assert_eq!(properties.title.as_deref(), Some("Middle"));
        assert_eq!(properties.artist.as_deref(), Some("Guest"));
        assert_eq!(properties.album.as_deref(), Some("Album Name"));
        assert_eq!(properties.genre.as_deref(), Some("Chiptune"));
        assert_eq!(properties.year, Some(1999));
        assert_eq!(properties.track_number, Some(2));
        assert!(properties.codec.is_some_and(|codec| codec.contains("PCM")));

        let last_image_track = registry.probe(&sources[2]).expect("probe last image track");
        assert_eq!(
            last_image_track.duration,
            Some(Duration::from_millis(40) / 3)
        );
        let other_file = registry.probe(&sources[3]).expect("probe other CUE file");
        assert_eq!(other_file.title.as_deref(), Some("Other File"));
        assert_eq!(other_file.duration, Some(Duration::from_millis(40)));
    }

    #[test]
    fn ranged_source_starts_seeks_and_ends_at_exact_cue_boundaries() {
        let fixture = Fixture::new();
        let source = PlaybackSource {
            path: fixture.cue.clone(),
            subsong: Some(1),
            archive_origin: None,
        };
        let mut source = open_source(&source).expect("open second CueSheet track");
        assert_eq!(source.sample_rate(), NonZeroU32::new(SAMPLE_RATE).unwrap());
        assert_eq!(source.channels(), NonZeroU16::new(1).unwrap());
        assert_eq!(
            source.total_duration(),
            Some(Duration::from_nanos(13_333_333))
        );

        let samples = source.by_ref().collect::<Vec<_>>();
        assert_eq!(samples.len(), 640);
        assert!((samples[0] - expected_sample(640)).abs() < 0.000_1);
        assert!(samples.iter().any(|sample| sample.abs() > 0.01));
        assert_eq!(source.next(), None);

        source
            .try_seek(Duration::from_nanos(6_666_666))
            .expect("seek within second CueSheet track");
        let tail = source.by_ref().collect::<Vec<_>>();
        assert_eq!(tail.len(), 321);
        assert!((tail[0] - expected_sample(959)).abs() < 0.000_1);
        assert!(tail.iter().any(|sample| sample.abs() > 0.01));
    }

    #[test]
    fn content_probe_routes_and_expands_embedded_mp3_cuesheet_only() {
        let fixture = Fixture::new();
        let embedded = fixture.directory.join("embedded.mp3");
        std::fs::write(&embedded, embedded_cue_mp3_bytes()).expect("write embedded CUE MP3");
        let registry = DecoderRegistry::new(DecoderSettings::default());

        assert_eq!(registry.backend_id_for(&embedded), Some("cuesheet"));
        assert_eq!(
            registry.backend_id_for(Path::new("ordinary.mp3")),
            Some("rodio-symphonia")
        );
        let sources = registry.expand(embedded).expect("expand embedded CUE");
        assert_eq!(sources.len(), 2);

        let first = registry.probe(&sources[0]).expect("probe embedded track");
        assert_eq!(first.sample_rate, Some(8_000));
        assert_eq!(first.channels, Some(1));
        assert_eq!(first.title.as_deref(), Some("Embedded First"));
        assert_eq!(first.artist.as_deref(), Some("Embedded Artist"));
        assert_eq!(first.album.as_deref(), Some("Embedded Album"));
        assert_eq!(first.track_number, Some(1));
        assert_eq!(first.duration, Some(Duration::from_micros(13_250)));

        let mut ranged = open_source(&sources[0]).expect("open embedded CUE track");
        let samples = ranged.by_ref().collect::<Vec<_>>();
        assert_eq!(samples.len(), 106);
        assert!(samples.iter().any(|sample| sample.abs() > 0.000_01));
        assert_eq!(ranged.next(), None);
    }

    #[test]
    fn out_of_range_track_is_reported() {
        let fixture = Fixture::new();
        let source = PlaybackSource {
            path: fixture.cue.clone(),
            subsong: Some(9),
            archive_origin: None,
        };
        assert!(
            CueSheetBackend
                .probe(&source)
                .unwrap_err()
                .contains("out of range")
        );
    }
}
