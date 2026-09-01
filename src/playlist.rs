//! Cog-compatible M3U/M3U8 and PLS playlist parsing.

use std::io::Write;
use std::path::{Path, PathBuf};

use encoding_rs::{GB18030, WINDOWS_1251};
use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};
use url::Url;

const UNPACK_COMPONENT_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'<')
    .add(b'>')
    .add(b'?')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaylistLocation {
    Local(PathBuf),
    Remote(String),
    Archive {
        archive_path: PathBuf,
        entry_name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaylistEntry {
    pub location: PlaylistLocation,
    pub fragment: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Playlist {
    entries: Vec<PlaylistEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaylistFormat {
    M3u,
    Pls,
}

impl Playlist {
    pub fn open(path: &Path) -> Result<Self, String> {
        let format = PlaylistFormat::for_path(path)?;
        let bytes = std::fs::read(path)
            .map_err(|error| format!("opening playlist {}: {error}", path.display()))?;
        let text = decode_text(&bytes);
        let text = text.replace('\r', "\n");
        let raw_entries = match format {
            PlaylistFormat::M3u => parse_m3u(path, &text)?,
            PlaylistFormat::Pls => parse_pls(&text),
        };
        let entries = raw_entries
            .into_iter()
            .map(|entry| resolve_entry(path, entry))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[PlaylistEntry] {
        &self.entries
    }

    pub fn is_path(path: &Path) -> bool {
        PlaylistFormat::for_path(path).is_ok()
    }

    pub fn is_hls(path: &Path) -> Result<bool, String> {
        if PlaylistFormat::for_path(path).ok() != Some(PlaylistFormat::M3u) {
            return Ok(false);
        }
        let bytes = std::fs::read(path)
            .map_err(|error| format!("opening playlist {}: {error}", path.display()))?;
        Ok(contains_hls_tag(&decode_text(&bytes).replace('\r', "\n")))
    }

    pub fn save(path: &Path, entries: &[PlaylistEntry]) -> Result<(), String> {
        let format = PlaylistFormat::for_path(path)?;
        let mut body = String::new();
        match format {
            PlaylistFormat::M3u => {
                // Cog deliberately writes a one-character comment instead of
                // EXTINF metadata. Preserve that small but observable detail.
                body.push_str("#\n");
                for entry in entries {
                    body.push_str(&serialize_entry(path, entry)?);
                    body.push('\n');
                }
            }
            PlaylistFormat::Pls => {
                body.push_str(&format!(
                    "[playlist]\nnumberOfEntries={}\n\n",
                    entries.len()
                ));
                for (index, entry) in entries.iter().enumerate() {
                    body.push_str(&format!(
                        "File{}={}\n",
                        index + 1,
                        serialize_entry(path, entry)?
                    ));
                }
                body.push_str("\nVERSION=2");
            }
        }

        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = tempfile::Builder::new()
            .prefix(".kog-playlist-")
            .tempfile_in(parent)
            .map_err(|error| {
                format!(
                    "creating a temporary playlist beside {}: {error}",
                    path.display()
                )
            })?;
        temporary.write_all(body.as_bytes()).map_err(|error| {
            format!("writing temporary playlist for {}: {error}", path.display())
        })?;
        temporary.as_file_mut().sync_all().map_err(|error| {
            format!(
                "flushing temporary playlist for {}: {error}",
                path.display()
            )
        })?;
        temporary
            .persist(path)
            .map_err(|error| format!("replacing playlist {}: {}", path.display(), error.error))?;
        Ok(())
    }
}

impl PlaylistFormat {
    fn for_path(path: &Path) -> Result<Self, String> {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if extension.eq_ignore_ascii_case("m3u") || extension.eq_ignore_ascii_case("m3u8") {
            Ok(Self::M3u)
        } else if extension.eq_ignore_ascii_case("pls") {
            Ok(Self::Pls)
        } else {
            Err(format!("{} is not an M3U or PLS playlist", path.display()))
        }
    }
}

fn parse_m3u<'a>(path: &Path, text: &'a str) -> Result<Vec<&'a str>, String> {
    let mut entries = Vec::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if is_hls_tag(line) {
            return Err(format!(
                "{} is an HLS playlist and must be opened through Kog's FFmpeg backend",
                path.display()
            ));
        }
        if !line.starts_with('#') {
            entries.push(line);
        }
    }
    Ok(entries)
}

fn contains_hls_tag(text: &str) -> bool {
    text.lines().map(str::trim).any(is_hls_tag)
}

fn is_hls_tag(line: &str) -> bool {
    line.get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("#EXT-X-"))
}

fn parse_pls(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter_map(|line| {
            let (name, value) = line.split_once('=')?;
            name.trim()
                .get(..4)
                .filter(|prefix| prefix.eq_ignore_ascii_case("File"))?;
            let value = value.trim();
            (!value.is_empty()).then_some(value)
        })
        .collect()
}

fn resolve_entry(playlist_path: &Path, entry: &str) -> Result<PlaylistEntry, String> {
    if let Some(entry) = resolve_unpack_entry(playlist_path, entry)? {
        return Ok(entry);
    }
    if entry.contains("://") {
        let mut url = Url::parse(entry).map_err(|error| {
            format!(
                "{} contains invalid URL {entry:?}: {error}",
                playlist_path.display()
            )
        })?;
        if url.scheme().eq_ignore_ascii_case("file") {
            let fragment = url.fragment().map(str::to_owned);
            url.set_fragment(None);
            let path = url.to_file_path().map_err(|()| {
                format!(
                    "{} contains a file URL that is not valid on this platform: {entry}",
                    playlist_path.display()
                )
            })?;
            return Ok(PlaylistEntry {
                location: PlaylistLocation::Local(path),
                fragment,
            });
        }
        return Ok(PlaylistEntry {
            location: PlaylistLocation::Remote(entry.to_owned()),
            fragment: None,
        });
    }

    let (path, fragment) = split_numeric_fragment(entry);
    let normalized = path.replace('\\', "/");
    let path = Path::new(&normalized);
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        playlist_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(path)
    };
    Ok(PlaylistEntry {
        location: PlaylistLocation::Local(path),
        fragment,
    })
}

fn resolve_unpack_entry(
    playlist_path: &Path,
    entry: &str,
) -> Result<Option<PlaylistEntry>, String> {
    if !entry
        .get(.."unpack://".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("unpack://"))
    {
        return Ok(None);
    }

    let (packed, fragment) = split_numeric_fragment(entry);
    let decoded = percent_decode_str(packed).decode_utf8().map_err(|error| {
        format!(
            "{} contains an invalid UTF-8 Cog archive URL {entry:?}: {error}",
            playlist_path.display()
        )
    })?;
    let rest = decoded
        .get("unpack://".len()..)
        .ok_or_else(|| format!("Malformed Cog archive URL {entry:?}"))?;
    let (kind, rest) = rest
        .split_once('|')
        .ok_or_else(|| format!("Malformed Cog archive URL {entry:?}"))?;
    if kind != "fex" {
        return Err(format!(
            "{} contains unsupported Cog archive source type {kind:?}",
            playlist_path.display()
        ));
    }
    let (length, packed_paths) = rest
        .split_once('|')
        .ok_or_else(|| format!("Malformed Cog archive URL {entry:?}"))?;
    let length = length.parse::<usize>().map_err(|error| {
        format!(
            "{} contains invalid Cog archive path length {length:?}: {error}",
            playlist_path.display()
        )
    })?;
    let (archive, remainder) = split_utf16_prefix(packed_paths, length).ok_or_else(|| {
        format!(
            "{} contains a truncated Cog archive path in {entry:?}",
            playlist_path.display()
        )
    })?;
    let entry_name = remainder
        .strip_prefix('|')
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "{} contains an empty Cog archive entry",
                playlist_path.display()
            )
        })?
        .replace('\\', "/");
    let archive_path = if archive.contains("://") {
        let url = Url::parse(archive).map_err(|error| {
            format!(
                "{} contains invalid archive URL {archive:?}: {error}",
                playlist_path.display()
            )
        })?;
        if !url.scheme().eq_ignore_ascii_case("file") {
            return Err(format!(
                "{} contains a non-file archive URL {archive:?}",
                playlist_path.display()
            ));
        }
        url.to_file_path().map_err(|()| {
            format!(
                "{} contains an archive URL that is invalid on this platform: {archive}",
                playlist_path.display()
            )
        })?
    } else {
        PathBuf::from(archive)
    };
    let archive_path = if archive_path.is_absolute() {
        archive_path
    } else {
        playlist_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(archive_path)
    };
    Ok(Some(PlaylistEntry {
        location: PlaylistLocation::Archive {
            archive_path,
            entry_name,
        },
        fragment,
    }))
}

fn split_utf16_prefix(value: &str, units: usize) -> Option<(&str, &str)> {
    if units == 0 {
        return Some(("", value));
    }
    let mut consumed = 0_usize;
    for (index, character) in value.char_indices() {
        consumed += character.len_utf16();
        if consumed == units {
            let end = index + character.len_utf8();
            return Some(value.split_at(end));
        }
        if consumed > units {
            return None;
        }
    }
    None
}

fn serialize_entry(playlist_path: &Path, entry: &PlaylistEntry) -> Result<String, String> {
    let fragment = entry
        .fragment
        .as_deref()
        .map(validate_fragment)
        .transpose()?;
    let mut value = match &entry.location {
        PlaylistLocation::Local(path) => serialize_local_path(playlist_path, path, fragment)?,
        PlaylistLocation::Remote(url) => {
            validate_line_value(url, "remote playlist URL")?;
            url.clone()
        }
        PlaylistLocation::Archive {
            archive_path,
            entry_name,
        } => {
            let archive = archive_path.to_str().ok_or_else(|| {
                format!(
                    "Archive path is not valid UTF-8 and cannot be saved: {}",
                    archive_path.display()
                )
            })?;
            validate_line_value(archive, "archive path")?;
            validate_line_value(entry_name, "archive entry name")?;
            if entry_name.is_empty() {
                return Err("Archive entry name cannot be empty".to_owned());
            }
            let archive_length = archive.encode_utf16().count();
            format!(
                "unpack://fex|{archive_length}|{}|{}",
                utf8_percent_encode(archive, UNPACK_COMPONENT_ENCODE_SET),
                utf8_percent_encode(entry_name, UNPACK_COMPONENT_ENCODE_SET)
            )
        }
    };
    if !matches!(&entry.location, PlaylistLocation::Local(_))
        && let Some(fragment) = fragment
    {
        value.push('#');
        value.push_str(fragment);
    }
    Ok(value)
}

fn serialize_local_path(
    playlist_path: &Path,
    path: &Path,
    fragment: Option<&str>,
) -> Result<String, String> {
    let base = playlist_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let relative = path
        .strip_prefix(base)
        .ok()
        .filter(|path| !path.as_os_str().is_empty());
    let candidate = relative.unwrap_or(path);
    let raw = candidate.to_str().ok_or_else(|| {
        format!(
            "Playlist entry path is not valid UTF-8 and cannot be saved: {}",
            path.display()
        )
    })?;
    validate_line_value(raw, "playlist entry path")?;
    let normalized = raw.replace('\\', "/");
    let needs_file_url = relative.is_none()
        || normalized.starts_with('#')
        || normalized.contains("://")
        || (fragment.is_none() && split_numeric_fragment(&normalized).1.is_some());
    let mut value = if needs_file_url {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            base.join(path)
        };
        Url::from_file_path(&absolute)
            .map_err(|()| {
                format!(
                    "Playlist entry is not a valid local path on this platform: {}",
                    path.display()
                )
            })?
            .into()
    } else {
        normalized
    };
    if let Some(fragment) = fragment {
        value.push('#');
        value.push_str(fragment);
    }
    Ok(value)
}

fn validate_fragment(fragment: &str) -> Result<&str, String> {
    if fragment.is_empty() || !fragment.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!(
            "Kog can currently save only numeric playlist fragments, not #{fragment}"
        ));
    }
    Ok(fragment)
}

fn validate_line_value(value: &str, description: &str) -> Result<(), String> {
    if value.contains(['\0', '\r', '\n']) {
        return Err(format!(
            "Cannot save {description} containing a line break or NUL"
        ));
    }
    Ok(())
}

fn split_numeric_fragment(value: &str) -> (&str, Option<String>) {
    let Some((path, fragment)) = value.rsplit_once('#') else {
        return (value, None);
    };
    if !fragment.is_empty() && fragment.bytes().all(|byte| byte.is_ascii_digit()) {
        (path, Some(fragment.to_owned()))
    } else {
        (value, None)
    }
}

fn decode_text(bytes: &[u8]) -> String {
    let bytes = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
    if let Some(bytes) = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
        return String::from_utf8_lossy(bytes).into_owned();
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_owned();
    }
    let (text, _, errors) = GB18030.decode(bytes);
    if !errors {
        return text.into_owned();
    }
    let (text, _, errors) = WINDOWS_1251.decode(bytes);
    if !errors {
        return text.into_owned();
    }
    bytes.iter().map(|byte| char::from(*byte)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::{DecoderRegistry, DecoderSettings};
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let id = FIXTURE_ID.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "kog-playlist-parser-fixture-{}-{id}",
                std::process::id()
            ));
            std::fs::create_dir_all(directory.join("audio")).expect("create playlist fixture");
            Self(directory)
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn m3u_preserves_order_paths_urls_and_numeric_fragments() {
        let fixture = Fixture::new();
        let playlist_path = fixture.path("mix.m3u8");
        std::fs::write(
            &playlist_path,
            concat!(
                "\u{feff}#EXTM3U\r\n",
                "#EXTINF:123,Ignored title\r\n",
                " audio\\first.nsf#2 \r\n",
                "https://example.invalid/live.mp3\r",
                "file:///tmp/encoded%20name.flac#7\r",
                "audio/hash#name.mp3\r",
            ),
        )
        .unwrap();

        let playlist = Playlist::open(&playlist_path).expect("parse M3U8");
        assert_eq!(playlist.entries.len(), 4);
        assert_eq!(
            playlist.entries[0],
            PlaylistEntry {
                location: PlaylistLocation::Local(fixture.path("audio/first.nsf")),
                fragment: Some("2".to_owned()),
            }
        );
        assert_eq!(
            playlist.entries[1].location,
            PlaylistLocation::Remote("https://example.invalid/live.mp3".to_owned())
        );
        assert_eq!(
            playlist.entries[2],
            PlaylistEntry {
                location: PlaylistLocation::Local(PathBuf::from("/tmp/encoded name.flac")),
                fragment: Some("7".to_owned()),
            }
        );
        assert_eq!(
            playlist.entries[3].location,
            PlaylistLocation::Local(fixture.path("audio/hash#name.mp3"))
        );
        assert_eq!(playlist.entries[3].fragment, None);
    }

    #[test]
    fn pls_uses_only_file_prefixed_keys_in_line_order() {
        let fixture = Fixture::new();
        let playlist_path = fixture.path("radio.pls");
        std::fs::write(
            &playlist_path,
            concat!(
                "[playlist]\n",
                "File2=second.ogg\n",
                "Title2=Ignored\n",
                "Length2=42\n",
                "fIlE1 = first.flac#0\n",
                "NumberOfEntries=2\n",
                "Version=2\n",
            ),
        )
        .unwrap();

        let playlist = Playlist::open(&playlist_path).expect("parse PLS");
        assert_eq!(playlist.entries.len(), 2);
        assert_eq!(
            playlist.entries[0].location,
            PlaylistLocation::Local(fixture.path("second.ogg"))
        );
        assert_eq!(
            playlist.entries[1],
            PlaylistEntry {
                location: PlaylistLocation::Local(fixture.path("first.flac")),
                fragment: Some("0".to_owned()),
            }
        );
    }

    #[test]
    fn saves_cog_m3u_and_pls_and_roundtrips_every_source_identity() {
        let fixture = Fixture::new();
        let archive_path = fixture.path("set 🎵.zip");
        let entries = vec![
            PlaylistEntry {
                location: PlaylistLocation::Local(fixture.path("audio/first.nsf")),
                fragment: Some("2".to_owned()),
            },
            PlaylistEntry {
                location: PlaylistLocation::Local(fixture.path("audio/literal#7")),
                fragment: None,
            },
            PlaylistEntry {
                location: PlaylistLocation::Remote(
                    "https://example.invalid/live.mp3?token=test".to_owned(),
                ),
                fragment: None,
            },
            PlaylistEntry {
                location: PlaylistLocation::Archive {
                    archive_path: archive_path.clone(),
                    entry_name: "disc/song #1.jxs".to_owned(),
                },
                fragment: Some("1".to_owned()),
            },
        ];

        for extension in ["m3u", "m3u8", "pls"] {
            let path = fixture.path(&format!("saved.{extension}"));
            Playlist::save(&path, &entries).expect("save playlist");
            assert_eq!(
                Playlist::open(&path).expect("reopen playlist").entries,
                entries
            );
            let text = std::fs::read_to_string(&path).expect("read saved playlist");
            if extension == "pls" {
                assert!(text.starts_with("[playlist]\nnumberOfEntries=4\n\n"));
                assert!(text.ends_with("\nVERSION=2"));
            } else {
                assert!(text.starts_with("#\n"));
                assert!(text.contains("audio/first.nsf#2\n"));
            }
            assert!(text.contains("file://"));
            assert!(text.contains("unpack://fex|"));
            assert!(text.contains("#1"));
        }
    }

    #[test]
    fn rejects_malformed_or_unsupported_cog_archive_urls() {
        let fixture = Fixture::new();
        let path = fixture.path("bad.m3u");
        for entry in [
            "unpack://fex|999|/tmp/archive.zip|song.wav",
            "unpack://other|4|/tmp|song.wav",
            "unpack://fex|4|/tmp|",
        ] {
            std::fs::write(&path, format!("{entry}\n")).unwrap();
            assert!(Playlist::open(&path).is_err(), "accepted {entry}");
        }
    }

    #[test]
    fn decodes_cogs_gb18030_cp1251_and_latin1_fallback_order() {
        let fixture = Fixture::new();
        let playlist_path = fixture.path("encoded.m3u");

        let (gb18030, _, errors) = GB18030.encode("音乐.flac\n");
        assert!(!errors);
        std::fs::write(&playlist_path, gb18030.as_ref()).unwrap();
        assert_eq!(
            Playlist::open(&playlist_path).unwrap().entries[0].location,
            PlaylistLocation::Local(fixture.path("音乐.flac"))
        );

        let (cp1251, _, errors) = WINDOWS_1251.encode("песня.ogg\n");
        assert!(!errors);
        std::fs::write(&playlist_path, cp1251.as_ref()).unwrap();
        assert_eq!(
            Playlist::open(&playlist_path).unwrap().entries[0].location,
            PlaylistLocation::Local(fixture.path("песня.ogg"))
        );

        std::fs::write(&playlist_path, b"caf\xe9.mp3\n").unwrap();
        assert_eq!(
            Playlist::open(&playlist_path).unwrap().entries[0].location,
            PlaylistLocation::Local(fixture.path("cafй.mp3"))
        );

        std::fs::write(&playlist_path, b"latin\x98.mp3\n").unwrap();
        assert_eq!(
            Playlist::open(&playlist_path).unwrap().entries[0].location,
            PlaylistLocation::Local(fixture.path("latin\u{98}.mp3"))
        );
    }

    #[test]
    fn detects_hls_content_for_ffmpeg_instead_of_treating_segments_as_songs() {
        let fixture = Fixture::new();
        let playlist_path = fixture.path("stream.m3u8");
        std::fs::write(
            &playlist_path,
            "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:12\n#EXTINF:6,\nsegment.ts\n",
        )
        .unwrap();
        assert!(Playlist::is_hls(&playlist_path).expect("detect HLS"));
        assert!(
            Playlist::open(&playlist_path)
                .unwrap_err()
                .contains("FFmpeg backend")
        );
        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(playlist_path.clone())
            .expect("route local HLS playlist");
        assert_eq!(expansion.sources.len(), 1);
        assert_eq!(
            expansion.sources[0].path,
            playlist_path.canonicalize().unwrap()
        );
        assert_eq!(registry.backend_id_for(&playlist_path), Some("ffmpeg"));
    }

    fn write_wav(path: &Path) {
        const SAMPLE_RATE: u32 = 8_000;
        const FRAMES: u32 = 80;
        let data_size = FRAMES * 2;
        let mut file = std::fs::File::create(path).expect("create playlist WAV");
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
        for frame in 0..FRAMES {
            file.write_all(&((frame as i16 - 40) * 500).to_le_bytes())
                .unwrap();
        }
    }

    #[test]
    fn registry_recurses_in_order_resolves_cue_numbers_and_reports_skips() {
        let fixture = Fixture::new();
        let first = fixture.path("first.wav");
        let second = fixture.path("second.wav");
        let image = fixture.path("image.wav");
        write_wav(&first);
        write_wav(&second);
        write_wav(&image);

        let cue = fixture.path("album.cue");
        std::fs::write(
            &cue,
            concat!(
                "FILE \"image.wav\" WAVE\n",
                "  TRACK 01 AUDIO\n",
                "    INDEX 01 0\n",
                "  TRACK 07 AUDIO\n",
                "    INDEX 01 40\n",
            ),
        )
        .unwrap();
        let inner = fixture.path("inner.m3u");
        std::fs::write(fixture.path("unsupported.txt"), "not audio").unwrap();
        std::fs::write(
            &inner,
            concat!(
                "first.wav\n",
                "album.cue#07\n",
                "https://example.invalid/radio.mp3\n",
                "missing.flac\n",
                "unsupported.txt\n",
            ),
        )
        .unwrap();
        let outer = fixture.path("outer.pls");
        std::fs::write(&outer, "[playlist]\nFile2=inner.m3u\nFile1=second.wav#0\n").unwrap();

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(outer)
            .expect("expand nested playlists");
        assert_eq!(expansion.sources.len(), 4);
        assert_eq!(expansion.sources[0].path, first.canonicalize().unwrap());
        assert_eq!(expansion.sources[0].subsong, None);
        assert_eq!(expansion.sources[1].path, cue.canonicalize().unwrap());
        assert_eq!(expansion.sources[1].subsong, Some(1));
        assert_eq!(
            expansion.sources[2].remote_url.as_deref(),
            Some("https://example.invalid/radio.mp3")
        );
        assert_eq!(expansion.sources[3].path, second.canonicalize().unwrap());
        assert_eq!(expansion.sources[3].subsong, Some(0));
        assert_eq!(expansion.warnings.len(), 2);
        assert!(expansion.warnings[0].contains("missing.flac"));
        assert!(expansion.warnings[1].contains("No installed decoder backend accepts .txt"));
    }

    #[test]
    fn registry_stops_playlist_cycles_but_keeps_other_tracks() {
        let fixture = Fixture::new();
        let song = fixture.path("song.wav");
        write_wav(&song);
        let first = fixture.path("first.m3u");
        let second = fixture.path("second.m3u");
        std::fs::write(&first, "second.m3u\nsong.wav\n").unwrap();
        std::fs::write(&second, "first.m3u\n").unwrap();

        let registry = DecoderRegistry::new(DecoderSettings::default());
        let expansion = registry
            .expand_detailed(first)
            .expect("expand cyclic playlist safely");
        assert_eq!(expansion.sources.len(), 1);
        assert_eq!(expansion.sources[0].path, song.canonicalize().unwrap());
        assert_eq!(expansion.warnings.len(), 1);
        assert!(expansion.warnings[0].contains("playlist cycle"));
    }
}
