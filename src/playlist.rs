//! Cog-compatible M3U/M3U8 and PLS playlist parsing.

use std::path::{Path, PathBuf};

use encoding_rs::{GB18030, WINDOWS_1251};
use url::Url;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlaylistLocation {
    Local(PathBuf),
    Remote(String),
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
        if line
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("#EXT-X-"))
        {
            return Err(format!(
                "{} is an HLS playlist; Kog's HLS backend is not implemented yet",
                path.display()
            ));
        }
        if !line.starts_with('#') {
            entries.push(line);
        }
    }
    Ok(entries)
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
    fn rejects_hls_content_instead_of_treating_segments_as_songs() {
        let fixture = Fixture::new();
        let playlist_path = fixture.path("stream.m3u8");
        std::fs::write(
            &playlist_path,
            "#EXTM3U\n#EXT-X-MEDIA-SEQUENCE:12\n#EXTINF:6,\nsegment.ts\n",
        )
        .unwrap();
        assert!(
            Playlist::open(&playlist_path)
                .unwrap_err()
                .contains("HLS backend")
        );
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
        assert_eq!(expansion.sources.len(), 3);
        assert_eq!(expansion.sources[0].path, first.canonicalize().unwrap());
        assert_eq!(expansion.sources[0].subsong, None);
        assert_eq!(expansion.sources[1].path, cue.canonicalize().unwrap());
        assert_eq!(expansion.sources[1].subsong, Some(1));
        assert_eq!(expansion.sources[2].path, second.canonicalize().unwrap());
        assert_eq!(expansion.sources[2].subsong, Some(0));
        assert_eq!(expansion.warnings.len(), 3);
        assert!(expansion.warnings[0].contains("network sources"));
        assert!(expansion.warnings[1].contains("missing.flac"));
        assert!(expansion.warnings[2].contains("No installed decoder backend accepts .txt"));
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
