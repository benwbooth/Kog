//! Parser for external and embedded CUE sheets.

use std::path::{Path, PathBuf};

use encoding_rs::WINDOWS_1252;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReplayGain {
    pub album_gain_db: Option<f32>,
    pub album_peak: Option<f32>,
    pub track_gain_db: Option<f32>,
    pub track_peak: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CuePosition {
    Samples(u64),
    CdFrames(u64),
}

impl CuePosition {
    pub fn sample_frame(self, sample_rate: u32) -> u64 {
        match self {
            Self::Samples(samples) => samples,
            Self::CdFrames(frames) => {
                u64::try_from((u128::from(frames) * u128::from(sample_rate)) / 75_u128)
                    .unwrap_or(u64::MAX)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CueTrack {
    pub number: u32,
    pub audio_path: PathBuf,
    pub position: CuePosition,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
    pub replay_gain: ReplayGain,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CueSheet {
    tracks: Vec<CueTrack>,
}

#[derive(Clone, Debug, Default)]
struct MetadataState {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    genre: Option<String>,
    year: Option<u32>,
    replay_gain: ReplayGain,
}

impl CueSheet {
    pub fn open(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path)
            .map_err(|error| format!("opening CUE sheet {}: {error}", path.display()))?;
        let text = decode_text(path, &bytes)?;
        Self::parse(&text, path, None)
    }

    pub fn embedded(audio_path: &Path, text: &str) -> Result<Self, String> {
        Self::parse(text, audio_path, Some(audio_path))
    }

    pub fn tracks(&self) -> &[CueTrack] {
        &self.tracks
    }

    pub fn track(&self, index: u32) -> Result<&CueTrack, String> {
        self.tracks.get(index as usize).ok_or_else(|| {
            format!(
                "CUE track index {} is out of range for {} tracks",
                index,
                self.tracks.len()
            )
        })
    }

    pub fn frame_range(&self, index: u32, sample_rate: u32) -> Result<(u64, Option<u64>), String> {
        let track = self.track(index)?;
        let start = track.position.sample_frame(sample_rate);
        let end = self.tracks.get(index as usize + 1).and_then(|next| {
            (next.audio_path == track.audio_path).then(|| next.position.sample_frame(sample_rate))
        });
        if end.is_some_and(|end| end <= start) {
            return Err(format!(
                "CUE track {} ends at or before its start",
                track.number
            ));
        }
        Ok((start, end))
    }

    fn parse(text: &str, origin: &Path, embedded_audio: Option<&Path>) -> Result<Self, String> {
        let mut tracks = Vec::new();
        let mut metadata = MetadataState::default();
        let mut current_file = None;
        let mut track_number = None;
        let mut track_is_audio = true;
        let mut track_added = false;

        for raw_line in text.lines() {
            let line = raw_line.trim().trim_start_matches('\u{feff}');
            if line.is_empty() {
                continue;
            }
            let (command, rest) = split_command(line);

            if command.eq_ignore_ascii_case("FILE") {
                current_file = Some(if let Some(audio_path) = embedded_audio {
                    audio_path.to_path_buf()
                } else {
                    let referenced = parse_text(rest).ok_or_else(|| {
                        format!("{} has a FILE command without a path", origin.display())
                    })?;
                    resolve_audio_path(origin, &referenced)?
                });
                track_added = false;
            } else if command.eq_ignore_ascii_case("TRACK") {
                let mut fields = rest.split_whitespace();
                track_number = fields.next().and_then(|value| value.parse::<u32>().ok());
                track_is_audio = fields
                    .next()
                    .is_some_and(|value| value.eq_ignore_ascii_case("AUDIO"));
                track_added = false;
            } else if command.eq_ignore_ascii_case("INDEX") {
                let mut fields = rest.split_whitespace();
                let index = fields.next().unwrap_or_default();
                let position = fields.next().unwrap_or_default();
                if index == "01" && track_is_audio && !track_added {
                    let audio_path = current_file.clone().ok_or_else(|| {
                        format!(
                            "{} has an AUDIO track before any FILE command",
                            origin.display()
                        )
                    })?;
                    let position = parse_position(origin, position)?;
                    tracks.push(CueTrack {
                        number: track_number.unwrap_or(1),
                        audio_path,
                        position,
                        title: metadata.title.clone(),
                        artist: metadata.artist.clone(),
                        album: metadata.album.clone(),
                        genre: metadata.genre.clone(),
                        year: metadata.year,
                        replay_gain: metadata.replay_gain,
                    });
                    track_added = true;
                }
            } else if command.eq_ignore_ascii_case("TITLE") {
                let value = parse_text(rest);
                if current_file.is_some() {
                    metadata.title = value;
                } else {
                    metadata.album = value;
                }
            } else if command.eq_ignore_ascii_case("PERFORMER") {
                metadata.artist = parse_text(rest);
            } else if command.eq_ignore_ascii_case("REM") {
                let (name, value) = split_command(rest);
                apply_metadata(name, value, &mut metadata);
            } else {
                apply_metadata(command, rest, &mut metadata);
            }
        }

        if tracks.is_empty() {
            return Err(format!(
                "{} contains no playable CUE tracks",
                origin.display()
            ));
        }
        Ok(Self { tracks })
    }
}

fn apply_metadata(name: &str, raw_value: &str, metadata: &mut MetadataState) {
    if name.eq_ignore_ascii_case("GENRE") {
        metadata.genre = parse_text(raw_value);
    } else if name.eq_ignore_ascii_case("DATE") {
        metadata.year = leading_u32(raw_value);
    } else if name.eq_ignore_ascii_case("REPLAYGAIN_ALBUM_GAIN") {
        metadata.replay_gain.album_gain_db = leading_f32(raw_value);
    } else if name.eq_ignore_ascii_case("REPLAYGAIN_ALBUM_PEAK") {
        metadata.replay_gain.album_peak = leading_f32(raw_value);
    } else if name.eq_ignore_ascii_case("REPLAYGAIN_TRACK_GAIN") {
        metadata.replay_gain.track_gain_db = leading_f32(raw_value);
    } else if name.eq_ignore_ascii_case("REPLAYGAIN_TRACK_PEAK") {
        metadata.replay_gain.track_peak = leading_f32(raw_value);
    }
}

fn resolve_audio_path(origin: &Path, referenced: &str) -> Result<PathBuf, String> {
    if referenced.contains("://") {
        return Err(format!(
            "{} references a URL; Kog's CUE backend currently accepts filesystem audio paths",
            origin.display()
        ));
    }
    let normalized = referenced.replace('\\', "/");
    let referenced = Path::new(&normalized);
    let path = if referenced.is_absolute() {
        referenced.to_path_buf()
    } else {
        origin
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(referenced)
    };
    let canonical = path.canonicalize().map_err(|error| {
        format!(
            "resolving audio file referenced by {} ({}): {error}",
            origin.display(),
            path.display()
        )
    })?;
    if !canonical.is_file() {
        return Err(format!(
            "audio file referenced by {} is not a file: {}",
            origin.display(),
            canonical.display()
        ));
    }
    Ok(canonical)
}

fn parse_position(origin: &Path, value: &str) -> Result<CuePosition, String> {
    let components = value
        .split(':')
        .map(|component| component.parse::<u64>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            format!(
                "{} has invalid CUE INDEX time {value:?}: {error}",
                origin.display()
            )
        })?;
    match components.as_slice() {
        [samples] => Ok(CuePosition::Samples(*samples)),
        [seconds, frames] if *frames < 75 => Ok(CuePosition::CdFrames(
            seconds.saturating_mul(75).saturating_add(*frames),
        )),
        [minutes, seconds, frames] if *seconds < 60 && *frames < 75 => Ok(CuePosition::CdFrames(
            minutes
                .saturating_mul(60)
                .saturating_add(*seconds)
                .saturating_mul(75)
                .saturating_add(*frames),
        )),
        _ => Err(format!(
            "{} has invalid CUE INDEX time {value:?}",
            origin.display()
        )),
    }
}

fn split_command(line: &str) -> (&str, &str) {
    let end = line.find(char::is_whitespace).unwrap_or(line.len());
    (&line[..end], line[end..].trim())
}

fn parse_text(value: &str) -> Option<String> {
    let value = value.trim();
    let value = if let Some(quoted) = value.strip_prefix('"') {
        quoted.split_once('"').map_or(quoted, |(value, _)| value)
    } else {
        value.split_whitespace().next().unwrap_or_default()
    };
    (!value.is_empty()).then(|| value.to_owned())
}

fn leading_u32(value: &str) -> Option<u32> {
    parse_text(value)?
        .split(|character: char| !character.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

fn leading_f32(value: &str) -> Option<f32> {
    parse_text(value)?.parse().ok()
}

fn decode_text(path: &Path, bytes: &[u8]) -> Result<String, String> {
    if let Some(bytes) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|error| format!("decoding UTF-8 CUE sheet {}: {error}", path.display()));
    }
    if let Some(bytes) = bytes.strip_prefix(&[0xff, 0xfe]) {
        return decode_utf16(path, bytes, u16::from_le_bytes);
    }
    if let Some(bytes) = bytes.strip_prefix(&[0xfe, 0xff]) {
        return decode_utf16(path, bytes, u16::from_be_bytes);
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(text.to_owned());
    }
    let (text, _, _) = WINDOWS_1252.decode(bytes);
    Ok(text.into_owned())
}

fn decode_utf16(path: &Path, bytes: &[u8], order: fn([u8; 2]) -> u16) -> Result<String, String> {
    if !bytes.len().is_multiple_of(2) {
        return Err(format!(
            "{} contains an odd-length UTF-16 CUE sheet",
            path.display()
        ));
    }
    let words = bytes
        .chunks_exact(2)
        .map(|chunk| order([chunk[0], chunk[1]]));
    std::char::decode_utf16(words)
        .collect::<Result<String, _>>()
        .map_err(|error| format!("decoding UTF-16 CUE sheet {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        directory: PathBuf,
        cue: PathBuf,
        first: PathBuf,
        second: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "kog-cuesheet-parser-fixture-{}-{id}",
                std::process::id()
            ));
            std::fs::create_dir_all(directory.join("audio")).expect("create fixture directory");
            let first = directory.join("audio/first.wav");
            let second = directory.join("second.wav");
            std::fs::write(&first, b"first").expect("write first fixture file");
            std::fs::write(&second, b"second").expect("write second fixture file");
            let cue = directory.join("album.cue");
            Self {
                directory,
                cue,
                first,
                second,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.directory).ok();
        }
    }

    #[test]
    fn parses_metadata_replaygain_positions_and_multiple_files() {
        let fixture = Fixture::new();
        std::fs::write(
            &fixture.cue,
            concat!(
                "PERFORMER \"Album Artist\"\r\n",
                "TITLE \"Album Title\"\r\n",
                "REM GENRE \"Game Music\"\r\n",
                "REM DATE 1998\r\n",
                "REPLAYGAIN_ALBUM_GAIN -7.25 dB\r\n",
                "REPLAYGAIN_ALBUM_PEAK 0.998\r\n",
                "FILE \"audio\\first.wav\" WAVE\r\n",
                "  TRACK 01 AUDIO\r\n",
                "    TITLE \"Opening\"\r\n",
                "    INDEX 00 00:00:00\r\n",
                "    INDEX 01 00:00:00\r\n",
                "  TRACK 02 AUDIO\r\n",
                "    TITLE \"Second\"\r\n",
                "    PERFORMER \"Guest\"\r\n",
                "    REM REPLAYGAIN_TRACK_GAIN -3.5 dB\r\n",
                "    REM REPLAYGAIN_TRACK_PEAK 0.75\r\n",
                "    INDEX 01 00:00:01\r\n",
                "FILE \"second.wav\" WAVE\r\n",
                "  TRACK 07 AUDIO\r\n",
                "    TITLE \"Samples\"\r\n",
                "    INDEX 01 12345\r\n",
            ),
        )
        .expect("write CUE fixture");

        let sheet = CueSheet::open(&fixture.cue).expect("parse CUE fixture");
        assert_eq!(sheet.tracks.len(), 3);
        let first = &sheet.tracks[0];
        assert_eq!(first.audio_path, fixture.first.canonicalize().unwrap());
        assert_eq!(first.number, 1);
        assert_eq!(first.position, CuePosition::CdFrames(0));
        assert_eq!(first.title.as_deref(), Some("Opening"));
        assert_eq!(first.artist.as_deref(), Some("Album Artist"));
        assert_eq!(first.album.as_deref(), Some("Album Title"));
        assert_eq!(first.genre.as_deref(), Some("Game Music"));
        assert_eq!(first.year, Some(1998));
        assert_eq!(first.replay_gain.album_gain_db, Some(-7.25));
        assert_eq!(first.replay_gain.album_peak, Some(0.998));

        let second = &sheet.tracks[1];
        assert_eq!(second.position, CuePosition::CdFrames(1));
        assert_eq!(second.artist.as_deref(), Some("Guest"));
        assert_eq!(second.replay_gain.track_gain_db, Some(-3.5));
        assert_eq!(second.replay_gain.track_peak, Some(0.75));
        assert_eq!(sheet.frame_range(0, 48_000).unwrap(), (0, Some(640)));
        assert_eq!(sheet.frame_range(1, 48_000).unwrap(), (640, None));

        let third = &sheet.tracks[2];
        assert_eq!(third.audio_path, fixture.second.canonicalize().unwrap());
        assert_eq!(third.number, 7);
        assert_eq!(third.position, CuePosition::Samples(12_345));
        assert_eq!(third.artist.as_deref(), Some("Guest"));
        assert_eq!(third.replay_gain.track_gain_db, Some(-3.5));
    }

    #[test]
    fn parses_utf16_and_windows_1252_cue_text() {
        let fixture = Fixture::new();
        let text = "TITLE \"Album\"\nFILE \"second.wav\" WAVE\nTRACK 01 AUDIO\nTITLE \"Café\"\nINDEX 01 00:00:00\n";
        let mut utf16 = vec![0xff, 0xfe];
        for word in text.encode_utf16() {
            utf16.extend_from_slice(&word.to_le_bytes());
        }
        std::fs::write(&fixture.cue, utf16).unwrap();
        assert_eq!(
            CueSheet::open(&fixture.cue).unwrap().tracks[0]
                .title
                .as_deref(),
            Some("Café")
        );

        let windows = text.replace("Café", "PLACEHOLDER");
        let mut windows = windows.into_bytes();
        let offset = windows
            .windows(b"PLACEHOLDER".len())
            .position(|window| window == b"PLACEHOLDER")
            .unwrap();
        windows.splice(
            offset..offset + b"PLACEHOLDER".len(),
            b"Caf\xe9".iter().copied(),
        );
        std::fs::write(&fixture.cue, windows).unwrap();
        assert_eq!(
            CueSheet::open(&fixture.cue).unwrap().tracks[0]
                .title
                .as_deref(),
            Some("Café")
        );
    }

    #[test]
    fn rejects_missing_files_urls_and_invalid_or_empty_indexes() {
        let fixture = Fixture::new();
        std::fs::write(
            &fixture.cue,
            "FILE \"missing.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00\n",
        )
        .unwrap();
        assert!(
            CueSheet::open(&fixture.cue)
                .unwrap_err()
                .contains("resolving")
        );

        std::fs::write(
            &fixture.cue,
            "FILE \"https://example.invalid/a.flac\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:00:00\n",
        )
        .unwrap();
        assert!(CueSheet::open(&fixture.cue).unwrap_err().contains("URL"));

        let embedded = CueSheet::embedded(
            &fixture.second,
            "FILE \"ignored.wav\" WAVE\nTRACK 01 AUDIO\nINDEX 01 00:99:00\n",
        );
        assert!(embedded.unwrap_err().contains("invalid CUE INDEX"));

        std::fs::write(&fixture.cue, "TITLE \"Nothing\"\n").unwrap();
        assert!(
            CueSheet::open(&fixture.cue)
                .unwrap_err()
                .contains("no playable")
        );
    }
}
